<!--
07-flicker-and-rendering.md - Volume IV Chapter 7: Rendering strategy,
flicker analysis, and future DMA2D staging.
-->

**[<- Prev](06-star-crawl-integration.md) . [Index](README.md)**

# Chapter 7 — Flicker, Tearing & Rendering Strategy

## Volume II reference

Vol II [Chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
introduced the ERIF holdoff and AXI contention model. Bare-metal
uses a `Compositor` with dirty-rect tracking to render changed
regions into both ping-pong buffers, achieving flicker-free
double-buffered output.

## What this chapter covers

The rendering tradeoffs explored during the FreeRTOS bring-up.
No code — this chapter documents the problem space and why the
current single-buffer approach was chosen.

## The double-buffer divergence problem

With double-buffering (render into BACK, swap to FRONT), the two
buffers diverge because `ctrl.tick()` advances widget state between
renders. The display alternates between FRONT (frame N-1) and
BACK (frame N) at 30 Hz — a visible flicker.

Bare-metal solves this with the `Compositor`: it tracks which
screen regions changed and only redraws those regions in both
buffers. After `dirty_frames=4` cycles, both buffers converge
because the unchanged regions are already identical.

FreeRTOS does a full-screen pristine restore + full widget tree
draw. The two buffers can never converge during animation because
each frame has a different widget state.

## Approaches tested

### 1. Double-buffer + buf_ready swap (initial approach)

- Render into BACK, give `buf_ready_sem`, present swaps.
- **Result**: visible flicker during dirty_frames window.
  FRONT and BACK show different frames.

### 2. BACK-to-FRONT memcpy after render

- After rendering into BACK, copy BACK to FRONT so both match.
- **Result**: the copy (~3.75 ms) races the LTDC retrigger.
  Present runs at higher priority and retriggers while the copy
  is still writing FRONT. Tearing.

### 3. Single-buffer FRONT render (chosen approach)

- Render directly into FRONT while LTDC is off (32 ms holdoff).
- **Result**: no flicker, no tearing, but ~18 Hz frame rate.
  Each render must complete within the holdoff or LTDC scans
  a partial frame.

### 4. DMA2D staging blit (tested, deferred)

- Render into BACK at leisure. DMA2D M2M blit BACK->FRONT
  (~1 ms) during the back porch.
- **Result**: DMA2D blit confirmed working (`DMA:ok` on
  hardware). Architecturally correct but was tangled with touch
  debugging. Ready to re-integrate when touch is stable.

## Pristine restore tradeoff

The splash JPEG (decoded at boot into `0xD030_0000`) serves as
the desktop background. The `DiscoController`'s root container
is transparent (`alpha=0`), so the widget tree relies on the
splash being in the framebuffer.

Full-screen pristine restore (1.5 MB `copy_nonoverlapping`) takes
~3.75 ms. When done every frame, the display shows a brief flash
of the splash without widgets. Gating pristine behind
`NEEDS_PRISTINE` (set only on state-changing events like Enter)
eliminates the flash for focus-highlight-only changes.

## Holdoff tuning

| Holdoff | Frame rate | Fits settings wing? |
|---------|-----------|---------------------|
| 15 ms | ~30 Hz | No — 5 icon decode overruns |
| 22 ms | ~23 Hz | Borderline |
| 28 ms | ~20 Hz | Usually |
| 32 ms | ~18 Hz | Yes, with margin |

The settings wing (5 RLE icon decodes, ~3 ms each + pristine
3.75 ms + CpuBlitter draw) pushes total render time to ~20-25 ms.
32 ms holdoff provides sufficient margin.

## Future: DMA2D staging blit

The correct long-term architecture:

```
ERIF -> render_task wakes:
  Phase A: DMA2D blit BACK->FRONT (~1 ms)   <- previous frame
  Phase B: pristine + draw -> BACK           <- next frame
```

Phase A is fast (DMA2D hardware) and safe (LTDC off during porch).
Phase B can take as long as needed — if it spans past the next
ERIF, Phase A just re-blits the old BACK content (frame repeat).

Present always retriggers FRONT. No swap, no flicker. Proven
working on hardware (`DMA:ok` diagnostic). Will be re-integrated
once the rendering pipeline is fully stable.

## Future: Compositor dirty-rect port

The ultimate solution (matching bare-metal quality): port the
`Compositor` dirty-rect tracker to the FreeRTOS render task.
Only redraw changed regions. Both buffers converge after 4 frames.
This enables full 30 Hz double-buffered rendering without flicker.

---

**[<- Prev](06-star-crawl-integration.md) . [Index](README.md)**
