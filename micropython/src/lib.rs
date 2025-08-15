#![no_std]
#![deny(missing_docs)]

//! MicroPython bindings for rlvgl.
//!
//! This crate is platform-agnostic; board-specific integrations such as
//! STM32H747I-DISCO are enabled through feature flags.

use rlvgl_api::{InputEvent, NodeSpec, ZIndex};

/// Initialize the MicroPython binding.
pub fn init() {}

/// Notify an input event from the platform layer.
///
/// # Parameters
/// - `event`: The input event to forward.
pub fn notify_input(_event: InputEvent) {}

/// Add a node to the display stack.
///
/// # Parameters
/// - `z`: Z-index layer.
/// - `node`: The node to add.
pub fn stack_add(_z: ZIndex, _node: NodeSpec) {}
