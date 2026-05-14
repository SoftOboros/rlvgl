# Agent Runbook

This file is the source of truth for Codex/Claude-style agents working on the
STM32H747I-DISCO example. README build snippets are human-facing and may lag
behind the currently flashable artifact set.

## Board Target

- Board: `STM32H747I-DISCO`
- CM7 binary: `rlvgl-stm32h747i-disco`
- Probe-rs chip id: `STM32H747XIHx`

## Build Profiles

Use these commands unless the task explicitly calls for a different feature mix.

### Current profiling-oriented dev build

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio
```

- This is the current rust-only profiling build.
- It links and boots successfully as a dev build.
- `cpu_stats` is for DWT/D3 telemetry and is not the known flashable release profile.
- `audio` enables WM8994 codec init over I2C4 + SAI1 I2S TX + SAI4 PDM mic.
- `sd_storage` enables SDMMC block device; file browser listing is still a stub.

### FreeRTOS build

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,freertos,adapted_cmd,dma2d,splash,desktop
```

- This is the FreeRTOS preemptive task build (present/render/touch/playit).
- `freertos` links libfreertos.a and enables the FreeRTOS entry path.
- `adapted_cmd` selects DSI adapted command mode (portrait, pulsed scan).
- Uses single-buffer FRONT rendering with 32 ms holdoff.
- Joystick (PK2-PK6) + button (PC13) for navigation.
- Touch detection works; touch dispatch to widget tree disabled pending
  ActionHotspot bounds fix.
- 64 KB Rust heap required (settings wing draws 5 RLE icons).

### Compile-safety check for the leaner profiling feature set

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo check \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-disco \
  --features cm7,dma2d,cpu_stats,pac_sdram_init,sdram_ramtest,backlight_pwm
```

### Cached flashable release profile

The most recent cached successful release fingerprint under
`target/thumbv7em-none-eabihf/release/.fingerprint/.../bin-rlvgl-stm32h747i-disco.json`
records:

```text
cm7,dma2d,splash,desktop,audio
```

Cached artifact expectations from `target/`:

- `target/thumbv7em-none-eabihf/release/rlvgl-stm32h747i-disco`: about `321K`
- `target/thumbv7em-none-eabihf/release/rlvgl-stm32h747i-disco.bin`: about `152K`
- `target/thumbv7em-none-eabihf/release/rlvgl-stm32h747i-disco.hex`: about `448K`

Do not assume a new `cpu_stats` build will still fit flash just because the
cached `.hex` does.

## Flashing And Debug

All flash/debug workflows use `make` targets. Run `make help` for the full list.

### Build + flash (preferred)

```bash
make flash-disco          # build debug + flash ELF via probe-rs
make flash-disco-hex      # flash from .hex
make flash-disco-bin      # flash from .bin (with --base-address 0x08000000)
```

### Build + flash + GDB server

```bash
make probe-rs-gdb
```

### VS Code one-click

Use the **"CM7 (probe-rs)"** launch config — it builds, flashes, and halts at
reset.  To debug after loading via any other method (CLI, CubeProgrammer, etc.),
use **"CM7 attach (probe-rs)"** which provides symbols without reflashing.

### Direct probe-rs flash

```bash
probe-rs download --chip STM32H747XIHx \
  target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco
```

## Serial Helper

Use the helper script instead of retyping the `miniterm` invocation:

```bash
examples/stm32h747i-disco/DiscoBiscuit/tools/serial.sh [PORT] [BAUD]
```

Defaults:

- port: `/dev/ttyACM0`
- baud: `115200`

The runtime USART1 path is interrupt-driven and FIFO-backed. Treat serial as a
control plane plus sparse summaries, not as a streaming profiler.

## Runtime Command Protocol (rlvgl-playit)

The serial test driver is implemented by the `rlvgl-playit` crate (`playit/`).
Commands are single lines terminated by `\n` or `\r\n`.  See `playit/README.md`
for the full wire protocol reference.

Core commands:

- `?` — tick count, present count, serial queue/drop state
- `T<x>,<y>` — inject `PressRelease` at landscape `(x, y)`
- `PD<x>,<y>` / `PM<x>,<y>` / `PU<x>,<y>` — raw pointer down/move/up
- `MT<n>:<id>,<s>,<x>,<y>;...` — multi-touch frame (s=D/U/C)
- `KD:<key>` / `KU:<key>` — key down/up
- `T@<tag>:<x>,<y>` — inject tap to tagged widget
- `QB:<tag>` / `QE:<tag>` / `QC:<tag>` — query bounds/exists/children
- `D<x>,<y>,<w>,<h>[,<frames>]` — framebuffer pixel dump
- `RS` / `RE` / `RD` — start / stop+dump / dump event recorder
- `C` — toggles the star crawl (app extension)

## Pre-Publish Validation

Run the `/pre-publish` skill or execute these phases manually before committing
changes that touch publishable crates.  All phases must pass.

```bash
# Phase 0: format
cargo fmt --all -- --check

# Phase 1: clippy (all workspace crates, warnings = errors)
RUSTFLAGS="" cargo clippy --workspace -- -D warnings

# Phase 2: tests (all workspace crates including doc tests)
RUSTFLAGS="" cargo test --workspace

# Phase 2.5: hardware-abstraction discipline (STM32H747I-DISCO)
# Locks in: typed framebuffer ownership, DMA2D InFlight tokens, typed
# DSI/LTDC/I2C/USART/GPIO/TIM register blocks, IsrChannel<T,N> + IsrFlag
# + IsrCounter ISR primitives, address-domain newtypes (MmioAddr<T>,
# PhysAddr, DmaAddr). Strict mode asserts BASELINE is empty — any new
# raw cast / static mut / compiler_fence outside the documented exempts
# fails CI. See the "Register-Mashing Discipline" section above.
RLVGL_LINT_STRICT=1 RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile
# The disco-sim host integration test for the typed types runs in
# Phase 4.5 via `cargo test -p rlvgl-example-disco-sim` — do not
# duplicate here.

# Phase 3: playit crate (standalone — no_std cross-compile + package)
RUSTFLAGS="" cargo test -p rlvgl-playit
RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-playit
cd playit && cargo package --list --allow-dirty && cd ..

# Phase 4: simulator + creator tests
RUSTFLAGS="" cargo build -p rlvgl-example-sim
RUSTFLAGS="" cargo test -p rlvgl-example-sim
RUSTFLAGS="" cargo test --tests --features "creator" -p rlvgl

# Phase 4.5: disco demo + simulator automation tests
RUSTFLAGS="" cargo test -p rlvgl-app-disco-demo
RUSTFLAGS="" cargo test -p rlvgl-example-disco-sim
cd playit/node && RLVGL_DISCO_SIM_BIN="$PWD/../../target/debug/rlvgl-disco-sim" node --test && cd ../..

# Phase 4.6: ESP32-C3 BSP generator + beetle-esp32c3 both feature sets
# The `compile-verify` feature spins up a throwaway cargo project around the
# generated BSP and type-checks it against real `esp32c3 = 0.31` on
# `riscv32imc-unknown-none-elf`; it needs `rustup target add riscv32imc-unknown-none-elf`
# and network access for the PAC crate, so it's opt-in.
RUSTFLAGS="" cargo test -p rlvgl-chips-esp
RUSTFLAGS="" cargo test -p rlvgl --test esp_ir_roundtrip --features creator
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_cli --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c3_compile --features compile-verify -- --test-threads=1
RUSTFLAGS="" cargo check -p rlvgl-example-beetle-esp32c3 --features esp_hal --target riscv32imc-unknown-none-elf
RUSTFLAGS="" cargo check -p rlvgl-example-beetle-esp32c3 --features bsp_pac --target riscv32imc-unknown-none-elf
RUSTFLAGS="" cargo clippy -p rlvgl-example-beetle-esp32c3 --features esp_hal --target riscv32imc-unknown-none-elf -- -D warnings
RUSTFLAGS="" cargo clippy -p rlvgl-example-beetle-esp32c3 --features bsp_pac --target riscv32imc-unknown-none-elf -- -D warnings

# Phase 4.7: ESP32-P4 + ESP32-C6 BSP generator tests
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32p4_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32p4_cli --features creator
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c6_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c6_cli --features creator

# Phase 4.7b: remaining RISC-V chips with linker-script emission
# (ESP32-C5 / H2 / C61). Render tests cover memory.x + <chip>.x.
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c5_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32h2_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_esp32c61_render --features creator,regression

# Phase 4.7c: Silicon Labs BSP generator (CHIPS-SILABS-NN). Render test
# covers the 8-file emission set (6 .rs + memory.x + efm32_gg11.x) for
# SLSTK3701A after CHIPS-SILABS-05 of 2026-05-14 ratified and shipped
# the linker emission per CHIPS-SILABS-00 §11. The opt-in compile-verify test
# (CHIPS-SILABS-04) materialises a throwaway cargo project around the
# generated BSP and runs `cargo check --target thumbv7em-none-eabihf`
# against efm32gg11b-pac 0.1.4. Four amendments landed: -02 of
# 2026-05-13 (SKU-flatten in pac.rs) resolved 5×E0433. -02b of
# 2026-05-13 (field-style register access in clocks/io_mux/peripherals)
# dropped errors 102 → 11. -01c of 2026-05-14 (chipdb yaml: GPIO
# clock-gate routed through `cmu.hfbusclken0.gpio` to match PAC, not
# RM's `hfperclken0`). -02c of 2026-05-14 (io_mux MODEH branch emits
# absolute `mode{N}` not relative `mode{N-8}` to match PAC's writer
# field names). Compile-verify gate now PASSES end-to-end.
RUSTFLAGS="" cargo test -p rlvgl-chips-silabs
RUSTFLAGS="" cargo test -p rlvgl --test bsp_silabs_slstk3701a_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_silabs_slstk3701a_compile --features compile-verify -- --test-threads=1

# Phase 4.7d: Texas Instruments BSP generator (CHIPS-TI-NN). Render
# test covers the 8-file emission set (6 .rs + memory.x + cc1352_r.x)
# for LAUNCHXL-CC1352R1; CHIPS-TI-05 of 2026-05-14 ratified the
# already-shipping linker emission per CHIPS-TI-00 §11. The opt-in
# compile-verify test (CHIPS-TI-01d)
# runs `cargo check --target thumbv7em-none-eabihf` against
# cc13x2_26x2_pac 0.10.3. Lowercase-peripheral amendment (-01b of
# 2026-05-13) lowercased `p.PRCM` / `p.IOC` / `p.GPIO` / `p.UART0`
# field access. Residual-structural amendment (-01e of 2026-05-13)
# converted `iocfg(n)` indexer → per-DIO `iocfgN()` methods, added
# `clk_en_variant` chip-yaml field for the 2-bit `ClkEn` FieldWriter,
# and corrected PRCM reset-register field names to PAC's instance
# suffixes (`uart0`/`i2c0`). Compile-verify gate now PASSES end-to-end.
RUSTFLAGS="" cargo test -p rlvgl-chips-ti
RUSTFLAGS="" cargo test -p rlvgl --test bsp_ti_cc1352r_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_ti_cc1352r_compile --features compile-verify -- --test-threads=1

# Phase 4.8: Microchip BSP generator (CHIPS-MICROCHIP-NN). Render test
# covers the 8-file emission set (6 .rs + memory.x + atsamd51j19a.x)
# for the Adafruit Feather M4 Express after CHIPS-MICROCHIP-05 of
# 2026-05-14 ratified the linker chapter and added the per-chip
# include alongside the already-shipping memory.x per
# CHIPS-MICROCHIP-00 §11. PB22/PB23 chip-yaml correction (CHIPS-MICROCHIP-01a
# of 2026-05-13) replaced the MISMATCH fallback comments with real
# SERCOM1_PAD2/PAD3 PMUX writes. Field-style template amendment
# (CHIPS-MICROCHIP-02 of 2026-05-13) switched `p.MCLK.apbamask` /
# `p.GCLK.pchctrl[N]` / `p.PORT.groupN.pmux[H]` to direct field access
# matching atsamd51j19a 0.7.1's pre-method-accessor svd2rust era.
# Compile-verify gate (CHIPS-MICROCHIP-04) now PASSES end-to-end.
RUSTFLAGS="" cargo test -p rlvgl-chips-microchip
RUSTFLAGS="" cargo test -p rlvgl --test bsp_microchip_render --features creator,regression
RUSTFLAGS="" cargo test -p rlvgl --test bsp_microchip_compile --features compile-verify -- --test-threads=1

# Phase 5: docs
RUSTFLAGS="" cargo doc --workspace --no-deps

# Phase 6: embedded target (full build + .hex/.bin)
make build-disco

# Phase 7: publish dry run
DRY_RUN=1 scripts/publish_changed.sh HEAD~1
```

## Profiling Guidance

- Prefer DWT + D3 SRAM telemetry over serial output when measuring timing.
- Use serial for control, coarse summaries, and targeted framebuffer dumps.
- The CM7 main loop is intended to stay responsive while DMA2D and USART1 IRQs
  run in the background. If you add new waits, they should be stateful and
  return to the loop instead of spinning.
- Relevant telemetry now includes idle cycles, loop count, pipeline stage/frame,
  DMA2D last/max cycles, DMA completion/error counts, and serial queue/drop
  counters.

## Register-Mashing Discipline (STM32H747I-DISCO)

Raw MMIO and pointer-heavy code in the 747I tree must follow an
ownership/provenance discipline — the goal is not to eliminate `unsafe`, but to
give `unsafe` blocks a narrow legal envelope and to make aliasing, provenance,
and address-domain mistakes compile errors rather than silent runtime bugs.

This discipline is being staged into `platform/` via the plan at
`/Users/iraabbott/.claude/plans/let-s-plan-to-mitigate-parallel-anchor.md` and
locked in by `platform/tests/discipline.rs` (grep scanner) plus
`platform/tests/discipline_compile.rs` (trybuild compile-fail fixtures). Until
Step 9 lands, the scanner runs in baseline mode with a shrinking exemption list.

### Normative rules

1. **Contain raw MMIO at the perimeter.** `*mut u32 = ADDR as *mut u32` style
   constants MUST live in `platform/src/hwcore/regs/` (typed register-block
   modules) or in vendored PAC code. They MUST NOT appear inline in example
   binaries, app-level code, or other `platform/` modules.
2. **Typed framebuffer ownership.** Framebuffer addresses MUST flow as
   `FrameBuffer` / `FrontBuffer<'a>` / `BackBuffer<'a>` handles, not as bare
   `u32` or `*mut u8`. Shims returning `u32` (e.g. `front_buffer_addr()`) are
   `#[deprecated]` during migration and REMOVED at Step 9.
3. **DMA as ownership transfer.** DMA2D submission MUST return an
   `InFlight<'dma, T>` token that borrows the destination buffer; CPU access
   during a transfer is prevented at compile time. Raw pointer DMA2D entry
   points (`start_fill_raw`, `start_blit_raw`) are `#[doc(hidden)]` during
   migration and REMOVED at Step 9.
4. **Three address domains, three types.** MMIO register addresses, CPU RAM
   pointers, and DMA bus addresses MUST NOT collapse into a single `u32`.
   Use `MmioAddr<T>`, `PhysAddr`, `DmaAddr` from `platform::hwcore::addr`.
   Conversions between them go through explicit methods that assert alignment
   / provenance preconditions.
5. **Typed register blocks for DSI / LTDC / DMA2D.** Register layouts MUST be
   expressed as `#[repr(C)]` structs with `const_assert_eq!(offset_of!(...),
   0x…)` assertions. Hand-offset pointer arithmetic (the class of bug that
   caused the LCCR-at-0x2C panel-snow incident) MUST NOT recur — the wrong
   offset becomes a compile-time failure.
6. **ISR shared state through `IsrChannel<T,N>`.** `static mut` declarations
   MUST live in `platform/src/hwcore/isr.rs`. Application ISRs use
   `IsrChannel<T,N>`, `IsrFlag`, `IsrCounter` — the volatile + `compiler_fence`
   plumbing is encapsulated. Direct `compiler_fence(` calls outside `hwcore/`
   are a discipline violation.
7. **Unsafe block hygiene.** Every `unsafe { ... }` block or `unsafe fn` in
   `platform/` and `examples/stm32h747i-disco/` MUST carry a `// SAFETY:`
   comment naming (a) what memory is accessed, (b) who owns it, (c) why
   aliasing is acceptable, (d) what synchronization is required. Large
   `unsafe` functions SHOULD be decomposed so the `unsafe` envelope is as
   small as practical.
8. **Volatile is not provenance.** Volatile access semantics (`read_volatile`,
   `write_volatile`, PAC `.write()` / `.modify()`) govern reordering and
   elision. They do NOT repair aliasing, lifetime, or provenance errors in
   surrounding RAM logic. Treat them as orthogonal concerns.

### Applicability

- **IN SCOPE**: `platform/` (all modules), `examples/stm32h747i-disco/`
  (bare-metal + FreeRTOS builds), `display_init.rs`, `dma2d.rs`,
  `dma2d_draw.rs`, ISR paths in `main.rs`.
- **OUT OF SCOPE**: `examples/beaglebone-black/src/bsp/` (DevMem abstraction
  already clean), `examples/beetle-esp32c3/src/bsp_generated/` (PAC-typed by
  generator), `esp_hal`-based code, vendored PAC crates, audio/QSPI/SDMMC
  paths (already PAC/HAL-typed).
- **Zephyr port**: consumes `display_init.rs` and therefore inherits the
  typed register-block rule (#5) once Step 6 lands. Zephyr-specific glue in
  `zephyr_entry.rs` follows #7 but is otherwise scoped separately.

### Enforcement

During staged migration (Steps 1–8):

```bash
# Shows violations against BASELINE exemptions; fails only on *new* violations.
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
# trybuild fixtures enforce the InFlight / Scanout / MmioAddr contracts.
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile
```

At Step 9 (full strict mode, enforced by Phase 2.5 of pre-publish):

```bash
# Phase 2.5: hardware-abstraction discipline (STM32H747I-DISCO)
RLVGL_LINT_STRICT=1 RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile
# The disco-sim host integration test for the typed types runs in Phase 4.5
# via `cargo test -p rlvgl-example-disco-sim` — do not duplicate here.
```

`RLVGL_LINT_STRICT=1` asserts the `BASELINE` array in `discipline.rs` is empty.
Reviewers MAY grant a temporary `RLVGL_LINT_STRICT=0` waiver for an emergency
hotfix, but the same PR MUST file a follow-up issue to restore strict mode.

### Opt-out marker for legitimate exceptions

Lines containing `// rlvgl-discipline: allow(<rule_id>)` are exempt from the
scanner. Use sparingly, with justification in the surrounding comment. Rule
IDs: `raw_mmio_cast`, `raw_addr_cast`, `static_mut`, `raw_dma2d`,
`fb_addr_shim`, `compiler_fence`.

## Espressif BSP Generator

`rlvgl-creator bsp from-yaml --vendor esp` generates raw-PAC bring-up code
for Espressif boards (ESP32-C3, ESP32-C6, ESP32-P4) from chipdb YAML:

- **Chip inventory**: `chipdb/rlvgl-chips-esp/db/chips/{esp32c3,esp32c6,esp32p4}.yaml` —
  memory map, clock tree, system clock-gate table (SYSTEM for C3, PCR for C6,
  HP_SYS_CLKRST for P4), IO MUX per-pin function table, GPIO matrix signal
  subset. Derived from each chip's Technical Reference Manual.
- **Board specs**: `chipdb/rlvgl-chips-esp/db/boards/*.yaml` — pin
  assignments, console config, optional `i2c_configs:` SCL frequency map.
  Ships with `esp32c3_devkitm_1.yaml`, `beetle_esp32c3.yaml`,
  `beetle_esp32p4.yaml` (DFR1172 P4 side), and `beetle_esp32c6.yaml`
  (DFR1172 C6 companion).
- **Generated output**: 6 files per board (`mod.rs`, `pac.rs`, `clocks.rs`,
  `io_mux.rs`, `peripherals.rs`, `board.rs`) emitting svd2rust-style writes
  against `esp32c3 = 0.31`. Peripheral instances use uppercase field access
  (`p.UART0.clkdiv()`, `p.IO_MUX.gpio(21)`). Sibling module references use
  `super::` so the output works both as a crate root and as a child module
  of a host crate.
- **Templates**: `src/bin/creator/bsp/espressif/templates/*.rs.jinja` —
  edit these to grow peripheral init coverage. `peripherals.rs.jinja`
  currently has real UART0-as-console and I2C0 master init; everything
  else is a TODO stub pointing at the relevant PAC register block.

### Regenerate a board's BSP

```bash
cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
    --vendor esp \
    --board beetle_esp32c3 \
    --out /tmp/rlvgl-bsp \
    --emit-pac
```

Copy the five child files (`board.rs`, `clocks.rs`, `io_mux.rs`, `pac.rs`,
`peripherals.rs`) from `/tmp/rlvgl-bsp/dfr0868_beetle_esp32_c3/` to the
consuming crate's `src/bsp_generated/`. The host `mod.rs` is written by
hand as a module index; the generator's own `mod.rs` is crate-root-shaped
and not copied in.

### Compile-verify the output

The snapshot tests (`tests/bsp_esp32c3_render.rs`) only check *text*. To
prove the generated code actually type-checks against the real
`esp32c3 0.31` PAC crate on `riscv32imc-unknown-none-elf`, run:

```bash
rustup target add riscv32imc-unknown-none-elf
cargo test --test bsp_esp32c3_compile --features compile-verify -- --test-threads=1
```

This materializes a throwaway cargo project around the generated files for
every chipdb board and runs `cargo check`. The target dir is cached under
`$TMPDIR/rlvgl-bsp-<tag>-compile-verify-target` so reruns are fast. Template
edits that break compilation surface here first.

### beetle-esp32c3 feature matrix

`examples/beetle-esp32c3/` has two parallel entry points selected by
mutually exclusive features:

- `--features esp_hal` → `src/esp_hal_main.rs`, uses esp-hal's high-level
  I2C/Delay/`#[esp_hal::main]`. Known-working rlvgl + SSD1306 path.
- `--features bsp_pac` → `src/bsp_pac_main.rs`, consumes the generated
  BSP under `src/bsp_generated/` and drives an LED blink via raw PAC. Proves
  the chipdb → generator → compile → boot pipeline. Does **not** currently
  include SSD1306 or rlvgl — SSD1306 over raw PAC needs command-list
  chunking (ESP32-C3 TX FIFO is 32 bytes, SSD1306 framebuffer writes are
  ~1 KB) and is deferred to a follow-up milestone.

## Spec-Before-Code Planning Discipline

Multi-phase initiatives in this repo — the STM32H747I-DISCO bring-up
(`docs/disco-platform-guide/`, `docs/disco-tutorial/`,
`docs/disco-freertos-guide/`, `docs/disco-zephyr-guide/`,
`docs/disco-test-and-debug/`), the BeagleBone Black + NHD cape four-prong
port (`docs/beaglebone-black/`), the `rlvgl-creator` + chipdb family
(`docs/creator/`, `docs/bsp/`, `chipdb/rlvgl-chips-*`), and any future
multi-chapter guide — follow a
standards-body-style planning cycle: every behaviour change is preceded by
a ratified *terms* doc. Vocabulary drift and invariant erosion are the
dominant failure modes once a plan crosses ~3 phases, especially across
parallel ports (Linux / bare-metal / FreeRTOS / Zephyr) where the same
concept name can quietly pick up different semantics in each tree. The
cycle exists to prevent silent forks, not as ceremony.

### Normative keywords (RFC 2119 / 8174)

The key words **MUST**, **MUST NOT**, **SHALL**, **SHOULD**, **SHOULD
NOT**, **MAY**, and **RECOMMENDED** in initiative-family guide docs and
per-chapter concepts docs are interpreted per RFC 2119 and RFC 8174. Use
capitals when invoking the keyword; lowercase for ordinary English. Plain
narrative without capitalised keywords is advisory, not binding.

### Normative vs. informative sections

In a per-chapter doc (e.g. `docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`,
`docs/beaglebone-black/README.md` sections):

- Sections referenced by the chapter's **Acceptance** checklist (or by
  `docs/releases/roadmap-pre-v0.2.md` checkboxes that cite the chapter) are
  **normative** — binding on implementers.
- All other sections (problem statement, narrative, lessons-learned,
  non-goals, change log) are **informative**.
- The initiative README (e.g. `docs/disco-platform-guide/README.md`) is
  **informative**; per-chapter docs are the normative artifacts.

Do not re-derive normative rules in README narrative — cite the
per-chapter doc and section heading.

### Conformance targets

Each initiative README MUST name the conforming artifact and its
acceptance gates. Optional phases yield a second conformance level so
reviewers can reason about partial deployments without re-arguing scope.
Worked example for the BBB port:

- "A conforming BBB deployment MUST satisfy the Phase 3 Linux-prong
  acceptance gates in `docs/beaglebone-black/README.md`."
- "A conforming BBB deployment MAY additionally satisfy Phase 4
  (bare-metal), FreeRTOS, or Zephyr; each is independently conformant."

Same pattern for the DISCO guide (Volume I tutorial MUST, Volume II
platform guide SHOULD, FreeRTOS/Zephyr MAY) and for `rlvgl-creator`
(chipdb render tests MUST pass; `compile-verify` SHOULD pass;
hardware bring-up on the generated BSP MAY pass at initiative close).

### Definitions — reference vs. restatement

For every term that also exists in code, the glossary entry MUST cite the
authoritative source and mark the relationship:

- **"As defined in [path/to/file.rs:line]; used without modification."**
  — repo is canonical; spec references it. Example: `DiscoCommand` in
  `examples/apps/disco-demo/src/lib.rs`.
- **"As defined in [path/to/file.rs:line]; adapted: [delta]."** — repo
  is canonical; spec extends/narrows it with a named delta. Example: BBB
  `Screen::landscape(800, 480)` uses the same type as DISCO but with a
  different `frame_hz` derivation (`PIXEL_CLOCK_HZ / (HTOTAL * VTOTAL)`).
- **"Owned by <CHAPTER>; does not exist in repo yet."** — spec is
  canonical; repo will mirror once the chapter lands. Example:
  `DevMem::translate_mut` before the bsp refactor shipped.

Silent restatement of an existing repo definition is how forks form
between the four BBB prongs and the four DISCO build profiles. Don't do
it.

### Frozen enumerations — registration policy

Every frozen enum (`PixelFmt`, `Rotation`, `ColorFormat`, `DiscoCommand`,
`Effect`, chipdb vendor set `{esp, stm, ti, nxp, nrf, renesas, silabs,
rp2040, microchip}`, BBB prong set `{linux, bare_metal, freertos,
zephyr}`, etc.) declares its registration policy in the concepts doc:

- **Standards Action** — adding a value requires a change-log amendment to
  the initiative's canonical concepts doc and an explicit go-ahead from
  the owner. Use for enums encoding cross-phase contracts (prong set,
  chipdb vendor set, `PixelFmt`).
- **Specification Required** — adding a value requires a per-chapter
  walkthrough update, no concepts-doc amendment. Use for enums local to
  one chapter's surface (e.g. `DiscoCommand` variants for a new demo
  feature).
- **Expert Review** — chapter owner MAY add with a PR-level note. Use
  for internal enums with no cross-phase coupling (private state
  machines inside one crate).

Default to Standards Action when in doubt; demote later if churn
justifies.

### Phase document shape

A per-chapter concepts doc follows this section layout: §0 authority
policy (which external doc owns which vocabulary — RM0399 for H747,
SPRUH73Q for AM335x, TRM-by-vendor for chipdb crates), §1 purpose, §2
problem statement (evidence pinned to code paths, e.g.
`platform/src/dsi_cmd_mode.rs:NN`), §3 canonical glossary, §4
source-of-truth map (one owner per concept across the four prongs), §5–§9
frozen decisions (enums, register-bit positions, timing invariants), §10
reconciliation decisions vs. adjacent repo primitives (e.g. how a new
`rlvgl-creator` BSP maps onto the hand-written H747 BSP), §11 non-goals,
§12 acceptance checklist, §13 files cited, §14 unblocks, §15 change log.
Chapters beyond the §0 concepts gate MAY omit sections that do not apply;
§0, §3/§4, §10, §12, §15 are load-bearing.

See
[`docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`](docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
and [`docs/beaglebone-black/README.md`](docs/beaglebone-black/README.md) as the current
reference shapes. Neither yet uses the full §0–§15 structure; they MUST
adopt it when their initiatives cross the ~3-phase threshold or gain a
sibling port.

### Execution discipline

Once a concepts doc is ratified (dated change-log entry), execution PRs:

- Cite the initiative-and-phase code in the commit subject. Suggested
  prefixes: `DISCO-NN[a-z]:` for disco-platform-guide chapters,
  `BBB-NN[a-z]:` for BeagleBone Black phases,
  `CREATOR-NN[a-z]:` for rlvgl-creator phases,
  `CHIPS-<VENDOR>-NN[a-z]:` for per-vendor chipdb crates
  (e.g. `CHIPS-ESP-02b:`, `CHIPS-STM-04a:`).
- Name in the PR description which invariants (from the concepts doc's
  frozen-decisions sections) the change touches, and how each is
  preserved.
- Touching a frozen enum value or an invariant (register-bit position,
  pixel format, prong-set membership) requires a change-log amendment
  **first**, in a separate PR. No behaviour PR rides on an unamended
  invariant.

Conventional-commit style (`feat:`, `fix:`, `docs:`, `tools:`) remains
the default for non-initiative work; the initiative prefix replaces the
conventional type when the change is scoped to a ratified phase.

### Initiative retrospective

Multi-phase initiatives MUST land an **initiative retrospective**
when they reach natural completion (every named phase either
shipped or closed-with-deferral; the BASELINE / acceptance
gate the initiative was designed to clear is satisfied; or the
owner declares completion explicitly). Retrospective in the
agile sense — neutral framing, oriented at *what to encode for
next time*, not at blame or celebration.

#### File and naming

One retrospective per initiative, co-located with the phase
docs at `<initiative-dir>/<INIT>-RETROSPECTIVE.md` (e.g.
`docs/concepts/DCB-RETROSPECTIVE.md`,
`docs/disco-platform-guide/DISCO-RETROSPECTIVE.md`,
`docs/beaglebone-black/BBB-RETROSPECTIVE.md`). The file is
preserved as a historical artifact; behaviour PRs reference
the canonical concepts doc + §15 change log directly, never
the retrospective.

A second retrospective for the same initiative is permitted
only if the initiative resumes from a closed state (a
DCB-NN-B reopen succeeds, a paused phase-set reactivates,
etc.) and produces a structurally distinct second completion
event. The second retrospective amends the original via a
new dated entry in §8, not a separate file.

#### Content shape (sections §1–§7)

The retrospective MUST capture:

- **§1 Outcome snapshot**. Final architecture / artifacts;
  deferred items enumerated explicitly (not implied); known
  residual risks named with the configuration-or-platform
  assumption each one rides on.
- **§2 Divergence log**. Where reality diverged from plan,
  one entry per divergence. Each entry as **Assumption →
  Symptom → Root cause → Detection gap**: what the spec
  said; observable failure mode (not interpretation);
  mechanistic root cause (not narrative); why automated
  gates didn't catch it earlier. This is the load-bearing
  section.
- **§3 Refactor points**. Decision inflection nodes —
  where the initiative changed direction. Each as
  **Trigger → Alternatives → Selection rationale → Cost of
  switch**. Future efforts use this section to short-circuit
  re-exploration of branches that were already evaluated.
- **§4 Mitigation patterns**. Abstract the fixes into
  reusable units. "When X + Y → apply Z pattern". Encode as
  guardrails, invariants, pre-flight checks, or template
  code patterns. This is the bridge into project-wide
  AGENTS.md / coding guidelines.
- **§5 Deferred work reclassification**. Don't leave
  deferred items as a flat list. Classify as **Safe**
  (orthogonal, no impact on core invariants), **Coupled**
  (affects assumptions; must be revisited with context —
  name the assumption explicitly), or **Abandoned**
  (explicitly killed, with a resurrection-prevention note
  to deter future agents from re-deriving it).
- **§6 Forward constraints**. Lessons turned into
  preconditions for the next initiative. "Do not start
  without X validated"; "Y must be treated as unstable
  boundary"; "Z requires instrumentation before
  integration". This is the only normative section in the
  retrospective; future planning docs treat these as
  binding rules.
- **§7 Provenance hooks**. Link each divergence and
  refactor point to the authoritative artifacts: commit
  range, sub-letter doc, §15 amendment, datasheet
  reference, external evidence. Future agents traverse
  **outcome → issue → fix → underlying evidence** in one
  hop.

A `§8 change log` follows §1–§7, dated, recording when the
retrospective was drafted and any subsequent amendments.

#### Tone

Neutral. Surface root causes mechanistically; don't soften
findings or assign blame. The audience is future Codex /
Claude agents working on a structurally similar initiative
— they need actionable signal, not narrative.

#### Applicability

The retrospective discipline applies to every initiative
covered by this Spec-Before-Code Planning Discipline section
(see §Applicability below). For port-shaped initiatives
(`DISCO-NN`, `BBB-NN`, `CHIPS-VENDOR-NN`) the retrospective
captures lessons from the multi-prong / multi-chapter
trajectory. For cross-cutting initiatives (`DCB`, future
`docs/concepts/`-based families) the retrospective captures
the spec-vs-implementation divergences that the per-phase
docs don't.

The first reference implementation is
`docs/concepts/DCB-RETROSPECTIVE.md` (DCB initiative,
2026-05-03). Future retrospectives MAY adopt that doc's
specific section breakdowns or amend the §1–§7 shape with a
named justification in their §8 change log.

### Applicability

This discipline applies to:

- `docs/disco-platform-guide/` (DISCO Volume II platform guide, 11
  chapters)
- `docs/disco-tutorial/` (DISCO Volume I tutorial, 7 chapters)
- `docs/disco-freertos-guide/` (DISCO FreeRTOS port, 7 chapters)
- `docs/disco-zephyr-guide/` (DISCO Zephyr port, 7 chapters)
- `docs/disco-test-and-debug/` (DISCO test & debug, 4 chapters)
- `docs/beaglebone-black/` (BBB four-prong)
- `docs/creator/`, `docs/bsp/`, and `chipdb/rlvgl-chips-*` (rlvgl-creator + chipdb
  family)
- Any future multi-chapter initiative with ≥3 phases.

Single-doc TODOs, phase-1 prototypes, and one-off explorations MAY use
informal form. The moment a family produces a second chapter citing the
first, the full discipline applies to that family.

The point is convergence over time: form is cheaper to align than
vocabulary.
