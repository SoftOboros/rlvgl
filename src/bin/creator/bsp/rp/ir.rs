//! RP2040/RP2350 intermediate representation types.
//!
//! These types model the GPIO funcsel routing, RESETS subsystem, PLL clock
//! tree, and peripheral configuration needed to generate a PAC-style BSP
//! for Raspberry Pi RP2040 and RP2350 microcontrollers.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Pin direction.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RpDir {
    /// Output only.
    Out,
    /// Input only.
    In,
    /// Bidirectional.
    Inout,
}

// ---------------------------------------------------------------------------
// Chip IR
// ---------------------------------------------------------------------------

/// Top-level chip specification deserialized from `db/chips/<name>.yaml`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpChip {
    pub name: String,
    pub arch: String,
    /// Alternate architecture name (e.g. `"hazard3"` for RP2350 RISC-V core).
    #[serde(default)]
    pub alt_arch: Option<String>,
    pub cores: u8,
    pub max_freq_hz: u32,
    pub package: String,
    pub pac_crate: String,
    pub gpio_count: u8,
    #[serde(default)]
    pub memory: Vec<RpMemoryRegion>,
    pub clock_tree: RpClockTree,
    /// RESETS register bit assignments (peripheral name -> bit index).
    #[serde(default)]
    pub resets: IndexMap<String, u8>,
    #[serde(default)]
    pub peripherals: IndexMap<String, RpPeripheral>,
    /// FUNCSEL table: one entry per GPIO with available function selections.
    #[serde(default)]
    pub funcsel: Vec<RpFuncselPin>,
}

/// A contiguous memory region.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpMemoryRegion {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub access: String,
}

/// Clock tree configuration (XOSC, ROSC, PLLs, clock domains).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpClockTree {
    pub xosc_hz: u32,
    pub rosc_hz: u32,
    #[serde(default)]
    pub plls: Vec<RpPll>,
    #[serde(default)]
    pub clk_domains: Vec<RpClkDomain>,
}

/// A PLL definition.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpPll {
    pub name: String,
    pub ref_div: u8,
    pub vco_freq_hz: u64,
    pub post_div1: u8,
    pub post_div2: u8,
}

/// A clock domain with its default frequency.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpClkDomain {
    pub name: String,
    pub default_hz: u32,
}

/// A peripheral instance on the chip.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpPeripheral {
    pub class: String,
    pub instance: String,
    pub base: u32,
    pub irq: Option<u8>,
    pub resets_bit: String,
    /// Number of state machines (PIO only).
    #[serde(default)]
    pub state_machines: Option<u8>,
    #[serde(default)]
    pub signals: Vec<RpPeripheralSignal>,
}

/// A signal role on a peripheral (e.g. tx, rx, sda, scl).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpPeripheralSignal {
    pub role: String,
    pub direction: RpDir,
}

/// FUNCSEL table entry for a single GPIO pin.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpFuncselPin {
    pub gpio: u8,
    #[serde(default)]
    pub funcs: Vec<RpFuncselEntry>,
}

/// One function selection option for a GPIO pin.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpFuncselEntry {
    pub sel: u8,
    pub func: String,
}

// ---------------------------------------------------------------------------
// Board IR
// ---------------------------------------------------------------------------

/// Top-level board specification deserialized from `db/boards/<name>.yaml`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpBoard {
    pub name: String,
    pub chip: String,
    #[serde(default)]
    pub arch_select: Option<String>,
    #[serde(default)]
    pub flash_mb: Option<u32>,
    #[serde(default)]
    pub console: Option<RpConsoleConfig>,
    #[serde(default)]
    pub pins: Vec<RpPinAssignment>,
    #[serde(default)]
    pub features: IndexMap<String, String>,
    #[serde(default)]
    pub i2c_configs: IndexMap<String, RpI2cConfig>,
    #[serde(default)]
    pub spi_configs: IndexMap<String, RpSpiConfig>,
}

/// Console peripheral configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpConsoleConfig {
    pub peripheral: String,
    pub baud: u32,
}

/// A pin assignment from the board spec.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpPinAssignment {
    pub gpio: u8,
    pub signal: String,
    #[serde(default)]
    pub peripheral: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub direction: RpDir,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub pull: Option<String>,
}

/// I2C bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpI2cConfig {
    pub scl_hz: u32,
}

/// SPI bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpSpiConfig {
    pub clk_hz: u32,
    #[serde(default)]
    pub mode: u8,
}

// ---------------------------------------------------------------------------
// Merged / resolved IR
// ---------------------------------------------------------------------------

/// Fully resolved IR after merge.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpIr {
    pub chip: RpChip,
    pub board: RpBoard,
    pub clocks: RpClocksConfig,
    pub pins: Vec<RpPinAssignment>,
}

/// Resolved clock configuration for templates.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpClocksConfig {
    pub sys_hz: u32,
    pub ref_hz: u32,
    pub peri_hz: u32,
    pub usb_hz: u32,
    pub xosc_hz: u32,
}
