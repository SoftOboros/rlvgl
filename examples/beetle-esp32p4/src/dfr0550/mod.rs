//! DFR0550-V2 5" 800×480 IPS DSI panel bring-up.
//!
//! This module groups the steps required to drive the panel attached via
//! the DFR1237 IO-expansion shield's Pi-style DSI FFC. Each submodule
//! corresponds to one phase of the verified IDF C reference:
//!
//! ```c
//! // /tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c
//! ```
//!
//! Verified working configuration (2026-04-29; see memory file
//! `project_dfr1237_dfr0550v2.md`):
//!
//! - PSRAM: octal HEX @ 200 MHz (`CONFIG_IDF_EXPERIMENTAL_FEATURES=y`)
//! - DPHY LDO: internal LDO_VO3 @ 2500 mV
//! - DSI host: 1 data lane @ 750 Mbps, RGB888 in/out
//! - DPI panel: 26 MHz pixel clock, 800×480
//! - Pi 7" video timing: HFP=1, HSA=2, HBP=46, VFP=7, VSA=2, VBP=21
//! - I2C bridge wake @ 0x45: POWERON=1 → wait PORTB.0 → PORTA=0x04 → PWM=255
//! - FB updates require `esp_cache_msync` (C2M direction) after CPU writes
//! - Continuous re-fill loop required (one-shot paint desyncs the bridge)
//!
//! Bring-up phase ordering (mirrors IDF `app_main`):
//!   1. [`psram`]      — enable octal HEX PSRAM at 200 MHz
//!   2. [`ldo`]        — acquire LDO_VO3 @ 2500 mV for the DSI DPHY
//!   3. [`i2c_bridge`] — wake the panel's STM32F072 bridge over I2C @ 0x45
//!   4. [`dsi_host`]   — configure DSI host PHY + link layer (1 lane @ 750 Mbps)
//!   5. [`dpi_panel`]  — configure DPI controller + framebuffer (26 MHz)
//!   6. [`cache`]      — writeback CPU caches before each DSI scan
//!
//! Each submodule is currently a stub. The signatures are stable; the
//! implementations are the next-session work.

#![allow(dead_code)]

pub mod cache;
pub mod clk_init;
pub mod dpi_panel;
pub mod dsi_host;
pub mod i2c0;
pub mod i2c_bridge;
pub mod ldo;
pub mod psram;
pub mod regi2c;

/// Native panel resolution (DFR0550-V2).
pub const H_RES: u16 = 800;
/// Native panel resolution (DFR0550-V2).
pub const V_RES: u16 = 480;

/// Pi 7" Touchscreen v1 video timing (matches the bridge's expectations).
pub const H_FRONT_PORCH: u16 = 1;
pub const H_SYNC: u16 = 2;
pub const H_BACK_PORCH: u16 = 46;
pub const V_FRONT_PORCH: u16 = 7;
pub const V_SYNC: u16 = 2;
pub const V_BACK_PORCH: u16 = 21;

/// DSI parameters that produced stable color cycling on 2026-04-29.
pub const DSI_LANES: u8 = 1;
pub const DSI_LANE_MBPS: u32 = 750;
pub const DPI_PIXEL_CLK_MHZ: u32 = 26;

/// DPHY LDO channel and voltage.
pub const DPHY_LDO_CHAN: u8 = 3;
pub const DPHY_LDO_MV: u16 = 2500;
