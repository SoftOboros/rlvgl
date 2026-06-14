//! Basic text label.
use alloc::string::String;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr};
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
    /// Font assignment for this label (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
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
            font: WidgetFont::new(),
        }
    }

    /// Assign the font used to render this label (FONT-00 §5).
    ///
    /// Pass any process-lifetime [`FontMetrics`] — e.g. a `PackedFont` for
    /// anti-aliased text. With no assignment the label renders with the
    /// built-in `FONT_6X10`.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    /// Update the text displayed by the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Retrieve the current label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Update text color for this label.
    ///
    /// Prefer migrated `TextStyle` plumbing for future callers; this method
    /// is the compatibility path while upstream migration continues.
    #[allow(deprecated)]
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }

    /// Return text color blended by widget alpha.
    ///
    /// Prefer `TextStyle` plumbing for long-lived callers; this helper keeps
    /// existing widget implementations warning-free until migration completes.
    #[allow(deprecated)]
    pub fn text_color_with_alpha(&self, alpha: u8) -> Color {
        self.text_color.with_alpha(alpha)
    }

    /// Draw this label using an explicit font metrics backend.
    ///
    /// The shaped text path clips glyph coverage to the label bounds. The
    /// `Widget::draw` impl calls this with the [`set_font`](Self::set_font)
    /// assignment resolved via `WidgetFont` (FONT-00 §5); callers may also
    /// invoke it directly with any `&dyn FontMetrics`.
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
        self.draw_with_font(renderer, self.font.resolve());
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
