//! Tests for button click boundaries.
use rlvgl_core::{event::Event, widget::Rect, widget::Widget};
use rlvgl_widgets::button::Button;
use std::{cell::Cell, rc::Rc};

#[test]
fn button_clicks_on_corners() {
    let rect = Rect {
        x: 10,
        y: 20,
        width: 30,
        height: 40,
    };
    let counter = Rc::new(Cell::new(0));
    let mut btn = Button::new("ok", rect);
    let c = counter.clone();
    btn.set_on_click(move |_| c.set(c.get() + 1));

    for &(x, y) in &[
        (rect.x, rect.y),
        (rect.x + rect.width - 1, rect.y),
        (rect.x, rect.y + rect.height - 1),
        (rect.x + rect.width - 1, rect.y + rect.height - 1),
    ] {
        assert!(btn.handle_event(&Event::PointerUp { x, y }));
    }
    assert_eq!(counter.get(), 4);
}
