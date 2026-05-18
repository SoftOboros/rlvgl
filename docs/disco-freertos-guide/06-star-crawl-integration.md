<!--
06-star-crawl-integration.md - Volume IV Chapter 6: Star crawl under
FreeRTOS with jumbo-buffer CFBAR model.
-->

**[<- Prev](05-input-dispatch.md) . [Index](README.md) . [Next ->](07-flicker-and-rendering.md)**

# Chapter 6 — Star Crawl Under FreeRTOS

## Volume II reference

Vol II [Chapters 9-10](../disco-platform-guide/09-star-crawl-part-1.md)
dissected the star crawl pipeline: text pre-render, FIR
resampler, DMA2D compose, and the `RenderStage` state machine.
The FreeRTOS integration preserves the same pipeline but runs it
inside the render task with FreeRTOS semaphore-based DMA2D sync.

## What this chapter covers

The `CRAWL_REQ` toggle handshake, the jumbo/CFBAR model that
eliminates per-frame DMA2D blits for the starfield, the LTDC
Layer 2 A8 text overlay, touch-to-dismiss, and the
`DiscoCommand::StartEffect` bridge from the widget tree.

## Walkthrough

### 1. Toggle handshake

The star crawl is toggled by two sources:
- **Serial `C` command**: `playit_task` sets `CRAWL_REQ` atomic.
- **Widget tree**: info wing star crawl button queues
  `DiscoCommand::StartEffect(StarCrawl)`, drained by render task.

The render task checks `CRAWL_REQ` each iteration:

```rust
let req = CRAWL_REQ.swap(false, Ordering::AcqRel);
if req {
    if cr.is_active() {
        cr.deactivate();
        CRAWL_FB_ADDR.store(0, Ordering::Release);
        disable_ltdc_layer2();
    } else {
        cr.activate(dma);
        // Zero Layer 2 ARGB buffer, enable LTDC Layer 2
        setup_ltdc_layer2_a8(w, h);
    }
}
```

### 2. Jumbo/CFBAR model

Instead of DMA2D-blitting the starfield into a display buffer
each frame, the render task points LTDC directly at the starfield
source buffer at the current scroll offset:

```rust
const CRAWL_BASE: usize = 0xD100_0000;
let star_row = (cr.star_scroll_q8() >> 8) as u32 % 1600;
let cfbar = CRAWL_BASE as u32 + star_row * 480 * 4;
CRAWL_FB_ADDR.store(cfbar, Ordering::Release);
```

The present task reads `CRAWL_FB_ADDR` and retriggers LTDC with
it instead of `FRONT_FB_ADDR`. Zero DMA2D per frame — just a
CFBAR register write.

### 3. LTDC Layer 2 text overlay

The crawl text is pre-rendered as FIR-resampled A8 alpha values
in D2 SRAM (`0x3000_0000`), then expanded to ARGB8888 yellow
(`(alpha << 24) | 0x00FFD700`) into SDRAM at `0xD180_0000`.
LTDC Layer 2 blends this over Layer 1 (starfield) with per-pixel
alpha blending.

### 4. Touch-to-dismiss

While the crawl is active, any touch event in the SPSC ring
deactivates it:

```rust
if cr.is_active() {
    if touch_evt_pop().is_some() {
        while touch_evt_pop().is_some() {} // drain
        cr.deactivate();
        CRAWL_FB_ADDR.store(0, Ordering::Release);
        disable_ltdc_layer2();
        CRAWL_ACTIVE.store(false, Ordering::Release);
        continue; // skip to next ERIF, desktop will render
    }
}
```

## Verify

1. Navigate to info wing -> star crawl icon -> Enter.
2. Crawl should scroll with text overlay.
3. Touch the screen -> crawl dismisses, desktop returns.
4. Serial `C` command should also toggle.

## Going deeper

- Vol II [Ch 9](../disco-platform-guide/09-star-crawl-part-1.md)
  — text pre-render and FIR resampler.
- Vol II [Ch 10](../disco-platform-guide/10-star-crawl-part-2.md)
  — DMA2D pipeline and state machine.
- `star_crawl.rs` — the complete `StarCrawl` implementation.

---

**[<- Prev](05-input-dispatch.md) . [Index](README.md) . [Next ->](07-flicker-and-rendering.md)**
