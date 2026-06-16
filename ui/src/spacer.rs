// SPDX-License-Identifier: MIT
//! Spacer primitive for reserving empty space in rlvgl-ui layouts.
//!
//! [`Spacer`] is intentionally non-rendering and non-interactive. It exists so
//! layout code can name intentional gaps using the same widget contract as
//! visible controls.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};

/// Empty widget used to reserve layout space.
pub struct Spacer {
    bounds: Rect,
}

impl Spacer {
    /// Create a spacer with explicit bounds.
    pub fn new(bounds: Rect) -> Self {
        Self { bounds }
    }

    /// Create an origin-based spacer with only a width.
    pub fn width(width: i32) -> Self {
        Self::new(Rect {
            x: 0,
            y: 0,
            width,
            height: 0,
        })
    }

    /// Create an origin-based spacer with only a height.
    pub fn height(height: i32) -> Self {
        Self::new(Rect {
            x: 0,
            y: 0,
            width: 0,
            height,
        })
    }

    /// Create an origin-based square spacer.
    pub fn square(size: i32) -> Self {
        Self::new(Rect {
            x: 0,
            y: 0,
            width: size,
            height: size,
        })
    }
}

impl Widget for Spacer {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, _renderer: &mut dyn Renderer) {}

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacer_constructors_create_expected_bounds() {
        assert_eq!(
            Spacer::width(12).bounds(),
            Rect {
                x: 0,
                y: 0,
                width: 12,
                height: 0,
            }
        );
        assert_eq!(Spacer::height(7).bounds().height, 7);
        assert_eq!(Spacer::square(5).bounds().width, 5);
        assert_eq!(Spacer::square(5).bounds().height, 5);
    }

    #[test]
    fn spacer_adopts_layout_bounds() {
        let mut spacer = Spacer::height(4);
        spacer.set_bounds(Rect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        });

        assert_eq!(
            spacer.bounds(),
            Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }
        );
    }
}
