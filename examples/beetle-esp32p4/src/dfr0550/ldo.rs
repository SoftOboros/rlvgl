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

const DREF_FOR_2500MV: u8 = 9;
const MUL_FOR_2500MV: u8 = 6;

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

        // ldo_ll_set_owner(unit=2, OWNER_SW): force_tieh_sel=1, tieh_sel=0.
        // ldo_ll_adjust_voltage(unit=2, dref=9, mul=6, use_rail_voltage=false):
        // tieh=0 in CTRL; dref/mul in ANA.
        p.PMU.ext_ldo_p0_0p2a().modify(|_, w| {
            w._0p2a_force_tieh_sel_0().set_bit();
            unsafe { w._0p2a_tieh_sel_0().bits(0) };
            w._0p2a_tieh_0().clear_bit();
            w
        });
        p.PMU.ext_ldo_p0_0p2a_ana().modify(|_, w| {
            unsafe {
                w.ana_0p2a_dref_0().bits(DREF_FOR_2500MV);
                w.ana_0p2a_mul_0().bits(MUL_FOR_2500MV);
            }
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

        Self {
            chan_id: super::DPHY_LDO_CHAN,
        }
    }
}
