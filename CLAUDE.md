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
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage
```

- This is the current rust-only profiling build.
- It links and boots successfully as a dev build.
- `cpu_stats` is for DWT/D3 telemetry and is not the known flashable release profile.
- `audio` is excluded: the WM8994 I2C4 init hangs at boot, blocking the
  main loop.  SAI1 clocks run (open-channel noise audible) but the codec
  never completes configuration.  Re-enable after fixing the I2C4 timeout.

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

# Phase 3: playit crate (standalone — no_std cross-compile + package)
RUSTFLAGS="" cargo test -p rlvgl-playit
RUSTFLAGS="-C target-cpu=cortex-m7" cargo check --target thumbv7em-none-eabihf -p rlvgl-playit
cd playit && cargo package --list --allow-dirty && cd ..

# Phase 4: simulator + creator tests
RUSTFLAGS="" cargo build -p rlvgl-example-sim
RUSTFLAGS="" cargo test -p rlvgl-example-sim
RUSTFLAGS="" cargo test --tests --features "creator" -p rlvgl

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
