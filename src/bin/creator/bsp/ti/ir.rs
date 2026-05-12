//! Intermediate representation for Texas Instruments BSP generation.
//!
//! TI SimpleLink CC13x2 / CC26x2 (and siblings MSP432, AM335x, ...) use a
//! pin/peripheral model that does not map cleanly onto the STM32-centric
//! [`crate::ir::Ir`] or the Espressif [`crate::bsp::espressif::EspIr`]:
//!
//! - A single-stage IO Controller (IOC) writes one `IOCFGn` register per
//!   DIO; the `port_id` field of that register names the peripheral event
//!   signal directly. There is no second-stage GPIO matrix.
//! - Peripheral clocks are gated by a Power, Reset and Clock Module
//!   (PRCM) using **staged** writes: software sets the bit in
//!   `PRCM.<class>CLKGR.<field>`, pulses `PRCM.CLKLOADCTL.LOAD`, and
//!   polls `LOAD_DONE` before the gate change takes effect.
//! - On SimpleLink Cortex-M4F there is no PLL; the CPU runs at 48 MHz
//!   directly off SCLK_HF (XOSC_HF / RCOSC_HF / TCXO selectable).
//!
//! Each of those concerns lives in its own struct below. YAML chip and
//! board specs deserialise directly into [`TiChip`] and [`TiBoard`];
//! [`super::merge`] resolves them into a concrete [`TiIr`] for rendering.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Direction of a peripheral signal on a DIO (IOC pin).
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TiDir {
    /// Output from the chip.
    Out,
    /// Input to the chip.
    In,
    /// Bidirectional signal (e.g. I2C SDA/SCL, I2S AD0/AD1).
    Inout,
}

/// Top-level resolved IR fed into the render pipeline.
///
/// [`TiIr`] is the product of merging a chip spec and a board spec.
/// Consumers should not construct it manually — go through
/// [`super::merge`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiIr {
    /// Schema version of the spec format. Currently `"0.1"`.
    #[serde(default = "default_version")]
    pub version: String,
    /// Chip inventory (from `db/chips/<chip>.yaml`).
    pub chip: TiChip,
    /// Board wiring (from `db/boards/<board>.yaml`).
    pub board: TiBoard,
    /// Resolved clock configuration for this specific build.
    pub clocks: TiClocksConfig,
    /// Pin assignments copied from [`TiBoard::pins`] for template convenience.
    pub pins: Vec<TiPinAssignment>,
    /// Free-form metadata propagated into generated file headers.
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

fn default_version() -> String {
    "0.1".to_string()
}

/// Full chip inventory sourced from the vendor Technical Reference Manual.
///
/// This is the schema for `db/chips/<chip>.yaml` under
/// `chipdb/rlvgl-chips-ti/`. Authority: TI SWCU185 (CC13x2/CC26x2 TRM)
/// for SimpleLink chips; the chip yaml's top-level comment cites the
/// relevant TRM section for each populated field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiChip {
    /// Marketing name, e.g. `"CC1352R"`.
    pub name: String,
    /// ISA / core string, e.g. `"cortex-m4f"`.
    pub arch: String,
    /// Package identifier, e.g. `"RGZ"` (VQFN-48).
    pub package: String,
    /// PAC crate name used in generated code (`use <pac_crate> as pac;`).
    pub pac_crate: String,
    /// Optional Cargo version requirement for the PAC crate (e.g. `"^0.10"`).
    #[serde(default)]
    pub pac_crate_version: Option<String>,
    /// Optional Rust target triple (e.g. `"thumbv7em-none-eabihf"`).
    #[serde(default)]
    pub target_triple: Option<String>,
    /// Memory map regions sourced from the TRM Memory Map section.
    pub memory: Vec<TiMemoryRegion>,
    /// Clock tree topology (XOSC_HF / SCLK_HF / APB / RTC sources).
    pub clock_tree: TiClockTree,
    /// PRCM clock-gate / reset table keyed by peripheral instance name.
    ///
    /// Each entry names the PRCM register that holds the clock-enable bit
    /// and the per-class reset register. Bring-up sequence is documented
    /// in `templates/clocks.rs.jinja`.
    #[serde(default)]
    pub prcm: IndexMap<String, TiPrcmGate>,
    /// Peripheral instances keyed by instance name (`uart0`, `i2c0`, ...).
    pub peripherals: IndexMap<String, TiPeripheral>,
    /// IO Controller (IOC) per-pin table — one entry per DIO exposed by
    /// the chip package.
    pub io_mux: Vec<TiIoMuxPin>,
    /// GPIO matrix subset — the named peripheral event signals that the
    /// IOC PORT_CFG `port_id` field can resolve, with their `port_id`
    /// integer encoding from SWCU185 §11.2 Table 11-2.
    #[serde(default)]
    pub gpio_matrix: Vec<TiGpioMatrixSignal>,
    /// Linker layout — chip-level overrides consumed by the `memory.x`
    /// and `<chip>.x` templates. Optional; absent for chips that do not
    /// need a customer-configuration block.
    #[serde(default)]
    pub linker: Option<TiLinker>,
}

/// Linker-region alias map plus optional CCFG section parameters for the
/// chip.
///
/// `region_text` / `region_data` name entries in [`TiChip::memory`]; the
/// renderer maps these onto `cortex-m-rt`'s `REGION_TEXT` / `REGION_DATA`
/// aliases. `ccfg_origin` / `ccfg_length` define the SimpleLink Customer
/// Configuration block (TI SWCU185 §11.1) when present — the boot ROM
/// reads CCFG on every reset; absent or malformed CCFG prevents user
/// code from running.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiLinker {
    /// Memory region that holds `.init` / `.text` / `.rodata`
    /// (e.g. `"flash"`).
    pub region_text: String,
    /// Memory region that holds `.data` / `.bss` / `.heap` / `.stack`
    /// (e.g. `"sram"`).
    pub region_data: String,
    /// Origin address of the CCFG block in the chip address space.
    #[serde(default)]
    pub ccfg_origin: Option<u32>,
    /// Length of the CCFG block in bytes.
    #[serde(default)]
    pub ccfg_length: Option<u32>,
}

/// Contiguous memory region described by the chip memory map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiMemoryRegion {
    /// Region name (e.g. `"flash"`, `"sram"`, `"rom"`).
    pub name: String,
    /// Base address in the chip's address space.
    pub base: u32,
    /// Region size in bytes.
    pub size: u32,
    /// Access bitfield as a short string: `"r"`, `"rw"`, `"rx"`, or `"rwx"`.
    pub access: String,
}

/// Clock tree topology and source-selection maps.
///
/// SimpleLink Cortex-M4F runs at 48 MHz directly off SCLK_HF (either the
/// 48 MHz crystal XOSC_HF, the on-chip 48 MHz RCOSC_HF, or an external
/// TCXO). There is no PLL on SimpleLink so `cpu_hz` and `apb_hz` are
/// fixed at the configured SCLK_HF frequency.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiClockTree {
    /// High-frequency crystal frequency in Hz (XOSC_HF, typically 48 MHz).
    #[serde(default)]
    pub xtal_hi_hz: u32,
    /// Low-frequency crystal frequency in Hz (XOSC_LF, typically 32.768 kHz).
    #[serde(default = "default_xtal_lo_hz")]
    pub xtal_lo_hz: u32,
    /// Selected CPU frequency in Hz.
    pub cpu_hz: u32,
    /// Peripheral / APB bus frequency in Hz.
    pub apb_hz: u32,
    /// Valid source identifiers for SCLK_HF (e.g. `"xosc_hf"`,
    /// `"rcosc_hf"`, `"tcxo"`).
    #[serde(default)]
    pub sclk_hf_src: Vec<String>,
    /// Valid source identifiers for SCLK_LF.
    #[serde(default)]
    pub sclk_lf_src: Vec<String>,
}

fn default_xtal_lo_hz() -> u32 {
    32_768
}

/// Per-peripheral PRCM clock enable + reset gate entry.
///
/// `clk_en_reg` / `clk_en_field` and `rst_reg` / `rst_field` use dotted
/// PAC notation: `"prcm.uartclkgr"` / `"clk_en"`. The `clocks.rs.jinja`
/// template emits svd2rust-shaped writes like
/// `p.PRCM.uartclkgr().modify(|_, w| w.clk_en().set_bit())` followed by
/// the PRCM staged-write protocol pulse on `CLKLOADCTL.LOAD`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiPrcmGate {
    /// PAC path of the clock-enable register (e.g. `"prcm.uartclkgr"`).
    pub clk_en_reg: String,
    /// Field name within the clock-enable register (e.g. `"clk_en"`).
    pub clk_en_field: String,
    /// PAC path of the reset register (e.g. `"prcm.resetuart"`).
    pub rst_reg: String,
    /// Field name within the reset register (e.g. `"uart"`).
    pub rst_field: String,
}

/// Peripheral instance with its signal list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiPeripheral {
    /// Peripheral class (`"uart"`, `"spi_master"`, `"i2c"`, `"i2s"`,
    /// `"gptimer"`, `"watchdog"`, ...). Templates dispatch on this to
    /// pick the right init body.
    pub class: String,
    /// Instance name mirrored from the `peripherals` map key.
    pub instance: String,
    /// Base address of the peripheral register block in chip address space.
    pub base: u32,
    /// External NVIC IRQ number (svd2rust `pac::Interrupt` variant index,
    /// i.e. NVIC vector slot minus 16), if any.
    #[serde(default)]
    pub irq: Option<u8>,
    /// Signal list — per-role direction and IOC signal name.
    #[serde(default)]
    pub signals: Vec<TiPeripheralSignal>,
}

/// A single role within a peripheral (`tx`, `rx`, `sck`, ...).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiPeripheralSignal {
    /// Role identifier, e.g. `"tx"`, `"rx"`, `"sck"`, `"mosi"`, `"scl"`.
    pub role: String,
    /// Direction relative to the chip.
    pub direction: TiDir,
    /// IOC signal name as it appears in the chip's `gpio_matrix:` table
    /// (e.g. `"UART0_TX"`, `"I2C_MSSDA"`). The render pipeline looks
    /// this up against [`TiChip::gpio_matrix`] to obtain the runtime
    /// `port_id` encoding.
    #[serde(default)]
    pub ioc_signal: Option<String>,
}

/// Per-DIO entry in the chip's IOC table.
///
/// SimpleLink IOC is single-stage: each `IOCFGn` register selects the
/// peripheral event signal directly via `port_id`. So unlike Espressif
/// IO_MUX (which exposes 4..6 function slots per pad), this entry just
/// records the DIO's physical properties (pin number, high-drive class,
/// analog / JTAG alternates). The runtime `port_id` selection happens
/// in the generated `io_mux.rs` per-pin write.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiIoMuxPin {
    /// DIO number (0..30 on CC1352R RGZ).
    pub dio: u8,
    /// Physical package pin number.
    #[serde(default)]
    pub pin: Option<u8>,
    /// High-drive class flag (8 mA at IOCURR=2 instead of 4 mA at IOCURR=1).
    #[serde(default)]
    pub high_drive: bool,
    /// Analog-capable flag (AUX_ADC_DIO_n).
    #[serde(default)]
    pub analog_capable: bool,
    /// Secondary JTAG role on JTAG-multiplexed DIOs (e.g. `"TDO"`, `"TDI"`).
    #[serde(default)]
    pub jtag_alt: Option<String>,
}

/// GPIO matrix subset entry — named peripheral event signal with its
/// `port_id` integer encoding.
///
/// SimpleLink chips have a single-stage routing model: the IOC PORT_CFG
/// register's `port_id` field directly names the peripheral signal. This
/// table enumerates the valid `port_id` values (e.g. `0x00 = GPIO`,
/// `0x07 = UART0_TX`, `0x24 = I2C_MSSDA`) — drawn from SWCU185 §11.2
/// Table 11-2. The render pipeline uses these to resolve a board pin's
/// `signal:` token into the integer value to write to `IOCFGn.PORT_CFG`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiGpioMatrixSignal {
    /// PORT_CFG `port_id` integer encoding (e.g. `0x07` for `UART0_TX`).
    pub id: u16,
    /// Human-readable signal name from the TRM.
    pub name: String,
    /// Direction relative to the chip.
    pub direction: TiDir,
}

/// Board description: what chip, which pins are wired where, what the
/// console peripheral is. This is the schema for `db/boards/<board>.yaml`
/// under `chipdb/rlvgl-chips-ti/`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiBoard {
    /// Board's human-friendly name (e.g. `"LAUNCHXL-CC1352R1"`).
    pub name: String,
    /// Chip spec this board targets (lookup into `db/chips/<chip>.yaml`).
    pub chip: String,
    /// Optional module / launchpad identifier.
    #[serde(default)]
    pub module: Option<String>,
    /// On-module flash capacity in megabytes.
    #[serde(default)]
    pub flash_mb: u32,
    /// Pin assignments consumed by the BSP generator.
    #[serde(default)]
    pub pins: Vec<TiPinAssignment>,
    /// Free-form feature map (e.g. `led: dio6`).
    #[serde(default)]
    pub features: IndexMap<String, String>,
    /// Console peripheral and baud rate for `println!` / panic output.
    #[serde(default)]
    pub console: Option<TiConsoleConfig>,
    /// Optional per-I2C-instance timing overrides keyed by peripheral
    /// instance name (e.g. `"i2c0"`). Missing entries default to 100 kHz.
    #[serde(default)]
    pub i2c_configs: IndexMap<String, TiI2cConfig>,
}

/// A single board-level pin assignment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiPinAssignment {
    /// DIO number being assigned.
    pub dio: u8,
    /// Signal name from the board wiring (e.g. `"UART0_TX"`, `"LED"`).
    pub signal: String,
    /// Optional label used for generated pin constants (becomes a
    /// `pub const` DIO number in `board.rs`).
    #[serde(default)]
    pub label: Option<String>,
    /// Optional peripheral instance that owns this signal.
    #[serde(default)]
    pub peripheral: Option<String>,
    /// Pin direction.
    pub direction: TiDir,
    /// Optional internal pull configuration: `"up"`, `"down"`, or `"none"`.
    #[serde(default)]
    pub pull: Option<String>,
}

/// Console peripheral selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiConsoleConfig {
    /// Peripheral instance serving as the console (`"uart0"`).
    pub peripheral: String,
    /// Baud rate (UART consoles).
    pub baud: u32,
}

/// Board-level I2C timing config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TiI2cConfig {
    /// Target SCL frequency in Hz (typical: 100_000 or 400_000).
    pub scl_hz: u32,
}

impl Default for TiI2cConfig {
    fn default() -> Self {
        Self { scl_hz: 100_000 }
    }
}

/// Resolved per-build clock configuration.
///
/// Generated by [`super::merge`] from the chip's [`TiClockTree`] defaults
/// and any `--cpu-hz` overrides from the CLI. SimpleLink does not have a
/// PLL, so `cpu_hz` and `apb_hz` are simply propagated from the chip
/// inventory.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TiClocksConfig {
    /// Selected CPU frequency in Hz.
    pub cpu_hz: u32,
    /// Selected APB / peripheral bus frequency in Hz.
    pub apb_hz: u32,
    /// Backwards-compatible alias for `xtal_hi_hz` for templates that
    /// reference `ir.clocks.xtal_hz`. Matches `xtal_hi_hz` when both are
    /// populated.
    pub xtal_hz: u32,
    /// High-frequency crystal frequency in Hz (XOSC_HF).
    pub xtal_hi_hz: u32,
    /// Low-frequency crystal frequency in Hz (XOSC_LF).
    pub xtal_lo_hz: u32,
    /// Optional peripheral kernel clock source overrides.
    #[serde(default)]
    pub kernels: IndexMap<String, String>,
}
