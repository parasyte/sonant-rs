//! A Rust port of the [Sonant 4K synth](http://www.pouet.net/prod.php?which=53615) with streaming
//! support.
//!
//! Sonant [(C) 2008-2009 Jake Taylor](https://creativecommons.org/licenses/by-nc-sa/2.5/)
//! [ Ferris / Youth Uprising ]
//!
//! # Crate features
//!
//! - `std` (default) - Allow `std::error::Error`. Disable default features to use `sonant` in a
//!   `no_std` environment.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![forbid(unsafe_code)]

mod consts;
mod song;
mod synth;

pub use song::{Error, Song};
pub use synth::Synth;
