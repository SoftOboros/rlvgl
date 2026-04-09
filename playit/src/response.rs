//! Response types returned by the playit executor.

/// Runtime telemetry data supplied by the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusData {
    /// Main-loop tick count.
    pub tick_count: u32,
    /// LTDC / display present count.
    pub present_count: u32,
}

/// A response emitted by the executor after processing a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response<'a> {
    /// Command executed successfully (no further data).
    Ok,
    /// Error with a reason string.
    Error(&'a str),
    /// Widget bounds result.
    Bounds {
        /// Horizontal position.
        x: i32,
        /// Vertical position.
        y: i32,
        /// Width.
        width: i32,
        /// Height.
        height: i32,
    },
    /// Widget existence check.
    Exists(bool),
    /// Child count of a tagged widget.
    ChildCount(u16),
    /// Basic status / telemetry line.
    Status(StatusData),
    /// Framebuffer dump complete marker.
    DumpEnd,
}
