<!--
10-star-crawl-part-2.md - Volume II Chapter 10: state machine + DMA2D + cache + teardown.
-->

**[← Prev](09-star-crawl-part-1.md) · [Index](README.md) · [Next →](11-generated-bsps.md)**

# Chapter 10 — Star Crawl Part II: Pipeline, Cache & State Machine

## Volume I reference

Chapter 9 gave you buffers and a FIR pass. This chapter
shows how the crawl actually runs frame to frame — the state
machine, the DMA2D choreography, the cache flush that makes
D2 SRAM usable by DMA2D, and the teardown when the user
dismisses the effect.

Every mechanism Chapter 5–7 introduced shows up here, doing
real work at once. This is the capstone.

## What this chapter covers

1. Two enums, `RenderStage` (internal) and `StepResult`
   (returned from `tick()`).
2. The **non-blocking step machine** that walks one small
   unit of work per call.
3. The **DMA2D starfield row blits** with admission gating
   and the ISR completion latch.
4. The **D-cache clean** before the A8→ARGB blend — the
   write-back-SRAM trap from
   [Ch 1 §5](01-why-bare-metal.md#5-d-cache-transparency--the-write-back-trap).
5. The **A8 blend** that paints all text into the back buffer
   in one DMA2D pass with YELLOW as the foreground color.
6. **Q.8 scroll physics**, including the 1/3-speed parallax
   star layer.
7. **Teardown** and pristine-desktop restore.

## The HAL / PAC gap

Three recurring ones:

- D-cache transparency (
  [Ch 1 §5](01-why-bare-metal.md#5-d-cache-transparency--the-write-back-trap)).
- DMA2D completion is ISR-driven because polling races
  (Chapter 7).
- The entire crawl exists as a non-blocking task so it can
  coexist with the ERIF-gated present loop (Chapter 5) — you
  cannot just render the whole frame with a blocking helper
  and keep the UI responsive.

## Walkthrough

### 1. Two enums, one machine

At
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L73–92:

```rust
#[derive(Copy, Clone, Eq, PartialEq)]
enum RenderStage {
    Idle           = 0,
    RenderFrame    = 1,
    StartTextBlend = 2,
    WaitTextBlend  = 3,
}

/// Result of advancing the crawl task by one step.
pub enum StepResult {
    /// Crawl is inactive.
    Idle,
    /// More work remains before the frame can be presented.
    Pending,
    /// The back buffer is complete and ready to present.
    FrameReady,
    /// Crawl reached the end of the script and deactivated.
    Finished,
}
```

Keep these distinct:

- **`RenderStage`** is the **internal** sub-state inside a
  single frame. The crawl lives in `RenderFrame` for most
  of a tick sequence, briefly passes through `StartTextBlend`
  and `WaitTextBlend`, then loops.
- **`StepResult`** is what `tick()` returns to the **main
  loop**. `Pending` means "call me again"; `FrameReady` means
  "back buffer is done, present it"; `Finished` means "turn
  me off." `Idle` is the inactive state.

### 2. One frame, stage by stage

`tick()` body at
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L316–438. Trimmed to the key decisions:

```rust
match self.stage {
    RenderStage::Idle => StepResult::Pending,

    RenderStage::RenderFrame => {
        // Gate: wait for LTDC scan to finish (ERIF) before first DMA2D burst.
        if self.bg_row == 0
            && self.frame_id > 1
            && !crate::ERIF_FLAG.load(Ordering::Acquire)
        {
            return StepResult::Pending;
        }

        // --- DMA2D starfield management ---
        // Use the ISR completion latch instead of poll_complete():
        // the DMA2D ISR clears TCIF before poll_complete() can
        // see it, causing a race that prevents bg_row from advancing.
        if !dma2d.is_in_flight() && crate::dma2d_irq::take_complete() {
            self.bg_row += 1;
        }

        if self.bg_row < FB_H && !dma2d.is_in_flight() {
            // Admission: each row blit is ~500 cycles.
            if !crate::dma2d_admits(500) {
                return StepResult::Pending;
            }
            let star_row = (self.frame_star_row + self.bg_row) % STAR_ROWS;
            let src = unsafe { self.starfield.add((star_row * STAR_STRIDE) as usize) };
            let dst = unsafe { self.back_buf.add((self.bg_row * self.fb_w * BPP) as usize) };
            crate::dma2d_irq::note_start();
            dma2d.start_blit_raw(
                src as *const u8, STAR_STRIDE,
                dst, self.fb_w * BPP,
                FB_W, 1, PixelFmt::Argb8888,
            );
        }

        // --- CPU FIR: one text row → A8 portrait column (Chapter 9) ---
        if self.text_row < CRAWL_H {
            // ...perspective + fir_resample_text_row()...
            // ...copy scanline_buf into A8_BUF at dst_x_off...
        }

        // --- Check completion ---
        let star_done = self.bg_row  >= FB_H;
        let text_done = self.text_row >= CRAWL_H;
        let dma_done  = !dma2d.is_in_flight();
        if star_done && text_done && dma_done {
            self.stage = RenderStage::StartTextBlend;
        }
        StepResult::Pending
    }

    RenderStage::StartTextBlend => {
        // A8 blend is ~800K cycles. Don't start if budget is tight.
        if !crate::dma2d_admits(800_000) {
            return StepResult::Pending;
        }

        // Flush D-cache for the A8 buffer so DMA2D sees all CPU
        // writes. D2 SRAM at 0x3000_0000 is Write-Back cached
        // under the default Cortex-M7 background map.
        dcache_clean_range(A8_BUF, A8_SIZE);

        // Single DMA2D A8→ARGB blend of the entire text layer.
        let dst_offset = (A8_Y_BASE * self.fb_w * BPP) as usize;
        let dst = unsafe { self.back_buf.add(dst_offset) };
        crate::dma2d_irq::note_start();
        dma2d.start_blend_a8_color(
            A8_BUF as *const u8, A8_WIDTH, A8_HEIGHT,
            YELLOW,
            dst, self.fb_w * BPP,
        );
        self.stage = RenderStage::WaitTextBlend;
        StepResult::Pending
    }

    RenderStage::WaitTextBlend => {
        if dma2d.is_in_flight() { return StepResult::Pending; }
        let _ = crate::dma2d_irq::take_complete();
        self.finish_frame();
        StepResult::FrameReady
    }
}
```

Sequence per frame:

1. **ERIF gate** (Chapter 5) — first `RenderFrame` of each
   frame waits for LTDC's end-of-refresh interrupt.
2. **Starfield row blits** — one DMA2D M2M copy per
   scheduler pass, admission-gated at ~500 cycles each.
   Completion comes through `COMPLETE_LATCH` (Chapter 7).
3. **CPU FIR** — Chapter 9's text resampler runs in parallel
   with DMA2D; the CPU and DMA2D are on different AXI
   masters and don't compete for SDRAM (which is why the
   A8 buffer is in D2 SRAM — see Ch 9 §1).
4. **When both finish**, transition to `StartTextBlend`.
5. **D-cache clean** over the A8 buffer (see §4 below).
6. **A8→ARGB blend** — one DMA2D job paints every text
   pixel onto the starfield back buffer with YELLOW as the
   foreground color.
7. **`WaitTextBlend`** polls BUSY + consumes the completion
   latch, then marks the frame ready.

### 3. The scroll physics

At
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L176–180:

```rust
/// Advance the logical scroll position after a successful present.
pub fn advance_scroll(&mut self) {
    self.scroll_q8      += self.scroll_speed_q8;
    self.star_scroll_q8 += self.scroll_speed_q8 / 3;
}
```

Signed Q.8 fixed-point for both the text scroll and the
starfield scroll, with the stars moving at **1/3** the text
speed. That speed differential is the parallax cue — nearer
text rolls past while the distant starfield drifts slowly.
`scroll_speed_q8` is computed from `SCROLL_PX_PER_SEC = 40`
and the runtime frame rate (Chapter 8's SysTick at 30 Hz
produces `40 * 256 / 30 ≈ 341` Q.8 per frame).

`advance_scroll()` is called **after** `display.present()`
succeeds, so a dropped frame (admission failure or touch
pre-emption) doesn't visually advance the scroll.

### 4. The D-cache clean

At
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L718–736:

```rust
/// Clean D-cache lines covering `[addr, addr+size)` so DMA2D sees CPU writes.
///
/// D2 SRAM at 0x3000_0000 is Write-Back Write-Allocate under the default
/// Cortex-M7 background map. Without a clean, DMA2D reads stale data.
fn dcache_clean_range(addr: usize, size: usize) {
    const DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32;
    const LINE_SIZE: usize = 32;
    let start = addr & !(LINE_SIZE - 1);
    let end   = (addr + size + LINE_SIZE - 1) & !(LINE_SIZE - 1);
    let mut a = start;
    while a < end {
        unsafe { DCCMVAC.write_volatile(a as u32); }
        a += LINE_SIZE;
    }
    cortex_m::asm::dsb();
}
```

Every write to `DCCMVAC` (SCB register at `0xE000_EF68`)
cleans the cache line containing the address you passed.
The loop walks the full 288 KB A8 buffer in 32-byte steps,
and `dsb()` makes sure all cleans have drained before
kicking DMA2D.

Without this call, DMA2D's A8 read picks up zeros — the CPU
wrote alpha into the D-cache lines but nothing flushed them
to D2 SRAM, and DMA2D (an AXI master) reads main memory
directly. The screen shows the starfield with **no text**.

### 5. A8→ARGB blend

The call to `dma2d.start_blend_a8_color(…)` is configured as:

- **FG** = A8 source at `A8_BUF`, 480 × 600 pixels.
- **FGCOLR** (constant color for A8 mode) = `YELLOW =
  0x00FF_D700`.
- **BG** = the back buffer (already containing starfield +
  any widget that rendered before the crawl stage).
- **Output** = same as BG; ARGB8888.
- **OOR** (output offset) = `fb_w × BPP − dst width`.

DMA2D walks the A8 buffer, multiplies each alpha by the
constant color, and blends over the back buffer. Result:
yellow text on starfield, perspective applied.

### 6. Teardown

At
[`star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
L171–174 and L227–232:

```rust
pub fn deactivate(&mut self) {
    self.active = false;
    self.drop_frame();
}

pub fn drop_frame(&mut self) {
    if self.frame_active {
        self.diag_dropped_frames = self.diag_dropped_frames.saturating_add(1);
    }
    self.reset_frame_state();
}
```

`deactivate()` is what the main loop calls when the user
taps to dismiss the crawl. It drops any in-flight frame and
clears internal state. The main loop notices `is_active() ==
false` and takes the standard "pristine desktop restore"
path (Vol I Chapter 3 saved this pristine copy when the
desktop was first painted).

The ending condition at L282–286 is symmetric — when
`scroll_px` walks past the tail of the text:

```rust
let scroll_px = self.scroll_q8 >> 8;
if scroll_px >= (self.text_h + CRAWL_H) as i32 {
    self.deactivate();
    return StepResult::Finished;
}
```

## Register diagram — the crawl's full register surface

```
SCB DCCMVAC  @ 0xE000_EF68   (Chapter 10 §4)
DMA2D        @ 0x5200_1000   (Chapter 7)
LTDC / DSI   @ 0x5001_0000 / 0x5000_0000 (Chapter 5, ERIF-gated)
RCC AHB2ENR  @ 0x5802_44DC   (Chapter 9 §2, D2 SRAM clocks)
DWT_CYCCNT   @ 0xE000_1004   (Chapters 5 + 7, timing)
FMC SDRAM    @ 0xD000_0000   (Chapter 3, starfield + text_src base)
D2 SRAM      @ 0x3000_0000   (Chapter 9 §1, A8 portrait)
```

Every chapter of Volume II has shown up in one call path.

## Verify

- Tap **Info → StarCrawl** (Vol I Chapter 6 left this as a
  pointer to `star_crawl.rs`; replace the stub with a real
  `star_crawl.activate(dma2d)` call to see it run).
- Yellow perspective text crawls from the bottom of the panel,
  shrinking as it approaches the top, against a slowly
  drifting starfield.
- Tap anywhere else → `deactivate()` runs and the desktop
  restore takes over within a frame.
- `diag_words()` (L197–212) exports dropped-frame and error
  counts. With a correct build, dropped frames should be
  zero at 30 Hz.

Fault modes:

- Starfield with **no text** → you forgot the D-cache clean.
- Text shows **only at certain scroll positions** → one of the
  admission gates is too strict; DMA2D is running out of
  budget for the A8 blend.
- Text **drifts left or right as it scrolls** → perspective
  calc in Chapter 9 §4 is off — check `dst_x_off` centering.

## Going deeper

- RM0399 §B3.2.1 — Cortex-M7 cache maintenance operations.
- `rlvgl_platform::dma2d::Dma2dBlitter::start_blend_a8_color`
  at
  [`platform/src/dma2d.rs`](../../platform/src/dma2d.rs) —
  the actual register sequence.
- [`docs/rendering/ALPHA-BLENDING.md`](../rendering/ALPHA-BLENDING.md)
  — the blend math DMA2D implements.
- [`examples/stm32h747i-disco/src/star_crawl.rs`](../../examples/stm32h747i-disco/src/star_crawl.rs)
  — the full 736-line source this chapter walked through.

---

**[← Prev](09-star-crawl-part-1.md) · [Index](README.md) · [Next →](11-generated-bsps.md)**
