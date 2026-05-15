//! Intermediate representation for Espressif BSP generation.
//!
//! The ESP32-C3 (and siblings C6/S3/H2/P4) use a pin/peripheral model that
//! does not map cleanly onto the STM32-centric [`crate::ir::Ir`]:
//!
//! - Up to four function selectors per GPIO (`fn0..fn3`) instead of an 8-bit
//!   alternate function number.
//! - A GPIO matrix that routes any peripheral signal to any GPIO by signal
//!   ID, in parallel with the direct IO MUX fast path.
//! - Peripheral clock/reset gates in either `SYSTEM` (C3) or `PCR` (C6/H2)
//!   with per-peripheral enable and reset bits.
//! - Distinct XTAL/PLL/CPU/APB/RTC clock domains rather than the STM32
//!   HSE/PLL/D1/D2/D3 shape.
//!
//! Each of those concerns lives in its own struct in this module. YAML chip
//! and board specs deserialise directly into [`EspChip`] and [`EspBoard`];
//! the pipeline then resolves them into a concrete [`EspIr`] for rendering.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Direction of a peripheral signal on a GPIO pin.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EspDir {
    /// Output from the chip.
    Out,
    /// Input to the chip.
    In,
    /// Bidirectional signal (e.g. I2C SDA, USB D+/D-).
    Inout,
}

/// Top-level resolved IR fed into the render pipeline.
///
/// [`EspIr`] is the product of merging a chip spec, a board spec, and any
/// per-invocation overrides (CPU frequency, console baud, …). Consumers
/// should not construct it manually — go through [`super::merge`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspIr {
    /// Schema version of the spec format. Currently `"0.1"`.
    #[serde(default = "default_version")]
    pub version: String,
    /// Chip inventory (from `db/chips/<chip>.yaml`).
    pub chip: EspChip,
    /// Board wiring (from `db/boards/<board>.yaml`).
    pub board: EspBoard,
    /// Resolved clock configuration for this specific build.
    pub clocks: EspClocksConfig,
    /// Pin assignments copied from [`EspBoard::pins`] for template convenience.
    pub pins: Vec<EspPinAssignment>,
    /// Free-form metadata propagated into generated file headers.
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

fn default_version() -> String {
    "0.1".to_string()
}

fn default_pac_vintage() -> String {
    "legacy".to_string()
}

/// Full chip inventory sourced from the vendor Technical Reference Manual.
///
/// This is the schema for `db/chips/<chip>.yaml`. Every field is populated
/// from the TRM so that the render pipeline does not need to hard-code
/// chip-specific constants.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspChip {
    /// Marketing name, e.g. `"ESP32-C3"`.
    pub name: String,
    /// ISA string, e.g. `"rv32imc"` or `"xtensa-lx7"`.
    pub arch: String,
    /// Package identifier, e.g. `"QFN32-5x5"`.
    pub package: String,
    /// PAC crate name used in generated code (`use <pac_crate> as pac;`).
    pub pac_crate: String,
    /// PAC API vintage. `"legacy"` (default) means the upstream PAC exposes
    /// a top-level `pub struct Peripherals` with `take()` / `steal()` — the
    /// pre-svd2rust-0.37 shape used by `esp32c3 = 0.31` and `esp32p4 = 0.2`.
    /// `"modern"` means the upstream PAC dropped the aggregate `Peripherals`
    /// struct in favour of per-peripheral `pac::FOO::steal()` (svd2rust 0.37+
    /// shape used by `esp32c6 = 0.23`, `esp32h2 = 0.19`, `esp32c5 = 0.2`,
    /// `esp32c61 = 0.3`). When `modern`, `pac.rs.jinja` emits a local
    /// `Peripherals` shim struct so the rest of the generated code keeps the
    /// same `p.UART0.foo()` field-access shape across vintages.
    #[serde(default = "default_pac_vintage")]
    pub pac_vintage: String,
    /// Number of user-accessible GPIO pads on this chip.
    pub gpio_count: u8,
    /// Memory map (ROM, SRAM, cache windows, RTC).
    pub memory: Vec<EspMemoryRegion>,
    /// Clock tree and per-peripheral clock/reset gates.
    pub clock_tree: EspClockTree,
    /// Peripheral instances keyed by instance name (`uart0`, `spi2`, ...).
    pub peripherals: IndexMap<String, EspPeripheral>,
    /// Per-GPIO IO MUX function table (F0..F3 on C3).
    pub io_mux: Vec<EspIoMuxPin>,
    /// GPIO matrix signal ID ↔ name table.
    pub gpio_matrix: Vec<EspGpioMatrixSignal>,
    /// Linker layout — which memory regions back the
    /// `REGION_TEXT`/`REGION_DATA`/etc aliases that `riscv-rt` and
    /// `esp-riscv-rt` expect. Only required for RISC-V chips that consume
    /// the bsp_pac flashable path; absent on Xtensa entries that go
    /// through esp-hal.
    #[serde(default)]
    pub linker: Option<EspLinker>,
}

/// Linker-region alias map for the chip.
///
/// Each field names one of the `name:` entries in [`EspChip::memory`]. The
/// renderer turns these into `REGION_ALIAS("REGION_TEXT", FLASH_CACHE)` and
/// friends in the generated `memory.x`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspLinker {
    /// Memory region that holds `.init`/`.text`/`.rodata` (e.g. `"flash_cache"`).
    pub region_text: String,
    /// Memory region that holds `.data`/`.bss`/`.heap`/`.stack` (e.g. `"hp_l2mem"`).
    pub region_data: String,
}

/// Contiguous memory region described by the chip memory map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspMemoryRegion {
    /// Region name (e.g. `"iram"`, `"dram"`, `"rtc_fast"`).
    pub name: String,
    /// Base address in the chip's address space.
    pub base: u32,
    /// Region size in bytes.
    pub size: u32,
    /// Access bitfield as a short string: `"r"`, `"rw"`, `"rx"`, or `"rwx"`.
    pub access: String,
}

/// Clock tree topology and per-peripheral gates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspClockTree {
    /// External crystal frequency in Hz (e.g. `40_000_000`).
    pub xtal_hz: u32,
    /// Supported PLL output frequencies in Hz.
    pub pll_freqs_hz: Vec<u32>,
    /// CPU frequency options selectable via the CPU clock mux.
    pub cpu_freqs_hz: Vec<u32>,
    /// Nominal APB frequency in Hz (peripheral reference clock).
    pub apb_hz: u32,
    /// Valid source identifiers for the RTC_FAST clock.
    pub rtc_fast_src: Vec<String>,
    /// Valid source identifiers for the RTC_SLOW clock.
    pub rtc_slow_src: Vec<String>,
    /// Peripheral clock/reset gate table keyed by peripheral instance.
    ///
    /// On ESP32-C3 these live in the `SYSTEM` block (Chapter 16 SYSREG).
    /// On C6/H2 the same concept is in the `PCR` block.
    pub system_gates: IndexMap<String, EspSystemGate>,
}

/// Per-peripheral clock enable, reset, and optional kernel mux.
///
/// `clk_en_reg`/`clk_en_field` and `rst_reg`/`rst_field` use dotted PAC
/// notation: `"system.perip_clk_en0"` / `"uart_clk_en"`. Templates render
/// these into `pac.system().perip_clk_en0().modify(|_, w| w.uart_clk_en()
/// .set_bit())` calls.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspSystemGate {
    /// PAC path of the clock-enable register.
    pub clk_en_reg: String,
    /// Field name within the clock-enable register.
    pub clk_en_field: String,
    /// PAC path of the reset register (often the same as `clk_en_reg`).
    pub rst_reg: String,
    /// Field name within the reset register.
    pub rst_field: String,
    /// Optional kernel clock mux register (e.g. UART0 sclk select).
    #[serde(default)]
    pub clk_sel_reg: Option<String>,
    /// Optional kernel clock mux field.
    #[serde(default)]
    pub clk_sel_field: Option<String>,
}

/// Peripheral instance with its signal list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspPeripheral {
    /// Peripheral class (`"uart"`, `"spi_master"`, `"i2c"`, …). Templates
    /// dispatch on this to pick the right init sequence.
    pub class: String,
    /// Instance name mirrored from the `peripherals` map key.
    pub instance: String,
    /// Base address of the peripheral register block.
    pub base: u32,
    /// Interrupt source number in the CPU interrupt matrix, if any.
    #[serde(default)]
    pub irq: Option<u8>,
    /// Signal list: per-role GPIO matrix ID and/or direct IO MUX fast path.
    #[serde(default)]
    pub signals: Vec<EspPeripheralSignal>,
}

/// A single role within a peripheral (`tx`, `rx`, `sck`, ...).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspPeripheralSignal {
    /// Role identifier, e.g. `"tx"`, `"rx"`, `"sck"`, `"mosi"`, `"cs0"`.
    pub role: String,
    /// Direction relative to the chip.
    pub direction: EspDir,
    /// GPIO matrix signal index used by
    /// `GPIO.func<id>_{out,in}_sel_cfg`. `None` when this signal cannot be
    /// routed through the matrix.
    #[serde(default)]
    pub gpio_matrix_id: Option<u16>,
    /// Direct-route pin number if this signal has an IO MUX fast path.
    #[serde(default)]
    pub iomux_pin: Option<u8>,
    /// Function slot (0..3) that selects the direct route on `iomux_pin`.
    #[serde(default)]
    pub iomux_fn: Option<u8>,
}

/// Per-GPIO IO MUX function selector row.
///
/// ESP32-C3 exposes exactly four function slots (F0..F3). Other chips may
/// expose more; extending this struct is the expected way to handle that.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspIoMuxPin {
    /// GPIO number (0..21 on C3).
    pub gpio: u8,
    /// Silk-screen / schematic name from the TRM (e.g. `"MTMS"`).
    pub pad_name: String,
    /// Function 0 selection (IO MUX default, often a chip-specific signal).
    #[serde(default)]
    pub fn0: Option<String>,
    /// Function 1 selection (typically `GPIO<n>`).
    #[serde(default)]
    pub fn1: Option<String>,
    /// Function 2 selection (often an `FSPI*` signal on C3).
    #[serde(default)]
    pub fn2: Option<String>,
    /// Function 3 selection.
    #[serde(default)]
    pub fn3: Option<String>,
    /// Function 4 selection (present on S2/S3 with 5+ IO MUX slots).
    #[serde(default)]
    pub fn4: Option<String>,
    /// Function 5 selection (present on classic ESP32 with 6 IO MUX slots).
    #[serde(default)]
    pub fn5: Option<String>,
    /// RTC subsystem function name (e.g. `"RTC_GPIO0"`).
    #[serde(default)]
    pub rtc_fn: Option<String>,
    /// Analog function (e.g. `"ADC1_CH0"`).
    #[serde(default)]
    pub analog: Option<String>,
    /// Strapping pin flag — these pins sample boot options at reset.
    #[serde(default)]
    pub strap: bool,
    /// Reserved for in-package SPI flash on modules that include it
    /// (ESP32-C3-MINI-1). Such pins must not be reassigned.
    #[serde(default)]
    pub flash_reserved: bool,
}

/// GPIO matrix signal table entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspGpioMatrixSignal {
    /// Signal index as used by the GPIO matrix routing registers.
    pub id: u16,
    /// Human-readable signal name from the TRM.
    pub name: String,
    /// Direction (input from GPIO, output to GPIO, or both).
    pub direction: EspDir,
}

/// Board description: what chip, which pins are wired where, what the
/// console peripheral is. This is the schema for `db/boards/<board>.yaml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspBoard {
    /// Board's human-friendly name.
    pub name: String,
    /// Chip spec this board targets (lookup into `db/chips/<name>.yaml`).
    pub chip: String,
    /// Optional module identifier (e.g. `"ESP32-C3-MINI-1"`).
    #[serde(default)]
    pub module: Option<String>,
    /// On-module flash capacity in megabytes.
    pub flash_mb: u32,
    /// On-module PSRAM capacity in megabytes, if any.
    #[serde(default)]
    pub psram_mb: Option<u32>,
    /// Pin assignments consumed by the BSP generator.
    pub pins: Vec<EspPinAssignment>,
    /// Free-form feature map (e.g. `led: gpio8`).
    #[serde(default)]
    pub features: IndexMap<String, String>,
    /// Console peripheral and baud rate for `println!`/panic output.
    #[serde(default)]
    pub console: Option<EspConsoleConfig>,
    /// Optional per-I2C-instance timing overrides. Keyed by peripheral
    /// instance name (e.g. `"i2c0"`). Missing entries default to 100 kHz.
    #[serde(default)]
    pub i2c_configs: IndexMap<String, EspI2cConfig>,
    /// Optional per-SPI-instance configuration. Keyed by peripheral
    /// instance name (e.g. `"spi2"`).
    #[serde(default)]
    pub spi_configs: IndexMap<String, EspSpiConfig>,
    /// Optional per-LEDC channel configuration. Keyed by channel label.
    #[serde(default)]
    pub ledc_configs: IndexMap<String, EspLedcConfig>,
}

/// A single board-level pin assignment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspPinAssignment {
    /// GPIO number being assigned.
    pub gpio: u8,
    /// Signal name from the board wiring (e.g. `"UART0_TX"`, `"LED"`).
    pub signal: String,
    /// Optional label used for generated pin constants.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional peripheral instance that owns this signal.
    #[serde(default)]
    pub peripheral: Option<String>,
    /// Pin direction.
    pub direction: EspDir,
    /// Optional internal pull configuration: `"up"`, `"down"`, or `"none"`.
    #[serde(default)]
    pub pull: Option<String>,
    /// Optional drive strength override (0..3).
    #[serde(default)]
    pub drive: Option<u8>,
}

/// Console peripheral selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspConsoleConfig {
    /// Peripheral instance serving as the console (`"uart0"`, `"usb_sj"`).
    pub peripheral: String,
    /// Baud rate (only meaningful for UART consoles).
    pub baud: u32,
}

/// Board-level I2C timing config.
///
/// The generator renders `peripherals::init_i2c<N>()` with the SCL period
/// derived from this struct. Falls back to 100 kHz if a board doesn't
/// specify an I2C peripheral here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspI2cConfig {
    /// Target SCL frequency in Hz (typical: 100_000 or 400_000).
    pub scl_hz: u32,
}

impl Default for EspI2cConfig {
    fn default() -> Self {
        Self { scl_hz: 100_000 }
    }
}

/// Board-level SPI master config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspSpiConfig {
    /// Target SPI clock frequency in Hz.
    #[serde(default = "default_spi_clk_hz")]
    pub clk_hz: u32,
    /// SPI mode (0-3, controls CPOL/CPHA).
    #[serde(default)]
    pub mode: u8,
}

fn default_spi_clk_hz() -> u32 {
    1_000_000
}

impl Default for EspSpiConfig {
    fn default() -> Self {
        Self {
            clk_hz: 1_000_000,
            mode: 0,
        }
    }
}

/// Board-level LEDC (PWM) channel config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EspLedcConfig {
    /// PWM frequency in Hz.
    #[serde(default = "default_ledc_freq_hz")]
    pub freq_hz: u32,
    /// Duty resolution in bits (1-14).
    #[serde(default = "default_ledc_duty_bits")]
    pub duty_bits: u8,
}

fn default_ledc_freq_hz() -> u32 {
    5_000
}

fn default_ledc_duty_bits() -> u8 {
    13
}

impl Default for EspLedcConfig {
    fn default() -> Self {
        Self {
            freq_hz: 5_000,
            duty_bits: 13,
        }
    }
}

/// Resolved per-build clock configuration.
///
/// Generated by [`super::merge`] from the chip's [`EspClockTree`] defaults
/// and any `--cpu-hz`/`--baud` overrides from the CLI.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EspClocksConfig {
    /// Selected CPU frequency in Hz.
    pub cpu_hz: u32,
    /// Selected APB frequency in Hz (fixed at 80 MHz on C3).
    pub apb_hz: u32,
    /// Crystal frequency in Hz.
    pub xtal_hz: u32,
    /// PLL output frequency in Hz.
    pub pll_hz: u32,
    /// Optional peripheral kernel clock source overrides.
    #[serde(default)]
    pub kernels: IndexMap<String, String>,
}
