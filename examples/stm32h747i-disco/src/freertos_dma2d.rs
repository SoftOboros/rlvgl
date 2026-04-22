//! FreeRTOS-flavored DMA2D blitter.
//!
//! Wraps `rlvgl_platform::dma2d::Dma2dBlitter` so that the `Blitter::fill`
//! / `blit` / `blend` operations kick the DMA2D engine (non-blocking)
//! and then **block on the DMA2D transfer-complete semaphore** instead
//! of the platform blitter's spin-wait.
//!
//! Why this matters:
//!
//! * On bare-metal, `Dma2dBlitter::wait()` busy-loops on `CR_START`.
//!   That's fine when there's no other code to run.
//! * Under FreeRTOS the render task is one of several preemptible
//!   tasks. Spinning inside the render task burns cycles that should
//!   go to the idle hook, the touch task, or anything else ready to
//!   run. Blocking on a semaphore lets the scheduler pick whoever is
//!   next and returns to us when the DMA2D ISR gives the sem.
//!
//! The DMA2D ISR (see `freertos_entry::dma2d_isr_body`) already
//! performs `xSemaphoreGiveFromISR(dma2d_done_sem)` on TC. We just
//! need the task side to (a) enable the TCIE bit in CR (only done
//! once, at blitter construction) and (b) block on the sem rather
//! than spin.
//!
//! # Scope
//!
//! This iteration targets the `fill` path only — the demo compositor
//! in `freertos_layers` uses fills exclusively, and mapping `blit`
//! / `blend` onto semaphore-blocked variants follows the same
//! pattern. For unsupported ops we fall through to the inner spin
//! path so unrelated callers still work.

#![allow(dead_code)]

use rlvgl_platform::blit::{BlitCaps, Blitter, Rect, Surface};
use rlvgl_platform::dma2d::Dma2dBlitter;
use rlvgl_platform::hwcore::addr::PhysAddr;
use rlvgl_platform::hwcore::surface::{BackBuffer, FrameBuffer};

use crate::freertos_sync::{self, SemaphoreHandle_t, pdTRUE};

/// How long to wait for DMA2D transfer completion before giving up.
/// 50 ms is generous — a 480×800 R2M fill is ~2 ms at 200 MHz AHB3,
/// small dirty rects finish in microseconds. A timeout at this scale
/// indicates a real fault (DMA2D wedged, ISR not firing) rather than
/// normal slow progress.
const DMA2D_WAIT_TICKS: u32 = 50;

pub struct FreeRtosDma2dBlitter {
    inner: Dma2dBlitter,
    done_sem: SemaphoreHandle_t,
    /// Counts sem-take timeouts so a debugger can spot DMA2D stalls.
    pub timeouts: u32,
}

impl FreeRtosDma2dBlitter {
    /// Build a new semaphore-blocking DMA2D blitter.
    ///
    /// Enables the transfer-complete interrupt on the underlying
    /// peripheral; the DMA2D ISR handler in `freertos_entry` does
    /// the sem-give from IRQ context.
    ///
    /// # Safety
    ///
    /// `done_sem` must be a valid, initialized FreeRTOS binary
    /// semaphore, shared with the DMA2D ISR.
    pub unsafe fn new(mut inner: Dma2dBlitter, done_sem: SemaphoreHandle_t) -> Self {
        inner.enable_tc_interrupt();
        // Drain any stale give — if the bare-metal init kicked DMA2D
        // before we installed this blitter, the sem may already have
        // one count available which would satisfy our first take
        // without a real transfer completing.
        unsafe { freertos_sync::rlvgl_sem_take(done_sem, 0) };
        Self {
            inner,
            done_sem,
            timeouts: 0,
        }
    }

    #[inline]
    fn wait_for_tc(&mut self) {
        let r = unsafe { freertos_sync::rlvgl_sem_take(self.done_sem, DMA2D_WAIT_TICKS) };
        if r != pdTRUE {
            // Sem didn't fire within budget — fall back to polling so
            // the render task makes forward progress even if an ISR
            // gets lost. Rare; counted for diagnostics.
            self.timeouts = self.timeouts.wrapping_add(1);
            while self.inner.is_in_flight() {
                cortex_m::asm::nop();
            }
        }
    }
}

impl Blitter for FreeRtosDma2dBlitter {
    fn caps(&self) -> BlitCaps {
        // Inherit from the inner blitter; we support fill today and
        // delegate others as-is (still correct, just spin-wait for
        // now — to be promoted to sem-blocked in a follow-up).
        self.inner.caps()
    }

    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32) {
        // Compute raw pointer + stride from the Surface. This mirrors
        // the inner `start_fill`'s arithmetic but uses the public
        // `start_fill_raw` entry point so we can cleanly insert the
        // sem-wait between start and completion.
        let bpp: usize = match dst.format {
            rlvgl_platform::blit::PixelFmt::Argb8888 => 4,
            rlvgl_platform::blit::PixelFmt::Rgb565 => 2,
            _ => 1,
        };
        if area.w == 0 || area.h == 0 {
            return;
        }
        // Clip to surface bounds to match CpuBlitter semantics.
        let sx = area.x.max(0) as usize;
        let sy = area.y.max(0) as usize;
        let ex = (area.x + area.w as i32).min(dst.width as i32).max(0) as usize;
        let ey = (area.y + area.h as i32).min(dst.height as i32).max(0) as usize;
        if ex <= sx || ey <= sy {
            return;
        }
        let w = (ex - sx) as u32;
        let h = (ey - sy) as u32;
        let _ = bpp; // pixel size is rederived inside `start_fill_typed`.

        // SAFETY: `dst.buf` is a `&mut [u8]` reborrowed by the caller for
        // the duration of `fill`. We synthesize a transient
        // `FrameBuffer` over its base pointer so `start_fill_typed` can
        // apply the typed borrow contract; the `BackBuffer<'_>` lives
        // only across the typed call and `into_borrow` returns the
        // reborrow before this method exits, restoring `dst.buf` for
        // the caller.
        let mut fb = unsafe {
            FrameBuffer::from_phys(
                PhysAddr::new(dst.buf.as_mut_ptr() as u32),
                dst.width,
                dst.height,
                dst.stride as u32,
                dst.format,
            )
        };
        let mut back = BackBuffer::wrap(&mut fb);
        let inflight = self.inner.start_fill_typed(
            back.dma_dst(),
            Rect {
                x: sx as i32,
                y: sy as i32,
                w,
                h,
            },
            color,
        );
        let _ = inflight.into_borrow();
        self.wait_for_tc();
        // Drain the M7 write queue before returning so the compositor's
        // next read sees the DMA2D fill — without this the migrated
        // typed-API path showed residual crawl content over the desktop
        // when star_crawl deactivated. The original raw `start_fill_raw`
        // was structurally similar but went through fewer stack frames;
        // the explicit barrier makes the post-fill memory state
        // observable regardless of how the borrow chain unwinds.
        cortex_m::asm::dsb();
    }

    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        // Fall through to the platform blitter's spin path for now.
        // Promoting this to sem-blocked follows the same recipe as
        // `fill` once a consumer needs it.
        self.inner.blit(src, src_area, dst, dst_pos);
    }

    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32)) {
        self.inner.blend(src, src_area, dst, dst_pos);
    }
}
