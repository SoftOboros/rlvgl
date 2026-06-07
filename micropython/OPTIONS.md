<!--
OPTIONS.md - Cargo feature reference for the rlvgl-micropython crate.
-->
# rlvgl-micropython Options

`rlvgl-micropython` provides the current MicroPython binding surface on top of
`rlvgl-api`. The crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std`.
- Important note: the current feature set is marker-only.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `stm32h747i_disco` | Marker for board-specific MicroPython integration on STM32H747I-DISCO. | `no_std`-friendly. | No direct impact in the current implementation. |
