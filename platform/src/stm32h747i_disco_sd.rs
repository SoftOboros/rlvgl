//! SD card block device for the STM32H747I-DISCO board.
//!
//! Provides [`DiscoSdBlockDevice`], an implementation of [`rlvgl_core::fs::BlockDevice`]
//! using the SDMMC1 peripheral with DMA and explicit D-Cache maintenance.
#![deny(missing_docs)]

use cortex_m::{asm, peripheral::SCB};
use rlvgl_core::fs::{BlockDevice, FsError};
use stm32h7xx_hal::pac::SDMMC1;
use stm32h7xx_hal::sdmmc::{SdCard, Sdmmc};

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

    /// Clean and invalidate the D-Cache for `buf` to prepare for DMA.
    fn invalidate(buf: &mut [u8]) {
        unsafe {
            // SAFETY: Accessing the SCB registers is safe here because we have
            // exclusive access to the buffer and perform a full memory
            // barrier after touching the cache.
            (&mut *(SCB::ptr() as *mut SCB)).invalidate_dcache_by_slice(buf);
        }
        asm::dmb();
    }

    /// Clean the D-Cache for `buf` before a DMA write.
    fn clean(buf: &[u8]) {
        unsafe {
            // SAFETY: See rationale in [`Self::invalidate`].
            (&mut *(SCB::ptr() as *mut SCB)).clean_dcache_by_slice(buf);
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
