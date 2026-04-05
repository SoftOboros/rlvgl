//! QSPI flash driver for the Micron MT25TL01G on the STM32H747I-DISCO.
//!
//! Provides [`Mt25tlFlash`] for indirect-mode read/write/erase and
//! [`QspiMemoryMapped`] for memory-mapped access at `0x9000_0000`.
//!
//! The driver applies errata workarounds from ES0396:
//! - **2.8.3**: Timing-critical reset sequence on init and mode transitions.
//! - **2.8.5**: Kernel clock must not be HCLK with prescaler 0 (caller must
//!   select PLL1Q or PLL2R via `RCC.D1CCIPR` before constructing).

use core::ptr;

use stm32h7xx_hal::xspi::{Qspi, QspiError, QspiWord};

/// MT25TL01G command opcodes.
mod cmd {
    pub const RESET_ENABLE: u8 = 0x66;
    pub const RESET_MEMORY: u8 = 0x99;
    pub const READ_ID: u8 = 0x9F;
    pub const READ_STATUS: u8 = 0x05;
    pub const WRITE_ENABLE: u8 = 0x06;
    pub const READ: u8 = 0x03;
    pub const FAST_READ_QUAD_IO: u8 = 0xEB;
    pub const PAGE_PROGRAM: u8 = 0x02;
    pub const SUBSECTOR_ERASE_4K: u8 = 0x20;
    pub const SECTOR_ERASE_64K: u8 = 0xD8;
    pub const CHIP_ERASE: u8 = 0xC7;
}

/// Status register bit masks.
mod status {
    pub const WIP: u8 = 0x01; // Write In Progress
    pub const WEL: u8 = 0x02; // Write Enable Latch
}

/// Expected JEDEC ID for MT25TL01G: Micron (0x20), type 0xBA, capacity 0x22.
pub const MT25TL01G_JEDEC_ID: [u8; 3] = [0x20, 0xBA, 0x22];

/// Page size for page-program operations (256 bytes).
pub const PAGE_SIZE: usize = 256;
/// Subsector (4 KB) erase granularity.
pub const SUBSECTOR_SIZE: usize = 4 * 1024;
/// Sector (64 KB) erase granularity.
pub const SECTOR_SIZE: usize = 64 * 1024;
/// Total flash size per die (64 MB).
pub const FLASH_SIZE: usize = 64 * 1024 * 1024;

/// QSPI flash memory-mapped base address.
pub const MEMORY_MAPPED_BASE: u32 = 0x9000_0000;

/// MT25TL01G flash driver in indirect mode.
pub struct Mt25tlFlash {
    qspi: Qspi<stm32h7xx_hal::pac::QUADSPI>,
}

/// Result type for flash operations.
pub type FlashResult<T> = Result<T, FlashError>;

/// Flash operation errors.
#[derive(Debug, Clone, Copy)]
pub enum FlashError {
    /// QSPI peripheral error.
    Qspi(QspiError),
    /// JEDEC ID mismatch.
    BadId([u8; 3]),
    /// Write-enable latch failed to set.
    WriteEnableFailed,
    /// Timeout waiting for operation to complete.
    Timeout,
}

impl From<QspiError> for FlashError {
    fn from(e: QspiError) -> Self {
        FlashError::Qspi(e)
    }
}

impl Mt25tlFlash {
    /// Create a new flash driver, applying errata 2.8.3 reset sequence.
    ///
    /// **Prerequisite**: Caller must have already set `RCC.D1CCIPR` QSPISEL
    /// bits to select PLL1Q or PLL2R as the QSPI kernel clock (errata 2.8.5).
    pub fn new(qspi: Qspi<stm32h7xx_hal::pac::QUADSPI>) -> Self {
        let mut s = Self { qspi };
        s.errata_2_8_3_reset();
        s
    }

    /// Errata 2.8.3: Internal timing reset sequence.
    ///
    /// Must be executed upon reset and upon switching from memory-mapped to
    /// any other mode.
    fn errata_2_8_3_reset(&mut self) {
        let rb = self.qspi.inner_mut();
        unsafe {
            // Step 1: Disable the peripheral, ensure prescaler is not max
            rb.cr.write(|w| w.bits(0));
            while rb.sr.read().busy().bit_is_set() {}

            // Step 2: Set maximum prescaler and enable
            rb.cr.write(|w| w.bits(0xFF00_0001));

            // Step 3: Activate free-running clock
            rb.ccr.write(|w| w.bits(0x2000_0000));
            // Repeat to prevent back-to-back disable
            rb.ccr.write(|w| w.bits(0x2000_0000));

            // Step 4: Disable within 127 kernel clocks
            rb.cr.write(|w| w.bits(0));
            while rb.sr.read().busy().bit_is_set() {}

            // Step 5: Re-enable with normal prescaler (will be set by HAL)
            // For now, just enable with a reasonable prescaler
            rb.cr.modify(|_, w| w.en().set_bit());
        }

        // Issue software reset to the flash device
        let _ = self.command_only(cmd::RESET_ENABLE);
        let _ = self.command_only(cmd::RESET_MEMORY);
        // Wait 30us for reset to complete (at 400 MHz, ~12000 cycles)
        cortex_m::asm::delay(12_000);
    }

    /// Send a command-only transaction (no address, no data).
    fn command_only(&mut self, instruction: u8) -> FlashResult<()> {
        self.qspi.write_extended(
            QspiWord::U8(instruction),
            QspiWord::None,
            QspiWord::None,
            &[],
        )?;
        Ok(())
    }

    /// Read the JEDEC ID (manufacturer, type, capacity).
    pub fn read_id(&mut self) -> FlashResult<[u8; 3]> {
        let mut id = [0u8; 3];
        self.qspi.read_extended(
            QspiWord::U8(cmd::READ_ID),
            QspiWord::None,
            QspiWord::None,
            0,
            &mut id,
        )?;
        Ok(id)
    }

    /// Read the status register.
    pub fn read_status(&mut self) -> FlashResult<u8> {
        let mut sr = [0u8; 1];
        self.qspi.read_extended(
            QspiWord::U8(cmd::READ_STATUS),
            QspiWord::None,
            QspiWord::None,
            0,
            &mut sr,
        )?;
        Ok(sr[0])
    }

    /// Poll until the Write-In-Progress bit clears, with timeout.
    pub fn wait_wip(&mut self) -> FlashResult<()> {
        let mut timeout = 10_000_000u32;
        loop {
            let sr = self.read_status()?;
            if sr & status::WIP == 0 {
                return Ok(());
            }
            timeout = timeout.checked_sub(1).ok_or(FlashError::Timeout)?;
            cortex_m::asm::nop();
        }
    }

    /// Issue WRITE_ENABLE and verify the WEL bit is set.
    pub fn write_enable(&mut self) -> FlashResult<()> {
        self.command_only(cmd::WRITE_ENABLE)?;
        let sr = self.read_status()?;
        if sr & status::WEL == 0 {
            return Err(FlashError::WriteEnableFailed);
        }
        Ok(())
    }

    /// Read data from the flash using standard SPI read (1-1-1, no dummies).
    ///
    /// For reads up to 32 bytes, uses the HAL directly. For larger reads,
    /// manually manages the FIFO via the DR register.
    pub fn read(&mut self, addr: u32, dest: &mut [u8]) -> FlashResult<()> {
        if dest.is_empty() {
            return Ok(());
        }
        if dest.len() <= 32 {
            self.qspi.read_extended(
                QspiWord::U8(cmd::READ),
                QspiWord::U24(addr),
                QspiWord::None,
                0,
                dest,
            )?;
            return Ok(());
        }
        // Bulk read: use begin_read_extended + manual FIFO drain
        self.qspi.begin_read_extended(
            QspiWord::U8(cmd::READ),
            QspiWord::U24(addr),
            QspiWord::None,
            0,
            dest.len(),
        )?;
        self.drain_fifo_read(dest);
        Ok(())
    }

    /// Read data using Quad I/O Fast Read (1-4-4, 10 dummy cycles).
    pub fn read_quad(&mut self, addr: u32, dest: &mut [u8]) -> FlashResult<()> {
        if dest.is_empty() {
            return Ok(());
        }
        // For quad read we need to temporarily switch the QSPI mode.
        // The HAL's extended API sends instruction in current mode, so we
        // use a manual CCR configuration for the mixed-mode command.
        let rb = self.qspi.inner();
        unsafe {
            // Clear TCF
            rb.fcr.write(|w| w.bits(0x1));
            // Set data length
            rb.dlr.write(|w| w.bits(dest.len() as u32 - 1));
            // CCR: FMODE=01 (indirect read), DMODE=11 (4-line data),
            //       DCYC=10, ABMODE=00, ADSIZE=10 (24-bit), ADMODE=11 (4-line),
            //       IMODE=01 (1-line instruction), INSTRUCTION=0xEB
            let ccr: u32 = (0b01 << 26)     // FMODE: indirect read
                | (0b11 << 24)              // DMODE: 4-line
                | (10 << 18)                // DCYC: 10 dummy cycles
                | (0b00 << 14)              // ABMODE: none
                | (0b10 << 12)              // ADSIZE: 24-bit
                | (0b11 << 10)              // ADMODE: 4-line
                | (0b01 << 8)               // IMODE: 1-line
                | (cmd::FAST_READ_QUAD_IO as u32);
            rb.ccr.write(|w| w.bits(ccr));
            // Write address to start the transfer
            rb.ar.write(|w| w.bits(addr));
        }
        self.drain_fifo_read(dest);
        Ok(())
    }

    /// Drain the FIFO into `dest` after a read has been started.
    fn drain_fifo_read(&mut self, dest: &mut [u8]) {
        let rb = self.qspi.inner();
        let dr_ptr = &rb.dr as *const _ as *const u8;
        let mut idx = 0;
        while idx < dest.len() {
            // Wait until there's data in the FIFO
            let sr = rb.sr.read();
            let level = sr.flevel().bits() as usize;
            if level > 0 {
                let to_read = core::cmp::min(level, dest.len() - idx);
                for _ in 0..to_read {
                    dest[idx] = unsafe { ptr::read_volatile(dr_ptr) };
                    idx += 1;
                }
            }
        }
        // Wait for transfer complete
        while rb.sr.read().tcf().bit_is_clear() {}
        // Clear TCF
        rb.fcr.write(|w| w.ctcf().set_bit());
        // Wait not busy
        while rb.sr.read().busy().bit_is_set() {}
    }

    /// Program up to 256 bytes (one page). Address must be page-aligned for
    /// full-page writes, but partial pages at any offset are allowed.
    ///
    /// The caller must ensure `data.len() <= PAGE_SIZE` and that the write
    /// does not cross a page boundary.
    pub fn page_program(&mut self, addr: u32, data: &[u8]) -> FlashResult<()> {
        assert!(data.len() <= PAGE_SIZE);
        if data.is_empty() {
            return Ok(());
        }
        self.write_enable()?;

        if data.len() <= 32 {
            self.qspi.write_extended(
                QspiWord::U8(cmd::PAGE_PROGRAM),
                QspiWord::U24(addr),
                QspiWord::None,
                data,
            )?;
        } else {
            // Bulk write: begin + fill FIFO manually
            self.qspi.begin_write_extended(
                QspiWord::U8(cmd::PAGE_PROGRAM),
                QspiWord::U24(addr),
                QspiWord::None,
                data.len(),
            )?;
            self.fill_fifo_write(data);
        }

        self.wait_wip()?;
        Ok(())
    }

    /// Write arbitrary data starting at `addr`. Handles page boundary
    /// splitting automatically.
    pub fn write(&mut self, mut addr: u32, mut data: &[u8]) -> FlashResult<()> {
        while !data.is_empty() {
            let page_offset = (addr as usize) % PAGE_SIZE;
            let chunk = core::cmp::min(PAGE_SIZE - page_offset, data.len());
            self.page_program(addr, &data[..chunk])?;
            addr += chunk as u32;
            data = &data[chunk..];
        }
        Ok(())
    }

    /// Fill the FIFO from `data` after a write has been started.
    fn fill_fifo_write(&mut self, data: &[u8]) {
        let rb = self.qspi.inner();
        let dr_ptr = &rb.dr as *const _ as *const core::cell::UnsafeCell<u8>;
        let mut idx = 0;
        while idx < data.len() {
            let sr = rb.sr.read();
            // FTF: FIFO threshold flag -- space available
            if sr.ftf().bit_is_set() {
                let fifo_free = 32 - sr.flevel().bits() as usize;
                let to_write = core::cmp::min(fifo_free, data.len() - idx);
                for _ in 0..to_write {
                    unsafe { ptr::write_volatile(core::cell::UnsafeCell::raw_get(dr_ptr), data[idx]) };
                    idx += 1;
                }
            }
        }
        // Wait for transfer complete
        while rb.sr.read().tcf().bit_is_clear() {}
        // Clear TCF
        rb.fcr.write(|w| w.ctcf().set_bit());
        // Wait not busy
        while rb.sr.read().busy().bit_is_set() {}
    }

    /// Erase a 4 KB subsector containing `addr`.
    pub fn erase_subsector(&mut self, addr: u32) -> FlashResult<()> {
        let aligned = addr & !(SUBSECTOR_SIZE as u32 - 1);
        self.write_enable()?;
        self.qspi.write_extended(
            QspiWord::U8(cmd::SUBSECTOR_ERASE_4K),
            QspiWord::U24(aligned),
            QspiWord::None,
            &[],
        )?;
        self.wait_wip()?;
        Ok(())
    }

    /// Erase a 64 KB sector containing `addr`.
    pub fn erase_sector(&mut self, addr: u32) -> FlashResult<()> {
        let aligned = addr & !(SECTOR_SIZE as u32 - 1);
        self.write_enable()?;
        self.qspi.write_extended(
            QspiWord::U8(cmd::SECTOR_ERASE_64K),
            QspiWord::U24(aligned),
            QspiWord::None,
            &[],
        )?;
        self.wait_wip()?;
        Ok(())
    }

    /// Erase the entire flash chip. This can take several minutes.
    pub fn chip_erase(&mut self) -> FlashResult<()> {
        self.write_enable()?;
        self.command_only(cmd::CHIP_ERASE)?;
        self.wait_wip()?;
        Ok(())
    }

    /// Switch to memory-mapped mode and return a [`QspiMemoryMapped`] handle.
    ///
    /// In memory-mapped mode the flash is readable at [`MEMORY_MAPPED_BASE`].
    /// The QSPI peripheral is consumed; call [`QspiMemoryMapped::exit`] to
    /// return to indirect mode.
    pub fn into_memory_mapped(mut self) -> QspiMemoryMapped {
        self.errata_2_8_3_reset();

        let rb = self.qspi.inner_mut();
        unsafe {
            // Abort any ongoing operation
            rb.cr.modify(|_, w| w.abort().set_bit());
            while rb.sr.read().busy().bit_is_set() {}

            // Clear flags
            rb.fcr.write(|w| w.bits(0x1F));

            // Configure DCR: FSIZE=25 (2^26 = 64 MB), CSHT=1, CKMODE=0
            rb.dcr.write(|w| w.bits((25 << 16) | (1 << 8)));

            // Configure CCR for memory-mapped quad read:
            // FMODE=11 (memory-mapped), DMODE=11 (4-line), DCYC=10,
            // ADSIZE=10 (24-bit), ADMODE=11 (4-line), IMODE=01 (1-line),
            // INSTRUCTION=0xEB (Fast Read Quad I/O)
            let ccr: u32 = (0b11 << 26)     // FMODE: memory-mapped
                | (0b11 << 24)              // DMODE: 4-line
                | (10 << 18)                // DCYC: 10 dummy cycles
                | (0b00 << 14)              // ABMODE: none
                | (0b10 << 12)              // ADSIZE: 24-bit
                | (0b11 << 10)              // ADMODE: 4-line
                | (0b01 << 8)               // IMODE: 1-line
                | (cmd::FAST_READ_QUAD_IO as u32);
            rb.ccr.write(|w| w.bits(ccr));
        }

        QspiMemoryMapped { qspi: self.qspi }
    }

    /// Consume the driver and return the raw HAL QSPI peripheral.
    pub fn free(self) -> Qspi<stm32h7xx_hal::pac::QUADSPI> {
        self.qspi
    }
}

/// QSPI flash in memory-mapped mode.
///
/// The flash is readable as a byte slice at [`MEMORY_MAPPED_BASE`].
/// No indirect commands can be issued while in this mode.
pub struct QspiMemoryMapped {
    qspi: Qspi<stm32h7xx_hal::pac::QUADSPI>,
}

impl QspiMemoryMapped {
    /// Get a raw pointer to the memory-mapped flash base.
    pub fn base_ptr(&self) -> *const u8 {
        MEMORY_MAPPED_BASE as *const u8
    }

    /// Read a slice from memory-mapped flash.
    ///
    /// # Safety
    ///
    /// The caller must ensure `offset + len` does not exceed [`FLASH_SIZE`]
    /// and that the MPU is configured to allow access to the QSPI region.
    pub unsafe fn read_slice(&self, offset: usize, len: usize) -> &[u8] {
        let ptr = (MEMORY_MAPPED_BASE as usize + offset) as *const u8;
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }

    /// Exit memory-mapped mode and return to indirect mode.
    pub fn exit(mut self) -> Mt25tlFlash {
        let rb = self.qspi.inner_mut();
        unsafe {
            // Abort memory-mapped mode
            rb.cr.modify(|_, w| w.abort().set_bit());
            while rb.sr.read().busy().bit_is_set() {}
            rb.fcr.write(|w| w.bits(0x1F));
        }
        let mut flash = Mt25tlFlash { qspi: self.qspi };
        // Re-apply errata reset after leaving memory-mapped mode
        flash.errata_2_8_3_reset();
        flash
    }
}
