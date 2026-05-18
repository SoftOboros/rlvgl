# BeagleBone Black + NHD-7.0CTP-CAPE-P

rlvgl demo for the BeagleBone Black with the Newhaven 7" capacitive touch
display cape. Runs under Linux (kernel `tilcdc` + evdev touch) and is being
brought up under bare-metal, FreeRTOS, and Zephyr.

## Quick start

The Linux path boots off an SD card (eMMC cannot coexist with the cape's
LCD pin mux). Ready-made tooling lives in `tools/`:

```bash
# One-time SD card prep on macOS (requires a Bookworm image)
bash tools/prepare-sd.sh
# Optional: install the fbcon-unbind systemd unit into the SD
bash tools/install-fbcon-unbind-sd.sh
```

Build + deploy the Linux demo:

```bash
cargo build --release --target armv7-unknown-linux-gnueabihf
scp target/armv7-unknown-linux-gnueabihf/release/rlvgl-example-bbb \
    debian@<bbb-ip>:/home/debian/rlvgl-bin/
ssh debian@<bbb-ip> sudo /home/debian/rlvgl-bin/rlvgl-example-bbb
```

## Prong layout

| Prong      | Directory          | Status |
|------------|--------------------|--------|
| Linux      | `src/` (default)   | Working: splash + desktop + EDMA blitter + playit |
| Bare-metal | `src/bin/bare.rs`  | Boots at `0x82000000`, panel lit. Touch blocked on hardware RMA. |
| FreeRTOS   | `freertos/`        | Scaffolding |
| Zephyr     | `zephyr/`          | Scaffolding |

## Local docs

- [`RMA-newhaven-2026-04-22.md`](./RMA-newhaven-2026-04-22.md) — touch panel RMA record.

## Deep references

- [`docs/beaglebone-black/`](../../docs/beaglebone-black/) — full
  multi-prong guide: DT overlay strategy, SD prep, bare-metal bring-up,
  FreeRTOS/Zephyr status, phase-by-phase acceptance.
