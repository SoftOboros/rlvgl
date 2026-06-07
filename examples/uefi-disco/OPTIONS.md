<!--
OPTIONS.md - Cargo feature reference for the rlvgl-example-uefi-disco crate.
-->
# rlvgl-example-uefi-disco Options

`rlvgl-example-uefi-disco` builds the `rlvgl-uefi-disco` binary for a UEFI
runtime.

## Default configuration

- Default features: none.
- Runtime model: `no_std` UEFI application.
- Required target: `aarch64-unknown-uefi`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- UEFI support comes from the dependency graph, not from local Cargo features:
  the package always enables `rlvgl-platform` with its `uefi` feature.
- Because the crate is excluded from the workspace, build it with
  `--manifest-path examples/uefi-disco/Cargo.toml`.
