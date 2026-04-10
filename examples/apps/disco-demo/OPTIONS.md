<!--
OPTIONS.md - Cargo feature reference for the rlvgl-app-disco-demo crate.
-->
# rlvgl-app-disco-demo Options

`rlvgl-app-disco-demo` is the shared 747-style demo controller reused by the
desktop simulator, the UEFI target, and the STM32H747 firmware.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Host-specific behavior is selected by the runtime that embeds the controller,
  not by this crate's own feature set.
- Code size is driven by which host or firmware package links the controller
  and by the features enabled in those packages.
