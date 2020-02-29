#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::{self, Read};
use std::process;

use arrayvec::ArrayVec;
use byteorder::{ByteOrder, NativeEndian};
use colored::Colorize;
use cpal::traits::{DeviceTrait, EventLoopTrait, HostTrait};
use cpal::{StreamData, UnknownTypeOutputBuffer};
use error_iter::ErrorIter;
use thiserror::Error;

use sonant::{Error as SonantError, Song, Synth};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing filename argument")]
    MissingFilename,

    #[error("Sonant error")]
    Sonant(#[from] SonantError),

    #[error("I/O error")]
    IO(#[from] io::Error),
}

impl ErrorIter for Error {}

fn main() {
    handle_errors(player());
}

fn player() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let filename = args.next().ok_or(Error::MissingFilename)?;

    // cpal boilerplate
    let host = cpal::default_host();
    let event_loop = host.event_loop();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let mut supported_formats_range = device
        .supported_output_formats()
        .expect("error while querying formats");
    let format = supported_formats_range
        .next()
        .expect("no supported format?!")
        .with_max_sample_rate();

    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop
        .play_stream(stream_id)
        .expect("failed to play_stream");

    // Read the file
    let mut file = File::open(filename)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    // Create a seed for the PRNG
    let mut seed = [0_u8; 16];
    getrandom::getrandom(&mut seed).expect("failed to getrandom");
    let seed = (
        NativeEndian::read_u64(&seed[0..8]),
        NativeEndian::read_u64(&seed[8..16]),
    );

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let mut synth = Synth::new(&song, seed, format.sample_rate.0 as f32)
        .flat_map(ArrayVec::from)
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
                let max = f32::from(i16::max_value());
                for (elem, sample) in buffer.iter_mut().zip(synth.by_ref()) {
                    *elem = sample.mul_add(max, max).round() as u16;
                }
                if synth.peek() == None {
                    process::exit(0);
                }
            }
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::I16(mut buffer),
            } => {
                for (elem, sample) in buffer.iter_mut().zip(synth.by_ref()) {
                    *elem = (sample * f32::from(i16::max_value())).round() as i16;
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

pub fn handle_errors<E>(result: Result<(), E>)
where
    E: std::error::Error + ErrorIter + 'static,
{
    match result {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);

            for cause in e.chain().skip(1) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
            }

            process::exit(1);
        }
        Ok(()) => (),
    };
}
