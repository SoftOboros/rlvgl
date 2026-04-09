//! Transport abstraction for the playit driver.

/// Byte-level I/O between the test host and the playit executor.
///
/// Implementors provide the raw byte stream.  The [`PlayitExecutor`] handles
/// line accumulation and protocol framing on top.
///
/// [`PlayitExecutor`]: crate::executor::PlayitExecutor
pub trait PlayitTransport {
    /// Read one byte from the transport, or `None` if no data is available.
    fn read_byte(&mut self) -> Option<u8>;

    /// Write a slice of bytes to the transport.
    fn write_bytes(&mut self, bytes: &[u8]);
}
