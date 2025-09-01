<!--
README.md - Top-level overview and navigation for rlvgl.
-->
<p align="center">
  <img src="./rlvgl-logo.png" alt="rlvgl" />
</p>

<span style="font-size:26px"><b>rlvgl</b></span> is a modular, idiomatic Rust reimplementation of LVGL (Light and Versatile Graphics Library).

rlvgl preserves the widget-based UI paradigm of LVGL while eliminating unsafe C-style memory management and global state. This library is structured to support no_std environments, embedded targets (e.g., STM32H7), and simulator backends for rapid prototyping.

The C version of LVGL is included as a git submodule for reference and test vector extraction, but not linked or compiled into this library.

## Goals
Package: `rlvgl`
- Preserve LVGL architecture and layout system
- Replace C memory handling with idiomatic Rust ownership
- Support embedded display flush/input via embedded-hal
- Enable widget hierarchy, styles, and events using Rust traits
- Use existing Rust crates where possible (e.g., embedded-graphics, heapless, tinybmp)

## Features
- no_std + allocator support
- Component-based module layout (core, widgets, platform)
- Simulatable via std-enabled feature flag
- Pluggable display and input backends
- Optional Lottie support via the `rlottie` crate for dynamic playback.
  Embedded targets should pre-render animations to APNG for minimal size.

## Project Structure
- [core](./core/README.md) – Widget base trait, layout, event dispatch
- [widgets](./widgets/README.md) – Rust-native reimplementations of LVGL widgets
- [platform](./platform/README.md) – Display/input traits and HAL adapters
- [ui](./ui/README.md) – Higher-level UI components
- [examples](./examples/README.md) – Sample applications and board demos
- [docs](./docs/README.md) – Project documentation and task lists
- [lvgl](./lvgl/README.md) – C submodule (reference only)
- [chips/stm/bsps](./chips/stm/bsps/README.md) 🆕 – Generated STM32 BSP stubs

## Vendor chip databases

Vendor-specific board definitions live in the [`chipdb/`](./chipdb/README.md) crates. The
`tools/gen_pins.py` helper aggregates raw vendor inputs into JSON
blobs, while `tools/build_vendor.sh` orchestrates generation and stamps
license files. When building a vendor crate, set `RLVGL_CHIP_SRC` to the
directory containing these JSON files so the build script can embed them
via `include_bytes!`.

## STM32CubeMX BSP generation 🆕

`rlvgl-creator` 🆕 converts STM32 CubeMX `.ioc` projects into board support
stubs. Generated modules ship in
[`rlvgl-bsps-stm` 🆕](./chips/stm/bsps/README.md). The older `board`
overlay support remains but is deprecated.

## BSP Generator (`rlvgl-creator` 🆕)

`rlvgl-creator` 🆕 offers a two-stage pipeline for board support packages:

1. **Import** vendor project files (e.g., STM32CubeMX `.ioc`, NXP `.mex`,
   RP2040 YAML). Each adapter mines the vendor data and emits a small, vendor-neutral
   YAML **IR** describing clocks, pins, DMA and peripherals.
2. **Generate** Rust initialization code by rendering MiniJinja templates
   against the IR. Users may choose from built-in template packs or provide
   their own.

The STM32CubeMX adapter also parses PLL multipliers and peripheral kernel
clock selections so that clock setup can be generated alongside pin
configuration.

No per-chip tables are maintained. Class-level rules are reused across
instances and vendors. Alternate functions are resolved from the canonical
vendor databases bundled in `chipdb/` crates.
Reserved SWD pins (`PA13`, `PA14`) are rejected unless explicitly allowed.

Typical flow:

```bash
rlvgl-creator platform import --vendor st --input board.ioc --out board.yaml
rlvgl-creator platform gen --spec board.yaml --templates templates/stm32h7 \
  --out src/generated.rs
```

Alternate-function numbers are resolved automatically from the canonical STM32
database embedded in `rlvgl-chips-stm`; no external AF JSON is required.

To package vendor chip databases for testing or publishing, run:

```bash
tools/build_vendor.sh
RLVGL_CHIP_SRC=chipdb/rlvgl-chips-stm/generated cargo build -p rlvgl-chips-stm
```

For a full asset workflow overview see the [rlvgl-creator 🆕 README](./README-CREATOR.md).
Command details live in [docs/CREATOR-CLI.md](./docs/CREATOR-CLI.md).

### IR schema

The import step emits a concise YAML specification describing the board:

```yaml
mcu: STM32H747XIHx
package: LQFP176
power: { supply: smps, vos: scale1 }
clocks:
  sources: { hse_hz: 25000000 }
  pll:
    pll1: { m: 5, n: 400, p: 2, q: 4, r: 2 }
  kernels: { usart1: pclk2 }
pinctrl:
  - group: usart1-default
    signals:
      - { pin: PA9,  func: USART1_TX, af: 7, pull: none, speed: veryhigh }
      - { pin: PA10, func: USART1_RX, af: 7, pull: up,   speed: veryhigh }
peripherals:
  usart1:
    class: serial
    params: { baud: 115200, parity: none, stop_bits: 1 }
    pinctrl: [ usart1-default ]
reserved_pins: [ PA13, PA14 ]
```

Field summary:

- `mcu`, `package` – identifiers from the vendor project.
- `power` – supply configuration; values map directly to HAL calls.
- `clocks` – input frequencies (`sources`), PLL multipliers (`pll`) and
  per‑peripheral kernel selections (`kernels`).
- `pinctrl` – groups of pins with their functions, alternate functions,
  pulls and speeds.
- `peripherals` – map of peripheral instances keyed by name (`usart1`),
  each with a `class` (e.g. `serial`) and optional `params`.
- `dma`, `interrupts` – optional arrays describing DMA requests and IRQ
  priorities.
- `reserved_pins` – pins that must not be reconfigured (e.g. SWD).

### Template helpers

MiniJinja templates can use the following filters:

- `pin_var` – convert a pin like `PA9` into the variable name `pa9`.
- `periph_num` – extract trailing digits from a peripheral name
  (`usart12` → `12`).
- `af_alt` – render an alternate-function number for
  `into_alternate::<AF>()` (`7` → `<7>`).

Users may supply custom templates by pointing `--templates` at any
directory; the filters above are always available.

See `docs/TODO-CREATOR-BSP.md` for remaining work.

## Status

As-built. See [docs](./docs/README.md) for component-by-component progress and outstanding tasks.

As of 0.1.0 many features are implemented and an 87% unit test coverage
is achived, but functional testing has and bare metal testing have not
occured.

## Quick Example

```rust
use rlvgl_core::widget::Rect;
use rlvgl_widgets::label::Label;

fn main() {
    let mut label = Label::new(
        "hello",
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 20,
        },
    );
    label.style.bg_color = rlvgl_core::widget::Color(0, 0, 255, 255);
    // Rendering would use a DisplayDriver implementation.
}
```

## Testing

Run host-based tests with the default toolchain:

```bash
cargo test --workspace
```

Cross-target tests (e.g., `thumbv7em-none-eabihf`) require a linker. Cargo
defaults to `arm-none-eabi-gcc`, but you can avoid installing GCC by adding
the `rust-lld` component and configuring:

```bash
rustup component add rust-lld
```

```toml
[target.thumbv7em-none-eabihf]
linker = "rust-lld"
```

See [docs/CROSS-TESTING.md](docs/CROSS-TESTING.md) for troubleshooting tips.

## Coverage

LLVM coverage instrumentation is configured via `.cargo/config.toml` and the
`coverage` target in the `Makefile`. Run `make coverage` to execute the tests
with instrumentation and generate an HTML report under `./coverage/`.

## [rlvgl crate](https://crates.io/crates/rlvgl)
- The link above is for the top crate which bundles the others and include the simulator.
- [rlvgl-core crate](https://crates.io/crates/rlvgl-core)
- [rlvgl-widgets crate](https://crates.io/crates/rlvgl-widgets)
- [rlvgl-platform crate](https://crates.io/crates/rlvgl-platform)

Run the following Cargo command in your project directory:
```bash
cargo add rlvgl
```
Or add the following line to your Cargo.toml:
```toml
rlvgl = "0.1.5"
```

## Community
- [Code of Conduct](./CODE_OF_CONDUCT.md)
- [Contributor notes](./AGENTS.md)

## Dockerhub
The build image used by the Github worflow for this repo is publiclly available on [Dockerhub](https://hub.docker.com/r/iraa/rlvgl).
```bash
docker pull iraa/rlvgl:latest
```

Consult the [Dockerfile](https://github.com/SoftOboros/rlvgl/blob/main/Dockerfile) for details on the build environment.

Other useful helper scripts may be found in [`/scripts`](https://github.com/SoftOboros/rlvgl/blob/main/scripts).

## License
rlvgl is licensed under the MIT license. See [LICENSE](./LICENSE) for more details.
Third-party license notices are summarized in [NOTICES.md](./NOTICES.md).
