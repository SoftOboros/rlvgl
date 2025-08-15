#![no_std]
#![deny(missing_docs)]

//! Shared API definitions for rlvgl bindings.
//!
//! This crate is feature-flagged for multiple environments:
//! - `micropython`
//! - `cpython`
//! - `cm4`
//! - `sim`

/// Semantic version of the shared API.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiVersion {
    /// Major version component.
    pub major: u8,
    /// Minor version component.
    pub minor: u8,
    /// Patch version component.
    pub patch: u8,
}

/// Current API version following SemVer.
pub const API_VERSION: ApiVersion = ApiVersion {
    major: 0,
    minor: 1,
    patch: 0,
};

/// Z-index for stacking nodes.
pub type ZIndex = i16;

/// Supported node kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    /// Solid rectangle node.
    Rect,
    /// Text label node.
    Text,
}

/// Rectangle node specification.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectSpec {
    /// Horizontal position.
    pub x: i16,
    /// Vertical position.
    pub y: i16,
    /// Width of the rectangle.
    pub w: u16,
    /// Height of the rectangle.
    pub h: u16,
    /// Fill color in ARGB8888 format.
    pub color: u32,
}

/// Text node specification.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSpec {
    /// Horizontal position.
    pub x: i16,
    /// Vertical position.
    pub y: i16,
    /// Pointer to a NUL-terminated UTF-8 string.
    pub text: *const u8,
    /// Foreground color in ARGB8888 format.
    pub fg: u32,
    /// Background color in ARGB8888 format.
    pub bg: u32,
}

/// Node specification combining all supported node types.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    /// Node type selector.
    pub kind: NodeKind,
    /// Rectangle parameters when `kind == Rect`.
    pub rect: RectSpec,
    /// Text parameters when `kind == Text`.
    pub text: TextSpec,
}

/// Supported input event kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InputKind {
    /// A press or touch event.
    Press,
    /// A release or end of touch.
    Release,
}

/// Minimal input event representation.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEvent {
    /// Kind of the event.
    pub kind: InputKind,
    /// Horizontal coordinate if applicable.
    pub x: i16,
    /// Vertical coordinate if applicable.
    pub y: i16,
    /// Key code for keyboard events.
    pub key: u32,
}
