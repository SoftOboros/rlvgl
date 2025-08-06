//! Tests for switch event handling.
use rlvgl_core::{event::Event, widget::Rect, widget::Widget};
use rlvgl_widgets::switch::Switch;

#[test]
fn switch_toggle_and_bounds() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    let mut sw = Switch::new(rect);
    assert_eq!(sw.bounds().x, rect.x);
    assert_eq!(sw.bounds().y, rect.y);
    assert_eq!(sw.bounds().width, rect.width);
    assert_eq!(sw.bounds().height, rect.height);
    let evt = Event::PointerUp { x: 5, y: 5 };
    assert!(sw.handle_event(&evt));
    assert!(sw.is_on());
}
