#![no_std]
#![deny(missing_docs)]

//! MicroPython bindings for rlvgl.
//!
//! This crate is platform-agnostic; board-specific integrations such as
//! STM32H747I-DISCO are enabled through feature flags.

use rlvgl_api::{API_VERSION, ApiVersion, InputEvent, NodeSpec, ZIndex};

/// Status codes returned by binding calls.
///
/// A value of `Ok` indicates success; negative values map to a
/// corresponding MicroPython exception.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MpStatus {
    /// Operation completed successfully.
    Ok = 0,
    /// One or more arguments were invalid.
    InvalidArgument = -1,
    /// An unspecified failure occurred.
    Fail = -2,
}

/// Initialize the MicroPython binding.
pub fn init() -> MpStatus {
    MpStatus::Ok
}

/// Notify an input event from the platform layer.
///
/// # Parameters
/// - `event`: The input event to forward.
pub fn notify_input(_event: InputEvent) -> MpStatus {
    MpStatus::Ok
}

/// Add a node to the display stack.
///
/// # Parameters
/// - `z`: Z-index layer.
/// - `node`: The node to add.
pub fn stack_add(_z: ZIndex, _node: NodeSpec) -> MpStatus {
    MpStatus::Ok
}

/// Remove a node at a given z-index.
///
/// # Parameters
/// - `z`: Z-index layer to remove.
pub fn stack_remove(_z: ZIndex) -> MpStatus {
    MpStatus::Ok
}

/// Replace the node at a given z-index.
///
/// # Parameters
/// - `z`: Z-index layer.
/// - `node`: Replacement node.
pub fn stack_replace(_z: ZIndex, _node: NodeSpec) -> MpStatus {
    MpStatus::Ok
}

/// Clear the entire display stack.
pub fn stack_clear() -> MpStatus {
    MpStatus::Ok
}

/// Present the current frame boundary.
pub fn present() -> MpStatus {
    MpStatus::Ok
}

/// Retrieve statistics for debugging.
pub fn stats() -> MpStatus {
    MpStatus::Ok
}

/// Get the current API version.
pub fn api_version() -> ApiVersion {
    API_VERSION
}

/// C-ABI: initialize the binding.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_init() -> MpStatus {
    init()
}

/// C-ABI: forward an input event.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_notify_input(event: InputEvent) -> MpStatus {
    notify_input(event)
}

/// C-ABI: add a node to the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_add(z: ZIndex, node: NodeSpec) -> MpStatus {
    stack_add(z, node)
}

/// C-ABI: remove a node from the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_remove(z: ZIndex) -> MpStatus {
    stack_remove(z)
}

/// C-ABI: replace a node in the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_replace(z: ZIndex, node: NodeSpec) -> MpStatus {
    stack_replace(z, node)
}

/// C-ABI: clear the display stack.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stack_clear() -> MpStatus {
    stack_clear()
}

/// C-ABI: present the current frame.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_present() -> MpStatus {
    present()
}

/// C-ABI: retrieve statistics.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_stats() -> MpStatus {
    stats()
}

/// C-ABI: return the current API version.
#[unsafe(no_mangle)]
pub extern "C" fn mp_rlvgl_api_version() -> ApiVersion {
    api_version()
}
