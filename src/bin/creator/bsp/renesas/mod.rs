//! Renesas RA BSP generator.
//!
//! Provides vendor-specific IR types, YAML loading, merge validation, and
//! MiniJinja rendering for Renesas RA family microcontrollers (RA6M5, etc.).
//! The pin routing model uses PFS (Pin Function Select) registers rather than
//! the IOMUX of NXP, funcsel of RP, or GPIO matrix of Espressif.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    RenesasBoard, RenesasChip, RenesasClocksConfig, RenesasConsoleConfig, RenesasDir,
    RenesasI2cConfig, RenesasIr, RenesasPinAssignment, RenesasSpiConfig,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{RenesasPinRoute, RenesasPinRouteKind, render_renesas_pac};
