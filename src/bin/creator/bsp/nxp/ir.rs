//! NXP i.MX RT intermediate representation types.
//!
//! These types model the IOMUX pad routing, CCM clock gating, and
//! peripheral configuration needed to generate a PAC-style BSP.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Pin direction.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NxpDir {
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
pub struct NxpChip {
    pub name: String,
    pub arch: String,
    pub package: String,
    pub pac_crate: String,
    pub target: String,
    pub cpu_hz_max: u32,
    #[serde(default)]
    pub memory: Vec<NxpMemoryRegion>,
    #[serde(default)]
    pub cores: Vec<NxpCore>,
    #[serde(default)]
    pub gpio_ports: Vec<NxpGpioPort>,
    pub clock_tree: NxpClockTree,
    #[serde(default)]
    pub peripherals: IndexMap<String, NxpPeripheral>,
    #[serde(default)]
    pub iomux: Vec<NxpIomuxPad>,
    #[serde(default)]
    pub daisy_chain: Vec<NxpDaisyChain>,
}

/// A CPU core present on the chip.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpCore {
    pub name: String,
    pub arch: String,
}

/// A contiguous memory region.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpMemoryRegion {
    pub name: String,
    pub base: u32,
    pub size: u32,
    pub access: String,
}

/// A GPIO port with its pin count.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpGpioPort {
    pub port: u8,
    pub pin_count: u8,
}

/// Clock tree configuration (CCM).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpClockTree {
    pub xtal_hz: u32,
    #[serde(default)]
    pub plls: IndexMap<String, NxpPll>,
    pub cpu_hz: u32,
    pub ahb_hz: u32,
    pub ipg_hz: u32,
    #[serde(default)]
    pub ccgr_gates: IndexMap<String, NxpCcgrGate>,
}

/// A PLL definition.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpPll {
    #[serde(default)]
    pub base_hz: u64,
    pub output_range: Option<(u64, u64)>,
    pub configurable: Option<bool>,
}

/// A CCGR clock-gate entry: register index + field index.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpCcgrGate {
    pub ccgr: u8,
    pub field: u8,
}

/// A peripheral instance on the chip.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpPeripheral {
    pub class: String,
    pub instance: String,
    pub base: u32,
    pub irq: Option<u16>,
    #[serde(default)]
    pub signals: Vec<NxpPeripheralSignal>,
}

/// A signal role on a peripheral (e.g. tx, rx, scl, sda).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpPeripheralSignal {
    pub role: String,
    pub direction: NxpDir,
}

/// An IOMUX pad entry — the i.MX RT fixed-mux model.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpIomuxPad {
    pub pad: String,
    pub mux_reg: u32,
    pub pad_reg: u32,
    pub alt0: Option<String>,
    pub alt1: Option<String>,
    pub alt2: Option<String>,
    pub alt3: Option<String>,
    pub alt4: Option<String>,
    pub alt5: Option<String>,
    pub alt6: Option<String>,
    pub alt7: Option<String>,
    pub gpio_port: Option<u8>,
    pub gpio_pin: Option<u8>,
}

impl NxpIomuxPad {
    /// Returns the signal name for the given ALT function index (0..7).
    pub fn alt_signal(&self, alt: u8) -> Option<&str> {
        match alt {
            0 => self.alt0.as_deref(),
            1 => self.alt1.as_deref(),
            2 => self.alt2.as_deref(),
            3 => self.alt3.as_deref(),
            4 => self.alt4.as_deref(),
            5 => self.alt5.as_deref(),
            6 => self.alt6.as_deref(),
            7 => self.alt7.as_deref(),
            _ => None,
        }
    }
}

/// A daisy-chain (SELECT_INPUT) register entry.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpDaisyChain {
    pub signal: String,
    pub select_input_reg: u32,
    pub options: Vec<NxpDaisyOption>,
}

/// One option in a daisy-chain SELECT_INPUT register.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpDaisyOption {
    pub pad: String,
    pub alt: u8,
    pub daisy_val: u8,
}

// ---------------------------------------------------------------------------
// Board IR
// ---------------------------------------------------------------------------

/// Top-level board specification deserialized from `db/boards/<name>.yaml`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpBoard {
    pub name: String,
    pub chip: String,
    pub flash_type: Option<String>,
    pub flash_mb: Option<u32>,
    pub sdram_mb: Option<u32>,
    pub console: Option<NxpConsoleConfig>,
    #[serde(default)]
    pub pins: Vec<NxpPinAssignment>,
    #[serde(default)]
    pub features: IndexMap<String, String>,
    #[serde(default)]
    pub i2c_configs: IndexMap<String, NxpI2cConfig>,
    #[serde(default)]
    pub spi_configs: IndexMap<String, NxpSpiConfig>,
}

/// Console peripheral configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpConsoleConfig {
    pub peripheral: String,
    pub baud: u32,
}

/// A pin assignment from the board spec.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpPinAssignment {
    pub pad: String,
    pub signal: String,
    pub peripheral: Option<String>,
    pub role: Option<String>,
    pub alt: u8,
    pub direction: NxpDir,
    pub label: Option<String>,
    pub pull: Option<String>,
    pub drive_strength: Option<String>,
    pub slew_rate: Option<String>,
    pub speed: Option<String>,
}

/// I2C bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpI2cConfig {
    pub scl_hz: u32,
}

/// SPI bus configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpSpiConfig {
    pub clk_hz: u32,
    #[serde(default)]
    pub mode: u8,
}

// ---------------------------------------------------------------------------
// Merged / resolved IR
// ---------------------------------------------------------------------------

/// Fully resolved IR after merge.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpIr {
    pub chip: NxpChip,
    pub board: NxpBoard,
    pub clocks: NxpClocksConfig,
    pub pins: Vec<NxpPinAssignment>,
}

/// Resolved clock configuration for templates.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NxpClocksConfig {
    pub cpu_hz: u32,
    pub ahb_hz: u32,
    pub ipg_hz: u32,
    pub xtal_hz: u32,
}
