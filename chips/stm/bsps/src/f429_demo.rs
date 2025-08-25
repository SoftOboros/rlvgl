// SPDX-License-Identifier: MIT
//! BSP module generated from `tests/data/gen_bsps/f429_demo.ioc`.
//!
//! Demonstrates a second STM32 board with basic pin data.

use crate::{BoardInfo, PinInfo};

/// Pin assignments for the F429 demo configuration.
pub const PINS: &[PinInfo] = &[
    PinInfo { pin: "PA1", signal: "GPIO_Output" },
];

/// Board info for the F429 demo configuration.
pub const INFO: BoardInfo = BoardInfo {
    board: "f429_demo",
    chip: "STM32F429",
    pins: PINS,
};
