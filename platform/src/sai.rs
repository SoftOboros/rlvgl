//! SAI1 (Serial Audio Interface) driver for I2S audio on the STM32H747.
//!
//! Provides a raw register driver for SAI1 sub-block A (transmit) and sub-block B
//! (receive).  Configured for I2S master TX with 16-bit stereo at 48 kHz by default.
//! Sub-block B operates as a synchronous slave receiver.
//!
//! Register definitions from RM0399 Rev 4, Section 54.6.

// ---------------------------------------------------------------------------
// SAI1 register offsets (from base 0x4001_5800)
// ---------------------------------------------------------------------------

const SAI1_BASE: u32 = 0x4001_5800;

// Global
const GCR: u32 = 0x00;

// Sub-block A
const ACR1: u32 = 0x04;
const ACR2: u32 = 0x08;
const AFRCR: u32 = 0x0C;
const ASLOTR: u32 = 0x10;
const AIM: u32 = 0x14;
const ASR: u32 = 0x18;
const ACLRFR: u32 = 0x1C;
const ADR: u32 = 0x20;

// Sub-block B
const BCR1: u32 = 0x24;
const BCR2: u32 = 0x28;
const BFRCR: u32 = 0x2C;
const BSLOTR: u32 = 0x30;
const BIM: u32 = 0x34;
const BSR: u32 = 0x38;
const BCLRFR: u32 = 0x3C;
const BDR: u32 = 0x40;

// PDM control
const PDMCR: u32 = 0x44;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xCR1
// ---------------------------------------------------------------------------

const CR1_SAIEN: u32 = 1 << 16;
const CR1_DMAEN: u32 = 1 << 17;
const CR1_NODIV: u32 = 1 << 19;
const CR1_MCKEN: u32 = 1 << 27;

// MODE[1:0] bits 0-1
const CR1_MODE_MASTER_TX: u32 = 0b00;
const CR1_MODE_MASTER_RX: u32 = 0b01;
const CR1_MODE_SLAVE_TX: u32 = 0b10;
const CR1_MODE_SLAVE_RX: u32 = 0b11;

// DS[2:0] bits 5-7: data size
const CR1_DS_16BIT: u32 = 0b100 << 5;

// SYNCEN[1:0] bits 10-11: sync mode
const CR1_SYNCEN_ASYNC: u32 = 0b00 << 10;
const CR1_SYNCEN_INTERNAL: u32 = 0b01 << 10; // sync with other sub-block

// OUTDRIV bit 13
const CR1_OUTDRIV: u32 = 1 << 13;

// MCKDIV[5:0] bits 20-25
const CR1_MCKDIV_SHIFT: u32 = 20;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xCR2
// ---------------------------------------------------------------------------

const CR2_FFLUSH: u32 = 1 << 3;

// FTH[2:0] bits 0-2: FIFO threshold
const CR2_FTH_QUARTER: u32 = 0b001;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xFRCR
// ---------------------------------------------------------------------------

// FRL[7:0] bits 0-7: frame length - 1
// FSALL[6:0] bits 8-14: FS active length - 1
const FRCR_FSDEF: u32 = 1 << 16; // FS is start of frame + channel identification
const FRCR_FSPOL: u32 = 1 << 17; // FS active high (0 = active low for I2S)
const FRCR_FSOFF: u32 = 1 << 18; // FS one bit before first data bit

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xSLOTR
// ---------------------------------------------------------------------------

// NBSLOT[3:0] bits 8-11: number of slots - 1
// SLOTSZ[1:0] bits 6-7: 00=data size, 01=16-bit, 10=32-bit
// SLOTEN[15:0] bits 16-31: slot enable mask

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xSR
// ---------------------------------------------------------------------------

const SR_FREQ: u32 = 1 << 3; // FIFO request (ready for data)
const SR_OVRUDR: u32 = 1 << 0; // overrun/underrun

// ---------------------------------------------------------------------------
// RCC addresses for SAI1 clocking
// ---------------------------------------------------------------------------

const RCC_APB2ENR: u32 = 0x5802_44F0;
const RCC_D2CCIP1R: u32 = 0x5802_4D28;

/// SAI1 audio driver.
///
/// Provides I2S master transmit on sub-block A and synchronous slave receive
/// on sub-block B.
pub struct Sai1Audio {
    base: u32,
}

impl Sai1Audio {
    /// Create a new SAI1 driver at the default base address.
    pub fn new() -> Self {
        Self { base: SAI1_BASE }
    }

    /// Enable SAI1 peripheral clock and configure the kernel clock source.
    ///
    /// `clock_source`:
    /// - 0 = PLL1_Q
    /// - 1 = PLL2_P
    /// - 2 = PLL3_P
    /// - 3 = I2S_CKIN
    /// - 4 = PER_CK
    pub fn enable_clock(&self, clock_source: u8) {
        unsafe {
            // Enable SAI1 in APB2ENR (bit 22)
            let apb2 = RCC_APB2ENR as *mut u32;
            apb2.write_volatile(apb2.read_volatile() | (1 << 22));
            // Readback fence
            let _ = (apb2 as *const u32).read_volatile();

            // Set SAI1SEL in D2CCIP1R[2:0]
            let d2ccip1r = RCC_D2CCIP1R as *mut u32;
            let val = d2ccip1r.read_volatile();
            d2ccip1r.write_volatile((val & !0b111) | ((clock_source as u32) & 0b111));
        }
    }

    /// Configure sub-block A as I2S master transmitter.
    ///
    /// `mckdiv` is the master clock divider (0 = /1, 1 = /2, etc.).
    /// Frame: 32 bit-clocks, 16-bit left + 16-bit right, I2S Philips standard.
    pub fn configure_tx(&self, mckdiv: u8) {
        unsafe {
            // Ensure sub-block A is disabled
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() & !CR1_SAIEN);
            // Wait until disabled
            while acr1.read_volatile() & CR1_SAIEN != 0 {}

            // Flush FIFO
            let acr2 = self.reg(ACR2);
            acr2.write_volatile(CR2_FTH_QUARTER | CR2_FFLUSH);

            // Global config: no external sync
            self.reg(GCR).write_volatile(0);

            // CR1: master TX, 16-bit, async, MCLK output enabled, OUTDRIV
            let cr1 = CR1_MODE_MASTER_TX
                | CR1_DS_16BIT
                | CR1_SYNCEN_ASYNC
                | CR1_OUTDRIV
                | CR1_MCKEN
                | ((mckdiv as u32) << CR1_MCKDIV_SHIFT);
            acr1.write_volatile(cr1);

            // CR2: FIFO threshold = 1/4
            acr2.write_volatile(CR2_FTH_QUARTER);

            // Frame config: FRL=31 (32 clocks), FSALL=15 (16 active),
            // FSDEF=1 (channel ID), FSPOL=0 (active low = I2S), FSOFF=1
            let frcr = 31u32         // FRL = 32 - 1
                | (15u32 << 8)       // FSALL = 16 - 1
                | FRCR_FSDEF
                | FRCR_FSOFF;
            // FSPOL = 0 (I2S active low) - don't set FRCR_FSPOL
            self.reg(AFRCR).write_volatile(frcr);

            // Slot config: 2 slots (NBSLOT=1), slot size = data size,
            // enable slots 0 and 1
            let slotr = (1u32 << 8)        // NBSLOT = 2 - 1
                | (0b11u32 << 16);          // SLOTEN: slots 0 and 1
            self.reg(ASLOTR).write_volatile(slotr);

            // Disable all interrupts
            self.reg(AIM).write_volatile(0);

            // Clear all flags
            self.reg(ACLRFR).write_volatile(0x77);
        }
    }

    /// Configure sub-block B as synchronous slave receiver.
    ///
    /// Must be called after `configure_tx()`.  Sub-block B uses the clock
    /// and frame sync from sub-block A.
    pub fn configure_rx(&self) {
        unsafe {
            // Ensure sub-block B is disabled
            let bcr1 = self.reg(BCR1);
            bcr1.write_volatile(bcr1.read_volatile() & !CR1_SAIEN);
            while bcr1.read_volatile() & CR1_SAIEN != 0 {}

            // Flush FIFO
            let bcr2 = self.reg(BCR2);
            bcr2.write_volatile(CR2_FTH_QUARTER | CR2_FFLUSH);

            // CR1: slave RX, 16-bit, synchronous with sub-block A
            let cr1 = CR1_MODE_SLAVE_RX
                | CR1_DS_16BIT
                | CR1_SYNCEN_INTERNAL;
            bcr1.write_volatile(cr1);

            // CR2: FIFO threshold = 1/4
            bcr2.write_volatile(CR2_FTH_QUARTER);

            // Frame config: same as sub-block A (ignored when synchronous,
            // but set for consistency)
            let frcr = 31u32 | (15u32 << 8) | FRCR_FSDEF | FRCR_FSOFF;
            self.reg(BFRCR).write_volatile(frcr);

            // Slot config: same as A
            let slotr = (1u32 << 8) | (0b11u32 << 16);
            self.reg(BSLOTR).write_volatile(slotr);

            // Disable interrupts, clear flags
            self.reg(BIM).write_volatile(0);
            self.reg(BCLRFR).write_volatile(0x77);
        }
    }

    /// Enable sub-block A (transmitter).
    ///
    /// Call `configure_tx()` before this.
    pub fn enable_tx(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() | CR1_SAIEN);
        }
    }

    /// Enable sub-block B (receiver).
    ///
    /// Should be enabled before sub-block A when both are used, per RM0399.
    pub fn enable_rx(&self) {
        unsafe {
            let bcr1 = self.reg(BCR1);
            bcr1.write_volatile(bcr1.read_volatile() | CR1_SAIEN);
        }
    }

    /// Disable sub-block A.
    pub fn disable_tx(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() & !CR1_SAIEN);
            while acr1.read_volatile() & CR1_SAIEN != 0 {}
        }
    }

    /// Disable sub-block B.
    pub fn disable_rx(&self) {
        unsafe {
            let bcr1 = self.reg(BCR1);
            bcr1.write_volatile(bcr1.read_volatile() & !CR1_SAIEN);
            while bcr1.read_volatile() & CR1_SAIEN != 0 {}
        }
    }

    /// Returns `true` when the TX FIFO can accept more data.
    pub fn tx_fifo_ready(&self) -> bool {
        unsafe { self.reg(ASR).read_volatile() & SR_FREQ != 0 }
    }

    /// Returns `true` when the RX FIFO has data available.
    pub fn rx_fifo_ready(&self) -> bool {
        unsafe { self.reg(BSR).read_volatile() & SR_FREQ != 0 }
    }

    /// Write a 32-bit sample to the TX FIFO (blocking until space is available).
    ///
    /// For 16-bit stereo, the sample packs left in the upper 16 bits and right
    /// in the lower 16 bits.
    pub fn write_sample(&self, sample: u32) {
        while !self.tx_fifo_ready() {}
        unsafe {
            self.reg(ADR).write_volatile(sample);
        }
    }

    /// Read a 32-bit sample from the RX FIFO (blocking until data is available).
    pub fn read_sample(&self) -> u32 {
        while !self.rx_fifo_ready() {}
        unsafe { self.reg(BDR).read_volatile() }
    }

    /// Return the address of the TX data register (for DMA configuration).
    pub fn tx_data_register_addr(&self) -> u32 {
        self.base + ADR
    }

    /// Return the address of the RX data register (for DMA configuration).
    pub fn rx_data_register_addr(&self) -> u32 {
        self.base + BDR
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    #[inline(always)]
    unsafe fn reg(&self, offset: u32) -> *mut u32 {
        (self.base + offset) as *mut u32
    }
}
