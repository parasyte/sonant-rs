extern crate arrayvec;
extern crate colored;
extern crate cpal;
extern crate failure;
extern crate failure_derive;
extern crate sonant;

use arrayvec::ArrayVec;
use colored::Colorize;
use cpal::{EventLoop, StreamData, UnknownTypeOutputBuffer};
use failure::Fail;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process;

use sonant::Error as SonantError;
use sonant::{Song, Synth};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Missing filename argument")]
    MissingFilename,

    #[fail(display = "Sonant error")]
    SonantError(#[cause] SonantError),

    #[fail(display = "I/O error")]
    IOError(#[cause] io::Error),
}

impl From<SonantError> for Error {
    fn from(e: SonantError) -> Self {
        Error::SonantError(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IOError(e)
    }
}

fn main() {
    handle_errors(player());
}

fn player() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let filename = args.next().ok_or(Error::MissingFilename)?;

    // cpal boilerplate
    let event_loop = EventLoop::new();
    let device = cpal::default_output_device().expect("no output device available");

    let mut supported_formats_range = device
        .supported_output_formats()
        .expect("error while querying formats");
    let format = supported_formats_range
        .next()
        .expect("no supported format?!")
        .with_max_sample_rate();

    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop.play_stream(stream_id);

    // Read the file
    let mut file = File::open(filename)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let mut synth = Synth::new(&song, None, format.sample_rate.0 as f32)
        .map(ArrayVec::from)
        .flatten()
        .peekable();

    // cpal event loop; this is the actual audio player
    event_loop.run(move |_stream_id, stream_data| match stream_data {
        StreamData::Output {
            buffer: UnknownTypeOutputBuffer::U16(mut buffer),
        } => {
            let max = i16::max_value() as f32;
            for (elem, sample) in buffer.iter_mut().zip(synth.by_ref()) {
                *elem = (sample * max + max) as u16;
            }
            if synth.peek() == None {
                process::exit(0);
            }
        }
        StreamData::Output {
            buffer: UnknownTypeOutputBuffer::I16(mut buffer),
        } => {
            for (elem, sample) in buffer.iter_mut().zip(synth.by_ref()) {
                *elem = (sample * i16::max_value() as f32) as i16;
            }
            if synth.peek() == None {
                process::exit(0);
            }
        }
        StreamData::Output {
            buffer: UnknownTypeOutputBuffer::F32(mut buffer),
        } => {
            for (elem, sample) in buffer.iter_mut().zip(synth.by_ref()) {
                *elem = sample;
            }
            if synth.peek() == None {
                process::exit(0);
            }
        }
        _ => (),
    });
}

pub fn handle_errors<F>(result: Result<(), F>)
where
    F: Fail,
{
    match result {
        Err(e) => {
            let stderr = io::stderr();
            let mut stderr = stderr.lock();

            writeln!(stderr, "{} {}", "error:".red(), e);

            for cause in Fail::iter_causes(&e) {
                writeln!(stderr, "{} {}", "caused by:".bright_red(), cause).ok();
            }

            if let Some(backtrace) = e.backtrace() {
                writeln!(stderr, "{:?}", backtrace).ok();
            }

            process::exit(1);
        }
        Ok(()) => (),
    };
}
