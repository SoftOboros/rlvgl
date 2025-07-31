//! Tests for slider event handling.
use rlvgl_core::widget::Widget;
use rlvgl_core::{event::Event, widget::Rect};
use rlvgl_widgets::slider::Slider;

#[test]
fn slider_clamp_and_event_handling() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 20,
    };
    let mut s = Slider::new(rect, 0, 100);

    s.set_value(-5);
    assert_eq!(s.value(), 0);
    s.set_value(150);
    assert_eq!(s.value(), 100);

    s.set_value(50);
    // Event outside vertical bounds should not change value
    let evt = Event::PointerUp { x: 10, y: -1 };
    assert!(!s.handle_event(&evt));
    assert_eq!(s.value(), 50);

    // Event at rightmost edge selects near max
    let evt = Event::PointerUp {
        x: rect.x + rect.width - 1,
        y: rect.y + rect.height / 2,
    };
    assert!(s.handle_event(&evt));
    assert_eq!(s.value(), 99);
}
