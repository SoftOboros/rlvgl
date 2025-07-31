//! Theme application tests.
use rlvgl_core::style::Style;
use rlvgl_core::theme::{DarkTheme, LightTheme, Theme};
use rlvgl_core::widget::Color;
use rlvgl_core::widget::Rect;
use rlvgl_widgets::button::Button;

#[test]
fn light_theme_applies_defaults() {
    let mut style = Style::default();
    style.bg_color = Color(10, 20, 30);
    style.border_color = Color(5, 5, 5);

    LightTheme.apply(&mut style);

    assert_eq!(style.bg_color, Color(255, 255, 255));
    assert_eq!(style.border_color, Color(0, 0, 0));
}

#[test]
fn dark_theme_applies_defaults() {
    let mut style = Style::default();
    style.bg_color = Color(250, 250, 250);
    style.border_color = Color(10, 10, 10);

    DarkTheme.apply(&mut style);

    assert_eq!(style.bg_color, Color(0, 0, 0));
    assert_eq!(style.border_color, Color(255, 255, 255));
}

#[test]
fn theme_updates_widget_style() {
    let mut button = Button::new(
        "ok",
        Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        },
    );

    DarkTheme.apply(button.style_mut());

    assert_eq!(button.style().bg_color, Color(0, 0, 0));
    assert_eq!(button.style().border_color, Color(255, 255, 255));
}
