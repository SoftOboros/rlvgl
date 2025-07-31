//! Regression tests for zero-range sliders.
use rlvgl_core::{event::Event, widget::Rect, widget::Widget};
use rlvgl_widgets::slider::Slider;

#[test]
fn slider_zero_range_behavior() {
    let rect = Rect {
        x: 10,
        y: 10,
        width: 30,
        height: 10,
    };
    let mut s = Slider::new(rect, 5, 5);
    assert_eq!(s.bounds().x, rect.x);
    assert_eq!(s.bounds().y, rect.y);
    assert_eq!(s.bounds().width, rect.width);
    assert_eq!(s.bounds().height, rect.height);

    // event inside should keep value at min when range is zero
    let evt = Event::PointerUp { x: 25, y: 15 };
    assert!(s.handle_event(&evt));
    assert_eq!(s.value(), 5);
}
