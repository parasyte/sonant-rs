use arrayvec::ArrayVec;
#[allow(unused_imports)]
use libm::F32Ext;
use randomize::PCG32;
use std::f32::consts::PI;
use std::num::Wrapping as w;

use consts::*;
use song::{Envelope, Filter, Instrument, Song, Waveform};

/// The main struct for audio synthesis. `Synth` implements `Iterator`, so
/// calling the `next` method on it will generate the next sample.
///
/// Currently only generates 2-channel f32 samples at the given `sample_rate`.
#[derive(Debug)]
pub struct Synth<'a> {
    song: &'a Song,
    random: PCG32,
    sample_rate: f32,
    sample_ratio: f32,
    quarter_note_length: u32,
    eighth_note_length: u32,

    // TODO: Support seamless loops

    // Iterator state
    seq_count: usize,
    note_count: usize,
    sample_count: u32,
    tracks: [TrackState; NUM_INSTRUMENTS],
}

/// Iterator state for a single instrument track.
#[derive(Debug)]
struct TrackState {
    env: Envelope,

    // Max simultaneous notes per track
    notes: [Note; MAX_OVERLAPPING_NOTES],

    delay_samples: u32,
    delay_count: u32,

    // Static frequencies
    pan_freq: f32,
    lfo_freq: f32,
}

/// Data structure for quarter notes, which includes the pitch and sample
/// counter reference for waveform modulation. It also contains state for sample
/// synthesis and filtering.
#[derive(Debug)]
struct Note {
    pitch: u8,
    sample_count: u32,
    volume: f32,
    swap_stereo: bool,

    // Iterator state
    osc_freq: [f32; 2],
    osc_time: [f32; 2],
    low: f32,
    band: f32,
}

/// Sine wave generator
fn osc_sin(value: f32) -> f32 {
    ((value + 0.5) * PI * 2.0).sin()
}

/// Square wave generator
fn osc_square(value: f32) -> f32 {
    if osc_sin(value) < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Saw wave generator
fn osc_saw(value: f32) -> f32 {
    (1.0 - value.fract()) - 0.5
}

/// Triangle wave generator
fn osc_tri(value: f32) -> f32 {
    let v2 = value.fract() * 4.0;

    if v2 < 2.0 {
        v2 - 1.0
    } else {
        3.0 - v2
    }
}

/// Get a `note` frequency on the exponential scale defined by reference
/// frequency `ref_freq` and reference pitch `ref_pitch`, using the interval
/// `semitone`.
fn get_frequency(ref_freq: f32, semitone: f32, note: u8, ref_pitch: u8) -> f32 {
    ref_freq * semitone.powf(note as f32 - ref_pitch as f32)
}

/// Get the absolute frequency for a note value on the 12-TET scale.
fn get_note_frequency(note: u8) -> f32 {
    const SEMITONE: f32 = 1.059463094; // Twelfth root of 2
    get_frequency(1.0 / 256.0, SEMITONE, note, 128)
}

/// Get a sample from the waveform generator at time `t`
fn get_osc_output(waveform: &Waveform, t: f32) -> f32 {
    match waveform {
        Waveform::Sine => osc_sin(t),
        Waveform::Square => osc_square(t),
        Waveform::Saw => osc_saw(t),
        Waveform::Triangle => osc_tri(t),
    }
}

impl TrackState {
    fn new() -> Self {
        let mut notes = ArrayVec::new();
        for _ in 0..MAX_OVERLAPPING_NOTES {
            notes.push(Note::new(0, 0, 0.0, false));
        }
        let notes = notes.into_inner().unwrap();

        TrackState {
            env: Envelope {
                attack: 0,
                sustain: 0,
                release: 0,
                master: 0.0,
            },
            notes,
            delay_samples: 0,
            delay_count: 0,
            pan_freq: 0.0,
            lfo_freq: 0.0,
        }
    }
}

impl Note {
    fn new(pitch: u8, sample_count: u32, volume: f32, swap_stereo: bool) -> Self {
        Note {
            pitch,
            sample_count,
            volume,
            swap_stereo,
            osc_freq: [0.0; 2],
            osc_time: [0.0; 2],
            low: 0.0,
            band: 0.0,
        }
    }
}

impl<'a> Synth<'a> {
    /// Create a `Synth` that will play the provided `Song`.
    /// The optional seed will be used for the noise generator.
    /// `Synth` implements `Iterator` and generates two stereo samples at a time.
    ///
    /// ```rust
    /// let synth = Synth::new(song, None, 44100.0);
    /// for (sample_l, sample_r) in synth {
    ///     // Do something with the samples
    /// }
    /// ```
    pub fn new(song: &'a Song, seed: (u64, u64), sample_rate: f32) -> Self {
        let random = PCG32::seed(seed.0, seed.1);
        let sample_ratio = sample_rate / 44100.0;
        let quarter_note_length = (sample_ratio * song.quarter_note_length as f32) as u32;
        let eighth_note_length = quarter_note_length / 2;

        let mut synth = Synth {
            song,
            random,
            sample_rate,
            sample_ratio,
            quarter_note_length,
            eighth_note_length,
            seq_count: 0,
            sample_count: 0,
            note_count: 0,
            tracks: Self::load_tracks(
                &song,
                sample_ratio,
                quarter_note_length as f32 * sample_ratio,
                eighth_note_length,
            ),
        };
        synth.load_notes();

        synth
    }

    /// Load the static state for each track.
    fn load_tracks(
        song: &Song,
        sample_ratio: f32,
        quarter_note_length: f32,
        eighth_note_length: u32,
    ) -> [TrackState; NUM_INSTRUMENTS] {
        let mut tracks = ArrayVec::<[_; NUM_INSTRUMENTS]>::new();
        for _ in 0..NUM_INSTRUMENTS {
            tracks.push(TrackState::new());
        }
        let mut tracks = tracks.into_inner().unwrap();

        for (i, inst) in song.instruments.iter().enumerate() {
            // Configure attack, sustain, and release
            tracks[i].env.attack = (inst.env.attack as f32 * sample_ratio) as u32;
            tracks[i].env.sustain = (inst.env.sustain as f32 * sample_ratio) as u32;
            tracks[i].env.release = (inst.env.release as f32 * sample_ratio) as u32;

            // Configure delay
            tracks[i].delay_samples = inst.fx.delay_time as u32 * eighth_note_length;
            tracks[i].delay_count = if inst.fx.delay_amount == 0.0 {
                // Special case for zero repeats
                0
            } else if inst.fx.delay_amount == 1.0 {
                // Special case for infinite repeats
                u32::max_value()
            } else if tracks[i].delay_samples == 0 {
                // Special case for zero-delay time: only repeat once
                1
            } else {
                // This gets the number of iterations required for the note
                // volume to drop below the audible threashold.
                (256.0_f32).log(1.0 / inst.fx.delay_amount) as u32
            };

            // Set LFO and panning frequencies
            tracks[i].lfo_freq = get_frequency(1.0, 2.0, inst.lfo.freq, 8) / quarter_note_length;
            tracks[i].pan_freq = get_frequency(1.0, 2.0, inst.fx.pan_freq, 8) / quarter_note_length;
        }

        tracks
    }

    /// Load the next set of notes into the iterator state.
    fn load_notes(&mut self) {
        let seq_count = self.seq_count;
        if seq_count > self.song.seq_length {
            return;
        }

        for i in 0..self.song.instruments.len() {
            // Add the note
            let note_count = self.note_count;
            self.add_note(i, seq_count, note_count, 1.0, false);
        }
    }

    /// Load delayed notes into the iterator state.
    fn load_delayed_notes(&mut self) {
        for (i, inst) in self.song.instruments.iter().enumerate() {
            for round in 1..=self.tracks[i].delay_count {
                // Compute the delay position
                let delay = self.tracks[i].delay_samples * round;
                if delay > self.sample_count {
                    continue;
                }

                // Seek to the delayed note, and ensure it's aligned to the quarter note
                let position = self.sample_count - delay;
                if position % self.quarter_note_length != 0 {
                    continue;
                }

                // Convert position into seq_count and note_count
                let pattern_length = self.quarter_note_length * PATTERN_LENGTH as u32;
                let seq_count = (position / pattern_length) as usize;
                if seq_count > self.song.seq_length {
                    continue;
                }
                let note_count = ((position % pattern_length) / self.quarter_note_length) as usize;

                // Add the note
                let volume = inst.fx.delay_amount.powf(round as f32);
                self.add_note(i, seq_count, note_count, volume, round % 2 == 1);
            }
        }
    }

    /// Get the index of the first empty note in the given `notes` slice.
    fn get_note_slot(notes: &[Note]) -> usize {
        // Find the first empty note
        match notes.iter().enumerate().find(|(_, x)| x.pitch == 0) {
            Some((i, _)) => i,
            // If that fails, use the oldest note
            None => {
                let iter = notes.iter().enumerate();
                iter.min_by_key(|(_, x)| x.sample_count).unwrap().0
            }
        }
    }

    /// Add a note to track `i`.
    fn add_note(
        &mut self,
        i: usize,
        seq_count: usize,
        note_count: usize,
        volume: f32,
        swap_stereo: bool,
    ) {
        let inst = &self.song.instruments[i];

        // Get the pattern index
        let p = inst.seq[seq_count];
        if p == 0 {
            return;
        }

        // Get the pattern
        let pattern = &inst.pat[p - 1];

        // Get the note pitch
        let pitch = pattern.notes[note_count];
        if pitch == 0 {
            return;
        }

        // Create a new note
        let j = Self::get_note_slot(&self.tracks[i].notes);
        self.tracks[i].notes[j] = Note::new(pitch, self.sample_count, volume, swap_stereo);

        // Set oscillator frequencies
        let pitch = w(self.tracks[i].notes[j].pitch);
        for o in 0..2 {
            let pitch = (pitch + w(inst.osc[o].octave) + w(inst.osc[o].detune_freq)).0;
            self.tracks[i].notes[j].osc_freq[o] =
                get_note_frequency(pitch) * inst.osc[o].detune / self.sample_ratio;
        }
    }

    /// Envelope
    fn env(position: u32, inst_env: &Envelope) -> Option<(f32, f32)> {
        let attack = inst_env.attack;
        let sustain = inst_env.sustain;
        let release = inst_env.release;

        let mut env = 1.0;

        if position < attack {
            env = position as f32 / attack as f32;
        } else if position >= attack + sustain + release {
            return None;
        } else if position >= attack + sustain {
            let pos = (position - attack - sustain) as f32;
            env -= pos / release as f32;
        }

        Some((env, env * env))
    }

    /// Oscillator 0
    fn osc0(&mut self, inst: &Instrument, i: usize, j: usize, lfo: f32, env_sq: f32) -> f32 {
        let r = get_osc_output(&inst.osc[0].waveform, self.tracks[i].notes[j].osc_time[0]);
        let mut t = self.tracks[i].notes[j].osc_freq[0];

        if inst.lfo.osc0_freq {
            t += lfo;
        }
        if inst.osc[0].envelope {
            t *= env_sq;
        }
        self.tracks[i].notes[j].osc_time[0] += t;

        r * inst.osc[0].volume
    }

    /// Oscillator 1
    fn osc1(&mut self, inst: &Instrument, i: usize, j: usize, env_sq: f32) -> f32 {
        let r = get_osc_output(&inst.osc[1].waveform, self.tracks[i].notes[j].osc_time[1]);
        let mut t = self.tracks[i].notes[j].osc_freq[1];

        if inst.osc[1].envelope {
            t *= env_sq;
        }
        self.tracks[i].notes[j].osc_time[1] += t;

        r * inst.osc[1].volume
    }

    /// Filters
    fn filters(&mut self, inst: &Instrument, i: usize, j: usize, lfo: f32, sample: f32) -> f32 {
        let mut f = inst.fx.freq * self.sample_ratio;

        if inst.lfo.fx_freq {
            f *= lfo;
        }
        f = (f * PI / self.sample_rate).sin() * 1.5;

        let low = self.tracks[i].notes[j].low + f * self.tracks[i].notes[j].band;
        let high = inst.fx.resonance * (sample - self.tracks[i].notes[j].band) - low;
        let band = self.tracks[i].notes[j].band + f * high;

        self.tracks[i].notes[j].low = low;
        self.tracks[i].notes[j].band = band;

        let sample = match inst.fx.filter {
            Filter::None => sample,
            Filter::HighPass => high,
            Filter::LowPass => low,
            Filter::BandPass => band,
            Filter::Notch => low + high,
        } * inst.env.master;

        sample
    }

    /// Generate samples for 2 channels using the given instrument.
    fn generate_samples(
        &mut self,
        inst: &Instrument,
        i: usize,
        j: usize,
        position: f32,
    ) -> Option<[f32; NUM_CHANNELS]> {
        // Envelope
        let note_sample_count = self.tracks[i].notes[j].sample_count;
        let (env, env_sq) =
            match Self::env(self.sample_count - note_sample_count, &self.tracks[i].env) {
                Some((env, env_sq)) => (env, env_sq),
                None => return None,
            };

        // LFO
        let lfo_freq = self.tracks[i].lfo_freq;
        let lfo = get_osc_output(&inst.lfo.waveform, lfo_freq * position)
            * inst.lfo.amount
            * self.sample_ratio
            + 0.5;

        // Oscillator 0
        let mut sample = self.osc0(inst, i, j, lfo, env_sq);

        // Oscillator 1
        sample += self.osc1(inst, i, j, env_sq);

        // Noise oscillator
        sample += osc_sin(randomize::f32_closed(self.random.next_u32())) * inst.noise_fader * env;

        // Envelope
        sample *= env * self.tracks[i].notes[j].volume;

        // Filters
        sample += self.filters(inst, i, j, lfo, sample);

        let pan_freq = self.tracks[i].pan_freq;
        let pan_t = osc_sin(pan_freq * position) * inst.fx.pan_amount * self.sample_ratio + 0.5;

        if self.tracks[i].notes[j].swap_stereo {
            Some([sample * (1.0 - pan_t), sample * pan_t])
        } else {
            Some([sample * pan_t, sample * (1.0 - pan_t)])
        }
    }

    /// Update the sample generator. This is the main workhorse of the
    /// synthesizer.
    fn update(&mut self) -> [f32; NUM_CHANNELS] {
        let amplitude = i16::max_value() as f32;
        let position = self.sample_count as f32;

        // Output samples
        let mut samples = [0.0; NUM_CHANNELS];

        for (i, inst) in self.song.instruments.iter().enumerate() {
            for j in 0..self.tracks[i].notes.len() {
                if self.tracks[i].notes[j].pitch == 0 {
                    continue;
                }

                if let Some(note_samples) = self.generate_samples(inst, i, j, position) {
                    // Mix the samples
                    for i in 0..NUM_CHANNELS {
                        samples[i] += note_samples[i];
                    }
                } else {
                    // Remove notes that have ended
                    self.tracks[i].notes[j] = Note::new(0, 0, 0.0, false);
                }
            }
        }

        // Clip samples to [-1.0, 1.0]
        for i in 0..NUM_CHANNELS {
            samples[i] = (samples[i] / amplitude).min(1.0).max(-1.0);
        }

        samples
    }
}

impl<'a> Iterator for Synth<'a> {
    type Item = [f32; NUM_CHANNELS];

    fn next(&mut self) -> Option<Self::Item> {
        // Check for end of song
        if self.seq_count > self.song.seq_length && !self
            .tracks
            .iter()
            .flat_map(|x| x.notes.iter())
            .any(|x| x.pitch != 0)
        {
            return None;
        }

        // Generate the next sample
        let samples = self.update();

        // Advance to next sample
        self.sample_count += 1;
        let sample_in_quarter_note = self.sample_count % self.quarter_note_length;
        if sample_in_quarter_note == 0 {
            // Advance to next note
            self.note_count += 1;
            if self.note_count >= PATTERN_LENGTH {
                self.note_count = 0;

                // Advance to next pattern
                self.seq_count += 1;
            }

            // Fetch the next set of notes
            self.load_delayed_notes();
            self.load_notes();
        } else if sample_in_quarter_note == self.eighth_note_length {
            // Fetch the next set of notes
            self.load_delayed_notes();
        }

        Some(samples)
    }
}
