# Sonant-rs

A Rust port of the [Sonant 4K synth](http://www.pouet.net/prod.php?which=53615) with streaming support.

Sonant [(C) 2008-2009 Jake Taylor](https://creativecommons.org/licenses/by-nc-sa/2.5/) [ Ferris / Youth Uprising ]

## What is it?

A tiny synthesizer written for 4K intros. It is capable of producing high quality audio with very little code and instrument data. Song files are just over 3KB, but can also be customized to reduce the number of instrument tracks or patterns if you have a tighter size budget.

The Sonant-rs `Synth` is implemented as an iterator, which makes it ideal for producing realtime audio streams with very little memory overhead; about 6.2 KB for the song data, and another 2.5 KB for buffering note frequencies. It was originally written to target Nintendo 64, which has a baseline of 4 MB of system memory!

## How does it work?

Flippin' maths and magics! I have no idea. Synthesizers are weird and alien to me, but they make really pretty ear-candy.

Each song has eight instrument tracks, and each instrument has two oscillators. The oscillators work together (or adversarially canceling each other, if you like) to vary the instrument frequencies. The "personality" of the instrument is provided by one of four waveforms: `Sine`, `Square`, `Saw`, or `Triangle`. The oscillators' frequencies modulate these basic waveforms to produce the final sounds.

In addition to the primary oscillators, each instrument also has its own [LFO](https://en.wikipedia.org/wiki/Low-frequency_oscillation), which is what makes that slow pitch-bending that you hear all the time in electronic music.

Finally, each instrument also has it own effects channel, which can do `HighPass`, `LowPass`, `BandPass`, and `Notch` filtering. The effects also provide simple resonance, delay (echo), and panning.

The rest of the song structure is pretty standard for tracked tunes; Each instrument can have up to 10 patterns. And any pattern can be referenced from a 48-element sequence. Each pattern itself contains 32 notes.

Delay effects are implemented as extra notes, which greatly reduces the memory footprint. The original implementation uses over 42 MB of memory to maintain the delay buffers. I made the tradeoff to pay for better memory efficiency by recomputing all of the delayed samples as they are needed.

See the Sonant manual (bundled with the original release archive on Pouët) if you would like to learn more about the synth, tracker, or song format.

## How to use it?

See the [./examples/player.rs](player.rs example) for some code that loads and plays a `.snt` file.

```bash
cargo run --release --example player -- ./examples/poseidon.snt
```

You can create `.snt` files using [sonant-tool](http://www.pouet.net/prod.php?which=53615) from the original release. You can also use the "Save" button (NOT the "Save JavaScript" button!) on [Sonant Live](http://sonantlive.bitsnbites.eu/tool/), but don't forget to check [its manual](http://sonantlive.bitsnbites.eu/)!

## Limitations

The original synthesizer doesn't have many limitations beyond what the `.snt` format is capable of storing. The iterator-based implementation of this port does come with a few restrictions, though. For example, only up to 8 overlapping notes are able to be played simultaneously for each instrument track. `sonant-tool` is capable of producing `.snt` files which require up to 100 overlapping notes per instrument track, but this is only true in the most extreme possible case. *The `.snt` format itself is theoretically able to require up to 1,536 overlapping notes!*

Songs which use a lot of delay effects on the instruments will more quickly hit the overlapping note limits. If you need to support more overlapping notes, you can simply increase the value in `consts.rs`; any value up to 32 will work without any other changes.
