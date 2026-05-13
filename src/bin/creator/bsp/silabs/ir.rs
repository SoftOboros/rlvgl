//! Intermediate representation for Silicon Labs BSP generation.
//!
//! Silicon Labs EFM32 Series 1 parts (EFM32GG11, EFM32xG12, …) use a
//! peripheral routing model that does not map onto either the STM32
//! alternate-function table or the Espressif IO-MUX/GPIO-matrix shape:
//!
//! - Per-peripheral `ROUTELOC0` / `ROUTELOC1` register selects a fixed
//!   pin location for each role (TX, RX, CLK, CS, CTS, RTS, SCL, SDA,
//!   …); a parallel `ROUTEPEN` register gates that role onto the pad.
//! - Pin addressing uses `(port, pin)` tuples — `port` is `A..I` and
//!   `pin` is `0..15`. GPIO ports are independent register blocks
//!   (`GPIO.PA_*`, `GPIO.PB_*`, …) on the chip.
//! - Clock gating lives in three CMU register families:
//!   `cmu.hfperclken0` / `cmu.hfperclken1` for HF peripheral bus,
//!   `cmu.hfbusclken0` for the HF system bus, and the LF clken bank
//!   (`cmu.lfaclken0`, `cmu.lfbclken0`, `cmu.lfeclken0`) for low-energy
//!   peripherals. There is no per-peripheral reset register — RMU is
//!   soft-reset only.
//!
//! Each of those concerns lives in its own struct in this module. YAML
//! chip and board specs deserialise directly into [`SilabsChip`] and
//! [`SilabsBoard`]; the pipeline then resolves them into a concrete
//! [`SilabsIr`] for rendering.
//!
//! See `docs/concepts/CHIPS-SILABS-00-CONCEPTS.md` §10 for the frozen
//! decisions this IR implements.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Direction of a peripheral signal on a GPIO pin.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SilabsDir {
    /// Output from the chip.
    Out,
    /// Input to the chip.
    In,
    /// Bidirectional signal (e.g. I2C SDA, I2C SCL).
    Inout,
}

/// Routing kind frozen by CHIPS-SILABS-00 §10.2.
///
/// EFM32 Series 0 + Series 1 parts (Tiny / Zero / Happy / Wonder / Giant
/// / Giant 11) use the per-peripheral `ROUTELOC` register family. EFM32
/// Series 2 / EFR32 parts (xG21, xG22, xG23, …) replaced it with a
/// global GPIO matrix similar to the ESP shape. The chip YAML declares
/// which family it belongs to via this enum so templates can branch
/// without sniffing the peripheral signal shape.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SilabsRoutingKind {
    /// Series 1 ROUTELOC/ROUTEPEN family.
    Routeloc,
    /// Series 2 GPIO matrix family (reserved for future chips).
    GpioMatrix,
}

/// Top-level resolved IR fed into the render pipeline.
///
/// [`SilabsIr`] is the product of merging a chip spec, a board spec, and
/// any per-invocation overrides (CPU frequency, console baud, …).
/// Consumers should not construct it manually — go through
/// [`super::merge`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsIr {
    /// Schema version of the spec format. Currently `"0.1"`.
    #[serde(default = "default_version")]
    pub version: String,
    /// Chip inventory (from `db/chips/<chip>.yaml`).
    pub chip: SilabsChip,
    /// Board wiring (from `db/boards/<board>.yaml`).
    pub board: SilabsBoard,
    /// Resolved clock configuration for this specific build.
    pub clocks: SilabsClocksConfig,
    /// Pin assignments copied from [`SilabsBoard::pins`] for template
    /// convenience.
    pub pins: Vec<SilabsPinAssignment>,
    /// Free-form metadata propagated into generated file headers.
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

fn default_version() -> String {
    "0.1".to_string()
}

/// Full chip inventory sourced from the EFM32GG11 Family Reference
/// Manual.
///
/// This is the schema for `db/chips/<chip>.yaml`. Every field is
/// populated from the RM so the render pipeline does not need to
/// hard-code chip-specific constants.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsChip {
    /// Marketing name, e.g. `"EFM32GG11"`.
    pub name: String,
    /// CPU architecture string, e.g. `"cortex-m4f"`.
    pub arch: String,
    /// Package identifier, e.g. `"BGA192"`.
    pub package: String,
    /// PAC crate name used in generated code (`use <pac_crate> as pac;`).
    pub pac_crate: String,
    /// Optional SKU sub-module to flatten into the `pac` namespace.
    ///
    /// Some Silicon Labs PAC crates (notably `efm32gg11b-pac` 0.1.4)
    /// gate the per-SKU `Peripherals` type behind a SKU-named sub-module
    /// (`efm32gg11b_pac::efm32gg11b820::Peripherals`) rather than
    /// re-exporting it at the crate root. When this field is set, the
    /// `pac.rs` template emits an additional
    /// `pub use <pac_crate>::<pac_sku_module>::*;` line so the BSP's
    /// `pac::Peripherals::steal()` call site continues to resolve
    /// without per-call-site changes.
    #[serde(default)]
    pub pac_sku_module: Option<String>,
    /// Number of user-accessible GPIO pads on this chip.
    pub gpio_count: u16,
    /// Pin-routing family per §10.2.
    pub routing_kind: SilabsRoutingKind,
    /// Memory map (flash, SRAM, info-page regions).
    pub memory: Vec<SilabsMemoryRegion>,
    /// Clock tree and per-peripheral clock-gate table.
    pub clock_tree: SilabsClockTree,
    /// Peripheral instances keyed by instance name (`usart4`, `i2c2`,
    /// `gpio`, …).
    pub peripherals: IndexMap<String, SilabsPeripheral>,
    /// GPIO port + pin inventory.
    pub gpio: SilabsGpioInventory,
    /// Linker layout hint. Optional — consumed by `CHIPS-SILABS-05`
    /// when linker emission lands; ignored by `-02`.
    #[serde(default)]
    pub linker: Option<SilabsLinker>,
}

/// Linker-region alias map for the chip.
///
/// Mirrors the ESP shape. Reserved for future memory.x emission per
/// CHIPS-SILABS-05.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsLinker {
    /// Memory region that holds `.init`/`.text`/`.rodata`.
    pub region_text: String,
    /// Memory region that holds `.data`/`.bss`/`.heap`/`.stack`.
    pub region_data: String,
}

/// Contiguous memory region described by the chip memory map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsMemoryRegion {
    /// Region name (e.g. `"flash"`, `"sram"`, `"userdata"`).
    pub name: String,
    /// Base address in the chip's address space.
    pub base: u32,
    /// Region size in bytes.
    pub size: u32,
    /// Access bitfield as a short string: `"r"`, `"rw"`, `"rx"`, or
    /// `"rwx"`.
    pub access: String,
}

/// EFM32 clock tree topology and per-peripheral clock-gate table.
///
/// EFM32GG11 carries six oscillators (HFXO, HFRCO, AUXHFRCO, LFXO,
/// LFRCO, USHFRCO); the chip YAML pins the nominal frequency of each
/// for the post-init clock state. Sources can be switched at runtime by
/// reprogramming `CMU.HFCLKSEL` / `CMU.LFCLKSEL`, outside this IR's
/// remit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsClockTree {
    /// External HF crystal frequency in Hz.
    pub hfxo_hz: u32,
    /// Internal HFRCO frequency in Hz.
    pub hfrco_hz: u32,
    /// AUXHFRCO frequency in Hz.
    pub auxhfrco_hz: u32,
    /// External LF crystal frequency in Hz (typically 32_768).
    pub lfxo_hz: u32,
    /// Internal LFRCO frequency in Hz (typically 32_768).
    pub lfrco_hz: u32,
    /// USB-dedicated HF RC frequency in Hz (typically 48_000_000).
    pub ushfrco_hz: u32,
    /// HFCLK frequency in Hz after `clocks::init()` runs.
    pub hfclk_hz: u32,
    /// CPU clock frequency in Hz after `clocks::init()` runs.
    pub cpu_hz: u32,
    /// Peripheral clock-gate map keyed by peripheral instance.
    pub system_gates: IndexMap<String, SilabsSystemGate>,
}

/// Per-peripheral clock-enable entry.
///
/// `clk_en_reg` / `clk_en_field` use the dotted PAC notation matching
/// `efm32gg11b-pac`'s svd2rust accessor surface
/// (e.g. `"cmu.hfperclken0"` / `"usart4"`). EFM32GG11 has no
/// per-peripheral reset register; `rst_reg`/`rst_field` are kept as
/// `Option` for future Series 2 parts that may add one.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsSystemGate {
    /// PAC path of the clock-enable register.
    pub clk_en_reg: String,
    /// Field name within the clock-enable register.
    pub clk_en_field: String,
    /// PAC path of the reset register, if any. Always `None` on
    /// EFM32GG11 (RMU is soft-reset only).
    #[serde(default)]
    pub rst_reg: Option<String>,
    /// Field name within the reset register, if any.
    #[serde(default)]
    pub rst_field: Option<String>,
    /// Optional kernel clock mux register (reserved).
    #[serde(default)]
    pub clk_sel_reg: Option<String>,
    /// Optional kernel clock mux field (reserved).
    #[serde(default)]
    pub clk_sel_field: Option<String>,
}

/// Peripheral instance with its routing-signal list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsPeripheral {
    /// Peripheral class (`"usart"`, `"uart"`, `"leuart"`, `"i2c"`,
    /// `"timer"`, `"letimer"`, `"gpio"`, `"adc"`, `"dac"`, `"rtc"`,
    /// `"rtcc"`, `"wdog"`, `"cryotimer"`, `"cmu"`, `"emu"`, `"rmu"`,
    /// `"msc"`, `"ldma"`). Templates dispatch on this to pick the right
    /// init sequence.
    pub class: String,
    /// Instance name mirrored from the `peripherals` map key.
    pub instance: String,
    /// Base address of the peripheral register block.
    pub base: u32,
    /// Primary interrupt source number in the NVIC, if any.
    #[serde(default)]
    pub irq: Option<u16>,
    /// Optional second IRQ for peripherals with split RX/TX lines
    /// (USART) or even/odd lines (GPIO).
    #[serde(default)]
    pub irq_tx: Option<u16>,
    /// Optional odd-pin IRQ for GPIO.
    #[serde(default)]
    pub irq_odd: Option<u16>,
    /// Signal list — one entry per role this peripheral can route.
    #[serde(default)]
    pub signals: Vec<SilabsPeripheralSignal>,
}

/// A single role within a peripheral (`tx`, `rx`, `clk`, `cs`, `cts`,
/// `rts`, `scl`, `sda`, `cc0`, …) on a Series 1 chip.
///
/// `route_reg` / `route_field` and `pen_reg` / `pen_field` use dotted
/// PAC notation matching the `efm32gg11b-pac` accessor surface:
/// `route_reg = "usart4.routeloc0"`, `route_field = "txloc"`,
/// `pen_reg = "usart4.routepen"`, `pen_field = "txpen"`. The concrete
/// LOC integer (0..31) for this signal on a specific board is supplied
/// by [`SilabsPinAssignment::routeloc`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsPeripheralSignal {
    /// Role identifier, e.g. `"tx"`, `"rx"`, `"clk"`, `"cs"`, `"scl"`,
    /// `"sda"`, `"cc0"`.
    pub role: String,
    /// Direction relative to the chip.
    pub direction: SilabsDir,
    /// PAC path of the ROUTELOC register holding this role's location
    /// field.
    pub route_reg: String,
    /// Field name within the ROUTELOC register
    /// (e.g. `"txloc"`, `"rxloc"`, `"clkloc"`).
    pub route_field: String,
    /// PAC path of the ROUTEPEN register.
    pub pen_reg: String,
    /// Field name within the ROUTEPEN register
    /// (e.g. `"txpen"`, `"rxpen"`, `"clkpen"`).
    pub pen_field: String,
}

/// GPIO port + pin inventory.
///
/// EFM32GG11 BGA192 exposes all nine ports `A..I` with 16 pins each.
/// Smaller packages bond out a strict subset; chip YAML is canonical
/// for the SKU it models.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsGpioInventory {
    /// Ordered list of present ports (`["A", "B", "C", ...]`).
    pub ports: Vec<String>,
    /// Pin count per port keyed by port letter.
    pub pins_per_port: IndexMap<String, u8>,
}

/// Board description.
///
/// This is the schema for `db/boards/<board>.yaml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsBoard {
    /// Board's human-friendly name.
    pub name: String,
    /// Chip spec this board targets (lookup into
    /// `db/chips/<name>.yaml`).
    pub chip: String,
    /// On-module flash capacity in megabytes.
    pub flash_mb: u32,
    /// Pin assignments consumed by the BSP generator.
    pub pins: Vec<SilabsPinAssignment>,
    /// Free-form feature map.
    #[serde(default)]
    pub features: IndexMap<String, serde_yaml::Value>,
    /// Console peripheral and baud rate for `println!` / panic output.
    #[serde(default)]
    pub console: Option<SilabsConsoleConfig>,
    /// Optional per-I2C-instance timing overrides, keyed by peripheral
    /// instance name (e.g. `"i2c2"`).
    #[serde(default)]
    pub i2c_configs: IndexMap<String, SilabsI2cConfig>,
}

/// A single board-level pin assignment per CHIPS-SILABS-00 §10.3.
///
/// EFM32 addresses GPIOs as `(port, pin)` tuples — `port` is `A..I`
/// (carried here as a single-char string for YAML readability), `pin`
/// is `0..15`. Templates emit two `pub const`s per labelled pin:
/// `<LABEL>_PORT: char` and `<LABEL>_PIN: u8`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsPinAssignment {
    /// GPIO port letter (`"A"..="I"`).
    pub port: String,
    /// GPIO pin within the port (`0..15`).
    pub pin: u8,
    /// Signal name from the board wiring
    /// (e.g. `"USART4_TX"`, `"I2C2_SCL"`, `"GPIO"`).
    pub signal: String,
    /// Optional label used for generated pin constants.
    #[serde(default)]
    pub label: Option<String>,
    /// Optional peripheral instance that owns this signal.
    #[serde(default)]
    pub peripheral: Option<String>,
    /// Pin direction.
    pub direction: SilabsDir,
    /// Optional internal pull configuration: `"up"`, `"down"`, or
    /// `"none"`.
    #[serde(default)]
    pub pull: Option<String>,
    /// Optional initial drive state for output pins: `"high"` or
    /// `"low"`.
    #[serde(default)]
    pub initial: Option<String>,
    /// ROUTELOC integer (0..31) selecting this signal's location on
    /// its peripheral, per the chip's RM ROUTELOC table. `None` for
    /// plain GPIO assignments.
    #[serde(default)]
    pub routeloc: Option<u8>,
    /// Optional alternate-function name for a pin that doubles as both
    /// a GPIO and a peripheral signal (e.g. an LED on PH10 that is
    /// also `WTIMER1_CC2`). Informational only — the generator emits
    /// the GPIO routing; the consumer can opt into the alt route at
    /// runtime.
    #[serde(default)]
    pub alt_function: Option<String>,
    /// Optional alternate-function ROUTELOC integer paired with
    /// `alt_function`.
    #[serde(default)]
    pub alt_routeloc: Option<u8>,
}

/// Console peripheral selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsConsoleConfig {
    /// Peripheral instance serving as the console
    /// (`"usart4"`, `"leuart0"`, …).
    pub peripheral: String,
    /// Baud rate.
    pub baud: u32,
}

/// Board-level I2C timing config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SilabsI2cConfig {
    /// Target SCL frequency in Hz (typical: 100_000 or 400_000).
    pub scl_hz: u32,
}

impl Default for SilabsI2cConfig {
    fn default() -> Self {
        Self { scl_hz: 100_000 }
    }
}

/// Resolved per-build clock configuration.
///
/// Generated by [`super::merge`] from the chip's [`SilabsClockTree`]
/// defaults and any `--cpu-hz` / `--baud` overrides from the CLI.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SilabsClocksConfig {
    /// Selected CPU frequency in Hz.
    pub cpu_hz: u32,
    /// Selected HFCLK frequency in Hz.
    pub hfclk_hz: u32,
    /// HFXO frequency in Hz.
    pub hfxo_hz: u32,
    /// LFXO frequency in Hz.
    pub lfxo_hz: u32,
    /// HFRCO frequency in Hz.
    pub hfrco_hz: u32,
}
