#![cfg_attr(not(feature = "std"), no_std)]

mod consts;
#[cfg(feature = "std")]
pub mod errors;
mod song;
mod synth;

pub use song::{Error, Song};
pub use synth::Synth;
