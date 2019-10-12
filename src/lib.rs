#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate core as std;

mod consts;
mod song;
mod synth;

pub use song::{Error, Song};
pub use synth::Synth;
