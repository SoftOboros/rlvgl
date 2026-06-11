<!--
README.md - Volume III index. How to test and debug the disco app across
the simulator, UEFI, and hardware, plus the playit automation protocol
and VS Code debug workflow. Linked from the repo root README.
-->

# Testing & Debugging the Disco Demo — Volume III

**Volume III** is the short, practical follow-up to
[Volume I](../disco-tutorial/README.md). Once you have the disco
app built (Vol I) you still need to run it somewhere and debug it
when something is wrong. This guide covers the three runtimes
rlvgl supports and the tooling that sits on top of them.

- Host **simulator** for fast UI iteration without hardware.
- **UEFI / QEMU** for platform-agnostic no_std testing in a
  virtual environment.
- **STM32H747I-DISCO hardware** for the real thing.
- **Playit** — the text-command automation protocol that drives
  all three using the same test suite.
- **VS Code + probe-rs + GDB** for line-by-line debugging on
  hardware.

> **Status:** Closed (4/4 chapters; tooling ships in v0.2.0). No
> `DISCO-TEST-AND-DEBUG-RETROSPECTIVE.md` was authored — this
> initiative completed before the retrospective discipline was added
> to CLAUDE.md (first reference implementation
> `docs/concepts/DCB-RETROSPECTIVE.md`, 2026-05-03). Lessons-learned
> material is embedded in the per-chapter narratives.

## Prerequisites

- Vol I completed — you have a build that flashes and runs on
  at least one runtime.
- Rust toolchain + the targets you plan to test:
  - Host sim: default target only.
  - UEFI: `rustup target add aarch64-unknown-uefi`.
  - Hardware: `rustup target add thumbv7em-none-eabihf`.
- `probe-rs` for hardware flash + debug.
- `qemu-system-aarch64` for UEFI (on macOS:
  `brew install qemu` installs QEMU + the EDK2 firmware
  scripts expect).
- Node.js 20+ for the playit test suites.

All install details live in
[`docs/EMBEDDED-TOOLING.md`](../EMBEDDED-TOOLING.md) and
[`CLAUDE.md`](../../CLAUDE.md) §Pre-Publish Validation.

## Chapters

| Ch | Title | What it covers |
|----|-------|----------------|
| [1](01-simulator.md) | Host simulator | Generic `rlvgl-sim` and disco-specific `rlvgl-disco-sim`; how to link your own app through the shared `rlvgl-app-disco-demo` crate. |
| [2](02-uefi.md) | UEFI under QEMU | Building `rlvgl-uefi-disco` for `aarch64-unknown-uefi`; boot flow, GOP/ConIn/PL011 constraints, EDK2 firmware discovery. |
| [3](03-playit.md) | Playit automation | The wire protocol, how it attaches to TCP (sim), PL011+ConIn (UEFI), and ST-Link VCP (hardware); the Node.js test suite. |
| [4](04-vscode-and-hardware.md) | VS Code + probe-rs + GDB | The checked-in `.vscode/launch.json` configurations, one-click build/flash/debug, GDB server via `make probe-rs-gdb`, SVD-backed register views. |

Triaged defects for these surfaces live in the family
[errata log](ERRATA.md) (GH Issues is the intake queue).

## Quick reference: runtime × action

The full command grid; later chapters explain each row.

| | Simulator | UEFI / QEMU | Hardware |
|---|-----------|-------------|----------|
| **Build** | `make build-sim` / `make build-disco-sim` | `make build-uefi-disco` | `make build-disco` |
| **Run** | `./target/debug/rlvgl-sim` | `scripts/run-uefi-aarch64.sh` (see Ch 2) | `make flash-disco` |
| **Playit test** | `make test-disco-sim` | `make test-uefi-disco` | `make test-stm32h747i-disco` |
| **VS Code debug** | "Host: rlvgl-sim" (LLDB) | *(not supported)* | "CM7 (probe-rs)" |
| **GDB** | `rust-gdb ./target/debug/rlvgl-sim` | *(not supported)* | `make probe-rs-gdb` + arm-none-eabi-gdb |

`make test-playit-all` runs all three test suites in sequence.

## Going deeper (guide-wide)

- [`playit/README.md`](../../playit/README.md) — full wire
  protocol reference for every command this guide uses.
- [`examples/stm32h747i-disco/DEBUG-SETUP.md`](../../examples/stm32h747i-disco/DEBUG-SETUP.md)
  — hardware debug recovery playbook for when SWD misbehaves.
- [`docs/MAKE.md`](../MAKE.md) — every `make` target with its
  arguments and dependencies.
- [`docs/TEST-STRATEGY.md`](../TEST-STRATEGY.md) — the testing
  architecture behind the playit automation.
- [`CLAUDE.md`](../../CLAUDE.md) §Flashing and Debug — canonical
  flash/debug commands used throughout this guide.

---

**[← Vol I Index](../disco-tutorial/README.md)** · **[← Vol II Index](../disco-platform-guide/README.md)** · **Next →** [Chapter 1 — Host simulator](01-simulator.md)
