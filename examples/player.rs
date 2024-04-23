#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::cast_precision_loss)]
#![forbid(unsafe_code)]

use arrayvec::ArrayVec;
use byteorder::{ByteOrder, NativeEndian};
use colored::Colorize;
use cpal::traits::{DeviceTrait as _, HostTrait as _, StreamTrait as _};
use cpal::{FromSample, SampleFormat, SizedSample};
use error_iter::ErrorIter as _;
use sonant::{Song, Synth};
use std::process::ExitCode;
use std::sync::mpsc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing filename argument")]
    MissingFilename,

    #[error("Sonant error")]
    Sonant(#[from] sonant::Error),

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("CPAL audio stream config error")]
    AudioConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("CPAL audio stream builder error")]
    AudioStream(#[from] cpal::BuildStreamError),

    #[error("CPAL audio stream play error")]
    AudioPlay(#[from] cpal::PlayStreamError),
}

fn main() -> ExitCode {
    match player() {
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

fn player() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let filename = args.next().ok_or(Error::MissingFilename)?;

    // cpal boilerplate
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let stream_config = device.default_output_config()?;
    let sample_rate = stream_config.sample_rate();
    let format = stream_config.sample_format();

    // Read the file
    let data = std::fs::read(filename)?;

    // Create a seed for the PRNG
    let mut seed = [0_u8; 16];
    getrandom::getrandom(&mut seed).expect("failed to getrandom");
    let seed = (
        NativeEndian::read_u64(&seed[0..8]),
        NativeEndian::read_u64(&seed[8..16]),
    );

    // Load a sonant song and create a synth
    let song = Song::from_slice(&data)?;
    let synth = Synth::new(&song, seed, sample_rate.0 as f32);

    match format {
        SampleFormat::I8 => run::<i8>(&device, &stream_config.into(), synth),
        SampleFormat::I16 => run::<i16>(&device, &stream_config.into(), synth),
        SampleFormat::I32 => run::<i32>(&device, &stream_config.into(), synth),
        SampleFormat::I64 => run::<i64>(&device, &stream_config.into(), synth),
        SampleFormat::U8 => run::<u8>(&device, &stream_config.into(), synth),
        SampleFormat::U16 => run::<u16>(&device, &stream_config.into(), synth),
        SampleFormat::U32 => run::<u32>(&device, &stream_config.into(), synth),
        SampleFormat::U64 => run::<u64>(&device, &stream_config.into(), synth),
        SampleFormat::F32 => run::<f32>(&device, &stream_config.into(), synth),
        SampleFormat::F64 => run::<f64>(&device, &stream_config.into(), synth),
        sample_format => panic!("Unsupported sample format '{}'", sample_format),
    }
}

fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig, synth: Synth) -> Result<(), Error>
where
    T: SizedSample + FromSample<f32>,
{
    // Create a channel so the audio thread can request samples
    let (audio_tx, audio_rx) = mpsc::sync_channel(10);

    // Create the audio thread
    let stream = device.build_output_stream(
        config,
        move |buffer: &mut [T], _: &cpal::OutputCallbackInfo| {
            let (tx, rx) = mpsc::sync_channel(1);

            // Request samples from the main thread
            audio_tx.send((buffer.len(), tx)).unwrap();
            let samples = rx.recv().unwrap();

            for (elem, sample) in buffer.iter_mut().zip(samples) {
                *elem = T::from_sample(sample);
            }
        },
        |err| eprintln!("an error occurred on stream: {err}"),
        None,
    )?;
    stream.play()?;

    let mut synth = synth.flat_map(ArrayVec::from);

    // Send samples requested by the audio thread.
    while let Ok((len, tx)) = audio_rx.recv() {
        let samples = synth.by_ref().take(len).collect::<Vec<_>>();
        let done = samples.is_empty();
        tx.send(samples).unwrap();
        if done {
            break;
        }
    }

    Ok(())
}
