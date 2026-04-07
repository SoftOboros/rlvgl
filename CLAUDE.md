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
  --bin rlvgl-stm32h747i-disco \
  --features stm32h747i_disco_cm7,splash,desktop,dma2d,cpu_stats
```

- This is the current rust-only profiling build.
- It links successfully as a dev build.
- `cpu_stats` is for DWT/D3 telemetry and is not the known flashable release profile.

### Compile-safety check for the leaner profiling feature set

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo check \
  --target thumbv7em-none-eabihf \
  --bin rlvgl-stm32h747i-disco \
  --features stm32h747i_disco_cm7,dma2d,cpu_stats,pac_sdram_init,sdram_ramtest,backlight_pwm
```

### Cached flashable release profile

The most recent cached successful release fingerprint under
`target/thumbv7em-none-eabihf/release/.fingerprint/.../bin-rlvgl-stm32h747i-disco.json`
records:

```text
stm32h747i_disco_cm7,dma2d,splash,desktop,audio
```

Cached artifact expectations from `target/`:

- `target/thumbv7em-none-eabihf/release/rlvgl-stm32h747i-disco`: about `321K`
- `target/thumbv7em-none-eabihf/release/rlvgl-stm32h747i-disco.bin`: about `152K`
- `target/rlvgl-disco.hex`: about `448K`

Do not assume a new `cpu_stats` build will still fit flash just because the
cached `.hex` does.

## Flashing And Debug

### Probe-rs via Makefile

```bash
make probe-rs-gdb
```

That target builds the disco image, downloads it, and starts a probe-rs GDB
server.

### Direct probe-rs flash

```bash
probe-rs download --chip STM32H747XIHx target/rlvgl-disco.hex
```

For ELF-based downloads, point `probe-rs download` at the matching ELF instead.

### Direct probe-rs GDB server

```bash
probe-rs gdb --chip STM32H747XIHx --gdb-connection-string 127.0.0.1:3333
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

## Runtime Command Protocol

Commands accepted over the serial helper:

- `?`
  - prints the current tick count and serial queue/drop state
- `T<x>,<y>`
  - injects a synthetic `PressRelease` at landscape coordinates `(x, y)`
- `D<x>,<y>,<w>,<h>[,<frames>]`
  - queues a front-buffer hex dump window; one frame is emitted per completed present
- `C`
  - toggles the star crawl

## Profiling Guidance

- Prefer DWT + D3 SRAM telemetry over serial output when measuring timing.
- Use serial for control, coarse summaries, and targeted framebuffer dumps.
- The CM7 main loop is intended to stay responsive while DMA2D and USART1 IRQs
  run in the background. If you add new waits, they should be stateful and
  return to the loop instead of spinning.
- Relevant telemetry now includes idle cycles, loop count, pipeline stage/frame,
  DMA2D last/max cycles, DMA completion/error counts, and serial queue/drop
  counters.
