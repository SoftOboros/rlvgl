//! Basic UI events used for widgets.

/// Event types propagated through the widget tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Called periodically to advance animations or timers.
    Tick,
    /// A pointer (mouse or touch) was pressed at the given coordinates.
    PointerDown {
        /// Horizontal coordinate relative to the widget origin.
        x: i32,
        /// Vertical coordinate relative to the widget origin.
        y: i32,
    },
    /// The pointer was released.
    PointerUp {
        /// Horizontal coordinate relative to the widget origin.
        x: i32,
        /// Vertical coordinate relative to the widget origin.
        y: i32,
    },
    /// The pointer moved while still pressed.
    PointerMove {
        /// Horizontal coordinate relative to the widget origin.
        x: i32,
        /// Vertical coordinate relative to the widget origin.
        y: i32,
    },
}
