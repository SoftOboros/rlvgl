//! Silicon Labs BSP generator vendor path.
//!
//! Silicon Labs EFM32 Series 1 parts (EFM32GG11, EFM32xG12, …) carry
//! a peripheral routing model that does not map cleanly onto either
//! the STM32-shaped generic [`crate::ir::Ir`] or the Espressif
//! IO-MUX/GPIO-matrix shape:
//!
//! - Per-peripheral `ROUTELOC0` / `ROUTELOC1` register selects a
//!   fixed pin location for each role (TX, RX, CLK, CS, CTS, RTS,
//!   SCL, SDA, CC0..CC3) with a parallel `ROUTEPEN` enable register.
//! - Pin addressing uses `(port, pin)` tuples — `port` is `A..I`,
//!   `pin` is `0..15`. GPIO ports are independent register blocks
//!   on the chip rather than a single linear pad number.
//! - Clock gating lives in three CMU register families: HF peripheral
//!   bus (`cmu.hfperclken0/1`), HF system bus (`cmu.hfbusclken0`), and
//!   the LF clken bank (`cmu.lfaclken0`, `cmu.lfbclken0`,
//!   `cmu.lfeclken0`) for low-energy peripherals. EFM32GG11 has no
//!   per-peripheral reset register — RMU is soft-reset only.
//!
//! To keep the STM32 path untouched while still supporting the
//! `efm32gg11b-pac` target, this module carries its own intermediate
//! representation and loader that reads YAML chip / board specs from
//! the `rlvgl-chips-silabs` chipdb crate.
//!
//! See `docs/concepts/CHIPS-SILABS-00-CONCEPTS.md` for the frozen
//! decisions this adapter implements; §10.2 is the canonical
//! description of the ROUTELOC routing kind and §10.3 the port-pin
//! tuple shape.

pub mod ir;
pub mod load;
pub mod render;

pub use ir::{
    SilabsBoard, SilabsChip, SilabsClockTree, SilabsClocksConfig, SilabsConsoleConfig, SilabsDir,
    SilabsGpioInventory, SilabsI2cConfig, SilabsIr, SilabsLinker, SilabsMemoryRegion,
    SilabsPeripheral, SilabsPeripheralSignal, SilabsPinAssignment, SilabsRoutingKind,
    SilabsSystemGate,
};
pub use load::{
    load_board_db, load_board_file, load_chip_db, load_chip_file, merge, yaml_to_board,
    yaml_to_chip,
};
pub use render::{PinRoute, PinRouteKind, render_silabs_pac};
