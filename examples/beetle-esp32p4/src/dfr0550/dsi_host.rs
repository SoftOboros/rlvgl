//! DSI host bring-up (1 lane @ 750 Mbps, RGB888).
//!
//! Equivalent of:
//! ```c
//! esp_lcd_dsi_bus_config_t dsi_bus_cfg = {
//!     .bus_id = 0,
//!     .num_data_lanes = 1,
//!     .phy_clk_src = MIPI_DSI_PHY_CLK_SRC_DEFAULT,  // PLL_F20M
//!     .lane_bit_rate_mbps = 750,
//! };
//! esp_lcd_new_dsi_bus(&dsi_bus_cfg, &dsi_bus);
//! ```
//!
//! Two layers in this module:
//!
//! 1. [`clocks`] — HP_SYS_CLKRST clock-gate sequence (DSI bus, DPI clock,
//!    PHY config clock, PHY PLL ref clock, source select). Derived from
//!    `components/hal/esp32p4/include/hal/mipi_dsi_ll.h`.
//!
//! 2. [`init`] — DSI host PHY PLL config (M/N for 750 Mbps from F20M ref),
//!    lane enable (CLK + D0), HS timing, calibration. Derived from
//!    `components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c` +
//!    `components/hal/esp32p4/include/hal/mipi_dsi_phy_ll.h`.
//!
//! The PHY in (2) is a Synopsys DesignWare DSI host (74 registers in the
//! P4 PAC) and the calibration sequence touches dozens of them. Layer 1
//! is implemented here; layer 2 is staged as a TODO with the canonical
//! reference paths cited inline.

#![allow(dead_code)]

/// Clock-tree configuration for the DSI host + bridge + PHY.
///
/// All accessors hit `HP_SYS_CLKRST`. The IDF wraps each access in a
/// critical section; this module assumes the caller is in single-hart
/// pre-runtime context (boot path before interrupts), so the locks are
/// elided. Re-entry from a multi-hart / interrupt context is unsupported.
pub mod clocks {
    use esp32p4 as pac;

    /// DPI clock source taps. `Default` = `PllF240m`, matching IDF's
    /// `MIPI_DSI_DPI_CLK_SRC_DEFAULT` resolution.
    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum DpiClockSource {
        Xtal = 0,
        PllF240m = 1,
        PllF160m = 2,
    }

    /// PHY clock source taps. `Default` = `PllF20m`.
    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum PhyClockSource {
        PllF20m = 0,
        RcFast = 1,
        PllF25m = 2,
    }

    impl DpiClockSource {
        /// Source frequency in MHz, used to compute the divider.
        pub const fn freq_mhz(self) -> u32 {
            match self {
                DpiClockSource::Xtal => 40,
                DpiClockSource::PllF240m => 240,
                DpiClockSource::PllF160m => 160,
            }
        }
    }

    /// Enable the DSI bus clock and pulse the bridge reset.
    ///
    /// `mipi_dsi_ll_enable_bus_clock(0, true)` + `mipi_dsi_ll_reset_register(0)`.
    ///
    /// # Safety
    /// Steals `HP_SYS_CLKRST`. No other code may be writing it concurrently.
    pub unsafe fn enable_bus_and_reset() {
        let p = unsafe { pac::Peripherals::steal() };
        p.HP_SYS_CLKRST
            .soc_clk_ctrl1()
            .modify(|_, w| w.dsi_sys_clk_en().set_bit());
        // Pulse: set then clear.
        p.HP_SYS_CLKRST
            .hp_rst_en0()
            .modify(|_, w| w.rst_en_dsi_brg().set_bit());
        p.HP_SYS_CLKRST
            .hp_rst_en0()
            .modify(|_, w| w.rst_en_dsi_brg().clear_bit());
    }

    /// Configure the DPI pixel clock.
    ///
    /// `pixel_clk_mhz` is rounded up — the IDF helper computes
    /// `div = round_up(src_mhz / target_mhz)` and writes `div - 1` to
    /// the divider field. For 26 MHz from F240M: div=9 (actual 26.67 MHz).
    ///
    /// # Safety
    /// As above.
    pub unsafe fn enable_dpi_clock(src: DpiClockSource, pixel_clk_mhz: u32) {
        assert!(pixel_clk_mhz > 0);
        let src_mhz = src.freq_mhz();
        let div = src_mhz.div_ceil(pixel_clk_mhz).max(1);
        let div_field: u8 = (div - 1) as u8;
        let p = unsafe { pac::Peripherals::steal() };
        p.HP_SYS_CLKRST.peri_clk_ctrl03().modify(|_, w| {
            unsafe {
                w.mipi_dsi_dpiclk_src_sel().bits(src as u8);
                w.mipi_dsi_dpiclk_div_num().bits(div_field);
            }
            w.mipi_dsi_dpiclk_en().set_bit()
        });
    }

    /// Enable PHY config clock + PHY PLL reference clock and select the
    /// PHY clock source.
    ///
    /// # Safety
    /// As above.
    pub unsafe fn enable_phy_clocks(src: PhyClockSource) {
        let p = unsafe { pac::Peripherals::steal() };
        p.HP_SYS_CLKRST
            .peri_clk_ctrl02()
            .modify(|_, w| unsafe { w.mipi_dsi_dphy_clk_src_sel().bits(src as u8) });
        p.HP_SYS_CLKRST.peri_clk_ctrl03().modify(|_, w| {
            w.mipi_dsi_dphy_cfg_clk_en().set_bit();
            w.mipi_dsi_dphy_pll_refclk_en().set_bit();
            w
        });
    }
}

/// Opaque DSI host handle returned by [`init`].
pub struct DsiBus {
    _private: (),
}

#[derive(Debug)]
pub enum DsiError {
    /// PHY PLL config layer not yet ported to PAC.
    Unimplemented,
    /// PHY PLL did not lock within the expected window.
    PllLock,
    /// Lane calibration timed out.
    LaneCal,
}

/// Initialize the DSI host PHY and link layer at 1 lane × 750 Mbps.
///
/// Sequence (port of `esp_lcd_new_dsi_bus` + `mipi_dsi_phy_*`):
///
/// 1. `clocks::enable_bus_and_reset()`        ← done in caller
/// 2. `clocks::enable_phy_clocks(F20M)`       ← done in caller
/// 3. PHY reset assert (`PHY_RSTZ` → 0)
/// 4. PHY testclr pulse (`PHY_TST_CTRL0`)
/// 5. PHY PLL M/N programming for 750 Mbps from 20 MHz ref:
///    M = lane_bit_rate / 2 / ref = 750 / 2 / 20 = 18.75 → use loop band
///    Reference: `dphy_pll_pms_calculation` in `esp_lcd_mipi_dsi_bus.c`
/// 6. PHY testen pulse, write PHY_TST_CTRL1 register pairs (≈ 30 of them
///    per the Synopsys DSI test-bus protocol)
/// 7. Deassert reset (`PHY_RSTZ` → 1, `PHY_SHUTDOWNZ` → 1)
/// 8. Poll `PHY_STATUS.PHY_LOCK` and `PHY_STATUS.PHY_STOPSTATEDATA_0`
/// 9. Set `LANE_ENABLE` = CLK | D0 (0x03 for 1-lane mode; per the kernel
///    DesignWare DSI driver this is correct even though the chipdb still
///    enumerates D1+/D1- — those pads are tri-stated)
/// 10. Configure HS timings (`PHY_TMR_CFG`, `PHY_TMR_LPCLK_CFG`, etc.)
///
/// # Safety
/// Caller must have powered the DPHY rail via [`super::ldo::LdoChannel`]
/// and ungated clocks via [`clocks::enable_bus_and_reset`] +
/// [`clocks::enable_phy_clocks`].
pub unsafe fn init() -> Result<DsiBus, DsiError> {
    // TODO(phase 5b.3 — needs hardware-in-loop verification):
    //
    //   Reference for PHY M/N PLL calculation:
    //     ~/esp/esp-idf/components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c
    //       fn dphy_pll_pms_calculation(uint32_t lane_rate_mbps,
    //                                   uint32_t ref_clk_mhz,
    //                                   uint32_t *m, uint32_t *n,
    //                                   uint32_t *s);
    //
    //   Reference for PHY test-bus writes (testen + testdin via
    //   PHY_TST_CTRL0/CTRL1 registers):
    //     ~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_phy_ll.h
    //       fn mipi_dsi_phy_ll_write_test_bus(uint8_t code, uint8_t value)
    //
    //   The 74-register PHY surface is in
    //   `~/.cargo/registry/src/index.crates.io-*/esp32p4-0.2.0/src/mipi_dsi_host/`.
    //
    // This stub keeps the public signature stable so the rest of the
    // bring-up (ldo, clocks, cache, i2c bridge) can be wired and tested
    // in isolation. Phase 5b.3 turns this into ~150 register pokes.
    Err(DsiError::Unimplemented)
}
