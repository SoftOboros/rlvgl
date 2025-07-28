use rlvgl_core::event::Event;
use rlvgl_platform::input::{DummyInput, InputDevice};

struct VecInput {
    events: Vec<Event>,
}

impl InputDevice for VecInput {
    fn poll(&mut self) -> Option<Event> {
        if self.events.is_empty() {
            None
        } else {
            Some(self.events.remove(0))
        }
    }
}

#[test]
fn dummy_input_returns_none() {
    let mut input = DummyInput;
    assert!(input.poll().is_none());
}

#[test]
fn vec_input_yields_events() {
    let mut input = VecInput {
        events: vec![
            Event::PointerDown { x: 1, y: 2 },
            Event::PointerUp { x: 1, y: 2 },
        ],
    };
    assert_eq!(input.poll(), Some(Event::PointerDown { x: 1, y: 2 }));
    assert_eq!(input.poll(), Some(Event::PointerUp { x: 1, y: 2 }));
    assert_eq!(input.poll(), None);
}
