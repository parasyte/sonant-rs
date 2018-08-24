extern crate arrayvec;
extern crate byteorder;
extern crate core;
extern crate failure;
extern crate rand;

mod consts;
mod song;
mod synth;

pub use song::{Error, Song};
pub use synth::Synth;
