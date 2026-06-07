//! Theme application tests.
use rlvgl_core::style::Style;
use rlvgl_core::theme::{DarkTheme, LightTheme, Theme};
use rlvgl_core::widget::Color;

struct StyledFixture {
    style: Style,
}

impl StyledFixture {
    fn new() -> Self {
        Self {
            style: Style::default(),
        }
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

#[test]
fn light_theme_applies_defaults() {
    let mut style = Style {
        bg_color: Color(10, 20, 30, 255),
        border_color: Color(5, 5, 5, 255),
        ..Default::default()
    };

    LightTheme.apply(&mut style);

    assert_eq!(style.bg_color, Color(255, 255, 255, 255));
    assert_eq!(style.border_color, Color(0, 0, 0, 255));
}

#[test]
fn dark_theme_applies_defaults() {
    let mut style = Style {
        bg_color: Color(250, 250, 250, 255),
        border_color: Color(10, 10, 10, 255),
        ..Default::default()
    };

    DarkTheme.apply(&mut style);

    assert_eq!(style.bg_color, Color(0, 0, 0, 255));
    assert_eq!(style.border_color, Color(255, 255, 255, 255));
}

#[test]
fn theme_updates_widget_style() {
    let mut widget = StyledFixture::new();

    DarkTheme.apply(widget.style_mut());

    assert_eq!(widget.style().bg_color, Color(0, 0, 0, 255));
    assert_eq!(widget.style().border_color, Color(255, 255, 255, 255));
}
