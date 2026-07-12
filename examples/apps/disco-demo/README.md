# rlvgl-app-disco-demo

Shared STM32H747I-DISCO-style demo controller used by the simulator, UEFI
runtime, and board-specific adapters.

This crate is a `no_std` (with `alloc`) library — it has no binary targets
of its own.  It is consumed by:

- `examples/disco-sim/` (`rlvgl-disco-sim`) — host wgpu simulator
- `examples/uefi-disco/` (`rlvgl-uefi-disco`) — `aarch64-unknown-uefi`
- `examples/stm32h747i-disco/` (`rlvgl-stm32h747i-disco`) — Cortex-M7 firmware

## Regenerating the Qt Media Player

`src/media_player_gen.rs` is generated from the Bolero `FrameMedia.qml`, then
receives one deterministic embedded integration overlay. The overlay is
required for two consumer contracts:

- this crate owns `media_player` as a local module, so the generated machine
  import becomes `crate::media_player::Machine`;
- STM32H747I-DISCO has a 64 KiB Rust heap, so the overlay replaces the default
  leaked-pixel image backend with a no-heap, row-streamed `RleImage` backend.

Do not edit the generated source directly. Regenerate or check it with:

```bash
./scripts/regenerate_disco_media_player.sh --write
./scripts/regenerate_disco_media_player.sh --check
```

The script runs the canonical Qt emitter and applies
`codegen/media_player_gen.patch` with zero fuzz. An emitter-shape change is
expected to fail patch application until the overlay is deliberately reviewed
and refreshed from the new direct output.

## Tests

Unit tests cover navigation, focus management, hotkeys, command emission,
and the focus highlight wiring:

```bash
make test-disco-demo
```

Equivalently:

```bash
cargo test -p rlvgl-app-disco-demo
```

See [`OPTIONS.md`](./OPTIONS.md) for the (currently empty) feature reference.
