use rlvgl_core::event::Event;

/// Trait for input devices such as touchscreens or mice.
pub trait InputDevice {
    fn poll(&mut self) -> Option<Event>;
}

/// Dummy input device that yields no events.
pub struct DummyInput;

impl InputDevice for DummyInput {
    fn poll(&mut self) -> Option<Event> {
        None
    }
}

/// Alias used by platform backends for standard events.
pub type InputEvent = Event;
