# rlvgl-app-disco-demo

Shared STM32H747I-DISCO-style demo controller used by the simulator, UEFI
runtime, and board-specific adapters.

This crate is a `no_std` (with `alloc`) library — it has no binary targets
of its own.  It is consumed by:

- `examples/disco-sim/` (`rlvgl-disco-sim`) — host wgpu simulator
- `examples/uefi-disco/` (`rlvgl-uefi-disco`) — `aarch64-unknown-uefi`
- `examples/stm32h747i-disco/` (`rlvgl-stm32h747i-disco`) — Cortex-M7 firmware

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
