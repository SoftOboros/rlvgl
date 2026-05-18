//! Dirty-region compositor for the FreeRTOS render task.
//!
//! A minimal, self-contained layered renderer that demonstrates the
//! efficiency pattern we want to back-propagate into widget z-axis
//! analysis:
//!
//! 1. Every visible element is a `Layer` in a z-ordered stack.
//! 2. Each layer reports which rectangles it has modified since the
//!    last composite via `dirty()`.
//! 3. The `Compositor` unions dirty rects and, for each rect, walks
//!    the layer stack bottom-to-top calling `render(surface, clip)`.
//! 4. Layers render only inside the clip rect — so static layers
//!    repaint "behind" a moving layer's old position without touching
//!    the rest of the frame.
//!
//! ## Double-buffer sync
//!
//! The FreeRTOS render task writes into the back buffer, which the
//! present task swaps to the front on the next ERIF. After the swap,
//! the buffer that was last displayed becomes the new back — it holds
//! content from two frames ago relative to this frame's compositor
//! state. To converge both buffers, the compositor replays the
//! **previous** frame's dirty rects alongside the current frame's.
//! After two consecutive renders, both buffers have identical content.
//!
//! This replay is why `Compositor` tracks `prev_dirty` in addition to
//! the current `dirty` list.
//!
//! ## Widget z-axis back-propagation
//!
//! The contract here — `dirty()` returns the rects a layer has
//! modified; `render(surface, clip)` paints only inside `clip` —
//! maps directly onto a widget tree where each widget is a layer and
//! the parent-child order implies z-order. Widgets that don't change
//! return an empty `dirty()` and cost zero work that frame.
//!
//! This module intentionally lives under the FreeRTOS-only `cfg` so
//! it doesn't collide with parallel work extending the existing
//! platform compositor.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl_platform::blit::{Blitter, Rect, Surface};

/// A paintable element in z-order.
///
/// The compositor walks layers bottom-to-top for each dirty rect. A
/// layer may have zero contribution inside a given clip rect — in
/// that case `render()` is expected to be a no-op.
pub trait Layer {
    /// Advance animation / state by one frame. Implementations should
    /// push into an internal dirty list describing what pixels this
    /// layer changed as a result of the tick.
    fn tick(&mut self) {}

    /// Rects this layer has modified since the last `clear_dirty()`.
    fn dirty(&self) -> &[Rect] {
        &[]
    }

    /// Clear the internal dirty list. Called by the compositor after
    /// it has accumulated this layer's rects into its own.
    fn clear_dirty(&mut self) {}

    /// Paint the layer's contribution to `clip` into `surface`.
    ///
    /// `clip` is guaranteed to be bounded by the surface dimensions.
    /// Layers should clip their own geometry against `clip` so they
    /// don't over-draw neighbouring dirty regions.
    fn render(&self, blitter: &mut dyn Blitter, surface: &mut Surface, clip: Rect);
}

/// Per-frame statistics — useful for `probe-rs read32` over SRAM
/// breadcrumbs so perf tuning can happen without a debugger UI.
#[derive(Copy, Clone, Default)]
pub struct FrameStats {
    /// Sum of `w * h` over all dirty rects (duplicates counted).
    pub pixels_touched: u32,
    /// Number of dirty rects composited this frame.
    pub rect_count: u32,
    /// Cycles spent inside `render_frame`.
    pub cycles: u32,
}

/// Ordered layer stack + dirty-region accumulator.
pub struct Compositor {
    layers: Vec<Box<dyn Layer>>,
    dirty: Vec<Rect>,
    prev_dirty: Vec<Rect>,
    /// Full-surface seed rect, applied for `seed_remaining` consecutive
    /// frames. We prime **both** buffers on startup — not just one —
    /// because the present task swaps front/back and the buffer that
    /// wasn't primed would still carry whatever boot-time content was
    /// in SDRAM (splash image, garbage, etc). Dirty-region replay only
    /// touches the small rects each layer moved, so areas outside any
    /// dirty rect would forever show the un-primed content on the
    /// buffer that never got the full fill — visible as "lines
    /// alternating" when LTDC switches between buffers.
    seed: Option<Rect>,
    seed_remaining: u8,
}

impl Compositor {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            dirty: Vec::new(),
            prev_dirty: Vec::new(),
            seed: None,
            seed_remaining: 0,
        }
    }

    /// Push a layer on top of the current stack.
    pub fn push(&mut self, layer: Box<dyn Layer>) {
        self.layers.push(layer);
    }

    /// Borrow a layer mutably by index (z-order from bottom).
    pub fn layer_mut(&mut self, idx: usize) -> Option<&mut (dyn Layer + 'static)> {
        self.layers.get_mut(idx).map(|b| b.as_mut())
    }

    /// Force the next **two** frames to repaint the entire surface.
    /// Two frames is the minimum needed to seed both sides of a
    /// front/back double buffer when the present task swaps on every
    /// buf_ready signal.
    pub fn prime_full(&mut self, surface: Rect) {
        self.seed = Some(surface);
        self.seed_remaining = 2;
    }

    /// Tick all layers, collect their dirty rects, render from bottom
    /// to top into `surface`. The caller supplies the blitter — any
    /// implementation of `Blitter` works, so the same Compositor code
    /// runs with `CpuBlitter` (software) or `FreeRtosDma2dBlitter`
    /// (hardware-accelerated + semaphore-blocked).
    pub fn render_frame(&mut self, blitter: &mut dyn Blitter, surface: &mut Surface) -> FrameStats {
        let t0 = cyccnt();

        self.dirty.clear();

        // 1. Double-buffer sync: replay the previous frame's dirty
        //    rects so the buffer we just got from the present task
        //    (which has two-frame-old content) converges.
        self.dirty.extend_from_slice(&self.prev_dirty);

        // 2. Seed — full-surface repaint for `seed_remaining` frames.
        //    Primes both buffers of a double-buffered present pipeline
        //    (see `prime_full` docs for the rationale).
        if self.seed_remaining > 0 {
            if let Some(r) = self.seed {
                self.dirty.push(r);
            }
            self.seed_remaining -= 1;
            if self.seed_remaining == 0 {
                self.seed = None;
            }
        }

        // 3. Tick each layer, collect its dirty rects.
        let mut this_frame: Vec<Rect> = Vec::new();
        for layer in &mut self.layers {
            layer.tick();
            for r in layer.dirty() {
                this_frame.push(*r);
            }
            layer.clear_dirty();
        }
        self.dirty.extend_from_slice(&this_frame);
        self.prev_dirty = this_frame;

        // 4. Composite — for each dirty rect, walk layers bottom-up.
        let mut pixels = 0u32;
        for rect in &self.dirty {
            pixels = pixels.saturating_add(rect.w.saturating_mul(rect.h));
            for layer in &self.layers {
                layer.render(blitter, surface, *rect);
            }
        }

        let t1 = cyccnt();
        FrameStats {
            pixels_touched: pixels,
            rect_count: self.dirty.len() as u32,
            cycles: t1.wrapping_sub(t0),
        }
    }
}

#[inline(always)]
fn cyccnt() -> u32 {
    // Typed access to the Cortex-M DWT cycle counter at 0xE000_1004.
    // The cortex-m crate's `DWT::cycle_count()` is a safe wrapper around
    // the same volatile read, with `#[doc(hidden)]` PAC handling for
    // the underlying MMIO base — replaces the prior raw cast that the
    // discipline scanner caught under `raw_mmio_cast` / `raw_addr_cast`.
    cortex_m::peripheral::DWT::cycle_count()
}

/// Intersect two rects. Returns `None` if they don't overlap.
pub fn rect_intersect(a: Rect, b: Rect) -> Option<Rect> {
    let ax2 = a.x.saturating_add(a.w as i32);
    let ay2 = a.y.saturating_add(a.h as i32);
    let bx2 = b.x.saturating_add(b.w as i32);
    let by2 = b.y.saturating_add(b.h as i32);
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let x2 = ax2.min(bx2);
    let y2 = ay2.min(by2);
    if x2 > x && y2 > y {
        Some(Rect {
            x,
            y,
            w: (x2 - x) as u32,
            h: (y2 - y) as u32,
        })
    } else {
        None
    }
}

// ── Demo layers ──────────────────────────────────────────────────────────────

/// Solid color fill covering the whole surface. Has no dirty state of
/// its own — the compositor's `prime_full` seeds its initial paint,
/// and subsequent repaints come only from dirty rects produced by
/// layers above it.
pub struct SolidBackgroundLayer {
    pub color: u32,
}

impl Layer for SolidBackgroundLayer {
    fn render(&self, blitter: &mut dyn Blitter, surface: &mut Surface, clip: Rect) {
        blitter.fill(surface, clip, self.color);
    }
}

/// Rectangular block that moves horizontally each tick, wrapping on
/// the right edge. Dirties both its previous and new positions.
pub struct MotionBlockLayer {
    pub bounds: Rect,
    pub color: u32,
    pub dx: i32,
    pub surface_w: u32,
    pub surface_h: u32,
    dirty_buf: [Rect; 2],
    dirty_len: u8,
}

impl MotionBlockLayer {
    pub fn new(initial: Rect, color: u32, dx: i32, surface: (u32, u32)) -> Self {
        Self {
            bounds: initial,
            color,
            dx,
            surface_w: surface.0,
            surface_h: surface.1,
            dirty_buf: [Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            }; 2],
            dirty_len: 0,
        }
    }
}

impl Layer for MotionBlockLayer {
    fn tick(&mut self) {
        let old = self.bounds;

        let mut nx = self.bounds.x + self.dx;
        let block_w = self.bounds.w as i32;
        if nx >= self.surface_w as i32 {
            nx = -block_w;
        } else if nx + block_w <= 0 {
            nx = self.surface_w as i32;
        }
        self.bounds = Rect { x: nx, ..old };

        // Record both old and new bounds as dirty so the background
        // gets repainted over the uncovered old position.
        self.dirty_buf[0] = old;
        self.dirty_buf[1] = self.bounds;
        self.dirty_len = 2;
    }

    fn dirty(&self) -> &[Rect] {
        &self.dirty_buf[..self.dirty_len as usize]
    }

    fn clear_dirty(&mut self) {
        self.dirty_len = 0;
    }

    fn render(&self, blitter: &mut dyn Blitter, surface: &mut Surface, clip: Rect) {
        if let Some(isect) = rect_intersect(self.bounds, clip) {
            blitter.fill(surface, isect, self.color);
        }
    }
}
