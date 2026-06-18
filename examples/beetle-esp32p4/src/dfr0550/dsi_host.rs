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

use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use esp32p4 as pac;

/// BEETLE-05 diagnostic: the value of `ref_clk_ctrl1.ref_20m_clk_div_num`
/// as the bootloader left it, captured before `enable_phy_clocks`
/// overwrites it. The PLL_F20M tap is `SPLL(480 MHz) / (div_num + 1)`; for
/// the 20 MHz our M/N math assumes, `div_num` must be 23. If the Morse
/// readout shows anything other than 023 here, the bootloader never set up
/// the 20 MHz reference (nothing else uses it) and the DPHY PLL was being
/// fed the wrong reference frequency — the real reason lock never asserted.
pub static LAST_REF20M_DIV: AtomicU8 = AtomicU8::new(0xFF);

/// BEETLE-05 diagnostic: raw `MIPI_DSI_HOST.phy_status` captured at the
/// moment `init` gives up waiting for PLL lock (or lane stop-state). The
/// LED Morse loop emits its low byte so the bench operator can tell
/// *why* the DSI PHY didn't come up:
///   - `0x00` (000) → PHY reports nothing: the DPHY analog rail (LDO_VO3)
///     is almost certainly dead/off-target, OR the PHY config clock isn't
///     reaching the test interface. Power/clock-infrastructure problem.
///   - non-zero with bit 0 (`phy_lock`) clear → PHY is alive and clocking
///     but the PLL won't lock: reference-clock frequency or analog
///     voltage-trim (eFuse-calibrated dref/mul) problem.
///   - bit 0 set but bits 2/4/(7) clear → PLL locked, lanes never reached
///     stop-state (LaneCal path).
/// Sentinel `0xFFFF_FFFF` = init never reached the PLL-lock wait.
pub static LAST_PHY_STATUS: AtomicU32 = AtomicU32::new(0xFFFF_FFFF);

/// BEETLE-05 diagnostic: read-back of the DPHY clock gates after
/// `enable_phy_clocks`, to prove they actually latched (e.g. the PAC marks
/// `ref_20m_clk_en` "Reserved" — `.set_bit()` on it may not stick). All
/// four bits set (value 017 octal-looking → decimal 15) means the clock
/// tree is configured as intended:
///   bit 0 — `ref_clk_ctrl2.ref_20m_clk_en` (20 MHz tap gate)
///   bit 1 — `peri_clk_ctrl03.mipi_dsi_dphy_cfg_clk_en`
///   bit 2 — `peri_clk_ctrl03.mipi_dsi_dphy_pll_refclk_en`
///   bit 3 — `peri_clk_ctrl02.mipi_dsi_dphy_clk_src_sel == 0` (PLL_F20M)
/// Sentinel 0xFF = enable_phy_clocks never ran.
pub static LAST_DPHY_CLK_VERIFY: AtomicU8 = AtomicU8::new(0xFF);

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

        // BEETLE-05 ROOT CAUSE: enable the upstream reference-clock GATE for
        // the selected PHY source. IDF reaches this via
        // `esp_clk_tree_enable_src(PLL_F20M)` → `clk_gate_ll_ref_20m_clk_en`
        // → `HP_SYS_CLKRST.ref_clk_ctrl2.ref_20m_clk_en`. The IDF bootloader
        // leaves this gate OFF (nothing else uses the 20 MHz reference), so
        // the SPLL-derived PLL_F20M tap never reaches the DPHY. Result: the
        // DPHY PLL has no reference and `phy_lock` never asserts, even though
        // the PHY digital logic powers up and reports status (`phy_status`
        // reads 0x28 — `ulpsactivenot{clk,0}` set, `phy_lock`/stop-state
        // clear). We previously enabled only the DPHY-side `pll_refclk_en`
        // gate (below), which routes a clock that isn't running. This is the
        // same "infrastructure the HAL assumes is already up" class as
        // ERRATA-005 (I2C APB gate). See docs/beetle-esp32p4/ERRATA.md.
        match src {
            PhyClockSource::PllF20m => {
                // Capture the bootloader's divider, then force the canonical
                // 480/24 = 20 MHz divider (div_num=23). esp_clk_tree READS
                // this divider to compute the PHY PLL M/N; we hardcode
                // ref=20 MHz, so the divider MUST actually be 24. If the
                // bootloader left it unset, the DPHY PLL reference is wrong
                // and lock is impossible regardless of the gate.
                super::LAST_REF20M_DIV.store(
                    p.HP_SYS_CLKRST
                        .ref_clk_ctrl1()
                        .read()
                        .ref_20m_clk_div_num()
                        .bits(),
                    super::Ordering::Relaxed,
                );
                p.HP_SYS_CLKRST
                    .ref_clk_ctrl1()
                    .modify(|_, w| unsafe { w.ref_20m_clk_div_num().bits(23) });
                p.HP_SYS_CLKRST
                    .ref_clk_ctrl2()
                    .modify(|_, w| w.ref_20m_clk_en().set_bit());
            }
            PhyClockSource::PllF25m => {
                p.HP_SYS_CLKRST
                    .ref_clk_ctrl1()
                    .modify(|_, w| w.ref_25m_clk_en().set_bit());
            }
            // RC_FAST needs no SPLL-tap gate.
            PhyClockSource::RcFast => {}
        }

        p.HP_SYS_CLKRST
            .peri_clk_ctrl02()
            .modify(|_, w| unsafe { w.mipi_dsi_dphy_clk_src_sel().bits(src as u8) });
        p.HP_SYS_CLKRST.peri_clk_ctrl03().modify(|_, w| {
            w.mipi_dsi_dphy_cfg_clk_en().set_bit();
            w.mipi_dsi_dphy_pll_refclk_en().set_bit();
            w
        });

        // BEETLE-05: read the gates back to prove they latched.
        let mut v: u8 = 0;
        if p.HP_SYS_CLKRST
            .ref_clk_ctrl2()
            .read()
            .ref_20m_clk_en()
            .bit_is_set()
        {
            v |= 1 << 0;
        }
        let c03 = p.HP_SYS_CLKRST.peri_clk_ctrl03().read();
        if c03.mipi_dsi_dphy_cfg_clk_en().bit_is_set() {
            v |= 1 << 1;
        }
        if c03.mipi_dsi_dphy_pll_refclk_en().bit_is_set() {
            v |= 1 << 2;
        }
        if p.HP_SYS_CLKRST
            .peri_clk_ctrl02()
            .read()
            .mipi_dsi_dphy_clk_src_sel()
            .bits()
            == 0
        {
            v |= 1 << 3;
        }
        super::LAST_DPHY_CLK_VERIFY.store(v, super::Ordering::Relaxed);
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

/// Synopsys PHY test-bus READ: latch an address, then sample `testdout`
/// (bits[15:8] of `phy_tst_ctrl1`). Used by BEETLE-05 to confirm the PHY
/// config-clock + test interface actually work — if a register we wrote
/// reads back its value, the M/N pokes are landing; if it reads 0/garbage,
/// the test interface is dead and the PLL can never be configured.
unsafe fn phy_read_register(host: &pac::MIPI_DSI_HOST, reg_addr: u8) -> u8 {
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().clear_bit();
        w.phy_testclr().clear_bit();
        w
    });
    host.phy_tst_ctrl1().write(|w| {
        w.phy_testen().set_bit();
        unsafe { w.phy_testdin().bits(reg_addr) };
        w
    });
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().set_bit();
        w.phy_testclr().clear_bit();
        w
    });
    host.phy_tst_ctrl0().write(|w| {
        w.phy_testclk().clear_bit();
        w.phy_testclr().clear_bit();
        w
    });
    ((host.phy_tst_ctrl1().read().bits() >> 8) & 0xFF) as u8
}

/// BEETLE-05 diagnostic: read-back of PHY test register 0x17 (PLL N − 1)
/// after the M/N pokes. For 1 lane @ 750 Mbps the value written is 3
/// (N=4). `003` here means the test interface + config clock work and the
/// pokes landed (so the no-lock cause is elsewhere); `000`/other means the
/// pokes never reached the PHY — the real root cause. 0xFF = never read.
pub static LAST_PHY_REG17: AtomicU8 = AtomicU8::new(0xFF);

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

    // mipi_dsi_hal_init(): one LL call per register write, in IDF order.
    // BEETLE-05: these MUST be separate read-modify-writes (not collapsed)
    // — the DesignWare PHY samples enableclk/forcepll relative to the rstz
    // rising edge, and a single combined write that asserts rstz=1 +
    // enableclk + forcepll simultaneously leaves the PLL state machine in a
    // state where lock never asserts (phy_status stuck at 0x28). IDF does
    // reset → enable_clock_lane → force_pll as three distinct writes.
    host.phy_if_cfg()
        .modify(|_, w| unsafe { w.n_lanes().bits(num_data_lanes - 1) });
    // host_ll_power_on_off(true)
    host.pwr_up().write(|w| w.shutdownz().set_bit());
    // phy_ll_power_on_off(true)
    host.phy_rstz().modify(|_, w| w.phy_shutdownz().set_bit());
    // phy_ll_reset(): rstz=0 then rstz=1 (two writes)
    host.phy_rstz().modify(|_, w| w.phy_rstz().clear_bit());
    host.phy_rstz().modify(|_, w| w.phy_rstz().set_bit());
    // phy_ll_enable_clock_lane(true)
    host.phy_rstz().modify(|_, w| w.phy_enableclk().set_bit());
    // phy_ll_force_pll(true)
    host.phy_rstz().modify(|_, w| w.phy_forcepll().set_bit());

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

    // BEETLE-05 diagnostic: read PHY reg 0x17 (N-1) back through the test
    // interface to prove the pokes actually reached the PHY.
    LAST_PHY_REG17.store(unsafe { phy_read_register(host, 0x17) }, Ordering::Relaxed);

    // Wait for PLL lock (phy_status.phy_lock = bit 0).
    let mut tries = 1_000_000u32;
    while host.phy_status().read().phy_lock().bit_is_clear() {
        tries -= 1;
        if tries == 0 {
            // BEETLE-05 diagnostic: snapshot the raw PHY status so the
            // Morse loop can distinguish "PHY dead (LDO/clock)" from
            // "PHY alive, PLL won't lock (ref-clock/voltage trim)".
            LAST_PHY_STATUS.store(host.phy_status().read().bits(), Ordering::Relaxed);
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
            // BEETLE-05 diagnostic: PLL locked but lanes never stopped —
            // snapshot so the Morse loop shows which stop-state bits are
            // missing (bit 2 = clk lane, 4 = data0, 7 = data1).
            LAST_PHY_STATUS.store(host.phy_status().read().bits(), Ordering::Relaxed);
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
