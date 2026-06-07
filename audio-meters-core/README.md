# rlvgl-audio-meters-core

L0 ballistic state machines and dB calibration helpers for the rlvgl
audio-meters initiative. `no_std`, single-precision-float, no allocation.

This crate is the canonical implementation of the meter ballistics defined
in [`docs/audio-meters/00-concepts.md`](../docs/audio-meters/00-concepts.md)
§5. Both the rlvgl widget tree (Rust) and the TypeScript companion port
(`@rlvgl/audio-meters`) consume the same input fixture sequences (under
`fixtures/inputs/`) and MUST produce identical outputs to within a small
floating-point epsilon (`fixtures/expected/`). See `tests/parity.rs` for
the Rust side.

## Surface

- `BallisticState` — per-meter state. One per visible meter.
- `Ballistic` — frozen enum of supported ballistics (concepts §5).
- `apply_calibration(dbfs, offset_db)` — display-time additive offset for
  dBu / dBV / dBSPL conversion.

## Out of scope

- PCM acquisition, weighting filters, RMS/peak detection, true-peak
  oversampling, sample-rate conversion. These live upstream of the meter
  per concepts §9.
- Visual rendering. Widget code lives in `widgets/src/meters/` (rlvgl) and
  `@rlvgl/audio-meters-widgets` (TS).

## Cross-runtime parity

Fixtures under `fixtures/` are the contract between Rust and TS. The Rust
test suite generates expected outputs and compares against the committed
JSON; the TS port runs the same fixtures and checks against the same
expected files. Any divergence breaks one side's CI.
