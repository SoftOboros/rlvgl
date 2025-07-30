#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use rlvgl_core as core;
#[cfg(feature = "canvas")]
pub use rlvgl_core::canvas;
#[cfg(feature = "fontdue")]
pub use rlvgl_core::fontdue;
#[cfg(feature = "gif")]
pub use rlvgl_core::gif;
#[cfg(feature = "lottie")]
pub use rlvgl_core::lottie;
#[cfg(feature = "pinyin")]
pub use rlvgl_core::pinyin;
#[cfg(feature = "fatfs")]
pub use rlvgl_core::fatfs;
#[cfg(feature = "nes")]
pub use rlvgl_core::nes;
pub use rlvgl_platform as platform;
pub use rlvgl_widgets as widgets;
