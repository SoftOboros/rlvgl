<!--
05-host-and-bare-metal.md - Chapter 5 of the Ratatui on rlvgl tutorial.
-->

# Chapter 5 — Host and Bare Metal

**←** [Chapter 4 — Driving the live table](04-live-state.md) **·** [Index](README.md)

---

Develop and verify on the host first. The SCTD simulator mounts the same
`rlvgl-app-sctd-demo` crate and the same `ratatui-rlvgl` bridge as the board;
only the display/input wrapper changes.

## Run the host simulator

From the repository root:

```bash
cargo run -p rlvgl-example-disco-sim --bin rlvgl-sctd-sim
```

The logical screen is 800×480 by default. Select the Dining Philosophers
screen, exercise a few transitions on the native table, then press
**Ratatui**. The popup should preserve the current seat/fork state. Its title,
rounded border, close button, and bottom controls are native rlvgl; the table
inside is Ratatui.

For a noninteractive render:

```bash
cargo run -p rlvgl-example-disco-sim --bin rlvgl-sctd-sim -- \
  --headless=/tmp/sctd-frame.txt
```

For automated input and framebuffer queries, add `--playit-port=<port>` and
use the repository's playit protocol. The simulator entry point documents all
three options in
[`sctd_main.rs`](../../examples/disco-sim/src/sctd_main.rs).

## Run the focused tests

```bash
cargo test -p ratatui-rlvgl
cargo test -p rlvgl-app-sctd-demo
cargo test -p rlvgl-example-disco-sim
```

These cover retained-surface semantics, cell rendering and input translation,
hero lifecycle, state preservation, action dispatch, and host integration.

## Build the STM32H747I-DISCO proof

Install the Rust target once:

```bash
rustup target add thumbv7em-none-eabihf
```

Then build the SCTD payload on the established bare-metal runtime:

```bash
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-sctd \
  --features cm7,sctd,dma2d
```

The `sctd` feature selects the tutorial payload inside the same runtime that
initializes clocks, SDRAM, touch, DMA2D, LTDC, DSI, framebuffers, and playit.
There is no second or tutorial-specific board bring-up path.

To demonstrate that target compilation does not depend on a C compiler, run
the same build with `CC` and `CXX` pointing at nonexistent executables:

```bash
CC=/no-c-compiler CXX=/no-cxx-compiler \
RUSTFLAGS="-C target-cpu=cortex-m7" \
cargo build \
  --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco \
  --bin rlvgl-stm32h747i-sctd \
  --features cm7,sctd,dma2d
```

This feature set intentionally excludes `c_hal`, FreeRTOS, Zephyr, and
ESP-IDF. It is the narrow Rust-all-the-way-down proof, not a claim that every
other supported rlvgl platform forbids C components.

## What to verify on the board

Use the same sequence as the host:

1. start with all five seats occupied on the native table;
2. issue `Depart`, then `Arrive`;
3. open the Ratatui hero without resetting the machine;
4. issue another `Depart`, then `Arrive` using the native graphical controls;
5. close and reopen the popup and confirm state is preserved; and
6. leave it running through timer updates while watching for flicker,
   incomplete buffer swaps, clipped corners, or stale cells.

The promotional GIF in the tutorial index is the deterministic host capture.
A board video separately proves that the same composition reaches the
STM32H747I-DISCO panel through the Rust bare-metal display path.

## The completed stack

You now have both integration directions:

- a Ratatui terminal renders through rlvgl without a terminal emulator; and
- an rlvgl interface hosts the Ratatui terminal as one clipped child pane.

The retained surface keeps ownership honest, the presentation snapshot keeps
the generated machine out of rendering code, and the host/board split changes
only the platform layer.

Continue with the [State Chart → Reactive UI tutorial](../sctd-tutorial/README.md)
to reconstruct the generated machine and Qt-derived media-player portions of
the larger demo.

---

**←** [Chapter 4 — Driving the live table](04-live-state.md) **·** [Index](README.md)
