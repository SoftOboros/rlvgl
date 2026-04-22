//! DMA1 driver for SAI1 sub-block A transmit streaming.
//!
//! Configures DMA1 Stream 0 in double-buffer circular mode to feed PCM samples
//! to the SAI1_A data register without CPU intervention.  Uses DMAMUX1 to route
//! the SAI1_A DMA request (input 87) to the stream.
//!
//! Register definitions from RM0399 Rev 4, Sections 16 and 18.

// ---------------------------------------------------------------------------
// DMA1 register addresses
// ---------------------------------------------------------------------------

const DMA1_BASE: u32 = 0x4002_0000;

/// Low interrupt status register (streams 0–3).
const DMA1_LISR: u32 = DMA1_BASE + 0x000;
/// Low interrupt flag clear register (streams 0–3).
const DMA1_LIFCR: u32 = DMA1_BASE + 0x008;

// Stream 0 registers: base + 0x010 + 0x18 * stream
const S0_BASE: u32 = DMA1_BASE + 0x010;
const S0CR: u32 = S0_BASE + 0x00;
const S0NDTR: u32 = S0_BASE + 0x04;
const S0PAR: u32 = S0_BASE + 0x08;
const S0M0AR: u32 = S0_BASE + 0x0C;
const S0M1AR: u32 = S0_BASE + 0x10;
const S0FCR: u32 = S0_BASE + 0x14;

// ---------------------------------------------------------------------------
// DMAMUX1 register addresses
// ---------------------------------------------------------------------------

const DMAMUX1_BASE: u32 = 0x4002_0800;
/// Channel 0 configuration register (maps to DMA1 Stream 0).
const DMAMUX1_C0CR: u32 = DMAMUX1_BASE + 0x000;

// ---------------------------------------------------------------------------
// RCC
// ---------------------------------------------------------------------------

const RCC_AHB1ENR: u32 = 0x5802_44D8;

// ---------------------------------------------------------------------------
// DMA_SxCR bit definitions
// ---------------------------------------------------------------------------

const CR_EN: u32 = 1 << 0;
#[allow(dead_code)]
const CR_TCIE: u32 = 1 << 4; // transfer complete interrupt enable
#[allow(dead_code)]
const CR_HTIE: u32 = 1 << 3; // half-transfer interrupt enable
const CR_DIR_M2P: u32 = 0b01 << 6;
const CR_CIRC: u32 = 1 << 8;
const CR_MINC: u32 = 1 << 10;
// PSIZE = 16-bit (01)
const CR_PSIZE_16: u32 = 0b01 << 11;
// MSIZE = 16-bit (01)
const CR_MSIZE_16: u32 = 0b01 << 13;
// Priority level high (10)
const CR_PL_HIGH: u32 = 0b10 << 16;
const CR_DBM: u32 = 1 << 18;
const CR_CT: u32 = 1 << 19;
/// Target RAM bufferable (improves AXI bus utilization for SDRAM targets).
const CR_TRBUFF: u32 = 1 << 20;

// ---------------------------------------------------------------------------
// DMA_SxFCR bit definitions
// ---------------------------------------------------------------------------

/// Disable direct mode (enable FIFO).
const FCR_DMDIS: u32 = 1 << 2;
/// FIFO threshold = full (11).
const FCR_FTH_FULL: u32 = 0b11;

// ---------------------------------------------------------------------------
// LISR / LIFCR bit positions for stream 0
// ---------------------------------------------------------------------------

const LISR_TCIF0: u32 = 1 << 5;
const LISR_HTIF0: u32 = 1 << 4;
const LISR_TEIF0: u32 = 1 << 3;
const LISR_DMEIF0: u32 = 1 << 2;
const LISR_FEIF0: u32 = 1 << 0;
/// All interrupt flags for stream 0.
const LIFCR_ALL_S0: u32 = LISR_TCIF0 | LISR_HTIF0 | LISR_TEIF0 | LISR_DMEIF0 | LISR_FEIF0;

// ---------------------------------------------------------------------------
// SAI1_A DMAMUX request ID
// ---------------------------------------------------------------------------

/// DMAMUX1 request input for SAI1 sub-block A (RM0399 Table 126).
const SAI1A_DMAREQ_ID: u32 = 87;

/// DMA1 Stream 0 driver for SAI1_A double-buffer TX streaming.
pub struct DmaSai1Tx {
    /// Number of 16-bit transfers per buffer (set during configure).
    ndtr: u16,
}

impl DmaSai1Tx {
    /// Create a new (unconfigured) driver instance.
    pub fn new() -> Self {
        Self { ndtr: 0 }
    }

    /// Enable the DMA1 peripheral clock.
    pub fn enable_clock(&self) {
        unsafe {
            let reg = RCC_AHB1ENR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            reg.write_volatile(reg.read_volatile() | 1); // bit 0 = DMA1EN
            // Readback fence
            let _ = (reg as *const u32).read_volatile(); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
    }

    /// Configure DMA1 Stream 0 for double-buffer memory-to-peripheral transfers.
    ///
    /// - `periph_addr`: SAI1_A data register address (`Sai1Audio::tx_data_register_addr()`)
    /// - `buf0`, `buf1`: pointers to the two SDRAM audio buffers
    /// - `buf_bytes`: size of each buffer in bytes (must be even; transfers are 16-bit)
    ///
    /// The stream is left disabled after configuration; call `start()` to begin.
    pub fn configure(
        &mut self,
        periph_addr: u32,
        buf0: *const u8,
        buf1: *const u8,
        buf_bytes: usize,
    ) {
        let transfers = (buf_bytes / 2) as u16; // 16-bit transfers
        self.ndtr = transfers;

        unsafe {
            // Ensure stream is disabled
            let cr = S0CR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            cr.write_volatile(cr.read_volatile() & !CR_EN);
            while (cr as *const u32).read_volatile() & CR_EN != 0 {} // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Clear all interrupt flags for stream 0
            (DMA1_LIFCR as *mut u32).write_volatile(LIFCR_ALL_S0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Number of data items
            (S0NDTR as *mut u32).write_volatile(transfers as u32); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Peripheral address (SAI1_A DR)
            (S0PAR as *mut u32).write_volatile(periph_addr); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Memory addresses
            (S0M0AR as *mut u32).write_volatile(buf0 as u32); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            (S0M1AR as *mut u32).write_volatile(buf1 as u32); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // FIFO control: enable FIFO, full threshold
            (S0FCR as *mut u32).write_volatile(FCR_DMDIS | FCR_FTH_FULL); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Configure DMAMUX1 channel 0 → SAI1_A (request ID 87)
            (DMAMUX1_C0CR as *mut u32).write_volatile(SAI1A_DMAREQ_ID); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

            // Stream configuration: M2P, circular, double-buffer, 16-bit, high priority
            let cr_val = CR_DIR_M2P
                | CR_CIRC
                | CR_MINC
                | CR_PSIZE_16
                | CR_MSIZE_16
                | CR_PL_HIGH
                | CR_DBM
                | CR_TRBUFF;
            // CT=0 means DMA starts reading from M0AR (buf0)
            cr.write_volatile(cr_val);
        }
    }

    /// Start DMA streaming.
    ///
    /// Both buffers should already contain valid audio data before calling this.
    pub fn start(&self) {
        unsafe {
            let cr = S0CR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            cr.write_volatile(cr.read_volatile() | CR_EN);
        }
    }

    /// Stop DMA streaming.
    pub fn stop(&self) {
        unsafe {
            let cr = S0CR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            cr.write_volatile(cr.read_volatile() & !CR_EN);
            while (cr as *const u32).read_volatile() & CR_EN != 0 {} // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            // Clear flags
            (DMA1_LIFCR as *mut u32).write_volatile(LIFCR_ALL_S0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
    }

    /// Returns which buffer the DMA is currently reading from (0 or 1).
    ///
    /// CT=0 → DMA reads M0AR (buf0), CT=1 → DMA reads M1AR (buf1).
    pub fn current_target(&self) -> u8 {
        unsafe {
            let cr = (S0CR as *const u32).read_volatile(); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            if cr & CR_CT != 0 { 1 } else { 0 }
        }
    }

    /// Returns `true` if the transfer-complete flag is set for stream 0.
    ///
    /// This flag is set each time the DMA finishes one full buffer and switches
    /// to the other.
    pub fn transfer_complete(&self) -> bool {
        unsafe { (DMA1_LISR as *const u32).read_volatile() & LISR_TCIF0 != 0 } // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    }

    /// Returns `true` if the half-transfer flag is set.
    pub fn half_transfer(&self) -> bool {
        unsafe { (DMA1_LISR as *const u32).read_volatile() & LISR_HTIF0 != 0 } // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    }

    /// Clear the transfer-complete interrupt flag.
    pub fn clear_transfer_complete(&self) {
        unsafe {
            (DMA1_LIFCR as *mut u32).write_volatile(LISR_TCIF0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
    }

    /// Clear the half-transfer interrupt flag.
    pub fn clear_half_transfer(&self) {
        unsafe {
            (DMA1_LIFCR as *mut u32).write_volatile(LISR_HTIF0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
    }

    /// Clear all DMA stream 0 interrupt flags.
    pub fn clear_all_flags(&self) {
        unsafe {
            (DMA1_LIFCR as *mut u32).write_volatile(LIFCR_ALL_S0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
    }

    /// Returns the number of 16-bit transfers per buffer.
    pub fn transfers_per_buffer(&self) -> u16 {
        self.ndtr
    }
}
