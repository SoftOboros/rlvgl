//! Tests for the StyleBuilder API.
use rlvgl_core::style::{Style, StyleBuilder};
use rlvgl_core::widget::Color;

#[test]
fn default_style() {
    let style = Style::default();
    assert_eq!(style.bg_color, Color(255, 255, 255, 255));
    assert_eq!(style.border_color, Color(0, 0, 0, 255));
    assert_eq!(style.border_width, 0);
}

#[test]
fn builder_defaults_match() {
    let style = StyleBuilder::default().build();
    assert_eq!(style, Style::default());
}

#[test]
fn builder_overrides() {
    let custom = StyleBuilder::new()
        .bg_color(Color(10, 20, 30, 255))
        .border_color(Color(40, 50, 60, 255))
        .border_width(3)
        .build();
    assert_eq!(custom.bg_color, Color(10, 20, 30, 255));
    assert_eq!(custom.border_color, Color(40, 50, 60, 255));
    assert_eq!(custom.border_width, 3);
}
