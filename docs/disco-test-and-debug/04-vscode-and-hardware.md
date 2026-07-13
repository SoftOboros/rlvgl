<!--
04-vscode-and-hardware.md - Volume III Chapter 4: VS Code + probe-rs + GDB on hardware.
-->

**[← Prev](03-playit.md) · [Index](README.md) · Next →**

# Chapter 4 — VS Code, probe-rs & GDB on Hardware

## What this chapter covers

The hardware debug workflow:

- The checked-in `.vscode/launch.json` configurations and
  what each one does.
- One-click build / flash / debug via probe-rs.
- GDB over probe-rs for commands VS Code's debugger doesn't
  expose.
- SVD-backed register views so you can inspect peripherals
  Vol II Chapters 3–8 wrote by hand.

## What's checked in

Three folders at the repo root drive this chapter:

| Path | Role |
|------|------|
| [`.vscode/launch.json`](../../.vscode/launch.json) | Five debug configurations — three hardware, two host. |
| [`.vscode/tasks.json`](../../.vscode/tasks.json) | Six `make`-backed build tasks wired as `preLaunchTask` hooks. |
| `.svd/STM32H747_CM7.svd` | Peripheral register descriptions — powers VS Code's register panel during a debug session. This is a local/generated support file rather than a checked-in repository artifact. |

## The launch configurations

Three are for hardware (probe-rs):

### "CM7 (probe-rs)" — build + flash + halt at reset

The default. Runs the `build-disco (cm7)` task first, flashes
the debug ELF, resets, and halts. This is the one you'll use
99% of the time during development.

```json
{
  "name": "CM7 (probe-rs)",
  "type": "probe-rs-debug",
  "request": "launch",
  "chip": "STM32H747XIHx",
  "probe": "0483:3754:004F00273133510837363734",
  "connectUnderReset": true,
  "coreConfigs": [ {
    "programBinary": "${workspaceFolder}/target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco",
    "svdFile":       "${workspaceFolder}/.svd/STM32H747_CM7.svd",
    "flashingConfig": { "flashingEnabled": true, "haltAfterReset": true }
  } ],
  "preLaunchTask": "build-disco (cm7)"
}
```

### "CM7 attach (probe-rs)" — attach without reflash

For cases where the board is already running (flashed via
CLI, CubeProgrammer, another session) and you just want
symbols + breakpoints. `flashingEnabled: false`,
`connectUnderReset: false`.

### "CM7 release (probe-rs)" — release build variant

Same as the first config but builds `build-disco-release` and
loads the release ELF at `target/thumbv7em-none-eabihf/release/…`.
Use when release-only timing bugs show up.

Two more are for host binaries:

### "Host: rlvgl-sim" / "Host: rlvgl-creator" — LLDB

Plain host binaries; VS Code uses the LLDB extension. The
sim config covers Chapter 1's debugging-the-simulator case.
Duplicate and re-point `program` if you want to debug
`rlvgl-disco-sim` instead — the file is a starting point, not
a complete set.

## One-time setup: the probe ID

The three CM7 configs hardcode a specific ST-Link probe:

```
"probe": "0483:3754:004F00273133510837363734"
```

That's one developer's probe serial. On first run, either:

- **Remove the `probe` line** to let probe-rs pick the only
  attached probe (works if you only have one), or
- **Replace the serial** with your probe's. Run
  `probe-rs list` to print it.

Keep this change local — don't commit the rewrite unless you
intend to swap the default for everyone.

## The tasks

Every `preLaunchTask` in `launch.json` maps to a `make` invocation
in [`tasks.json`](../../.vscode/tasks.json):

| Task label | Command |
|-----------|---------|
| `build-disco (cm7)` | `make build-disco` |
| `build-disco (cm7, release)` | `make build-disco-release` |
| `build-disco (cm4)` | `make build-disco-cm4` |
| `flash-disco (cm7)` | `make flash-disco` (depends on `build-disco (cm7)`) |
| `build-sim` | `cargo build -p rlvgl-example-sim --bin rlvgl-sim` |
| `build-creator` | `cargo build --bin rlvgl-creator --features creator` |

Everything the guide's previous chapters called from the
command line is available inside VS Code through
Terminal → Run Task…

## A typical debug session

1. Plug the board in. Confirm the USB VCP enumerates
   (`ls /dev/cu.usbmodem*`).
2. Open VS Code, set a breakpoint in the renderer (e.g.
   the desktop refresh in Vol II Ch 5 or the crawl state
   machine in Vol II Ch 10).
3. Hit **F5** with "CM7 (probe-rs)" selected. VS Code runs
   `make build-disco`, flashes, resets, and halts at
   `main()`.
4. **Step / continue / break** as usual.
5. **Cortex-M** and **peripheral register views** appear in
   the left pane — the SVD file drives them. Expand DMA2D,
   LTDC, FMC, RCC to watch registers change. The Vol II
   register-diagram bits become live values.
6. The **Variables** pane shows stack locals and atomics —
   useful for the `ERIF_FLAG`, `COMPLETE_LATCH`, and
   `FRAME_BUDGET_CYCLES` Vol II chapters 5 and 7 referenced.

## GDB — when the VS Code debugger isn't enough

For commands VS Code doesn't expose (`examine`, dumping large
regions of SDRAM, scripted breakpoints), use GDB directly:

```bash
make probe-rs-gdb
```

This flashes the firmware (depends on `flash-disco`), then
starts a probe-rs GDB server on port 3333. In a second
terminal:

```bash
arm-none-eabi-gdb -q \
  target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco \
  -ex "target extended-remote :3333" \
  -ex "source scripts/gdb_init_h7.gdb"
```

[`scripts/gdb_init_h7.gdb`](../../scripts/gdb_init_h7.gdb)
loads a handful of H7-specific helpers (SVD-derived pretty-
printers, memory aliases for the breadcrumb region etc.).

If probe-rs GDB gives you trouble and you need OpenOCD
instead, [`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
documents the `make openocd` and `make openocd-dual` paths
(the latter brings up both cores on ports 3333/3334).

## Common hardware debug tasks

| Task | How |
|------|-----|
| Read a breadcrumb (Vol II Ch 2) | VS Code Memory view at `0x3800_0300`, or GDB `x/4xw 0x38000300`. |
| Watch `ERIF_FLAG` flip | Variables pane → expand the `ERIF_FLAG` atomic. |
| Check DMA2D completion count | VS Code peripheral view → DMA2D → ISR, or GDB `x/w 0x52001004`. |
| Dump SDRAM framebuffer | GDB `dump binary memory /tmp/fb.bin 0xD0300000 0xD02ffff` then `xxd`. |
| Inspect the touch ring | Variables pane → `TOUCH_RING.head` / `.tail` / `.slots`. |

## Troubleshooting

- **"probe not found"** — the hardcoded serial in
  `launch.json` doesn't match your probe. See §"the probe
  ID" above.
- **"flashing failed, core not halted"** — the board got
  into a bad state. Hold the Reset button, start the debug
  session, release on connect.
- **Stuck under reset** — walk the recovery checklist in
  [`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
  (slow SWD speed, connect-under-reset, clear flash, etc.).
- **Symbols missing in GDB** — you built release but loaded
  the debug ELF's symbols, or vice versa. Match the ELF
  passed to `arm-none-eabi-gdb` to the one probe-rs flashed.

## Going deeper

- [`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
  — the full hardware debug playbook, including recovery
  and dual-core coordination.
- [`CLAUDE.md`](../../CLAUDE.md) §Flashing and Debug — the
  canonical source of build/flash commands every chapter of
  this guide reuses.
- [probe-rs documentation](https://probe.rs) — the debug
  tool the VS Code configs use.
- [Cortex-Debug for VS Code](https://marketplace.visualstudio.com/items?itemName=marus25.cortex-debug)
  — alternative plugin if you prefer OpenOCD-based workflows.

## End of Volume III

You now have:

- A running simulator for fast UI iteration (Ch 1).
- A UEFI/QEMU path for no_std portability checks (Ch 2).
- Playit driving the same test suite against all three
  runtimes (Ch 3).
- VS Code + probe-rs + GDB for hardware-level debugging
  with SVD-backed register views (Ch 4).

Together with Vol I (building the app) and Vol II (the
platform bring-up under the helpers), you have a complete
development loop for the disco demo and any rlvgl app
that reuses the same adapter pattern.

---

**[← Prev](03-playit.md) · [Index](README.md) · Next →**
