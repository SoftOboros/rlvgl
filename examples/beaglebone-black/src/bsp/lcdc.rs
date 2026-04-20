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

// ---------------------------------------------------------------------------
// Bare-metal LCDC raster controller driver
// ---------------------------------------------------------------------------

#[cfg(feature = "bare_metal")]
use super::am335x::*;

/// Initialize the LCDC raster controller for 800x480 24-bit TFT output.
///
/// `fb_addr` is the physical address of the framebuffer in DDR (must be
/// word-aligned). The framebuffer is expected to be ARGB8888, 800x480,
/// i.e. `800 * 480 * 4 = 1,536,000` bytes.
///
/// # Safety
///
/// Writes to LCDC hardware registers. Must be called after PRCM clocks
/// are enabled and pin mux is configured.
#[cfg(feature = "bare_metal")]
pub unsafe fn init_raster(fb_addr: u32, fb_size: u32) {
    unsafe {
        // 1. Enable internal clocks: DMA + Core (raster)
        reg_write(LCDC_CLKC_ENABLE, CLKC_ENABLE_DMA | CLKC_ENABLE_CORE);

        // 2. LCD_CTRL: select raster mode, set clock divisor
        //    LCD pixel clock = L3_CLK / (clkdiv + 1)
        //    L3_CLK ~200 MHz (U-Boot default). For ~33.3 MHz: clkdiv = 5 → 200/6 ≈ 33.3 MHz
        let clkdiv: u32 = 5;
        reg_write(LCD_CTRL, LCD_CTRL_MODESEL_RASTER | (clkdiv << 8));

        // 3. RASTER_CTRL: TFT mode, 24-bit unpacked, data-only (no palette load)
        //    Leave lcden=0 for now (enabled last)
        reg_write(
            RASTER_CTRL,
            RASTER_CTRL_LCDTFT
                | RASTER_CTRL_TFT24
                | RASTER_CTRL_TFT24_UNPACKED
                | RASTER_CTRL_PALMODE_DATA_ONLY,
        );

        // 4. RASTER_TIMING_0: horizontal timing
        let ppl = (HACTIVE / 16) - 1;
        let ppl_lsb = (ppl & 0x3F) << 4;
        let ppl_msb = (ppl >> 6) & 0x0F;
        let hbp_val = (HBP - 1) << 24;
        let hfp_val = (HFP - 1) << 16;
        let hsw_val = (HSW - 1) << 10;
        reg_write(
            RASTER_TIMING_0,
            hbp_val | hfp_val | hsw_val | ppl_lsb | ppl_msb,
        );

        // 5. RASTER_TIMING_1: vertical timing
        let lpp = VACTIVE - 1;
        let vbp_val = VBP << 24;
        let vfp_val = VFP << 16;
        let vsw_val = (VSW - 1) << 10;
        reg_write(RASTER_TIMING_1, vbp_val | vfp_val | vsw_val | (lpp & 0x3FF));

        // 6. RASTER_TIMING_2: sync polarity + LPP bit 10 + HSW bit 5
        let lpp_b10 = ((lpp >> 10) & 1) << 26;
        let hsw_msb = (((HSW - 1) >> 6) & 1) << 27;
        reg_write(RASTER_TIMING_2, hsw_msb | lpp_b10 | TIMING2_IPC);

        // 7. LCDDMA_CTRL: burst size 16, single framebuffer
        reg_write(
            LCDDMA_CTRL,
            DMA_BURST_16 | DMA_FIFO_TH_8 | DMA_FRAME_MODE_SINGLE,
        );

        // 8. Framebuffer address (must be word-aligned)
        reg_write(LCDDMA_FB0_BASE, fb_addr);
        reg_write(LCDDMA_FB0_CEILING, fb_addr + fb_size - 4);

        // 9. Enable end-of-frame interrupt
        reg_write(LCDC_IRQENABLE_SET, IRQ_EOF0);

        // 10. Enable the raster engine — must be last
        reg_set_bits(RASTER_CTRL, RASTER_CTRL_LCDEN);
    }
}

/// Clear the EOF0 interrupt status.
#[cfg(feature = "bare_metal")]
pub unsafe fn clear_eof_irq() {
    unsafe {
        reg_write(LCDC_IRQSTATUS, IRQ_EOF0);
    }
}

/// Check if EOF0 interrupt is pending.
#[cfg(feature = "bare_metal")]
pub unsafe fn is_eof_pending() -> bool {
    unsafe { reg_read(LCDC_IRQSTATUS_RAW) & IRQ_EOF0 != 0 }
}
