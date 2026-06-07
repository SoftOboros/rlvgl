//! Espressif BSP generator vendor path.
//!
//! Espressif chips carry a richer pin/clock model than the STM32-shaped
//! generic [`crate::ir::Ir`] can express (four-slot IO MUX, GPIO matrix
//! signal routing, `SYSTEM`/`PCR` peripheral gates rather than RCC). To
//! keep the STM32 path untouched while still supporting esp-pac targets,
//! this module carries its own intermediate representation and loader
//! that reads YAML chip/board specs from the `rlvgl-chips-esp` chipdb
//! crate.
//!
//! The rendering pipeline (`render_esp_pac`) lives alongside in a future
//! `render.rs` submodule; this initial landing only covers the IR types
//! and load path.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    EspBoard, EspChip, EspClockTree, EspClocksConfig, EspConsoleConfig, EspDir,
    EspGpioMatrixSignal, EspI2cConfig, EspIoMuxPin, EspIr, EspMemoryRegion, EspPeripheral,
    EspPeripheralSignal, EspPinAssignment, EspSystemGate,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{PinRoute, PinRouteKind, render_esp_pac};
