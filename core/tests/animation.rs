//! Tests for animation primitives.
use rlvgl_core::animation::{Fade, Slide, Timeline};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect};

#[test]
fn fade_updates_bg_color() {
    let mut style = Style::default();
    style.bg_color = Color(0, 0, 0);
    let start_color = style.bg_color;
    let mut timeline = Timeline::new();
    timeline.add_fade(Fade::new(&mut style, start_color, Color(255, 0, 0), 100));

    timeline.tick(50);
    assert_eq!(style.bg_color, Color(127, 0, 0));
    assert!(!timeline.is_empty());

    timeline.tick(50);
    assert_eq!(style.bg_color, Color(255, 0, 0));
    assert!(timeline.is_empty());
}

#[test]
fn slide_moves_rect() {
    let mut rect = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let start = rect;
    let end = Rect {
        x: 10,
        y: 0,
        width: 10,
        height: 10,
    };
    let mut timeline = Timeline::new();
    timeline.add_slide(Slide::new(&mut rect, start, end, 100));

    timeline.tick(30);
    assert_eq!(rect.x, 3);
    assert!(!timeline.is_empty());

    timeline.tick(70);
    assert_eq!(rect.x, 10);
    assert!(timeline.is_empty());
}
