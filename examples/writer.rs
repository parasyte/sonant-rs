use std::fs::File;
use std::io::{self, BufWriter, Read};
use std::process;

use arrayvec::ArrayVec;
use byteorder::{ByteOrder, LittleEndian};
use colored::Colorize;
use riff_wave::{WaveWriter, WriteError};
use thiserror::Error;

use sonant::{errors::iter_sources, Error as SonantError, Song, Synth};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing snt-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingSntFilename,

    #[error("Missing wav-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingWavFilename,

    #[error("Sonant error")]
    SonantError(#[from] SonantError),

    #[error("I/O error")]
    IOError(#[from] io::Error),

    #[error("Wave writer error")]
    WriterError(#[from] WriteError),
}

fn main() {
    handle_errors(writer());
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
        LittleEndian::read_u64(&seed[0..8]),
        LittleEndian::read_u64(&seed[8..16]),
    );

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let synth = Synth::new(&song, seed, 44100.0 as f32)
        .map(ArrayVec::from)
        .flatten()
        .peekable();

    // Write the wav file
    let file = File::create(wav_filename)?;
    let writer = BufWriter::new(file);
    let mut wave_writer = WaveWriter::new(2, 44100, 16, writer)?;

    for sample in synth {
        let sample = (sample * i16::max_value() as f32) as i16;
        wave_writer.write_sample_i16(sample)?;
    }

    Ok(())
}

pub fn handle_errors<E>(result: Result<(), E>)
where
    E: std::error::Error + 'static,
{
    match result {
        Err(e) => {
            eprintln!("{} {}", "error:".red(), e);

            for cause in iter_sources(&e) {
                eprintln!("{} {}", "caused by:".bright_red(), cause);
            }

            process::exit(1);
        }
        Ok(()) => (),
    };
}
