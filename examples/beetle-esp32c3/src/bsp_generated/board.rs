//! Board-level constants for DFR0868 Beetle ESP32-C3.
//!
//! Each pin label from `db/boards/dfr0868_beetle_esp32_c3.yaml` becomes a `pub const`
//! GPIO number so application code can refer to board features by name
//! rather than magic pin numbers.

/// Board identifier.
pub const BOARD_NAME: &str = "DFR0868 Beetle ESP32-C3";

/// Chip targeted by this BSP.
pub const CHIP: &str = "ESP32-C3";

/// Chip package.
pub const PACKAGE: &str = "QFN32-5x5";

/// On-module flash capacity, megabytes.
pub const FLASH_MB: u32 = 4;

/// CPU clock frequency configured by [`crate::clocks::init`], in hertz.
pub const CPU_HZ: u32 = 160000000;

/// APB clock frequency configured by [`crate::clocks::init`], in hertz.
pub const APB_HZ: u32 = 80000000;

/// External crystal frequency, in hertz.
pub const XTAL_HZ: u32 = 40000000;

/// GPIO number for `sda` (I2C0_SDA).
pub const SDA: u8 = 1;

/// GPIO number for `scl` (I2C0_SCL).
pub const SCL: u8 = 2;

/// GPIO number for `usb_dm` (USB_DM).
pub const USB_DM: u8 = 18;

/// GPIO number for `usb_dp` (USB_DP).
pub const USB_DP: u8 = 19;

/// GPIO number for `led` (LED).
pub const LED: u8 = 8;
