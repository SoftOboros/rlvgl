//! Texas Instruments BSP generator vendor path.
//!
//! TI SimpleLink chips (CC13x2 / CC26x2 family, MSP432, AM335x, ...) use a
//! pin/peripheral model distinct from both the STM32 generic
//! [`crate::ir::Ir`] and the Espressif
//! [`crate::bsp::espressif::EspIr`]. The defining differences (frozen by
//! `chipdb/rlvgl-chips-ti/docs/CHIPS-TI-00-CONCEPTS.md` §10):
//!
//! - **Single-stage IOC routing** (§10.3): each DIO is configured by one
//!   PORT_CFG (`IOCFGn`) register that names the peripheral event signal
//!   directly via the `port_id` field — there is no separate GPIO matrix
//!   stage as on Espressif chips.
//! - **PRCM staged clock gating** (§10.2): peripheral clock enable bits
//!   live in `PRCM.<class>CLKGR.<field>`; the write does not take effect
//!   until software pulses `PRCM.CLKLOADCTL.LOAD` and polls `LOAD_DONE`.
//!   Resets are released by clearing per-class `PRCM.RESET<class>` bits.
//! - **No PLL on SimpleLink Cortex-M4F**: the CPU runs at 48 MHz directly
//!   off SCLK_HF (XOSC_HF / RCOSC_HF / TCXO selectable). Other TI
//!   families (MSP432P, AM335x) carry their own clock-tree shapes and
//!   are accommodated by additional optional fields rather than separate
//!   IR types.
//!
//! The pipeline reads YAML chip/board specs from the `rlvgl-chips-ti`
//! chipdb crate (`db/chips/<chip>.yaml`, `db/boards/<board>.yaml`),
//! merges them into [`TiIr`], then renders PAC-level init code via
//! MiniJinja templates under `bsp/ti/templates/`.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    TiBoard, TiChip, TiClockTree, TiClocksConfig, TiConsoleConfig, TiDir, TiGpioMatrixSignal,
    TiI2cConfig, TiIoMuxPin, TiIr, TiLinker, TiMemoryRegion, TiPeripheral, TiPeripheralSignal,
    TiPinAssignment, TiPrcmGate,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{PinRoute, PinRouteKind, render_ti_pac};
