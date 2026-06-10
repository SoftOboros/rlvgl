//! Full DSI + LTDC init sequence for STM32H747I-DISCO + NT35510 panel.
//!
//! Raw-register implementation of the bare-metal init sequence, callable
//! from any context (no PAC peripheral ownership required). Used by the
//! Zephyr `adapted_cmd` path to bypass Zephyr's video-mode display drivers
//! and bring the DSI up in adapted command mode from scratch.
//!
//! # Reference
//!
//! - RM0399 Rev 3 §34.14 "Programming procedure"
//! - STM32CubeH7 stm32h747i_discovery_lcd.c (HAL_DSI_Init,
//!   HAL_DSI_ConfigAdaptedCommandMode, HAL_DSI_Start)
//! - The proven inline sequence in `stm32h747i_disco.rs::new()`
//!
//! # Preconditions
//!
//! - PLL3 is running and feeding LTDC pixel clock at ~32 MHz
//! - DSI/LTDC/DMA2D peripheral clocks are enabled (RCC AHB3ENR.DMA2D,
//!   APB3ENR.LTDC, APB3ENR.DSI). On Zephyr, enabling these via the
//!   DTS nodes (or via clock-control API) handles this. On bare-metal
//!   the HAL `peripheral.X.enable()` calls do it.
//! - HSE = 25 MHz on the H747I-DISCO
//! - GPIOG and GPIOJ clocks enabled (for panel reset PG3, backlight PJ12,
//!   TE input PJ2)

use crate::dsi_cmd_mode;

// ── DSI host register addresses (base = 0x5000_0000) ─────────────────────────

const DSI: u32 = 0x5000_0000;
const DSI_VR: *const u32 = (DSI + 0x000) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_CR: *mut u32 = (DSI + 0x004) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_CCR: *mut u32 = (DSI + 0x008) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_LVCIDR: *mut u32 = (DSI + 0x00C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_LCOLCR: *mut u32 = (DSI + 0x010) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_LPCR: *mut u32 = (DSI + 0x014) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_LPMCR: *mut u32 = (DSI + 0x018) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_PCR: *mut u32 = (DSI + 0x02C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_GVCIDR: *mut u32 = (DSI + 0x030) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_MCR: *mut u32 = (DSI + 0x034) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VMCR: *mut u32 = (DSI + 0x038) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VPCR: *mut u32 = (DSI + 0x03C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VCCR: *mut u32 = (DSI + 0x040) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VNPCR: *mut u32 = (DSI + 0x044) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VHSACR: *mut u32 = (DSI + 0x048) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VHBPCR: *mut u32 = (DSI + 0x04C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VLCR: *mut u32 = (DSI + 0x050) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VVSACR: *mut u32 = (DSI + 0x054) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VVBPCR: *mut u32 = (DSI + 0x058) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VVFPCR: *mut u32 = (DSI + 0x05C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_VVACR: *mut u32 = (DSI + 0x060) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
// LCCR (LTDC command configuration) lives at 0x64 — NOT 0x2C (which is PCR).
// Verified against stm32h7-0.15.1 PAC and RM0399 §34.16. The previous 0x2C
// alias caused configure_adapted_cmd_mode(width) to corrupt PCR and leave
// CMDSIZE=0, producing snow on the panel.
const DSI_LCCR: *mut u32 = (DSI + 0x064) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_CMCR: *mut u32 = (DSI + 0x068) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_GHCR: *mut u32 = (DSI + 0x06C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_GPSR: *const u32 = (DSI + 0x074) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_TCCR0: *mut u32 = (DSI + 0x078) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_CLCR: *mut u32 = (DSI + 0x094) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_CLTCR: *mut u32 = (DSI + 0x098) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_DLTCR: *mut u32 = (DSI + 0x09C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_PCTLR: *mut u32 = (DSI + 0x0A0) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_PCONFR: *mut u32 = (DSI + 0x0A4) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_PSR: *const u32 = (DSI + 0x0B0) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// DSI wrapper registers (base = DSI + 0x400)
const DSI_W: u32 = DSI + 0x400;
const DSI_WCR: *mut u32 = (DSI_W + 0x04) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_WIER: *mut u32 = (DSI_W + 0x08) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_WISR: *const u32 = (DSI_W + 0x0C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_WIFCR: *mut u32 = (DSI_W + 0x10) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_WPCR0: *mut u32 = (DSI_W + 0x18) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const DSI_WRPCR: *mut u32 = (DSI_W + 0x30) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// ── LTDC register addresses (base = 0x5000_1000) ─────────────────────────────

const LTDC: u32 = 0x5000_1000;
const LTDC_SSCR: *mut u32 = (LTDC + 0x008) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_BPCR: *mut u32 = (LTDC + 0x00C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_AWCR: *mut u32 = (LTDC + 0x010) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_TWCR: *mut u32 = (LTDC + 0x014) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_GCR: *mut u32 = (LTDC + 0x018) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_SRCR: *mut u32 = (LTDC + 0x024) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_BCCR: *mut u32 = (LTDC + 0x02C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// LTDC Layer 1 (base = LTDC + 0x84)
const LTDC_L1: u32 = LTDC + 0x084;
const LTDC_L1CR: *mut u32 = (LTDC_L1 + 0x00) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1WHPCR: *mut u32 = (LTDC_L1 + 0x04) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1WVPCR: *mut u32 = (LTDC_L1 + 0x08) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1PFCR: *mut u32 = (LTDC_L1 + 0x10) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1CACR: *mut u32 = (LTDC_L1 + 0x14) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1BFCR: *mut u32 = (LTDC_L1 + 0x1C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1CFBAR: *mut u32 = (LTDC_L1 + 0x28) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1CFBLR: *mut u32 = (LTDC_L1 + 0x2C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const LTDC_L1CFBLNR: *mut u32 = (LTDC_L1 + 0x30) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// ── RCC ──────────────────────────────────────────────────────────────────────

const RCC: u32 = 0x5802_4400;
const RCC_CR: *mut u32 = (RCC + 0x000) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const RCC_AHB3ENR: *mut u32 = (RCC + 0x0D4) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const RCC_APB3ENR: *mut u32 = (RCC + 0x0E4) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// ── GPIO ─────────────────────────────────────────────────────────────────────

const GPIOG: u32 = 0x5802_1800;
const GPIOG_BSRR: *mut u32 = (GPIOG + 0x18) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

const GPIOJ: u32 = 0x5802_2400;
const GPIOJ_MODER: *mut u32 = (GPIOJ + 0x00) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
const GPIOJ_BSRR: *mut u32 = (GPIOJ + 0x18) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)

// ── Panel timing constants (NT35510 480×800 portrait, MB1166-A09) ────────────

/// NT35510 active panel width (portrait).
pub const PANEL_W: u16 = 480;
/// NT35510 active panel height (portrait).
pub const PANEL_H: u16 = 800;
/// HSYNC width (pixels).
pub const HSW: u16 = 2;
/// Horizontal back porch (pixels).
pub const HBP: u16 = 34;
/// Horizontal front porch (pixels).
pub const HFP: u16 = 34;
/// VSYNC width (lines).
pub const VSW: u16 = 120;
/// Vertical back porch (lines).
pub const VBP: u16 = 150;
/// Vertical front porch (lines).
pub const VFP: u16 = 150;

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline(always)]
fn delay_cycles(cycles: u32) {
    cortex_m::asm::delay(cycles);
}

/// Direct USART1 TX byte. Bypasses Zephyr's console subsystem so it
/// works even when called from contexts where printk isn't available.
fn u1_putc(c: u8) {
    const U1_ISR: *const u32 = (0x4001_1000 + 0x1C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    const U1_TDR: *mut u32 = (0x4001_1000 + 0x28) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    unsafe {
        // Wait for TXE (bit 7) — bounded poll
        let mut tries = 100_000u32;
        while U1_ISR.read_volatile() & (1 << 7) == 0 {
            tries -= 1;
            if tries == 0 {
                return;
            }
        }
        U1_TDR.write_volatile(c as u32);
    }
}

fn u1_str(s: &[u8]) {
    for &b in s {
        u1_putc(b);
    }
}

fn u1_hex(v: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for i in (0..8).rev() {
        let nibble = ((v >> (i * 4)) & 0xF) as usize;
        u1_putc(HEX[nibble]);
    }
}

/// Drive PG3 (panel reset) low, wait, then high.
unsafe fn pulse_panel_reset() {
    unsafe {
        // Configure PG3 as GP output: MODER bits 7:6 = 01
        const GPIOG_MODER: *mut u32 = GPIOG as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        let moder = GPIOG_MODER.read_volatile();
        GPIOG_MODER.write_volatile((moder & !(3u32 << 6)) | (1u32 << 6));
        // PG3 low
        GPIOG_BSRR.write_volatile(1u32 << 19);
        delay_cycles(4_000_000); // ~10 ms
        // PG3 high
        GPIOG_BSRR.write_volatile(1u32 << 3);
        delay_cycles(4_000_000); // ~10 ms recovery
    }
}

/// Drive PJ12 (backlight) high after configuring as GP output.
unsafe fn enable_backlight() {
    unsafe {
        let moder = GPIOJ_MODER.read_volatile();
        // PJ12 MODER bits 25:24 = 01 (GP output)
        GPIOJ_MODER.write_volatile((moder & !(3u32 << 24)) | (1u32 << 24));
        // PJ12 high
        GPIOJ_BSRR.write_volatile(1u32 << 12);
    }
}

// ── Step 2: DSI regulator ────────────────────────────────────────────────────

/// Step 2 (RM0399 §34.14.1): Enable DSI regulator and wait for ready (RRS).
pub unsafe fn init_dsi_regulator() -> bool {
    unsafe {
        // WRPCR.REGEN = bit 24
        let wrpcr = DSI_WRPCR.read_volatile();
        DSI_WRPCR.write_volatile(wrpcr | (1 << 24));
        // Wait for WISR.RRS = bit 12
        let mut tries = 1_000_000u32;
        while DSI_WISR.read_volatile() & (1 << 12) == 0 {
            tries -= 1;
            if tries == 0 {
                return false;
            }
            cortex_m::asm::nop();
        }
        true
    }
}

// ── Step 3: DSI wrapper PLL (IDF=5, NDIV=100, ODF=0) ─────────────────────────

/// Step 3 (RM0399 §34.14.1): Configure DSI PLL.
///
/// HSE=25 MHz / IDF=5 = 5 MHz, NDIV=100 → VCO=1000 MHz, ODF=0 → 500 Mbps/lane.
/// Lane byte clock = 500 / 8 = 62.5 MHz.
pub unsafe fn init_dsi_pll() -> bool {
    unsafe {
        // WRPCR layout (PAC):
        //   REGEN = bit 24, PLLEN = bit 0,
        //   IDF[15:11], NDIV[8:2], ODF[17:16] — but PAC field positions
        //   in stm32h7 0.15 use: idf=11, ndiv=2, odf=16, pllen=0, regen=24.
        //
        // Build the value matching the proven bare-metal init that uses
        // PAC `.idf().bits(5).ndiv().bits(100).odf().bits(0).pllen()
        //     .set_bit().regen().set_bit()`.
        let val: u32 = (1 << 24)        // REGEN
            | (1 << 0)                  // PLLEN
            | ((5u32 & 0x0F) << 11)     // IDF
            | ((100u32 & 0x7F) << 2)    // NDIV
            | ((0u32 & 0x03) << 16); // ODF
        DSI_WRPCR.write_volatile(val);
        // Wait for WISR.PLLLS = bit 8
        let mut tries = 1_000_000u32;
        while DSI_WISR.read_volatile() & (1 << 8) == 0 {
            tries -= 1;
            if tries == 0 {
                return false;
            }
            cortex_m::asm::nop();
        }
        true
    }
}

// ── Step 4: D-PHY init (HAL order) ───────────────────────────────────────────

/// Step 4 (RM0399 §34.14.2): Initialize D-PHY for 2 lanes.
pub unsafe fn init_dsi_phy() {
    unsafe {
        // 4a: Enable DSI host first (HAL does this before PHY config)
        DSI_CR.write_volatile(1); // CR.EN = 1
        // 4b: TX escape clock divider — CCR.TXECKDIV = 4
        DSI_CCR.write_volatile(4);
        // 4c: Enable D-PHY — PCTLR.DEN (bit 1), PCTLR.CKE (bit 2)
        let pctlr = DSI_PCTLR.read_volatile();
        DSI_PCTLR.write_volatile(pctlr | (1 << 1));
        let pctlr = DSI_PCTLR.read_volatile();
        DSI_PCTLR.write_volatile(pctlr | (1 << 2));
        // 4d: Number of lanes — PCONFR.NL = 1 (2 lanes), SW_TIME = 0x28
        // PAC: nl=bits 1:0, sw_time=bits 15:8
        DSI_PCONFR.write_volatile((1u32 & 0x3) | ((0x28u32 & 0xFF) << 8));
        // 4e: Wait for PHY lane stop state — PSR for 2 lanes:
        // PSS0(bit 1) + PSS1(bit 2) + PSSC(bit 3) = 0x0E
        let mut tries = 1_000_000u32;
        loop {
            let psr = DSI_PSR.read_volatile();
            if psr & 0x0E == 0x0E {
                break;
            }
            tries -= 1;
            if tries == 0 {
                break;
            }
            cortex_m::asm::nop();
        }
        // 4f: UIX4 in WPCR0 = 8
        let wpcr0 = DSI_WPCR0.read_volatile();
        DSI_WPCR0.write_volatile((wpcr0 & !0x3F) | 8);
        // 4g: Disable DSI host (HAL does this AFTER PHY init)
        DSI_CR.write_volatile(0);
        // 4h: CLCR = DPCC | ACR (auto clock lane control)
        DSI_CLCR.write_volatile(0x03); // DPCC=bit 0, ACR=bit 1
    }
}

// ── Step 5: Lane timings ─────────────────────────────────────────────────────

/// Step 5 (RM0399 §34.14.3): Lane timing config (while DSI disabled).
pub unsafe fn init_dsi_lane_timings() {
    unsafe {
        // CLTCR: HS2LP=50, LP2HS=50 (each in bits 25:16 and 9:0 fashion via PAC layout)
        DSI_CLTCR.write_volatile((50u32 << 16) | 50);
        // DLTCR: MRD_TIME[23:16]=15, HS2LP[31:24] etc — original writes
        //   (15 << 24) | (50 << 16) | 50
        DSI_DLTCR.write_volatile((15u32 << 24) | (50u32 << 16) | 50);
    }
}

// ── Step 6: Flow control ─────────────────────────────────────────────────────

/// Step 6 (RM0399 §34.14.4): Flow control — ETTXE + BTAE + ECCRXE.
pub unsafe fn init_dsi_flow_control() {
    unsafe {
        DSI_PCR.write_volatile(0x15); // ETTXE | BTAE | ECCRXE
    }
}

// ── Step 7: DSI Host LTDC interface ──────────────────────────────────────────

/// Step 7 (RM0399 §34.14.5): VCID=0, RGB888 color coding, default polarity.
pub unsafe fn init_dsi_ltdc_interface() {
    unsafe {
        DSI_LVCIDR.write_volatile(0); // VCID=0
        // LCOLCR: LPE (bit 8) | COLC=5 (RGB888)
        DSI_LCOLCR.write_volatile((1 << 8) | 5);
        // LPCR: polarity bits all 0
        DSI_LPCR.write_volatile(0);
    }
}

// ── Step 8: Video mode timings (needed even for adapted cmd) ─────────────────

/// Step 8 (RM0399 §34.14.6): Video mode timing registers — required even
/// in adapted command mode per ST HAL (sets DSI packet sizing).
pub unsafe fn init_dsi_video_timings(
    width: u16,
    height: u16,
    hsw: u16,
    hbp: u16,
    hfp: u16,
    vsw: u16,
    vbp: u16,
    vfp: u16,
) {
    unsafe {
        let lane_byte_clk: u32 = 62500; // kHz (DSI PLL: HSE/5 * 2 * 100 / 8)
        // Pixel clock from PLL3_R, set by Zephyr shield overlay to 27.5 MHz
        // (HSE/5 * 132 / 24). Bare-metal HAL configures 32 MHz; if you change
        // PLL3 in the DTS, update this constant.
        let pixel_clk: u32 = 27500; // kHz
        let total_pixels = (hsw as u32) + (hbp as u32) + (width as u32) + (hfp as u32);
        let hsa_dsi = (hsw as u32) * lane_byte_clk / pixel_clk;
        let hbp_dsi = (hbp as u32) * lane_byte_clk / pixel_clk;
        let hline_dsi = total_pixels * lane_byte_clk / pixel_clk;

        DSI_VHSACR.write_volatile(hsa_dsi & 0xFFF);
        DSI_VHBPCR.write_volatile(hbp_dsi & 0xFFF);
        DSI_VLCR.write_volatile(hline_dsi & 0x7FFF);
        DSI_VVSACR.write_volatile(vsw as u32 & 0x3FF);
        DSI_VVBPCR.write_volatile(vbp as u32 & 0x3FF);
        DSI_VVFPCR.write_volatile(vfp as u32 & 0x3FF);
        DSI_VVACR.write_volatile(height as u32 & 0x3FFF);
        DSI_VPCR.write_volatile(width as u32 & 0x3FFF);
        DSI_VCCR.write_volatile(0);
        DSI_VNPCR.write_volatile(0xFFF);
        // VMCR: video mode = non-burst with sync events, LP commands enabled
        // Bits: VMT[1:0]=0b10, LPVSAE+LPVBPE+LPVFPE+LPVAE+LPHBPE+LPHFPE,
        // FBTAAE — values from bare-metal proven init.
        DSI_VMCR.write_volatile(
            (0b10 << 0)
                | (1 << 8)
                | (1 << 9)
                | (1 << 10)
                | (1 << 11)
                | (1 << 12)
                | (1 << 13)
                | (1 << 15),
        );
        DSI_LPMCR.write_volatile((64u32 << 16) | 64);
    }
}

// ── Step "0": Enable DSI peripheral clock + reset ───────────────────────────

/// Enable DMA2D, LTDC, DSI peripheral clocks + GPIOG/GPIOJ clocks via RCC.
///
/// GPIOG (panel reset PG3) and GPIOJ (TE PJ2, backlight PJ12) clocks
/// must be enabled before we touch those pins. Zephyr only enables a
/// GPIO bank's clock if some driver references it; with our display
/// nodes disabled, those banks may not be clocked.
///
/// Safe to call even if Zephyr already enabled them.
pub unsafe fn enable_display_peripheral_clocks() {
    unsafe {
        // AHB3ENR.DMA2DEN = bit 4
        let ahb3 = RCC_AHB3ENR.read_volatile();
        RCC_AHB3ENR.write_volatile(ahb3 | (1 << 4));
        // APB3ENR.LTDCEN = bit 3, DSIEN = bit 4
        let apb3 = RCC_APB3ENR.read_volatile();
        RCC_APB3ENR.write_volatile(apb3 | (1 << 3) | (1 << 4));
        // AHB4ENR (0x5802_44E0): GPIOG = bit 6, GPIOJ = bit 9
        const RCC_AHB4ENR: *mut u32 = (RCC + 0x0E0) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        let ahb4 = RCC_AHB4ENR.read_volatile();
        RCC_AHB4ENR.write_volatile(ahb4 | (1 << 6) | (1 << 9));

        // ── KEEP CLOCKED IN CSLEEP ────────────────────────────────────
        // Per RM0399 §8.7.x, *xxxLPENR registers gate peripheral clocks
        // *while the CPU is in CSleep* (entered via WFI / Zephyr k_sleep).
        // If LTDC/DSI/DMA2D LPEN bits are 0 and Zephyr's idle drops into
        // CSleep, those peripherals lose their clocks mid-operation —
        // any scan or DMA2D transfer in flight halts, and a just-pulsed
        // WCR.LTDCEN can be lost before the wrapper acts on it.
        //
        // **H747 dual-core:** Per RM0399 / RM0433 §8.7.1 the per-CPU
        // C1 view (`RCC_C1_*LPENR` at offset 0x130+0x60 from D-domain)
        // is a SEPARATE physical register from the D-domain view, and
        // governs CM7's own CSleep gating. Empirically confirmed via
        // boot dump: D-domain LPENR shows our bits set, but C1_LPENR
        // reads 0 — meaning CM7 CSleep would gate everything if we
        // didn't also write the C1 view. (See feedback_h747_c1_lpenr.md.)
        // Note: ENR registers ARE aliased on H747 (C1_*ENR same as
        // D-domain), so we don't need to mirror those.
        const RCC_AHB3LPENR: *mut u32 = (RCC + 0x13C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_APB3LPENR: *mut u32 = (RCC + 0x14C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_AHB4LPENR: *mut u32 = (RCC + 0x148) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_AHB3LPENR: *mut u32 = (RCC + 0x19C) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_APB3LPENR: *mut u32 = (RCC + 0x1AC) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_AHB4LPENR: *mut u32 = (RCC + 0x1A8) as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        // DMA2DLPEN (bit 4) + FMCLPEN (bit 12) — LTDC's framebuffer
        // reads go through FMC to SDRAM; if FMC is gated in CSleep,
        // LTDC can't fetch pixels and the scan stalls.
        let ahb3lp_mask = (1 << 4) | (1 << 12);
        let apb3lp_mask = (1 << 3) | (1 << 4); // LTDC + DSI LPEN
        let ahb4lp_mask = (1 << 6) | (1 << 9); // GPIOG + GPIOJ LPEN
        let ahb3lp = RCC_AHB3LPENR.read_volatile();
        RCC_AHB3LPENR.write_volatile(ahb3lp | ahb3lp_mask);
        let apb3lp = RCC_APB3LPENR.read_volatile();
        RCC_APB3LPENR.write_volatile(apb3lp | apb3lp_mask);
        let ahb4lp = RCC_AHB4LPENR.read_volatile();
        RCC_AHB4LPENR.write_volatile(ahb4lp | ahb4lp_mask);
        // Mirror to CM7's per-CPU LPENR view.
        let c1_ahb3lp = RCC_C1_AHB3LPENR.read_volatile();
        RCC_C1_AHB3LPENR.write_volatile(c1_ahb3lp | ahb3lp_mask);
        let c1_apb3lp = RCC_C1_APB3LPENR.read_volatile();
        RCC_C1_APB3LPENR.write_volatile(c1_apb3lp | apb3lp_mask);
        let c1_ahb4lp = RCC_C1_AHB4LPENR.read_volatile();
        RCC_C1_AHB4LPENR.write_volatile(c1_ahb4lp | ahb4lp_mask);

        // Brief settle
        cortex_m::asm::dsb();
    }
}

/// Force PLL3 on if not already running, wait for lock.
pub unsafe fn ensure_pll3_running() {
    unsafe {
        let cr = RCC_CR.read_volatile();
        if cr & (1 << 29) == 0 {
            // PLL3RDY not set — turn PLL3 on
            RCC_CR.write_volatile(cr | (1 << 28)); // PLL3ON
            // Wait for PLL3RDY (bit 29)
            let mut tries = 1_000_000u32;
            while RCC_CR.read_volatile() & (1 << 29) == 0 {
                tries -= 1;
                if tries == 0 {
                    break;
                }
                cortex_m::asm::nop();
            }
        }
    }
}

// ── LTDC config ──────────────────────────────────────────────────────────────

/// Configure LTDC global timing (SSCR, BPCR, AWCR, TWCR, BCCR).
///
/// Uses raw writes — PAC LTDC struct has incorrect offsets due to missing
/// reserved padding (per existing notes in stm32h747i_disco.rs).
pub unsafe fn configure_ltdc_timing(
    width: u16,
    height: u16,
    hsw: u16,
    hbp: u16,
    hfp: u16,
    vsw: u16,
    vbp: u16,
    vfp: u16,
) {
    unsafe {
        let vsh = vsw;
        let hswm1 = (hsw.saturating_sub(1)) as u32;
        let vshm1 = (vsh.saturating_sub(1)) as u32;
        let ahbp = (hsw as u32 + hbp as u32).saturating_sub(1);
        let avbp = (vsh as u32 + vbp as u32).saturating_sub(1);
        let aaw = (hsw as u32 + hbp as u32 + width as u32).saturating_sub(1);
        let aah = (vsh as u32 + vbp as u32 + height as u32).saturating_sub(1);
        let totalw = (hsw as u32 + hbp as u32 + width as u32 + hfp as u32).saturating_sub(1);
        let totalh = (vsh as u32 + vbp as u32 + height as u32 + vfp as u32).saturating_sub(1);

        LTDC_SSCR.write_volatile((hswm1 << 16) | vshm1);
        LTDC_BPCR.write_volatile((ahbp << 16) | avbp);
        LTDC_AWCR.write_volatile((aaw << 16) | aah);
        LTDC_TWCR.write_volatile((totalw << 16) | totalh);
        LTDC_BCCR.write_volatile(0);
    }
}

/// Configure LTDC Layer 1 with framebuffer at `fb_addr`, ARGB8888 format.
pub unsafe fn setup_ltdc_layer(
    fb_addr: u32,
    width: u16,
    height: u16,
    hsw: u16,
    hbp: u16,
    vsw: u16,
    vbp: u16,
) {
    unsafe {
        let pitch = (width as u32) * 4; // ARGB8888
        let x0 = (hsw as u32) + (hbp as u32); // = BPCR.AHBP + 1
        let x1 = x0 + (width as u32) - 1;
        let y0 = (vsw as u32) + (vbp as u32); // = BPCR.AVBP + 1
        let y1 = y0 + (height as u32) - 1;

        LTDC_L1WHPCR.write_volatile((x1 << 16) | x0);
        LTDC_L1WVPCR.write_volatile((y1 << 16) | y0);
        LTDC_L1PFCR.write_volatile(0); // ARGB8888
        LTDC_L1CACR.write_volatile(255); // alpha
        LTDC_L1BFCR.write_volatile(0x0405); // blending: PAxCA / 1-PAxCA
        LTDC_L1CFBAR.write_volatile(fb_addr);
        // CFBLR: bits[28:16]=CFBP(pitch), bits[12:0]=CFBLL(line length + 3)
        LTDC_L1CFBLR.write_volatile((pitch << 16) | (pitch + 3));
        LTDC_L1CFBLNR.write_volatile(height as u32);
        // CR.LEN = bit 0
        let cr = LTDC_L1CR.read_volatile();
        LTDC_L1CR.write_volatile(cr | 1);
        // SRCR.IMR
        LTDC_SRCR.write_volatile(1);
    }
}

/// Enable LTDC global control (GCR.LTDCEN) with reset polarity values.
pub unsafe fn enable_ltdc() {
    unsafe {
        // GCR reset value 0x0000_2220 + LTDCEN(bit 0) = 0x0000_2221
        LTDC_GCR.write_volatile(0x0000_2221);
        LTDC_SRCR.write_volatile(1);
        cortex_m::asm::dsb();
    }
}

// ── NT35510 panel init via direct DCS (no PAC dep) ──────────────────────────

/// Wait for DSI command FIFO empty (GPSR.CMDFE = bit 0).
unsafe fn wait_cmd_fifo_empty() -> bool {
    unsafe {
        let mut tries = 1_000_000u32;
        while DSI_GPSR.read_volatile() & 1 == 0 {
            tries -= 1;
            if tries == 0 {
                return false;
            }
            cortex_m::asm::nop();
        }
        true
    }
}

/// DCS short write, no parameter (data type 0x05).
unsafe fn dcs_short_write(cmd: u8) -> bool {
    unsafe {
        if !wait_cmd_fifo_empty() {
            return false;
        }
        // GHCR: DT[5:0] | VCID[7:6] | WCLSB[15:8] | WCMSB[23:16]
        DSI_GHCR.write_volatile(0x05u32 | ((cmd as u32) << 8));
        true
    }
}

/// DCS short write, one parameter (data type 0x15).
unsafe fn dcs_short_write1(cmd: u8, param: u8) -> bool {
    unsafe {
        if !wait_cmd_fifo_empty() {
            return false;
        }
        DSI_GHCR.write_volatile(0x15u32 | ((cmd as u32) << 8) | ((param as u32) << 16));
        true
    }
}

/// Send the full NT35510 panel init sequence (MCS, gamma, voltages, etc.).
///
/// Delegates to `nt35510::Nt35510::init()` which uses PAC accessors to
/// write the ~100 init commands required for proper panel operation.
/// Steals the PAC DSIHOST peripheral — must not be in use by other code.
///
/// # Safety
///
/// DSI host clocks must be enabled and DSI must be in adapted command
/// mode with LP overrides active.
pub unsafe fn init_nt35510_panel() -> bool {
    unsafe {
        let p = stm32h7::stm32h747cm7::Peripherals::steal();
        let mut dsi = p.DSIHOST;
        crate::nt35510::Nt35510::init(&mut dsi)
    }
}

// ── High-level: full adapted command mode init ──────────────────────────────

/// Full DSI + LTDC bring-up in adapted command mode, NT35510 480×800 portrait.
///
/// Performs the entire sequence steps 2–14 from RM0399 §34.14.1, ending
/// with LTDC enabled and a single LTDCEN pulse triggering the first scan.
///
/// # Safety
///
/// - Peripheral clocks (DMA2D, LTDC, DSI) must be enabled — call
///   `enable_display_peripheral_clocks()` first if not done by the OS.
/// - PLL3R must be running at the pixel clock value used in
///   `init_dsi_video_timings` (currently 27.5 MHz, matching Zephyr's
///   `pll3` DTS config: HSE/5 * 132 / 24). Call `ensure_pll3_running()`
///   if not done by the OS.
/// - GPIOJ + GPIOG clocks enabled, PG3 reset pin available.
/// - `fb_addr` must point to a valid ARGB8888 framebuffer of size
///   PANEL_W * PANEL_H * 4 bytes in coherent SDRAM.
///
/// Returns `true` on success.
pub unsafe fn init_full_adapted_cmd(fb_addr: u32) -> bool {
    unsafe {
        u1_str(b"\r\nDSI[start]\r\n");
        // Step 2: regulator
        u1_str(b"DSI[2 reg ");
        let ok = init_dsi_regulator();
        u1_str(if ok { b"OK]\r\n" } else { b"FAIL]\r\n" });
        if !ok {
            return false;
        }
        // Step 3: DSI PLL
        u1_str(b"DSI[3 pll ");
        let ok = init_dsi_pll();
        u1_str(if ok { b"OK]\r\n" } else { b"FAIL]\r\n" });
        if !ok {
            return false;
        }
        // Step 4: D-PHY
        u1_str(b"DSI[4 phy]\r\n");
        init_dsi_phy();
        // Step 5: lane timings
        u1_str(b"DSI[5 lane]\r\n");
        init_dsi_lane_timings();
        // Step 6: flow control
        u1_str(b"DSI[6 flow]\r\n");
        init_dsi_flow_control();
        // Step 7: LTDC interface
        u1_str(b"DSI[7 ltdc-if]\r\n");
        init_dsi_ltdc_interface();
        // Step 8: video timings (still needed for adapted cmd)
        u1_str(b"DSI[8 vtim]\r\n");
        init_dsi_video_timings(PANEL_W, PANEL_H, HSW, HBP, HFP, VSW, VBP, VFP);
        // Step 9: adapted cmd mode config (PJ2 first to avoid floating TE)
        u1_str(b"DSI[9 acm]\r\n");
        dsi_cmd_mode::configure_te_gpio();
        dsi_cmd_mode::configure_adapted_cmd_mode(PANEL_W);
        // Step 10: HAL_DSI_Start — enable DSI host + wrapper
        u1_str(b"DSI[10 start]\r\n");
        dsi_cmd_mode::start_dsi();
        // Step 13: panel DCS init via LP. Reset is NOT pulsed here on
        // Zephyr or FreeRTOS builds — the early PG3 pulse in main.rs
        // (or Zephyr's SYS_INIT hook) already reset both the LCD panel
        // and FT5336. A second pulse corrupts the FT5336 touch sensor
        // (registers respond but TD_STATUS stays zero — sensor disabled).
        // On bare-metal builds, the early pulse provides sufficient reset
        // but we keep the second pulse for historical compatibility.
        u1_str(b"DSI[13 panel ");
        dsi_cmd_mode::enable_lp_cmd_overrides();
        #[cfg(not(feature = "zephyr"))]
        pulse_panel_reset();
        let panel_ok = init_nt35510_panel();
        dsi_cmd_mode::disable_lp_cmd_overrides();
        u1_str(if panel_ok { b"OK]\r\n" } else { b"FAIL]\r\n" });
        if !panel_ok {
            return false;
        }
        // Step 1+14: LTDC timing + layer + enable
        u1_str(b"DSI[14 ltdc-cfg]\r\n");
        configure_ltdc_timing(PANEL_W, PANEL_H, HSW, HBP, HFP, VSW, VBP, VFP);
        setup_ltdc_layer(fb_addr, PANEL_W, PANEL_H, HSW, HBP, VSW, VBP);
        enable_ltdc();
        // Backlight on
        u1_str(b"DSI[BL]\r\n");
        enable_backlight();
        // Brief settle, then first LTDCEN pulse to trigger first scan
        delay_cycles(8_000_000); // ~20ms
        DSI_WCR.write_volatile(0x0C); // DSIEN+LTDCEN pulse
        cortex_m::asm::dsb();
        delay_cycles(8_000_000); // let first frame complete
        u1_str(b"DSI[done]\r\n");
        true
    }
}

// Suppress dead_code warnings on items the bare-metal path doesn't (yet) use.
#[allow(dead_code)]
const _: () = {
    let _ = DSI_VR;
    let _ = DSI_GVCIDR;
    let _ = DSI_MCR;
    let _ = DSI_LCCR;
    let _ = DSI_TCCR0;
    let _ = DSI_WIER;
    let _ = DSI_WIFCR;
};

// ── Runtime register dump ────────────────────────────────────────────────────

/// Dump the live DSI host, DSI wrapper, LTDC, and PLL3 register values
/// over USART1 in a single block. Intended for diagnostics — mismatches
/// between expected and actual register values are usually the cause of
/// "ACM init reports OK but display is wrong" symptoms.
///
/// Each register is one line of the form `NAME=0xVVVVVVVV\r\n`.
///
/// # Safety
///
/// Reads are non-mutating but require DSI/LTDC peripheral clocks to be
/// enabled. Call after `enable_display_peripheral_clocks()` and (ideally)
/// after `init_full_adapted_cmd()` so the values reflect post-init state.
pub unsafe fn dump_registers() {
    unsafe fn line(name: &[u8], v: u32) {
        u1_str(name);
        u1_putc(b'=');
        u1_str(b"0x");
        u1_hex(v);
        u1_str(b"\r\n");
    }
    unsafe {
        u1_str(b"\r\n--- DSI/LTDC register dump ---\r\n");
        // RCC PLL3
        const RCC_PLLCKSELR: *const u32 = (RCC + 0x028) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_PLLCFGR: *const u32 = (RCC + 0x02C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        // Verified against stm32h7-0.15.1 PAC: PLL3DIVR=0x40, PLL3FRACR=0x44
        // (NOT 0x44/0x48 — _reserved8 padding shifts these earlier than RM0399's
        // listing initially suggests).
        const RCC_PLL3DIVR: *const u32 = (RCC + 0x040) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_PLL3FRACR: *const u32 = (RCC + 0x044) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        line(b"RCC.CR        ", RCC_CR.cast_const().read_volatile());
        line(b"RCC.PLLCKSELR ", RCC_PLLCKSELR.read_volatile());
        line(b"RCC.PLLCFGR   ", RCC_PLLCFGR.read_volatile());
        line(b"RCC.PLL3DIVR  ", RCC_PLL3DIVR.read_volatile());
        line(b"RCC.PLL3FRACR ", RCC_PLL3FRACR.read_volatile());
        const RCC_AHB3ENR_R: *const u32 = (RCC + 0x0D4) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_APB3ENR_R: *const u32 = (RCC + 0x0E4) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_AHB4ENR_R: *const u32 = (RCC + 0x0E0) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_D1CCIPR: *const u32 = (RCC + 0x04C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        line(b"RCC.AHB3ENR   ", RCC_AHB3ENR_R.read_volatile());
        line(b"RCC.AHB4ENR   ", RCC_AHB4ENR_R.read_volatile());
        line(b"RCC.APB3ENR   ", RCC_APB3ENR_R.read_volatile());
        line(b"RCC.D1CCIPR   ", RCC_D1CCIPR.read_volatile());
        const RCC_AHB3RSTR_R: *const u32 = (RCC + 0x07C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_APB3RSTR_R: *const u32 = (RCC + 0x08C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        line(b"RCC.AHB3RSTR  ", RCC_AHB3RSTR_R.read_volatile());
        line(b"RCC.APB3RSTR  ", RCC_APB3RSTR_R.read_volatile());
        // ── H747 dual-core CPU1 (CM7) view of ENR/LPENR ──────────────────────
        // Per RM0399 (and aliased on single-core RM0433/RM0468 §8.7.1), the
        // C1 view of these registers governs CM7's per-CPU bus access /
        // CSleep gating. If our writes hit the D-domain view but C1 view
        // shows the bits as 0, CM7 cannot access the peripheral. Boot dump
        // showing all LTDC.* = 0 (despite display working) is consistent
        // with C1_APB3ENR.LTDCEN being 0. See feedback_h747_c1_lpenr.md.
        const RCC_C1_AHB3ENR: *const u32 = (RCC + 0x134) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_AHB4ENR: *const u32 = (RCC + 0x140) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_APB3ENR: *const u32 = (RCC + 0x144) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_AHB3LPENR: *const u32 = (RCC + 0x19C) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_AHB4LPENR: *const u32 = (RCC + 0x1A8) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        const RCC_C1_APB3LPENR: *const u32 = (RCC + 0x1AC) as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        line(b"RCC.C1_AHB3EN ", RCC_C1_AHB3ENR.read_volatile());
        line(b"RCC.C1_AHB4EN ", RCC_C1_AHB4ENR.read_volatile());
        line(b"RCC.C1_APB3EN ", RCC_C1_APB3ENR.read_volatile());
        line(b"RCC.C1_AHB3LP ", RCC_C1_AHB3LPENR.read_volatile());
        line(b"RCC.C1_AHB4LP ", RCC_C1_AHB4LPENR.read_volatile());
        line(b"RCC.C1_APB3LP ", RCC_C1_APB3LPENR.read_volatile());
        // DSI host
        line(b"DSI.VR        ", DSI_VR.read_volatile());
        line(b"DSI.CR        ", DSI_CR.cast_const().read_volatile());
        line(b"DSI.CCR       ", DSI_CCR.cast_const().read_volatile());
        line(b"DSI.LVCIDR    ", DSI_LVCIDR.cast_const().read_volatile());
        line(b"DSI.LCOLCR    ", DSI_LCOLCR.cast_const().read_volatile());
        line(b"DSI.LPCR      ", DSI_LPCR.cast_const().read_volatile());
        line(b"DSI.LPMCR     ", DSI_LPMCR.cast_const().read_volatile());
        line(b"DSI.PCR       ", DSI_PCR.cast_const().read_volatile());
        line(b"DSI.MCR       ", DSI_MCR.cast_const().read_volatile());
        line(b"DSI.VMCR      ", DSI_VMCR.cast_const().read_volatile());
        line(b"DSI.VPCR      ", DSI_VPCR.cast_const().read_volatile());
        line(b"DSI.VHSACR    ", DSI_VHSACR.cast_const().read_volatile());
        line(b"DSI.VHBPCR    ", DSI_VHBPCR.cast_const().read_volatile());
        line(b"DSI.VLCR      ", DSI_VLCR.cast_const().read_volatile());
        line(b"DSI.VVSACR    ", DSI_VVSACR.cast_const().read_volatile());
        line(b"DSI.VVBPCR    ", DSI_VVBPCR.cast_const().read_volatile());
        line(b"DSI.VVFPCR    ", DSI_VVFPCR.cast_const().read_volatile());
        line(b"DSI.VVACR     ", DSI_VVACR.cast_const().read_volatile());
        line(b"DSI.LCCR      ", DSI_LCCR.cast_const().read_volatile());
        line(b"DSI.CMCR      ", DSI_CMCR.cast_const().read_volatile());
        line(b"DSI.GPSR      ", DSI_GPSR.read_volatile());
        line(b"DSI.PSR       ", DSI_PSR.read_volatile());
        line(b"DSI.PCONFR    ", DSI_PCONFR.cast_const().read_volatile());
        // DSI wrapper
        const DSI_WCFGR: *const u32 = DSI_W as *const u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        line(b"DSI.WCFGR     ", DSI_WCFGR.read_volatile());
        line(b"DSI.WCR       ", DSI_WCR.cast_const().read_volatile());
        line(b"DSI.WIER      ", DSI_WIER.cast_const().read_volatile());
        line(b"DSI.WISR      ", DSI_WISR.read_volatile());
        line(b"DSI.WPCR0     ", DSI_WPCR0.cast_const().read_volatile());
        line(b"DSI.WRPCR     ", DSI_WRPCR.cast_const().read_volatile());
        // LTDC
        line(b"LTDC.SSCR     ", LTDC_SSCR.cast_const().read_volatile());
        line(b"LTDC.BPCR     ", LTDC_BPCR.cast_const().read_volatile());
        line(b"LTDC.AWCR     ", LTDC_AWCR.cast_const().read_volatile());
        line(b"LTDC.TWCR     ", LTDC_TWCR.cast_const().read_volatile());
        line(b"LTDC.GCR      ", LTDC_GCR.cast_const().read_volatile());
        line(b"LTDC.BCCR     ", LTDC_BCCR.cast_const().read_volatile());
        // LTDC layer 1
        line(b"LTDC.L1CR     ", LTDC_L1CR.cast_const().read_volatile());
        line(b"LTDC.L1WHPCR  ", LTDC_L1WHPCR.cast_const().read_volatile());
        line(b"LTDC.L1WVPCR  ", LTDC_L1WVPCR.cast_const().read_volatile());
        line(b"LTDC.L1PFCR   ", LTDC_L1PFCR.cast_const().read_volatile());
        line(b"LTDC.L1CFBAR  ", LTDC_L1CFBAR.cast_const().read_volatile());
        line(b"LTDC.L1CFBLR  ", LTDC_L1CFBLR.cast_const().read_volatile());
        line(
            b"LTDC.L1CFBLNR ",
            LTDC_L1CFBLNR.cast_const().read_volatile(),
        );
        u1_str(b"--- end dump ---\r\n");
    }
}
