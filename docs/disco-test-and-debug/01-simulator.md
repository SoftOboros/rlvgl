<!--
01-simulator.md - Volume III Chapter 1: rlvgl-sim and rlvgl-disco-sim.
-->

**← Prev · [Index](README.md) · [Next →](02-uefi.md)**

# Chapter 1 — Host Simulator

## What this chapter covers

Two binaries that let you run rlvgl UIs on your dev machine
with no hardware attached:

- **`rlvgl-sim`** — generic simulator for the stock demo with
  image/codec plugins enabled. Good for experimenting with
  widgets and assets.
- **`rlvgl-disco-sim`** — the disco app itself, running the
  shared `rlvgl-app-disco-demo` controller against a wgpu
  window sized to match the real panel. This is the one you
  want if you're iterating on the disco UI before flashing.

Both sit on top of `rlvgl-platform` with the `simulator`
feature; the rest of the stack is identical to the hardware
build.

## The two binaries

### `rlvgl-sim` (generic)

[`examples/sim/`](../../examples/sim/) — package
`rlvgl-example-sim`, binary `rlvgl-sim`.

```bash
make build-sim
./target/debug/rlvgl-sim              # default 320×240 window
./target/debug/rlvgl-sim --screen=800x480
```

Builds with `--features png,jpeg,gif,qrcode,fontdue` per the
repo README's host-tools table. Use this to try rlvgl widgets
against your own asset pack — it hard-links the
`rlvgl-app-demo` crate as the running application.

### `rlvgl-disco-sim` (disco-specific)

[`examples/disco-sim/`](../../examples/disco-sim/) — package
`rlvgl-example-disco-sim`, binary `rlvgl-disco-sim`.

```bash
make build-disco-sim
./target/debug/rlvgl-disco-sim                       # 800×480 GUI
./target/debug/rlvgl-disco-sim --automation-headless # no window, playit only
./target/debug/rlvgl-disco-sim --playit-port=4567    # explicit TCP port
./target/debug/rlvgl-disco-sim --headless=/tmp/ascii.txt  # dump frame + exit
```

This binary hosts the shared `rlvgl-app-disco-demo` controller
— the **exact same code** the firmware and UEFI targets run —
over a wgpu-accelerated window (eframe + winit) at 800×480.
The playit TCP server is built in; see
[Chapter 3](03-playit.md) for how tests talk to it.

## How to link your own app

The cleanest way to get a new app running in the simulator is
the pattern `examples/disco-sim/` already demonstrates:

1. **Put your app in a shared no_std + alloc library crate**
   — like `rlvgl-app-disco-demo`. It owns the widget tree,
   depends on `rlvgl-core` / `rlvgl-widgets` / `rlvgl-ui` and
   emits abstract commands (`DiscoCommand` in the disco case).

2. **Build two thin frontends that depend on the library** —
   one for host (`rlvgl-platform/simulator` feature, wgpu
   blitter, winit event loop) and one for hardware
   (`rlvgl-platform/stm32h747i_disco` feature, Chapter 5 of
   Vol II's bring-up). The library does not care which is
   running.

3. **Wire platform-specific command handlers** in the
   frontends — backlight PWM on hardware, a log print in the
   sim, etc. Vol I
   [Chapter 6](../disco-tutorial/06-hook-actions.md) showed
   this pattern for hardware.

4. **Expose a playit server** in the host frontend (Ch 3).
   Once playit is bound, the same test scripts drive sim and
   hardware.

The disco app follows this structure end-to-end — see
[`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
for its capability matrix and the three adapters that consume
it (host sim, UEFI, hardware).

## Debugging the simulator

`rlvgl-sim` and `rlvgl-disco-sim` are plain host binaries, so
every host debugging tool works:

- **LLDB via VS Code**: the checked-in `.vscode/launch.json`
  has a **"Host: rlvgl-sim"** configuration (LLDB, pre-builds
  via `build-sim` task). Duplicate and point `program` at
  `target/debug/rlvgl-disco-sim` if you want the disco
  variant. Chapter 4 walks through the VS Code setup.
- **`rust-gdb` / `rust-lldb`** from the command line:
  ```bash
  rust-lldb ./target/debug/rlvgl-disco-sim
  (lldb) b rlvgl_app_disco_demo::ControllerState::tick
  (lldb) run
  ```
- **`println!`** works. On hardware it doesn't — this is one
  of the reasons to iterate in the sim first.
- **`RUST_BACKTRACE=1`** gives real backtraces on panics,
  unlike the hardware path where a panic halts and you have
  to read the breadcrumb at `0x3800_0300` under probe-rs
  (Vol II Chapter 2).

## Verify

```bash
make build-disco-sim
./target/debug/rlvgl-disco-sim
```

A 800×480 window appears with the disco UI — splash, desktop,
icon strip, wings. Tap the right-edge icons with the mouse.
Behavior should match what Vol I
[Chapter 5](../disco-tutorial/05-menu-stubs.md) produced on
hardware.

Then try the automation path:

```bash
./target/debug/rlvgl-disco-sim --automation-headless --playit-port=4567
# in another terminal:
nc localhost 4567
?<Enter>
```

`?` returns a tick/present summary. That's playit talking to
your binary; Chapter 3 expands.

## Going deeper

- [`examples/sim/README.md`](../../examples/sim/README.md)
  and
  [`examples/disco-sim/OPTIONS.md`](../../examples/disco-sim/OPTIONS.md)
  — binary flags and feature surface.
- [`examples/apps/disco-demo/README.md`](../../examples/apps/disco-demo/README.md)
  — the shared-controller pattern that makes sim + UEFI +
  hardware run the same UI code.
- [`docs/rendering/BACKEND-ARCHITECTURE.md`](../rendering/BACKEND-ARCHITECTURE.md)
  — how the simulator's wgpu blitter slots into the same
  `Blitter` trait the hardware DMA2D blitter does.
- [`docs/CUSTOM-SIMULATOR.md`](../CUSTOM-SIMULATOR.md) —
  building your own simulator binary around a different app
  crate.

---

**← Prev · [Index](README.md) · [Next →](02-uefi.md)**
