<!--
02-uefi.md - Volume III Chapter 2: UEFI/QEMU build, run, and constraints.
-->

**[← Prev](01-simulator.md) · [Index](README.md) · [Next →](03-playit.md)**

# Chapter 2 — UEFI under QEMU

## What this chapter covers

The third runtime: `rlvgl-uefi-disco`, a no_std UEFI
application that runs the same shared `rlvgl-app-disco-demo`
controller the simulator and hardware do, booted under QEMU
with EDK2 firmware. Useful when you want a platform-agnostic
no_std environment to catch portability regressions without
reflashing a board.

## The binary

[`examples/uefi-disco/`](../../examples/uefi-disco/) — package
`rlvgl-example-uefi-disco`, produces
`rlvgl-uefi-disco.efi` for `aarch64-unknown-uefi`.

```bash
rustup target add aarch64-unknown-uefi
make build-uefi-disco
```

Artifact path: `examples/uefi-disco/target/aarch64-unknown-uefi/debug/rlvgl-uefi-disco.efi`

## Constraints (that matter)

UEFI is a legitimate no_std target but differs from bare-metal
on four points the app cares about. The frontend adapter
handles each:

1. **Heap allocator** — UEFI provides one (`uefi` crate's
   `global_allocator`). The shared controller's
   `alloc::{Vec, String}` usage Just Works. No custom
   allocator like embedded-bare-metal requires.

2. **Framebuffer via GOP** — the Graphics Output Protocol
   hands you a linear framebuffer address + stride + pixel
   format at runtime. The app writes to it using a CPU
   blitter; there's no DMA2D equivalent. Performance is
   acceptable for UI-speed interactions but not for
   effects — the star crawl (Vol II Ch 9–10) is gated on
   DMA2D and does not run here.

3. **Input via ConIn** — UEFI text input only. The frontend
   synthesizes playit-style keyboard events; there is no
   touch. Arrow keys drive focus + wing navigation.

4. **Serial via PL011 MMIO** — playit output goes out a raw
   write to the PL011 UART at `0x0900_0000` (the virt machine
   baseline MMIO address). Input comes through ConIn, which
   **echoes bytes back by default** — so the playit suite
   marks the keyboard-input UEFI test as `todo` because raw
   frames round-trip corrupted.

5. **Workspace exclusion** — the UEFI crate is not part of
   the top-level cargo workspace because `aarch64-unknown-uefi`
   conflicts with the host build targets. Always build it
   with `--manifest-path examples/uefi-disco/Cargo.toml`
   (the `make` target already does this for you).

## Run under QEMU

The helper script
[`scripts/run-uefi-aarch64.sh`](../../scripts/run-uefi-aarch64.sh)
stages the `.efi` as `BOOTAA64.EFI` inside a FAT-formatted
ESP image and boots QEMU with the right `-machine virt -cpu
cortex-a72` flags:

```bash
./scripts/run-uefi-aarch64.sh
```

On macOS, `brew install qemu` pulls in the EDK2 firmware
(`edk2-aarch64-code.fd` + `edk2-arm-vars.fd`) under
`/opt/homebrew/share/qemu/`. The script auto-discovers them
and falls back to the standard Linux paths if not found. If
your firmware lives elsewhere, export `UEFI_CODE` and
`UEFI_VARS` to point at the files.

For automated playit testing (Ch 3) the script sibling
[`scripts/test-uefi-aarch64-playit.sh`](../../scripts/test-uefi-aarch64-playit.sh)
boots headless, exposes the PL011 UART over TCP, and runs the
Node.js suite.

## Gotchas

- **Multiple firmware copies** — if you installed QEMU via
  more than one package manager, the script may find a stale
  copy first. Force the path via `UEFI_CODE` / `UEFI_VARS`.
- **Workspace build errors** — `cargo build --workspace`
  will *not* include this crate. Use `make build-uefi-disco`
  or pass `--manifest-path` explicitly.
- **No DMA2D** — anything that cfg-gates on `dma2d` is inert
  here. The shared controller's `DiscoCapabilities::uefi()`
  preset (see
  [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
  L81–89) disables effects and pointer input accordingly.
- **No live screen** when running `test-uefi-disco` — the test
  script runs headless on purpose. For an interactive window
  pass `-display gtk` (or `-display cocoa` on macOS) to
  `run-uefi-aarch64.sh` via `QEMU_DISPLAY=gtk` env var.

## Verify

```bash
make build-uefi-disco
./scripts/run-uefi-aarch64.sh
```

A QEMU window opens, UEFI boot screens flash, and the disco UI
appears at 800×480 (or whatever GOP mode the firmware picked).
Arrow keys move focus along the icon strip; Enter opens a
wing; Escape closes it.

Headless / automated:

```bash
make test-uefi-disco
```

The Node.js suite runs; expect every test to pass except the
keyboard-input one, which is marked `todo` for the ConIn-echo
reason above.

## Going deeper

- [`examples/uefi-disco/README.md`](../../examples/uefi-disco/README.md)
  — UEFI-specific notes and known issues.
- [`examples/uefi-disco/OPTIONS.md`](../../examples/uefi-disco/OPTIONS.md)
  — crate options and features.
- [`platform/src/uefi.rs`](../../platform/src/uefi.rs) — the
  `rlvgl-platform` UEFI backend (GOP, timer, blitter glue).
- [`platform/src/uefi_serial_transport.rs`](../../platform/src/uefi_serial_transport.rs)
  — the hybrid PL011 TX / ConIn RX playit transport.
- UEFI Specification v2.x — GOP, ConIn, EFI_SIMPLE_TEXT_INPUT
  protocol definitions.

---

**[← Prev](01-simulator.md) · [Index](README.md) · [Next →](03-playit.md)**
