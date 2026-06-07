//! Intermediate representation for Nordic nRF BSP generation.
//!
//! Nordic nRF chips (nRF52840, nRF5340, nRF9160) have a pin/peripheral model
//! fundamentally different from both STM32 and Espressif:
//!
//! - Any GPIO can be assigned to any peripheral signal via per-peripheral
//!   PSEL registers — no IO MUX function slots, no GPIO matrix.
//! - Multiple peripheral drivers (TWIM0, SPIM0, SPIS0, SPI0) share a single
//!   hardware instance slot. Only one can be enabled at a time.
//! - GPIO is addressed as (port, pin) pairs: P0.00–P0.31, P1.00–P1.15.
//! - The clock tree is much simpler: HFCLK (64 MHz) + LFCLK (32.768 kHz)
//!   with selectable sources (XTAL, RC, synth).
//! - Peripherals are enabled by writing a magic value to their ENABLE
//!   register (e.g. 8 for UARTE, 7 for SPIM, 6 for TWIM).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Direction of a pin signal.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NrfDir {
    /// Output from the chip.
    Out,
    /// Input to the chip.
    In,
    /// Bidirectional (e.g. I2C SDA/SCL).
    Inout,
}

/// Top-level resolved IR fed into the render pipeline.
///
/// Produced by [`super::merge`] from a chip spec and board spec.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfIr {
    /// Schema version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Chip inventory.
    pub chip: NrfChip,
    /// Board wiring.
    pub board: NrfBoard,
    /// Resolved clock configuration.
    pub clocks: NrfClocksConfig,
    /// Pin assignments copied from board for template convenience.
    pub pins: Vec<NrfPinAssignment>,
    /// Free-form metadata.
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

fn default_version() -> String {
    "0.1".to_string()
}

/// Chip inventory sourced from the Nordic Product Specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfChip {
    /// Marketing name, e.g. `"nRF52840"`.
    pub name: String,
    /// Architecture string, e.g. `"cortex-m4f"`.
    pub arch: String,
    /// Package identifier.
    pub package: String,
    /// PAC crate name, e.g. `"nrf52840_pac"`.
    pub pac_crate: String,
    /// GPIO port definitions.
    pub gpio_ports: Vec<NrfGpioPort>,
    /// Memory regions.
    pub memory: Vec<NrfMemoryRegion>,
    /// Clock tree.
    pub clock_tree: NrfClockTree,
    /// Shared peripheral instance slots.
    #[serde(default)]
    pub peripheral_slots: Vec<NrfPeripheralSlot>,
    /// Peripheral instances keyed by name.
    pub peripherals: IndexMap<String, NrfPeripheral>,
}

/// A GPIO port on the chip.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfGpioPort {
    /// Port number (0 or 1 on nRF52840).
    pub port: u8,
    /// Number of pins on this port.
    pub pin_count: u8,
}

/// Memory region.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfMemoryRegion {
    /// Region name (e.g. `"flash"`, `"ram"`).
    pub name: String,
    /// Base address.
    pub base: u32,
    /// Size in bytes.
    pub size: u32,
    /// Access: `"r"`, `"rw"`, `"rx"`, `"rwx"`.
    pub access: String,
}

/// Nordic clock tree — much simpler than ESP or STM32.
///
/// HFCLK runs at 64 MHz from either the external crystal (HFXO) or the
/// internal RC oscillator (HFINT). LFCLK runs at 32.768 kHz from LFXO,
/// LFRC, or synthesized from HFCLK.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfClockTree {
    /// High-frequency clock in Hz (typically 64 MHz).
    pub hfclk_hz: u32,
    /// Low-frequency clock in Hz (typically 32768).
    pub lfclk_hz: u32,
    /// Available HFCLK sources.
    pub hfclk_src: Vec<String>,
    /// Available LFCLK sources.
    pub lfclk_src: Vec<String>,
    /// Peripheral clock in Hz (16 MHz on nRF52).
    pub pclk16m_hz: u32,
}

/// Shared peripheral instance slot.
///
/// On nRF52840, TWIM0/SPIM0/SPIS0/TWIS0 share hardware slot 0 at
/// `0x40003000`. Only one can be enabled at a time via the ENABLE register.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfPeripheralSlot {
    /// Slot index.
    pub slot: u8,
    /// Peripheral instance names sharing this slot.
    pub instances: Vec<String>,
    /// Base address of the shared register block.
    pub base: u32,
    /// Shared IRQ number.
    pub irq: u8,
}

/// A peripheral instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfPeripheral {
    /// Peripheral class: `"uart"`, `"spi"`, `"i2c"`, `"pwm"`, `"timer"`, etc.
    pub class: String,
    /// Instance name.
    pub instance: String,
    /// Base address (for non-slot peripherals).
    #[serde(default)]
    pub base: u32,
    /// IRQ number (for non-slot peripherals).
    #[serde(default)]
    pub irq: Option<u8>,
    /// Slot index if this peripheral shares an instance slot.
    #[serde(default)]
    pub slot: Option<u8>,
    /// Value to write to the ENABLE register.
    #[serde(default)]
    pub enable_val: Option<u8>,
    /// PSEL signal roles available on this peripheral.
    #[serde(default)]
    pub psel: Vec<NrfPselRole>,
}

/// A PSEL register role on a peripheral.
///
/// The chip says *what* PSEL registers exist; the board says *which pins*
/// are wired to each role.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfPselRole {
    /// Role name (e.g. `"txd"`, `"rxd"`, `"sck"`, `"mosi"`, `"scl"`).
    pub role: String,
    /// Direction of this signal.
    pub direction: NrfDir,
}

/// Board description.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfBoard {
    /// Board's human-friendly name.
    pub name: String,
    /// Chip this board targets.
    pub chip: String,
    /// On-board flash capacity in MB.
    pub flash_mb: u32,
    /// Console peripheral config.
    #[serde(default)]
    pub console: Option<NrfConsoleConfig>,
    /// Pin assignments.
    pub pins: Vec<NrfPinAssignment>,
    /// Free-form feature map.
    #[serde(default)]
    pub features: IndexMap<String, String>,
    /// Per-SPI-instance configuration overrides.
    #[serde(default)]
    pub spi_configs: IndexMap<String, NrfSpiConfig>,
    /// Per-I2C-instance configuration overrides.
    #[serde(default)]
    pub i2c_configs: IndexMap<String, NrfI2cConfig>,
}

/// A board-level pin assignment with port/pin addressing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfPinAssignment {
    /// GPIO port (0 or 1).
    pub port: u8,
    /// GPIO pin within the port.
    pub pin: u8,
    /// Signal name from the board wiring.
    pub signal: String,
    /// Owning peripheral instance, if any.
    #[serde(default)]
    pub peripheral: Option<String>,
    /// PSEL role mapping to the peripheral's signal.
    #[serde(default)]
    pub role: Option<String>,
    /// Pin direction.
    pub direction: NrfDir,
    /// Optional label for generated constants.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional pull configuration: `"up"`, `"down"`, `"none"`.
    #[serde(default)]
    pub pull: Option<String>,
    /// Optional drive strength override.
    #[serde(default)]
    pub drive: Option<String>,
}

/// Console peripheral selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfConsoleConfig {
    /// Peripheral instance (e.g. `"uarte0"`).
    pub peripheral: String,
    /// Baud rate.
    pub baud: u32,
}

/// Resolved per-build clock configuration.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct NrfClocksConfig {
    /// HFCLK frequency in Hz.
    pub hfclk_hz: u32,
    /// LFCLK frequency in Hz.
    pub lfclk_hz: u32,
    /// Selected HFCLK source.
    pub hfclk_src: String,
    /// Selected LFCLK source.
    pub lfclk_src: String,
}

/// Board-level SPI configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfSpiConfig {
    /// SPI clock frequency in Hz.
    #[serde(default = "default_spi_freq")]
    pub frequency: u32,
    /// SPI mode (0-3).
    #[serde(default)]
    pub mode: u8,
}

fn default_spi_freq() -> u32 {
    4_000_000
}

impl Default for NrfSpiConfig {
    fn default() -> Self {
        Self {
            frequency: 4_000_000,
            mode: 0,
        }
    }
}

/// Board-level I2C configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrfI2cConfig {
    /// I2C clock frequency in Hz.
    #[serde(default = "default_i2c_freq")]
    pub frequency: u32,
}

fn default_i2c_freq() -> u32 {
    100_000
}

impl Default for NrfI2cConfig {
    fn default() -> Self {
        Self { frequency: 100_000 }
    }
}
