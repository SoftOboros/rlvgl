//! Command types for the playit test driver.

use rlvgl_core::event::{Event, Key, MAX_TOUCH_POINTS, TouchPoint, TouchState};

/// A test automation command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command<'a> {
    /// Inject an event into the widget tree root.
    Inject(EventSpec),
    /// Inject an event targeted at a tagged widget.
    InjectTagged(&'a str, EventSpec),
    /// Query widget state by tag.
    Query(QuerySpec<'a>),
    /// Dump framebuffer pixels in a region.
    DumpPixels(DumpSpec),
    /// Request runtime telemetry / status.
    Status,
    /// Start the event recorder (clears buffer).
    RecordStart,
    /// Stop recording and dump all entries.
    RecordStop,
    /// Dump recorded entries without stopping.
    RecordDump,
    /// Application-defined extension command (raw payload after `X`).
    Extension(&'a [u8]),
}

/// Specifies an event to inject.  Mirrors [`Event`] but is easier to
/// serialize and does not carry multi-touch arrays.
///
/// Coordinate fields (`x`, `y`) are in the widget/landscape coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum EventSpec {
    /// Advance animations / timers.
    Tick,
    /// Debounced tap (primary action trigger).
    PressRelease { x: i32, y: i32 },
    /// Stable contact began.
    PressDown { x: i32, y: i32 },
    /// Raw pointer pressed.
    PointerDown { x: i32, y: i32 },
    /// Raw pointer released.
    PointerUp { x: i32, y: i32 },
    /// Pointer moved while pressed.
    PointerMove { x: i32, y: i32 },
    /// Two consecutive short taps.
    DoubleTap { x: i32, y: i32 },
    /// Keyboard key pressed.
    KeyDown { key: KeySpec },
    /// Keyboard key released.
    KeyUp { key: KeySpec },
    /// Multi-touch frame with per-point data.
    Touch {
        count: u8,
        points: [TouchPointSpec; MAX_TOUCH_POINTS],
    },
}

/// Per-contact state in a multi-touch frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum TouchStateSpec {
    /// New contact this frame.
    Down,
    /// Contact lifted this frame.
    Up,
    /// Contact still held, possibly moved.
    Contact,
}

/// One contact point in a multi-touch frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchPointSpec {
    /// Touch tracking ID (0–4).
    pub id: u8,
    /// Per-point event flag.
    pub state: TouchStateSpec,
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

impl TouchStateSpec {
    /// Convert to the core [`TouchState`].
    pub fn to_core(self) -> TouchState {
        match self {
            Self::Down => TouchState::Down,
            Self::Up => TouchState::Up,
            Self::Contact => TouchState::Contact,
        }
    }
}

impl Default for TouchPointSpec {
    fn default() -> Self {
        Self {
            id: 0,
            state: TouchStateSpec::Up,
            x: 0,
            y: 0,
        }
    }
}

/// Serializable key identifier, parallel to [`Key`].
///
/// Variant names match the core [`Key`] enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum KeySpec {
    Escape,
    Enter,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Function(u8),
    Character(char),
    Other(u32),
}

/// What to query about a tagged widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuerySpec<'a> {
    /// Get bounds of the widget with the given tag.
    Bounds(&'a str),
    /// Check if a tagged widget exists in the tree.
    Exists(&'a str),
    /// Count children of the tagged widget.
    ChildCount(&'a str),
}

/// Framebuffer dump specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpSpec {
    /// Landscape X origin of the dump window.
    pub x: i32,
    /// Landscape Y origin of the dump window.
    pub y: i32,
    /// Width of the dump window in pixels (clamped 1..=40).
    pub width: u16,
    /// Height of the dump window in pixels (clamped 1..=40).
    pub height: u16,
    /// Number of frames to capture (clamped 1..=4).
    pub frames: u8,
}

// ---------------------------------------------------------------------------
// Conversions to core types
// ---------------------------------------------------------------------------

impl KeySpec {
    /// Convert to the core [`Key`] type.
    pub fn to_key(self) -> Key {
        match self {
            Self::Escape => Key::Escape,
            Self::Enter => Key::Enter,
            Self::Space => Key::Space,
            Self::ArrowUp => Key::ArrowUp,
            Self::ArrowDown => Key::ArrowDown,
            Self::ArrowLeft => Key::ArrowLeft,
            Self::ArrowRight => Key::ArrowRight,
            Self::Function(n) => Key::Function(n),
            Self::Character(c) => Key::Character(c),
            Self::Other(v) => Key::Other(v),
        }
    }
}

impl EventSpec {
    /// Convert to a core [`Event`].
    pub fn to_event(self) -> Event {
        match self {
            Self::Tick => Event::Tick,
            Self::PressRelease { x, y } => Event::PressRelease { x, y },
            Self::PressDown { x, y } => Event::PressDown { x, y },
            Self::PointerDown { x, y } => Event::PointerDown { x, y },
            Self::PointerUp { x, y } => Event::PointerUp { x, y },
            Self::PointerMove { x, y } => Event::PointerMove { x, y },
            Self::DoubleTap { x, y } => Event::DoubleTap { x, y },
            Self::KeyDown { key } => Event::KeyDown { key: key.to_key() },
            Self::KeyUp { key } => Event::KeyUp { key: key.to_key() },
            Self::Touch { count, points } => {
                let mut core_points = [TouchPoint::default(); MAX_TOUCH_POINTS];
                for i in 0..MAX_TOUCH_POINTS {
                    core_points[i] = TouchPoint {
                        id: points[i].id,
                        x: points[i].x,
                        y: points[i].y,
                        state: points[i].state.to_core(),
                    };
                }
                Event::Touch {
                    count,
                    points: core_points,
                }
            }
        }
    }
}
