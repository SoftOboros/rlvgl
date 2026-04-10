<!--
OPTIONS.md - Cargo feature reference for the rlvgl-example-sim crate.
-->
# rlvgl-example-sim Options

`rlvgl-example-sim` is the desktop demo binary package that builds the
`rlvgl-sim` executable.

## Default configuration

- Default features: `png`, `jpeg`, `gif`, `qrcode`, `fontdue`.
- Runtime model: host-only `std`.
- Useful workflow: use `--no-default-features` when you want the smallest
  possible simulator binary, then re-enable only the codecs you actually need.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `png` | Enables PNG demo support. | Host-oriented through `rlvgl-core`. | Moderate decoder overhead. |
| `jpeg` | Enables JPEG demo support. | Host-oriented through `rlvgl-core`. | Moderate decoder overhead. |
| `gif` | Enables GIF demo support. | Pulls in the heavier animated-image path. | Moderate-to-high code-size increase. |
| `qrcode` | Enables the QR-code demo panel. | Host-oriented through `rlvgl-core`. | Small-to-moderate increase. |
| `fontdue` | Enables Fontdue-backed text rendering helpers in the demo. | Host-oriented in current practice. | Moderate compile-time and code-size increase. |
| `cpu_stats` | Prints simple periodic CPU-side render timing diagnostics. | Host-only. | Minimal code-size increase; small runtime overhead from timing and logging. |
