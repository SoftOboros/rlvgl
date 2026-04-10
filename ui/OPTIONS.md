<!--
OPTIONS.md - Cargo feature reference for the rlvgl-ui crate.
-->
# rlvgl-ui Options

`rlvgl-ui` provides higher-level UI building blocks on top of
`rlvgl-core` and `rlvgl-widgets`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `view` | Enables the experimental `view!` macro module. | `no_std`-friendly. | Negligible runtime impact; primarily affects ergonomics and a small amount of macro/module code size. |
