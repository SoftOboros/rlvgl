//! Basic UI events used for widgets.

/// Maximum number of simultaneous touch contacts reported in a single frame.
pub const MAX_TOUCH_POINTS: usize = 5;

/// Per-point event flag reported by a touch controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchState {
    /// New contact this frame.
    Down,
    /// Contact lifted this frame.
    Up,
    /// Contact still held, possibly moved.
    Contact,
}

/// A single touch contact point within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchPoint {
    /// Touch tracking ID assigned by the controller (0–4 for FT5336).
    pub id: u8,
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
    /// Per-point event flag.
    pub state: TouchState,
}

impl Default for TouchPoint {
    fn default() -> Self {
        Self {
            id: 0,
            x: 0,
            y: 0,
            state: TouchState::Up,
        }
    }
}

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
    /// Multi-touch frame with per-point data.
    ///
    /// Emitted when two or more simultaneous contacts are detected.
    /// Only `points[..count]` entries are valid.
    Touch {
        /// Number of active contact points (2..=[`MAX_TOUCH_POINTS`]).
        count: u8,
        /// Per-point data. Entries beyond `count` are meaningless.
        points: [TouchPoint; MAX_TOUCH_POINTS],
    },
    /// Stable contact began (debounced). Use for visual press feedback
    /// such as button highlighting. Emitted by the gesture recognizer,
    /// not by raw hardware input.
    PressDown {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Stable contact released (debounced). The primary "click/tap" action
    /// trigger. Widgets should match this instead of `PointerUp` for
    /// reliable single-fire behavior.
    PressRelease {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Two consecutive short taps detected at the given coordinates.
    /// Emitted by the `DoubleTapRecognizer` when two `PressRelease` events
    /// with short hold durations occur within the double-tap time window.
    DoubleTap {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// A drag gesture started: the pointer moved at least the start
    /// threshold away from its press origin. Emitted once per drag by the
    /// `DragRecognizer`. A contact that starts a drag will not also
    /// produce a `PressRelease` (click-vs-drag suppression — INPUT-00 §6).
    DragStart {
        /// Horizontal coordinate of the move that crossed the threshold.
        x: i32,
        /// Vertical coordinate of the move that crossed the threshold.
        y: i32,
        /// Horizontal coordinate of the originating press.
        origin_x: i32,
        /// Vertical coordinate of the originating press.
        origin_y: i32,
    },
    /// Pointer position update while a drag is in progress. Emitted by
    /// the `DragRecognizer` for every `PointerMove` after `DragStart`.
    DragMove {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// The dragged pointer lifted; drop position for resolution logic.
    /// Emitted by the `DragRecognizer` in place of the raw `PointerUp`.
    DragEnd {
        /// Horizontal coordinate.
        x: i32,
        /// Vertical coordinate.
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
    /// Backspace key — editable fields delete the character before the
    /// caret (WID-00 §5.3).
    Backspace,
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
