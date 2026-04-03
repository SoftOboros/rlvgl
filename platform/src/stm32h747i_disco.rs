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
use crate::otm8009a::Otm8009a;
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
    ltdc: LTDC,
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
    ) -> Self
    where
        BL: SetDutyCycle,
        RST: OutputPin,
    {
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
            ltdc,
            dsi,
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            dma2d: Some(dma2d),
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            staging_pool: [StagingBuf::EMPTY; 3],
        };
        disp.set_backlight(0);

        // ── ST BSP reference values for OTM8009A 800×480 ────────────────────
        let width = 800u16;
        let height = 480u16;
        let hsw: u16 = 10;   // HSYNC width
        let hbp: u16 = 20;   // Horizontal back porch
        let hfp: u16 = 10;   // Horizontal front porch
        let vsw: u16 = 2;    // VSYNC width
        let vbp: u16 = 13;   // Vertical back porch
        let vfp: u16 = 17;   // Vertical front porch

        // ── RM0399 §34.14: DSI programming procedure ───────────────────────

        // Step 1: LTDC timing
        disp.configure_ltdc_timing(width, height, hsw, hbp, hfp, vsw, vbp, vfp);

        // Step 2: DSI regulator enable + wait ready
        disp.dsi.wrpcr.modify(|r, w| unsafe { w.bits(r.bits() | (1 << 24)) }); // REGEN
        {
            let mut tries = 100_000u32;
            while !disp.dsi.wisr.read().rrs().bit() {
                tries -= 1;
                if tries == 0 { break; }
                cortex_m::asm::nop();
            }
        }

        // Step 3: DSI wrapper PLL (matches ST BSP: IDF=5, NDIV=100, ODF=0)
        //   HSE = 25 MHz, IDF=5 → ref=5 MHz
        //   NDIV=100 → VCO = 5 × 2 × 100 = 1000 MHz
        //   ODF=0 (÷1) → f_PHY = 1000/2 = 500 MHz → 500 Mbps/lane
        //   Lane byte clock = 500/8 = 62.5 MHz
        disp.dsi.wrpcr.write(|w| {
            w.regen().set_bit();
            w.pllen().set_bit();
            unsafe {
                w.idf().bits(5);
                w.ndiv().bits(100);
                w.odf().bits(0); // ODF=0 (DSI_PLL_OUT_DIV1)
            }
            w
        });
        {
            let mut tries = 100_000u32;
            while !disp.dsi.wisr.read().pllls().bit() {
                tries -= 1;
                if tries == 0 { break; }
                cortex_m::asm::nop();
            }
        }

        // Step 4: D-PHY — 2 data lanes
        disp.dsi.pconfr.write(|w| unsafe {
            w.nl().bits(1)          // 2 lanes
             .sw_time().bits(0x28)
        });
        // Clock lane: disable auto clock lane control (matches ST BSP)
        disp.dsi.clcr.write(|w| unsafe { w.bits(0x01) }); // DPCC only, no ACR

        // Step 5: DSI Host timings
        disp.dsi.cltcr.write(|w| unsafe { w.bits((10 << 16) | 35) });
        disp.dsi.dltcr.write(|w| unsafe { w.bits((15 << 24) | (10 << 16) | 20) });

        // Step 5b: TX escape clock divider (required for LP↔HS transitions!)
        // ST BSP: TXEscapeCkdiv = 4 → escape_clk = 62.5/8 = 7.8 MHz (< 20 MHz max)
        disp.dsi.ccr.write(|w| unsafe { w.bits(4) }); // TXECKDIV = 4

        // Step 6: Flow control
        disp.dsi.pcr.write(|w| unsafe { w.bits(0x15) }); // ETTXE + BTAE + ECCRXE

        // Step 7: DSI Host LTDC interface — VCID=0, RGB888 (ST BSP), all active-high
        disp.dsi.lvcidr.write(|w| unsafe { w.vcid().bits(0) });
        disp.dsi.lcolcr.write(|w| unsafe { w.colc().bits(5) }); // 5 = RGB888
        // Polarity: all active-high (matches ST BSP)
        disp.dsi.lpcr.write(|w| unsafe { w.bits(0x07) }); // HSP=1, VSP=1, DEP=1

        // Step 8: Video mode — burst mode (matches ST BSP)
        //   Lane byte clock = 62.5 MHz, pixel clock ≈ 27.4 MHz (PLL3_R=32MHz)
        //   In burst mode, HLINE = total line period in lane byte clocks
        let lane_byte_clk: u32 = 62500; // kHz
        let pixel_clk: u32 = 27429;     // kHz (from ST BSP)
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
        disp.dsi.vccr.write(|w| unsafe { w.bits(0) }); // 0 chunks (burst)
        disp.dsi.vnpcr.write(|w| unsafe { w.bits(0xFFF) }); // max null packets
        // VMCR: burst mode + LP transitions enabled + LP command enable
        disp.dsi.vmcr.write(|w| unsafe {
            w.bits(
                (0b10 << 0)     // VMT = burst mode
                | (1 << 8)      // LPVSAE
                | (1 << 9)      // LPVBPE
                | (1 << 10)     // LPVFPE
                | (1 << 11)     // LPVAE
                | (1 << 12)     // LPHBPE
                | (1 << 13)     // LPHFPE
                | (1 << 15)     // LPCE
            )
        });
        // LP largest packet sizes (for LP command during blanking)
        disp.dsi.lpmcr.write(|w| unsafe { w.bits((4 << 16) | 4) }); // VLPSIZE=4, LPSIZE=4

        // Step 9: Mode + wrapper config — video mode, RGB888
        // MCR.CMDM must be cleared for video mode (ST HAL requirement)
        disp.dsi.mcr.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 0)) }); // CMDM=0
        disp.dsi.wcfgr.write(|w| unsafe {
            w.bits(
                (0 << 0)    // DSIM = 0 (video mode)
                | (5 << 1)  // COLMUX = 5 (RGB888, matches LCOLCR)
            )
        });

        // Step 10: Enable D-PHY
        disp.dsi.pctlr.write(|w| w.den().set_bit().cke().set_bit());

        // Step 11: Enable DSI Host
        disp.dsi.cr.write(|w| w.en().set_bit());

        // Step 12: Enable DSI Wrapper
        disp.dsi.wcr.write(|w| w.dsien().set_bit());
        cortex_m::asm::delay(2_000_000);

        // Step 13: Reset panel and send OTM8009A init commands
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
        let _panel_ok = Otm8009a::init(&mut disp.dsi);
        // Disable LP command overrides after panel init (video mode takes over)
        disp.dsi.cmcr.write(|w| unsafe { w.bits(0) });

        // ── SDRAM framebuffer allocation ────────────────────────────────────
        let fb = Self::init_sdram();
        sdram_alloc::init(fb, 32 * 1024 * 1024);
        let fb_bytes = (width as usize) * (height as usize) * 4; // ARGB8888
        let fb_addr = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb);
        let fb_back = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb_addr);
        disp.fb_addr = fb_addr;
        disp.fb_addr_back = fb_back;
        disp.width = width;
        disp.height = height;
        Self::configure_mpu_sdram_writethrough(0xC000_0000, 32 * 1024 * 1024);
        // Fill with solid white (ARGB8888: 0xFFFFFFFF)
        unsafe {
            let ptr = fb_addr as *mut u32;
            for i in 0..(width as usize * height as usize) {
                ptr.add(i).write_volatile(0xFF_FF_00_00); // Red for visibility
            }
        }

        // Step 14: Setup LTDC layer (but don't enable GCR yet)
        disp.setup_ltdc_layer(fb_addr, width, height);

        #[cfg(feature = "sdram_ramtest")]
        {
            let _ = sdram_alloc::alloc(fb_bytes, 64)
                .map(|addr| Self::fill_smpte_bars_rgb565(addr as *mut u16, width, height));
        }

        // Step 15: Start video — LTDCEN first (opens bridge), then LTDC GCR
        // (starts pixel scanning).  Both must be active for video flow.
        // Write LTDCEN via direct register write to avoid read-modify-write hang.
        unsafe { (0x50000404 as *mut u32).write_volatile(0x0C) }; // DSIEN + LTDCEN
        disp.ltdc.gcr.modify(|_, w| w.ltdcen().set_bit());
        disp.ltdc.srcr.write(|w| w.imr().reload());

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

        self.ltdc
            .sscr
            .write(|w| unsafe { w.bits((vshm1 << 16) | hswm1) });
        self.ltdc
            .bpcr
            .write(|w| unsafe { w.bits((avbp << 16) | ahbp) });
        self.ltdc
            .awcr
            .write(|w| unsafe { w.bits((aah << 16) | aaw) });
        self.ltdc
            .twcr
            .write(|w| unsafe { w.bits((totalh << 16) | totalw) });
        self.ltdc.bccr.write(|w| unsafe { w.bits(0) });
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
        let layer0 = &self.ltdc.layer1;
        // Match the timing values used in new() (ST BSP OTM8009A values)
        let hsw: u32 = 10;
        let hbp: u32 = 20;
        let vsw: u32 = 2;
        let vbp: u32 = 13;
        let x0 = hsw + hbp + 1;
        let x1 = x0 + (width as u32) - 1;
        let y0 = vsw + vbp + 1;
        let y1 = y0 + (height as u32) - 1;
        layer0.whpcr.write(|w| unsafe { w.bits((x1 << 16) | x0) });
        layer0.wvpcr.write(|w| unsafe { w.bits((y1 << 16) | y0) });
        layer0.cfbar.write(|w| w.cfbadd().bits(fb));
        layer0
            .cfblr
            .write(|w| unsafe { w.bits(((pitch + 3) << 16) | pitch) });
        layer0.cfblnr.write(|w| w.cfblnbr().bits(height));
        layer0.pfcr.write(|w| w.pf().argb8888());
        layer0.cacr.write(|w| w.consta().bits(255));
        layer0.bfcr.write(|w| unsafe { w.bits(0x0405) });
        layer0.cr.modify(|_, w| w.len().enabled());
        self.ltdc.srcr.write(|w| w.imr().reload());
        // NOTE: LTDC GCR enable is deferred to after DSI WCR LTDCEN
        // to prevent AXI bus lockup (LTDC must not scan before DSI consumes).
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
        // For H747I-DISCO SDRAM: base=0xC000_0000, size=32 MiB.
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
            // XN=1 (execute never), AP=0b011 (full access), TEX=0, C=1 (WT), B=0, S=0
            let xn = 1u32 << 28;
            let ap = 0b011u32 << 24;
            let tex_cb_s = (0u32 << 19) | (1u32 << 17) | (0u32 << 16); // TEX=0,C=1,B=0
            let s = 0u32 << 18;
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

    /// Swap LTDC layer address between front/back buffers and reload
    pub fn present(&mut self) {
        let next = self.fb_addr_back;
        self.ltdc.layer1.cfbar.write(|w| w.cfbadd().bits(next));
        self.ltdc.srcr.write(|w| w.imr().reload());
        core::mem::swap(&mut self.fb_addr, &mut self.fb_addr_back);
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    /// Initialize the external SDRAM and return its base address.
    fn init_sdram() -> u32 {
        0xC000_0000
    }

    #[inline(always)]
    fn rgb565(r: u8, g: u8, b: u8) -> u16 {
        let r5 = (r as u16) >> 3;
        let g6 = (g as u16) >> 2;
        let b5 = (b as u16) >> 3;
        (r5 << 11) | (g6 << 5) | b5
    }

    /// Fill the framebuffer with a SMPTE color bars pattern.
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
