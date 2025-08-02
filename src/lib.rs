//! Public re-export crate for the `rlvgl` ecosystem.
//!
//! This crate simply exposes the core runtime, widgets and platform layers in a
//! single package for convenience when publishing to crates.io.
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub use rlvgl_core as core;
#[cfg(feature = "apng")]
pub use rlvgl_core::apng;
#[cfg(feature = "canvas")]
pub use rlvgl_core::canvas;
#[cfg(feature = "lottie")]
pub use rlvgl_core::lottie;
#[cfg(feature = "fatfs")]
pub use rlvgl_core::fatfs;
#[cfg(feature = "fontdue")]
pub use rlvgl_core::fontdue;
#[cfg(feature = "gif")]
pub use rlvgl_core::gif;
#[cfg(feature = "nes")]
pub use rlvgl_core::nes;
#[cfg(feature = "pinyin")]
pub use rlvgl_core::pinyin;
pub use rlvgl_platform as platform;
pub use rlvgl_widgets as widgets;
