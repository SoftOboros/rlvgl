//! Plugins for optional media formats and UI integrations.
//!
//! These modules are included conditionally via features such as 'gif', 'jpeg', 'qrcode', etc.
// Auto-generated plugin exports

#[cfg(feature = "canvas")]
pub mod canvas;

#[cfg(feature = "fatfs")]
pub mod fatfs;

#[cfg(feature = "fontdue")]
pub mod fontdue;

#[cfg(feature = "gif")]
pub mod gif;

#[cfg(feature = "apng")]
pub mod apng;

#[cfg(feature = "jpeg")]
pub mod jpeg;

#[cfg(feature = "lottie")]
pub mod dash_lottie;
#[cfg(feature = "lottie")]
pub mod lottie;

#[cfg(feature = "nes")]
pub mod nes;

#[cfg(feature = "pinyin")]
pub mod pinyin;

#[cfg(feature = "png")]
pub mod png;

#[cfg(feature = "qrcode")]
pub mod qrcode;
