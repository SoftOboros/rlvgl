//! Intermediate representation for Microchip SAM Cortex-M BSP generation.
//!
//! Per `chipdb/rlvgl-chips-microchip/docs/CHIPS-MICROCHIP-00-CONCEPTS.md` §10,
//! the SAM D-class clock tree (GCLK + MCLK + PCHCTRL) is structurally
//! different from ESP's flat `system_gates` table. The Microchip IR therefore
//! carries its own type set rather than reusing [`super::super::espressif::ir::EspIr`].
//!
//! Key SAM differences captured here:
//!
//! - **Two-step peripheral clock gating** (CHIPS-MICROCHIP-00 §6 INV-MC1):
//!   `MCLK.APBxMASK` ungates the bus clock; `GCLK.PCHCTRL[n]` selects the
//!   generic-clock generator and enables the functional clock. The IR
//!   carries both via [`MicrochipChip::clock_tree`].
//! - **PMUX letter encoding** (§6 INV-MC2): pad alternate-function
//!   selectors are single-character strings (`"A"`..`"H"`, optional `"N"`),
//!   not integers. The IR preserves them as `String`; the renderer
//!   translates them to PMUX field bits at emission time.
//! - **SERCOM polymorphism** (§3 glossary): SERCOM instances are
//!   runtime-configurable as USART / I²C / SPI / etc. The chip YAML
//!   leaves `mode:` null on every SERCOM; the board YAML's `pins:` entries
//!   determine the role.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Direction of a board signal on a SAM pad.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MicrochipDir {
    /// Input to the chip.
    In,
    /// Output from the chip.
    Out,
    /// Bidirectional signal (I²C SDA/SCL, USB DM/DP).
    Inout,
}

/// Top-level resolved IR fed into the render pipeline.
///
/// [`MicrochipIr`] is the product of merging a chip spec, a board spec, and
/// any per-invocation overrides. Consumers should not construct it
/// manually — go through [`super::merge`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipIr {
    /// Schema version of the spec format. Currently `"0.1"`.
    #[serde(default = "default_version")]
    pub version: String,
    /// Chip inventory (from `db/chips/<chip>.yaml`).
    pub chip: MicrochipChip,
    /// Board wiring (from `db/boards/<board>.yaml`).
    pub board: MicrochipBoard,
    /// Resolved clock configuration for this build.
    pub clocks: MicrochipClocksConfig,
    /// Pin assignments copied from [`MicrochipBoard::pins`] for template
    /// convenience.
    pub pins: Vec<MicrochipPinAssignment>,
    /// Free-form metadata propagated into generated file headers.
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

fn default_version() -> String {
    "0.1".to_string()
}

/// Full chip inventory sourced from a Microchip SAM D-class datasheet.
///
/// This is the schema for `db/chips/<chip>.yaml` per CHIPS-MICROCHIP-00 §5.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipChip {
    /// Marketing name, e.g. `"ATSAMD51J19A"`.
    pub name: String,
    /// ISA string, one of `"cortex-m4f"` or `"cortex-m0+"`. Drives target
    /// triple selection.
    pub arch: String,
    /// Package identifier, e.g. `"TQFP64"`.
    pub package: String,
    /// PAC crate name used in generated code (`use <pac_crate> as pac;`).
    pub pac_crate: String,
    /// Minor-version pin (e.g. `"0.7"`), per §6 INV-MC5.
    pub pac_version: String,
    /// Authoritative datasheet citation (§0 authority policy).
    pub source: MicrochipSource,
    /// Memory map (flash, sram, backup ram).
    pub memory: Vec<MicrochipMemoryRegion>,
    /// Clock tree: GCLK generators + MCLK APB gates + PCHCTRL channels.
    pub clock_tree: MicrochipClockTree,
    /// Peripheral instances keyed by lowercase instance name
    /// (`sercom0`, `tc0`, `tcc0`, `adc0`, ...).
    pub peripherals: IndexMap<String, MicrochipPeripheral>,
    /// Per-pad PMUX function table; one entry per `(group, pin)`.
    pub io_mux: Vec<MicrochipIoMuxPad>,
    /// Linker region aliases consumed by the `cortex-m-rt` link script.
    /// Names which `memory:` entry backs each role.
    #[serde(default)]
    pub linker: Option<MicrochipLinker>,
}

/// Datasheet citation block from the chip YAML.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipSource {
    /// Microchip DS-number (e.g. `"DS60001507"`).
    pub datasheet: String,
    /// Revision letter or date (e.g. `"F"`, `"2020-09"`).
    pub revision: String,
    /// Optional errata sheet DS-number.
    #[serde(default)]
    pub errata: Option<String>,
}

/// Linker-region alias map for the chip.
///
/// Each field names one of the `name:` entries in [`MicrochipChip::memory`].
/// The renderer turns these into the `FLASH`/`RAM` regions
/// `cortex-m-rt`'s `link.x.in` expects.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipLinker {
    /// Memory region that holds `.vector_table`/`.text`/`.rodata`.
    pub region_text: String,
    /// Memory region that holds `.data`/`.bss`/`.heap`/`.stack`.
    pub region_data: String,
}

/// Contiguous memory region described by the chip memory map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipMemoryRegion {
    /// Region name (e.g. `"flash"`, `"ram"`, `"bkupram"`).
    pub name: String,
    /// Base address in the chip's address space.
    pub base: u32,
    /// Region size in bytes.
    pub size: u32,
    /// Access bitfield as a short string: `"r"`, `"rw"`, `"rx"`, or `"rwx"`.
    pub access: String,
}

/// SAM D-class clock tree per CHIPS-MICROCHIP-00 §7.
///
/// Holds the GCLK generator inventory, the MCLK APB bus-clock gate table,
/// and the GCLK PCHCTRL channel assignments. Frequencies are nominal targets
/// from the chip YAML; the bootloader/init path actually programs the
/// oscillators.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipClockTree {
    /// External crystal frequency in Hz (or null if board-supplied).
    #[serde(default)]
    pub xosc_hz: Option<u32>,
    /// DFLL48M target frequency in Hz (or null on chips without one).
    #[serde(default)]
    pub dfll_target_hz: Option<u32>,
    /// FDPLL target frequency in Hz (or null on chips without one).
    #[serde(default)]
    pub dpll_target_hz: Option<u32>,
    /// CPU clock frequency in Hz.
    pub cpu_hz: u32,
    /// APB-A divider output in Hz.
    pub apba_hz: u32,
    /// APB-B divider output in Hz.
    pub apbb_hz: u32,
    /// APB-C divider output in Hz. Null on D21-class chips without APB-C.
    #[serde(default)]
    pub apbc_hz: Option<u32>,
    /// APB-D divider output in Hz. Null on D21-class chips without APB-D.
    #[serde(default)]
    pub apbd_hz: Option<u32>,
    /// GCLK generator inventory (GCLK0..GCLK11 on D5x).
    pub gclk_generators: Vec<MicrochipGclkGenerator>,
    /// MCLK APB bus-clock gate table keyed by peripheral instance name.
    pub mclk_gates: IndexMap<String, MicrochipMclkGate>,
    /// GCLK PCHCTRL channel inventory keyed by channel role
    /// (e.g. `sercom0_core`, `dfll48_ref`).
    pub pchctrl_channels: IndexMap<String, MicrochipPchctrlChannel>,
}

/// One GCLK generator entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipGclkGenerator {
    /// Generator index (0..11 on D5x; 0..8 on D21).
    pub id: u8,
    /// Clock source identifier (e.g. `"dpll0"`, `"dfll48m"`, `"xosc32k"`).
    pub source: String,
    /// Division factor applied to the source.
    pub div: u32,
}

/// MCLK APB clock-gate descriptor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipMclkGate {
    /// MCLK register holding the gate bit (e.g. `"apbamask"`).
    pub reg: String,
    /// Field name on the register (e.g. `"sercom0_"`).
    pub field: String,
}

/// GCLK PCHCTRL channel descriptor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipPchctrlChannel {
    /// Channel index passed to `gclk.pchctrl(n)`.
    pub id: u8,
    /// Default GCLK generator binding (overridable in board YAML).
    pub generator: u8,
}

/// Peripheral instance per CHIPS-MICROCHIP-00 §8.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipPeripheral {
    /// Peripheral class (`"sercom"`, `"tc"`, `"tcc"`, `"adc"`, ...).
    pub class: String,
    /// PAC instance name mirrored from the `peripherals` map key.
    pub instance: String,
    /// Base address of the peripheral register block.
    pub base: u32,
    /// First NVIC interrupt vector for the peripheral, if any.
    #[serde(default)]
    pub irq: Option<u16>,
    /// MCLK `apbamask`/`apbbmask`/... field name to write `1` to. Null for
    /// AHB-only peripherals (USB on D5x).
    #[serde(default)]
    pub mclk_field: Option<String>,
    /// PCHCTRL channel index for the functional clock. Null for AHB-only
    /// peripherals.
    #[serde(default)]
    pub gclk_pchctrl_id: Option<u8>,
    /// SERCOM mode (`"usart"`, `"i2c_master"`, etc.). Null for non-SERCOM
    /// peripherals or for SERCOMs not assigned a role by the board.
    #[serde(default)]
    pub mode: Option<String>,
    /// Per-signal pad-routing entries (typically empty in the chip YAML;
    /// board YAML's `pins:` is canonical).
    #[serde(default)]
    pub signals: Vec<MicrochipPeripheralSignal>,
}

/// A single role within a peripheral.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipPeripheralSignal {
    /// Role identifier (`"tx"`, `"rx"`, `"sda"`, `"sck"`, ...).
    pub role: String,
    /// Direction relative to the chip.
    pub direction: MicrochipDir,
    /// Pad name in `PAxx` form (e.g. `"PB16"`).
    pub pad: String,
    /// PMUX function letter selecting this role on the pad.
    pub pmux: String,
}

/// Per-pad PMUX function table row.
///
/// Mirrors CHIPS-MICROCHIP-00 §9 schema: each pad has up to nine function
/// columns (A..H, plus N as a reserved/"no peripheral" slot on D21
/// superset) plus an analog assignment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipIoMuxPad {
    /// Pad name in `PAxx` / `PBxx` / `PCxx` / `PDxx` form.
    pub pad: String,
    /// PORT group index (0=A, 1=B, 2=C, 3=D).
    pub group: u8,
    /// Pin index within the group (0..31).
    pub pin: u8,
    /// PMUX function A.
    #[serde(default)]
    pub fn_a: Option<String>,
    /// PMUX function B.
    #[serde(default)]
    pub fn_b: Option<String>,
    /// PMUX function C.
    #[serde(default)]
    pub fn_c: Option<String>,
    /// PMUX function D.
    #[serde(default)]
    pub fn_d: Option<String>,
    /// PMUX function E.
    #[serde(default)]
    pub fn_e: Option<String>,
    /// PMUX function F.
    #[serde(default)]
    pub fn_f: Option<String>,
    /// PMUX function G.
    #[serde(default)]
    pub fn_g: Option<String>,
    /// PMUX function H.
    #[serde(default)]
    pub fn_h: Option<String>,
    /// PMUX function N (reserved/special, D21 superset).
    #[serde(default)]
    pub fn_n: Option<String>,
    /// Analog assignment (ADCx_AINn, DAC_VOUTn, etc.).
    #[serde(default)]
    pub analog: Option<String>,
}

/// Board description: which chip, which pads are wired where, what the
/// console peripheral is. This is the schema for `db/boards/<board>.yaml`
/// per CHIPS-MICROCHIP-00 §5.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipBoard {
    /// Board's human-friendly name.
    pub name: String,
    /// Chip spec this board targets (lookup into `db/chips/<name>.yaml`).
    pub chip: String,
    /// On-chip flash capacity in megabytes. May be fractional (e.g. 0.5).
    pub flash_mb: f32,
    /// Authoritative user-guide / pinout-sheet citation.
    pub source: MicrochipBoardSource,
    /// Console peripheral and baud rate, if any.
    #[serde(default)]
    pub console: Option<MicrochipConsoleConfig>,
    /// Pin assignments consumed by the BSP generator.
    pub pins: Vec<MicrochipPinAssignment>,
    /// Per-SERCOM I²C SCL frequency overrides keyed by instance name.
    #[serde(default)]
    pub i2c_configs: IndexMap<String, MicrochipI2cConfig>,
    /// Per-SERCOM SPI master configuration overrides keyed by instance name.
    #[serde(default)]
    pub spi_configs: IndexMap<String, MicrochipSpiConfig>,
    /// Free-form feature map (e.g. `led: pa23`).
    #[serde(default)]
    pub features: IndexMap<String, String>,
}

/// Board user-guide citation block from the board YAML.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipBoardSource {
    /// User-guide identifier (slug, product number, or DS-number).
    pub user_guide: String,
    /// Date the source was last read, or a board revision letter.
    pub revision: String,
    /// Optional pinout sheet URL.
    #[serde(default)]
    pub pinout_sheet: Option<String>,
}

/// A single board-level pad assignment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipPinAssignment {
    /// Pad name in `PAxx` / `PBxx` form.
    pub pad: String,
    /// Signal name (e.g. `"SERCOM5_PAD0"`, `"GPIO"`, `"USB_DM"`).
    pub signal: String,
    /// Optional owning peripheral instance.
    #[serde(default)]
    pub peripheral: Option<String>,
    /// Pin direction.
    pub direction: MicrochipDir,
    /// Optional silkscreen / header label used for `pub const` names.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional pull configuration: `"up"`, `"down"`, or null.
    #[serde(default)]
    pub pull: Option<String>,
    /// Optional drive-strength override (0..1).
    #[serde(default)]
    pub drive: Option<u8>,
}

/// Console peripheral selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipConsoleConfig {
    /// SERCOM instance serving as the console (e.g. `"sercom5"`).
    pub peripheral: String,
    /// Baud rate for the USART.
    pub baud: u32,
}

/// Per-SERCOM I²C timing override.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipI2cConfig {
    /// Target SCL frequency in Hz.
    pub scl_hz: u32,
}

impl Default for MicrochipI2cConfig {
    fn default() -> Self {
        Self { scl_hz: 100_000 }
    }
}

/// Per-SERCOM SPI master timing override.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicrochipSpiConfig {
    /// Target SPI clock frequency in Hz.
    #[serde(default = "default_spi_clk_hz")]
    pub clk_hz: u32,
    /// SPI mode (0..3), controls CPOL/CPHA.
    #[serde(default)]
    pub mode: u8,
}

fn default_spi_clk_hz() -> u32 {
    1_000_000
}

impl Default for MicrochipSpiConfig {
    fn default() -> Self {
        Self {
            clk_hz: 1_000_000,
            mode: 0,
        }
    }
}

/// Resolved per-build clock configuration.
///
/// Built by [`super::merge`] from the chip's [`MicrochipClockTree`] defaults
/// and any `--cpu-hz`/`--baud` overrides from the CLI.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MicrochipClocksConfig {
    /// Selected CPU frequency in Hz.
    pub cpu_hz: u32,
    /// APB-A frequency in Hz.
    pub apba_hz: u32,
    /// APB-B frequency in Hz.
    pub apbb_hz: u32,
    /// APB-C frequency in Hz (zero on D21-class chips without APB-C).
    pub apbc_hz: u32,
    /// APB-D frequency in Hz (zero on D21-class chips without APB-D).
    pub apbd_hz: u32,
    /// External crystal frequency in Hz (zero if none).
    pub xosc_hz: u32,
    /// DFLL48M target frequency in Hz (zero if not used).
    pub dfll_target_hz: u32,
    /// FDPLL target frequency in Hz (zero if not used).
    pub dpll_target_hz: u32,
    /// Optional peripheral kernel-clock generator overrides keyed by
    /// PCHCTRL channel role.
    #[serde(default)]
    pub kernels: IndexMap<String, u8>,
}
