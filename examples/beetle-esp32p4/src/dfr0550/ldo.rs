//! DPHY LDO_VO3 power-up (2500 mV).
//!
//! Equivalent of:
//! ```c
//! esp_ldo_channel_config_t ldo_cfg = { .chan_id = 3, .voltage_mv = 2500 };
//! esp_ldo_acquire_channel(&ldo_cfg, &dphy_ldo);
//! ```
//!
//! Derived from the canonical IDF low-level driver:
//! `components/hal/esp32p4/include/hal/ldo_ll.h`. That header maps the
//! analog channel ids (1..4 → VO1..VO4) to PMU `ext_ldo[]` slots via
//! `index_array = {0, 3, 1, 4}`, so:
//!
//! - chan_id=1 → unit 0 → ext_ldo[0] (LDO_VO1)
//! - chan_id=2 → unit 1 → ext_ldo[3] (LDO_VO2)
//! - chan_id=3 → unit 2 → ext_ldo[1] (LDO_VO3, DPHY rail)  ← this module
//! - chan_id=4 → unit 3 → ext_ldo[4] (LDO_VO4)
//!
//! In the `esp32p4` PAC the indexed `ext_ldo[]` array is split into named
//! peripherals. Slot 1 is `EXT_LDO_P0_0P2A` (CTRL) + `EXT_LDO_P0_0P2A_ANA`
//! (analog DREF/MUL).
//!
//! Voltage formula (from `ldo_ll_voltage_to_dref_mul`, with no efuse cal):
//!   Vref = (dref < 9) ? 0.5 + dref*0.05 : 1.0 + (dref-9)*0.1
//!   Vout = Vref * (1 + 0.25 * mul)
//!
//! For 2500 mV the closest tap is dref=9 (Vref=1.0V), mul=6
//! (Vout = 1.0 * (1 + 1.5) = 2.5 V).
//!
//! Configured fields (matching `ldo_ll_set_owner` / `ldo_ll_adjust_voltage` /
//! `ldo_ll_enable`):
//!
//! | Field          | Value | Meaning                                     |
//! |----------------|-------|---------------------------------------------|
//! | force_tieh_sel | 1     | software-owned (vs efuse hw default)        |
//! | tieh_sel       | 0     | use `tieh` field (not sdmmc / 3.3V rail)    |
//! | tieh           | 0     | output = Vref * (1 + 0.25*mul) (not 3.3V)   |
//! | dref           | 9     | Vref = 1.0 V                                |
//! | mul            | 6     | Vout = 1.0 * 2.5 = 2.5 V                    |
//! | xpd            | 1     | enable LDO output                           |

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

/// BEETLE-05 diagnostic: packed read-back of the LDO_VO3 config after
/// `acquire_dphy`, so the Morse loop can confirm our PMU writes actually
/// landed (the DPHY rail being mis-configured is the prime suspect for the
/// PHY-alive-but-PLL-won't-lock state). All five bits set (value 031) means
/// the rail is configured exactly as intended:
///   bit 0 — `xpd` == 1            (LDO output enabled)
///   bit 1 — `force_tieh_sel` == 1 (software-owned)
///   bit 2 — `tieh` == 0           (output = Vref*Mul, not 3.3 V)
///   bit 3 — `dref` == 9           (Vref = 1.0 V)
///   bit 4 — `mul` == 6            (Vout = 2.5 V)
/// Sentinel 0xFF = acquire_dphy never ran.
pub static LAST_LDO_VERIFY: AtomicU8 = AtomicU8::new(0xFF);

/// BEETLE-05 diagnostic: the eFuse-calibrated `(dref * 10 + mul)` actually
/// programmed into LDO_VO3. `096` means calibration resolved to the
/// uncalibrated 9/6 tap (so the DPHY voltage was never the problem);
/// anything else means this chip's eFuse trim shifted the dref/mul to hit
/// a true 2500 mV — and our previous hardcoded 9/6 was at the wrong
/// voltage, which is the prime suspect for the PLL refusing to lock.
pub static LAST_LDO_DREFMUL: AtomicU8 = AtomicU8::new(0xFF);

/// Target DPHY rail voltage (LDO_VO3) in millivolts.
const DPHY_MV: i32 = 2500;

/// Port of IDF `ldo_ll_voltage_to_dref_mul` for LDO_VO3 (unit 2): pick the
/// `(dref, mul)` pair that lands closest to `target_mv`, applying this
/// chip's eFuse calibration. `efuse_raw` is `EFUSE.rd_mac_sys_3` (BLK1
/// word 3): LDO_VO3_K = bits[13:6], LDO_VO3_VOS = bits[19:14],
/// LDO_VO3_C = bits[25:20]. Unprogrammed (0) fields fall back to the
/// uncalibrated constants, reproducing the 9/6 = 2500 mV default. All math
/// is i32 fixed-point (constants ×1000) to avoid the FPU — same as IDF.
fn calibrated_dref_mul(efuse_raw: u32, target_mv: i32) -> (u8, u8) {
    let efuse_k = ((efuse_raw >> 6) & 0xFF) as i32;
    let efuse_vos = ((efuse_raw >> 14) & 0x3F) as i32;
    let efuse_c = ((efuse_raw >> 20) & 0x3F) as i32;

    let mut k_1000: i32 = 1000;
    let mut vos_1000: i32 = 0;
    let mut c_1000: i32 = 1000;
    if efuse_k != 0 {
        k_1000 = if efuse_k & 0x80 != 0 {
            -(efuse_k & 0x7F) + 975
        } else {
            efuse_k + 975
        };
    }
    if efuse_vos != 0 {
        vos_1000 = if efuse_vos & 0x20 != 0 {
            -(efuse_vos & 0x1F) - 3
        } else {
            efuse_vos - 3
        };
    }
    if efuse_c != 0 {
        c_1000 = if efuse_c & 0x20 != 0 {
            -(efuse_c & 0x1F) + 990
        } else {
            efuse_c + 990
        };
    }

    let mut min_diff: i32 = 400_000_000;
    let mut best_dref: u8 = 9;
    let mut best_mul: u8 = 6;
    for dref_val in 0..16i32 {
        let vref_20 = if dref_val < 9 {
            10 + dref_val
        } else {
            20 + (dref_val - 9) * 2
        };
        for mul_val in 0..8i32 {
            let vout = (vref_20 * k_1000 + 20 * vos_1000) * (4000 + mul_val * c_1000);
            let diff = (target_mv * 80_000 - vout).abs();
            if diff < min_diff {
                min_diff = diff;
                best_dref = dref_val as u8;
                best_mul = mul_val as u8;
            }
        }
    }
    (best_dref, best_mul)
}

pub struct LdoChannel {
    chan_id: u8,
}

impl LdoChannel {
    /// Acquire LDO_VO3 at 2500 mV for the DSI DPHY.
    ///
    /// # Safety
    /// Steals the global PAC `Peripherals` to reach `PMU.EXT_LDO_P0_0P2A`.
    /// Caller must ensure no other code is mutating PMU concurrently. Must
    /// be called before [`super::dsi_host::init`].
    pub unsafe fn acquire_dphy() -> Self {
        use esp32p4 as pac;
        let p = unsafe { pac::Peripherals::steal() };

        // BEETLE-05: resolve the eFuse-calibrated dref/mul for 2500 mV on
        // THIS chip. On a calibrated part (efuse trim programmed) this is
        // NOT 9/6 — and the previous hardcoded 9/6 sat at the wrong DPHY
        // voltage, the prime suspect for the PHY PLL not locking.
        let efuse_raw = p.EFUSE.rd_mac_sys_3().read().bits();
        let (dref, mul) = calibrated_dref_mul(efuse_raw, DPHY_MV);
        LAST_LDO_DREFMUL.store(
            dref.saturating_mul(10).saturating_add(mul),
            Ordering::Relaxed,
        );

        // ldo_ll_set_owner(unit=2, OWNER_SW): force_tieh_sel=1, tieh_sel=0.
        // ldo_ll_adjust_voltage(unit=2, dref, mul, use_rail_voltage=false):
        // tieh=0 in CTRL; dref/mul in ANA.
        p.PMU.ext_ldo_p0_0p2a().modify(|_, w| {
            w._0p2a_force_tieh_sel_0().set_bit();
            unsafe { w._0p2a_tieh_sel_0().bits(0) };
            w._0p2a_tieh_0().clear_bit();
            w
        });
        p.PMU.ext_ldo_p0_0p2a_ana().modify(|_, w| {
            unsafe {
                w.ana_0p2a_dref_0().bits(dref);
                w.ana_0p2a_mul_0().bits(mul);
            }
            // ldo_ll_enable_ripple_suppression(unit=2, true): en_vdet=1.
            // IDF calls this immediately before xpd=1; without it the
            // analog rail can oscillate enough to deassert the DPHY
            // ready signal mid-PHY-init.
            w.ana_0p2a_en_vdet_0().set_bit();
            w
        });
        // ldo_ll_enable(unit=2, true): xpd=1.
        p.PMU
            .ext_ldo_p0_0p2a()
            .modify(|_, w| w._0p2a_xpd_0().set_bit());

        // IDF doesn't poll a ready bit here — the analog rail settles in
        // < 1 ms. esp_ldo_acquire_channel waits internally on a delay.
        // Conservative spin: ~5000 NOPs ≈ several µs at 400 MHz.
        for _ in 0..5_000 {
            core::hint::spin_loop();
        }

        // BEETLE-05 diagnostic: confirm every field we wrote actually stuck.
        let ctrl = p.PMU.ext_ldo_p0_0p2a().read();
        let ana = p.PMU.ext_ldo_p0_0p2a_ana().read();
        let mut verify: u8 = 0;
        if ctrl._0p2a_xpd_0().bit_is_set() {
            verify |= 1 << 0;
        }
        if ctrl._0p2a_force_tieh_sel_0().bit_is_set() {
            verify |= 1 << 1;
        }
        if ctrl._0p2a_tieh_0().bit_is_clear() {
            verify |= 1 << 2;
        }
        if ana.ana_0p2a_dref_0().bits() == dref {
            verify |= 1 << 3;
        }
        if ana.ana_0p2a_mul_0().bits() == mul {
            verify |= 1 << 4;
        }
        LAST_LDO_VERIFY.store(verify, Ordering::Relaxed);

        Self {
            chan_id: super::DPHY_LDO_CHAN,
        }
    }
}
