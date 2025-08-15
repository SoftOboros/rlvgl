#![no_std]
#![deny(missing_docs)]

//! Shared API definitions for rlvgl bindings.

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

/// Minimal node specification.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    /// Node type.
    pub kind: NodeKind,
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
}
