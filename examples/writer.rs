#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![forbid(unsafe_code)]

use arrayvec::ArrayVec;
use byteorder::{ByteOrder, NativeEndian};
use colored::Colorize;
use error_iter::ErrorIter as _;
use riff_wave::{WaveWriter, WriteError};
use std::io::{self, BufWriter, Read};
use std::{fs::File, process::ExitCode};
use thiserror::Error;

use sonant::{Error as SonantError, Song, Synth};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing snt-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingSntFilename,

    #[error("Missing wav-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingWavFilename,

    #[error("Sonant error")]
    Sonant(#[from] SonantError),

    #[error("I/O error")]
    IO(#[from] io::Error),

    #[error("Wave writer error")]
    Writer(#[from] WriteError),
}

fn main() -> ExitCode {
    match writer() {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);

            for cause in e.sources().skip(1) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
            }

            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

fn writer() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let snt_filename = args.next().ok_or(Error::MissingSntFilename)?;
    let wav_filename = args.next().ok_or(Error::MissingWavFilename)?;

    // Read the snt file
    let mut file = File::open(snt_filename)?;
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
    let synth = Synth::new(&song, seed, 44100.0)
        .flat_map(ArrayVec::from)
        .peekable();

    // Write the wav file
    let file = File::create(wav_filename)?;
    let writer = BufWriter::new(file);
    let mut wave_writer = WaveWriter::new(2, 44100, 16, writer)?;

    for sample in synth {
        let sample = (sample * f32::from(i16::MAX)).round() as i16;
        wave_writer.write_sample_i16(sample)?;
    }

    Ok(())
}
