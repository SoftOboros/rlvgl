use rlvgl_core::{event::Event, widget::Rect, widget::Widget};
use rlvgl_widgets::checkbox::Checkbox;

#[test]
fn checkbox_toggle_and_bounds() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 20,
    };
    let mut cb = Checkbox::new("cb", rect);
    assert_eq!(cb.bounds().x, rect.x);
    assert_eq!(cb.bounds().y, rect.y);
    assert_eq!(cb.bounds().width, rect.width);
    assert_eq!(cb.bounds().height, rect.height);
    // click inside toggles
    let evt = Event::PointerUp { x: 5, y: 5 };
    assert!(cb.handle_event(&evt));
    assert!(cb.is_checked());

    // click outside does nothing
    let evt = Event::PointerUp { x: 30, y: 30 };
    assert!(!cb.handle_event(&evt));
    assert!(cb.is_checked());
}
