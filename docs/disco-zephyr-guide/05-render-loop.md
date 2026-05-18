<!--
05-render-loop.md - Volume V Chapter 5: Single-threaded render loop
with k_sleep pacing.
-->

**[<- Prev](04-touch-and-input.md) . [Index](README.md) . [Next ->](06-star-crawl-and-dma2d.md)**

# Chapter 5 — Render Loop

## Volume II reference

Vol II's bare-metal main loop runs cooperatively: poll input,
tick widgets, render dirty regions with the Compositor, present
via DMA2D, repeat. Zephyr replaces this with a blocking loop
paced by `k_sleep(K_MSEC(33))`.

## What this chapter covers

The single-threaded render loop in `zephyr_entry.rs`: frame
budget, pristine restore, the CpuBlitter + RotatedRenderer
pipeline, D-cache coherency, buffer swap, and how
`dirty_frames` gates rendering.

## The Zephyr delta

Unlike FreeRTOS (which uses separate present/render/touch tasks),
Zephyr runs everything in a single thread — the C `main` thread
that called `rlvgl_init()`. Frame pacing is via `k_sleep`, not
ERIF semaphores.

In **adapted command mode**, the render loop also handles present
(DSI_WCR pulse). In **video mode**, LTDC scans continuously so
present is just a CFBAR register write.

## Walkthrough

### 1. Frame budget

Target: ~33 ms per iteration (30 fps). Budget breakdown:

| Phase | Time | Notes |
|-------|------|-------|
| Input poll | <1 ms | Atomic reads |
| Controller tick | <1 ms | tick_count, focus sync |
| Pristine restore | ~3.75 ms | 1.5 MB memcpy |
| Widget tree draw | 5-15 ms | CpuBlitter, varies by visible widgets |
| D-cache clean | ~1 ms | Clean-by-address |
| Present | <1 ms | CFBAR + shadow reload |
| k_sleep remainder | 10-20 ms | Yields to Zephyr kernel |

### 2. Pristine restore

Same pattern as FreeRTOS: copy splash from `0xD030_0000` to the
render buffer before drawing the widget tree:

```rust
core::ptr::copy_nonoverlapping(
    pristine_base,
    render_buf,
    fb_bytes,
);
```

Zephyr restores every frame (unlike FreeRTOS which gates on
`NEEDS_PRISTINE`). This works without flicker because:
- In video mode: LTDC scans FRONT continuously while we write
  to BACK (double-buffered, no tearing).
- In ACM: LTDC is off during the back porch.

### 3. Widget tree draw

Identical to FreeRTOS and bare-metal — same `DiscoController`,
same `CpuBlitter`, same `RotatedRenderer`:

```rust
#[cfg(feature = "adapted_cmd")]
{
    let mut renderer = RotatedRenderer::new(&mut blit, fb_w);
    root.borrow().draw(&mut renderer);
}
#[cfg(not(feature = "adapted_cmd"))]
{
    root.borrow().draw(&mut blit);  // landscape, no rotation
}
```

In video mode, the framebuffer is already landscape — no
rotation needed.

### 4. Buffer swap

After draw + D-cache clean, swap render_buf:
```rust
render_buf = if render_buf == fb_front { fb_back } else { fb_front };
```

The NEXT present uses the just-swapped buffer.

### 5. k_sleep pacing

```rust
k_sleep(K_MSEC(33));
```

Yields to the Zephyr kernel for ~33 ms. Other Zephyr threads
(input driver, filesystem, idle) run during this sleep.

## Verify

- Splash visible on boot.
- Joystick navigation responsive.
- Star crawl smooth in ACM mode.
- `?` serial command responds during render loop.

## Going deeper

- Vol IV [Ch 4](../disco-freertos-guide/04-render-task.md) — the
  FreeRTOS render task for comparison.
- `zephyr_entry.rs` L750-1291 — the complete render loop.
- `zephyr_sync.rs` — `ZephyrFrameSync` with k_sem wrappers.

---

**[<- Prev](04-touch-and-input.md) . [Index](README.md) . [Next ->](06-star-crawl-and-dma2d.md)**
