<!--
OPTIONS.md - Cargo feature reference for the rlvgl-example-disco-sim crate.
-->
# rlvgl-example-disco-sim Options

`rlvgl-example-disco-sim` builds the `rlvgl-disco-sim` desktop binary.

## Default configuration

- Default features: none.
- Runtime model: host-only `std`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Automation and runtime behavior are controlled through CLI arguments such as
  `--screen`, `--headless`, `--automation-headless`, and `--playit-port`, not
  through Cargo features.
- The package always links the simulator backend and the `rlvgl-playit` TCP
  transport; there is no feature gate to trim that further today.
