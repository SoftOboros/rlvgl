//! BDMA (Basic DMA) driver for SAI4 PDM microphone capture.
//!
//! Configures BDMA Channel 1 in circular mode with half-transfer and
//! transfer-complete interrupts for ping-pong buffering of PDM data from
//! SAI4 sub-block A.
//!
//! BDMA and SAI4 are both in the D3 power domain.  Buffers **must** reside
//! in SRAM4 (`0x3800_0000`, 64 KB) — BDMA cannot access SDRAM or D1/D2 SRAM.
//!
//! Register definitions from RM0399 Rev 4, Section 17.6 (BDMA) and 18.6 (DMAMUX2).

// ---------------------------------------------------------------------------
// BDMA register addresses (base 0x5802_5400)
// ---------------------------------------------------------------------------

const BDMA_BASE: u32 = 0x5802_5400;

/// Interrupt status register.
const BDMA_ISR: u32 = BDMA_BASE + 0x00;
/// Interrupt flag clear register.
const BDMA_IFCR: u32 = BDMA_BASE + 0x04;

// Channel 1 registers: base + 0x08 + 0x14 * channel
const CH1_BASE: u32 = BDMA_BASE + 0x08 + 0x14 * 1;
const CH1_CCR: u32 = CH1_BASE + 0x00;
const CH1_CNDTR: u32 = CH1_BASE + 0x04;
const CH1_CPAR: u32 = CH1_BASE + 0x08;
const CH1_CM0AR: u32 = CH1_BASE + 0x0C;
const CH1_CM1AR: u32 = CH1_BASE + 0x10;

// ---------------------------------------------------------------------------
// DMAMUX2 register addresses (base 0x5802_5800)
// ---------------------------------------------------------------------------

const DMAMUX2_BASE: u32 = 0x5802_5800;
/// Channel 1 configuration register (maps to BDMA Channel 1).
const DMAMUX2_C1CR: u32 = DMAMUX2_BASE + 0x04;

// ---------------------------------------------------------------------------
// RCC
// ---------------------------------------------------------------------------

const RCC_AHB4ENR: u32 = 0x5802_44E0;

// ---------------------------------------------------------------------------
// BDMA_CCRx bit definitions
// ---------------------------------------------------------------------------

const CCR_EN: u32 = 1 << 0;
const CCR_TCIE: u32 = 1 << 1;
const CCR_HTIE: u32 = 1 << 2;
#[allow(dead_code)]
const CCR_TEIE: u32 = 1 << 3;
// DIR = 0: peripheral-to-memory (read from peripheral)
const CCR_MINC: u32 = 1 << 7;
// PSIZE[1:0] bits 8-9: 01 = 16-bit
const CCR_PSIZE_16: u32 = 0b01 << 8;
// MSIZE[1:0] bits 10-11: 01 = 16-bit
const CCR_MSIZE_16: u32 = 0b01 << 10;
// PL[1:0] bits 12-13: 10 = high priority
const CCR_PL_HIGH: u32 = 0b10 << 12;
const CCR_CIRC: u32 = 1 << 5;
const CCR_DBM: u32 = 1 << 15;
const CCR_CT: u32 = 1 << 16;

// ---------------------------------------------------------------------------
// ISR / IFCR bit positions for channel 1
// ---------------------------------------------------------------------------

const ISR_GIF1: u32 = 1 << 4;
const ISR_TCIF1: u32 = 1 << 5;
const ISR_HTIF1: u32 = 1 << 6;
const ISR_TEIF1: u32 = 1 << 7;
/// All flags for channel 1.
const IFCR_ALL_CH1: u32 = ISR_GIF1 | ISR_TCIF1 | ISR_HTIF1 | ISR_TEIF1;

// ---------------------------------------------------------------------------
// DMAMUX2 request ID for SAI4_A
// ---------------------------------------------------------------------------

/// DMAMUX2 request input for SAI4 sub-block A (from HAL: BDMA_REQUEST_SAI4_A = 15).
const SAI4A_DMAREQ_ID: u32 = 15;

/// BDMA Channel 1 driver for SAI4 PDM capture with double-buffering.
pub struct BdmaSai4Rx {
    ndtr: u16,
}

impl BdmaSai4Rx {
    /// Create a new (unconfigured) driver instance.
    pub fn new() -> Self {
        Self { ndtr: 0 }
    }

    /// Enable the BDMA peripheral clock.
    pub fn enable_clock(&self) {
        unsafe {
            let reg = RCC_AHB4ENR as *mut u32;
            reg.write_volatile(reg.read_volatile() | (1 << 21)); // bit 21 = BDMAEN
            let _ = (reg as *const u32).read_volatile(); // readback fence
        }
    }

    /// Configure BDMA Channel 1 for double-buffer peripheral-to-memory transfers.
    ///
    /// - `periph_addr`: SAI4_A data register address
    /// - `buf0`, `buf1`: pointers to SRAM4 buffers (must be in `0x3800_xxxx`)
    /// - `buf_bytes`: size of each buffer in bytes (must be even; transfers are 16-bit)
    pub fn configure(
        &mut self,
        periph_addr: u32,
        buf0: *mut u16,
        buf1: *mut u16,
        buf_bytes: usize,
    ) {
        let transfers = (buf_bytes / 2) as u16;
        self.ndtr = transfers;

        unsafe {
            // Disable channel
            let ccr = CH1_CCR as *mut u32;
            ccr.write_volatile(ccr.read_volatile() & !CCR_EN);
            while (ccr as *const u32).read_volatile() & CCR_EN != 0 {}

            // Clear all flags for channel 1
            (BDMA_IFCR as *mut u32).write_volatile(IFCR_ALL_CH1);

            // Number of data items
            (CH1_CNDTR as *mut u32).write_volatile(transfers as u32);

            // Peripheral address (SAI4_A DR)
            (CH1_CPAR as *mut u32).write_volatile(periph_addr);

            // Memory buffer addresses
            (CH1_CM0AR as *mut u32).write_volatile(buf0 as u32);
            (CH1_CM1AR as *mut u32).write_volatile(buf1 as u32);

            // Configure DMAMUX2 channel 1 → SAI4_A (request ID 15)
            (DMAMUX2_C1CR as *mut u32).write_volatile(SAI4A_DMAREQ_ID);

            // Channel configuration:
            // DIR = 0 (periph-to-memory), circular, double-buffer,
            // 16-bit peripheral & memory, memory increment, high priority,
            // half-transfer + transfer-complete interrupts
            let ccr_val = CCR_CIRC
                | CCR_MINC
                | CCR_PSIZE_16
                | CCR_MSIZE_16
                | CCR_PL_HIGH
                | CCR_DBM
                | CCR_TCIE
                | CCR_HTIE;
            ccr.write_volatile(ccr_val);
        }
    }

    /// Start BDMA transfer.
    pub fn start(&self) {
        unsafe {
            let ccr = CH1_CCR as *mut u32;
            ccr.write_volatile(ccr.read_volatile() | CCR_EN);
        }
    }

    /// Stop BDMA transfer.
    pub fn stop(&self) {
        unsafe {
            let ccr = CH1_CCR as *mut u32;
            ccr.write_volatile(ccr.read_volatile() & !CCR_EN);
            while (ccr as *const u32).read_volatile() & CCR_EN != 0 {}
            (BDMA_IFCR as *mut u32).write_volatile(IFCR_ALL_CH1);
        }
    }

    /// Returns which buffer the BDMA is currently writing to (0 or 1).
    pub fn current_target(&self) -> u8 {
        unsafe {
            if (CH1_CCR as *const u32).read_volatile() & CCR_CT != 0 {
                1
            } else {
                0
            }
        }
    }

    /// Returns `true` if the transfer-complete flag is set.
    pub fn transfer_complete(&self) -> bool {
        unsafe { (BDMA_ISR as *const u32).read_volatile() & ISR_TCIF1 != 0 }
    }

    /// Returns `true` if the half-transfer flag is set.
    pub fn half_transfer(&self) -> bool {
        unsafe { (BDMA_ISR as *const u32).read_volatile() & ISR_HTIF1 != 0 }
    }

    /// Clear the transfer-complete flag.
    pub fn clear_transfer_complete(&self) {
        unsafe {
            (BDMA_IFCR as *mut u32).write_volatile(ISR_TCIF1);
        }
    }

    /// Clear the half-transfer flag.
    pub fn clear_half_transfer(&self) {
        unsafe {
            (BDMA_IFCR as *mut u32).write_volatile(ISR_HTIF1);
        }
    }

    /// Clear all channel 1 flags.
    pub fn clear_all_flags(&self) {
        unsafe {
            (BDMA_IFCR as *mut u32).write_volatile(IFCR_ALL_CH1);
        }
    }

    /// Returns the number of 16-bit transfers per buffer.
    pub fn transfers_per_buffer(&self) -> u16 {
        self.ndtr
    }
}
