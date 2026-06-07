//! NXP i.MX RT BSP generator.
//!
//! Provides vendor-specific IR types, YAML loading, merge validation, and
//! MiniJinja rendering for NXP i.MX RT microcontrollers (RT1060, RT1170).

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    NxpBoard, NxpChip, NxpClocksConfig, NxpConsoleConfig, NxpDir, NxpI2cConfig, NxpIr,
    NxpPinAssignment, NxpSpiConfig,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{NxpPinDaisy, NxpPinRoute, render_nxp_pac};
