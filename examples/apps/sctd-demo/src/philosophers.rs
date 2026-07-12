// SPDX-License-Identifier: MIT
//! Philosophers Table — a live state visualization of a Dining Philosophers
//! machine, drawn over the imported tutorial illustration.
//!
//! The backdrop is the authentic tutorial table art (`HERO_DP`, transcoded from
//! `Qt/DiningPhilosophers/Images/dininig_philosophers.svg` per SCTD-00 §6.4).
//! On top of it, one disc per philosopher is recolored to reflect that seat's
//! live state (thinking / hungry / waiting / eating, or empty for the
//! interactive machine's unoccupied chairs) — mirroring the Qt tutorial, whose
//! philosopher nodes change color as the machine runs.
//!
//! The five disc centers are the red-philosopher coordinates extracted directly
//! from the source SVG (viewBox 0 0 600 600), so the overlay registers exactly
//! on the backdrop regardless of the box size.

use core::cell::RefCell;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};

/// Live state of one seat, unified across the faithful and interactive machines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatState {
    /// No philosopher seated (interactive machine only).
    Empty,
    /// Thinking — not yet hungry.
    Thinking,
    /// Hungry — wants to eat, has not acquired both forks.
    Hungry,
    /// Waiting — holds one fork, blocked on the second.
    Waiting,
    /// Eating — holds both forks.
    Eating,
}

impl SeatState {
    /// Fill color for the philosopher disc in this state.
    fn fill(self) -> Color {
        match self {
            SeatState::Empty => Color(38, 42, 52, 255),
            SeatState::Thinking => Color(64, 128, 214, 255),
            SeatState::Hungry => Color(232, 170, 60, 255),
            SeatState::Waiting => Color(220, 120, 70, 255),
            SeatState::Eating => Color(72, 200, 120, 255),
        }
    }
}

/// Source-SVG viewBox extent (square), used to scale the overlay to the box.
const VIEW: i32 = 600;
/// Red-philosopher centers in viewBox units, extracted from the source SVG.
/// Index 0 → seat 1 … index 4 → seat 5.
const PHIL_CENTERS: [(i32, i32); 5] = [(200, 147), (361, 148), (407, 301), (154, 298), (280, 394)];
/// Philosopher disc radius in viewBox units (from the SVG ellipse `rx`).
const PHIL_R_VIEW: i32 = 37;

struct Decoded {
    pixels: alloc::vec::Vec<Color>,
    width: u32,
    height: u32,
}

/// The live philosophers table: tutorial-art backdrop + per-seat state discs.
pub struct PhilosophersTable {
    bounds: Rect,
    backdrop: &'static [u8],
    states: [SeatState; 5],
    visible: bool,
    generation: u64,
    decoded: RefCell<Option<Decoded>>,
}

impl PhilosophersTable {
    /// Create a table that draws `backdrop` (the DP illustration RLE) within
    /// `bounds`, hidden until [`set_states`](Self::set_states) supplies data.
    pub fn new(bounds: Rect, backdrop: &'static [u8]) -> Self {
        Self {
            bounds,
            backdrop,
            states: [SeatState::Thinking; 5],
            visible: false,
            generation: 0,
            decoded: RefCell::new(None),
        }
    }

    /// Push the latest seat states. `Some` shows the table and recolors the
    /// discs; `None` (a non-DP machine) hides it.
    pub fn set_states(&mut self, states: Option<[SeatState; 5]>) {
        match states {
            Some(s) => {
                if !self.visible || self.states != s {
                    self.generation = self.generation.wrapping_add(1);
                }
                self.states = s;
                self.visible = true;
            }
            None => {
                if self.visible {
                    self.generation = self.generation.wrapping_add(1);
                }
                self.visible = false;
            }
        }
    }

    /// Monotonic revision incremented only when visible table state changes.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether the table is currently shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Test-only: the seat states last pushed.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn debug_states(&self) -> [SeatState; 5] {
        self.states
    }

    fn decode(rle: &[u8]) -> Option<Decoded> {
        let (width, height, palette_bytes, stream) = rlvgl_decomp::parse_rle_blob(rle).ok()?;
        let palette_len = palette_bytes.len() / 2;
        let mut palette = alloc::vec![0u16; palette_len];
        for i in 0..palette_len {
            palette[i] = u16::from_le_bytes([palette_bytes[i * 2], palette_bytes[i * 2 + 1]]);
        }
        let rgba =
            rlvgl_decomp::decode_rgba(width as usize, height as usize, &palette, stream).ok()?;
        let pixels = rgba
            .chunks_exact(4)
            .map(|c| Color(c[0], c[1], c[2], c[3]))
            .collect();
        Some(Decoded {
            pixels,
            width: width as u32,
            height: height as u32,
        })
    }

    /// Test-only: force-decode and return the backdrop dimensions.
    #[cfg(test)]
    pub fn debug_backdrop_size(&self) -> Option<(u32, u32)> {
        let mut d = self.decoded.borrow_mut();
        if d.is_none() {
            *d = Self::decode(self.backdrop);
        }
        d.as_ref().map(|e| (e.width, e.height))
    }

    /// Integer square root (no_std, no libm).
    fn isqrt(n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
    }

    /// Filled disc via horizontal scanline spans (the renderer only guarantees
    /// `fill_rect`; `draw_pixels` would be per-pixel and slower here).
    fn fill_disc(renderer: &mut dyn Renderer, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        for dy in -r..=r {
            let half = Self::isqrt(r * r - dy * dy);
            renderer.fill_rect(
                Rect {
                    x: cx - half,
                    y: cy + dy,
                    width: 2 * half + 1,
                    height: 1,
                },
                color,
            );
        }
    }
}

impl Widget for PhilosophersTable {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        {
            let mut d = self.decoded.borrow_mut();
            if d.is_none() {
                *d = Self::decode(self.backdrop);
            }
        }
        let decoded = self.decoded.borrow();
        let Some(entry) = decoded.as_ref() else {
            return;
        };

        // Backdrop: the tutorial table illustration, top-left aligned in bounds.
        let bx = self.bounds.x;
        let by = self.bounds.y;
        renderer.draw_pixels((bx, by), &entry.pixels, entry.width, entry.height);

        // Overlay one state-colored disc per philosopher, registered on the
        // backdrop via the SVG-extracted centers scaled to the rendered size.
        let w = entry.width as i32;
        let outline = Color(12, 14, 20, 255);
        let ring = Color(96, 104, 120, 255);
        for (i, &(vx, vy)) in PHIL_CENTERS.iter().enumerate() {
            let cx = bx + vx * w / VIEW;
            let cy = by + vy * w / VIEW;
            let r = (PHIL_R_VIEW * w / VIEW).max(4);
            let state = self.states[i];
            // Dark outline frames the disc against the busy backdrop.
            Self::fill_disc(renderer, cx, cy, r, outline);
            if state == SeatState::Empty {
                // Vacant chair: hollow — dark fill with a light ring.
                Self::fill_disc(renderer, cx, cy, r - 1, ring);
                Self::fill_disc(renderer, cx, cy, r - 3, state.fill());
            } else {
                Self::fill_disc(renderer, cx, cy, r - 2, state.fill());
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
