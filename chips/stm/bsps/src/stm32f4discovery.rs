// SPDX-License-Identifier: MIT
//! BSP module for the STM32F4DISCOVERY board.

use crate::{BoardInfo, PinInfo};

/// Pin assignments for STM32F4DISCOVERY.
pub const PINS: &[PinInfo] = &[
    PinInfo { pin: "PA0", signal: "USART2_TX" },
    PinInfo { pin: "PA1", signal: "USART2_RX" },
];

/// Board info for STM32F4DISCOVERY.
pub const INFO: BoardInfo = BoardInfo {
    board: "STM32F4DISCOVERY",
    chip: "STM32F407",
    pins: PINS,
};
