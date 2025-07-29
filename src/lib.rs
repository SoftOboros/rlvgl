#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use rlvgl_core as core;
#[cfg(feature = "fontdue")]
pub use rlvgl_core::fontdue;
#[cfg(feature = "gif")]
pub use rlvgl_core::gif;
#[cfg(feature = "lottie")]
pub use rlvgl_core::lottie;
#[cfg(feature = "canvas")]
pub use rlvgl_core::canvas;
pub use rlvgl_platform as platform;
pub use rlvgl_widgets as widgets;
