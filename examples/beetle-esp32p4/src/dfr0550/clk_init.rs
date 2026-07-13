//! ESP32-P4 CPU operating-point bring-up (raw-PAC port of IDF
//! `rtc_clk_cpu_freq_to_cpll_mhz`).
//!
//! BEETLE-06 (ERRATA-009). A full-block JTAG register diff proved every
//! DSI-DPHY-relevant register byte-identical between a locking IDF binary
//! and our (non-locking) firmware. The *only* config-independent system
//! difference left is the CPU/MEM clock operating point: IDF runs the
//! HP system at the full 360 MHz point (CPLL ÷1 → CPU 360 / MEM 180 /
//! SYS 180 / APB 90), while our espflash-bootloader boot stays at the
//! slow default (CPLL ÷4 → CPU 90 / MEM 90 / SYS 90 / APB 90). The DPHY
//! `cfg_clk` (PLL-lock FSM clock) is documented as SPLL/24, independent
//! of this tree — but it is the last untested delta, so this module
//! replicates IDF's operating point exactly to settle the question.
//!
//! Observed register targets (from the IDF JTAG dump):
//!   `root_clk_ctrl0 = 0x0`  (cpu_clk_div_num = 0 → CPU = CPLL ÷1)
//!   `root_clk_ctrl1 = 0x1`  (mem_clk_div_num = 1 → MEM = CPU ÷2;
//!                            sys_clk_div_num = 0 → SYS = MEM ÷1)
//!   `root_clk_ctrl2 = 0x10000` (apb_clk_div_num = 1 → APB = SYS ÷2)
//!
//! Field encoding: each `*_div_num` field holds `divider - 1`.

use esp32p4 as pac;

/// Pulse `soc_clk_div_update` and wait for the hardware to apply the
/// staged CPU/MEM/SYS/APB dividers. Mirrors IDF `clk_ll_bus_update`.
#[inline]
fn bus_update(c: &pac::HP_SYS_CLKRST) {
    c.root_clk_ctrl0()
        .modify(|_, w| w.soc_clk_div_update().set_bit());
    while c.root_clk_ctrl0().read().soc_clk_div_update().bit_is_set() {}
}

/// Raise the HP system from the bootloader's CPLL÷4 (CPU 90 MHz) point to
/// the IDF CPLL÷1 (CPU 360 MHz) point, matching the operating point a
/// known-good IDF binary holds when its DSI DPHY PLL locks.
///
/// This is an **upscale** (90 → 360 MHz), so dividers are committed in
/// APB → SYS → MEM → CPU order (each followed by `soc_clk_div_update`) to
/// avoid illegal intermediate states where MEM/APB would momentarily
/// exceed their max (MEM ≤ 200 MHz, APB ≤ 100 MHz) — per IDF
/// `rtc_clk_cpu_freq_to_cpll_mhz`. Final CPU:MEM ratio is exactly 2 (the
/// cache constraint limit).
///
/// The CPU clock source is **not** touched: the bootloader already runs
/// us on CPLL (CPU = 360/4 = 90 MHz proves it), so the source mux is
/// already correct and rewriting it would only risk a wrong-source glitch.
/// Skipping it also makes the end-state registers byte-match the IDF dump.
///
/// # Safety
/// Reconfigures the live CPU/MEM/SYS/APB clock tree. Must run
/// single-threaded with no concurrent peripheral activity that depends on
/// MEM/APB timing. Call AFTER any CPU-clock-calibrated busy-wait code
/// (e.g. the I2C bit-bang bridge wake) and BEFORE the DSI bring-up. Our
/// core voltage (DCM_VSET = 31, dbias = 24) exceeds IDF's (27/24), so the
/// 360 MHz point has voltage margin.
pub unsafe fn set_cpu_cpll_360mhz() {
    let p = unsafe { pac::Peripherals::steal() };
    let c = &p.HP_SYS_CLKRST;

    // APB divider = 2  (field = 1)
    c.root_clk_ctrl2()
        .modify(|_, w| unsafe { w.apb_clk_div_num().bits(1) });
    bus_update(c);

    // SYS divider = 1  (field = 0)
    c.root_clk_ctrl1()
        .modify(|_, w| unsafe { w.sys_clk_div_num().bits(0) });
    bus_update(c);

    // MEM divider = 2  (field = 1)
    c.root_clk_ctrl1()
        .modify(|_, w| unsafe { w.mem_clk_div_num().bits(1) });
    bus_update(c);

    // CPU divider = 1  (integer field = 0, fractional num/den = 0)
    c.root_clk_ctrl0().modify(|_, w| unsafe {
        w.cpu_clk_div_num().bits(0);
        w.cpu_clk_div_numerator().bits(0);
        w.cpu_clk_div_denominator().bits(0);
        w
    });
    bus_update(c);
}
