//! RP2040/RP2350 BSP generator.
//!
//! Provides vendor-specific IR types, YAML loading, merge validation, and
//! MiniJinja rendering for Raspberry Pi RP2040 and RP2350 microcontrollers.
//! The GPIO routing model uses funcsel-based pin muxing rather than the
//! fixed-mux IOMUX of NXP or the PSEL model of Nordic.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    RpBoard, RpChip, RpClocksConfig, RpConsoleConfig, RpDir, RpI2cConfig, RpIr, RpPinAssignment,
    RpSpiConfig,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{RpPinRoute, RpPinRouteKind, render_rp_pac};
