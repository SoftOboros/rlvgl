//! STM32H747I-DISCO display and touch drivers.
//!
//! Offers a minimal bring-up path for the discovery board's MIPI-DSI display
//! and touch peripherals. The display driver enables LTDC and DSI clocks,
//! issues a short initialization sequence to the OTM8009A panel, and configures
//! layer 1 for an RGB565 framebuffer. Touch input is provided via the FT5336
//! controller. Backlight PWM and panel reset control are wired through
//! `embedded-hal` traits.

#[cfg(feature = "stm32h747i_disco")]
use crate::ft5336::Ft5336;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use crate::nt35510::Nt35510;
use crate::{Blitter, DisplayDriver, InputDevice};
#[cfg(feature = "stm32h747i_disco")]
use embedded_hal::{digital::InputPin, i2c::I2c, i2c::SevenBitAddress};
#[cfg(feature = "stm32h747i_disco")]
use embedded_hal::{digital::OutputPin, pwm::SetDutyCycle};
use rlvgl_core::event::Event;
use rlvgl_core::widget::{Color, Rect};
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use stm32h7::stm32h747cm7::DMA2D;
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::{DSIHOST, LTDC};
// SCB no longer needed after MPU WT change; remove import
#[cfg(feature = "semihosting")]
use core::fmt::Write as _;
#[cfg(feature = "semihosting")]
use cortex_m_semihosting::hio;

// Simple SDRAM bump allocator for framebuffer and large blocks.
// Not thread-safe; intended for early boot allocations only.
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
mod sdram_alloc {
    use core::sync::atomic::{AtomicU32, Ordering};

    static BASE: AtomicU32 = AtomicU32::new(0);
    static END: AtomicU32 = AtomicU32::new(0);
    static CUR: AtomicU32 = AtomicU32::new(0);

    pub fn init(base: u32, size: u32) {
        BASE.store(base, Ordering::Relaxed);
        END.store(base.wrapping_add(size), Ordering::Relaxed);
        CUR.store(base, Ordering::Relaxed);
    }

    pub fn alloc(size: usize, align: usize) -> Option<u32> {
        let cur = CUR.load(Ordering::Relaxed);
        let align_m1 = (align as u32).wrapping_sub(1);
        let aligned = (cur.wrapping_add(align_m1)) & !align_m1;
        let new_cur = aligned.wrapping_add(size as u32);
        if new_cur > END.load(Ordering::Relaxed) {
            return None;
        }
        CUR.store(new_cur, Ordering::Relaxed);
        Some(aligned)
    }

    pub fn alloc_bytes(size: usize, align: usize) -> Option<*mut u8> {
        alloc(size, align).map(|addr| addr as *mut u8)
    }
}

/// Display driver for the STM32H747I-DISCO board.
///
/// Wraps a [`Blitter`] and configures LTDC/DSI clocks. The actual flush path is
/// still unimplemented and will eventually transfer pixel data over MIPI-DSI.
pub struct Stm32h747iDiscoDisplay<B: Blitter, BL = (), RST = ()> {
    _blitter: B,
    #[cfg(feature = "stm32h747i_disco")]
    backlight: BL,
    #[cfg(feature = "stm32h747i_disco")]
    reset: RST,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fb_addr: u32,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    width: u16,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    height: u16,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    _ltdc: LTDC,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    dsi: DSIHOST,
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    dma2d: Option<DMA2D>,
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    staging_pool: [StagingBuf; 3],
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fb_addr_back: u32,
}

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[derive(Copy, Clone)]
struct StagingBuf {
    ptr: *mut u8,
    cap: usize,
    in_use: bool,
}

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
impl StagingBuf {
    const EMPTY: StagingBuf = StagingBuf {
        ptr: core::ptr::null_mut(),
        cap: 0,
        in_use: false,
    };
}

impl<B: Blitter, BL, RST> Stm32h747iDiscoDisplay<B, BL, RST> {
    /// Reset DMA2D staging pool (mark all buffers free) for a new frame.
    pub fn reset_staging_pool(&mut self) {
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        {
            for buf in &mut self.staging_pool {
                buf.in_use = false;
            }
        }
    }
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn staging_get(&mut self, size: usize) -> Option<(usize, *mut u8)> {
        for (i, buf) in self.staging_pool.iter_mut().enumerate() {
            if !buf.in_use && buf.cap >= size && !buf.ptr.is_null() {
                buf.in_use = true;
                return Some((i, buf.ptr));
            }
        }
        for (i, buf) in self.staging_pool.iter_mut().enumerate() {
            if !buf.in_use {
                if let Some(p) = sdram_alloc::alloc_bytes(size, 32) {
                    buf.ptr = p;
                    buf.cap = size;
                    buf.in_use = true;
                    return Some((i, buf.ptr));
                } else {
                    return None;
                }
            }
        }
        None
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn staging_release(&mut self, index: usize) {
        if let Some(buf) = self.staging_pool.get_mut(index) {
            buf.in_use = false;
        }
    }
    /// Create a new display driver, enabling LTDC and DSI clocks and preparing
    /// the panel control pins.
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    pub fn new(
        blitter: B,
        backlight: BL,
        mut reset: RST,
        ltdc: LTDC,
        dsi: DSIHOST,
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        dma2d: DMA2D,
        #[cfg(feature = "splash")]
        splash: Option<&[u8]>,
    ) -> Self
    where
        BL: SetDutyCycle,
        RST: OutputPin,
    {
        // D3 SRAM telemetry: write breadcrumbs at 0x3800_0100+ so probe-rs
        // can read them to diagnose how far boot progresses.
        #[inline(always)]
        fn d3(slot: u32, val: u32) {
            unsafe {
                ((0x3800_0100u32 + slot * 4) as *mut u32).write_volatile(val);
            }
        }
        d3(0, 0xB007_0001); // entered new()

        // Clocks for LTDC/DSI/PLL3 are already enabled by C BSP.
        let _ = reset.set_low();
        let mut disp = Self {
            _blitter: blitter,
            backlight,
            reset,
            fb_addr: 0,
            fb_addr_back: 0,
            width: 0,
            height: 0,
            _ltdc: ltdc,
            dsi,
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            dma2d: Some(dma2d),
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            staging_pool: [StagingBuf::EMPTY; 3],
        };
        disp.set_backlight(0);

        // ── ST BSP reference values for NT35510 480×800 (portrait) ──────────
        // MB1166 Rev A-09: FRD400B25025-A-CTK panel with NT35510 controller
        // Ref: STM32CubeH7 nt35510.h, stm32h747i_discovery_lcd.c
        let width = 480u16;
        let height = 800u16;
        let hsw: u16 = 2;    // HSYNC width
        let hbp: u16 = 34;   // Horizontal back porch
        let hfp: u16 = 34;   // Horizontal front porch
        let vsw: u16 = 120;  // VSYNC width
        let vbp: u16 = 150;  // Vertical back porch
        let vfp: u16 = 150;  // Vertical front porch

        // ── Serial debug helper (UART8 + USART1 VCP dual output) ────────
        fn dbg(s: &str) {
            const U8_ISR: *const u32 = (0x4000_7C00 + 0x1C) as *const u32;
            const U8_TDR: *mut u32 = (0x4000_7C00 + 0x28) as *mut u32;
            const U1_ISR: *const u32 = (0x4001_1000 + 0x1C) as *const u32;
            const U1_TDR: *mut u32 = (0x4001_1000 + 0x28) as *mut u32;
            for b in s.bytes() {
                unsafe {
                    while U8_ISR.read_volatile() & (1 << 7) == 0 {}
                    U8_TDR.write_volatile(b as u32);
                    while U1_ISR.read_volatile() & (1 << 7) == 0 {}
                    U1_TDR.write_volatile(b as u32);
                }
            }
        }
        fn dbg_hex(val: u32) {
            const U8_ISR: *const u32 = (0x4000_7C00 + 0x1C) as *const u32;
            const U8_TDR: *mut u32 = (0x4000_7C00 + 0x28) as *mut u32;
            const U1_ISR: *const u32 = (0x4001_1000 + 0x1C) as *const u32;
            const U1_TDR: *mut u32 = (0x4001_1000 + 0x28) as *mut u32;
            const HEX: &[u8; 16] = b"0123456789abcdef";
            unsafe {
                for i in (0..8).rev() {
                    let nibble = ((val >> (i * 4)) & 0xF) as usize;
                    while U8_ISR.read_volatile() & (1 << 7) == 0 {}
                    U8_TDR.write_volatile(HEX[nibble] as u32);
                    while U1_ISR.read_volatile() & (1 << 7) == 0 {}
                    U1_TDR.write_volatile(HEX[nibble] as u32);
                }
            }
        }

        // PLL3 is now enabled in main.rs after HAL freeze().
        // No LTDC/DSI reset — HAL enable() provides a clean state.
        unsafe {
            let gcr = (0x5000_1018u32 as *const u32).read_volatile();
            dbg_hex(gcr);
        }
        dbg(" OK\r\n");

        // ── RM0399 §34.14: DSI programming procedure ───────────────────────

        // Step 1: LTDC timing
        dbg("  [1] pre-timing\r\n");
        disp.configure_ltdc_timing(width, height, hsw, hbp, hfp, vsw, vbp, vfp);
        dbg("  [2] post-timing\r\n");

        // Verify LTDC timing writes stuck (BEFORE GCR enable)
        // Store at 0x24070100 to avoid clobbering main diag block
        // Skip LTDC readback — reading LTDC regs hangs AXI bus.
        // Jump straight to DSI init.
        dbg("  [SKIP readback]\r\n");

        d3(10, 0xD510_0002); // post LTDC timing
        // Step 2: DSI regulator enable + wait ready
        // RM0399 §34.16.4 WISR: RRS=bit12, PLLLS=bit8 (PAC is correct)
        disp.dsi.wrpcr.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 24)) }); // REGEN
        {
            let mut tries = 1_000_000u32;
            while !disp.dsi.wisr.read().rrs().bit() {
                tries -= 1;
                if tries == 0 { break; }
                cortex_m::asm::nop();
            }
        }

        d3(10, 0xD510_0003); // post regulator
        // Step 3: DSI wrapper PLL (matches ST BSP: IDF=5, NDIV=100, ODF=0)
        //   HSE = 25 MHz, IDF=5 → ref=5 MHz
        //   NDIV=100 → VCO = 500 MHz → 500 Mbps/lane
        //   Lane byte clock = 500/8 = 62.5 MHz
        disp.dsi.wrpcr.write(|w| {
            w.regen().set_bit();
            w.pllen().set_bit();
            unsafe {
                w.idf().bits(5);
                w.ndiv().bits(100);
                w.odf().bits(0);
            }
            w
        });
        {
            let mut tries = 1_000_000u32;
            while !disp.dsi.wisr.read().pllls().bit() {
                tries -= 1;
                if tries == 0 { break; }
                cortex_m::asm::nop();
            }
            let wisr = unsafe { (0x5000_040Cu32 as *const u32).read_volatile() };
            dbg("  PLL: WISR="); dbg_hex(wisr);
            if tries == 0 { dbg(" TIMEOUT"); }
            dbg("\r\n");
        }


        // ══════════════════════════════════════════════════════════════════
        // DSI init — follows ST HAL sequence (HAL_DSI_Init → ConfigAdapted
        // CommandMode → HAL_DSI_Start) exactly to match proven working order.
        // ══════════════════════════════════════════════════════════════════
        const DSI: u32 = 0x5000_0000;

        // Step 4a: Enable DSI host FIRST (HAL does this before PHY config)
        disp.dsi.cr.write(|w| w.en().set_bit());

        // Step 4b: TX escape clock divider
        disp.dsi.ccr.write(|w| unsafe { w.bits(4) }); // TXECKDIV = 4

        // Step 4c: Enable D-PHY — DEN and CKE separately (HAL order)
        disp.dsi.pctlr.modify(|_, w| w.den().set_bit());
        disp.dsi.pctlr.modify(|_, w| w.cke().set_bit());

        // Step 4d: Configure number of lanes
        disp.dsi.pconfr.write(|w| unsafe {
            w.nl().bits(1)          // 2 lanes
             .sw_time().bits(0x28)
        });

        // Step 4e: Wait for PHY lane stop state (HAL waits for PSS0+PSS1+PSSC)
        // PSR at offset 0xB0 (CMSIS): PSS0=bit1, PSS1=bit2, PSSC=bit3...
        // Actually PAC PSR — let me use raw read
        {
            let psr_addr = (DSI + 0xB0) as *const u32;
            let mut tries = 1_000_000u32;
            // For 2 lanes: wait for PSS0 (bit 1) + PSS1 (bit 2) + PSSC (bit 3) = 0x0E
            loop {
                let psr = unsafe { psr_addr.read_volatile() };
                if psr & 0x0E == 0x0E { break; }
                tries -= 1;
                if tries == 0 {
                    dbg("  PSR timeout! PSR=");
                    dbg_hex(unsafe { psr_addr.read_volatile() });
                    dbg("\r\n");
                    break;
                }
                cortex_m::asm::nop();
            }
        }

        // Step 4f: UIX4 — set bit period in WPCR0
        // UIX4 = 4000000 * IDF * (1 << ODF) / (HSE_kHz * NDIV) = 8
        disp.dsi.wpcr0.modify(|r, w| unsafe {
            w.bits((r.bits() & !0x3F) | 8)
        });

        // Step 4g: Disable DSI host (HAL does this after PHY init!)
        disp.dsi.cr.write(|w| unsafe { w.bits(0) }); // CR.EN=0

        // Step 4h: Configure CLCR with DPCC + ACR (automatic clock lane control)
        // HAL does this WHILE DSI IS DISABLED
        disp.dsi.clcr.write(|w| unsafe {
            w.bits(0x03) // DPCC=1 (bit 0) + ACR=1 (bit 1)
        });
        d3(10, 0xD510_0005); // post PHY init
        dbg("  PHY init done (CLCR=0x03 ACR+DPCC)\r\n");

        // Step 5: Lane timings (while DSI disabled)
        disp.dsi.cltcr.write(|w| unsafe {
            w.bits((50 << 16) | 50)
        });
        disp.dsi.dltcr.write(|w| unsafe { w.bits((15 << 24) | (50 << 16) | 50) });

        // Step 6: Flow control
        disp.dsi.pcr.write(|w| unsafe { w.bits(0x15) }); // ETTXE + BTAE + ECCRXE

        // Step 7: DSI Host LTDC interface — VCID=0, RGB888
        disp.dsi.lvcidr.write(|w| unsafe { w.vcid().bits(0) });
        // Color coding: RGB888, loosely packed
        disp.dsi.lcolcr.write(|w| unsafe { w.bits((1 << 8) | 5) }); // LPE + COLC=5
        // Polarity: match LTDC GCR defaults
        disp.dsi.lpcr.write(|w| unsafe { w.bits(0x00) });

        // Step 8: Video mode timing (needed even for adapted cmd mode per HAL)
        let lane_byte_clk: u32 = 62500; // kHz
        let pixel_clk: u32 = 32000;     // kHz
        let total_pixels = (hsw as u32) + (hbp as u32) + (width as u32) + (hfp as u32);
        let hsa_dsi = (hsw as u32) * lane_byte_clk / pixel_clk;
        let hbp_dsi = (hbp as u32) * lane_byte_clk / pixel_clk;
        let hline_dsi = total_pixels * lane_byte_clk / pixel_clk;
        unsafe {
            disp.dsi.vhsacr.write(|w| w.hsa().bits(hsa_dsi as u16));
            disp.dsi.vhbpcr.write(|w| w.hbp().bits(hbp_dsi as u16));
            disp.dsi.vlcr.write(|w| w.hline().bits(hline_dsi as u16));
            disp.dsi.vvsacr.write(|w| w.vsa().bits(vsw));
            disp.dsi.vvbpcr.write(|w| w.vbp().bits(vbp));
            disp.dsi.vvfpcr.write(|w| w.vfp().bits(vfp));
            disp.dsi.vvacr.write(|w| w.va().bits(height));
        }
        disp.dsi.vpcr.write(|w| unsafe { w.vpsize().bits(width) });
        disp.dsi.vccr.write(|w| unsafe { w.bits(0) });
        disp.dsi.vnpcr.write(|w| unsafe { w.bits(0xFFF) });
        disp.dsi.vmcr.write(|w| unsafe {
            w.bits(
                (0b10 << 0) | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11)
                | (1 << 12) | (1 << 13) | (1 << 15)
            )
        });
        disp.dsi.lpmcr.write(|w| unsafe { w.bits((64 << 16) | 64) });

        // Step 9: Adapted command mode config (HAL_DSI_ConfigAdaptedCommandMode)
        // MCR.CMDM=1 (default)
        disp.dsi.lccr.write(|w| unsafe { w.bits(width as u32) });

        // Configure PJ2 as DSI_TE (AF13)
        unsafe {
            let gpioj: u32 = 0x5802_2400;
            let moder = (gpioj as *mut u32).read_volatile();
            (gpioj as *mut u32).write_volatile((moder & !(3u32 << 4)) | (2u32 << 4));
            let afrl = ((gpioj + 0x20) as *mut u32).read_volatile();
            ((gpioj + 0x20) as *mut u32).write_volatile((afrl & !(0xFu32 << 8)) | (13u32 << 8));
        }
        dbg("  PJ2 = DSI_TE (AF13)\r\n");

        // WCFGR: adapted command mode + external TE + auto refresh
        disp.dsi.wcfgr.write(|w| unsafe {
            w.bits(
                (1 << 0)    // DSIM = 1 (adapted command mode)
                | (5 << 1)  // COLMUX = 5 (RGB888)
                | (1 << 4)  // TESRC = 1 (external TE pin)
                | (1 << 6)  // AR = 1 (automatic refresh)
            )
        });
        // CMCR: TEARE=1 for TE handshake
        disp.dsi.cmcr.write(|w| unsafe { w.bits(1) }); // TEARE only
        // Enable TE + End-of-Refresh interrupts
        disp.dsi.wier.write(|w| unsafe { w.bits(0x03) }); // TEIE + ERIE

        // Step 10: HAL_DSI_Start — re-enable DSI host + wrapper
        disp.dsi.cr.write(|w| w.en().set_bit());
        disp.dsi.wcr.write(|w| w.dsien().set_bit());
        cortex_m::asm::delay(2_000_000);
        d3(10, 0xD510_0008); // post DSI host+wrapper enable
        dbg("  DSI host+wrapper enabled (HAL sequence)\r\n");

        // Step 13: Reset panel and send NT35510 init commands
        // Enable LP command transmission in CMCR for DCS writes
        // Use PAC bit positions: DSW0TX=16, DSW1TX=17, DLWTX=19, MRDPS=24
        disp.dsi.cmcr.write(|w| {
            w.dlwtx().set_bit()   // DCS long write in LP
             .dsw1tx().set_bit()  // DCS short write 1p in LP
             .dsw0tx().set_bit()  // DCS short write 0p in LP
             .glwtx().set_bit()   // Generic long write in LP
             .gsw2tx().set_bit()  // Generic short write 2p in LP
             .gsw1tx().set_bit()  // Generic short write 1p in LP
             .gsw0tx().set_bit()  // Generic short write 0p in LP
        });
        disp.reset_panel();
        cortex_m::asm::delay(4_000_000);
        let panel_ok = Nt35510::init(&mut disp.dsi);
        d3(10, 0xD510_000A); // post NT35510 init
        dbg("  NT35510 init: ");
        dbg(if panel_ok { "ok" } else { "FAIL" });
        let gpsr = unsafe { (0x5000_0074u32 as *const u32).read_volatile() };
        dbg(" GPSR="); dbg_hex(gpsr); dbg("\r\n");
        // Clear LP command overrides — adapted cmd mode takes over
        // Keep TEARE=1 (bit 0) for TE handshake in adapted command mode
        disp.dsi.cmcr.write(|w| unsafe { w.bits(1) }); // TEARE=1 only

        // ── SDRAM framebuffer allocation ────────────────────────────────────
        d3(1, 0xB007_0002); // pre-SDRAM init
        let fb = Self::init_sdram();
        d3(2, 0xB007_0003); // post-SDRAM init
        sdram_alloc::init(fb, 32 * 1024 * 1024);
        let fb_bytes = (width as usize) * (height as usize) * 4; // ARGB8888
        let fb_addr = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb);
        let fb_back = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb_addr);
        disp.fb_addr = fb_addr;
        disp.fb_addr_back = fb_back;
        disp.width = width;
        disp.height = height;
        Self::configure_mpu_sdram_writethrough(0xD000_0000, 32 * 1024 * 1024);
        d3(3, 0xB007_0004); // post-MPU SDRAM config
        // Quick SDRAM smoke test: write/read one word
        unsafe {
            let test_addr = (fb_addr as *mut u32).add(0x1000);
            test_addr.write_volatile(0xCAFE_BABE);
            cortex_m::asm::dsb();
            let rb = test_addr.read_volatile();
            d3(4, rb); // should be 0xCAFE_BABE if SDRAM works
        }

        // Fill BOTH framebuffers: splash if provided, otherwise solid black.
        #[cfg(feature = "splash")]
        let splash_ok = splash.and_then(|blob| {
            let (w, h, pal_bytes, stream) = rlvgl_decomp::parse_rle_blob(blob).ok()?;
            if (w as usize) * (h as usize) * 4 != fb_bytes { return None; }
            let pal_count = pal_bytes.len() / 2;
            let mut palette = [0u16; 192];
            for i in 0..pal_count {
                palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
            }

            // Decode into front buffer
            let fb0 = unsafe { core::slice::from_raw_parts_mut(fb_addr as *mut u8, fb_bytes) };
            rlvgl_decomp::decode_argb_into(
                w as usize, h as usize, &palette[..pal_count], stream, fb0,
            ).ok()?;
            // Decode into back buffer
            let fb1 = unsafe { core::slice::from_raw_parts_mut(fb_back as *mut u8, fb_bytes) };
            rlvgl_decomp::decode_argb_into(
                w as usize, h as usize, &palette[..pal_count], stream, fb1,
            ).ok()?;
            cortex_m::asm::dsb();
            Some(())
        }).is_some();
        #[cfg(not(feature = "splash"))]
        let splash_ok = false;
        if !splash_ok {
            // Fill both buffers with solid black
            unsafe {
                let total_pixels = (width as usize) * (height as usize);
                for i in 0..total_pixels {
                    (fb_addr as *mut u32).add(i).write_volatile(0xFF00_0000); // black
                    (fb_back as *mut u32).add(i).write_volatile(0xFF00_0000);
                }
                cortex_m::asm::dsb();
            }
        }
        // Ensure all framebuffer writes are globally visible to LTDC DMA
        // before configuring the layer to read from this address.
        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        d3(5, if splash_ok { 0x5F1A_0001 } else { 0x5F1A_0000 }); // splash result
        dbg("  FB filled: "); dbg_hex(fb_addr);
        dbg(if splash_ok { " (splash)" } else { " (solid)" });
        dbg(" ("); dbg_hex((width as u32) * (height as u32)); dbg(" px)\r\n");

        // Readback a few FB pixels for telemetry
        unsafe {
            d3(6, (fb_addr as *const u32).read_volatile());          // pixel[0]
            d3(7, (fb_addr as *const u32).add(256).read_volatile()); // pixel[256]
            d3(8, (fb_addr as *const u32).add(480).read_volatile()); // pixel[480] (row 1)
        }

        // Step 14: Setup LTDC layer with SDRAM framebuffer
        disp.setup_ltdc_layer(fb_addr, width, height);

        // Readback layer regs BEFORE GCR enable (no aliasing yet)
        unsafe {
            let l1cr   = (0x5000_1084u32 as *const u32).read_volatile();
            let cfbar  = (0x5000_10ACu32 as *const u32).read_volatile();
            let cfblr  = (0x5000_10B0u32 as *const u32).read_volatile();
            let cfblnr = (0x5000_10B4u32 as *const u32).read_volatile();
            let pfcr   = (0x5000_1094u32 as *const u32).read_volatile();
            dbg("  L1 pre-en: CR="); dbg_hex(l1cr);
            dbg(" CFBAR="); dbg_hex(cfbar);
            dbg(" CFBLR="); dbg_hex(cfblr);
            dbg(" CFBLNR="); dbg_hex(cfblnr);
            dbg(" PFCR="); dbg_hex(pfcr); dbg("\r\n");
            // Store at 0x24070120
            (0x2407_0120u32 as *mut u32).write_volatile(0xBEEF_0002);
            (0x2407_0124u32 as *mut u32).write_volatile(l1cr);
            (0x2407_0128u32 as *mut u32).write_volatile(cfbar);
            (0x2407_012Cu32 as *mut u32).write_volatile(cfblr);
            (0x2407_0130u32 as *mut u32).write_volatile(cfblnr);
            (0x2407_0134u32 as *mut u32).write_volatile(pfcr);
        }

        #[cfg(feature = "sdram_ramtest")]
        {
            let _ = sdram_alloc::alloc(fb_bytes, 64)
                .map(|addr| Self::fill_smpte_bars_rgb565(addr as *mut u16, width, height));
        }

        // Verify all layer registers before LTDCEN (no aliasing yet)
        unsafe {
            let l1_cr    = (0x5000_1084u32 as *const u32).read_volatile();
            let l1_whpcr = (0x5000_1088u32 as *const u32).read_volatile();
            let l1_wvpcr = (0x5000_108Cu32 as *const u32).read_volatile();
            let l1_pfcr  = (0x5000_1094u32 as *const u32).read_volatile();
            let l1_cacr  = (0x5000_1098u32 as *const u32).read_volatile();
            let l1_bfcr  = (0x5000_10A0u32 as *const u32).read_volatile();
            let l1_cfbar = (0x5000_10ACu32 as *const u32).read_volatile();
            let l1_cfblr = (0x5000_10B0u32 as *const u32).read_volatile();
            let l1_cfblnr= (0x5000_10B4u32 as *const u32).read_volatile();
            let sscr     = (0x5000_1008u32 as *const u32).read_volatile();
            let bpcr     = (0x5000_100Cu32 as *const u32).read_volatile();
            let awcr     = (0x5000_1010u32 as *const u32).read_volatile();
            let twcr     = (0x5000_1014u32 as *const u32).read_volatile();
            let gcr      = (0x5000_1018u32 as *const u32).read_volatile();
            // Store comprehensive pre-LTDCEN dump at 0x24070140
            let base_dump: u32 = 0x2407_0140;
            let dump_vals: [u32; 16] = [
                0xBEEF_0003, // sentinel
                l1_cr, l1_whpcr, l1_wvpcr, l1_pfcr, l1_cacr,
                l1_bfcr, l1_cfbar, l1_cfblr, l1_cfblnr,
                sscr, bpcr, awcr, twcr, gcr,
                0xBEEF_EEEE, // end
            ];
            for (i, &v) in dump_vals.iter().enumerate() {
                ((base_dump + i as u32 * 4) as *mut u32).write_volatile(v);
            }
            dbg("  pre-LTDCEN: L1CR="); dbg_hex(l1_cr);
            dbg(" CFBAR="); dbg_hex(l1_cfbar);
            dbg(" CFBLR="); dbg_hex(l1_cfblr);
            dbg(" GCR="); dbg_hex(gcr); dbg("\r\n");
        }

        // Step 15: Enable LTDC, then start DSI wrapper bridge.
        // Use write (not modify) to ensure GCR polarity bits are all 0
        // (HSPOL=0, VSPOL=0, DEPOL=0, PCPOL=0) matching LPCR=0x00.
        // Raw write — PAC gcr.write() starts from 0 and may not set LTDCEN correctly
        unsafe { (0x5000_1018 as *mut u32).write_volatile(0x0000_2221) }; // GCR reset val + LTDCEN
        unsafe { (0x5000_1024 as *mut u32).write_volatile(1) }; // SRCR.IMR
        cortex_m::asm::dsb();
        // RM0399 §34.5: DSI host waits for the first VSYNC active
        // transition to start video mode.  Let LTDC run for at least
        // one full frame (~14ms at 32 MHz pixel clock, 840×512 total)
        // so it is generating periodic VSYNC before we bridge.
        let gcr = unsafe { (0x5000_1018u32 as *const u32).read_volatile() };
        let l1cr = unsafe { (0x5000_1084u32 as *const u32).read_volatile() };
        let cfbar = unsafe { (0x5000_10ACu32 as *const u32).read_volatile() };
        dbg("  LTDC: GCR="); dbg_hex(gcr);
        dbg(" L1CR="); dbg_hex(l1cr);
        dbg(" CFBAR="); dbg_hex(cfbar);
        dbg(" fb="); dbg_hex(fb_addr); dbg("\r\n");

        // Extended layer register readback
        unsafe {
            let whpcr = (0x5000_1088u32 as *const u32).read_volatile();
            let wvpcr = (0x5000_108Cu32 as *const u32).read_volatile();
            let pfcr  = (0x5000_1094u32 as *const u32).read_volatile();
            let cacr  = (0x5000_1098u32 as *const u32).read_volatile();
            let cfblr = (0x5000_10B0u32 as *const u32).read_volatile();
            let cfblnr= (0x5000_10B4u32 as *const u32).read_volatile();
            dbg("  L1: WHPCR="); dbg_hex(whpcr);
            dbg(" WVPCR="); dbg_hex(wvpcr);
            dbg(" PFCR="); dbg_hex(pfcr);
            dbg("\r\n");
            dbg("  L1: CACR="); dbg_hex(cacr);
            dbg(" CFBLR="); dbg_hex(cfblr);
            dbg(" CFBLNR="); dbg_hex(cfblnr);
            dbg("\r\n");

            // Framebuffer content: first 4 pixels (ARGB8888)
            dbg("  FB[0..3]: ");
            for i in 0..4u32 {
                let px = ((fb_addr + i * 4) as *const u32).read_volatile();
                dbg_hex(px); dbg(" ");
            }
            dbg("\r\n");

            // DSI state
            let wcfgr = (0x5000_0400u32 as *const u32).read_volatile();  // wrapper WCFGR
            let wcr   = (0x5000_0404u32 as *const u32).read_volatile();  // wrapper WCR
            let cmcr  = (0x5000_0068u32 as *const u32).read_volatile();  // host CMCR
            let mcr   = (0x5000_0004u32 as *const u32).read_volatile();  // host CR (EN)
            dbg("  DSI: WCFGR="); dbg_hex(wcfgr);
            dbg(" WCR="); dbg_hex(wcr);
            dbg(" CMCR="); dbg_hex(cmcr);
            dbg(" CR="); dbg_hex(mcr);
            dbg("\r\n");

            // LTDC timing readback
            let sscr = (0x5000_1008u32 as *const u32).read_volatile();
            let bpcr = (0x5000_100Cu32 as *const u32).read_volatile();
            let awcr = (0x5000_1010u32 as *const u32).read_volatile();
            let twcr = (0x5000_1014u32 as *const u32).read_volatile();
            dbg("  TIM: SSCR="); dbg_hex(sscr);
            dbg(" BPCR="); dbg_hex(bpcr);
            dbg("\r\n");
            dbg("  TIM: AWCR="); dbg_hex(awcr);
            dbg(" TWCR="); dbg_hex(twcr);
            dbg("\r\n");

            // ── SRAM diagnostic dump at 0x24070000 (probe-rs readable) ──
            // Layout: [sentinel, GCR, L1CR, CFBAR, fb_addr,
            //          WHPCR, WVPCR, PFCR, CACR, CFBLR, CFBLNR,
            //          WCFGR, WCR, CMCR, CR,
            //          SSCR, BPCR, AWCR, TWCR,
            //          FB[0], FB[1], FB[2], FB[3],
            //          WISR, VMCCR, PHYSTS, end_sentinel]
            let base: u32 = 0x2407_0000;
            let vals: [u32; 27] = [
                0xD1A6_0000, // sentinel: "DIAG"
                gcr, l1cr, cfbar, fb_addr,
                whpcr, wvpcr, pfcr, cacr, cfblr, cfblnr,
                wcfgr, wcr, cmcr, mcr,
                sscr, bpcr, awcr, twcr,
                ((fb_addr) as *const u32).read_volatile(),
                ((fb_addr + 4) as *const u32).read_volatile(),
                ((fb_addr + 8) as *const u32).read_volatile(),
                ((fb_addr + 12) as *const u32).read_volatile(),
                0, 0, 0, // placeholders for post-LTDCEN WISR/VMCCR/PHYSTS
                0xD1A6_FFFF, // end sentinel
            ];
            for (i, &v) in vals.iter().enumerate() {
                ((base + i as u32 * 4) as *mut u32).write_volatile(v);
            }
        }
        cortex_m::asm::delay(8_000_000); // ~20ms at 400 MHz

        // WCR LTDCEN in adapted command mode is a PULSE — it triggers one
        // frame transfer then auto-clears. The present() re-pulses each frame.
        // In the ST HAL, LTDCEN is indeed toggled per-refresh in adapted mode.
        disp.dsi.wcr.modify(|_, w| w.ltdcen().set_bit());
        cortex_m::asm::dsb();
        cortex_m::asm::delay(8_000_000); // wait another frame
        // Read WISR from wrapper (safe — wrapper regs don't hang debug port)
        let wisr = unsafe { (0x5000_040Cu32 as *const u32).read_volatile() };
        dbg("  post-LTDCEN: WISR="); dbg_hex(wisr); dbg("\r\n");
        unsafe {
            (0x2407_0060u32 as *mut u32).write_volatile(wisr);
            (0x2407_0064u32 as *mut u32).write_volatile(0xAA55_AA55); // done marker
        }

        // Backlight on
        #[allow(clippy::arithmetic_side_effects)]
        {
            let max = disp.backlight.max_duty_cycle();
            let target = max / 2;
            let step = core::cmp::max(1, target / 20);
            let mut lvl = 0u16;
            while lvl < target {
                let _ = disp.backlight.set_duty_cycle(lvl);
                cortex_m::asm::delay(5_000_00);
                lvl = lvl.saturating_add(step);
            }
            let _ = disp.backlight.set_duty_cycle(target);
        }
        disp
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn configure_ltdc_timing(
        &mut self,
        width: u16,
        height: u16,
        hsw: u16,
        hbp: u16,
        hfp: u16,
        vsw: u16,
        vbp: u16,
        vfp: u16,
    ) {
        let vsh = vsw;
        let hswm1 = (hsw.saturating_sub(1)) as u32;
        let vshm1 = (vsh.saturating_sub(1)) as u32;
        let ahbp = (hsw as u32 + hbp as u32).saturating_sub(1);
        let avbp = (vsh as u32 + vbp as u32).saturating_sub(1);
        let aaw = (hsw as u32 + hbp as u32 + width as u32).saturating_sub(1);
        let aah = (vsh as u32 + vbp as u32 + height as u32).saturating_sub(1);
        let totalw = (hsw as u32 + hbp as u32 + width as u32 + hfp as u32).saturating_sub(1);
        let totalh = (vsh as u32 + vbp as u32 + height as u32 + vfp as u32).saturating_sub(1);

        // RM0399 §33.7.1–4: bits 27:16 = horizontal, bits 10:0 = vertical
        // Raw writes — PAC LTDC struct has wrong offsets (missing initial padding)
        const LTDC: u32 = 0x5000_1000;
        unsafe {
            ((LTDC + 0x08) as *mut u32).write_volatile((hswm1 << 16) | vshm1);   // SSCR
            ((LTDC + 0x0C) as *mut u32).write_volatile((ahbp << 16) | avbp);     // BPCR
            ((LTDC + 0x10) as *mut u32).write_volatile((aaw << 16) | aah);       // AWCR
            ((LTDC + 0x14) as *mut u32).write_volatile((totalw << 16) | totalh); // TWCR
            ((LTDC + 0x2C) as *mut u32).write_volatile(0);                        // BCCR
        }
    }
    #[cfg(feature = "stm32h747i_disco")]
    fn set_backlight(&mut self, level: u16)
    where
        BL: SetDutyCycle,
    {
        let _ = self.backlight.set_duty_cycle(level);
    }

    #[cfg(feature = "stm32h747i_disco")]
    fn reset_panel(&mut self)
    where
        RST: OutputPin,
    {
        let _ = self.reset.set_low();
        cortex_m::asm::delay(4_000_000); // ~10ms at 400 MHz
        let _ = self.reset.set_high();
        cortex_m::asm::delay(4_000_000); // ~10ms recovery
    }
    /// Public helper to adjust backlight duty cycle.
    pub fn set_brightness(&mut self, level: u16)
    where
        BL: SetDutyCycle,
    {
        let _ = self.backlight.set_duty_cycle(level);
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn setup_ltdc_layer(&mut self, fb: u32, width: u16, height: u16) {
        let pitch = (width as u32) * 4; // ARGB8888 bytes/line
        // Must match timing values from new() — NT35510 480×800 portrait
        let hsw: u32 = 2;
        let hbp: u32 = 34;
        let vsw: u32 = 120;
        let vbp: u32 = 150;
        let x0 = hsw + hbp + 1;
        let x1 = x0 + (width as u32) - 1;
        let y0 = vsw + vbp + 1;
        let y1 = y0 + (height as u32) - 1;
        // ALL raw writes — PAC LAYER struct has wrong offsets for CFBAR+
        // due to missing reserved padding (RM0399 §33.7).
        // Layer 1 base = LTDC(0x50001000) + 0x84
        const L1: u32 = 0x5000_1084;
        unsafe {
            ((L1 + 0x04) as *mut u32).write_volatile((x1 << 16) | x0);       // WHPCR
            ((L1 + 0x08) as *mut u32).write_volatile((y1 << 16) | y0);       // WVPCR
            ((L1 + 0x10) as *mut u32).write_volatile(0);                      // PFCR = ARGB8888
            ((L1 + 0x14) as *mut u32).write_volatile(255);                    // CACR
            ((L1 + 0x1C) as *mut u32).write_volatile(0x0405);                 // BFCR
            ((L1 + 0x28) as *mut u32).write_volatile(fb);                     // CFBAR
            // CFBLR: bits[28:16]=CFBP (pitch), bits[12:0]=CFBLL (line_len + 7)
            ((L1 + 0x2C) as *mut u32).write_volatile((pitch << 16) | (pitch + 7)); // CFBLR
            ((L1 + 0x30) as *mut u32).write_volatile(height as u32);          // CFBLNR
            // Enable layer (CR bit 0 = LEN)
            let cr = ((L1) as *const u32).read_volatile();
            ((L1) as *mut u32).write_volatile(cr | 1);                        // CR.LEN
        }
        unsafe { (0x5000_1024 as *mut u32).write_volatile(1) } // SRCR.IMR;
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    #[cfg(feature = "experimental_dsi_host")]
    fn configure_dsi_video_mode(&mut self, _width: u16, _height: u16) {
        // Minimal configuration: set virtual channel and color coding to RGB565.
        // Full video mode timing/setup TBD.
        unsafe {
            // Virtual Channel ID = 0
            self.dsi.lvcidr.write(|w| w.vcid().bits(0));
            // Color coding: RGB565 (COLC = 0x5)
            self.dsi.lcolcr.write(|w| w.colc().bits(0x5));
            // Low-power: disable for HS video path by default
            // self.dsi.lpmcr.write(|w| w.bits(0));
            // Program basic video timings (units TBD; placeholder maps pixel counts)
            let hsw: u32 = 20;
            let hbp: u32 = 140;
            let hfp: u32 = 20;
            let vsw: u32 = 4;
            let vbp: u32 = 34;
            let vfp: u32 = 10;
            let vact: u32 = _height as u32;
            let hact: u32 = _width as u32;
            let hline: u32 = hsw + hbp + hact + hfp;
            // Vertical
            self.dsi.vvsacr.write(|w| w.vsa().bits(vsw as u16));
            self.dsi.vvbpcr.write(|w| w.vbp().bits(vbp as u16));
            self.dsi.vvfpcr.write(|w| w.vfp().bits(vfp as u16));
            self.dsi.vvacr.write(|w| w.va().bits(vact as u16));
            // Horizontal
            self.dsi.vhsacr.write(|w| w.hsa().bits(hsw as u16));
            self.dsi.vhbpcr.write(|w| w.hbp().bits(hbp as u16));
            self.dsi.vlcr.write(|w| w.hline().bits(hline as u16));
        }
        // Attempt DSI host/wrapper enable sequence (scaffold; PLL values TBD)
        let _ = self.enable_dsi_host_2lane_60hz();
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    #[cfg(not(feature = "experimental_dsi_host"))]
    #[allow(dead_code)] // stub pending experimental_dsi_host feature
    fn configure_dsi_video_mode(&mut self, _width: u16, _height: u16) {
        // DSI enable sequence is experimental and disabled by default to avoid wedging
        // the bus during bring-up.
        let _ = (_width, _height);
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    #[cfg(feature = "experimental_dsi_host")]
    fn enable_dsi_host_2lane_60hz(&mut self) -> bool {
        // NOTE: This is a scaffold for the host enable sequence. Exact PLL and
        // wrapper configuration values depend on reference clock and desired
        // lane byte clock. We keep this guarded and minimal to avoid illegal
        // state transitions until tuned.
        unsafe {
            let dsi = &self.dsi;
            // Target: 2 data lanes, ~60 Hz for 800x480 RGB565
            // Throughput estimate:
            //   HLINE = HSA+HBP+HACT+HFP = 20+140+800+20 = 980
            //   VLINE = VSA+VBP+VACT+VFP = 4+34+480+10 = 528
            //   Bits/frame ≈ HACT*VACT*16 = 800*480*16 = 6.144 Mbits (active)
            //   With porch/sync overhead, provision ~250 Mbps per lane (2 lanes => ~500 Mbps total)
            // Choose lane byte clock ≈ 31.25 MHz (bit clock 250 Mbps).
            // DSI Wrapper PLL (assume HSE=25 MHz): VCO = (HSE/IDF)*NDIV; ByteClk = VCO / 2^ODF
            // Pick IDF=5, NDIV=50 => VCO=250 MHz; ODF=3 (/8) => ByteClk=31.25 MHz → 250 Mbps per lane.
            const WRPCR_REGEN: u32 = 1 << 24; // Regulator enable
            const WRPCR_PLLEN: u32 = 1 << 0; // PLL enable
            // IDF/NDIV/ODF positions TBD; conservative defaults below
            const WRPCR_IDF_POS: u32 = 8;
            const WRPCR_NDIV_POS: u32 = 16;
            const WRPCR_ODF_POS: u32 = 2;

            const WCFGR_DSIM: u32 = 0; // DPI input mode (vs. command)
            const WCFGR_COLMUX_RGB565: u32 = 0x5 << 1; // Color coding bits

            const WCR_DSIEN: u32 = 1 << 3; // Wrapper enable

            const PCTLR_CKE: u32 = 1 << 2; // Clock lane enable
            const PCTLR_DEN: u32 = 1 << 1; // Data lanes enable

            const CLCR_DPCC: u32 = 1 << 1; // Data lanes in HS
            const CLCR_DPCC_CLK: u32 = 1 << 0; // Clock lane in HS

            const VMCR_VMT_NB_SYNC: u32 = 0b01 << 0; // Non-burst with sync pulses
            const VMCR_VMEN: u32 = 1 << 1; // Video mode enable

            // 1) Configure PLL (placeholder divisors for bring-up)
            let idf = 5u32; // input division factor (HSE/5 = 5 MHz)
            let ndiv = 50u32; // multiplication factor (→ 250 MHz VCO)
            let odf = 3u32; // output division factor (/8 → 31.25 MHz byte clk)
            let wrpcr = WRPCR_REGEN
                | ((idf & 0x0F) << WRPCR_IDF_POS)
                | ((ndiv & 0x7F) << WRPCR_NDIV_POS)
                | ((odf & 0x03) << WRPCR_ODF_POS)
                | WRPCR_PLLEN;
            // Write WRPCR and wait a short time (no explicit ready bit used here)
            dsi.wrpcr.write(|w| w.bits(wrpcr));
            // Clear prior interrupts (PLL lock/unlock, TE, EoR, regulator ready)
            dsi.wifcr.write(|w| {
                w.cplllif()
                    .set_bit()
                    .cplluif()
                    .set_bit()
                    .cteif()
                    .set_bit()
                    .cerif()
                    .set_bit()
                    .crrif()
                    .set_bit()
            });
            // Wait for PLL lock
            if !Self::wait_pll_lock(dsi, 500, 10_000) {
                Self::log_timeout("DSI: PLL lock timeout");
                return false;
            }

            // 2) Wrapper config: DPI + RGB565
            dsi.wcfgr
                .write(|w| w.bits(WCFGR_DSIM | WCFGR_COLMUX_RGB565));

            // 3) Enable wrapper and host; exit ULPS
            dsi.wcr.write(|w| w.bits(WCR_DSIEN));
            dsi.pctlr.write(|w| w.bits(PCTLR_CKE | PCTLR_DEN));
            // Wait for lanes to be active (clock + data lanes leave stop-state)
            if !Self::wait_lanes_ready(dsi, 500, 10_000) {
                Self::log_timeout("DSI: lanes ready timeout");
                return false;
            }

            // 4) Enable lanes in HS for video
            dsi.clcr.write(|w| w.bits(CLCR_DPCC | CLCR_DPCC_CLK));

            // 5) Video mode config: non-burst with sync pulses + enable
            dsi.vmcr.write(|w| w.bits(VMCR_VMT_NB_SYNC | VMCR_VMEN));
            // Optional: small delay after enabling video mode
            cortex_m::asm::delay(50_000);

            // 6) Small delay to let lanes settle
            cortex_m::asm::delay(100_000);
        }
        true
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    #[cfg(feature = "experimental_dsi_host")]
    fn wait_pll_lock(dsi: &DSIHOST, tries: u32, delay_cycles: u32) -> bool {
        let mut n = tries;
        while n > 0 {
            if dsi.wisr.read().pllls().bit() {
                return true;
            }
            cortex_m::asm::delay(delay_cycles);
            n -= 1;
        }
        false
    }

    #[cfg(feature = "experimental_dsi_host")]
    fn wait_lanes_ready(dsi: &DSIHOST, tries: u32, delay_cycles: u32) -> bool {
        let mut n = tries;
        while n > 0 {
            let psr = dsi.psr.read();
            let clk_active = !psr.pssc().bit();
            let d0_active = !psr.pss0().bit();
            // Assume 2 lanes; if single-lane, d1_active may remain true (stopped)
            let d1_active = !psr.pss1().bit();
            if clk_active && d0_active && d1_active {
                return true;
            }
            cortex_m::asm::delay(delay_cycles);
            n -= 1;
        }
        false
    }

    #[inline]
    #[cfg(feature = "experimental_dsi_host")]
    fn log_timeout(msg: &str) {
        #[cfg(feature = "semihosting")]
        {
            if let Ok(mut out) = hio::hstdout() {
                let _ = writeln!(out, "{}", msg);
            }
        }
        #[cfg(not(feature = "semihosting"))]
        {
            let _ = msg; // no-op
        }
    }

    /// Configure MPU region for SDRAM framebuffer as write-through, non-executable.
    fn configure_mpu_sdram_writethrough(base: u32, size_bytes: u32) {
        // Region size must be power-of-two and base must be aligned to size.
        // For H747I-DISCO SDRAM: base=0xD000_0000, size=32 MiB.
        unsafe {
            let p = cortex_m::Peripherals::steal();
            // Disable MPU
            p.MPU.ctrl.write(0);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();

            // Compute RASR SIZE field: size = 2^(SIZE+1)
            let mut sz = 0u32;
            let mut n = size_bytes;
            while n > 1 {
                n >>= 1;
                sz += 1;
            }
            // SIZE encodes as (log2(size) - 1)
            if sz > 0 {
                sz -= 1;
            }

            // Select region number 6 (arbitrary, avoid typical HAL regions)
            p.MPU.rnr.write(6);
            // Set base address
            p.MPU.rbar.write(base);
            // RASR attributes:
            // XN=1 (execute never), AP=0b011 (full access)
            // TEX=0, C=0, B=0, S=1 → Strongly-ordered / Device (no caching)
            // This ensures LTDC AXI reads see the same data CM7 writes.
            let xn = 1u32 << 28;
            let ap = 0b011u32 << 24;
            let tex_cb_s = (0u32 << 19) | (0u32 << 17) | (0u32 << 16); // TEX=0,C=0,B=0
            let s = 1u32 << 18; // Shareable
            let size_field = (sz & 0x1F) << 1;
            let enable = 1u32;
            p.MPU
                .rasr
                .write(xn | ap | s | tex_cb_s | size_field | enable);

            // Enable MPU with default memory map for privileged access
            // PRIVDEFENA=1 (bit 2), ENABLE=1 (bit 0)
            p.MPU.ctrl.write(0x5);
            cortex_m::asm::dsb();
            cortex_m::asm::isb();
        }
    }

    /// Return the address of the current back (off-screen) buffer.
    pub fn back_buffer_addr(&self) -> u32 {
        self.fb_addr_back
    }

    /// Return the display dimensions as (width, height) in pixels.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    /// Swap LTDC layer address between front/back buffers and reload
    pub fn present(&mut self) {
        let next = self.fb_addr_back;
        // Raw write to real CFBAR (PAC offset is wrong — see setup_ltdc_layer)
        unsafe { (0x5000_10AC as *mut u32).write_volatile(next) }; // L1CFBAR
        unsafe { (0x5000_1024 as *mut u32).write_volatile(1) } // SRCR.IMR;
        core::mem::swap(&mut self.fb_addr, &mut self.fb_addr_back);
        // Re-pulse WCR LTDCEN each frame (adapted command mode)
        unsafe { (0x5000_0404 as *mut u32).write_volatile(0x0C) };
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    /// Initialize SDRAM from scratch at the post-HAL clock rate.
    ///
    /// Configures FMC GPIO pins (AF12), enables FMC clock, then performs
    /// a full JEDEC init sequence. Can be called with or without the
    /// early PAC init (pac_sdram_init feature).
    ///
    /// IS42S32800J-6: 4 banks × 4096 rows × 512 cols × 32 bits, 166 MHz max.
    /// SDCLK = hclk3/2 = 100 MHz (10 ns/cycle).
    fn init_sdram() -> u32 {
        // ── FMC GPIO pin setup (AF12, high-speed, push-pull) ────────────
        // STM32H747I-DISCO SDRAM pin mapping (IS42S32800J):
        //   GPIOD: 0,1,8,9,10,14,15
        //   GPIOE: 0,1,7,8,9,10,11,12,13,14,15
        //   GPIOF: 0,1,2,3,4,5,11,12,13,14,15
        //   GPIOG: 0,1,2,4,5,8,15
        //   GPIOH: 5,6,7,8,9,10,11,12,13,14,15
        //   GPIOI: 0,1,2,3,4,5,6,7,9,10
        unsafe {
            // Enable GPIO D-I clocks (AHB4ENR bits 3-8)
            let ahb4enr = 0x5802_44E0u32 as *mut u32;
            ahb4enr.write_volatile(ahb4enr.read_volatile()
                | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8));
            let _ = (ahb4enr as *const u32).read_volatile();

            // Enable FMC clock (AHB3ENR bit 12)
            let ahb3enr = (0x5802_4400 + 0xD4) as *mut u32;
            ahb3enr.write_volatile(ahb3enr.read_volatile() | (1 << 12));
            let _ = (ahb3enr as *const u32).read_volatile();
        }

        // Configure pin as AF12, very-high speed, push-pull
        #[inline(always)]
        fn fmc_pin(gpio_base: u32, pin: u32) {
            unsafe {
                let moder = gpio_base as *mut u32;
                let ospeedr = (gpio_base + 0x08) as *mut u32;
                let afr = if pin < 8 {
                    (gpio_base + 0x20) as *mut u32 // AFRL
                } else {
                    (gpio_base + 0x24) as *mut u32 // AFRH
                };
                let bit = if pin < 8 { pin } else { pin - 8 };
                // MODER: AF mode (0b10)
                let m = moder.read_volatile();
                moder.write_volatile((m & !(3 << (pin * 2))) | (2 << (pin * 2)));
                // OSPEEDR: very high (0b11)
                let s = ospeedr.read_volatile();
                ospeedr.write_volatile((s & !(3 << (pin * 2))) | (3 << (pin * 2)));
                // AFR: AF12 (0xC)
                let a = afr.read_volatile();
                afr.write_volatile((a & !(0xF << (bit * 4))) | (0xC << (bit * 4)));
            }
        }

        const GPIOD: u32 = 0x5802_0C00;
        const GPIOE: u32 = 0x5802_1000;
        const GPIOF: u32 = 0x5802_1400;
        const GPIOG: u32 = 0x5802_1800;
        const GPIOH: u32 = 0x5802_1C00;
        const GPIOI: u32 = 0x5802_2000;

        for &p in &[0,1,8,9,10,14,15]          { fmc_pin(GPIOD, p); }
        for &p in &[0,1,7,8,9,10,11,12,13,14,15] { fmc_pin(GPIOE, p); }
        for &p in &[0,1,2,3,4,5,11,12,13,14,15]  { fmc_pin(GPIOF, p); }
        for &p in &[0,1,2,4,5,8,15]            { fmc_pin(GPIOG, p); }
        for &p in &[5,6,7,8,9,10,11,12,13,14,15] { fmc_pin(GPIOH, p); }
        for &p in &[0,1,2,3,4,5,6,7,9,10]      { fmc_pin(GPIOI, p); }

        // GPIO telemetry: verify clocks and pin config
        const D3_GPIO: *mut u32 = 0x3800_0500u32 as *mut u32;
        unsafe {
            let ahb4enr = (0x5802_44E0u32 as *const u32).read_volatile();
            let gpiod_moder = (GPIOD as *const u32).read_volatile();
            let gpiod_afrh = ((GPIOD + 0x24) as *const u32).read_volatile();
            let gpiof_moder = (GPIOF as *const u32).read_volatile();
            D3_GPIO.write_volatile(0x6F10_0001);  // sentinel
            D3_GPIO.add(1).write_volatile(ahb4enr);
            D3_GPIO.add(2).write_volatile(gpiod_moder);
            D3_GPIO.add(3).write_volatile(gpiod_afrh);
            D3_GPIO.add(4).write_volatile(gpiof_moder);
        }

        // ── FMC SDRAM controller init ───────────────────────────────────
        // STM32H747I-DISCO: SDRAM on FMC Bank 2 (SDNE1=PH6, SDCKE1=PH7)
        const FMC_BCR1:  *mut u32 = 0x5200_4000u32 as *mut u32;
        const FMC_SDCR1: *mut u32 = 0x5200_4140u32 as *mut u32;
        const FMC_SDCR2: *mut u32 = 0x5200_4144u32 as *mut u32;
        const FMC_SDTR1: *mut u32 = 0x5200_4148u32 as *mut u32;
        const FMC_SDTR2: *mut u32 = 0x5200_414Cu32 as *mut u32;
        const FMC_SDCMR: *mut u32 = 0x5200_4150u32 as *mut u32;
        const FMC_SDRTR: *mut u32 = 0x5200_4154u32 as *mut u32;
        const FMC_SDSR:  *const u32 = 0x5200_4158u32 as *const u32;

        #[inline(always)]
        fn sdram_busy_wait() {
            while unsafe { FMC_SDSR.read_volatile() } & (1 << 5) != 0 {
                cortex_m::asm::nop();
            }
        }
        fn sdram_cmd(mode: u32, nrfs: u32, mrd: u32) {
            sdram_busy_wait();
            unsafe {
                FMC_SDCMR.write_volatile(
                    ((mrd & 0x1FFF) << 9)
                    | ((nrfs & 0xF) << 5)
                    | (1 << 3)          // CTB2 = bank 2 (SDNE1/SDCKE1 on H747I-DISCO)
                    | (mode & 0x7)
                );
            }
            sdram_busy_wait();
        }

        // D3 SRAM telemetry base for SDRAM reinit
        const D3: *mut u32 = 0x3800_0400u32 as *mut u32;
        unsafe {
            D3.write_volatile(0x5D_000001); // entered reinit

            // FMC peripheral reset + full JEDEC reinit at post-HAL clock.
            // The early PAC init (SDCLK=0b10) runs at 32 MHz. After HAL
            // changes hclk3 to 200 MHz, SDCLK becomes 100 MHz with wrong
            // timing params. Reset the FMC and reinitialize from scratch.

            let old_sdcr1 = FMC_SDCR1.read_volatile();
            let old_sdsr  = FMC_SDSR.read_volatile();
            D3.add(1).write_volatile(old_sdcr1); // old SDCR1
            D3.add(4).write_volatile(old_sdsr);  // old SDSR

            // Step 1: FMC peripheral reset via RCC_AHB3RSTR bit 12
            const RCC_AHB3RSTR: *mut u32 = (0x5802_4400 + 0x7C) as *mut u32;
            let rstr = RCC_AHB3RSTR.read_volatile();
            RCC_AHB3RSTR.write_volatile(rstr | (1 << 12)); // assert
            cortex_m::asm::dsb();
            cortex_m::asm::delay(10_000);
            RCC_AHB3RSTR.write_volatile(rstr & !(1 << 12)); // release
            cortex_m::asm::dsb();
            cortex_m::asm::delay(10_000);

            // Re-enable FMC clock + FMCEN after reset
            const RCC_AHB3ENR: *mut u32 = (0x5802_4400 + 0xD4) as *mut u32;
            RCC_AHB3ENR.write_volatile(RCC_AHB3ENR.read_volatile() | (1 << 12));
            let _ = (RCC_AHB3ENR as *const u32).read_volatile();
            FMC_BCR1.write_volatile(FMC_BCR1.read_volatile() | (1u32 << 31));
            cortex_m::asm::dsb();
            cortex_m::asm::delay(10_000);

            // Verify reset cleared SDCR1
            let post_reset_sdcr1 = FMC_SDCR1.read_volatile();
            D3.add(10).write_volatile(post_reset_sdcr1); // should be 0x02D0

            // Step 2: Configure SDRAM Bank 2 registers.
            // SDCR1: shared bits only (SDCLK, RBURST, RPIPE). Bank-specific
            // bits (NC, NR, MWID, NB, CAS, WP) go in SDCR2.
            FMC_SDCR1.write_volatile(
                (0b00 << 13)  // RPIPE  = 0 (shared, only in SDCR1)
                | (1 << 12)   // RBURST = 1 (shared, only in SDCR1)
                | (0b10 << 10)// SDCLK  = hclk3/2 (shared, only in SDCR1)
            );
            // SDCR2: bank-specific config for IS42S32800J
            FMC_SDCR2.write_volatile(
                (0 << 9)      // WP     = 0
                | (0b11 << 7) // CAS    = 3
                | (1 << 6)    // NB     = 4 banks
                | (0b10 << 4) // MWID   = 32-bit
                | (0b01 << 2) // NR     = 12-bit row
                | (0b01 << 0) // NC     = 9-bit column
            );

            // SDTR1: shared timing (TRP, TRC must be in SDTR1 per RM0399)
            FMC_SDTR1.write_volatile(
                (1 << 20) // TRP  = 1 → 2 cycles (20 ns ≥ 18 ns) — shared
                | (5 << 12) // TRC  = 5 → 6 cycles (60 ns ≥ 60 ns) — shared
            );
            // SDTR2: bank-specific timing for IS42S32800J-6 at 100 MHz
            FMC_SDTR2.write_volatile(
                (1 << 24)   // TRCD = 1 → 2 cycles (20 ns ≥ 18 ns)
                | (1 << 16) // TWR  = 1 → 2 cycles
                | (4 << 8)  // TRAS = 4 → 5 cycles (50 ns ≥ 42 ns)
                | (7 << 4)  // TXSR = 7 → 8 cycles (80 ns ≥ 72 ns)
                | (1 << 0)  // TMRD = 1 → 2 cycles
            );
            cortex_m::asm::dsb();

            // Step 3: Re-enable FMC
            FMC_BCR1.write_volatile(FMC_BCR1.read_volatile() | (1u32 << 31));
            cortex_m::asm::dsb();
            cortex_m::asm::delay(10_000);
        }

        // Step 4: Full JEDEC cold init at the new clock rate (RM0399 §23.9.3).
        // Capture SDSR after each command to verify execution.
        sdram_cmd(0b001, 0, 0);             // clock config enable
        unsafe { D3.add(11).write_volatile(FMC_SDSR.read_volatile()); } // SDSR after clk_en
        cortex_m::asm::delay(1_000_000);    // generous 2.5 ms (≥ 100 µs required)
        sdram_cmd(0b010, 0, 0);             // precharge all
        unsafe { D3.add(12).write_volatile(FMC_SDSR.read_volatile()); } // SDSR after precharge
        sdram_cmd(0b011, 7, 0);             // 8× auto-refresh
        unsafe { D3.add(13).write_volatile(FMC_SDSR.read_volatile()); } // SDSR after refresh
        sdram_cmd(0b100, 0, 0x0230);        // load mode register (BL=1, CAS=3, sequential)
        unsafe { D3.add(14).write_volatile(FMC_SDSR.read_volatile()); } // SDSR after LMR
        sdram_cmd(0b000, 0, 0);             // normal mode
        unsafe { D3.add(15).write_volatile(FMC_SDSR.read_volatile()); } // SDSR after normal

        // Refresh: 64 ms / 4096 rows at 100 MHz → 1562; use 683 (conservative)
        unsafe { FMC_SDRTR.write_volatile(683 << 1); }
        sdram_busy_wait();

        // Also capture SDCMR readback, BCR1, SDTR1 for full picture
        unsafe {
            D3.add(16).write_volatile(FMC_SDCMR.read_volatile()); // SDCMR (should be 0)
            D3.add(17).write_volatile(FMC_BCR1.read_volatile());  // BCR1
            D3.add(18).write_volatile(FMC_SDTR1.read_volatile()); // SDTR1
        }

        // Diagnostics: readback registers + multi-address RAM test
        unsafe {
            let new_sdcr1 = FMC_SDCR1.read_volatile();
            let new_sdsr  = FMC_SDSR.read_volatile();
            D3.add(2).write_volatile(new_sdcr1); // new SDCR1
            D3.add(5).write_volatile(new_sdsr);  // new SDSR

            // Write 4 distinct values to 4 widely-spaced addresses
            let p0 = 0xD000_0000u32 as *mut u32;
            let p1 = 0xD000_1000u32 as *mut u32; // +4 KiB
            let p2 = 0xD010_0000u32 as *mut u32; // +1 MiB
            let p3 = 0xD100_0000u32 as *mut u32; // +16 MiB
            p0.write_volatile(0xCAFE_0000);
            p1.write_volatile(0xCAFE_1111);
            p2.write_volatile(0xCAFE_2222);
            p3.write_volatile(0xCAFE_3333);
            cortex_m::asm::dsb();
            // Read all four back
            let r0 = p0.read_volatile();
            let r1 = p1.read_volatile();
            let r2 = p2.read_volatile();
            let r3 = p3.read_volatile();
            D3.add(6).write_volatile(r0);  // expect 0xCAFE_0000
            D3.add(7).write_volatile(r1);  // expect 0xCAFE_1111
            D3.add(8).write_volatile(r2);  // expect 0xCAFE_2222
            D3.add(9).write_volatile(r3);  // expect 0xCAFE_3333
            D3.write_volatile(0x5D_000002); // reinit complete
        }

        0xD000_0000
    }

    #[inline(always)]
    #[allow(dead_code)]
    fn rgb565(r: u8, g: u8, b: u8) -> u16 {
        let r5 = (r as u16) >> 3;
        let g6 = (g as u16) >> 2;
        let b5 = (b as u16) >> 3;
        (r5 << 11) | (g6 << 5) | b5
    }

    /// Fill the framebuffer with a SMPTE color bars pattern.
    #[allow(dead_code)]
    fn fill_smpte_bars_rgb565(fb: *mut u16, width: u16, height: u16) {
        let w = width as usize;
        let h = height as usize;
        let top_h = (h * 2) / 3; // Top 2/3: standard bars
        let mid_h = top_h + (h / 6); // Next 1/6: alt bars
        let bot_h = h; // Bottom 1/6: pluge/simple

        // Top bars: White, Yellow, Cyan, Green, Magenta, Red, Blue
        let top_colors = [
            Self::rgb565(191, 191, 191), // 75% white
            Self::rgb565(191, 191, 0),   // 75% yellow
            Self::rgb565(0, 191, 191),   // 75% cyan
            Self::rgb565(0, 191, 0),     // 75% green
            Self::rgb565(191, 0, 191),   // 75% magenta
            Self::rgb565(191, 0, 0),     // 75% red
            Self::rgb565(0, 0, 191),     // 75% blue
        ];
        let seg_w = w / top_colors.len();
        for y in 0..top_h {
            let row = y * w;
            for i in 0..top_colors.len() {
                let x0 = i * seg_w;
                let x1 = if i + 1 == top_colors.len() {
                    w
                } else {
                    (i + 1) * seg_w
                };
                for x in x0..x1 {
                    unsafe {
                        fb.add(row + x).write_volatile(top_colors[i]);
                    }
                }
            }
        }

        // Middle bars: Blue, Black, Magenta, Black, Cyan, Black, Gray
        let mid_colors = [
            Self::rgb565(0, 0, 191),   // blue
            Self::rgb565(0, 0, 0),     // black
            Self::rgb565(191, 0, 191), // magenta
            Self::rgb565(0, 0, 0),     // black
            Self::rgb565(0, 191, 191), // cyan
            Self::rgb565(0, 0, 0),     // black
            Self::rgb565(96, 96, 96),  // gray (50%)
        ];
        let seg_w2 = w / mid_colors.len();
        for y in top_h..mid_h {
            let row = y * w;
            for i in 0..mid_colors.len() {
                let x0 = i * seg_w2;
                let x1 = if i + 1 == mid_colors.len() {
                    w
                } else {
                    (i + 1) * seg_w2
                };
                for x in x0..x1 {
                    unsafe {
                        fb.add(row + x).write_volatile(mid_colors[i]);
                    }
                }
            }
        }

        // Bottom: simple PLUGE approximation (dark, near-black, black, white)
        let bot_colors = [
            Self::rgb565(0, 0, 0),
            Self::rgb565(8, 8, 8),
            Self::rgb565(0, 0, 0),
            Self::rgb565(255, 255, 255),
        ];
        let seg_w3 = w / bot_colors.len();
        for y in mid_h..bot_h {
            let row = y * w;
            for i in 0..bot_colors.len() {
                let x0 = i * seg_w3;
                let x1 = if i + 1 == bot_colors.len() {
                    w
                } else {
                    (i + 1) * seg_w3
                };
                for x in x0..x1 {
                    unsafe {
                        fb.add(row + x).write_volatile(bot_colors[i]);
                    }
                }
            }
        }
    }

    /// Fill a framebuffer with SMPTE color bars in ARGB8888 format.
    #[allow(dead_code)]
    fn fill_smpte_bars_argb8888(fb: *mut u32, width: u16, height: u16) {
        let w = width as usize;
        let h = height as usize;
        let top_h = (h * 2) / 3;
        let mid_h = top_h + (h / 6);

        let argb = |r: u8, g: u8, b: u8| -> u32 {
            0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
        };

        // Top 2/3: 75% white, yellow, cyan, green, magenta, red, blue
        let top = [
            argb(191, 191, 191), argb(191, 191, 0), argb(0, 191, 191),
            argb(0, 191, 0), argb(191, 0, 191), argb(191, 0, 0), argb(0, 0, 191),
        ];
        // Middle 1/6: blue, black, magenta, black, cyan, black, gray
        let mid = [
            argb(0, 0, 191), argb(0, 0, 0), argb(191, 0, 191),
            argb(0, 0, 0), argb(0, 191, 191), argb(0, 0, 0), argb(96, 96, 96),
        ];
        // Bottom 1/6: dark, near-black, black, white
        let bot = [argb(8, 8, 8), argb(2, 2, 2), argb(0, 0, 0), argb(255, 255, 255)];

        let fill_rows = |colors: &[u32], y0: usize, y1: usize| {
            let seg = w / colors.len();
            for y in y0..y1 {
                let row = y * w;
                for (i, &c) in colors.iter().enumerate() {
                    let x0 = i * seg;
                    let x1 = if i + 1 == colors.len() { w } else { (i + 1) * seg };
                    for x in x0..x1 {
                        unsafe { fb.add(row + x).write_volatile(c); }
                    }
                }
            }
        };
        fill_rows(&top, 0, top_h);
        fill_rows(&mid, top_h, mid_h);
        fill_rows(&bot, mid_h, h);
    }
}

impl<B: Blitter> DisplayDriver for Stm32h747iDiscoDisplay<B> {
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        // Clip to screen bounds
        let sw = self.width as i32;
        let sh = self.height as i32;
        let x0 = core::cmp::max(0, area.x);
        let y0 = core::cmp::max(0, area.y);
        let x1 = core::cmp::min(sw, area.x + area.width);
        let y1 = core::cmp::min(sh, area.y + area.height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let w = (x1 - x0) as usize;
        let h = (y1 - y0) as usize;
        let src_stride = area.width as usize;
        // Draw into back buffer when available to reduce tearing
        let dst_base = if self.fb_addr_back != 0 && self.fb_addr_back != self.fb_addr {
            self.fb_addr_back
        } else {
            self.fb_addr
        } as usize;

        // Prefer DMA2D path when available by staging the ARGB8888 source
        // into SDRAM (write-through MPU region) so no cache maintenance is
        // required for DMA reads. Fallback to CPU if allocation fails or DMA2D
        // feature is disabled.
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        if let Some(dma2d) = self.dma2d.take() {
            // Acquire a staging buffer from the pool (ARGB8888)
            let stage_bytes = w * h * 4;
            if let Some((pool_ix, stage_ptr)) = self.staging_get(stage_bytes) {
                // Pack the clipped area into staging (row-major, tight stride)
                unsafe {
                    let mut dst = stage_ptr;
                    for row in 0..h {
                        let src_off = row * src_stride;
                        for col in 0..w {
                            let Color(r, g, b, a) = colors[src_off + col];
                            // ARGB8888 order for DMA2D
                            core::ptr::write(dst, a);
                            dst = dst.add(1);
                            core::ptr::write(dst, r);
                            dst = dst.add(1);
                            core::ptr::write(dst, g);
                            dst = dst.add(1);
                            core::ptr::write(dst, b);
                            dst = dst.add(1);
                        }
                    }
                }
                let dst_stride_bytes = (self.width as usize) * 2;
                let dst_start = dst_base + ((y0 as usize) * dst_stride_bytes) + ((x0 as usize) * 2);
                let dst_offset = dst_stride_bytes - (w * 2);
                unsafe {
                    // Configure DMA2D: M2M_PFC ARGB8888 -> RGB565
                    dma2d.fgmar.write(|w| w.bits(stage_ptr as u32));
                    dma2d.fgor.write(|w| w.bits(0));
                    dma2d.fgpfccr.write(|w| w.bits(0)); // ARGB8888
                    dma2d.omar.write(|w| w.bits(dst_start as u32));
                    dma2d.oor.write(|w| w.bits(dst_offset as u32));
                    dma2d.opfccr.write(|w| w.bits(2)); // RGB565
                    dma2d
                        .nlr
                        .write(|wr| wr.bits(((h as u32) << 16) | (w as u32)));
                    dma2d.cr.write(|w| w.bits(0x0001_0000)); // M2M_PFC
                    dma2d.cr.modify(|r, w| w.bits(r.bits() | 1)); // START
                    while dma2d.isr.read().bits() & 1 == 0 {}
                    dma2d.ifcr.write(|w| w.bits(1));
                }
                self.dma2d = Some(dma2d);
                self.staging_release(pool_ix);
                return;
            } else {
                // Put back DMA2D and fall through to CPU path
                self.dma2d = Some(dma2d);
            }
        }

        // CPU fallback path
        {
            let dst_ptr = dst_base as *mut u16;
            for row in 0..h {
                let src_off = row * src_stride;
                let dst_off = ((y0 as usize + row) * self.width as usize) + x0 as usize;
                for col in 0..w {
                    let Color(r, g, b, a) = colors[src_off + col];
                    let r5 = (r as u16) >> 3;
                    let g6 = (g as u16) >> 2;
                    let b5 = (b as u16) >> 3;
                    let rgb565: u16 = (r5 << 11) | (g6 << 5) | b5;
                    unsafe {
                        dst_ptr.add(dst_off + col).write_volatile(rgb565);
                    }
                    let _ = a;
                }
            }
        }
        // Layer uses direct framebuffer; no register reload required for content updates.
    }
}

/// Touch input driver for the STM32H747I-DISCO board.
///
/// Polls the FT5336 capacitive controller over I²C and optionally uses an
/// interrupt line. When no interrupt is provided the driver simply polls the
/// controller each time [`poll`](InputDevice::poll) is called.
#[cfg(feature = "stm32h747i_disco")]
pub struct Stm32h747iDiscoInput<I2C, INT> {
    touch: Ft5336<I2C>,
    int: INT,
    last: Option<(u16, u16)>,
}

#[cfg(feature = "stm32h747i_disco")]
/// Dummy pin used when no interrupt line is supplied.
pub struct DummyPin;

#[cfg(feature = "stm32h747i_disco")]
impl embedded_hal::digital::ErrorType for DummyPin {
    type Error = core::convert::Infallible;
}

#[cfg(feature = "stm32h747i_disco")]
impl InputPin for DummyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
/// Initialize I2C4 on PD12/PD13 at 400 kHz for the FT5336 touch controller.
pub fn init_touch_i2c(
    i2c4: stm32h7xx_hal::pac::I2C4,
    gpiod: stm32h7xx_hal::gpio::gpiod::Parts,
    i2c4_rec: stm32h7xx_hal::rcc::rec::I2c4,
    clocks: &stm32h7xx_hal::rcc::CoreClocks,
) -> stm32h7xx_hal::i2c::I2c<stm32h7xx_hal::pac::I2C4> {
    use stm32h7xx_hal::prelude::*;
    let _scl = gpiod.pd12.into_alternate_open_drain::<4>();
    let _sda = gpiod.pd13.into_alternate_open_drain::<4>();
    stm32h7xx_hal::i2c::I2c::i2c4(i2c4, 400.kHz(), i2c4_rec, clocks)
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C> Stm32h747iDiscoInput<I2C, DummyPin>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Create a new input driver from an initialized I²C peripheral without an
    /// interrupt line. The controller is polled on each call to
    /// [`InputDevice::poll`].
    pub fn new(i2c: I2C) -> Self {
        Self {
            touch: Ft5336::new(i2c),
            int: DummyPin,
            last: None,
        }
    }
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C, INT> Stm32h747iDiscoInput<I2C, INT>
where
    I2C: I2c<SevenBitAddress>,
    INT: InputPin,
{
    /// Create a new input driver using an interrupt line.
    pub fn new_with_int(i2c: I2C, int: INT) -> Self {
        Self {
            touch: Ft5336::new(i2c),
            int,
            last: None,
        }
    }

    fn int_active(&mut self) -> bool {
        self.int.is_low().unwrap_or(true)
    }
}

#[cfg(feature = "stm32h747i_disco")]
impl<I2C, INT> InputDevice for Stm32h747iDiscoInput<I2C, INT>
where
    I2C: I2c<SevenBitAddress>,
    INT: InputPin,
{
    fn poll(&mut self) -> Option<Event> {
        if !self.int_active() {
            return None;
        }
        let touch = self.touch.read_touch().ok()?;
        match (touch, self.last) {
            (Some((x, y)), Some((lx, ly))) => {
                self.last = Some((x, y));
                if (x, y) != (lx, ly) {
                    Some(Event::PointerMove {
                        x: x as i32,
                        y: y as i32,
                    })
                } else {
                    None
                }
            }
            (Some((x, y)), None) => {
                self.last = Some((x, y));
                Some(Event::PointerDown {
                    x: x as i32,
                    y: y as i32,
                })
            }
            (None, Some((lx, ly))) => {
                self.last = None;
                Some(Event::PointerUp {
                    x: lx as i32,
                    y: ly as i32,
                })
            }
            (None, None) => None,
        }
    }
}
