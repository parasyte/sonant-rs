use arrayvec::ArrayVec;
use byteorder::{ByteOrder, LittleEndian};
use colored::Colorize;
use cpal::{StreamData, UnknownTypeOutputBuffer};
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use failure::Fail;
use std::fs::File;
use std::io::{self, Read};
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
    let host = cpal::default_host();
    let event_loop = host.event_loop();
    let device = host.default_output_device().expect("no output device available");

    let mut supported_formats_range = device
        .supported_output_formats()
        .expect("error while querying formats");
    let format = supported_formats_range
        .next()
        .expect("no supported format?!")
        .with_max_sample_rate();

    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop.play_stream(stream_id).expect("failed to play_stream");

    // Read the file
    let mut file = File::open(filename)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    // Create a seed for the PRNG
    let mut seed = [0_u8; 16];
    getrandom::getrandom(&mut seed).expect("failed to getrandom");
    let seed = (
        LittleEndian::read_u64(&seed[0..8]),
        LittleEndian::read_u64(&seed[8..16]),
    );

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let mut synth = Synth::new(&song, seed, format.sample_rate.0 as f32)
        .map(ArrayVec::from)
        .flatten()
        .peekable();

    // cpal event loop; this is the actual audio player
    event_loop.run(move |stream_id, stream_result| {
        let stream_data = match stream_result {
            Ok(data) => data,
            Err(err) => {
                eprintln!("an error occurred on stream {:?}: {}", stream_id, err);
                return;
            }
        };

        match stream_data {
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
        }
    });
}

pub fn handle_errors<F>(result: Result<(), F>)
where
    F: Fail,
{
    match result {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);

            for cause in Fail::iter_causes(&e) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
            }

            if let Some(backtrace) = e.backtrace() {
                eprintln!("{:?}", backtrace);
            }

            process::exit(1);
        }
        Ok(()) => (),
    };
}
