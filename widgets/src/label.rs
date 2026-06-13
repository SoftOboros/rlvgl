//! Basic text label.
use alloc::string::String;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Simple text element.
pub struct Label {
    bounds: Rect,
    text: String,
    /// Visual style of the label background.
    pub style: Style,
    /// Color used to render the text.
    #[deprecated(note = "use the resolved TextStyle text_color cascade when drawing labels")]
    pub text_color: Color,
}

impl Label {
    /// Create a new label with the provided text and bounds.
    #[allow(deprecated)]
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
        }
    }

    /// Update the text displayed by the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Retrieve the current label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Draw this label using an explicit font metrics backend.
    ///
    /// The shaped text path clips glyph coverage to the label bounds. This is
    /// the insertion point for future font-registry lookup by resolved
    /// [`FontId`](rlvgl_core::font::FontId).
    #[allow(deprecated)]
    pub fn draw_with_font(&self, renderer: &mut dyn Renderer, font: &dyn FontMetrics) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        let metrics = font.line_metrics();
        let baseline = self.bounds.y + metrics.ascent as i32;
        let shaped = shape_text_ltr(font, &self.text, (self.bounds.x, baseline), 0);
        let mut clipped = ClipRenderer::new(renderer, self.bounds);
        clipped.draw_text_shaped(
            &shaped,
            (0, 0),
            self.text_color.with_alpha(self.style.alpha),
        );
    }
}

impl Widget for Label {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let font: &dyn FontMetrics = &FONT_6X10;
        self.draw_with_font(renderer, font);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
