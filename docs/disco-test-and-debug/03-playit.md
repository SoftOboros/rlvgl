<!--
03-playit.md - Volume III Chapter 3: playit automation across all three runtimes.
-->

**[← Prev](02-uefi.md) · [Index](README.md) · [Next →](04-vscode-and-hardware.md)**

# Chapter 3 — Playit Automation

## What this chapter covers

`playit` is the transport-agnostic text-command protocol that
drives all three runtimes — the simulator, UEFI, and the
STM32H747I-DISCO board — with the same test suite. This
chapter covers how it attaches to each runtime, the test
targets, and how to write a new test.

The wire-protocol reference already exists — this chapter
**does not duplicate it**. Every command you'll see here
(`?`, `T<x>,<y>`, `QB:<tag>`, etc.) is documented in
[`playit/README.md`](../../playit/README.md) §Wire protocol.

## The picture

```
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│ Node.js tests │    │ Node.js tests │    │ Node.js tests │
└───────┬───────┘    └───────┬───────┘    └───────┬───────┘
        │ TCP               │ TCP (bridge)      │ TCP (bridge)
┌───────▼───────┐    ┌───────▼───────┐    ┌───────▼───────┐
│ rlvgl-disco-  │    │ QEMU stdio /  │    │ serial_tcp_   │
│ sim (TCP)     │    │ PL011 MMIO    │    │ bridge.py     │
└───────────────┘    └───────┬───────┘    └───────┬───────┘
                             │                    │ USB VCP
                      ┌──────▼──────┐      ┌──────▼──────┐
                      │ rlvgl-uefi- │      │ STM32H747I- │
                      │ disco.efi   │      │ DISCO (UART1)│
                      └─────────────┘      └─────────────┘
```

The Node.js test suite (`playit/node/test/`) is the single
source of truth for cross-runtime tests. Each runtime has a
small adapter script that exposes playit over TCP, and the
tests don't care which runtime is behind the socket.

## Per-runtime wiring

### Simulator — direct TCP

`rlvgl-disco-sim` binds a playit server directly. Default
behavior is "pick a free port and print it"; specify
`--playit-port=N` to fix it. See
[Chapter 1](01-simulator.md).

```bash
make test-disco-sim
```

runs `scripts/compile-disco.sh` (compiles the binary), the
Rust-side unit tests, and the Node.js suite against the
running sim.

### UEFI — PL011 + ConIn, bridged

`scripts/test-uefi-aarch64-playit.sh` boots QEMU with the
PL011 UART attached to a socket and runs the Node.js suite
against that socket:

```bash
make test-uefi-disco
```

The keyboard-input test is marked `todo` because UEFI's ConIn
echoes bytes back, corrupting raw frames (Chapter 2). All
other tests pass.

### Hardware — ST-Link VCP, bridged to TCP

`scripts/serial_tcp_bridge.py` opens `/dev/cu.usbmodem*` (or
the path in `$SERIAL_PORT`) and forwards bytes to/from a TCP
socket. The Node.js suite connects to the socket and runs:

```bash
make test-stm32h747i-disco
```

Under the hood this uses
[`scripts/test-stm32h747i-disco-playit.sh`](../../scripts/test-stm32h747i-disco-playit.sh).
The firmware must already be flashed — the test target
does not reflash; run `make flash-disco` first if you've
changed the firmware.

**macOS gotcha**: multiple ST-Link probes connected at once
produce multiple `/dev/cu.usbmodem*` devices and the bridge
picks the first alphabetically. If you have more than one
board plugged in, set `SERIAL_PORT=/dev/cu.usbmodem…` to the
exact device before running.

### All three in sequence

```bash
make test-playit-all
```

Runs `test-disco-sim`, `test-uefi-disco`, then
`test-stm32h747i-disco` in order. Stops at the first failing
suite.

## The Node.js test suite

Lives at [`playit/node/`](../../playit/node/):

| File | Scope |
|------|-------|
| `test/shared-assertions.js` | Common assertion helpers (`waitForWidget`, `captureFrame`, tap-and-verify). |
| `test/disco-navigation.test.js` | Cross-runtime navigation tests — same test, every runtime. |
| `test/disco-sim.test.js` | Host-simulator-specific (things that require the GUI or host clock). |
| `test/uefi-disco.test.js` | UEFI-specific (firmware boot checks, PL011 framing). |
| `test/stm32h747i-disco.test.js` | Hardware-specific (touch interrupts, real-time timing assertions). |

Run one suite directly:

```bash
cd playit/node
RLVGL_DISCO_SIM_BIN="$PWD/../../target/debug/rlvgl-disco-sim" \
  node --test test/disco-sim.test.js
```

## Writing a new test

1. Pick the scope. Feature that should work on every runtime
   → add to `disco-navigation.test.js`. Runtime-specific →
   add to that runtime's file.
2. Use the helpers in `shared-assertions.js` — they speak the
   same wire protocol `playit/README.md` documents. Prefer
   tag-based queries (`QB:settings-icon`) over raw pixel
   coordinates so the test survives layout changes.
3. Run it against the host simulator first; that cycles the
   fastest. Then run it against the other runtimes
   (`make test-playit-all`) before committing.

## Verify the tooling works

```bash
make test-disco-demo    # unit tests on the shared controller, no playit
make test-disco-sim     # simulator + Node.js — should pass end-to-end
```

If `test-disco-sim` passes but a runtime-specific suite fails,
the failure is almost always in the adapter (Ch 1 / Ch 2 / Ch
4), not the tests themselves.

## Going deeper

- [`playit/README.md`](../../playit/README.md) — the wire
  protocol. You will refer to this often.
- [`docs/TEST-STRATEGY.md`](../TEST-STRATEGY.md) — the
  design intent behind playit and the cross-runtime test
  philosophy.
- [`playit/src/`](../../playit/src/) — Rust implementation of
  the protocol, `no_std` by default.
- [`scripts/serial_tcp_bridge.py`](../../scripts/serial_tcp_bridge.py)
  and [`scripts/test-stm32h747i-disco-playit.sh`](../../scripts/test-stm32h747i-disco-playit.sh)
  — the hardware bridge glue.

---

**[← Prev](02-uefi.md) · [Index](README.md) · [Next →](04-vscode-and-hardware.md)**
