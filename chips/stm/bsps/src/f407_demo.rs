// SPDX-License-Identifier: MIT
//! BSP module generated from `tests/data/gen_bsps/f407_demo.ioc`.
//!
//! This placeholder demonstrates wiring generated boards into the
//! `rlvgl-stm-bsps` crate.

use crate::{BoardInfo, PinInfo};

/// Pin assignments for the F407 demo configuration.
pub const PINS: &[PinInfo] = &[
    PinInfo { pin: "PA0", signal: "GPIO_Input" },
];

/// Board info for the F407 demo configuration.
pub const INFO: BoardInfo = BoardInfo {
    board: "f407_demo",
    chip: "STM32F407",
    pins: PINS,
};
