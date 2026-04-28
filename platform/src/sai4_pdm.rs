//! SAI4 PDM interface driver for the onboard MP34DT05-A digital microphone.
//!
//! Configures SAI4 sub-block A in TDM master receiver mode with the PDM
//! interface enabled, capturing raw PDM bitstream data from the MEMS
//! microphone.  The bitstream clock is output on SAI4_CK1 (PE2, AF10)
//! and data is received on SAI4_D1 (PC1, AF10).
//!
//! SAI4 lives in the D3 power domain at base `0x5800_5400`.  DMA must use
//! BDMA (not DMA1/DMA2) and buffers must reside in SRAM4 (`0x3800_0000`).
//!
//! Register definitions from RM0399 Rev 4, Section 54.6.

// ---------------------------------------------------------------------------
// SAI4 register offsets (from base 0x5800_5400)
// ---------------------------------------------------------------------------

const SAI4_BASE: u32 = 0x5800_5400;

// Sub-block A
const ACR1: u32 = 0x04;
const ACR2: u32 = 0x08;
const AFRCR: u32 = 0x0C;
const ASLOTR: u32 = 0x10;
const AIM: u32 = 0x14;
const ASR: u32 = 0x18;
const ACLRFR: u32 = 0x1C;
const ADR: u32 = 0x20;

// PDM control
const PDMCR: u32 = 0x44;
const PDMDLY: u32 = 0x48;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xCR1
// ---------------------------------------------------------------------------

const CR1_SAIEN: u32 = 1 << 16;
const CR1_DMAEN: u32 = 1 << 17;
const CR1_NODIV: u32 = 1 << 19;

// MODE[1:0] bits 0-1
const CR1_MODE_MASTER_RX: u32 = 0b01;

// DS[2:0] bits 5-7: data size
const CR1_DS_16BIT: u32 = 0b100 << 5;

// SYNCEN[1:0] bits 10-11
const CR1_SYNCEN_ASYNC: u32 = 0b00 << 10;

// MCKDIV[5:0] bits 20-25
const CR1_MCKDIV_SHIFT: u32 = 20;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xCR2
// ---------------------------------------------------------------------------

const CR2_FFLUSH: u32 = 1 << 3;
const CR2_FTH_QUARTER: u32 = 0b001;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_xFRCR
// ---------------------------------------------------------------------------

const FRCR_FSDEF: u32 = 1 << 16;
const FRCR_FSPOL: u32 = 1 << 17;

// ---------------------------------------------------------------------------
// Bit definitions for SAI_PDMCR
// ---------------------------------------------------------------------------

const PDMCR_PDMEN: u32 = 1 << 0;
// MICNBR[1:0] bits 4-5: number of microphone pairs - 1
const PDMCR_MICNBR_SHIFT: u32 = 4;
// CKEN1 bit 8: enable bitstream clock 1
const PDMCR_CKEN1: u32 = 1 << 8;

// ---------------------------------------------------------------------------
// RCC addresses
// ---------------------------------------------------------------------------

const RCC_APB4ENR: u32 = 0x5802_44F4;
// RCC_D3CCIPR offset is 0x58 per stm32h7 PAC v0.15.1
// (RegisterBlock::d3ccipr at offset 0x58). The previous constant
// 0x5802_4D2C resolved to a reserved/unmapped address — the
// `enable_clock` write silently dropped on the floor, leaving
// SAI4ASEL stuck at the reset value 0b000 = PLL1_Q. Bench-flash
// 2026-04-28 verified: PDM publish rate measured at 21 Hz vs the
// 244 Hz BENCH-FLASH.md prediction → SAI4 was running on PLL1_Q
// at an unexpected frequency. With this address corrected,
// `enable_clock(1)` properly selects PLL2_P (SAI4ASEL = 0b001).
const RCC_D3CCIPR: u32 = 0x5802_4458;

// ---------------------------------------------------------------------------
// Status register
// ---------------------------------------------------------------------------

const SR_OVRUDR: u32 = 1 << 0;

/// SAI4 PDM microphone capture driver.
pub struct Sai4Pdm {
    base: u32,
}

impl Sai4Pdm {
    /// Create a new SAI4 PDM driver.
    pub fn new() -> Self {
        Self { base: SAI4_BASE }
    }

    /// Enable SAI4 peripheral clock and configure the kernel clock source.
    ///
    /// `clock_source` selects SAI4ASEL in RCC_D3CCIPR[23:21]:
    /// - 0 = PLL1_Q
    /// - 1 = PLL2_P
    /// - 2 = PLL3_P
    /// - 3 = I2S_CKIN
    /// - 4 = PER_CK
    pub fn enable_clock(&self, clock_source: u8) {
        unsafe {
            // Enable SAI4 in APB4ENR (bit 21)
            // RCC clock-gate access — covered by a future typed Rcc handle.
            let apb4 = RCC_APB4ENR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            apb4.write_volatile(apb4.read_volatile() | (1 << 21));
            let _ = (apb4 as *const u32).read_volatile(); // readback fence // rlvgl-discipline: allow(raw_mmio_cast)

            // Set SAI4ASEL[2:0] in D3CCIPR bits [23:21]
            let d3ccipr = RCC_D3CCIPR as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            let val = d3ccipr.read_volatile();
            d3ccipr
                .write_volatile((val & !(0b111 << 21)) | (((clock_source as u32) & 0b111) << 21));
        }
    }

    /// Configure SAI4 sub-block A for PDM microphone capture.
    ///
    /// `mckdiv` is the master clock divider used to derive the bit clock.
    /// The PDM bitstream clock frequency is:
    ///   `F_PDM_CK = F_SAI4_KER_CK / (2 * MCKDIV)` when NODIV=1
    ///
    /// For a 1 mic pair with 16-bit slots:
    ///   - FRL = 15 (16 bit-clocks per frame)
    ///   - NBSLOT = 0 (1 slot of 16 bits)
    ///   - F_SCK_A = F_PDM_CK * 2 * 1 (MICNBR+1)
    pub fn configure(&self, mckdiv: u8) {
        unsafe {
            // Disable sub-block A
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() & !CR1_SAIEN);
            while acr1.read_volatile() & CR1_SAIEN != 0 {}

            // Flush FIFO
            let acr2 = self.reg(ACR2);
            acr2.write_volatile(CR2_FTH_QUARTER | CR2_FFLUSH);

            // CR1: master RX, 16-bit data, async, NODIV=1
            let cr1 = CR1_MODE_MASTER_RX
                | CR1_DS_16BIT
                | CR1_SYNCEN_ASYNC
                | CR1_NODIV
                | ((mckdiv as u32) << CR1_MCKDIV_SHIFT);
            acr1.write_volatile(cr1);

            // CR2: FIFO threshold = 1/4
            acr2.write_volatile(CR2_FTH_QUARTER);

            // Frame config for PDM with 1 mic pair:
            // FRL = 15 (16 bit-clocks), FSALL = 0 (1 active clock)
            // FSDEF = 1, FSPOL = 1 (FS active high)
            let frcr = 15u32 | FRCR_FSDEF | FRCR_FSPOL;
            self.reg(AFRCR).write_volatile(frcr);

            // Slot config: 1 slot of 16 bits, enable slot 0
            // NBSLOT = 0 (1 slot), SLOTSZ = 00 (data size), SLOTEN = bit 16
            let slotr = (0u32 << 8) | (1u32 << 16);
            self.reg(ASLOTR).write_volatile(slotr);

            // Disable all interrupts
            self.reg(AIM).write_volatile(0);

            // Clear all flags
            self.reg(ACLRFR).write_volatile(0x77);

            // PDM delay: default (no delay)
            self.reg(PDMDLY).write_volatile(0);

            // PDM control: 1 mic pair (MICNBR = 0b00), enable CK1, enable PDM
            // PDM must be enabled BEFORE SAI_A is enabled
            let pdmcr = PDMCR_PDMEN | (0b00 << PDMCR_MICNBR_SHIFT) | PDMCR_CKEN1;
            self.reg(PDMCR).write_volatile(pdmcr);
        }
    }

    /// Enable SAI4 sub-block A (start PDM capture).
    pub fn enable(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() | CR1_SAIEN);
        }
    }

    /// Disable SAI4 sub-block A (stop PDM capture).
    pub fn disable(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() & !CR1_SAIEN);
            while acr1.read_volatile() & CR1_SAIEN != 0 {}
        }
    }

    /// Enable DMA requests for sub-block A.
    pub fn enable_dma(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() | CR1_DMAEN);
        }
    }

    /// Disable DMA requests for sub-block A.
    pub fn disable_dma(&self) {
        unsafe {
            let acr1 = self.reg(ACR1);
            acr1.write_volatile(acr1.read_volatile() & !CR1_DMAEN);
        }
    }

    /// Return the address of the sub-block A data register (for DMA).
    pub fn data_register_addr(&self) -> u32 {
        self.base + ADR
    }

    /// Check and clear overrun flag. Returns `true` if overrun occurred.
    pub fn clear_overrun(&self) -> bool {
        unsafe {
            let sr = self.reg(ASR).read_volatile();
            if sr & SR_OVRUDR != 0 {
                self.reg(ACLRFR).write_volatile(SR_OVRUDR);
                true
            } else {
                false
            }
        }
    }

    #[inline(always)]
    unsafe fn reg(&self, offset: u32) -> *mut u32 {
        // SAI4 PDM peripheral access via base+offset helper. Same
        // deferral as SAI1 in `sai.rs` — a typed register module is
        // future work.
        (self.base + offset) as *mut u32 // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    }
}
