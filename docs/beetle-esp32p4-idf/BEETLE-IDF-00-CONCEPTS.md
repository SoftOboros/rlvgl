<!--
BEETLE-IDF-00-CONCEPTS.md - Concepts gate for the FireBeetle 2 ESP32-P4 +
DFR0550-V2 ESP-IDF-hybrid track. C owns hardware, Rust owns pixels.
-->

**[Index](README.md) · [BEETLE-IDF-01 →](BEETLE-IDF-01-RENDER-BRIDGE.md)**

# BEETLE-IDF-00 — Concepts Gate (ESP-IDF Hybrid)

> **Status:** Ratified 2026-06-19 (first §15 entry). This is the
> vocabulary-owning gate for the IDF-hybrid track. Milestone chapters
> 01–05 cite this glossary without restating it.

## §0 Authority policy

This track brings the shared disco-demo widget tree up on the
**DFR1237 + DFR0550-V2** hardware by a different route than the
[raw-PAC BEETLE family](../beetle-esp32p4/README.md): the ESP-IDF C
application owns all hardware, and a no_std Rust staticlib owns the
pixels. The two tracks share hardware identity (BEETLE-00 §1) but **not**
a conformance posture, a binary, or a register-bring-up sequence.

| Authority | Scope | Cite shape |
|---|---|---|
| ESP-IDF v5.3.5 `esp_lcd` MIPI-DSI/DPI driver | DSI bus + DPI panel bring-up, framebuffer alloc, `esp_cache_msync` | `(esp_lcd/...)` |
| [`main/dfr0550_idf_compare.c`](../../examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c) | C host: hardware ownership, refill loop, touch read, backlight hook | `(idf_compare.c:NN)` |
| [`components/rlvgl_app/rust/src/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Rust payload: software renderer, C-ABI entry, controller drive | `(rlvgl_app/lib.rs:NN)` |
| [`examples/apps/disco-demo/`](../../examples/apps/disco-demo/) | Shared widget tree, `DiscoController`, `DiscoCommand`/`DiscoEffect` | `(disco-demo/...)` |
| `rlvgl-core` / `rlvgl-platform` | `Renderer`/`Widget` traits, `Screen`, `Event` | `(core/...)` / `(platform/...)` |
| [BEETLE-00 §1](../beetle-esp32p4/BEETLE-00-CONCEPTS.md) | Hardware identity (kit/module/panel/bus/touch) | `(BEETLE-00 §1)` |
| Linux `panel-raspberrypi-touchscreen.c` | STM32F072 bridge register names (REG_PWM, POWERON) | `(pi-panel)` |

Hardware identity (DFR1237 kit, DFR1172 module, DFR0550-V2 panel, I2C
SCL=GPIO8 / SDA=GPIO7, bridge @ 0x45, FT5x06 touch @ 0x38) is **defined
in [BEETLE-00 §1](../beetle-esp32p4/BEETLE-00-CONCEPTS.md); used without
modification.** This gate does not restate it.

## §1 Purpose

Run the shared disco-demo widget tree — interactively, with touch and a
live backlight control — on the DFR0550-V2 5″ DSI panel, by letting
ESP-IDF own the DSI/DPHY bring-up it already locks reliably and letting
rlvgl own only the framebuffer contents.

This track exists because the **raw-PAC port cannot lock the DSI DPHY
PLL** ([ERRATA-009](../beetle-esp32p4/ERRATA.md)), while the IDF
`esp_lcd` path locks on the same board every time. Rather than keep
fighting the analog PLL to satisfy the raw-PAC family's v1 goal
([BEETLE-08](../beetle-esp32p4/BEETLE-08-DEMO-INTEGRATION.md)), this
track reaches the **same end state — disco-demo on the live panel** — by
a route that is shippable today and is a faithful third platform variant
of the shared payload.

## §2 Problem statement

1. **No HAL, incomplete raw PAC.** The `esp-hal` MIPI-DSI/DPI surface is
   incomplete and the raw-PAC DPHY PLL never locks
   ([ERRATA-009](../beetle-esp32p4/ERRATA.md)). The IDF C `esp_lcd`
   driver is the only path that drives this panel today.
2. **Two toolchains, one binary.** rlvgl is Rust/no_std; IDF is C. The
   GNU linker only accepts the mixed archive if the Rust objects match
   IDF's float ABI. ESP32-P4 IDF builds with `-mabi=ilp32f`; the Rust
   payload MUST target `riscv32imafc-unknown-none-elf` so it emits
   `EF_RISCV_FLOAT_ABI_SINGLE` (ilp32f) objects.
   (`rlvgl_app/lib.rs:15`).
3. **The bridge desyncs if the CPU idles.** The DFR0550-V2's STM32F072
   bridge desyncs to white if the framebuffer stops being touched. The
   render path MUST be a continuous re-fill loop with a cache writeback
   every frame, not a paint-once-and-idle model
   (inherited from [BEETLE-00 §9 INV-BEETLE-00-4](../beetle-esp32p4/BEETLE-00-CONCEPTS.md)).
4. **Panel byte order is not its config name.** The DPI panel is
   configured `LCD_COLOR_PIXEL_FORMAT_RGB888`, but the bytes that reach
   the panel are interpreted **B, G, R** in memory (verified on hardware
   2026-06-15). The renderer MUST store `[B, G, R]` per pixel
   (`rlvgl_app/lib.rs:92`).
5. **Capacitive touch jitters mid-press.** The FT5x06 routinely drops a
   contact for a single frame during a hold. A raw edge-per-frame would
   fragment one physical tap into several events. Release MUST be
   debounced (`rlvgl_app/lib.rs:231`).

## §3 Canonical glossary

For every term that also exists in code, the entry cites the
authoritative source and marks the relationship per
[CLAUDE.md §"Definitions — reference vs. restatement"](../../CLAUDE.md#spec-before-code-planning-discipline).

- **C host** — `main/dfr0550_idf_compare.c`. Owns PSRAM, LDO_VO3, the
  I2C bridge wake, `esp_lcd_new_dsi_bus`, `esp_lcd_new_panel_dpi`, the
  double framebuffer, the refill loop, the touch read, and the backlight
  PWM write. **Defined in `(idf_compare.c)`; canonical here.**
- **Rust payload** — the `rlvgl-p4-idf-glue` crate built as
  `crate-type=["staticlib"]` → `librlvgl_app.a`. Owns the software
  renderer and the `DiscoController`. **Defined in
  `(rlvgl_app/rust/Cargo.toml)`; canonical here.**
- **Render entry** — `rlvgl_app_render(fb, w, h, touch_x, touch_y,
  touch_active)`, the single C-ABI symbol the C host calls each frame.
  **Owned by BEETLE-IDF-01; defined `(rlvgl_app/lib.rs:300)`.** Its
  signature is a frozen invariant (§6, INV-BEETLE-IDF-1).
- **Backlight hook** — `rlvgl_host_set_backlight(level: u8)`, a Rust
  `extern "C"` declaration the C host implements; maps an abstract
  `0..=100` level to the bridge's `REG_PWM`. **Owned by BEETLE-IDF-04;
  C side `(idf_compare.c:187)`, Rust decl `(rlvgl_app/lib.rs:48)`.**
- **`Rgb888Renderer`** — the software `Renderer` impl that writes
  `[B,G,R]` packed pixels into the DPI framebuffer. **Defined
  `(rlvgl_app/lib.rs:100)`; canonical here.**
- **`DiscoController` / `DiscoCommand` / `DiscoEffect`** — the shared
  controller and its runtime-command surface. **Defined in
  `(disco-demo/src/lib.rs)`; used without modification.** This track is
  a *consumer*; it MUST NOT fork these types.
- **Release debounce** — the rule that a finger lift is confirmed only
  after `RELEASE_DEBOUNCE_FRAMES` (=3) consecutive no-touch frames, so
  one physical tap dispatches exactly one `PressRelease`. **Owned by
  BEETLE-IDF-02; defined `(rlvgl_app/lib.rs:231)`.**
- **Refill loop** — the C host's `for (;;)` at ~30 Hz that reads touch,
  calls the render entry into the back buffer, flips with
  `esp_lcd_panel_draw_bitmap`, and `vTaskDelay(33 ms)`. **Defined
  `(idf_compare.c:365)`; canonical here.**
- **Star crawl** — a software Star-Wars-style scrolling-text effect run
  by the Rust payload in response to a drained
  `StartEffect(DiscoEffect::StarCrawl)`. **Owned by BEETLE-IDF-05; does
  not yet exist in repo.** Distinct from the STM32 DMA2D
  `star_crawl.rs` (which is hardware-coupled and not reused here).

## §4 Source-of-truth map

One owner per concept across the raw-PAC and IDF-hybrid tracks.

| Concept | Owner |
|---|---|
| DSI/DPI hardware bring-up | C host (`idf_compare.c`) — code is canonical |
| Framebuffer allocation + cache writeback | C host (`esp_lcd` + `esp_cache_msync`) |
| Pixel format / byte order | Rust payload (`Rgb888Renderer`, `[B,G,R]`) |
| Render entry C-ABI shape | BEETLE-IDF-01 §9 (frozen) |
| Touch read + axis mapping | C host (`touch_read`) + BEETLE-IDF-02 (flip + debounce) |
| Touch → event conversion | Rust payload (`rlvgl_app_render` edge logic) |
| Backlight level → PWM | C host (`rlvgl_host_set_backlight`) + BEETLE-IDF-04 |
| Backlight slider widget | `disco-demo` `BacklightPanel` (shared; code is canonical) |
| Widget tree / controller | `disco-demo` (shared; code is canonical) |
| Star crawl effect renderer | BEETLE-IDF-05 (code mirrors after the chapter lands) |

## §5 Authority relationship matrix

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| ESP-IDF `esp_lcd` | DSI/DPI bring-up | consume | none — IDF is a fixed dependency | pin IDF v5.3.5; revisit on IDF bump |
| `examples/apps/disco-demo/` | widget tree, `DiscoCommand`/`DiscoEffect` | compose | none — payload shared with DISCO + BBB | upstream changes land at next rebuild |
| `rlvgl-core::Renderer` / `Widget` | render + event traits | mirror | none — owned by rlvgl-core | upstream trait changes break this consumer at rebuild |
| `panel-raspberrypi-touchscreen.c` | bridge register names | mirror | none | names match kernel driver verbatim |

## §6 Frozen enums & invariants

No new enums are introduced by this track. `DiscoCommand` and
`DiscoEffect` are **frozen in disco-demo under Standards Action**
([disco-demo concepts]); this track adds no variants and MUST NOT.

### INV-BEETLE-IDF-1 — Render entry signature is frozen

The C-ABI render entry MUST remain
`rlvgl_app_render(fb: *mut u8, width: i32, height: i32, touch_x: i32,
touch_y: i32, touch_active: i32) -> ()`. The header
(`components/rlvgl_app/include/rlvgl_app.h`) and the Rust `#[no_mangle]`
definition MUST agree. New per-frame inputs (e.g. a future audio sample)
require a **Standards Action** amendment here **first**, in a separate
PR, because the C host and Rust payload are compiled separately and a
silent signature skew links cleanly but corrupts the stack.

**Registration policy:** **Standards Action**.

### INV-BEETLE-IDF-2 — ilp32f float ABI

The Rust payload MUST build for `riscv32imafc-unknown-none-elf` so its
objects are `EF_RISCV_FLOAT_ABI_SINGLE` (ilp32f), matching IDF's
`-mabi=ilp32f`. No float crosses the C ABI; the constraint is purely the
GNU linker's archive-compatibility check. **Standards Action**.

### INV-BEETLE-IDF-3 — Every drawn frame clears then writes back

Because the disco-demo root container is transparent (it composites over
a desktop layer on STM32), the Rust payload MUST fully clear the back
buffer each frame before drawing; and the C host MUST
`esp_cache_msync(..., C2M)` the whole framebuffer after the render entry
returns and before the panel flip. The double buffers ping-pong, so each
must be cleared on the frame it draws. Skipping the clear accumulates
stale pixels; skipping the writeback desyncs the bridge to white.
**Standards Action** (the 30 Hz writeback floor is the bridge contract,
[BEETLE-00 §9](../beetle-esp32p4/BEETLE-00-CONCEPTS.md)).

### INV-BEETLE-IDF-4 — Pixel byte order is B,G,R

The renderer MUST store `[B, G, R]` per pixel despite the
`RGB888` config name. **Specification Required** (local to the renderer;
the hardware fact is fixed).

### INV-BEETLE-IDF-5 — Release-debounced single tap

A finger lift MUST be confirmed only after `RELEASE_DEBOUNCE_FRAMES`
consecutive no-touch frames; exactly one `PressRelease` is dispatched per
physical tap, at the last in-contact coordinate. **Specification
Required**.

## §7 Frozen topology & timing

- **Refresh:** ~30 Hz, `vTaskDelay(pdMS_TO_TICKS(33))`. The bridge desync
  floor is 30 Hz; this is the floor with no margin to spare — a render
  entry that overruns its ~33 ms slice risks desync.
- **Buffers:** two PSRAM RGB888 framebuffers, ping-ponged; the panel
  flip (`esp_lcd_panel_draw_bitmap`) happens on the panel's vblank.
- **Resolution:** 800×480 landscape (`DFR0550_H_RES` × `DFR0550_V_RES`).
- **Touch origin:** the FT5x06 reports coordinates point-reflected 180°
  vs. the panel mount; the C host flips both axes
  (`x = 799 - raw`, `y = 479 - raw`) before passing them in
  (`idf_compare.c:175`).

## §10 Reconciliation vs adjacent repo primitives

- **vs. raw-PAC BEETLE-08.** Both tracks target "disco-demo on the live
  FB." Raw-PAC ([BEETLE-08](../beetle-esp32p4/BEETLE-08-DEMO-INTEGRATION.md))
  owns the *register* bring-up and is blocked on ERRATA-009. This track
  reaches the same app state via IDF and is **shippable today**. They are
  parallel prongs, not competitors: the raw-PAC v2 goal (bootloader-free
  MSPI) remains the long-term target; the IDF-hybrid is the pragmatic
  bring-up that proves the app payload and toolchain bridge now.
- **vs. STM32 `star_crawl.rs`.** The STM32 crawl is DMA2D- and
  SDRAM-address-coupled (fixed `0xD1xx_xxxx` scratch, D2 SRAM A8 buffer,
  DWT gating). It is **not** reused here; BEETLE-IDF-05 specs a
  self-contained software crawl that runs through `Rgb888Renderer`. Both
  satisfy the same user-facing `DiscoEffect::StarCrawl` contract.
- **vs. shared `disco-demo`.** This track is a pure consumer. The one
  shared-crate change in this track's history — the `BacklightPanel`
  slider (BEETLE-IDF-04) — landed in disco-demo for all platforms, not
  forked into the P4 payload.

## §11 Non-goals

- **Raw-PAC DSI bring-up.** Owned by the [raw-PAC family](../beetle-esp32p4/README.md);
  this track deliberately delegates it to IDF.
- **Audio / storage.** No codec or SD wired on this hybrid; capabilities
  advertise `audio:false, storage:false`.
- **Sharing the crawl renderer with STM32.** A future consolidation may
  extract a platform-agnostic software crawl; out of scope here.
- **Multi-screen / windowing.** Single FB, single screen.

## §12 Acceptance checklist (gate-level)

This gate is satisfied when chapters 01–05 each carry a ratified §15
entry and:

- [x] (a) The hybrid builds: `idf.py build` compiles the Rust staticlib
      and links it (INV-BEETLE-IDF-2).
- [x] (b) The render entry signature matches between header and Rust
      (INV-BEETLE-IDF-1).
- [x] (c) disco-demo paints and is interactive on the panel (chapters
      01–03, HIL-verified).
- [x] (d) Backlight is adjustable from the UI (chapter 04, HIL-verified).
- [x] (e) The star crawl runs on a tap of the Info → Star Crawl item and
      dismisses on a tap (chapter 05; HIL-verified 2026-06-19).

## §13 Files cited

- `examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c` (C host)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs` (Rust payload)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/include/rlvgl_app.h` (C-ABI header)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/Cargo.toml` (staticlib manifest)
- `examples/apps/disco-demo/src/lib.rs` (shared controller)
- `examples/apps/disco-demo/src/backlight_panel.rs` (shared slider)
- `examples/stm32h747i-disco/src/star_crawl.rs` (STM32 crawl — reconciliation only)

## §14 Unblocks

- The hybrid milestone chapters (01–05) gain a vocabulary home.
- BEETLE-IDF-05 (star crawl) gains a ratified spec before code.
- A future `BEETLE-IDF-06` (e.g. file browser, audio) inherits the
  conformance frame.

## §15 Change log

- **2026-06-19** (ratified) — Concepts gate authored to give the
  ESP-IDF-hybrid track a vocabulary home after it crossed the
  3-phase threshold (M1 display, M3 touch, M4 disco-demo, M5 backlight +
  slider already shipped on the v0.2.4 branch, merged to main in #216 /
  `5187ce0`). Glossary §3, source-of-truth map §4, and invariants
  INV-BEETLE-IDF-1..5 frozen. Star crawl declared as BEETLE-IDF-05,
  owned-by-spec, not-yet-in-repo.

---

**[Index](README.md)** · **[BEETLE-IDF-01 →](BEETLE-IDF-01-RENDER-BRIDGE.md)**
