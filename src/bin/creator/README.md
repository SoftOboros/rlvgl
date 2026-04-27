<!--
src/bin/creator/README.md - Guide to the rlvgl-creator binary workflows.
-->
<p align="center">
  <img src="../../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator

A combined UI and command-line tool for normalizing assets and generating dual-mode assets crates for rlvgl projects. Running without arguments launches the desktop UI; providing arguments executes the CLI. This guide covers the end-to-end workflow from initialization to consumption.

## Workflow

1. **Initialize folders and manifest**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- init
   ```
   Creates `icons/`, `fonts/`, `media/`, and a `manifest.yml` in the working directory.

2. **Scan for new or changed assets**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- scan .
   ```
   Updates hashes in the manifest for assets under the allowed roots.

3. **Convert assets into raw sequences and font packs**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- convert
   ```
   Raster images become raw RGBA sequences, and fonts are packed into bitmap binaries and metrics. Conversions run in parallel
   with stable ordering. Use `--force` to rebuild all assets regardless of cache.

   To render vector assets, the `svg` command converts an SVG into one or more raw images at chosen DPI values:
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- svg logo.svg out/ --dpi 96 --dpi 192
   ```
   Supply `--threshold <VAL>` to apply a monochrome cutoff suitable for e-ink displays.

4. **Synchronize feature flags, constants, and index**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- sync
   ```
   Regenerates manifest-driven code without touching asset bytes.

5. **Scaffold a consumer assets crate**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- scaffold assets-crate
   ```
   Generates a crate with `embed` and `vendor` features that exposes your processed assets.

6. **Vendor assets for build output**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- vendor
   ```
   Copies processed assets to `$OUT_DIR` and emits an `rlvgl_assets.rs` module for inclusion.

The resulting crate can be built with `--features embed` to include raw bytes or `--features vendor` to copy files at build time while importing the generated module.

## BSP Generation

`rlvgl-creator` can generate board support crates from two different vendor
inputs:

### STM32 (.ioc input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-ioc \
    path/to/board.ioc --out gen/ --emit-hal --emit-pac
```

Runs the STM32 CubeMX `.ioc` → generic IR → MiniJinja → Rust pipeline
using the embedded `rlvgl-chips-stm` alternate-function database. Supports
HAL, PAC, and custom MiniJinja templates, single-file and per-peripheral
layouts, label-based identifier generation, and STM32H7 dual-core splits.

### Espressif (YAML chipdb input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-yaml \
    --vendor esp \
    --board esp32c3_devkitm_1 \
    --out gen/ \
    --emit-pac
```

Consumes the YAML chip and board specs embedded in `rlvgl-chips-esp` and
emits a PAC-style BSP targeting the `esp32c3` PAC crate. Each board spec
identifies its chip by name; `--chip` / `--chip-yaml` / `--board-yaml` are
available to override the chipdb lookups for out-of-tree data.

The generated BSP consists of six files under `gen/<board_stem>/`:

- `mod.rs` — `pub mod board; pub mod clocks; pub mod io_mux; pub mod pac;
  pub mod peripherals; pub use pac::init;`
- `pac.rs` — `pub fn init()` entry sequencing clocks → IO MUX → peripherals
- `clocks.rs` — `SYSTEM` peripheral clock enables and resets
- `io_mux.rs` — per-pin IO MUX / GPIO matrix routing
- `peripherals.rs` — per-instance init (UART0 real when it is the console,
  others stubbed with TODOs pointing at the PAC register path)
- `board.rs` — board constants and labeled pin consts (`LED`, `BOOT_BTN`, …)

Overrides:

| Flag            | Effect                                              |
|-----------------|-----------------------------------------------------|
| `--vendor`      | Vendor backend: `esp`, `nrf`, `nxp`, `rp`, `renesas` (aliases: `espressif`, `nordic`, `imxrt`, `rp2040`, `ra`) |
| `--cpu-hz`      | Override the resolved CPU frequency in hertz        |
| `--baud`        | Override the console baud rate                      |
| `--chip`        | Use a different chip spec file stem (default: esp32c3) |
| `--chip-yaml`   | Load chip spec from a file (bypasses chipdb)        |
| `--board-yaml`  | Load board spec from a file (bypasses chipdb)       |

Add or edit chips and boards by dropping new YAML files into
`chipdb/rlvgl-chips-esp/db/chips/` and `chipdb/rlvgl-chips-esp/db/boards/`
and rebuilding.

### Nordic nRF (YAML chipdb input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-yaml \
    --vendor nrf \
    --board nrf52840_dk \
    --out gen/ \
    --emit-pac
```

Consumes the YAML chip and board specs embedded in `rlvgl-chips-nrf` and
emits a PAC-style BSP targeting the `nrf52840_pac` PAC crate. Uses
PSEL-based pin routing for peripheral-to-pin assignment.

The generated BSP consists of six files under `gen/<board_stem>/`:

- `mod.rs` — module index
- `pac.rs` — `pub fn init()` entry sequencing clocks, GPIO, peripherals
- `clocks.rs` — clock configuration
- `gpio.rs` — PSEL-based pin routing for each peripheral
- `peripherals.rs` — per-instance peripheral init
- `board.rs` — board constants and labeled pin consts

### NXP i.MX RT (YAML chipdb input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-yaml \
    --vendor nxp \
    --board mimxrt1060_evkb \
    --out gen/ \
    --emit-pac
```

Consumes the YAML chip and board specs embedded in `rlvgl-chips-nxp` and
emits a BSP targeting the `imxrt_ral` register access layer. Uses IOMUX
ALT0-7 pad routing with daisy chain SELECT_INPUT muxing and CCM CCGR
clock gating.

The generated BSP consists of six files under `gen/<board_stem>/`:

- `mod.rs` — module index
- `pac.rs` — `pub fn init()` entry sequencing clocks, IOMUX, peripherals
- `clocks.rs` — CCM CCGR clock-gate enables
- `iomux.rs` — per-pad ALT mux + daisy chain SELECT_INPUT routing
- `peripherals.rs` — per-instance peripheral init
- `board.rs` — board constants and labeled pin consts

### RP2040 (YAML chipdb input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-yaml \
    --vendor rp \
    --board pico \
    --out gen/ \
    --emit-pac
```

Consumes the YAML chip and board specs embedded in `rlvgl-chips-rp2040`
and emits a BSP targeting the `rp2040_pac` PAC crate. Uses FUNCSEL
per-GPIO pin routing and RESETS register for peripheral reset release (no
clock gating).

The generated BSP consists of six files under `gen/<board_stem>/`:

- `mod.rs` — module index
- `pac.rs` — `pub fn init()` entry sequencing clocks, GPIO, peripherals
- `clocks.rs` — clock and RESETS configuration
- `gpio.rs` — per-GPIO FUNCSEL routing
- `peripherals.rs` — per-instance peripheral init
- `board.rs` — board constants and labeled pin consts

### Renesas RA (YAML chipdb input)

```sh
cargo run --bin rlgvl-creator --features creator -- bsp from-yaml \
    --vendor renesas \
    --board ek_ra6m5 \
    --out gen/ \
    --emit-pac
```

Consumes the YAML chip and board specs embedded in `rlvgl-chips-renesas`
and emits a BSP using raw register addresses (no PAC crate). Uses PFS
PSEL + PMR pin routing and MSTP clock gating.

The generated BSP consists of six files under `gen/<board_stem>/`:

- `mod.rs` — module index
- `pac.rs` — `pub fn init()` entry sequencing clocks, PFS, peripherals
- `clocks.rs` — MSTP clock-gate enables
- `pfs.rs` — PFS PSEL + PMR pin function select and routing
- `peripherals.rs` — per-instance peripheral init
- `board.rs` — board constants and labeled pin consts

## Desktop UI and Emulator

Launch the desktop UI explicitly:

```sh
cargo run --bin rlgvl-creator --features creator,creator_ui -- ui
```

Run the simulator from the same binary:

```sh
cargo run --bin rlgvl-creator --features creator,creator_ui -- sim --screen=800x480 --png --qrcode
```

## Developer Notes

For details on customizing scaffold templates and extending the conversion pipeline, see
[`docs/creator/TEMPLATES.md`](../../../docs/creator/TEMPLATES.md).
