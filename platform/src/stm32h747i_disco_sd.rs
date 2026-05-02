//! SD card block device for the STM32H747I-DISCO board.
//!
//! Provides [`DiscoSdBlockDevice`], an implementation of [`rlvgl_core::fs::BlockDevice`]
//! using the SDMMC1 peripheral with DMA and explicit D-Cache maintenance.
//!
//! ## DCB integration (DCB-02c, 2026-05-02)
//!
//! Cache maintenance for the SD DMA buffers routes through
//! [`DcaCacheCtx`] + the [`DcaCache`] trait rather than direct
//! `SCB::*_dcache_by_*` calls. Per DCB-00 §9 INV-D8 the scanner
//! whitelists those calls inside `hwcore::dca` (where the trait's
//! SCB impl lives); call sites in this module dispatch through the
//! trait, so the `raw_dcache` rule no longer matches here.
//!
//! The buffer-typestate substitution (`DcaBuf<u8, BLOCK_BYTES>`)
//! prescribed by DCB-00 §10 is **not** applied here: the
//! `rlvgl_core::fs::BlockDevice` trait method signatures take
//! caller-supplied `&mut [u8]` slices of arbitrary alignment, so a
//! true DcaBuf substitution would require a trait-surface change.
//! Instead this module preserves the existing trait surface and
//! moves only the cache-call discipline. INV-D9 ("new DMA buffers
//! MUST use DcaBuf") is read as forward-looking — these buffers
//! existed pre-DCB and are grandfathered to the address-form cache
//! ops via the DcaCache trait (which uses the unaligned-tolerant
//! `clean_dcache_by_address` / `invalidate_dcache_by_address`
//! variants internally).
//!
//! [`DcaCacheCtx`]: crate::hwcore::dca::DcaCacheCtx
//! [`DcaCache`]: crate::hwcore::dca::DcaCache
#![deny(missing_docs)]

use cortex_m::asm;
use rlvgl_core::fs::{BlockDevice, FsError};
use stm32h7xx_hal::pac::SDMMC1;
use stm32h7xx_hal::sdmmc::{SdCard, Sdmmc};

use crate::hwcore::dca::{DcaCache, DcaCacheCtx};

/// Block device backed by the onboard microSD slot.
pub struct DiscoSdBlockDevice {
    sdmmc: Sdmmc<SDMMC1, SdCard>,
    block_size: usize,
}

impl DiscoSdBlockDevice {
    /// Create a new [`DiscoSdBlockDevice`] from an initialized SDMMC1 peripheral.
    pub fn new(sdmmc: Sdmmc<SDMMC1, SdCard>) -> Self {
        Self {
            sdmmc,
            block_size: 512,
        }
    }

    /// Invalidate D-Cache lines covering `buf` before/after a DMA read.
    ///
    /// DCB-02c: routes through [`DcaCacheCtx`] rather than calling
    /// SCB directly. The trait's SCB impl uses
    /// `invalidate_dcache_by_address` (alignment-tolerant; rounds
    /// down to cache-line internally) so the caller's slice need
    /// not be 32-byte aligned.
    fn invalidate(buf: &mut [u8]) {
        // SAFETY: per DCB-00 §9 INV-D13, engine drivers privately
        // steal SCB at the cache call site. Single-threaded
        // firmware ensures no two `&mut SCB` are USED simultaneously
        // even though multiple drivers may steal in principle.
        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            let mut ctx = DcaCacheCtx::new(&mut scb);
            ctx.cache_mut().invalidate(buf.as_ptr() as usize, buf.len());
        }
        asm::dmb();
    }

    /// Clean D-Cache lines covering `buf` before a DMA write.
    ///
    /// DCB-02c: routes through [`DcaCacheCtx`] (see [`Self::invalidate`]
    /// for the SCB-ownership rationale).
    fn clean(buf: &[u8]) {
        // SAFETY: see Self::invalidate for the SCB-stealing contract.
        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            let mut ctx = DcaCacheCtx::new(&mut scb);
            ctx.cache_mut().clean(buf.as_ptr() as usize, buf.len());
        }
        asm::dmb();
    }
}

impl BlockDevice for DiscoSdBlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError> {
        if buf.len() % self.block_size != 0 {
            return Err(FsError::Device);
        }
        Self::invalidate(buf);
        self.sdmmc
            .read_blocks(lba as u32, buf)
            .map_err(|_| FsError::Device)?;
        Self::invalidate(buf);
        Ok(())
    }

    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError> {
        if buf.len() % self.block_size != 0 {
            return Err(FsError::Device);
        }
        Self::clean(buf);
        self.sdmmc
            .write_blocks(lba as u32, buf)
            .map_err(|_| FsError::Device)
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.sdmmc
            .card()
            .map(|c| c.csd.block_count() as u64)
            .unwrap_or(0)
    }

    fn flush(&mut self) -> Result<(), FsError> {
        self.sdmmc.card().map(|_| ()).map_err(|_| FsError::Device)
    }
}
