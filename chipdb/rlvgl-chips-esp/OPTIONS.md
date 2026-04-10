<!--
OPTIONS.md - Cargo feature reference for the rlvgl-chips-esp crate.
-->
# rlvgl-chips-esp Options

`rlvgl-chips-esp` packages Espressif board metadata for creator and
board-selection workflows.

## Default configuration

- Default features: none.
- Runtime model: `no_std`.

## Feature flags

This crate does not currently define any Cargo feature flags.

## Useful notes

- Set `RLVGL_CHIP_SRC` when you want the build script to package fresh vendor
  data from a workspace checkout.
- If `RLVGL_CHIP_SRC` is unset, the crate still builds and keeps its baked-in
  board catalog behavior.
