//! Nordic nRF BSP generator vendor path.
//!
//! Nordic nRF chips use a PSEL-based pin routing model and shared
//! peripheral instance slots that require a vendor-specific IR distinct
//! from both the STM32 generic [`crate::ir::Ir`] and the Espressif
//! [`crate::bsp::espressif::EspIr`].
//!
//! The pipeline reads YAML chip/board specs from `rlvgl-chips-nrf`,
//! validates and merges them into [`NrfIr`], then renders PAC-level
//! init code via MiniJinja templates.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    NrfBoard, NrfChip, NrfClockTree, NrfClocksConfig, NrfConsoleConfig, NrfDir, NrfGpioPort,
    NrfI2cConfig, NrfIr, NrfMemoryRegion, NrfPeripheral, NrfPeripheralSlot, NrfPinAssignment,
    NrfPselRole, NrfSpiConfig,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{NrfPinRoute, NrfPinRouteKind, render_nrf_pac};
