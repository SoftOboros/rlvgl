<!--
OPTIONS.md - Cargo feature reference for the rlvgl-app-demo crate.
-->
# rlvgl-app-demo Options

`rlvgl-app-demo` is the reusable demo application that powers `rlvgl-sim` and
other demo-oriented hosts. The base crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`; some media features are effectively
  host-oriented because the underlying decoders live in `rlvgl-core`.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `dylib` | Marker for dynamic-loading builds. The manifest comments point to this for `cdylib` workflows. | The current source does not gate code on this flag, so treat it as a build-intent marker rather than a guaranteed packaging switch. | No direct runtime impact in the current manifest. |
| `png` | Enables the PNG demo path. | Mirrors `rlvgl-core/png`, which is currently host-oriented. | Adds decoder code and decode-time cost. |
| `jpeg` | Enables the JPEG demo path. | Mirrors `rlvgl-core/jpeg`, which is currently host-oriented. | Similar cost profile to `png`. |
| `gif` | Enables the GIF demo path. | Mirrors `rlvgl-core/gif`, which currently pulls in `std`. | Moderate-to-high code-size increase. |
| `qrcode` | Enables the QR-code demo widget path. | Mirrors `rlvgl-core/qrcode`, which is currently host-oriented. | Small-to-moderate increase. |
| `fontdue` | Enables the Fontdue-backed text demo path. | Mirrors `rlvgl-core/fontdue`. | Moderate compile-time and code-size increase. |
