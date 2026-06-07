//! SD card adapter for `embedded-sdmmc` on the STM32H747I-DISCO.
//!
//! Provides [`SdMmcBlockDev`], an implementation of [`embedded_sdmmc::BlockDevice`]
//! wrapping the SDMMC1 HAL driver with D-Cache maintenance. Also provides a
//! dummy [`DummyTimeSource`] for filesystem timestamps.
//!
//! ## DCB integration (DCB-02c, 2026-05-02)
//!
//! Cache maintenance routes through [`DcaCacheCtx`] + the [`DcaCache`]
//! trait rather than direct `SCB::*_dcache_by_*` calls. See the
//! sibling `stm32h747i_disco_sd.rs` module docs for the rationale.
//!
//! `embedded-sdmmc` `Block` buffers are stack-allocated `[u8; 512]`
//! and may not be 32-byte aligned. The DcaCache trait's
//! `clean(addr, len)` / `invalidate(addr, len)` methods use the
//! `*_dcache_by_address` SCB variants, which round down to cache-line
//! boundaries internally — alignment-tolerant. The pre-DCB
//! `clean_invalidate_dcache_by_address(buf.as_ptr() as usize,
//! buf.len())` workaround for the unaligned-slice case is no longer
//! needed; the directional ops (`invalidate` for the read path,
//! `clean` for the write path) suffice on their own.
//!
//! [`DcaCacheCtx`]: crate::hwcore::dca::DcaCacheCtx
//! [`DcaCache`]: crate::hwcore::dca::DcaCache

use core::cell::RefCell;
use cortex_m::asm;
use embedded_sdmmc::blockdevice::{Block, BlockCount, BlockDevice, BlockIdx};
use stm32h7xx_hal::pac::SDMMC1;
use stm32h7xx_hal::sdmmc::{SdCard, Sdmmc};

use crate::hwcore::dca::{DcaCache, DcaCacheCtx};

/// Block device backed by the onboard microSD slot via SDMMC1.
///
/// Uses interior mutability (`RefCell`) because `embedded_sdmmc::BlockDevice`
/// requires `&self` for read/write operations, but the HAL's `Sdmmc` needs
/// `&mut self`.
pub struct SdMmcBlockDev {
    sdmmc: RefCell<Sdmmc<SDMMC1, SdCard>>,
}

/// Errors from the SD block device.
#[derive(Debug, Clone, Copy)]
pub enum SdError {
    /// The underlying SDMMC HAL returned an error.
    Hal,
}

impl SdMmcBlockDev {
    /// Create a new block device from an initialized SDMMC1 peripheral.
    pub fn new(sdmmc: Sdmmc<SDMMC1, SdCard>) -> Self {
        Self {
            sdmmc: RefCell::new(sdmmc),
        }
    }

    /// Invalidate D-Cache lines covering `buf` before/after a DMA read.
    ///
    /// DCB-02c: the previous `clean_invalidate_dcache_by_address`
    /// workaround for unaligned `embedded-sdmmc` Block buffers is
    /// no longer needed. The DcaCache trait's `invalidate(addr,
    /// len)` uses the alignment-tolerant address-form SCB op, and
    /// the directional operation suffices for the read path —
    /// CPU-dirty cache lines for an SDMMC read buffer are by
    /// contract uninteresting (the buffer is typically a fresh
    /// scratch the caller hands in).
    fn invalidate(buf: &mut [u8]) {
        // SAFETY: per DCB-00 §9 INV-D13, engine drivers privately
        // steal SCB at the cache call site. See sibling
        // stm32h747i_disco_sd.rs for the SCB-ownership note.
        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            let mut ctx = DcaCacheCtx::new(&mut scb);
            ctx.cache_mut().invalidate(buf.as_ptr() as usize, buf.len());
        }
        asm::dmb();
    }

    /// Clean D-Cache lines covering `buf` before a DMA write.
    ///
    /// DCB-02c: routes through DcaCacheCtx; same SCB-ownership
    /// rationale as [`Self::invalidate`].
    fn clean(buf: &[u8]) {
        // SAFETY: see Self::invalidate.
        unsafe {
            let mut scb = cortex_m::Peripherals::steal().SCB;
            let mut ctx = DcaCacheCtx::new(&mut scb);
            ctx.cache_mut().clean(buf.as_ptr() as usize, buf.len());
        }
        asm::dmb();
    }
}

impl BlockDevice for SdMmcBlockDev {
    type Error = SdError;

    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut sdmmc = self.sdmmc.borrow_mut();
        for (i, block) in blocks.iter_mut().enumerate() {
            Self::invalidate(&mut block.contents);
            sdmmc
                .read_blocks(start_block_idx.0 + i as u32, &mut block.contents)
                .map_err(|_| SdError::Hal)?;
            Self::invalidate(&mut block.contents);
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let mut sdmmc = self.sdmmc.borrow_mut();
        for (i, block) in blocks.iter().enumerate() {
            Self::clean(&block.contents);
            sdmmc
                .write_blocks(start_block_idx.0 + i as u32, &block.contents)
                .map_err(|_| SdError::Hal)?;
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let sdmmc = self.sdmmc.borrow();
        let count = sdmmc
            .card()
            .map(|c| c.csd.block_count() as u32)
            .unwrap_or(0);
        Ok(BlockCount(count))
    }
}

/// A dummy time source that always returns a fixed timestamp.
///
/// Replace with an RTC-backed implementation when real timestamps are needed.
pub struct DummyTimeSource;

impl embedded_sdmmc::TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        // 2025-01-01 00:00:00
        embedded_sdmmc::Timestamp {
            year_since_1970: 55,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
