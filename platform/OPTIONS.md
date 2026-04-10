<!--
OPTIONS.md - Cargo feature reference for the rlvgl-platform crate.
-->
# rlvgl-platform Options

`rlvgl-platform` contains host simulators, framebuffer backends, embedded board
drivers, and accelerator integrations. The base crate is `no_std`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` by default; `simulator` is host-only and `uefi` is
  target-specific.
- Pattern: many media features are thin forwards into `rlvgl-core`.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `regression` | Enables regression-only tests in this crate. | Test-oriented. | No production runtime impact. |
| `sdram_ramtest` | Enables SDRAM validation helpers. | Embedded-oriented. | Adds debug/test code; not usually wanted in release firmware. |
| `simulator` | Enables the desktop simulator stack (`wgpu`, `winit`, `eframe`, loading helpers). | Host-only; this is the main `std` path in the crate. | Large compile-time and binary-size increase, but essential for desktop simulation. |
| `uefi` | Enables the UEFI display/input backend and Playit transport support. | Requires a UEFI target such as `aarch64-unknown-uefi`. | Moderate code-size increase. |
| `st7789` | Enables SPI display integration for ST7789-class panels. | Embedded `no_std`; requires `embedded-hal` and display-interface traits. | Small-to-moderate increase tied to panel support. |
| `stm32h747i_disco` | Enables the STM32H747I-DISCO board support path. | Embedded ARM-only in practice. | Pulls in the largest board-specific dependency set in this crate. |
| `experimental_dsi_host` | Opts into the experimental DSI host bring-up path. | Only meaningful with the H747 display stack. | Can affect stability more than performance; use only for bring-up and testing. |
| `semihosting` | Enables semihosting debug output helpers. | ARM debug builds only. | Useful for diagnostics; can slow execution when heavily used. |
| `fatfs_nostd` | Enables the `fatfs` dependency configured for `no_std`/core I/O use. | Embedded-oriented; separate from the higher-level `fatfs` forwarding feature. | Storage support adds code size and I/O cost. |
| `sd_storage` | Enables SD/MMC-backed storage helpers. | Embedded targets with `embedded-sdmmc`. | Moderate code-size increase; runtime cost depends on storage traffic. |
| `audio` | Enables the STM32H747 audio path. | Meaningful only with the H747 runtime. | Adds significant peripheral setup and streaming logic. |
| `dma2d` | Enables DMA2D-accelerated drawing helpers. | STM32H747-oriented embedded feature. | Can materially improve rendering throughput at the cost of more code and hardware complexity. |
| `splash` | Enables RLE splash decoding through `rlvgl-decomp`. | `no_std`-friendly. | Small runtime cost for boot-time decode; modest code-size increase. |
| `fontdue` | Forwards Fontdue support from `rlvgl-core`. | Same practical host-oriented constraints as the core feature. | Moderate code-size increase. |
| `png` | Forwards PNG support from `rlvgl-core`. | Host-oriented in the current manifest layout. | Moderate decoder overhead. |
| `jpeg` | Forwards JPEG support from `rlvgl-core`. | Host-oriented in the current manifest layout. | Moderate decoder overhead. |
| `gif` | Forwards GIF support from `rlvgl-core`. | Brings in `std` via the core feature. | Moderate-to-high overhead for animated assets. |
| `qrcode` | Forwards QR code generation from `rlvgl-core`. | Host-oriented in the current manifest layout. | Small-to-moderate overhead. |
| `apng` | Forwards APNG support and enables the `image` crate locally. | Host-oriented. | Moderate-to-high code-size increase. |
| `lottie` | Forwards Lottie API support from `rlvgl-core`. | Backend availability depends on downstream host support. | Minimal by itself. |
| `canvas` | Enables canvas helpers and embedded-graphics integration. | `no_std`-friendly. | Moderate code-size increase. |
| `pinyin` | Forwards pinyin support from `rlvgl-core`. | Mirrors the core feature's current `std` behavior. | Small increase. |
| `fatfs` | Forwards high-level FAT helpers from `rlvgl-core`. | Mirrors the core feature's constraints. | Adds storage code paths. |
| `nes` | Forwards NES integration from `rlvgl-core`. | Mirrors the core feature's current `std` behavior. | High binary and CPU cost when exercised. |
