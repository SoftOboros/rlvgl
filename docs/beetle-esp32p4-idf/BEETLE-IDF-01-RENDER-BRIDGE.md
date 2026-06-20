<!--
BEETLE-IDF-01-RENDER-BRIDGE.md - Milestone M1: the C<->Rust render bridge
and the software RGB888 renderer. C owns hardware; Rust owns pixels.
-->

**[← BEETLE-IDF-00](BEETLE-IDF-00-CONCEPTS.md) · [Index](README.md) · [BEETLE-IDF-02 →](BEETLE-IDF-02-TOUCH.md)**

# BEETLE-IDF-01 — The C↔Rust Render Bridge + Software RGB888 Renderer

> **Status:** Shipped; HIL-verified 2026-06-15. Retroactive record of
> milestone **M1**. Vocabulary is owned by
> [BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md); this chapter references
> it rather than restating.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| [`components/rlvgl_app/rust/src/lib.rs`](../../examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs) | Rust payload: staticlib glue, software renderer, C-ABI entry | `(rlvgl_app/lib.rs:NN)` |
| [`main/dfr0550_idf_compare.c`](../../examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c) | C host: framebuffer alloc, refill loop, cache writeback | `(idf_compare.c:NN)` |
| ESP-IDF v5.3.5 `esp_lcd` MIPI-DSI/DPI + `esp_cache` | DSI/DPI bring-up, FB alloc, `esp_cache_msync` | `(esp_lcd/...)` |
| `rlvgl-core::Renderer` | render trait surface the payload implements | `(core/...)` |
| [BEETLE-00 §9 INV-BEETLE-00-4](../beetle-esp32p4/BEETLE-00-CONCEPTS.md) | continuous re-fill contract | `(BEETLE-00 §9)` |

## §1 Purpose

Prove the whole Rust↔ESP-IDF toolchain bridge: let the C host own the
DSI/DPI bring-up it already locks reliably, and let a no_std Rust
staticlib own the framebuffer contents through a single C-ABI entry and a
self-contained software RGB888 renderer. M1 is the foundation every later
milestone (touch, disco-demo, backlight) builds on.

## §2 Problem statement

1. **Two toolchains, one archive.** rlvgl is Rust/no_std; IDF is C. The
   GNU linker only accepts the mixed archive if the Rust objects match
   IDF's float ABI (`-mabi=ilp32f`). See INV-BEETLE-IDF-2.
2. **No Rust runtime under IDF.** A no_std staticlib has neither a heap
   nor a panic path until one is supplied; rlvgl's widget tree needs an
   allocator.
3. **Panel byte order ≠ config name.** The DPI panel is configured
   `LCD_COLOR_PIXEL_FORMAT_RGB888` (`idf_compare.c:228`) but the bytes
   that reach it are interpreted **B,G,R** in memory. See
   INV-BEETLE-IDF-4.
4. **The bridge desyncs if the CPU idles.** A paint-once model desyncs
   the DFR0550-V2 STM32F072 bridge to white; the render path MUST be a
   continuous re-fill loop with a per-frame cache writeback. See
   INV-BEETLE-IDF-3 / [BEETLE-00 §9](../beetle-esp32p4/BEETLE-00-CONCEPTS.md).

## §3 Glossary

All terms — **render entry**, **Rust payload**, **`Rgb888Renderer`**,
**refill loop**, **C host** — are defined in
[BEETLE-IDF-00 §3](BEETLE-IDF-00-CONCEPTS.md) and used here without
modification. This chapter adds none.

Note the historical shape of the **render entry**: M1 originally exposed
the 3-argument form `rlvgl_app_render(fb, width, height)`; the
`touch_x, touch_y, touch_active` parameters were added in M3
([BEETLE-IDF-02](BEETLE-IDF-02-TOUCH.md)). The 6-argument signature is now
the frozen surface (INV-BEETLE-IDF-1); the M1 record documents the renderer
and bridge, not the final argument list.

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| DSI/DPI bring-up, FB alloc, cache writeback | C host (`idf_compare.c`) — code is canonical |
| C-ABI render entry shape | INV-BEETLE-IDF-1 (frozen in concepts gate) |
| Pixel byte order (`[B,G,R]`) | Rust payload (`Rgb888Renderer`) — INV-BEETLE-IDF-4 |
| Allocator / panic glue | Rust payload (`IdfAlloc` / `#[panic_handler]`) — code is canonical |
| Renderer trait surface | `rlvgl-core::Renderer` (consumed, not forked) |

## §5 Authority relationship matrix

| External authority | Concept | Relationship | Mutation rights | Divergence policy |
|---|---|---|---|---|
| ESP-IDF `esp_lcd` / `esp_cache` | DSI/DPI bring-up + writeback | consume | none — IDF is a fixed dependency | pin IDF v5.3.5 |
| `rlvgl-core::Renderer` | render trait | mirror | none — owned by rlvgl-core | upstream trait changes break this consumer at rebuild |
| IDF newlib `malloc`/`free`/`abort` | Rust runtime backing | consume | none | host runtime owns the heap |

## §6 Frozen enums

None. No enums are introduced by this chapter.

## §7 Frozen timing & topology

- **Refresh:** ~30 Hz, `vTaskDelay(pdMS_TO_TICKS(33))` (`idf_compare.c:388`).
- **Buffers:** two PSRAM RGB888 framebuffers, ping-ponged; the C host
  renders into the off-screen buffer and flips with
  `esp_lcd_panel_draw_bitmap` on the panel's vblank (`idf_compare.c:384-387`).
- **FB size:** `800 × 480 × 3 = 1,152,000` bytes per buffer
  (`DFR0550_FB_BYTES`, `idf_compare.c:40`).
- **Cache:** `esp_cache_msync(..., C2M)` of the whole FB after the render
  entry returns, before the flip (`idf_compare.c:295-296`).

## §9 Frozen invariants

This chapter mints no new invariants; it is the first implementation of
several concepts-gate freezes:

- **INV-BEETLE-IDF-1** (render entry signature) — the `#[no_mangle]`
  definition lives at `(rlvgl_app/lib.rs:300)`; the C side calls it at
  `(idf_compare.c:293)`. The signature MUST match the header.
- **INV-BEETLE-IDF-2** (ilp32f) — the payload MUST build for
  `riscv32imafc-unknown-none-elf` so its objects are
  `EF_RISCV_FLOAT_ABI_SINGLE`; no float crosses the ABI (the internal f32
  raster math is ABI-isolated). Rationale documented at
  `(rlvgl_app/lib.rs:15-19)`.
- **INV-BEETLE-IDF-3** (clear + writeback) — M1 establishes the writeback
  half: the C host MUST `esp_cache_msync(..., C2M)` every frame
  (`idf_compare.c:295`). The per-frame *clear* half is the subject of
  [BEETLE-IDF-03](BEETLE-IDF-03-DISCO-DEMO.md).
- **INV-BEETLE-IDF-4** (B,G,R byte order) — `Rgb888Renderer::put` stores
  `[B, G, R]` per pixel (`rlvgl_app/lib.rs:121-127`). Verified on hardware
  2026-06-15: a logical blue `Color(40,90,200)` showed up red until the
  channels were swapped (`rlvgl_app/lib.rs:90-93`).

### Implementation notes (informative)

- **Runtime glue.** `IdfAlloc` is a `#[global_allocator]` over the host
  `malloc`/`free` (`rlvgl_app/lib.rs:57-74`); the `#[panic_handler]`
  routes to host `abort()` (`rlvgl_app/lib.rs:76-80`). rlvgl's
  `Rc`/`Vec`/`String` blocks are pointer-aligned (≤4 on rv32), so
  newlib's `max_align_t` malloc suffices (`rlvgl_app/lib.rs:51-56`).
- **Renderer surface.** Only `fill_rect` and `draw_text` are *required*
  by the `Renderer` trait; `blend_rect` / `blend_row` (and, in M4,
  `draw_pixels`) are overridden for alpha compositing
  (`rlvgl_app/lib.rs:150-212`). Everything else inherits the core
  software defaults, which funnel back through these methods.
- **Build wiring.** The component's `CMakeLists.txt` runs `cargo build`
  via `ExternalProject` and imports `librlvgl_app.a`, so a normal
  `idf.py build` builds the Rust payload too (README §"Build & flash").

## §10 Reconciliation vs adjacent repo primitives

- **vs. raw-PAC [BEETLE-08](../beetle-esp32p4/BEETLE-08-DEMO-INTEGRATION.md).**
  BEETLE-08 owns a register-level `Display` adapter over a raw-PAC DSI
  bring-up and is blocked on ERRATA-009. M1 reaches the same "rlvgl draws
  into the live FB" state by delegating bring-up to IDF. The
  `Rgb888Renderer` here is the IDF-hybrid analogue of BEETLE-08's planned
  `Display` adapter; see [BEETLE-IDF-00 §10](BEETLE-IDF-00-CONCEPTS.md).
- **vs. `rlvgl-platform` display adapters.** Unlike the DISCO LTDC adapter
  and the BBB fbdev adapter, this payload implements `Renderer` directly
  rather than going through a `Display`/`flush` abstraction — the C host
  owns the flush and the cache writeback.

## §11 Non-goals

- Raw-PAC DSI bring-up (delegated to IDF).
- Touch, disco-demo mount, backlight — those are M3/M4/M5
  (chapters 02–04).
- Over-aligned allocations (none expected; the allocator would need an
  aligned-alloc shim if that changes, `rlvgl_app/lib.rs:53-56`).

## §12 Acceptance checklist

- [x] (a) The hybrid builds: `idf.py build` compiles the Rust staticlib
      and links it against the IDF C app (INV-BEETLE-IDF-2).
- [x] (b) The render entry resolves at link and is called every frame by
      the C refill loop (`idf_compare.c:293`).
- [x] (c) `Rgb888Renderer` writes `[B,G,R]` packed pixels; a logical blue
      paints blue on the panel (INV-BEETLE-IDF-4, HIL 2026-06-15).
- [x] (d) The C host writes the FB back with `esp_cache_msync(..., C2M)`
      every frame; the bridge stays synced (INV-BEETLE-IDF-3 writeback half).
- [x] (e) **HIL control 2026-06-14:** IDF locked `phy_status=0x0000153d`
      and allocated two 1,152,000-byte RGB888 framebuffers.

## §13 Files cited

- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/src/lib.rs`
- `examples/beetle-esp32p4-idf/main/dfr0550_idf_compare.c`
- `examples/beetle-esp32p4-idf/components/rlvgl_app/rust/Cargo.toml` (staticlib manifest)
- `examples/beetle-esp32p4-idf/components/rlvgl_app/include/rlvgl_app.h` (C-ABI header)

## §14 Unblocks

- [BEETLE-IDF-02](BEETLE-IDF-02-TOUCH.md) (M3) — the render entry grows
  touch parameters and the payload gains release-debounced input.
- [BEETLE-IDF-03](BEETLE-IDF-03-DISCO-DEMO.md) (M4) — the shared
  disco-demo tree mounts on this renderer.

## §15 Change log

- **2026-06-19** (ratified retroactively) — documents work that shipped on
  the v0.2.4 branch (BEETLE M1/M3/M4/M5), merged to main in #216 /
  `5187ce0`.

---

**[← BEETLE-IDF-00](BEETLE-IDF-00-CONCEPTS.md) · [Index](README.md) · [BEETLE-IDF-02 →](BEETLE-IDF-02-TOUCH.md)**
