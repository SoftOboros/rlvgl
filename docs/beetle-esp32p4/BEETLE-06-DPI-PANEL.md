<!--
BEETLE-06-DPI-PANEL.md - DPI controller + framebuffer + DMA-2D
descriptor list. **Live blocker** for first light in Rust.
-->

**[← BEETLE-05](BEETLE-05-DSI-HOST.md) · [Index](README.md) · [Next →](BEETLE-07-CACHE.md)**

# BEETLE-06 — DPI Controller, Framebuffer, DMA-2D Descriptor List

> **Implementation status:** **Stub — live blocker for v0 conformance.**
> `dfr0550/dpi_panel.rs::DpiPanel::init` currently returns
> `Err(DpiError::Unimplemented)`. This chapter is the spec the
> next implementation PR (`BEETLE-06a:`) lands against.

## §0 Authority policy

| Authority | Scope | Cite shape |
|---|---|---|
| ESP32-P4 TRM "MIPI DSI Bridge" / "DPI Controller" chapter | DPI register block, video timing fields, pixel format encoding | `(TRM §MIPI_DSI_BRIDGE)` |
| ESP32-P4 TRM "DMA-2D Controller" chapter | DW-GDMA descriptor format, channel arbitration, link list | `(TRM §DMA2D)` |
| `esp32p4 = 0.2` PAC | `MIPI_DSI_BRIDGE` register block, `DMA2D` peripheral | `(pac::MIPI_DSI_BRIDGE...)` / `(pac::DMA2D...)` |
| `IDF components/esp_lcd/dsi/esp_lcd_panel_dpi.c` | `esp_lcd_new_panel_dpi` reference impl | `(IDF esp_lcd_panel_dpi.c)` |
| `IDF components/hal/esp32p4/include/hal/mipi_dsi_brg_ll.h` | DSI bridge / DPI register access pattern | `(IDF mipi_dsi_brg_ll.h)` |
| `IDF components/esp_hw_support/dma/dw_gdma.c` | DW-GDMA channel + descriptor setup | `(IDF dw_gdma.c)` |
| Linux `panel-raspberrypi-touchscreen.c` | Pi 7″ video timing | `(Pi-7" Linux)` |

## §1 Purpose

Complete the bring-up by driving the DPI controller registers
(pixel format, video timing, pixel clock divider, sync-event mode)
and standing up a DW-GDMA descriptor list that streams a PSRAM-
backed framebuffer to the DSI bridge in continuous refresh.

This chapter is **the live blocker** for v0 conformance. Until it
lands, the bare-metal binary stops at LED blink 4
(`BringUpStatus::DpiPanelInit`).

## §2 Problem statement

The IDF reference shape is:

```c
esp_lcd_dpi_panel_config_t dpi_cfg = {
    .virtual_channel = 0,
    .dpi_clk_src = MIPI_DSI_DPI_CLK_SRC_DEFAULT,
    .dpi_clock_freq_mhz = 26,
    .pixel_format = LCD_COLOR_PIXEL_FORMAT_RGB888,
    .in_color_format = LCD_COLOR_FMT_RGB888,
    .num_fbs = 1,
    .flags.use_dma2d = true,
    .video_timing = { 800, 480, 2, 46, 1, 2, 21, 7 },
};
esp_lcd_new_panel_dpi(dsi_bus, &dpi_cfg, &dpi_panel);
esp_lcd_panel_init(dpi_panel);
esp_lcd_dpi_panel_get_frame_buffer(dpi_panel, 1, &fb);
```

The raw-PAC port has four sub-tasks:

1. **DPI register programming.** Pixel format (24-bit RGB888 packed),
   video mode (NON-BURST sync events), HFP/HSA/HBP/VFP/VSA/VBP from
   `dfr0550/mod.rs:50-55`, pixel clock divider already programmed by
   BEETLE-04.
2. **Framebuffer allocation.** 800×480×3 = 1 152 000 bytes in PSRAM
   at 64-B alignment. v0/v1 path: take from the bootloader's PSRAM
   heap (raw pointer into the heap window). v2 path: take from the
   raw-PAC PSRAM slab (BEETLE-01).
3. **DW-GDMA descriptor list.** Allocate a circular descriptor list
   pointing at the FB; arm the channel; switch to continuous mode.
   Matches IDF `flags.use_dma2d = true`.
4. **Video-mode handoff.** Switch the DSI host out of command mode
   (set by BEETLE-05 step 10 `mode_cfg.cmd_video_mode = 1`) to video
   mode after the DPI is armed and the FB is initially painted.

Anchor: `dfr0550/dpi_panel.rs:50-67` (currently stub).

## §3 Canonical glossary

- **DPI controller** — ESP32-P4 `MIPI_DSI_BRIDGE` peripheral. Takes
  pixel data from CPU/DMA, generates pixel timing
  (HFP/HSA/HBP/VFP/VSA/VBP), feeds the DSI host in video mode.
  *Distinct from on-panel DSI bridge (BEETLE-03).* **Owned by
  BEETLE-00 §3; restated here for chapter accessibility.**
- **DW-GDMA** — Synopsys DesignWare General-Purpose DMA, ESP32-P4
  peripheral name `DMA2D`. Linked-list descriptor model.
  **As defined in TRM "DMA-2D Controller" chapter; used without
  modification.**
- **Descriptor list** — Linked list of DMA descriptors. Each
  descriptor carries source/dest address, transfer size, link to
  next. A circular list (last descriptor links to first) produces
  continuous DMA without CPU intervention. **As defined in
  TRM DMA2D chapter; used without modification.**
- **NON-BURST sync events** — DSI video mode where each video line
  is preceded by an HSS (Horizontal Sync Start) packet and HSA/HBP
  are filled with HSS + blanking. Required by the Pi-7″ bridge.
  **As defined in MIPI DSI specification; used without modification.**
- **`FrameBuffer<'p>`** — PSRAM pointer + length view, lifetime-bound
  to the parent `DpiPanel`. **As defined in
  `dfr0550/dpi_panel.rs:37-43`; used without modification.**
- **`DpiPanel`** — Opaque handle returned by `init()`. Lifecycle:
  init → continuous re-fill loop → (no deinit; panel runs for
  binary lifetime). **As defined in
  `dfr0550/dpi_panel.rs:46-48`; used without modification.**

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| `MIPI_DSI_BRIDGE` register field names | `esp32p4` PAC |
| Pixel format encoding | TRM (via PAC field enums) |
| Video timing field positions | TRM + PAC |
| DW-GDMA descriptor format | TRM + PAC |
| DPI controller setup sequence | This chapter §9 (mirrors IDF `esp_lcd_new_panel_dpi`) |
| FB allocation strategy (v0/v1 vs v2) | This chapter §9 INV-BEETLE-06-4 |
| Descriptor list shape (linear vs circular, count) | This chapter §9 INV-BEETLE-06-5 |
| `DpiPanel::init` API + `DpiError` variants | `dfr0550/dpi_panel.rs` (code is canonical post-implementation) |
| Video-mode handoff sequencing | This chapter §9 INV-BEETLE-06-6 |

## §5 Authority relationship matrix

Inherits from [BEETLE-00 §5](BEETLE-00-CONCEPTS.md#5-authority-relationship-matrix).
Adds (via the TRM row, no new external authority):
- DW-GDMA descriptor format — relationship: mirror (PAC + TRM).

## §6 Frozen enums

`DpiError` per BEETLE-00 §6. Implementation MUST remove
`DpiError::Unimplemented` and replace with concrete failure modes:

```rust
pub enum DpiError {
    PixelClock,     // pixel clock not properly programmed
    Dma,            // DW-GDMA channel arm / descriptor failure
    FbAlloc,        // PSRAM FB allocation failure (v0 path)
    VideoMode,      // video-mode handoff failed
}
```

Adding variants: **Standards Action**.

## §7 Frozen timing & topology

*To be finalized during BEETLE-06a implementation. Anchor points:*

- **DPI register write order:** TBD vs IDF
  `esp_lcd_new_panel_dpi` body.
- **Descriptor list count:** ≥ 2 (for circular continuous mode). IDF
  uses 1 descriptor covering the full FB; we MAY split into multiple
  smaller descriptors for finer DMA progress observability.
- **FB initial paint:** all zeros before video-mode handoff (so the
  panel doesn't show PSRAM garbage during the handoff).

## §8 (reserved)

## §9 Frozen invariants

### INV-BEETLE-06-1 — Pixel format RGB888 packed

The DPI controller MUST be configured for 24-bit RGB888 packed
(no padding to 32-bit). The FB layout in `dfr0550/dpi_panel.rs:70`
(`FB_BYTES = H_RES * V_RES * 3`) assumes this.

**Registration policy:** **Standards Action**.

### INV-BEETLE-06-2 — NON-BURST sync-event video mode

The DPI controller MUST be configured in NON-BURST sync-event mode
with LP transitions enabled between frames (`disable_lp = false`).
Burst mode causes the bridge to lose sync; disabling LP causes the
flickering-horizontal-lines failure mode documented in
`project_dfr1237_dfr0550v2.md`.

**Registration policy:** **Standards Action**.

### INV-BEETLE-06-3 — Pi 7″ video timing

The DPI controller MUST be programmed with HFP=1, HSA=2, HBP=46,
VFP=7, VSA=2, VBP=21 (per `dfr0550/mod.rs:50-55`). The bridge
expects these timings exactly; deviation causes the panel to either
not sync or to display offset pixel-shifted content.

**Registration policy:** **Standards Action**.

### INV-BEETLE-06-4 — FB allocation: bootloader heap (v0/v1) vs PSRAM slab (v2)

The framebuffer MUST be allocated at a 64-B-aligned base in PSRAM
of size FB_BYTES (1 152 000). The allocation source depends on the
deployment level:

- **v0 / v1:** take from the bootloader-managed PSRAM heap. The
  `DpiPanel::init` signature returns a `FrameBuffer<'static>` whose
  pointer is the heap allocation. Lifetime is "binary forever"
  (matches the panel's continuous-refresh model).
- **v2:** take from BEETLE-01's PSRAM slab API. Same pointer
  semantics; different provenance.

Either path MUST yield a 64-B-aligned pointer; misalignment
prevents the cache writeback from covering the FB cleanly.

**Registration policy:** **Specification Required** (the v0/v1 vs
v2 split moves with the BEETLE-01 implementation).

### INV-BEETLE-06-5 — DW-GDMA descriptor list is circular and continuous

The DW-GDMA descriptor list driving the DPI controller MUST be
configured as a circular linked list (last descriptor's link
pointer references the first), with the channel in continuous mode.
This is what `flags.use_dma2d = true` in the IDF reference produces.

A non-circular (one-shot) descriptor list will paint one frame and
then idle the channel — triggering INV-BEETLE-00-4 (continuous
re-fill) failure on the bridge side.

**Registration policy:** **Standards Action**.

### INV-BEETLE-06-6 — Video-mode handoff is post-arm

The DSI host's `mode_cfg.cmd_video_mode` MUST stay = 1 (command
mode) until *after*:

1. DPI controller registers are programmed.
2. FB is allocated and initially painted (all zeros minimum).
3. DW-GDMA descriptor list is built and channel armed in
   continuous mode.

Only then may `mode_cfg.cmd_video_mode` be cleared to enter video
mode. Switching to video mode before the DMA is armed causes the
DSI host to emit blanking-only video, which the bridge interprets
as a config error and refuses to drive the TFT.

**Registration policy:** **Standards Action**.

### INV-BEETLE-06-7 — Cache writeback covers the initial paint

The initial paint (step 2 of INV-BEETLE-06-6) MUST be followed by a
`cache::writeback(fb.ptr, fb.len)` call before the DW-GDMA channel
is armed. Otherwise the channel may DMA stale cache contents on its
first pass.

Reflects INV-BEETLE-00-3 in the init context.

**Registration policy:** **Standards Action**.

## §10 Reconciliation vs adjacent repo primitives

- **BEETLE-05's `mode_cfg.cmd_video_mode = 1`** (DSI host in command
  mode after init) is the entry condition for this chapter. The
  video-mode handoff (INV-BEETLE-06-6) is owned here, not in BEETLE-05.
- **BEETLE-07's `cache::writeback`** is consumed at the initial paint
  per INV-BEETLE-06-7 and at every continuous re-fill iteration per
  INV-BEETLE-00-3.
- **BEETLE-08's widget tree** writes into the FB returned by
  `init()`; the re-fill loop wraps the widget render with the
  writeback call. The widget tree's rendering model needs to be
  compatible with the continuous re-fill loop in BEETLE-08; that
  reconciliation is BEETLE-08's job, not this chapter's.

## §11 Non-goals

- Multiple framebuffers / page flipping. v0 single-FB.
- DSI command-mode write traffic (e.g. for runtime panel parameter
  updates). The Pi-7″ bridge is configured via I2C, not DSI commands.
- Pixel-format hot-swapping (RGB888 ↔ RGB565). Not needed.
- Partial-area redraws. The DMA descriptor list paints the whole FB
  every frame; partial-area is a future optimization.
- Tear-protection (vsync-locked render). Out of scope for v0.

## §12 Acceptance checklist

A conforming BEETLE-06 implementation MUST:

- [ ] (a) Program the DPI controller for RGB888 packed, NON-BURST
      sync events, Pi 7″ video timing (per INV-BEETLE-06-1 / -2 / -3).
- [ ] (b) Allocate a 64-B-aligned PSRAM FB of FB_BYTES (1 152 000)
      bytes per INV-BEETLE-06-4.
- [ ] (c) Stand up a circular DW-GDMA descriptor list pointing at
      the FB per INV-BEETLE-06-5.
- [ ] (d) Initial-paint the FB to all zeros, cache-writeback, then
      enter video mode per INV-BEETLE-06-6 / -7.
- [ ] (e) Return `Ok((DpiPanel, FrameBuffer<'static>))` on success;
      concrete `DpiError` variant (not `Unimplemented`) on failure.
- [ ] (f) **HIL verification:** confirm
      `BringUpStatus::DpiPanelInit` (LED 4 blinks) does NOT fire on
      first boot.
- [ ] (g) **HIL verification:** with `bsp_pac_main.rs::run_color_cycle`
      called after `init` returns Ok, panel cycles R → G → B → W → K
      at ~1 s per color (first-light per BEETLE-00 §3).

## §13 Files cited

- `examples/beetle-esp32p4/src/dfr0550/dpi_panel.rs` (currently stub)
- `examples/beetle-esp32p4/src/dfr0550/mod.rs:44-65` (constants)
- `examples/beetle-esp32p4/src/bsp_pac_main.rs:91-99, 108-139`
- `~/esp/esp-idf/components/esp_lcd/dsi/esp_lcd_panel_dpi.c`
- `~/esp/esp-idf/components/hal/esp32p4/include/hal/mipi_dsi_brg_ll.h`
- `~/esp/esp-idf/components/esp_hw_support/dma/dw_gdma.c`
- ESP32-P4 TRM "MIPI DSI Bridge" and "DMA-2D Controller" chapters
- `/tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c`
  (verified-working IDF reference)

## §14 Unblocks

- **v0 conformance** (first light in raw-PAC Rust on this hardware).
- BEETLE-08 (disco-demo widget tree mount).
- Future Espressif HMI ports of this rlvgl-creator pipeline.

## §15 Change log

- **2026-05-28** (initial) — Authored alongside BEETLE-00. Reflects
  the IDF reference at
  `/tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c`
  and the stub `dfr0550/dpi_panel.rs` from commit `9ed43fc` ("DPI
  panel timings + host pattern generator (5b.4)"). Invariants 1-7
  first ratification. §7 left partially open pending BEETLE-06a
  implementation — exact DPI register write order and descriptor
  list shape need to be pinned during implementation. Acceptance
  gates (f) and (g) are the v0 conformance criteria.

---

**[← BEETLE-05](BEETLE-05-DSI-HOST.md)** · **[Index](README.md)** · **Next →** [BEETLE-07 — Cache Writeback](BEETLE-07-CACHE.md)
