//! Animation primitives such as fades and slides.
//!
//! These helpers mirror LVGL's animation system while using safe Rust
//! abstractions where possible.

use crate::style::Style;
use crate::widget::{Color, Rect};

/// Simple linear fade animation for a style's background color.
///
/// The animation owns a mutable pointer to the [`Style`] being modified. This
/// keeps the API lightweight for `no_std` targets at the cost of requiring
/// unsafe access internally.
pub struct Fade {
    style: *mut Style,
    start: Color,
    end: Color,
    duration_ms: u32,
    elapsed: u32,
}

impl Fade {
    /// Create a new fade animation.
    pub fn new(style: &mut Style, start: Color, end: Color, duration_ms: u32) -> Self {
        Self {
            style: style as *mut Style,
            start,
            end,
            duration_ms,
            elapsed: 0,
        }
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = core::cmp::min(self.elapsed + delta_ms, self.duration_ms);
        let progress = self.elapsed as f32 / self.duration_ms as f32;
        let lerp = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * progress;
        unsafe {
            (*self.style).bg_color = Color(
                lerp(self.start.0, self.end.0) as u8,
                lerp(self.start.1, self.end.1) as u8,
                lerp(self.start.2, self.end.2) as u8,
            );
        }
    }

    /// Returns `true` when the animation has reached its end point.
    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration_ms
    }
}

/// Simple linear slide animation for a [`Rect`].
pub struct Slide {
    rect: *mut Rect,
    start: Rect,
    end: Rect,
    duration_ms: u32,
    elapsed: u32,
}

impl Slide {
    /// Create a new slide animation.
    pub fn new(rect: &mut Rect, start: Rect, end: Rect, duration_ms: u32) -> Self {
        Self {
            rect: rect as *mut Rect,
            start,
            end,
            duration_ms,
            elapsed: 0,
        }
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed = core::cmp::min(self.elapsed + delta_ms, self.duration_ms);
        let p = self.elapsed as f32 / self.duration_ms as f32;
        let lerp = |a: i32, b: i32| a as f32 + (b as f32 - a as f32) * p;
        unsafe {
            *self.rect = Rect {
                x: lerp(self.start.x, self.end.x) as i32,
                y: lerp(self.start.y, self.end.y) as i32,
                width: lerp(self.start.width, self.end.width) as i32,
                height: lerp(self.start.height, self.end.height) as i32,
            };
        }
    }

    /// Returns `true` once the slide has finished.
    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration_ms
    }
}

/// Animation timeline that updates multiple animations at once.
pub struct Timeline {
    fades: alloc::vec::Vec<Fade>,
    slides: alloc::vec::Vec<Slide>,
}

impl Timeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Self {
            fades: alloc::vec::Vec::new(),
            slides: alloc::vec::Vec::new(),
        }
    }

    /// Add a [`Fade`] animation to the timeline.
    pub fn add_fade(&mut self, fade: Fade) {
        self.fades.push(fade);
    }
    /// Add a [`Slide`] animation to the timeline.
    pub fn add_slide(&mut self, slide: Slide) {
        self.slides.push(slide);
    }

    /// Advance all animations by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u32) {
        for fade in &mut self.fades {
            fade.tick(delta_ms);
        }
        for slide in &mut self.slides {
            slide.tick(delta_ms);
        }
        self.fades.retain(|f| !f.finished());
        self.slides.retain(|s| !s.finished());
    }

    /// Returns `true` if no animations remain in the timeline.
    pub fn is_empty(&self) -> bool {
        self.fades.is_empty() && self.slides.is_empty()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
