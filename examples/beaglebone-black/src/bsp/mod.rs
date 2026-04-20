/// AM335x register address map (hand-written PAC).
#[cfg(feature = "bare_metal")]
pub mod am335x;

/// LCDC timing constants and bare-metal raster controller driver.
pub mod lcdc;

/// PRCM clock enable helpers.
#[cfg(feature = "bare_metal")]
pub mod prcm;

/// CTRLMOD pin mux configuration.
#[cfg(feature = "bare_metal")]
pub mod pinmux;
