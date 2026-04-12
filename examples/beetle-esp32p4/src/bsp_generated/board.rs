//! Board-level constants for DFR1172 FireBeetle 2 P4.
//!
//! Each pin label from `db/boards/dfr1172_fire_beetle_2_p4.yaml` becomes a `pub const`
//! GPIO number so application code can refer to board features by name
//! rather than magic pin numbers.

/// Board identifier.
pub const BOARD_NAME: &str = "DFR1172 FireBeetle 2 P4";

/// Chip targeted by this BSP.
pub const CHIP: &str = "ESP32-P4";

/// Chip package.
pub const PACKAGE: &str = "QFN104-7x7";

/// On-module flash capacity, megabytes.
pub const FLASH_MB: u32 = 16;

/// CPU clock frequency configured by [`crate::clocks::init`], in hertz.
pub const CPU_HZ: u32 = 400000000;

/// APB clock frequency configured by [`crate::clocks::init`], in hertz.
pub const APB_HZ: u32 = 100000000;

/// External crystal frequency, in hertz.
pub const XTAL_HZ: u32 = 40000000;

/// GPIO number for `led` (LED).
pub const LED: u8 = 3;

/// GPIO number for `boot_btn` (BOOT).
pub const BOOT_BTN: u8 = 35;

/// GPIO number for `touch_scl` (I2C0_SCL).
pub const TOUCH_SCL: u8 = 7;

/// GPIO number for `touch_sda` (I2C0_SDA).
pub const TOUCH_SDA: u8 = 8;
