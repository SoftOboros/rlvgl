//! LCDC timing constants for the NHD-7.0-800480AF-ASXP TFT panel.
//!
//! These values are derived from the Newhaven NHD-7.0-800480AF-ASXP datasheet
//! and apply to all three BBB runtime paths (Linux DT overlay, Zephyr DTS,
//! bare-metal LCDC register setup).
//!
//! The panel uses a Sitronix ST7277 driver IC configured in DE mode (recommended).
//! Data is clocked on the DCLK falling edge.

/// Horizontal active pixels.
pub const HACTIVE: u32 = 800;
/// Vertical active lines.
pub const VACTIVE: u32 = 480;

/// Typical pixel clock in Hz (~33.3 MHz).
pub const PIXEL_CLOCK_HZ: u32 = 33_300_000;

/// Horizontal back porch (DCLK cycles).
pub const HBP: u32 = 46;
/// Horizontal front porch (DCLK cycles).
pub const HFP: u32 = 210;
/// Horizontal sync width (DCLK cycles).
pub const HSW: u32 = 20;

/// Vertical back porch (lines).
pub const VBP: u32 = 23;
/// Vertical front porch (lines).
pub const VFP: u32 = 22;
/// Vertical sync width (lines).
pub const VSW: u32 = 10;

/// Total horizontal period (pixels + blanking).
pub const HTOTAL: u32 = HACTIVE + HBP + HFP + HSW;
/// Total vertical period (lines + blanking).
pub const VTOTAL: u32 = VACTIVE + VBP + VFP + VSW;

/// Frame rate derived from pixel clock and total timing.
pub const FRAME_HZ: u32 = PIXEL_CLOCK_HZ / (HTOTAL * VTOTAL);
