/// Basic UI events used for widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Called periodically to advance animations or timers.
    Tick,
    /// A pointer (mouse or touch) was pressed at the given coordinates.
    PointerDown { x: i32, y: i32 },
    /// The pointer was released.
    PointerUp { x: i32, y: i32 },
    /// The pointer moved while still pressed.
    PointerMove { x: i32, y: i32 },
}
