#![cfg_attr(not(feature = "std"), no_std)]

extern crate arrayvec;
extern crate byteorder;
extern crate libm;
extern crate randomize;

#[cfg(not(feature = "std"))]
extern crate core as std;

#[cfg(feature = "std")]
extern crate failure;

mod consts;
mod song;
mod synth;

pub use song::{Error, Song};
pub use synth::Synth;
