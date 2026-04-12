//! Renesas RA intermediate representation types.
//!
//! These types model the PFS (Pin Function Select) pin routing, MSTP clock
//! gating, and peripheral configuration needed to generate a PAC-style BSP
//! for Renesas RA family microcontrollers (RA6M5, etc.).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Pin direction.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RenesasDir {
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
pub struct RenesasChip {
    pub name: String,
    pub arch: String,
    #[serde(default)]
    pub core_features: Vec<String>,
    pub package: String,
    pub cpu_hz: u32,
    /// PAC crate name on crates.io, if one exists.
    #[serde(default)]
    pub pac_crate: Option<String>,
    /// Renesas FSP version this chip inventory was derived from.
    #[serde(default)]
    pub fsp_version: Option<String>,
    #[serde(default)]
    pub memory: Vec<RenesasMemoryRegion>,
    #[serde(default)]
    pub gpio_ports: Vec<RenesasGpioPort>,
    pub clock_tree: RenesasClockTree,
    #[serde(default)]
    pub peripherals: IndexMap<String, RenesasPeripheral>,
    #[serde(default)]
    pub pfs_table: Vec<RenesasPfsEntry>,
}

/// A contiguous memory region.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasMemoryRegion {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub access: String,
}

/// A GPIO port with its pin count.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasGpioPort {
    pub port: u8,
    pub pin_count: u8,
}

/// Clock tree configuration (CGC).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasClockTree {
    pub hoco_hz: u32,
    pub moco_hz: u32,
    pub loco_hz: u32,
    pub main_osc_hz: u32,
    pub sub_osc_hz: u32,
    pub pll_max_hz: u32,
    pub cpu_max_hz: u32,
    pub pclka_hz: u32,
    pub pclkb_hz: u32,
    pub pclkc_hz: u32,
    pub pclkd_hz: u32,
    /// Module Stop (MSTP) clock gate entries.
    #[serde(default)]
    pub mstp_gates: IndexMap<String, RenesasMstpGate>,
}

/// A single MSTP clock-gate entry: register name + bit index.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasMstpGate {
    pub reg: String,
    pub bit: u8,
}

/// A peripheral instance on the chip.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasPeripheral {
    pub class: String,
    pub instance: String,
    pub base: u32,
    #[serde(default)]
    pub irq: Option<u16>,
    #[serde(default)]
    pub irq_rxi: Option<u16>,
    #[serde(default)]
    pub irq_txi: Option<u16>,
    #[serde(default)]
    pub modes: Option<Vec<String>>,
    #[serde(default)]
    pub psel_signals: Vec<RenesasPselSignal>,
}

/// A signal role on a peripheral (e.g. txd, rxd, scl, sda).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasPselSignal {
    pub role: String,
    pub direction: RenesasDir,
    #[serde(default)]
    pub mode: Option<String>,
}

/// A PFS (Pin Function Select) table entry for one physical pin.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasPfsEntry {
    pub pin: RenesasPortPin,
    #[serde(default)]
    pub functions: Vec<RenesasPfsFunction>,
}

/// A port:pin address on the chip.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RenesasPortPin {
    pub port: u8,
    pub pin: u8,
}

/// One function selection option for a PFS entry.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasPfsFunction {
    /// PSEL field value (bits [28:24] of the PFS register).
    pub psel: u8,
    /// Signal name (e.g. `"SCI0_TXD"`).
    pub signal: String,
    /// Peripheral instance that owns this signal.
    pub peripheral: String,
    /// Role within the peripheral (e.g. `"txd"`, `"scl"`).
    pub role: String,
}

// ---------------------------------------------------------------------------
// Board IR
// ---------------------------------------------------------------------------

/// Top-level board specification deserialized from `db/boards/<name>.yaml`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasBoard {
    pub name: String,
    pub chip: String,
    #[serde(default)]
    pub flash_mb: Option<u32>,
    #[serde(default)]
    pub console: Option<RenesasConsoleConfig>,
    #[serde(default)]
    pub pins: Vec<RenesasPinAssignment>,
    /// SCI mode overrides (e.g. `{ "sci0": "uart" }`).
    #[serde(default)]
    pub sci_modes: IndexMap<String, String>,
    #[serde(default)]
    pub features: IndexMap<String, String>,
    #[serde(default)]
    pub i2c_configs: IndexMap<String, RenesasI2cConfig>,
    #[serde(default)]
    pub spi_configs: IndexMap<String, RenesasSpiConfig>,
}

/// Console peripheral configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasConsoleConfig {
    pub peripheral: String,
    #[serde(default)]
    pub sci_mode: Option<String>,
    pub baud: u32,
}

/// A pin assignment from the board spec.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasPinAssignment {
    pub port: u8,
    pub pin: u8,
    pub signal: String,
    #[serde(default)]
    pub peripheral: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub direction: RenesasDir,
    /// PSEL value to write into the PFS register for peripheral routing.
    #[serde(default)]
    pub psel: Option<u8>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub pull: Option<String>,
}

/// I2C bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasI2cConfig {
    pub frequency: u32,
}

/// SPI bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasSpiConfig {
    pub frequency: u32,
    #[serde(default)]
    pub mode: u8,
}

// ---------------------------------------------------------------------------
// Merged / resolved IR
// ---------------------------------------------------------------------------

/// Fully resolved IR after merge.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasIr {
    pub chip: RenesasChip,
    pub board: RenesasBoard,
    pub clocks: RenesasClocksConfig,
    pub pins: Vec<RenesasPinAssignment>,
}

/// Resolved clock configuration for templates.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenesasClocksConfig {
    pub cpu_hz: u32,
    pub pclka_hz: u32,
    pub pclkb_hz: u32,
    pub pclkc_hz: u32,
    pub pclkd_hz: u32,
}
