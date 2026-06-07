//! Framebuffer pixel inspection trait.

/// Read-only access to the display front buffer for pixel inspection.
///
/// Platform backends implement this to expose the current visible framebuffer
/// content to the playit executor for dump commands.
pub trait FramebufferReader {
    /// Read a single pixel at the given *landscape* coordinates.
    ///
    /// Returns an ARGB8888 packed value.  The implementor is responsible for
    /// any coordinate transforms (e.g. portrait ↔ landscape).
    fn read_pixel(&self, x: i32, y: i32) -> u32;

    /// Read a horizontal run of pixels starting at `(x, y)` into `out`.
    ///
    /// Returns the number of pixels actually written (may be less than
    /// `width` if the region extends past the framebuffer edge).
    fn read_row(&self, x: i32, y: i32, width: u16, out: &mut [u32]) -> usize;

    /// Current display present count, used to gate frame-synchronised dumps.
    fn present_count(&self) -> u32;
}
