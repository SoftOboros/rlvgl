//! DSI host bring-up (1 lane @ 750 Mbps, RGB888).
//!
//! ESP-IDF reference (verified-working call):
//!
//! ```c
//! esp_lcd_dsi_bus_config_t dsi_bus_cfg = {
//!     .bus_id = 0,
//!     .num_data_lanes = 1,
//!     .phy_clk_src = MIPI_DSI_PHY_CLK_SRC_DEFAULT,
//!     .lane_bit_rate_mbps = 750,
//! };
//! esp_lcd_new_dsi_bus(&dsi_bus_cfg, &dsi_bus);
//! ```
//!
//! The bridge has a narrow PHY-PLL lock window — see "Failed configurations"
//! in the memory file. 1000 Mbps gives black, 500 Mbps gives "almost
//! green". 750 Mbps is the only verified working rate.
//!
//! Although the chipdb labels D1+/D1- (chip pins 35/36), the lane is
//! disabled at 1-lane operation. The kernel driver writes
//! `DSI_LANEENABLE = CLOCK | D0` (0x03), and the IDF host follows the
//! same convention.
//!
//! TODO(TRM §MIPI DSI Host Controller): drive the DSI host registers:
//!   - PHY config: M/N PLL dividers for 750 Mbps from DEFAULT clock src
//!   - Lane enable: clock + D0 only
//!   - Timing: ULPS, HS-prepare, HS-exit, BTA — start from kernel values
//!   - Calibration: trigger PHY auto-calibration and poll done
//!
//! TODO(TRM §HP_SYS_CLKRST): un-gate `DSI_HOST_CLK` and release reset.

#![allow(dead_code)]

/// Opaque DSI host handle returned by [`init`].
pub struct DsiBus {
    _private: (),
}

/// Initialize the DSI host PHY and link layer at 1 lane × 750 Mbps.
///
/// # Safety
/// Caller must have powered the DPHY rail via [`super::ldo::LdoChannel`].
pub unsafe fn init() -> Result<DsiBus, DsiError> {
    // TODO(phase 5b)
    Err(DsiError::Unimplemented)
}

#[derive(Debug)]
pub enum DsiError {
    Unimplemented,
    PllLock,
    LaneCal,
}
