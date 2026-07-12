<!--
CHANGELOG.md - Notes on chip & board database releases.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Changelog

## v0.2.5

Qt → reactive rlvgl release. Full notes: [`releases/v0.2.5.md`](releases/v0.2.5.md).

Release comparison: `refs/tags/v0.2.4..v0.2.5` (the GitHub `main` baseline at
the start of this release line).

### Added - Ratatui ↔ rlvgl, Rust all the way down (SCTD-04)
- First release of `ratatui-rlvgl` `0.1.0`: a `no_std + alloc` Ratatui backend
  backed by a retained cell surface, plus `RatatuiView` for hosting that
  surface as a native rlvgl widget. The bridge uses `ratatui-core` and
  `rlvgl-core` directly—no C LVGL, FFI, embedded-graphics, or C toolchain.
- The Dining Philosophers demo gains an additional near-full-screen hybrid
  window: native rounded chrome, title and graphical buttons surround a
  Ratatui-rendered spatial table. Native and Ratatui views share the same
  generated state machine and preserve state across open/close transitions.
- Host simulator and STM32H747I-DISCO mounts exercise the same integration.
  The checked-in promotional GIF shows native full → Depart → Arrive, followed
  by the corresponding Ratatui transitions.

### Added - reactive Qt emitter (QT-03c, QT-05g…05k)
- `qt emit --target rlvgl --scxml-context <ctx>=<crate>` links the emitted
  widget tree to an iState-generated state-machine crate and emits typed
  `Binding`s. Five kinds: state-predicate artwork swap (QT-05g), visibility
  (QT-05h), chained-predicate first-active-wins ladder (QT-05i), button-event
  taps → machine events (QT-05j), and external-text consumer-resolved labels
  (QT-05k). One caller-driven `refresh_bindings(...)` pump. Emitter output
  version `14` → `23`; istate linkage v2 (`step`/`is_active`/`get_var`/`Value`).
- Sibling-anchor solver + component instantiation (QT-03c).
- Emitter v23 produces strict-Clippy-clean Rust: precedence-aware bounds,
  snake-case Rust identifiers with original QML tags, private binding sinks,
  direct leaf returns, conditional widget mutability, and a documented
  `BuiltScreen` result alias.

### Added - widgets
- `Image::set_pixels` (runtime artwork swap) and `Image::set_hidden`
  (hidden image draws as a no-op).

### Added - asset pipeline
- `rlvgl-creator compress --transparent-key`: magenta (`#FF00FF`) sentinel for
  1-bit transparency in RGB565 RLE blobs.

### Added - SCTD demo (SCTD-00…03)
- `examples/apps/sctd-demo`: SCXML Tutorial Demo — Setup + Dining Philosophers
  + the Škoda Bolero media player under a right-edge selector shell. DP uses the
  interactive true-parallel machine with Auto mode. Mounted on disco-sim
  (`rlvgl-sctd-sim`), bare-metal STM32H747I-DISCO, UEFI, and the FireBeetle-2
  ESP32-P4 ESP-IDF hybrid.
- Two generated machine crates (media-player normalized from `bolero.scxml`;
  interactive dining philosophers), with SCXML `In()` predicates normalized to
  datamodel variables.

### Added - docs
- `docs/sctd-tutorial/` — five-chapter state-chart-to-reactive-UI tutorial.
- `docs/qt-support/` QT-05g…05k binding chapters + media-player retrospective.
- SCTD-04 architecture/execution record, deterministic capture tooling, and
  the first Ratatui/rlvgl integration artifact.

### Added - state-chart HDL handoff
- Dining Philosophers gains generated Verilog/VHDL previews plus a Quartus
  project handoff for the five-seat LED demonstration.

### Release and publishing
- Publish order now includes the first `ratatui-rlvgl` crate immediately after
  `rlvgl-core`; Gate P packages the excluded submodule crate by manifest path.
- Changed publishable crates selected from `refs/tags/v0.2.4` are released in
  dependency order. A Ratatui upstream PR follows this rlvgl release.
- Documentation changes bump `rlvgl-chips-silabs` and `rlvgl-chips-ti` to
  `0.2.1`. `rlvgl-ui` moves to `0.2.6` because `0.2.5` is already published
  and the current package raises its `rlvgl-widgets` minimum to `0.2.5`.
- `rlvgl-core` moves to `0.2.5`; its image decoders use fixed-size slice
  chunks, clearing the refreshed nightly Clippy gate without changing decode
  behavior.

### WIP - ESP32-P4 (BEETLE-IDF)
- `dfr0550` raw-PAC `clk_init` + `regi2c` bring-up; software star crawl.

### Attribution
- SCTD charts + artwork derived from Alexander Zhornyak's SCXML Tutorial
  (BSD 3-Clause, © 2017); vendored assets keep their upstream license.

## v0.2.4

LPAR parity substrate and widget-family release.

### Added - LVGL parity waves
- LPAR-02 through LPAR-10: object/event/focus/input, invalidation,
  scroll, timers/object animations, style cascade, text/draw/image/mask,
  asset/filesystem, and layout substrate.
- LPAR-11 through LPAR-15: primitive, control, navigation/selection,
  data-rich, canvas/media/property/observer widget families.
- LPAR-16 conformance fixtures across deterministic runtime behavior,
  geometry, pixel goldens, and feature-gated surfaces.

### Added - FONT (font selection & anti-aliased widget text)
- `core::font::WidgetFont` + a uniform `set_font(&'static dyn FontMetrics)`
  on every text widget (`Label`, `ui::Input`/`Textarea`/`FileBrowser`, and
  21 `widgets::` widgets), defaulting to the built-in `FONT_6X10`. Purely
  additive — no constructor or `Widget`-trait signature changed (FONT-01).
- Anti-aliased widget text by font choice: feeding a `PackedFont` (8-bit
  coverage) through the existing shaped-text pipeline yields AA; `FONT_6X10`
  stays the 1-bit default. A conformance fixture asserts partial-alpha glyph
  pixels survive the widget pipeline through a real `blend_row`-overriding
  renderer (FONT-02).
- `Renderer::draw_glyph(font, ch, origin, color)` — a defaulted single-glyph
  coverage helper. `ArcLabel` migrated off the backend-opaque `draw_text` to
  render real glyph coverage along the arc (the last legacy-`draw_text`
  widget) and adopts `WidgetFont` (FONT-03).
- `RotatedRenderer` glyph throughput: `draw_glyph`/`draw_text_shaped` rotate
  each glyph's coverage once and blit physical rows via `inner.blend_row`
  instead of per-pixel dispatch, with zero-drift parity against the software
  reference (FONT-04).
- `core::font::FontRegistry` (`FontId → &'static dyn FontMetrics`) + a
  defaulted `Widget::widget_font_mut` font sink + `apply_font_registry`, which
  walks the object tree (via `resolve_tree_with_text`), resolves each node's
  cascade `font_id`, and writes the mapped handle into the widget's
  `WidgetFont` slot — so the LPAR-07 style cascade / theme / locale can select
  widget fonts. A registered `font_id` overrides; `DEFAULT`/unmapped preserves
  an explicit `set_font`; default-`font_id` trees render identically (FONT-05).

### Added - LVGL image converter & C/Rust array output
- `rlvgl-creator lvgl <in> <out>` converts any image (or `.raw`) to an LVGL v9
  binary image (`.bin`): `--cf rgb565|rgb888|argb8888|xrgb8888` (default
  `rgb565`) and optional `--rle` (LVGL run-length, `lv_image_compressed_t`).
  The v9 codec lives in the new `rlvgl-decomp::lvgl` module — header layout,
  per-format byte order, and the `lv_rle` grammar verified against upstream
  `LVGLImage.py`, with a `decode_bin` round-trip path.
- `--emit bin|c|rust` on `compress` and `lvgl` embeds the image directly in the
  binary — the cheapest path on all-RAM SoCs with no filesystem. `compress`
  emits the RLEC blob as a `uint8_t[]` / `[u8; N]`; `lvgl` emits a ready-to-use
  C `lv_image_dsc_t` (+ pixel map) or a Rust pixel map with width/height/format/
  stride constants.
- Alpha-only icon formats `--cf a8|a4` (LVGL `A8`/`A4`): store a single coverage
  channel (color applied at draw time via recolor), with `--coverage
  auto|alpha|luminance` derivation and Floyd–Steinberg dithering for `a4`. Far
  smaller than `argb8888` for monochrome line-art icons and freely retintable. A
  matching rlvgl coverage+tint draw path
  (`rlvgl_decomp::lvgl::blend_alpha_bin_into_argb`) composites the icon onto an
  ARGB8888 buffer with a runtime fill color.
- `rlvgl-decomp` bumped to `0.2.3` for the additive `lvgl` module.

### Release notes
- `rlvgl-core`, `rlvgl-platform`, `rlvgl-widgets`, `rlvgl-ui`,
  `rlvgl-fs-sim`, `rlvgl-app-demo`, and `rlvgl-app-disco-demo` are aligned
  to `0.2.4` with matching internal dependency constraints.
- Deferred conformance item: LPAR-09 FATFS-over-`SimBlockDevice` remains
  coupled to the unimplemented `FatfsAssetSource` + std-only `rlvgl-fs-sim`
  bridge.
- Deferred optional media widgets: Lottie, DashLottie, and Texture3d remain
  outside the LPAR-16 pixel-golden set until their runtime surfaces land.
- Known validation debt: `cargo doc --workspace --no-deps` passes, but the
  rustdoc run still reports broken/private intra-doc links that should be
  cleaned before tightening docs to warnings-as-errors.

## v0.2.2

Quality release — makes the crates.io distribution work and adds the CI
to keep it that way (v0.2.1 published with a broken `simulator` feature
and is superseded). See
[docs/releases/v0.2.2.md](releases/v0.2.2.md).

### Fixed — crates.io consumers
- `rlvgl --features simulator` builds from crates.io: disco-demo's RLE
  icons were `include_bytes!`ed from outside the crate root and could
  never be packaged; vendored into `rlvgl-app-disco-demo`.
- `rlvgl-platform` builds on macOS hosts: ELF `link_section` on the
  blit scratch buffer gated to `target_os = "none"`.
- `rlvgl-audio-meters-core` added to the publish order; the matrix test
  now derives the publishable set from `cargo metadata` so omissions
  fail CI.
- v0.2.1-cycle packaging repairs: fontdue feature-resolution order in
  `rlvgl-core`, disco-assets metadata, publish-order dev-dependencies,
  root-crate include set.

### New — CRATES-CI initiative (docs/crates-ci/, CRATES-CI-00…05)
- **Gate P** (`crates-ci.yml`, required by `publish.yml`): packages all
  25 publishable crates in publish order and builds three
  workspace-detached consumers against the packaged set — lib-smoke,
  the `rlvgl-creator` CLI (`cargo install` shape, plus the umbrella
  `simulator` feature), and a user-authored simulator per
  `docs/CUSTOM-SIMULATOR.md` with playit/node automation and a
  golden-PNG threshold check.
- **Gate R** (`gate-r.yml`): the same consumers against real crates.io
  after every publish and daily; includes the literal
  `cargo install rlvgl --features creator` end-user path.
- **Creator GUI testing**: `egui_kittest` in-process harness + snapshot
  baseline (Layer K), and `rlvgl-creator --automation-headless
  --playit-port=<n>` behind the new `creator_ui_automation` feature —
  the playit wire protocol served over TCP against the kittest engine
  (Layer W), driven by the unmodified `playit/node` client. No display
  server anywhere.

### Docs
- `docs/crates-ci/CRATES-CI-00-CONCEPTS.md` (ratified) + initiative README.
- no_std STM32H747I-DISCO provenance notes.
- v0.2.2 release notes.

## v0.2.0

### Multi-vendor BSP generation
- **5 vendors with full YAML→IR→render pipelines**: Espressif (9 chips,
  14 boards), Nordic nRF (nRF52840 + DK), NXP i.MX RT (MIMXRT1062 + EVKB),
  RP2040 (Pico), Renesas RA (R7FA6M5BH + EK-RA6M5).
- Each vendor has a distinct pin routing model captured in vendor-specific IR
  types: ESP IO MUX + GPIO matrix, Nordic PSEL, NXP IOMUX ALT + daisy chain,
  RP FUNCSEL + RESETS, Renesas PFS PSEL + MSTP.
- 6 MiniJinja templates per vendor generating real PAC register writes.
- `rlvgl-creator bsp from-yaml --vendor <v>` CLI dispatch for all 5 vendors.
- `bsp list-chips --vendor <v>` and `bsp list-boards --vendor <v>` commands.
- All 9 chipdb crates overhauled from legacy binary-blob stubs to YAML
  auto-discovery with `chip_yaml()` / `board_yaml()` / `chip_names()` /
  `board_names()` APIs.

### UEFI backend
- `rlvgl-platform` gains a `uefi` feature with GOP framebuffer display,
  SimpleTextInput keyboard polling (with synthesized KeyUp events), and Serial
  I/O playit transport for test automation over QEMU virtio-serial.
- `rlvgl-example-uefi-disco` runs the disco-demo controller as a UEFI boot
  application.

### Test automation (rlvgl-playit)
- Serial test driver with touch injection, pointer/key events, multi-touch
  frames, widget queries, framebuffer pixel dumps, and event recording.
- Node.js test harness for end-to-end simulator automation.
- UEFI serial transport for pre-OS test driving.

### STM32H747I-DISCO platform
- DMA2D hardware acceleration with ISR-driven completion.
- WM8994 audio codec over I2C4 + SAI1 I2S TX + SAI4 PDM microphone.
- SDMMC block device with FATFS adapter for SD card assets.
- QSPI flash support.
- Star crawl motion system (mirrored starfield + FIR text overlay).
- CPU stats via DWT/D3 SRAM telemetry.

### FreeRTOS platform (new)
- Preemptive task model: present (P3), render (P1), touch (P2), playit (P2).
- Interrupt-driven I2C4 touch via ISR state machine + FreeRTOS semaphore.
- TIM7 one-pulse ERIF-phase-locked present with configurable holdoff.
- Single-buffer FRONT rendering (32 ms holdoff, ~18 Hz, flicker-free).
- Joystick (PK2-PK6) and button (PC13) input with keyboard navigation.
- Star crawl integration with jumbo/CFBAR model + touch-to-dismiss.
- DiscoCommand drain: star crawl, storage refresh, backlight, effects.
- SVCall/PendSV naked trampolines via `ffi_shims.c` for FreeRTOS exception routing.
- SysTick pre-scheduler gate to prevent xPortSysTickHandler on uninitialized data.
- FT5336 CTRL=0x00 keep-active init; G_MODE left at default (0x00 kills touch).
- `make build-disco-freertos` / `make flash-disco-freertos` build targets.

### Zephyr platform (existing, documented)
- C+Rust hybrid: 440-line C shell (main.c) + 1,300-line Rust entry (zephyr_entry.rs).
- Video mode (Zephyr DSI driver, landscape, DMA2D deadlocks) and adapted command
  mode (Rust raw DSI init, portrait, DMA2D works).
- SYS_INIT PG3 early reset hook for FT5336 under adapted command mode.
- INPUT_MODE_SYNCHRONOUS for dropped-event-free touch/joystick input.
- C1_LPENR CSleep fix for dual-core H747 display peripheral clock gating.
- Star crawl pipeline functional under adapted command mode.

### Platform guides (new)
- Volume IV: FreeRTOS Platform Guide (7 chapters) — scaffolding, present task,
  touch ISR, render task, input dispatch, star crawl, flicker/rendering strategy.
- Volume V: Zephyr Platform Guide (7 chapters) — build/link, C shell/FFI,
  display modes, touch/input, render loop, DMA2D, adapted command mode deep dive.

### Rendering and UI
- Anti-aliased rounded corners in core draw.
- Compositor save-under and dirty-region restoration.
- Focus highlights and keyboard navigation.
- ColorFormat profile on Screen/DisplayDriver.
- Motion primitives: Direction, MotionRate, JumboBuffer, BackgroundPattern,
  Crawl trait, TextCrawl engine, CrawlWindow composable widget.

### Infrastructure
- All 31 workspace crates at version 0.2.0.
- Publish script with 24-crate dependency-ordered publish and crates.io
  index-wait between dependents.
- Pre-publish validation: 7 phases (fmt, clippy, tests, playit, creator,
  embedded build, docs).

## v0.1.9
- See [releases/v0.1.9.md](./releases/v0.1.9.md) for detailed notes.
- STM32 BSP generation from CubeMX `.ioc` files.
- Initial vendor chipdb crate stubs (9 vendors).
- STM32H747I-DISCO board bring-up: display, touch, SD, backlight.
- i18n crate with compile-time translation blobs.

## v0.1.7
- Initial vendor crates for STM, Nordic, Espressif, NXP, Silicon Labs,
  Microchip, Renesas, Texas Instruments, and RP2040 boards.
- Added `scripts/gen_ioc_bsps.sh` to batch-convert CubeMX `.ioc` files.
- `rlvgl-creator` can now load canonical MCU definitions alongside board
  overlays from vendor archives.

## v0.1.6
- FATFS adapter (`platform::sd_fatfs_adapter`) and optional SD assets demo.
- DISCO docs: linker script, touch I2C, backlight ramp, SDMMC checklist.
- `--allow-reserved` flag for SWD pins in `bsp from-ioc`.
