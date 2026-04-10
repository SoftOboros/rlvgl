<!--
OPTIONS.md - Cargo feature reference for the rlvgl-core crate.
-->
# rlvgl-core Options

`rlvgl-core` provides the widget tree, renderer traits, styling, plugins, and
other foundational runtime pieces. The crate is `no_std` by default.

## Default configuration

- Default features: none.
- Runtime model: `no_std` with `alloc`.
- Important caveat: several media-oriented features are currently wired through
  dependencies that are only declared on host targets, so not every feature is
  suitable for bare-metal builds even though the base crate is.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `png` | Enables PNG decoding support. | Current dependency is declared for non-`target_os = "none"` builds. | Adds decoder code and asset decode cost. |
| `jpeg` | Enables JPEG decoding support. | Same host-oriented dependency constraint as `png`. | Similar code-size increase; JPEG decode is CPU-heavy. |
| `gif` | Enables GIF decoding support. | `rlvgl-core` pulls in `std` when this feature is enabled today. | Moderate-to-high code-size increase, especially for animated assets. |
| `qrcode` | Enables QR code generation helpers. | Current dependency is host-oriented in the manifest. | Small-to-moderate code-size increase; runtime cost only on generation. |
| `fontdue` | Enables Fontdue-backed font loading and caching helpers. | Intended for richer host builds in the current manifest layout. | Improves font flexibility but increases compile time and memory use. |
| `lottie` | Enables the Lottie-facing API surface. | API-level flag; does not enable a backend by itself. | Minimal overhead by itself. |
| `lottie_backend` | Enables the rlottie runtime backend. | Host-only in practice; backend dep is only declared on Linux and Android. | Moderate code-size and runtime cost when vector animations are rendered. |
| `canvas` | Enables embedded-canvas and embedded-graphics integration. | Can stay `no_std`. | Moderate code-size increase for off-screen drawing helpers. |
| `pinyin` | Enables pinyin-related helpers. | Current source enables `std` when this flag is active. | Small code-size increase. |
| `fatfs` | Enables FAT filesystem helpers. | Current source enables `std` for this feature. | Adds storage code paths and I/O overhead when used. |
| `nes` | Enables NES integration helpers. | Current source enables `std` for this feature. | Large binary and CPU cost compared with the base crate. |
| `apng` | Enables APNG decoding support. | Current source enables `std` for this feature. | Moderate-to-high code-size increase. |
| `fs` | Exposes the filesystem abstraction module. | `no_std`-friendly. | Small API-only increase unless paired with a backend. |
