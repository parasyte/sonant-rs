extern crate arrayvec;
extern crate colored;
extern crate failure;
extern crate failure_derive;
extern crate riff_wave;
extern crate sonant;

use arrayvec::ArrayVec;
use colored::Colorize;
use failure::Fail;
use riff_wave::{WaveWriter, WriteError};
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::process;

use sonant::Error as SonantError;
use sonant::{Song, Synth};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Missing snt-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingSntFilename,

    #[fail(display = "Missing wav-file argument\nUsage: player <snt-file> <wav-file>")]
    MissingWavFilename,

    #[fail(display = "Sonant error")]
    SonantError(#[cause] SonantError),

    #[fail(display = "I/O error")]
    IOError(#[cause] io::Error),

    #[fail(display = "Wave writer error")]
    WriterError(#[cause] WriteError),
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

impl From<WriteError> for Error {
    fn from(e: WriteError) -> Self {
        Error::WriterError(e)
    }
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

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let synth = Synth::new(&song, None, 44100.0 as f32)
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
