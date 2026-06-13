//! Asset loading interfaces for filesystem-backed content.
//!
//! This module provides traits used by the optional `fs` feature to source
//! assets such as fonts or images from an underlying filesystem.

use alloc::boxed::Box;

/// Errors that can occur during filesystem operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// Underlying device reported an error.
    Device,
    /// Provided path was invalid.
    InvalidPath,
    /// File or directory was not found.
    NoSuchFile,
}

/// Block device abstraction used by the filesystem layer.
///
/// Implementors provide sector-based access to a storage medium.
pub trait BlockDevice {
    /// Read blocks starting at `lba` into `buf`.
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError>;

    /// Write blocks starting at `lba` from `buf`.
    ///
    /// Implementations may leave this unimplemented if the device is read-only.
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError>;

    /// Return the logical block size in bytes.
    fn block_size(&self) -> usize;

    /// Return the total number of addressable blocks.
    fn num_blocks(&self) -> u64;

    /// Flush any buffered data to the underlying device.
    fn flush(&mut self) -> Result<(), FsError>;
}

/// Error type returned by asset operations.
#[derive(Debug, Clone)]
pub enum AssetError {
    /// Underlying filesystem error.
    Fs(FsError),
    /// Asset bytes were retrieved but the decode step failed.
    ///
    /// The inner `String` contains a human-readable description of the failure.
    /// This variant is produced by [`crate::asset::AssetRegistry::resolve_image`]
    /// when a codec plugin returns an error after the source bytes were
    /// successfully read.
    Decode(alloc::string::String),
}

/// Reader trait for streaming asset data.
pub trait AssetRead {
    /// Read data into `out`, returning the number of bytes read.
    fn read(&mut self, out: &mut [u8]) -> Result<usize, AssetError>;

    /// Total length of the asset in bytes.
    fn len(&self) -> usize;

    /// Return `true` if the asset has a length of zero bytes.
    fn is_empty(&self) -> bool;

    /// Seek to an absolute byte position within the asset.
    fn seek(&mut self, pos: u64) -> Result<u64, AssetError>;
}

/// Source of assets such as fonts or images.
pub trait AssetSource {
    /// Open an asset by logical path, e.g., `"fonts/regular.bin"`.
    fn open<'a>(&'a self, path: &str) -> Result<Box<dyn AssetRead + 'a>, AssetError>;

    /// Determine whether an asset at `path` exists.
    fn exists(&self, path: &str) -> bool;

    /// List the contents of `dir`, returning an iterator over asset entries.
    fn list(&self, dir: &str) -> Result<AssetIter, AssetError>;
}

/// Iterator over asset entries returned by [`AssetSource::list`].
pub struct AssetIter;

impl Iterator for AssetIter {
    type Item = (); // placeholder until fleshed out

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// Manager that provides convenient typed loading helpers.
pub struct AssetManager<S: AssetSource> {
    source: S,
}

impl<S: AssetSource> AssetManager<S> {
    /// Create a new [`AssetManager`] from an [`AssetSource`].
    pub fn new(source: S) -> Self {
        Self { source }
    }

    /// Open a raw asset stream at `path`.
    pub fn open(&self, path: &str) -> Result<Box<dyn AssetRead + '_>, AssetError> {
        self.source.open(path)
    }

    /// Load the raw bytes of the asset at `path` into a heap buffer.
    ///
    /// This is the foundation for typed loaders such as packed-font loading.
    /// For a packed font (`PackedFont`) the caller must additionally map the
    /// returned bytes into static storage because `PackedFont::data` requires a
    /// `&'static [u8]`; that pattern is outside the scope of this helper.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::Fs`] when the source cannot open or read the asset.
    pub fn load_bytes(&self, path: &str) -> Result<alloc::vec::Vec<u8>, AssetError> {
        let mut reader = self.source.open(path)?;
        let len = reader.len();
        let mut buf = alloc::vec![0u8; len];
        let mut offset = 0usize;
        while offset < len {
            let n = reader.read(&mut buf[offset..])?;
            if n == 0 {
                break;
            }
            offset += n;
        }
        Ok(buf)
    }
}
