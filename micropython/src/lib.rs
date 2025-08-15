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

/// Remove a node at a given z-index.
///
/// # Parameters
/// - `z`: Z-index layer to remove.
pub fn stack_remove(_z: ZIndex) {}

/// Replace the node at a given z-index.
///
/// # Parameters
/// - `z`: Z-index layer.
/// - `node`: Replacement node.
pub fn stack_replace(_z: ZIndex, _node: NodeSpec) {}

/// Clear the entire display stack.
pub fn stack_clear() {}

/// Present the current frame boundary.
pub fn present() {}

/// Retrieve statistics for debugging.
pub fn stats() {}

/// C-ABI: initialize the binding.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_init() {
    init();
}

/// C-ABI: forward an input event.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_notify_input(event: InputEvent) {
    notify_input(event);
}

/// C-ABI: add a node to the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_add(z: ZIndex, node: NodeSpec) {
    stack_add(z, node);
}

/// C-ABI: remove a node from the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_remove(z: ZIndex) {
    stack_remove(z);
}

/// C-ABI: replace a node in the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_replace(z: ZIndex, node: NodeSpec) {
    stack_replace(z, node);
}

/// C-ABI: clear the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_clear() {
    stack_clear();
}

/// C-ABI: present the current frame.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_present() {
    present();
}

/// C-ABI: retrieve statistics.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stats() {
    stats();
}
