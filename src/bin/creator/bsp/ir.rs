//! IR types for BSP generator tests.
//!
//! Provides a minimal schema used by unit tests to validate the
//! `.ioc` → IR → template pipeline without relying on any
//! vendor-specific tables.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Top-level intermediate representation describing the board.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Ir {
    /// Microcontroller identifier, e.g. "STM32H747XIHx".
    pub mcu: String,
    /// Package identifier, e.g. "LQFP176".
    pub package: String,
    /// Clock tree configuration including PLL parameters and kernel muxes.
    pub clocks: Clocks,
    /// Pin configuration entries.
    pub pinctrl: Vec<Pin>,
    /// Discovered peripherals keyed by instance name.
    pub peripherals: IndexMap<String, Peripheral>,
}

/// Clock configuration extracted from the vendor project.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Clocks {
    /// Phase-locked loop settings keyed by name (`pll1`, `pll2`, ...).
    #[serde(default)]
    pub pll: IndexMap<String, Pll>,
    /// Kernel clock selections per peripheral (`usart1` → `pclk2`).
    #[serde(default)]
    pub kernels: IndexMap<String, String>,
}

/// PLL parameter block.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pll {
    /// Pre-divider value.
    pub m: u8,
    /// Multiplier value.
    pub n: u16,
    /// Post-divider P output.
    pub p: u8,
    /// Post-divider Q output.
    pub q: u8,
    /// Post-divider R output.
    pub r: u8,
}

/// Pin description capturing function, label, and alternate function number.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pin {
    /// Pin name, e.g. "PA9".
    pub pin: String,
    /// Signal name, e.g. "USART1_TX" or "GPIO_Output".
    pub func: String,
    /// Optional user-assigned label from the vendor project (e.g. `GPIO_Label` in `.ioc`).
    ///
    /// When present, code generators can surface this label in comments or use
    /// it to derive identifier aliases.
    #[serde(default)]
    pub label: Option<String>,
    /// Alternate function number for the signal (0 for GPIO).
    pub af: u8,
}

/// Peripheral description with class and signal-to-pin mapping.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Peripheral {
    /// Class name aligned with `embedded-hal` families, e.g. "serial".
    pub class: String,
    /// Mapping of signal role (tx, rx, sck, …) to pin name.
    #[serde(default)]
    pub signals: IndexMap<String, String>,
}
