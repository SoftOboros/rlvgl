//! SD card adapter for `embedded-sdmmc` on the STM32H747I-DISCO.
//!
//! Provides [`SdMmcBlockDev`], an implementation of [`embedded_sdmmc::BlockDevice`]
//! wrapping the SDMMC1 HAL driver with D-Cache maintenance. Also provides a
//! dummy [`DummyTimeSource`] for filesystem timestamps.

use core::cell::RefCell;
use cortex_m::asm;
use embedded_sdmmc::blockdevice::{Block, BlockCount, BlockDevice, BlockIdx};
use stm32h7xx_hal::pac::SDMMC1;
use stm32h7xx_hal::sdmmc::{SdCard, Sdmmc};

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

    /// Clean + invalidate D-Cache for a buffer (before and after DMA reads).
    ///
    /// Uses `clean_invalidate_dcache_by_address` instead of bare
    /// `invalidate_dcache_by_slice` because the latter asserts cache-line
    /// alignment.  `embedded-sdmmc` `Block` buffers are stack-allocated
    /// and may not be 32-byte aligned.
    fn invalidate(buf: &mut [u8]) {
        unsafe {
            cortex_m::Peripherals::steal()
                .SCB
                .clean_invalidate_dcache_by_address(buf.as_ptr() as usize, buf.len());
        }
        asm::dmb();
    }

    /// Clean D-Cache for a buffer (before DMA writes).
    fn clean(buf: &[u8]) {
        unsafe {
            cortex_m::Peripherals::steal()
                .SCB
                .clean_dcache_by_slice(buf);
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
