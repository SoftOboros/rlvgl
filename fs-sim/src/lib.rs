//! Simulator block device backed by a host file.
//!
//! Provides [`SimBlockDevice`], an implementation of [`rlvgl_core::fs::BlockDevice`]
//! that reads and writes sectors from a disk image stored on the host.
#![deny(missing_docs)]

#[cfg(feature = "mmap")]
use memmap2::{MmapMut, MmapOptions};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use rlvgl_core::fs::{BlockDevice, FsError};

/// Block device backed by a host file representing a disk image.
pub struct SimBlockDevice {
    /// Underlying disk image file.
    file: File,
    /// Optional memory mapping of the disk image for faster access.
    #[cfg(feature = "mmap")]
    mmap: Option<MmapMut>,
    /// Logical block size in bytes.
    block_size: usize,
    /// Total number of blocks in the disk image.
    num_blocks: u64,
}

impl SimBlockDevice {
    /// Create a new [`SimBlockDevice`] from `file` with the given `block_size`.
    ///
    /// The file length must be a multiple of `block_size`.
    pub fn new(file: File, block_size: usize) -> Result<Self, FsError> {
        let len = file.metadata().map_err(|_| FsError::Device)?.len();
        if block_size == 0 || len % block_size as u64 != 0 {
            return Err(FsError::InvalidPath);
        }
        #[cfg(feature = "mmap")]
        let mmap = unsafe { MmapOptions::new().map_mut(&file).ok() };
        Ok(Self {
            file,
            #[cfg(feature = "mmap")]
            mmap,
            block_size,
            num_blocks: len / block_size as u64,
        })
    }
}

impl BlockDevice for SimBlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError> {
        let offset = lba
            .checked_mul(self.block_size as u64)
            .ok_or(FsError::Device)?;
        #[cfg(feature = "mmap")]
        if let Some(mmap) = self.mmap.as_ref() {
            let start = offset as usize;
            let end = start.checked_add(buf.len()).ok_or(FsError::Device)?;
            buf.copy_from_slice(&mmap[start..end]);
            return Ok(());
        }
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| FsError::Device)?;
        self.file.read_exact(buf).map_err(|_| FsError::Device)
    }

    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError> {
        let offset = lba
            .checked_mul(self.block_size as u64)
            .ok_or(FsError::Device)?;
        #[cfg(feature = "mmap")]
        if let Some(mmap) = self.mmap.as_mut() {
            let start = offset as usize;
            let end = start.checked_add(buf.len()).ok_or(FsError::Device)?;
            mmap[start..end].copy_from_slice(buf);
            return Ok(());
        }
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| FsError::Device)?;
        self.file.write_all(buf).map_err(|_| FsError::Device)
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.num_blocks
    }

    fn flush(&mut self) -> Result<(), FsError> {
        #[cfg(feature = "mmap")]
        if let Some(mmap) = self.mmap.as_mut() {
            mmap.flush().map_err(|_| FsError::Device)?;
        }
        self.file.flush().map_err(|_| FsError::Device)
    }
}
