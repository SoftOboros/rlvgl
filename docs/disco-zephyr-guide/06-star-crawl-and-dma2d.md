<!--
06-star-crawl-and-dma2d.md - Volume V Chapter 6: DMA2D pipeline under
Zephyr, video mode deadlock, ACM solution.
-->

**[<- Prev](05-render-loop.md) . [Index](README.md) . [Next ->](07-adapted-cmd-deep-dive.md)**

# Chapter 6 — Star Crawl & DMA2D

## Volume II reference

Vol II [Chapters 9-10](../disco-platform-guide/09-star-crawl-part-1.md)
built the star crawl as a DMA2D-accelerated pipeline: row shifts,
FIR text overlay, ARGB compose. The pipeline requires DMA2D M2M
transfers.

## What this chapter covers

Why DMA2D M2M deadlocks in video mode, how adapted command mode
solves it, and how the star crawl integrates with Zephyr's
single-threaded render loop.

## The video mode DMA2D deadlock

In video mode, LTDC continuously scans the framebuffer at 60 Hz.
Each scan reads 1.5 MB from SDRAM via AXI. DMA2D M2M also reads
SDRAM via AXI. With LTDC's continuous high-bandwidth reads, DMA2D
M2M transfers stall indefinitely — the DMA2D START bit never
clears.

**DMA2D R2M (fill) works** because it only writes SDRAM (no AXI
read contention). **DMA2D M2M deadlocks** because both LTDC and
DMA2D try to read SDRAM simultaneously.

The AXI dead-time register (`DMA2D_AMTCR`) throttles DMA2D
bursts, but LTDC's continuous scan leaves no gaps for DMA2D to
read.

## Adapted command mode: the fix

In ACM, LTDC only scans when LTDCEN is pulsed. Between scans,
LTDC is off — the AXI bus is free for DMA2D. The ERIF ISR clears
LTDCEN after each scan, giving DMA2D exclusive SDRAM access
during the back porch.

The star crawl pipeline:
1. Advance scroll (Q.8 fixed-point physics).
2. DMA2D M2M row shift (starfield scroll).
3. CPU FIR text resampler (A8 alpha buffer).
4. DMA2D A8-to-ARGB compose (text over starfield).
5. Present (pulse LTDCEN).

### Batch processing

The render loop batches up to 1024 prep ticks per frame:

```rust
for _ in 0..1024 {
    match cr.prep_next_frame(dma, sync) {
        StepResult::FrameReady => { break; }
        StepResult::Pending => {}
        StepResult::Finished => { crawl_active = false; break; }
        StepResult::Idle => break,
    }
}
```

Each tick is one row of FIR text processing. When `FrameReady`
fires, the composed buffer is presented.

### DMA2D sync

`ZephyrFrameSync` implements `Dma2dSync`:
- `note_start()` records DWT cycle count.
- `take_complete()` polls DMA2D ISR TCIF bit (non-blocking).
- `take_error()` checks CEIF/TEIF.

The Zephyr build does NOT enable DMA2D TC interrupts — it polls
TCIF. This works because the render loop is single-threaded; no
other task is waiting on DMA2D completion.

## Verify

```bash
make zephyr-disco-acm       # adapted command mode
make zephyr-disco-flash
```

Navigate to info wing -> star crawl -> Enter. The crawl should
render with visible starfield scroll and yellow text overlay.
Touch to dismiss.

In video mode, the star crawl will hang (DMA2D deadlock). This
is expected.

## Going deeper

- Vol II [Ch 9](../disco-platform-guide/09-star-crawl-part-1.md)
  — text pre-render, perspective, FIR.
- Vol II [Ch 10](../disco-platform-guide/10-star-crawl-part-2.md)
  — DMA2D pipeline, state machine.
- Vol IV [Ch 6](../disco-freertos-guide/06-star-crawl-integration.md)
  — FreeRTOS crawl for comparison.
- `star_crawl.rs` — the shared pipeline implementation.

---

**[<- Prev](05-render-loop.md) . [Index](README.md) . [Next ->](07-adapted-cmd-deep-dive.md)**
