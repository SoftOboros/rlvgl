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
//! 1. [`clocks`] — HP_SYS_CLKRST clock-gate sequence. Derived from
//!    `components/hal/esp32p4/include/hal/mipi_dsi_ll.h`.
//!
//! 2. [`init`] — DSI host PHY PLL config, lane bring-up, post-PLL host
//!    setup. Derived from `components/esp_lcd/dsi/esp_lcd_mipi_dsi_bus.c`,
//!    `components/hal/mipi_dsi_hal.c`,
//!    `components/hal/esp32p4/include/hal/mipi_dsi_phy_ll.h`,
//!    `components/hal/esp32p4/include/hal/mipi_dsi_host_ll.h`.

#![allow(dead_code)]

use esp32p4 as pac;

/// Clock-tree configuration for the DSI host + bridge + PHY.
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
        pub const fn freq_mhz(self) -> u32 {
            match self {
                DpiClockSource::Xtal => 40,
                DpiClockSource::PllF240m => 240,
                DpiClockSource::PllF160m => 160,
            }
        }
    }

    impl PhyClockSource {
        pub const fn freq_mhz(self) -> u32 {
            match self {
                PhyClockSource::PllF20m => 20,
                PhyClockSource::RcFast => 17, // ~17.5 MHz, IDF uses 17 for arithmetic
                PhyClockSource::PllF25m => 25,
            }
        }
    }

    /// Enable the DSI bus clock and pulse the bridge reset.
    ///
    /// # Safety
    /// Steals `HP_SYS_CLKRST`. No other code may be writing it concurrently.
    pub unsafe fn enable_bus_and_reset() {
        let p = unsafe { pac::Peripherals::steal() };
        p.HP_SYS_CLKRST
            .soc_clk_ctrl1()
            .modify(|_, w| w.dsi_sys_clk_en().set_bit());
        p.HP_SYS_CLKRST
            .hp_rst_en0()
            .modify(|_, w| w.rst_en_dsi_brg().set_bit());
        p.HP_SYS_CLKRST
            .hp_rst_en0()
            .modify(|_, w| w.rst_en_dsi_brg().clear_bit());
    }

    /// Configure the DPI pixel clock.
    ///
    /// `pixel_clk_mhz` is rounded up. For 26 MHz from F240M: div=9
    /// (actual 26.67 MHz).
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
    /// Real lane bit rate after PLL quantization (may differ from request).
    pub lane_bit_rate_mbps: u32,
    /// Number of active data lanes (1 or 2).
    pub num_data_lanes: u8,
}

#[derive(Debug)]
pub enum DsiError {
    InvalidArg,
    /// PHY PLL did not lock within the expected window.
    PllLock,
    /// Lane stop-state never reached.
    LaneCal,
}

const MIN_PHY_MBPS: u32 = 80;
const MAX_PHY_MBPS: u32 = 1500;

/// PHY PLL frequency-range table from
/// `components/soc/esp32p4/mipi_dsi_periph.c::soc_mipi_dsi_phy_pll_ranges[]`.
///
/// Returns the `hs_freq_range_sel` value for `lane_bit_rate_mbps`.
fn phy_hs_freq_sel(mbps: u32) -> u8 {
    // Trimmed to bands actually used by this project (200..1050 Mbps).
    // Full table in the IDF source if more bands are needed later.
    const TABLE: &[(u32, u32, u8)] = &[
        (200, 219, 0x03),
        (220, 239, 0x13),
        (240, 249, 0x23),
        (250, 269, 0x04),
        (270, 299, 0x14),
        (300, 329, 0x05),
        (330, 359, 0x15),
        (360, 399, 0x25),
        (400, 449, 0x06),
        (450, 499, 0x16),
        (500, 549, 0x07),
        (550, 599, 0x17),
        (600, 649, 0x08),
        (650, 699, 0x18),
        (700, 749, 0x09),
        (750, 799, 0x19),
        (800, 849, 0x29),
        (850, 899, 0x39),
        (900, 949, 0x0A),
        (950, 999, 0x1A),
        (1000, 1049, 0x2A),
    ];
    for &(lo, hi, sel) in TABLE {
        if mbps >= lo && mbps <= hi {
            return sel;
        }
    }
    0
}

/// Compute PHY PLL feedback (M) and input divider (N) per
/// `mipi_dsi_hal_configure_phy_pll`.
///
/// Constraints from the Synopsys DesignWare PHY:
///   * `f_vco = (M / N) * f_ref`
///   * `5 MHz ≤ f_ref / N ≤ 40 MHz`
///   * `M` must be even
///
/// Returns `(M, N, real_mbps)`.
fn compute_phy_pll(ref_mhz: u32, target_mbps: u32) -> Option<(u16, u8, u32)> {
    let min_n = (ref_mhz / 40).max(1) as u8;
    let max_n = (ref_mhz / 5) as u8;
    let mut best: Option<(u16, u8, u32)> = None;
    let mut best_delta = u32::MAX;
    for n in min_n..=max_n {
        let m = (target_mbps * n as u32 / ref_mhz) as u16;
        if m == 0 || (m & 1) != 0 {
            continue;
        }
        let real = ref_mhz * m as u32 / n as u32;
        let delta = target_mbps.abs_diff(real);
        if delta < best_delta {
            best_delta = delta;
            best = Some((m, n, real));
            if delta == 0 {
                break;
            }
        }
    }
    best
}

/// Synopsys PHY test-bus write: 6 register pokes encoding one (addr,val) pair.
///
/// Port of `mipi_dsi_hal_phy_write_register` — drives `PHY_TST_CTRL0`
/// (testclr / testclk) and `PHY_TST_CTRL1` (testen / testdin) per the
/// DesignWare programming model:
///
/// ```text
///   testclk=0,testclr=0
///   testen=1, testdin=addr
///   testclk=1, falling-edge latches addr
///   testclk=0
///   testen=0, testdin=val
///   testclk=1, rising-edge latches val
///   testclk=0
/// ```
unsafe fn phy_write_register(host: &pac::MIPI_DSI_HOST, reg_addr: u8, reg_val: u8) {
    // testclk=0, testclr=0 (write_clock(0, false))
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().clear_bit();
        w.phy_testclr().clear_bit();
        w
    });
    // load address: testen=1, testdin=addr (write_reg_addr)
    host.phy_tst_ctrl1().write(|w| {
        w.phy_testen().set_bit();
        unsafe { w.phy_testdin().bits(reg_addr) };
        w
    });
    // testclk=1, latch on falling edge (write_clock(1, false))
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().set_bit();
        w.phy_testclr().clear_bit();
        w
    });
    // testclk=0
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().clear_bit();
        w.phy_testclr().clear_bit();
        w
    });
    // load value: testen=0, testdin=val (write_reg_val)
    host.phy_tst_ctrl1().write(|w| {
        w.phy_testen().clear_bit();
        unsafe { w.phy_testdin().bits(reg_val) };
        w
    });
    // testclk=1
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().set_bit();
        w.phy_testclr().clear_bit();
        w
    });
    // testclk=0
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().clear_bit();
        w.phy_testclr().clear_bit();
        w
    });
}

/// Initialize the DSI host PHY and link layer.
///
/// Port of `esp_lcd_new_dsi_bus` (with `mipi_dsi_hal_init` +
/// `mipi_dsi_hal_configure_phy_pll` inlined).
///
/// # Safety
/// Caller must have already called:
///   1. [`super::ldo::LdoChannel::acquire_dphy`] (DPHY rail)
///   2. [`clocks::enable_bus_and_reset`] (DSI bus clock)
///   3. [`clocks::enable_phy_clocks`] (PHY config + PLL ref clocks)
///
/// And must know the PHY clock source frequency (`phy_ref_mhz`) — for
/// `PhyClockSource::PllF20m` this is 20.
pub unsafe fn init(
    num_data_lanes: u8,
    lane_bit_rate_mbps: u32,
    phy_ref_mhz: u32,
) -> Result<DsiBus, DsiError> {
    if !(1..=2).contains(&num_data_lanes)
        || !(MIN_PHY_MBPS..=MAX_PHY_MBPS).contains(&lane_bit_rate_mbps)
    {
        return Err(DsiError::InvalidArg);
    }
    let p = unsafe { pac::Peripherals::steal() };
    let host = &p.MIPI_DSI_HOST;

    // mipi_dsi_hal_init():
    //   set lane number, power on host + PHY, reset PHY, enable clk lane,
    //   force PLL stay-on.
    host.phy_if_cfg()
        .modify(|_, w| unsafe { w.n_lanes().bits(num_data_lanes - 1) });
    host.pwr_up().write(|w| w.shutdownz().set_bit());
    host.phy_rstz().modify(|_, w| {
        w.phy_shutdownz().set_bit();
        w
    });
    // PHY reset pulse: rstz=0 then 1.
    host.phy_rstz().modify(|_, w| w.phy_rstz().clear_bit());
    host.phy_rstz().modify(|_, w| {
        w.phy_rstz().set_bit();
        w.phy_enableclk().set_bit();
        w.phy_forcepll().set_bit();
        w
    });

    // mipi_dsi_hal_configure_phy_pll(): compute M/N and write the PHY
    // test-bus register pairs.
    let (pll_m, pll_n, real_mbps) =
        compute_phy_pll(phy_ref_mhz, lane_bit_rate_mbps).ok_or(DsiError::InvalidArg)?;
    let hs_freq_sel = phy_hs_freq_sel(lane_bit_rate_mbps);
    unsafe {
        phy_write_register(host, 0x44, hs_freq_sel << 1);
        phy_write_register(host, 0x19, 0x30);
        phy_write_register(host, 0x17, pll_n - 1);
        phy_write_register(host, 0x18, ((pll_m - 1) & 0x1F) as u8);
        phy_write_register(host, 0x18, 0x80 | (((pll_m - 1) >> 5) & 0x0F) as u8);
    }

    // Wait for PLL lock (phy_status.phy_lock = bit 0).
    let mut tries = 1_000_000u32;
    while host.phy_status().read().phy_lock().bit_is_clear() {
        tries -= 1;
        if tries == 0 {
            return Err(DsiError::PllLock);
        }
        core::hint::spin_loop();
    }

    // Wait for clock + active data lanes to enter the stop state.
    //   bit 2 = STOPSTATECLKLANE
    //   bit 4 = STOPSTATE0LANE
    //   bit 7 = STOPSTATE1LANE
    let mut mask: u32 = (1 << 2) | (1 << 4);
    if num_data_lanes > 1 {
        mask |= 1 << 7;
    }
    let mut tries = 1_000_000u32;
    while (host.phy_status().read().bits() & mask) != mask {
        tries -= 1;
        if tries == 0 {
            return Err(DsiError::LaneCal);
        }
        core::hint::spin_loop();
    }

    // Post-PLL host setup (esp_lcd_new_dsi_bus tail):
    //   - enter command mode (will switch to video mode at DPI start)
    //   - clock lane LP (auto-switched to HS by DPI controller later)
    //   - PHY HS<->LP switch times (verified-working IDF defaults)
    //   - RX CRC + ECC, TX EoTp on HS
    //   - timeout/escape clock dividers
    //   - timeout counts disabled (zero everywhere)
    //   - PHY max read time + stop wait time

    // Command mode: cmd_video_mode = 1 (i.e. !en).
    host.mode_cfg().modify(|_, w| w.cmd_video_mode().set_bit());

    // Clock lane LP.
    host.lpclk_ctrl().modify(|_, w| {
        w.auto_clklane_ctrl().clear_bit();
        w.phy_txrequestclkhs().clear_bit();
        w
    });

    // Switch times: data hs2lp=50, data lp2hs=104, clk hs2lp=46, clk lp2hs=128.
    host.phy_tmr_cfg().modify(|_, w| {
        unsafe {
            w.phy_hs2lp_time().bits(50);
            w.phy_lp2hs_time().bits(104);
        }
        w
    });
    host.phy_tmr_lpclk_cfg().modify(|_, w| {
        unsafe {
            w.phy_clkhs2lp_time().bits(46);
            w.phy_clklp2hs_time().bits(128);
        }
        w
    });

    // Packet handler: rx_crc=1, rx_ecc=1, eotp_tx_en=1, eotp_tx_lp_en=0.
    host.pckhdl_cfg().modify(|_, w| {
        w.crc_rx_en().set_bit();
        w.ecc_rx_en().set_bit();
        w.eotp_tx_en().set_bit();
        w.eotp_tx_lp_en().clear_bit();
        w
    });

    // Clock dividers: timeout = lane_byte_clk / 10 MHz, escape = / 18 MHz.
    // lane_byte_clk = lane_bit_rate / 8. For 750 Mbps: byte_clk = 93.75 MHz.
    //   to_div = 93.75 / 10 ≈ 9
    //   esc_div = 93.75 / 18 ≈ 5 (must be > 1, < 256)
    let to_div = (lane_bit_rate_mbps / 8 / 10).max(1) as u8;
    let esc_div = (lane_bit_rate_mbps / 8 / 18).clamp(2, 255) as u8;
    host.clkmgr_cfg().modify(|_, w| {
        unsafe {
            w.to_clk_division().bits(to_div);
            w.tx_esc_clk_division().bits(esc_div);
        }
        w
    });

    // All timeout counts to zero (disabled).
    host.to_cnt_cfg().modify(|_, w| {
        unsafe {
            w.hstx_to_cnt().bits(0);
            w.lprx_to_cnt().bits(0);
        }
        w
    });
    host.hs_rd_to_cnt()
        .write(|w| unsafe { w.hs_rd_to_cnt().bits(0) });
    host.lp_rd_to_cnt()
        .write(|w| unsafe { w.lp_rd_to_cnt().bits(0) });
    host.hs_wr_to_cnt()
        .write(|w| unsafe { w.hs_wr_to_cnt().bits(0) });
    host.lp_wr_to_cnt()
        .write(|w| unsafe { w.lp_wr_to_cnt().bits(0) });
    host.bta_to_cnt()
        .write(|w| unsafe { w.bta_to_cnt().bits(0) });

    // PHY max read time + stop wait (verified-working IDF defaults).
    host.phy_tmr_rd_cfg()
        .modify(|_, w| unsafe { w.max_rd_time().bits(6000) });
    host.phy_if_cfg()
        .modify(|_, w| unsafe { w.phy_stop_wait_time().bits(0x3F) });

    Ok(DsiBus {
        lane_bit_rate_mbps: real_mbps,
        num_data_lanes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pll_for_750mbps_from_20mhz() {
        // M=150, N=4 → 20 * 150 / 4 = 750 (exact).
        let (m, n, real) = compute_phy_pll(20, 750).unwrap();
        assert_eq!(m, 150);
        assert_eq!(n, 4);
        assert_eq!(real, 750);
    }

    #[test]
    fn hs_freq_sel_for_750() {
        assert_eq!(phy_hs_freq_sel(750), 0x19);
    }

    #[test]
    fn hs_freq_sel_for_500() {
        assert_eq!(phy_hs_freq_sel(500), 0x07);
    }
}
