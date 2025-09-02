//! platform/src/sd_fatfs_adapter.rs - No-std FATFS adapter over `BlockDevice`.
//!
//! This module provides a minimal bridge between `rlvgl_core::fs::BlockDevice`
//! and the `fatfs` crate in `no_std` + `alloc` mode. It enables mounting a FAT
//! volume on top of an SDMMC-backed block device and listing simple directories
//! such as `/assets` during bring-up.

#![deny(missing_docs)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::min;
use core::ops::Range;
use core_io::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};
use fatfs::{Dir, FileSystem, FsOptions};
use rlvgl_core::fs::BlockDevice;

/// Buffered sector stream over a [`BlockDevice`], suitable for `fatfs`.
///
/// Implements `core2` I/O traits so `fatfs` can operate without `std`.
pub struct FatfsBlockStream<'a, BD: BlockDevice> {
    bd: &'a mut BD,
    pos: u64,
    total: u64,
    sector: usize,
    buf: Vec<u8>,
    buf_lba: u64,
}

impl<'a, BD: BlockDevice> FatfsBlockStream<'a, BD> {
    /// Create a new stream over `bd` with an internal single-sector buffer.
    pub fn new(bd: &'a mut BD) -> Self {
        let sector = bd.block_size();
        Self {
            bd,
            pos: 0,
            total: (bd.num_blocks() as u64) * (sector as u64),
            sector,
            buf: alloc::vec![0u8; sector],
            buf_lba: u64::MAX, // invalid to force initial load
        }
    }

    fn cur_lba_off(&self) -> (u64, usize) {
        let lba = self.pos / (self.sector as u64);
        let off = (self.pos % (self.sector as u64)) as usize;
        (lba, off)
    }

    fn load_lba(&mut self, lba: u64) -> Result<()> {
        if self.buf_lba == lba {
            return Ok(());
        }
        // Read single sector into buffer
        self.bd
            .read_blocks(lba, &mut self.buf)
            .map_err(|_| Error::from(ErrorKind::Other))?;
        self.buf_lba = lba;
        Ok(())
    }
}

impl<'a, BD: BlockDevice> Read for FatfsBlockStream<'a, BD> {
    fn read(&mut self, mut out: &mut [u8]) -> Result<usize> {
        if self.pos >= self.total {
            return Ok(0);
        }
        let mut read_total = 0usize;
        while !out.is_empty() && self.pos < self.total {
            let (lba, off) = self.cur_lba_off();
            self.load_lba(lba)?;
            let avail = min(self.sector - off, out.len());
            let tail = min(avail as u64, self.total - self.pos) as usize;
            out[..tail].copy_from_slice(&self.buf[off..off + tail]);
            out = &mut out[tail..];
            self.pos += tail as u64;
            read_total += tail;
        }
        Ok(read_total)
    }
}

impl<'a, BD: BlockDevice> Seek for FatfsBlockStream<'a, BD> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(off) => {
                if off >= 0 {
                    self.total.saturating_add(off as u64)
                } else {
                    self.total.saturating_sub((-off) as u64)
                }
            }
            SeekFrom::Current(off) => {
                if off >= 0 {
                    self.pos.saturating_add(off as u64)
                } else {
                    self.pos.saturating_sub((-off) as u64)
                }
            }
        };
        self.pos = new_pos.min(self.total);
        Ok(self.pos)
    }
}

impl<'a, BD: BlockDevice> Write for FatfsBlockStream<'a, BD> {
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        Err(Error::from(ErrorKind::Unsupported))
    }
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Mount a FAT filesystem in read-only mode and list the `/assets` directory.
///
/// Returns a vector of entry names on success. The underlying device is not
/// modified; `FsOptions::read_only(true)` is used to avoid any metadata writes.
pub fn mount_and_list_assets<BD: BlockDevice>(bd: &mut BD) -> Result<Vec<String>> {
    let stream = FatfsBlockStream::new(bd);
    let fs = FileSystem::new(stream, FsOptions::new().read_only(true))
        .map_err(|_| Error::from(ErrorKind::Other))?;
    let root = fs.root_dir();
    let mut out: Vec<String> = Vec::new();
    // Try `/assets`; if missing, fall back to root.
    let mut list_dir = |d: Dir<_>| -> Result<()> {
        for r in d.iter() {
            let e = r.map_err(|_| Error::from(ErrorKind::Other))?;
            out.push(e.file_name().to_string());
        }
        Ok(())
    };
    match root.open_dir("assets") {
        Ok(dir) => list_dir(dir)?,
        Err(_) => list_dir(root)?,
    }
    Ok(out)
}
