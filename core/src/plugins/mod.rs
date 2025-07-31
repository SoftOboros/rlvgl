//! Plugins for optional media formats and UI integrations.
//!
//! These modules are included conditionally via features such as 'gif', 'jpeg', 'qrcode', etc.
// Auto-generated plugin exports

#[cfg(feature = "canvas")]
pub mod canvas;
#[cfg(feature = "canvas")]
pub use canvas::*;

#[cfg(feature = "fatfs")]
pub mod fatfs;
#[cfg(feature = "fatfs")]
pub use fatfs::*;

#[cfg(feature = "fontdue")]
pub mod fontdue;
#[cfg(feature = "fontdue")]
pub use fontdue::*;

#[cfg(feature = "gif")]
pub mod gif;
#[cfg(feature = "gif")]
pub use gif::*;

#[cfg(feature = "jpeg")]
pub mod jpeg;
#[cfg(feature = "jpeg")]
pub use jpeg::*;

#[cfg(feature = "lottie")]
pub mod lottie;
#[cfg(feature = "lottie")]
pub use lottie::*;
#[cfg(feature = "lottie")]
pub mod dash_lottie;
#[cfg(feature = "lottie")]
pub use dash_lottie::*;
#[cfg(feature = "lottie")]
pub mod dash_lottie_render;
#[cfg(feature = "lottie")]
pub use dash_lottie_render::*;

#[cfg(feature = "nes")]
pub mod nes;
#[cfg(feature = "nes")]
pub use nes::*;

#[cfg(feature = "pinyin")]
pub mod pinyin;
#[cfg(feature = "pinyin")]
pub use pinyin::*;

#[cfg(feature = "png")]
pub mod png;
#[cfg(feature = "png")]
pub use png::*;

#[cfg(feature = "qrcode")]
pub mod qrcode;
#[cfg(feature = "qrcode")]
pub use qrcode::*;
