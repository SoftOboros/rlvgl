<!--
examples/uefi-disco/README.md - AArch64 UEFI build of the shared disco demo.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl UEFI Disco Demo
---

`rlvgl-uefi-disco` runs the shared `rlvgl-app-disco-demo` controller as a
no_std UEFI application targeting `aarch64-unknown-uefi`.  It uses the GOP
graphics protocol for display, the simple-text-input protocol for keyboard,
and a hybrid playit transport (raw PL011 MMIO TX + ConIn RX) so the same
playit test suite drives the UEFI target.

> **Workspace exclusion:** This crate is intentionally excluded from the
> root workspace because the `aarch64-unknown-uefi` target conflicts with
> the host `std` build of the rest of the workspace.  Always build it with
> `--manifest-path examples/uefi-disco/Cargo.toml`.

## Prerequisites

- `aarch64-unknown-uefi` Rust target: `rustup target add aarch64-unknown-uefi`
- `qemu-system-aarch64` for running the binary
- EDK2 AArch64 firmware (`edk2-aarch64-code.fd` + `edk2-arm-vars.fd`).
  On macOS Homebrew these install with `brew install qemu` and live under
  `/opt/homebrew/share/qemu/`.

## Build

| Method | Command |
| --- | --- |
| Make | `make build-uefi-disco` |
| Cargo | `cargo build --manifest-path examples/uefi-disco/Cargo.toml --target aarch64-unknown-uefi --bin rlvgl-uefi-disco` |

The resulting binary lands at
`examples/uefi-disco/target/aarch64-unknown-uefi/debug/rlvgl-uefi-disco.efi`.

## Run interactively

```bash
bash scripts/run-uefi-aarch64.sh
```

The script auto-discovers the EDK2 firmware (override with `UEFI_CODE` and
`UEFI_VARS` env vars), builds the binary, stages it as `BOOTAA64.EFI` in a
FAT ESP image, and launches QEMU with `-display default` and a `-serial`
chardev for playit on `PLAYIT_PORT` (default 4567).

## Run the playit test suite

```bash
make test-uefi-disco
```

That target invokes `scripts/test-uefi-aarch64-playit.sh`, which boots a
headless QEMU instance and runs `playit/node/test/uefi-disco.test.js`
against the running firmware.  Three of the four tests currently pass; the
keyboard navigation test is marked `todo` because EDK2's ConIn echoes
characters back to ConOut, corrupting playit responses (see the test file
for the full note).

See [`OPTIONS.md`](./OPTIONS.md) for the (currently empty) feature reference.
