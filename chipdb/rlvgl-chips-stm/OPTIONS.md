<!--
OPTIONS.md - Cargo feature reference for the rlvgl-chips-stm crate.
-->
# rlvgl-chips-stm Options

`rlvgl-chips-stm` packages STM32 board and chip metadata for creator and BSP
generation workflows.

## Default configuration

- Default features: none.
- Runtime model: `no_std`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Published builds use the packaged `assets/chipdb.bin.zst` archive.
- Workspace builds can fall back to `RLVGL_CHIP_SRC` when you want to package
  fresh uncompressed vendor JSON data during the build.
- Artifact size is dominated by the embedded database content, not by feature
  selection.
