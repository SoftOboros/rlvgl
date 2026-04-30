//! DPI panel controller — 26 MHz pixel clock, RGB888, Pi 7" timings.
//!
//! ESP-IDF reference (verified-working call):
//!
//! ```c
//! esp_lcd_dpi_panel_config_t dpi_cfg = {
//!     .virtual_channel = 0,
//!     .dpi_clk_src = MIPI_DSI_DPI_CLK_SRC_DEFAULT,
//!     .dpi_clock_freq_mhz = 26,
//!     .pixel_format = LCD_COLOR_PIXEL_FORMAT_RGB888,
//!     .in_color_format = LCD_COLOR_FMT_RGB888,
//!     .num_fbs = 1,
//!     .flags.use_dma2d = true,
//!     .video_timing = { 800, 480, 2, 46, 1, 2, 21, 7 },
//! };
//! esp_lcd_new_panel_dpi(dsi_bus, &dpi_cfg, &dpi_panel);
//! esp_lcd_panel_init(dpi_panel);
//! esp_lcd_dpi_panel_get_frame_buffer(dpi_panel, 1, &fb);
//! ```
//!
//! TODO(TRM §MIPI DSI Bridge / DPI controller): drive the DPI registers:
//!   - Pixel format: 24-bit RGB888 packed
//!   - Video mode: NON-BURST sync events (Pi-bridge needs LP transitions
//!     between frames; do NOT set `disable_lp`)
//!   - HFP/HSA/HBP/VFP/VSA/VBP from [`super`] constants
//!   - Pixel clock: divide DPI source by `(SOURCE_HZ / 26 MHz)`
//!
//! TODO(TRM §DMA-2D): allocate a DMA descriptor list pointing at the
//!   PSRAM-backed framebuffer; stream it to the DSI bridge in continuous
//!   refresh mode (matches IDF `flags.use_dma2d = true`).

#![allow(dead_code)]

use super::dsi_host::DsiBus;

/// 800×480 RGB888 framebuffer view, lifetime-bound to the DPI panel.
pub struct FrameBuffer<'p> {
    /// Pointer into PSRAM, aligned to a DMA boundary.
    pub ptr: *mut u8,
    /// Bytes occupied: 800 × 480 × 3 = 1_152_000.
    pub len: usize,
    _phantom: core::marker::PhantomData<&'p mut ()>,
}

/// Opaque DPI panel handle.
pub struct DpiPanel {
    _private: (),
}

impl DpiPanel {
    /// Initialize the DPI controller and return the framebuffer view.
    ///
    /// # Safety
    /// `dsi` must have been initialized by [`super::dsi_host::init`].
    /// PSRAM must already be mapped (see [`super::psram::init`]).
    pub unsafe fn init(_dsi: &DsiBus) -> Result<(Self, FrameBuffer<'static>), DpiError> {
        // TODO(phase 5b)
        Err(DpiError::Unimplemented)
    }
}

#[derive(Debug)]
pub enum DpiError {
    Unimplemented,
    PixelClock,
    Dma,
}

/// Total framebuffer size in bytes (RGB888).
pub const FB_BYTES: usize =
    super::H_RES as usize * super::V_RES as usize * 3;
