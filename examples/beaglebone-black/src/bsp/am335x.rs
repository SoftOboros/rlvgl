//! AM335x register address map — hand-written PAC layer.
//!
//! No upstream svd2rust PAC crate exists for the AM335x. Register addresses
//! and bit fields are derived from the AM335x TRM (SPRUH73Q) and verified
//! against memalpha RAG search results.
//!
//! Access pattern follows `platform/src/display_init.rs`: raw `*mut u32`
//! constants with volatile read/write helpers.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Volatile register access helpers
// ---------------------------------------------------------------------------

/// Write a 32-bit value to a memory-mapped register.
#[inline(always)]
pub unsafe fn reg_write(addr: *mut u32, val: u32) {
    addr.write_volatile(val);
}

/// Read a 32-bit value from a memory-mapped register.
#[inline(always)]
pub unsafe fn reg_read(addr: *const u32) -> u32 {
    addr.read_volatile()
}

/// Set bits in a memory-mapped register (read-modify-write).
#[inline(always)]
pub unsafe fn reg_set_bits(addr: *mut u32, bits: u32) {
    let val = (addr as *const u32).read_volatile();
    addr.write_volatile(val | bits);
}

/// Clear bits in a memory-mapped register (read-modify-write).
#[inline(always)]
pub unsafe fn reg_clear_bits(addr: *mut u32, bits: u32) {
    let val = (addr as *const u32).read_volatile();
    addr.write_volatile(val & !bits);
}

// ---------------------------------------------------------------------------
// PRCM — Clock Module PER domain (CM_PER)
// TRM Section 8.1.12.1
// ---------------------------------------------------------------------------

/// CM_PER base address.
pub const CM_PER: u32 = 0x44E0_0000;

/// LCDC module clock control. MODULEMODE bits [1:0], IDLEST bits [17:16].
pub const CM_PER_LCDC_CLKCTRL: *mut u32 = (CM_PER + 0x018) as *mut u32;
/// I2C2 module clock control.
pub const CM_PER_I2C2_CLKCTRL: *mut u32 = (CM_PER + 0x044) as *mut u32;
/// I2C1 module clock control.
pub const CM_PER_I2C1_CLKCTRL: *mut u32 = (CM_PER + 0x048) as *mut u32;
/// GPIO1 module clock control. Bit 18 = OPTFCLKEN_GPIO_1_GDBCLK.
pub const CM_PER_GPIO1_CLKCTRL: *mut u32 = (CM_PER + 0x0AC) as *mut u32;
/// GPIO2 module clock control.
pub const CM_PER_GPIO2_CLKCTRL: *mut u32 = (CM_PER + 0x0B0) as *mut u32;
/// GPIO3 module clock control.
pub const CM_PER_GPIO3_CLKCTRL: *mut u32 = (CM_PER + 0x0B4) as *mut u32;
/// LCDC clock domain state transition control.
pub const CM_PER_LCDC_CLKSTCTRL: *mut u32 = (CM_PER + 0x148) as *mut u32;

// MODULEMODE values
/// Module disabled.
pub const MODULEMODE_DISABLED: u32 = 0x0;
/// Module explicitly enabled.
pub const MODULEMODE_ENABLE: u32 = 0x2;

// ---------------------------------------------------------------------------
// CTRLMOD — Control Module (pin mux)
// TRM Section 9.3.1.50: conf_<module>_<pin> at offsets 0x800–0xA34
//
// Bit fields:
//   [2:0] mmode   — mux mode (0–7)
//   [3]   puden   — 0=pull enabled, 1=pull disabled
//   [4]   putypesel — 0=pull-down, 1=pull-up
//   [5]   rxactive  — 0=rx disabled, 1=rx enabled
//   [6]   slewctrl  — 0=fast, 1=slow
// ---------------------------------------------------------------------------

/// CTRLMOD base address.
pub const CTRLMOD: u32 = 0x44E1_0000;

// LCD data pins: conf_lcd_data0 through conf_lcd_data23
// Mode 0 = LCD function for all LCD_DATA pins
pub const CONF_LCD_DATA0: *mut u32 = (CTRLMOD + 0x8A0) as *mut u32;
pub const CONF_LCD_DATA1: *mut u32 = (CTRLMOD + 0x8A4) as *mut u32;
pub const CONF_LCD_DATA2: *mut u32 = (CTRLMOD + 0x8A8) as *mut u32;
pub const CONF_LCD_DATA3: *mut u32 = (CTRLMOD + 0x8AC) as *mut u32;
pub const CONF_LCD_DATA4: *mut u32 = (CTRLMOD + 0x8B0) as *mut u32;
pub const CONF_LCD_DATA5: *mut u32 = (CTRLMOD + 0x8B4) as *mut u32;
pub const CONF_LCD_DATA6: *mut u32 = (CTRLMOD + 0x8B8) as *mut u32;
pub const CONF_LCD_DATA7: *mut u32 = (CTRLMOD + 0x8BC) as *mut u32;
pub const CONF_LCD_DATA8: *mut u32 = (CTRLMOD + 0x8C0) as *mut u32;
pub const CONF_LCD_DATA9: *mut u32 = (CTRLMOD + 0x8C4) as *mut u32;
pub const CONF_LCD_DATA10: *mut u32 = (CTRLMOD + 0x8C8) as *mut u32;
pub const CONF_LCD_DATA11: *mut u32 = (CTRLMOD + 0x8CC) as *mut u32;
pub const CONF_LCD_DATA12: *mut u32 = (CTRLMOD + 0x8D0) as *mut u32;
pub const CONF_LCD_DATA13: *mut u32 = (CTRLMOD + 0x8D4) as *mut u32;
pub const CONF_LCD_DATA14: *mut u32 = (CTRLMOD + 0x8D8) as *mut u32;
pub const CONF_LCD_DATA15: *mut u32 = (CTRLMOD + 0x8DC) as *mut u32;
// LCD_DATA[16:23] are on separate pads (active-matrix 24-bit mode)
pub const CONF_LCD_DATA16: *mut u32 = (CTRLMOD + 0x8E0) as *mut u32;
pub const CONF_LCD_DATA17: *mut u32 = (CTRLMOD + 0x8E4) as *mut u32;
pub const CONF_LCD_DATA18: *mut u32 = (CTRLMOD + 0x8E8) as *mut u32;
pub const CONF_LCD_DATA19: *mut u32 = (CTRLMOD + 0x8EC) as *mut u32;
pub const CONF_LCD_DATA20: *mut u32 = (CTRLMOD + 0x8F0) as *mut u32;
pub const CONF_LCD_DATA21: *mut u32 = (CTRLMOD + 0x8F4) as *mut u32;
pub const CONF_LCD_DATA22: *mut u32 = (CTRLMOD + 0x8F8) as *mut u32;
pub const CONF_LCD_DATA23: *mut u32 = (CTRLMOD + 0x8FC) as *mut u32;

// LCD control signals
pub const CONF_LCD_VSYNC: *mut u32 = (CTRLMOD + 0x900) as *mut u32;
pub const CONF_LCD_HSYNC: *mut u32 = (CTRLMOD + 0x904) as *mut u32;
pub const CONF_LCD_PCLK: *mut u32 = (CTRLMOD + 0x908) as *mut u32;
pub const CONF_LCD_AC_BIAS_EN: *mut u32 = (CTRLMOD + 0x90C) as *mut u32;

// I2C2 pins (P9.19 = I2C2_SCL, P9.20 = I2C2_SDA) — Mode 3
pub const CONF_UART1_CTSN: *mut u32 = (CTRLMOD + 0x978) as *mut u32; // I2C2_SDA
pub const CONF_UART1_RTSN: *mut u32 = (CTRLMOD + 0x97C) as *mut u32; // I2C2_SCL

// Pin mux mode values
pub const PIN_MODE_0: u32 = 0x00; // LCD function
pub const PIN_MODE_3: u32 = 0x03; // I2C2 function
pub const PIN_PULL_UP_EN: u32 = (1 << 4) | (0 << 3); // pull-up, pull enabled
pub const PIN_RX_ACTIVE: u32 = 1 << 5;
pub const PIN_SLOW_SLEW: u32 = 1 << 6;

// ---------------------------------------------------------------------------
// LCDC — LCD Controller
// TRM Section 13.5.1
// Base: 0x4830_E000
// ---------------------------------------------------------------------------

/// LCDC base address.
pub const LCDC_BASE: u32 = 0x4830_E000;

pub const LCDC_PID: *const u32 = (LCDC_BASE + 0x00) as *const u32;
pub const LCD_CTRL: *mut u32 = (LCDC_BASE + 0x04) as *mut u32;
pub const LCDC_STAT: *mut u32 = (LCDC_BASE + 0x08) as *mut u32;
pub const RASTER_CTRL: *mut u32 = (LCDC_BASE + 0x28) as *mut u32;
pub const RASTER_TIMING_0: *mut u32 = (LCDC_BASE + 0x2C) as *mut u32;
pub const RASTER_TIMING_1: *mut u32 = (LCDC_BASE + 0x30) as *mut u32;
pub const RASTER_TIMING_2: *mut u32 = (LCDC_BASE + 0x34) as *mut u32;
pub const LCDDMA_CTRL: *mut u32 = (LCDC_BASE + 0x40) as *mut u32;
pub const LCDDMA_FB0_BASE: *mut u32 = (LCDC_BASE + 0x44) as *mut u32;
pub const LCDDMA_FB0_CEILING: *mut u32 = (LCDC_BASE + 0x48) as *mut u32;
pub const LCDDMA_FB1_BASE: *mut u32 = (LCDC_BASE + 0x4C) as *mut u32;
pub const LCDDMA_FB1_CEILING: *mut u32 = (LCDC_BASE + 0x50) as *mut u32;
pub const LCDC_SYSCONFIG: *mut u32 = (LCDC_BASE + 0x54) as *mut u32;
pub const LCDC_IRQSTATUS_RAW: *const u32 = (LCDC_BASE + 0x58) as *const u32;
pub const LCDC_IRQSTATUS: *mut u32 = (LCDC_BASE + 0x5C) as *mut u32;
pub const LCDC_IRQENABLE_SET: *mut u32 = (LCDC_BASE + 0x60) as *mut u32;
pub const LCDC_IRQENABLE_CLR: *mut u32 = (LCDC_BASE + 0x64) as *mut u32;
pub const LCDC_CLKC_ENABLE: *mut u32 = (LCDC_BASE + 0x6C) as *mut u32;
pub const LCDC_CLKC_RESET: *mut u32 = (LCDC_BASE + 0x70) as *mut u32;

// RASTER_CTRL bit definitions
pub const RASTER_CTRL_LCDEN: u32 = 1 << 0;
pub const RASTER_CTRL_LCDTFT: u32 = 1 << 7;
pub const RASTER_CTRL_TFT24: u32 = 1 << 25;
pub const RASTER_CTRL_TFT24_UNPACKED: u32 = 1 << 26;
pub const RASTER_CTRL_PALMODE_DATA_ONLY: u32 = 2 << 20;

// LCDDMA_CTRL bit definitions
pub const DMA_BURST_16: u32 = 4 << 4;
pub const DMA_FIFO_TH_8: u32 = 0 << 8;
pub const DMA_FRAME_MODE_SINGLE: u32 = 0 << 0;

// LCD_CTRL bit definitions
pub const LCD_CTRL_MODESEL_RASTER: u32 = 1 << 0;

// CLKC_ENABLE bit definitions
pub const CLKC_ENABLE_DMA: u32 = 1 << 2;
pub const CLKC_ENABLE_LIDD: u32 = 1 << 1;
pub const CLKC_ENABLE_CORE: u32 = 1 << 0;

// IRQ bit definitions
pub const IRQ_EOF0: u32 = 1 << 8;
pub const IRQ_DONE: u32 = 1 << 0;
pub const IRQ_FUF: u32 = 1 << 5;

// RASTER_TIMING_2 bit definitions
pub const TIMING2_IPC: u32 = 1 << 11; // Invert Pixel Clock (falling edge)
pub const TIMING2_IHS: u32 = 1 << 12; // Invert HSYNC
pub const TIMING2_IVS: u32 = 1 << 13; // Invert VSYNC

// ---------------------------------------------------------------------------
// I2C2 — I2C Controller
// TRM Section 21.4 (OMAP-compatible I2C)
// Base: 0x4819_C000
// ---------------------------------------------------------------------------

/// I2C2 base address.
pub const I2C2_BASE: u32 = 0x4819_C000;

pub const I2C2_SYSC: *mut u32 = (I2C2_BASE + 0x10) as *mut u32;
pub const I2C2_IRQSTATUS_RAW: *const u32 = (I2C2_BASE + 0x24) as *const u32;
pub const I2C2_IRQSTATUS: *mut u32 = (I2C2_BASE + 0x28) as *mut u32;
pub const I2C2_IRQENABLE_SET: *mut u32 = (I2C2_BASE + 0x2C) as *mut u32;
pub const I2C2_IRQENABLE_CLR: *mut u32 = (I2C2_BASE + 0x30) as *mut u32;
pub const I2C2_CON: *mut u32 = (I2C2_BASE + 0xA4) as *mut u32;
pub const I2C2_CNT: *mut u32 = (I2C2_BASE + 0x98) as *mut u32;
pub const I2C2_DATA: *mut u32 = (I2C2_BASE + 0x9C) as *mut u32;
pub const I2C2_SA: *mut u32 = (I2C2_BASE + 0xAC) as *mut u32;
pub const I2C2_PSC: *mut u32 = (I2C2_BASE + 0xB0) as *mut u32;
pub const I2C2_SCLL: *mut u32 = (I2C2_BASE + 0xB4) as *mut u32;
pub const I2C2_SCLH: *mut u32 = (I2C2_BASE + 0xB8) as *mut u32;
pub const I2C2_BUF: *mut u32 = (I2C2_BASE + 0x94) as *mut u32;

// I2C_CON bits
pub const I2C_CON_EN: u32 = 1 << 15;
pub const I2C_CON_MST: u32 = 1 << 10;
pub const I2C_CON_TRX: u32 = 1 << 9;
pub const I2C_CON_STP: u32 = 1 << 1;
pub const I2C_CON_STT: u32 = 1 << 0;

// I2C IRQSTATUS bits
pub const I2C_IRQ_XRDY: u32 = 1 << 4;
pub const I2C_IRQ_RRDY: u32 = 1 << 3;
pub const I2C_IRQ_ARDY: u32 = 1 << 2;
pub const I2C_IRQ_NACK: u32 = 1 << 1;
pub const I2C_IRQ_AL: u32 = 1 << 0;
pub const I2C_IRQ_BF: u32 = 1 << 8;

// ---------------------------------------------------------------------------
// GPIO1 — General Purpose I/O (for backlight control)
// Base: 0x4804_C000
// ---------------------------------------------------------------------------

/// GPIO1 base address.
pub const GPIO1_BASE: u32 = 0x4804_C000;

pub const GPIO1_OE: *mut u32 = (GPIO1_BASE + 0x134) as *mut u32;
pub const GPIO1_SETDATAOUT: *mut u32 = (GPIO1_BASE + 0x194) as *mut u32;
pub const GPIO1_CLEARDATAOUT: *mut u32 = (GPIO1_BASE + 0x190) as *mut u32;
pub const GPIO1_DATAIN: *const u32 = (GPIO1_BASE + 0x138) as *const u32;

// ---------------------------------------------------------------------------
// Memory map constants
// ---------------------------------------------------------------------------

/// DDR3L base address (U-Boot initializes this).
pub const DDR_BASE: u32 = 0x8000_0000;
/// On-chip SRAM base (available without DDR).
pub const SRAM_BASE: u32 = 0x402F_0400;
