//! Borrowed-slice polyline widget.
//!
//! The line widget draws adjacent point pairs as anti-aliased strokes. Points
//! are concrete pixel offsets relative to the widget bounds; percent
//! coordinates are intentionally deferred by LPAR-11.

use rlvgl_core::event::Event;
use rlvgl_core::raster::PointF;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

/// Pixel point relative to a [`Line`] widget's bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Horizontal offset from the widget bounds' left edge.
    pub x: i32,
    /// Vertical offset from the widget bounds' top edge.
    pub y: i32,
}

/// Connected line-segment widget backed by a borrowed point slice.
///
/// The lifetime parameter ties the widget to its point storage. The widget
/// never stores raw pointers and never copies the point slice.
#[derive(Debug)]
pub struct Line<'a> {
    bounds: Rect,
    points: &'a [Point],
    y_invert: bool,
    /// Style used for the line stroke.
    ///
    /// `border_color` supplies the stroke color, `border_width` supplies the
    /// stroke width in pixels, and `alpha` modulates the final stroke alpha.
    pub style: Style,
}

impl<'a> Line<'a> {
    /// Create a line widget with borrowed point storage.
    ///
    /// The default stroke is one pixel wide using [`Style::default`]'s border
    /// color so a newly created line is visible without additional styling.
    pub fn new(bounds: Rect, points: &'a [Point]) -> Self {
        let mut style = Style::default();
        style.border_width = 1;
        Self {
            bounds,
            points,
            y_invert: false,
            style,
        }
    }

    /// Return the currently borrowed point slice.
    pub fn points(&self) -> &'a [Point] {
        self.points
    }

    /// Replace the borrowed point slice.
    pub fn set_points(&mut self, points: &'a [Point]) {
        self.points = points;
    }

    /// Enable or disable bottom-origin y coordinate mapping.
    pub fn set_y_invert(&mut self, y_invert: bool) {
        self.y_invert = y_invert;
    }

    /// Return whether bottom-origin y coordinate mapping is enabled.
    pub fn y_invert(&self) -> bool {
        self.y_invert
    }

    fn point_to_framebuffer(&self, point: Point) -> PointF {
        let y = if self.y_invert {
            self.bounds.height - point.y
        } else {
            point.y
        };
        PointF::new((self.bounds.x + point.x) as f32, (self.bounds.y + y) as f32)
    }
}

impl Widget for Line<'_> {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.style.border_width == 0 || self.style.alpha == 0 {
            return;
        }

        let color = self.style.border_color.with_alpha(self.style.alpha);
        let width = self.style.border_width as f32;
        for pair in self.points.windows(2) {
            renderer.stroke_line_aa(
                self.point_to_framebuffer(pair[0]),
                self.point_to_framebuffer(pair[1]),
                width,
                color,
            );
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use rlvgl_core::font::ShapedText;
    use rlvgl_core::widget::Color;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Stroke {
        a: PointF,
        b: PointF,
        width: f32,
        color: Color,
    }

    struct RecordingRenderer {
        strokes: Vec<Stroke>,
    }

    impl RecordingRenderer {
        fn new() -> Self {
            Self {
                strokes: Vec::new(),
            }
        }
    }

    impl Renderer for RecordingRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}

        fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}

        fn draw_text_shaped(
            &mut self,
            _shaped: &ShapedText<'_>,
            _origin: (i32, i32),
            _color: Color,
        ) {
        }

        fn stroke_line_aa(&mut self, a: PointF, b: PointF, width: f32, color: Color) {
            self.strokes.push(Stroke { a, b, width, color });
        }
    }

    #[test]
    fn draws_adjacent_segments_with_style_stroke() {
        let points = [
            Point { x: 1, y: 2 },
            Point { x: 11, y: 12 },
            Point { x: 21, y: 2 },
        ];
        let mut line = Line::new(
            Rect {
                x: 10,
                y: 20,
                width: 40,
                height: 30,
            },
            &points,
        );
        line.style.border_color = Color(10, 20, 30, 200);
        line.style.border_width = 3;
        line.style.alpha = 128;

        let mut renderer = RecordingRenderer::new();
        line.draw(&mut renderer);

        assert_eq!(
            renderer.strokes,
            alloc::vec![
                Stroke {
                    a: PointF::new(11.0, 22.0),
                    b: PointF::new(21.0, 32.0),
                    width: 3.0,
                    color: Color(10, 20, 30, 200).with_alpha(128),
                },
                Stroke {
                    a: PointF::new(21.0, 32.0),
                    b: PointF::new(31.0, 22.0),
                    width: 3.0,
                    color: Color(10, 20, 30, 200).with_alpha(128),
                },
            ]
        );
    }

    #[test]
    fn y_invert_maps_against_bounds_height() {
        let points = [Point { x: 0, y: 0 }, Point { x: 10, y: 7 }];
        let mut line = Line::new(
            Rect {
                x: 4,
                y: 5,
                width: 30,
                height: 20,
            },
            &points,
        );
        line.set_y_invert(true);

        let mut renderer = RecordingRenderer::new();
        line.draw(&mut renderer);

        assert_eq!(
            renderer.strokes,
            alloc::vec![Stroke {
                a: PointF::new(4.0, 25.0),
                b: PointF::new(14.0, 18.0),
                width: 1.0,
                color: line.style.border_color.with_alpha(line.style.alpha),
            }]
        );
    }

    #[test]
    fn set_points_and_set_bounds_update_draw_inputs() {
        let old_points = [Point { x: 0, y: 0 }, Point { x: 1, y: 1 }];
        let new_points = [Point { x: 2, y: 3 }, Point { x: 4, y: 5 }];
        let mut line = Line::new(
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            &old_points,
        );

        line.set_points(&new_points);
        line.set_bounds(Rect {
            x: 7,
            y: 11,
            width: 20,
            height: 30,
        });

        assert_eq!(line.points(), &new_points);
        assert_eq!(
            line.bounds(),
            Rect {
                x: 7,
                y: 11,
                width: 20,
                height: 30,
            }
        );

        let mut renderer = RecordingRenderer::new();
        line.draw(&mut renderer);

        assert_eq!(
            renderer.strokes,
            alloc::vec![Stroke {
                a: PointF::new(9.0, 14.0),
                b: PointF::new(11.0, 16.0),
                width: 1.0,
                color: line.style.border_color.with_alpha(line.style.alpha),
            }]
        );
    }

    #[test]
    fn zero_width_or_short_point_lists_do_not_stroke() {
        let points = [Point { x: 0, y: 0 }];
        let mut line = Line::new(
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            &points,
        );
        let mut renderer = RecordingRenderer::new();
        line.draw(&mut renderer);
        assert!(renderer.strokes.is_empty());

        let points = [Point { x: 0, y: 0 }, Point { x: 5, y: 5 }];
        line.set_points(&points);
        line.style.border_width = 0;
        line.draw(&mut renderer);
        assert!(renderer.strokes.is_empty());
    }
}
