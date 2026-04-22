//! AM335x register address map — hand-written PAC layer.
//!
//! No upstream svd2rust PAC crate exists for the AM335x. Register addresses
//! and bit fields are derived from the AM335x TRM (SPRUH73Q) and verified
//! against memalpha RAG search results.
//!
//! Constants are physical `u32` addresses. The `reg_write` / `reg_read`
//! helpers dispatch between two resolution strategies:
//!
//! - `bare_metal` feature: identity-mapped, cast `pa as *mut u32` directly.
//! - `linux` feature: look the page up in [`super::devmem`], which
//!   maintains a `/dev/mem` page cache for the running userspace process.
//!
//! Both paths are compiled from the same bsp code, so a byte-for-byte
//! identical init sequence runs under Linux (as a userspace binary) and
//! under bare-metal / FreeRTOS / Zephyr (as on-chip code).

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Volatile register access helpers
// ---------------------------------------------------------------------------

/// Write a 32-bit value to a memory-mapped register.
#[cfg(feature = "bare_metal")]
#[inline(always)]
pub unsafe fn reg_write(pa: u32, val: u32) {
    unsafe { (pa as *mut u32).write_volatile(val) }
}

/// Read a 32-bit value from a memory-mapped register.
#[cfg(feature = "bare_metal")]
#[inline(always)]
pub unsafe fn reg_read(pa: u32) -> u32 {
    unsafe { (pa as *const u32).read_volatile() }
}

/// Set bits in a memory-mapped register (read-modify-write).
#[cfg(feature = "bare_metal")]
#[inline(always)]
pub unsafe fn reg_set_bits(pa: u32, bits: u32) {
    unsafe {
        let val = (pa as *const u32).read_volatile();
        (pa as *mut u32).write_volatile(val | bits);
    }
}

/// Clear bits in a memory-mapped register (read-modify-write).
#[cfg(feature = "bare_metal")]
#[inline(always)]
pub unsafe fn reg_clear_bits(pa: u32, bits: u32) {
    unsafe {
        let val = (pa as *const u32).read_volatile();
        (pa as *mut u32).write_volatile(val & !bits);
    }
}

/// Write a 32-bit value to a memory-mapped register.
#[cfg(feature = "linux")]
#[inline(always)]
pub unsafe fn reg_write(pa: u32, val: u32) {
    unsafe { super::devmem::translate_mut(pa).write_volatile(val) }
}

/// Read a 32-bit value from a memory-mapped register.
#[cfg(feature = "linux")]
#[inline(always)]
pub unsafe fn reg_read(pa: u32) -> u32 {
    unsafe { super::devmem::translate(pa).read_volatile() }
}

/// Set bits in a memory-mapped register (read-modify-write).
#[cfg(feature = "linux")]
#[inline(always)]
pub unsafe fn reg_set_bits(pa: u32, bits: u32) {
    unsafe {
        let ptr = super::devmem::translate_mut(pa);
        let val = ptr.read_volatile();
        ptr.write_volatile(val | bits);
    }
}

/// Clear bits in a memory-mapped register (read-modify-write).
#[cfg(feature = "linux")]
#[inline(always)]
pub unsafe fn reg_clear_bits(pa: u32, bits: u32) {
    unsafe {
        let ptr = super::devmem::translate_mut(pa);
        let val = ptr.read_volatile();
        ptr.write_volatile(val & !bits);
    }
}

// ---------------------------------------------------------------------------
// PRCM — Clock Module PER domain (CM_PER)
// TRM Section 8.1.12.1
// ---------------------------------------------------------------------------

/// CM_PER base address.
pub const CM_PER: u32 = 0x44E0_0000;

pub const CM_PER_LCDC_CLKCTRL: u32 = CM_PER + 0x018;
pub const CM_PER_I2C2_CLKCTRL: u32 = CM_PER + 0x044;
pub const CM_PER_I2C1_CLKCTRL: u32 = CM_PER + 0x048;
pub const CM_PER_GPIO1_CLKCTRL: u32 = CM_PER + 0x0AC;
pub const CM_PER_GPIO2_CLKCTRL: u32 = CM_PER + 0x0B0;
pub const CM_PER_GPIO3_CLKCTRL: u32 = CM_PER + 0x0B4;
pub const CM_PER_LCDC_CLKSTCTRL: u32 = CM_PER + 0x148;

/// CM_DPLL base — holds the LCDC pixel-clock source selector.
pub const CM_DPLL: u32 = 0x44E0_0500;

/// CM_DPLL_CLKSEL_LCDC_PIXEL_CLK: bits [1:0] select LCDC pixel clock source.
/// 0x0 = DPLL_DISP_M2 (U-Boot does not init DPLL_DISP on BBB → clock dead)
/// 0x1 = DPLL_CORE_M5
/// 0x2 = DPLL_PER_M2 (CLKDCOLDO, 192 MHz, initialized by U-Boot) ← use this
/// 0x3 = CLKIN_32K (too slow for a TFT pixel clock)
pub const CM_CLKSEL_LCDC_PIXEL_CLK: u32 = CM_DPLL + 0x34;

pub const MODULEMODE_DISABLED: u32 = 0x0;
pub const MODULEMODE_ENABLE: u32 = 0x2;

// ---------------------------------------------------------------------------
// CTRLMOD — Control Module (pin mux)
// TRM Section 9.3.1.50
// ---------------------------------------------------------------------------

pub const CTRLMOD: u32 = 0x44E1_0000;

pub const CONF_LCD_DATA0: u32 = CTRLMOD + 0x8A0;
pub const CONF_LCD_DATA1: u32 = CTRLMOD + 0x8A4;
pub const CONF_LCD_DATA2: u32 = CTRLMOD + 0x8A8;
pub const CONF_LCD_DATA3: u32 = CTRLMOD + 0x8AC;
pub const CONF_LCD_DATA4: u32 = CTRLMOD + 0x8B0;
pub const CONF_LCD_DATA5: u32 = CTRLMOD + 0x8B4;
pub const CONF_LCD_DATA6: u32 = CTRLMOD + 0x8B8;
pub const CONF_LCD_DATA7: u32 = CTRLMOD + 0x8BC;
pub const CONF_LCD_DATA8: u32 = CTRLMOD + 0x8C0;
pub const CONF_LCD_DATA9: u32 = CTRLMOD + 0x8C4;
pub const CONF_LCD_DATA10: u32 = CTRLMOD + 0x8C8;
pub const CONF_LCD_DATA11: u32 = CTRLMOD + 0x8CC;
pub const CONF_LCD_DATA12: u32 = CTRLMOD + 0x8D0;
pub const CONF_LCD_DATA13: u32 = CTRLMOD + 0x8D4;
pub const CONF_LCD_DATA14: u32 = CTRLMOD + 0x8D8;
pub const CONF_LCD_DATA15: u32 = CTRLMOD + 0x8DC;
pub const CONF_LCD_DATA16: u32 = CTRLMOD + 0x8E0;
pub const CONF_LCD_DATA17: u32 = CTRLMOD + 0x8E4;
pub const CONF_LCD_DATA18: u32 = CTRLMOD + 0x8E8;
pub const CONF_LCD_DATA19: u32 = CTRLMOD + 0x8EC;
pub const CONF_LCD_DATA20: u32 = CTRLMOD + 0x8F0;
pub const CONF_LCD_DATA21: u32 = CTRLMOD + 0x8F4;
pub const CONF_LCD_DATA22: u32 = CTRLMOD + 0x8F8;
pub const CONF_LCD_DATA23: u32 = CTRLMOD + 0x8FC;

pub const CONF_LCD_VSYNC: u32 = CTRLMOD + 0x900;
pub const CONF_LCD_HSYNC: u32 = CTRLMOD + 0x904;
pub const CONF_LCD_PCLK: u32 = CTRLMOD + 0x908;
pub const CONF_LCD_AC_BIAS_EN: u32 = CTRLMOD + 0x90C;

pub const CONF_UART1_CTSN: u32 = CTRLMOD + 0x978; // I2C2_SDA in Mode 3
pub const CONF_UART1_RTSN: u32 = CTRLMOD + 0x97C; // I2C2_SCL in Mode 3

pub const PIN_MODE_0: u32 = 0x00;
pub const PIN_MODE_3: u32 = 0x03;
pub const PIN_PULL_UP_EN: u32 = (1 << 4) | (0 << 3);
pub const PIN_RX_ACTIVE: u32 = 1 << 5;
pub const PIN_SLOW_SLEW: u32 = 1 << 6;

// ---------------------------------------------------------------------------
// LCDC — LCD Controller
// TRM Section 13.5.1
// ---------------------------------------------------------------------------

pub const LCDC_BASE: u32 = 0x4830_E000;

pub const LCDC_PID: u32 = LCDC_BASE + 0x00;
pub const LCD_CTRL: u32 = LCDC_BASE + 0x04;
pub const LCDC_STAT: u32 = LCDC_BASE + 0x08;
pub const RASTER_CTRL: u32 = LCDC_BASE + 0x28;
pub const RASTER_TIMING_0: u32 = LCDC_BASE + 0x2C;
pub const RASTER_TIMING_1: u32 = LCDC_BASE + 0x30;
pub const RASTER_TIMING_2: u32 = LCDC_BASE + 0x34;
pub const LCDDMA_CTRL: u32 = LCDC_BASE + 0x40;
pub const LCDDMA_FB0_BASE: u32 = LCDC_BASE + 0x44;
pub const LCDDMA_FB0_CEILING: u32 = LCDC_BASE + 0x48;
pub const LCDDMA_FB1_BASE: u32 = LCDC_BASE + 0x4C;
pub const LCDDMA_FB1_CEILING: u32 = LCDC_BASE + 0x50;
pub const LCDC_SYSCONFIG: u32 = LCDC_BASE + 0x54;
pub const LCDC_IRQSTATUS_RAW: u32 = LCDC_BASE + 0x58;
pub const LCDC_IRQSTATUS: u32 = LCDC_BASE + 0x5C;
pub const LCDC_IRQENABLE_SET: u32 = LCDC_BASE + 0x60;
pub const LCDC_IRQENABLE_CLR: u32 = LCDC_BASE + 0x64;
pub const LCDC_CLKC_ENABLE: u32 = LCDC_BASE + 0x6C;
pub const LCDC_CLKC_RESET: u32 = LCDC_BASE + 0x70;

pub const RASTER_CTRL_LCDEN: u32 = 1 << 0;
pub const RASTER_CTRL_LCDTFT: u32 = 1 << 7;
pub const RASTER_CTRL_TFT24: u32 = 1 << 25;
pub const RASTER_CTRL_TFT24_UNPACKED: u32 = 1 << 26;
pub const RASTER_CTRL_PALMODE_DATA_ONLY: u32 = 2 << 20;

// LCDC_SYSCONFIG field values. The reset default is 0 (FORCE-IDLE +
// FORCE-STANDBY), which lets the interconnect park LCDC whenever the
// CPU isn't actively poking it — DMA stalls, panel sees no signal.
// Write SMART_IDLE_WAKEUP + SMART_STANDBY so the module stays up
// while our DMA is streaming the framebuffer.
pub const SYSCONFIG_IDLEMODE_SMART_WAKEUP: u32 = 3 << 3;
pub const SYSCONFIG_STANDBYMODE_SMART_WAKEUP: u32 = 3 << 5;
pub const SYSCONFIG_AUTOIDLE: u32 = 1 << 0;

pub const DMA_BURST_16: u32 = 4 << 4;
pub const DMA_FIFO_TH_8: u32 = 0 << 8;
pub const DMA_FRAME_MODE_SINGLE: u32 = 0 << 0;

pub const LCD_CTRL_MODESEL_RASTER: u32 = 1 << 0;

pub const CLKC_ENABLE_DMA: u32 = 1 << 2;
pub const CLKC_ENABLE_LIDD: u32 = 1 << 1;
pub const CLKC_ENABLE_CORE: u32 = 1 << 0;

pub const IRQ_EOF0: u32 = 1 << 8;
pub const IRQ_DONE: u32 = 1 << 0;
pub const IRQ_FUF: u32 = 1 << 5;

// RASTER_TIMING_2 polarity bits. Verified against AM335x TRM SPRUH73Q
// Table 13-26. Earlier bits 11/12/13 were WRONG (inside the ACB field).
pub const TIMING2_IEO: u32 = 1 << 23;
pub const TIMING2_IPC: u32 = 1 << 22;
pub const TIMING2_IHS: u32 = 1 << 21;
pub const TIMING2_IVS: u32 = 1 << 20;

// ---------------------------------------------------------------------------
// I2C2 — I2C Controller
// TRM Section 21.4
// ---------------------------------------------------------------------------

pub const I2C2_BASE: u32 = 0x4819_C000;

pub const I2C2_SYSC: u32 = I2C2_BASE + 0x10;
pub const I2C2_IRQSTATUS_RAW: u32 = I2C2_BASE + 0x24;
pub const I2C2_IRQSTATUS: u32 = I2C2_BASE + 0x28;
pub const I2C2_IRQENABLE_SET: u32 = I2C2_BASE + 0x2C;
pub const I2C2_IRQENABLE_CLR: u32 = I2C2_BASE + 0x30;
pub const I2C2_CON: u32 = I2C2_BASE + 0xA4;
pub const I2C2_CNT: u32 = I2C2_BASE + 0x98;
pub const I2C2_DATA: u32 = I2C2_BASE + 0x9C;
pub const I2C2_SA: u32 = I2C2_BASE + 0xAC;
pub const I2C2_PSC: u32 = I2C2_BASE + 0xB0;
pub const I2C2_SCLL: u32 = I2C2_BASE + 0xB4;
pub const I2C2_SCLH: u32 = I2C2_BASE + 0xB8;
pub const I2C2_BUF: u32 = I2C2_BASE + 0x94;

pub const I2C_CON_EN: u32 = 1 << 15;
pub const I2C_CON_MST: u32 = 1 << 10;
pub const I2C_CON_TRX: u32 = 1 << 9;
pub const I2C_CON_STP: u32 = 1 << 1;
pub const I2C_CON_STT: u32 = 1 << 0;

pub const I2C_IRQ_XRDY: u32 = 1 << 4;
pub const I2C_IRQ_RRDY: u32 = 1 << 3;
pub const I2C_IRQ_ARDY: u32 = 1 << 2;
pub const I2C_IRQ_NACK: u32 = 1 << 1;
pub const I2C_IRQ_AL: u32 = 1 << 0;
pub const I2C_IRQ_BF: u32 = 1 << 8;

// ---------------------------------------------------------------------------
// GPIO1
// ---------------------------------------------------------------------------

pub const GPIO1_BASE: u32 = 0x4804_C000;

pub const GPIO1_OE: u32 = GPIO1_BASE + 0x134;
pub const GPIO1_SETDATAOUT: u32 = GPIO1_BASE + 0x194;
pub const GPIO1_CLEARDATAOUT: u32 = GPIO1_BASE + 0x190;
pub const GPIO1_DATAIN: u32 = GPIO1_BASE + 0x138;

// ---------------------------------------------------------------------------
// WDT1 — watchdog timer (the one U-Boot enables)
// TRM Section 20.4
// ---------------------------------------------------------------------------

pub const WDT1_BASE: u32 = 0x44E3_5000;

pub const WDT1_WWPS: u32 = WDT1_BASE + 0x34;
pub const WDT1_WSPR: u32 = WDT1_BASE + 0x48;

// ---------------------------------------------------------------------------
// Memory map constants
// ---------------------------------------------------------------------------

pub const DDR_BASE: u32 = 0x8000_0000;
pub const SRAM_BASE: u32 = 0x402F_0400;
