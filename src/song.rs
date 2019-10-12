use arrayvec::ArrayVec;
use byteorder::{ByteOrder, LittleEndian};
use std::f32;
use std::num::Wrapping as w;

#[cfg(feature = "std")]
use failure::Fail;

use std::fmt;

use crate::consts::*;

/// Possible errors.
#[derive(Debug)]
#[cfg_attr(feature = "std", derive(Fail))]
pub enum Error {
    #[cfg_attr(feature = "std", fail(display = "Incorrect file length"))]
    FileLength,

    #[cfg_attr(feature = "std", fail(display = "Invalid waveform"))]
    InvalidWaveform,

    #[cfg_attr(feature = "std", fail(display = "Invalid filter"))]
    InvalidFilter,
}

/// A `Song` contains a list of up to 8 `Instruments` and defines the sample
/// length for each row (in the tracker).
#[derive(Debug)]
pub struct Song {
    pub(crate) instruments: [Instrument; NUM_INSTRUMENTS],
    pub(crate) seq_length: usize, // Total number of patterns to play
    pub(crate) quarter_note_length: u32, // In samples
}

/// Contains two `Oscillator`s, a simple `Envelope`, `Effects` and `LFO`. The
/// tracker `Sequence` (up to 48) is defined here, as well as the tracker
/// `Patterns` (up to 10).
pub(crate) struct Instrument {
    pub(crate) osc: [Oscillator; 2],          // Oscillators 0 and 1
    pub(crate) noise_fader: f32,              // Noise Oscillator
    pub(crate) env: Envelope,                 // Envelope
    pub(crate) fx: Effects,                   // Effects
    pub(crate) lfo: LFO,                      // Low-Frequency Oscillator
    pub(crate) seq: [usize; SEQUENCE_LENGTH], // Sequence of patterns
    pub(crate) pat: [Pattern; NUM_PATTERNS],  // List of available patterns
}

/// The `Oscillator` defines the `Instrument` sound.
#[derive(Debug)]
pub(crate) struct Oscillator {
    pub(crate) octave: u8,         // Octave knob
    pub(crate) detune_freq: u8,    // Detune frequency
    pub(crate) detune: f32,        // Detune knob
    pub(crate) envelope: bool,     // Envelope toggle
    pub(crate) volume: f32,        // Volume knob
    pub(crate) waveform: Waveform, // Wave form
}

/// `Envelope` is for compressing the sample amplitude over time.
/// (E.g. raising and lowering volume.)
#[derive(Debug)]
pub(crate) struct Envelope {
    pub(crate) attack: u32,  // Attack
    pub(crate) sustain: u32, // Sustain
    pub(crate) release: u32, // Release
    pub(crate) master: f32,  // Master volume knob
}

/// The `Effects` provide filtering, resonance, and panning.
#[derive(Debug)]
pub(crate) struct Effects {
    pub(crate) filter: Filter,    // Hi, lo, bandpass, or notch toggle
    pub(crate) freq: f32,         // FX Frequency
    pub(crate) resonance: f32,    // FX Resonance
    pub(crate) delay_time: u8,    // Delay time
    pub(crate) delay_amount: f32, // Delay amount
    pub(crate) pan_freq: u8,      // Panning frequency
    pub(crate) pan_amount: f32,   // Panning amount
}

/// `LFO` is a Low-Frequency Oscillator. It can be used to adjust the frequency
/// of `Oscillator` 0 and `Effects` over time.
#[derive(Debug)]
pub(crate) struct LFO {
    pub(crate) osc0_freq: bool,    // Modify Oscillator 0 frequency (FM) toggle
    pub(crate) fx_freq: bool,      // Modify FX frequency toggle
    pub(crate) freq: u8,           // LFO frequency
    pub(crate) amount: f32,        // LFO amount
    pub(crate) waveform: Waveform, // LFO waveform
}

#[cfg(feature = "std")]
impl fmt::Debug for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Instrument {{ ")?;
        write!(f, "osc: {:?}, ", self.osc)?;
        write!(f, "env: {:?}, ", self.env)?;
        write!(f, "fx: {:?}, ", self.fx)?;
        write!(f, "lfo: {:?}, ", self.lfo)?;
        write!(f, "pat: {:?}, ", self.pat)?;
        write!(f, "seq: [")?;
        let mut iter = self.seq.iter();
        if let Some(i) = iter.next() {
            write!(f, "{:?}", i)?;
        }
        for i in iter {
            write!(f, ", {:?}", i)?;
        }
        write!(f, "] }}")
    }
}

#[cfg(not(feature = "std"))]
impl fmt::Debug for Instrument {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

/// Contains the tracker notes (up to 32).
#[derive(Debug)]
pub(crate) struct Pattern {
    pub(crate) notes: [u8; PATTERN_LENGTH],
}

/// Available filters.
#[derive(Debug)]
pub(crate) enum Filter {
    None,
    HighPass,
    LowPass,
    BandPass,
    Notch,
}

/// Available wave forms.
#[derive(Debug)]
pub(crate) enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

impl Song {
    /// Create a new `Song` from a byte slice.
    ///
    /// ```rust
    /// Song::from_slice(include_bytes!("/some/file.snt"))
    /// ```
    pub fn from_slice(slice: &[u8]) -> Result<Song, Error> {
        if slice.len() != SONG_LENGTH {
            return Err(Error::FileLength);
        }

        fn parse_waveform(waveform: u8) -> Result<Waveform, Error> {
            Ok(match waveform {
                0 => Waveform::Sine,
                1 => Waveform::Square,
                2 => Waveform::Saw,
                3 => Waveform::Triangle,
                _ => Err(Error::InvalidWaveform)?,
            })
        }

        let load_oscillator = |i, o| -> Result<Oscillator, Error> {
            let i = i + o * OSCILLATOR_LENGTH;
            let octave = ((w::<u8>(slice[i + 0]) - w(8)) * w(12)).0;
            let detune_freq = slice[i + 1];
            let detune = slice[i + 2] as f32 * 0.2 / 255.0 + 1.0;
            let envelope = slice[i + 3] != 0;
            let volume = slice[i + 4] as f32 / 255.0;
            let waveform = parse_waveform(slice[i + 5])?;

            Ok(Oscillator {
                octave,
                detune_freq,
                detune,
                envelope,
                volume,
                waveform,
            })
        };

        let load_envelope = |i| -> Envelope {
            let attack = LittleEndian::read_u32(&slice[i + 0..i + 4]);
            let sustain = LittleEndian::read_u32(&slice[i + 4..i + 8]);
            let release = LittleEndian::read_u32(&slice[i + 8..i + 12]);
            let master = slice[i + 12] as f32 * 156.0;

            Envelope {
                attack,
                sustain,
                release,
                master,
            }
        };

        let load_effects = |i| -> Result<Effects, Error> {
            let filter = match slice[i + 0] {
                0 => Filter::None,
                1 => Filter::HighPass,
                2 => Filter::LowPass,
                3 => Filter::BandPass,
                4 => Filter::Notch,
                _ => Err(Error::InvalidFilter)?,
            };
            let i = i + 3;
            let freq = f32::from_bits(LittleEndian::read_u32(&slice[i..i + 4]));
            let resonance = slice[i + 4] as f32 / 255.0;
            let delay_time = slice[i + 5];
            let delay_amount = slice[i + 6] as f32 / 255.0;
            let pan_freq = slice[i + 7];
            let pan_amount = slice[i + 8] as f32 / 512.0;

            Ok(Effects {
                filter,
                freq,
                resonance,
                delay_time,
                delay_amount,
                pan_freq,
                pan_amount,
            })
        };

        let load_lfo = |i| -> Result<LFO, Error> {
            let osc0_freq = slice[i + 0] != 0;
            let fx_freq = slice[i + 1] != 0;
            let freq = slice[i + 2];
            let amount = slice[i + 3] as f32 / 512.0;
            let waveform = parse_waveform(slice[i + 4])?;

            Ok(LFO {
                osc0_freq,
                fx_freq,
                freq,
                amount,
                waveform,
            })
        };

        let load_sequence = |i| -> [usize; SEQUENCE_LENGTH] {
            let mut seq = [0; SEQUENCE_LENGTH];

            slice[i..i + SEQUENCE_LENGTH]
                .iter()
                .enumerate()
                .for_each(|(i, &x)| {
                    seq[i] = x as usize;
                });

            seq
        };

        let load_pattern = |i, p| -> Pattern {
            let i = i + p * PATTERN_LENGTH;
            let mut notes = [0; PATTERN_LENGTH];
            notes.copy_from_slice(&slice[i..i + PATTERN_LENGTH]);

            Pattern { notes }
        };

        let load_instrument = |i| -> Result<Instrument, Error> {
            let i = HEADER_LENGTH + i * INSTRUMENT_LENGTH;
            let osc = [load_oscillator(i, 0)?, load_oscillator(i, 1)?];

            let i = i + OSCILLATOR_LENGTH * 2;
            let noise_fader = slice[i] as f32 / 255.0;

            let i = i + 4;
            let env = load_envelope(i);

            let i = i + 13;
            let fx = load_effects(i)?;

            let i = i + 12;
            let lfo = load_lfo(i)?;

            let i = i + 5;
            let seq = load_sequence(i);

            let i = i + SEQUENCE_LENGTH;
            let mut pat = ArrayVec::new();
            for j in 0..NUM_PATTERNS {
                pat.push(load_pattern(i, j));
            }
            let pat = pat.into_inner().unwrap();

            Ok(Instrument {
                osc,
                noise_fader,
                env,
                fx,
                lfo,
                seq,
                pat,
            })
        };

        // Get quarter note length and eighth note length (in samples)
        // This properly handles odd quarter note lengths
        let quarter_note_length = LittleEndian::read_u32(&slice[..HEADER_LENGTH]);
        let quarter_note_length = quarter_note_length - (quarter_note_length % 2);

        let seq_length = slice[HEADER_LENGTH + INSTRUMENT_LENGTH * 8] as usize;
        let mut instruments = ArrayVec::new();
        for i in 0..NUM_INSTRUMENTS {
            instruments.push(load_instrument(i)?);
        }
        let instruments = instruments.into_inner().unwrap();

        Ok(Song {
            instruments,
            seq_length,
            quarter_note_length,
        })
    }
}
