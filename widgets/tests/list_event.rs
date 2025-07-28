use rlvgl_core::widget::Widget;
use rlvgl_core::{event::Event, widget::Rect};
use rlvgl_widgets::list::List;

#[test]
fn list_selection_edges() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 32,
    };
    let mut list = List::new(rect);
    list.add_item("a");
    list.add_item("b");

    // click below list
    let evt = Event::PointerUp { x: 10, y: 40 };
    assert!(!list.handle_event(&evt));
    assert_eq!(list.selected(), None);

    // click at boundary for second item
    let evt = Event::PointerUp { x: 5, y: 16 };
    assert!(list.handle_event(&evt));
    assert_eq!(list.selected(), Some(1));

    // click above list
    let evt = Event::PointerUp { x: 5, y: -1 };
    assert!(!list.handle_event(&evt));
    assert_eq!(list.selected(), Some(1));
}
