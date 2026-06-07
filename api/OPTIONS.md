<!--
OPTIONS.md - Cargo feature reference for the rlvgl-api crate.
-->
# rlvgl-api Options

`rlvgl-api` defines shared ABI-facing types for bindings and coprocessor-style
integrations. The crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std`.
- Important note: the current features are marker flags. They do not add
  dependencies or gate code paths in the current source tree.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `micropython` | Marker for MicroPython-facing builds. | `no_std`-friendly. | No direct impact today. |
| `cpython` | Marker for CPython-facing builds. | `no_std`-friendly at the crate level, though a real CPython integration would normally live in a host build. | No direct impact today. |
| `cm4` | Marker for Cortex-M4-facing ABI slices. | `no_std`-friendly. | No direct impact today. |
| `sim` | Marker for simulator-facing ABI slices. | `no_std`-friendly at the crate level. | No direct impact today. |
