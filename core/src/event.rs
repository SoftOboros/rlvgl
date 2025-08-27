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
    /// A keyboard key was pressed.
    KeyDown {
        /// Key that was pressed.
        key: Key,
    },
    /// A keyboard key was released.
    KeyUp {
        /// Key that was released.
        key: Key,
    },
}

/// Identifiers for keyboard keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Escape key.
    Escape,
    /// Enter/Return key.
    Enter,
    /// Spacebar key.
    Space,
    /// Up arrow key.
    ArrowUp,
    /// Down arrow key.
    ArrowDown,
    /// Left arrow key.
    ArrowLeft,
    /// Right arrow key.
    ArrowRight,
    /// Function key with the given index (1–12).
    Function(u8),
    /// Printable character key.
    Character(char),
    /// Any other key not explicitly covered above.
    Other(u32),
}
