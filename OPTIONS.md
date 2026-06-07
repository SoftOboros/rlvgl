<!--
OPTIONS.md - Cargo feature reference for the top-level rlvgl crate.
-->
# rlvgl Options

`rlvgl` is the umbrella crate that re-exports `rlvgl-core`, `rlvgl-platform`,
`rlvgl-widgets`, `rlvgl-ui`, and `rlvgl-i18n`.

## Default configuration

- Default features: none.
- Runtime model: library use is `no_std` by default; tooling features are
  intentionally host-oriented.
- Binary note: `rlvgl-creator` only builds when `creator` is enabled. Adding
  `creator_ui` keeps the CLI available but also enables the desktop UI.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `regression` | Enables extra regression-only creator tests. | Test-only. | No production runtime impact. |
| `playit` | Re-exports the `rlvgl-playit` automation driver. | Works in `no_std`; `std` remains optional in `rlvgl-playit` itself. | Small code-size increase unless you actively use the APIs. |
| `png` | Enables PNG decoding support through `rlvgl-core` and `rlvgl-platform`. | Practical use is host-oriented because current PNG decoder deps are only declared on non-`target_os = "none"` targets. | Increases compile time and binary size; runtime cost appears only when decoding PNG assets. |
| `jpeg` | Enables JPEG decoding support. | Same host-oriented constraint as `png`. | Similar tradeoff to `png`; JPEG decode paths are CPU-heavy compared with raw assets. |
| `gif` | Enables GIF decoding support. | Pulls in `std` in `rlvgl-core` today. | Noticeable code-size and decode-time increase when animated assets are used. |
| `qrcode` | Enables QR code generation helpers. | Current dependency is host-oriented in `rlvgl-core`. | Small-to-moderate code-size increase; runtime cost only when generating codes. |
| `simulator` | Enables host simulator support and pulls in the demo app crates. | Host-only; relies on desktop windowing and graphics crates. | Largest single compile-time increase on the top-level crate. |
| `fontdue` | Enables Fontdue-backed text rendering helpers. | Intended mainly for host or richer builds; current support deps are not configured for bare `target_os = "none"` builds. | Adds text layout flexibility at the cost of extra code size and font processing work. |
| `lottie` | Enables Lottie-facing APIs. | Can stay `no_std` at the API layer. | Minimal by itself; pair with `lottie_runtime` for an actual host runtime backend. |
| `lottie_runtime` | Enables the runtime Lottie backend in addition to `lottie`. | Host-oriented; depends on the backend support exposed by `rlvgl-core`. | Moderate compile and binary-size increase. |
| `canvas` | Enables embedded-canvas based drawing helpers. | `no_std`-friendly if the downstream target supports the enabled graphics stack. | Adds rendering flexibility with a moderate code-size increase. |
| `pinyin` | Enables pinyin-related helpers. | `rlvgl-core` currently pulls in `std` for this feature. | Small code-size increase unless heavily used. |
| `fatfs` | Enables FAT filesystem integration. | Use on targets with `alloc`; the core implementation currently expects `std` on host-side builds and platform-specific support elsewhere. | Storage support adds code size and I/O paths; runtime cost is workload-dependent. |
| `nes` | Enables the NES integration hooks. | `rlvgl-core` currently treats this as a `std`-using feature. | Substantial code-size and CPU cost when the emulator path is exercised. |
| `apng` | Enables APNG decoding helpers. | Currently host-oriented because it depends on the `image` crate. | Moderate-to-high code-size increase. |
| `fs` | Exposes the core filesystem abstraction layer. | `no_std`-friendly. | Small API-only increase unless a filesystem backend is actually used. |
| `creator` | Enables the `rlvgl-creator` CLI and creator pipeline dependencies. | Host-only; the binary itself requires this feature. | Large compile-time and dependency-footprint increase. |
| `creator_ui` | Adds the desktop UI layer on top of `creator`, plus simulator/media helpers. | Host-only. | The heaviest feature in this crate; best kept off for CI or headless asset workflows unless you need the UI. |
