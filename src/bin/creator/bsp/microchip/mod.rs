//! Microchip BSP generator vendor path.
//!
//! Microchip SAM Cortex-M chips (SAM D5x / E5x / D21 / L21 / C / G / V /
//! S — the Atmel-lineage line) carry a clock-tree and pad-mux model that
//! does not collapse onto the STM32-shaped generic [`crate::ir::Ir`] or
//! the ESP `system_gates` flat table. Specifically, per
//! CHIPS-MICROCHIP-00 §10 / §6 INV-MC1 / INV-MC2:
//!
//! - **Two-step clock gating** — `MCLK.APBxMASK` (or D21's PM equivalent)
//!   ungates the bus clock; `GCLK.PCHCTRL[n]` selects the generic-clock
//!   generator and enables the functional clock.
//! - **PMUX letter encoding** — pad alternate-function selectors are
//!   single-character strings `"A"`..`"H"` (+ optional `"N"` for the D21
//!   superset), not 4-bit integers. The renderer packs adjacent pads as
//!   `pmuxe`/`pmuxo` halves of the SAM `PMUX` register.
//! - **SERCOM polymorphism** — each SERCOM instance is runtime-
//!   configurable as USART / I²C / SPI; the chip YAML leaves `mode:`
//!   null and the board YAML's `pins:` entries determine the role.
//!
//! Files in this module:
//!
//! - [`ir`] — `MicrochipIr` + the [`MicrochipChip`]/[`MicrochipBoard`]
//!   schema for YAML deserialisation.
//! - [`load`] — chip / board YAML loaders (from chipdb crate or from a
//!   disk path) plus the [`merge`] step.
//! - [`render`] — MiniJinja-based template renderer; produces six Rust
//!   files plus an optional `memory.x` per CHIPS-MICROCHIP-00 §6 INV-MC6.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    MicrochipBoard, MicrochipBoardSource, MicrochipChip, MicrochipClockTree, MicrochipClocksConfig,
    MicrochipConsoleConfig, MicrochipDir, MicrochipGclkGenerator, MicrochipI2cConfig,
    MicrochipIoMuxPad, MicrochipIr, MicrochipLinker, MicrochipMclkGate, MicrochipMemoryRegion,
    MicrochipPchctrlChannel, MicrochipPeripheral, MicrochipPeripheralSignal,
    MicrochipPinAssignment, MicrochipSource, MicrochipSpiConfig,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{PadRoute, render_microchip_pac};
