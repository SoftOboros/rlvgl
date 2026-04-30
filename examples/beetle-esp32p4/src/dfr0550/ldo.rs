//! DPHY LDO_VO3 power-up (2500 mV).
//!
//! ESP-IDF reference (verified-working call):
//!
//! ```c
//! esp_ldo_channel_config_t ldo_cfg = {
//!     .chan_id = 3,
//!     .voltage_mv = 2500,
//! };
//! esp_ldo_acquire_channel(&ldo_cfg, &dphy_ldo);
//! ```
//!
//! ESP32-P4 has four internal LDO channels (LDO_VO1..VO4). Channel 3
//! (LDO_VO3) is wired to the MIPI DSI DPHY supply on the DFR1172 module.
//! Without this rail at 2.5 V the DSI lanes are tri-stated and the bridge
//! sees no clock — typical symptom is a stuck white panel.
//!
//! TODO(TRM §LP_LDO / Reference Voltage Generator): configure the LDO
//!   control registers (`LP_LDO_LDO3_CTRL0`/`CTRL1` or equivalent) to set
//!   `LDO_LDO3_VMUX` for 2500 mV and assert the enable bit.
//!
//! TODO: spin-wait for the "ready" status bit before returning.

/// LDO channel handle. Opaque; carries any per-channel state we need to
/// release later.
#[allow(dead_code)]
pub struct LdoChannel {
    chan_id: u8,
}

#[allow(dead_code)]
impl LdoChannel {
    /// Acquire LDO_VO3 at 2500 mV for the DSI DPHY.
    ///
    /// # Safety
    /// Must be called before [`super::dsi_host::init`].
    pub unsafe fn acquire_dphy() -> Self {
        // TODO(phase 5b): write LP_LDO control registers.
        Self {
            chan_id: super::DPHY_LDO_CHAN,
        }
    }
}
