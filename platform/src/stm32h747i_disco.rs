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
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::{DSIHOST, FMC, LTDC};

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
        let mut cur = CUR.load(Ordering::Relaxed);
        let align_m1 = (align as u32).wrapping_sub(1);
        let aligned = (cur.wrapping_add(align_m1)) & !align_m1;
        let new_cur = aligned.wrapping_add(size as u32);
        if new_cur > END.load(Ordering::Relaxed) {
            return None;
        }
        CUR.store(new_cur, Ordering::Relaxed);
        Some(aligned)
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
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fb_addr_back: u32,
}

impl<B: Blitter, BL, RST> Stm32h747iDiscoDisplay<B, BL, RST> {
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
        fmc: FMC,
    ) -> Self
    where
        BL: SetDutyCycle,
        RST: OutputPin,
    {
        // Assume clocks for LTDC and DSI are already enabled by HAL setup.
        // Ensure the panel is held in reset and the backlight is off
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
        };
        // Start with backlight off
        disp.set_backlight(0);
        disp.reset_panel();
        // Panel bring-up over DSI is WIP; keep a stub for now
        Otm8009a::init(&mut disp.dsi);
        // Configure LTDC timing for 800x480 based on typical OTM8009A values
        let width = 800u16;
        let height = 480u16;
        disp.configure_ltdc_timing(
            width,
            height,
            20,  // HSW
            140, // HBP
            20,  // HFP
            4,   // VSW
            34,  // VBP
            10,  // VFP
        );
        let fb = Self::init_sdram(fmc);
        // Minimal DSI host setup for RGB565 on VCID 0 (video mode tuning TBD)
        disp.configure_dsi_video_mode(width, height);
        // Initialize a simple SDRAM allocator and allocate the primary framebuffer
        // H747I-DISCO uses a 32 MB SDRAM at 0xC000_0000.
        sdram_alloc::init(0xC000_0000, 32 * 1024 * 1024);
        let fb_bytes = (width as usize) * (height as usize) * 2; // RGB565
        let fb_addr = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb);
        // Allocate a back buffer as well for potential page-flip
        let fb_back = sdram_alloc::alloc(fb_bytes, 64).unwrap_or(fb_addr);
        disp.fb_addr = fb_addr;
        disp.fb_addr_back = fb_back;
        disp.width = width;
        disp.height = height;
        // Pre-fill framebuffer with SMPTE color bars for visual verification
        Self::fill_smpte_bars_rgb565(fb_addr as *mut u16, width, height);
        disp.setup_ltdc_layer(fb_addr, width, height);

        // Optionally allocate additional framebuffers for testing and fill them
        #[cfg(feature = "sdram_ramtest")]
        {
            let _ = sdram_alloc::alloc(fb_bytes, 64).map(|addr| {
                Self::fill_smpte_bars_rgb565(addr as *mut u16, width, height)
            });
        }
        // Gentle brightness ramp for bring-up
        #[allow(clippy::arithmetic_side_effects)]
        {
            let max = disp.backlight.max_duty_cycle();
            let target = max / 2;
            let step = core::cmp::max(1, target / 20);
            let mut lvl = 0u16;
            while lvl < target {
                let _ = disp.backlight.set_duty_cycle(lvl);
                // Small delay between steps; coarse cycle wait is sufficient here
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
    fn configure_ltdc_timing(&mut self, width: u16, height: u16, hsw: u16, hbp: u16, hfp: u16, vsw: u16, vbp: u16, vfp: u16) {
        let vsh = vsw;
        let hswm1 = (hsw.saturating_sub(1)) as u32;
        let vshm1 = (vsh.saturating_sub(1)) as u32;
        let ahbp = (hsw as u32 + hbp as u32).saturating_sub(1);
        let avbp = (vsh as u32 + vbp as u32).saturating_sub(1);
        let aaw = (hsw as u32 + hbp as u32 + width as u32).saturating_sub(1);
        let aah = (vsh as u32 + vbp as u32 + height as u32).saturating_sub(1);
        let totalw = (hsw as u32 + hbp as u32 + width as u32 + hfp as u32).saturating_sub(1);
        let totalh = (vsh as u32 + vbp as u32 + height as u32 + vfp as u32).saturating_sub(1);

        self.ltdc.sscr.write(|w| unsafe { w.bits((vshm1 << 16) | hswm1) });
        self.ltdc.bpcr.write(|w| unsafe { w.bits((avbp << 16) | ahbp) });
        self.ltdc.awcr.write(|w| unsafe { w.bits((aah << 16) | aaw) });
        self.ltdc.twcr.write(|w| unsafe { w.bits((totalh << 16) | totalw) });
        self.ltdc.bccr.write(|w| unsafe { w.bits(0) });
        self.ltdc.gcr.modify(|_, w| w.ltdcen().set_bit());
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
        // A real implementation would delay here to satisfy the reset timing
        let _ = self.reset.set_high();
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
        let pitch = width * 2; // RGB565 bytes/line
        let layer0 = &self.ltdc.layer1;
        // Windowing based on timing (positions relative to sync/backporch)
        // Here we assume typical OTM8009A timing constants as above
        let hsw: u32 = 20;
        let hbp: u32 = 140;
        let vsw: u32 = 4;
        let vbp: u32 = 34;
        let x0 = hsw + hbp + 1;
        let x1 = x0 + (width as u32) - 1;
        let y0 = vsw + vbp + 1;
        let y1 = y0 + (height as u32) - 1;
        layer0.whpcr.write(|w| unsafe { w.bits((x1 << 16) | x0) });
        layer0.wvpcr.write(|w| unsafe { w.bits((y1 << 16) | y0) });
        layer0.cfbar.write(|w| w.cfbadd().bits(fb));
        layer0.cfblr.write(|w| unsafe { w.bits(((pitch as u32 + 3) << 16) | (pitch as u32)) });
        layer0.cfblnr.write(|w| w.cfblnbr().bits(height));
        layer0.pfcr.write(|w| w.pf().rgb565());
        // Set full alpha and blending factors (constant alpha)
        layer0.cacr.write(|w| w.consta().bits(255));
        layer0.bfcr.write(|w| unsafe { w.bits(0x0405) });
        layer0.cr.modify(|_, w| w.len().enabled());
        self.ltdc.srcr.write(|w| w.imr().reload());
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
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
    fn init_sdram(_fmc: FMC) -> u32 {
        // Configure external SDRAM on FMC Bank1 and return its base address.
        // NOTE: GPIO pin mux for FMC must be configured before calling this.
        let fmc = _fmc;
        // Basic SDRAM control/timing for a 16-bit, 4-bank SDRAM with CAS=3.
        // Timings are conservative defaults suitable for ~100 MHz SDRAM clock.
        // SDCR1: RPIPE=1, RBURST=1, SDCLK=HCLK/2, WP=0, CAS=3, NB=4, MWID=16, NR=12, NC=8
        unsafe {
            // Control register
            // MWID=32-bit bus, CAS=3, 4 banks, SDCLK=HCLK/2, RBURST on, RPIPE=1
            fmc.sdcr1.write(|w| w.bits(
                (1 << 13) | // RPIPE = 1 (one HCLK cycle)
                (1 << 12) | // RBURST = 1 (enable burst)
                (2 << 10) | // SDCLK = HCLK/2 (target ~100 MHz)
                (3 << 7)  | // CAS = 3
                (1 << 6)  | // NB = 4 internal banks
                (2 << 4)  | // MWID = 32 bits (10b)
                (1 << 2)  | // NR = 12 rows (01b) — adjust if part differs
                (0 << 0)    // NC = 8 columns (00b)
            ));
            // Timing register (values are encoded as cycles-1)
            // TMRD=2, TXSR=7, TRAS=5, TRC=7, TWR=2, TRP=2, TRCD=2 at ~100 MHz
            fmc.sdtr1.write(|w| w.bits(
                (1 << 0)  | // TMRD = 2
                (6 << 4)  | // TXSR = 7
                (4 << 8)  | // TRAS = 5
                (6 << 12) | // TRC  = 7
                (1 << 16) | // TWR  = 2
                (1 << 20) | // TRP  = 2
                (1 << 24)   // TRCD = 2
            ));
        }

        // Command sequence: clock enable -> delay -> precharge all -> auto-refresh -> load mode -> set refresh
        unsafe {
            // Clock enable (CTB1)
            fmc.sdcmr.write(|w| w.bits(
                (1 << 0) | // MODE = 1 (Clock Configuration Enable)
                (1 << 3)   // CTB1 = 1 (Target bank 1)
            ));
            // Small delay to allow clock to start
            cortex_m::asm::delay(1000);
            // Precharge all
            fmc.sdcmr.write(|w| w.bits(
                (2 << 0) | // MODE = 2 (PALL)
                (1 << 3)   // CTB1 = 1
            ));
            // Auto-refresh (8 cycles)
            fmc.sdcmr.write(|w| w.bits(
                (3 << 0) | // MODE = 3 (Auto-refresh)
                (1 << 3) | // CTB1 = 1
                (7 << 5)   // NRFS = 8 auto-refresh cycles (encoded as value-1)
            ));
            // Load mode register: BL=1, BT=sequential, CAS=3, OP=standard
            let mrd: u32 = 0
                | 0b000 << 0   // Burst length = 1
                | 0b0   << 3   // Burst type = sequential
                | 0b011 << 4   // CAS latency = 3
                | 0b0   << 7   // Operating mode = standard
                | 0b0   << 9;  // Write burst mode = programmed burst length
            fmc.sdcmr.write(|w| w.bits(
                (4 << 0) | // MODE = 4 (Load Mode Register)
                (1 << 3) | // CTB1 = 1
                (mrd << 9)
            ));
            // Set refresh rate: COUNT = (SDCLK_Hz * 64ms / 8192) - margin
            // For ~100 MHz SDCLK: ~781 - 20 = ~761. Tune margin as needed.
            let count: u32 = 761 & 0x1FFF;
            fmc.sdrtr.write(|w| w.bits(count));
        }
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
        let top_h = (h * 2) / 3;      // Top 2/3: standard bars
        let mid_h = top_h + (h / 6);  // Next 1/6: alt bars
        let bot_h = h;                // Bottom 1/6: pluge/simple

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
                let x1 = if i + 1 == top_colors.len() { w } else { (i + 1) * seg_w };
                for x in x0..x1 {
                    unsafe { fb.add(row + x).write_volatile(top_colors[i]); }
                }
            }
        }

        // Middle bars: Blue, Black, Magenta, Black, Cyan, Black, Gray
        let mid_colors = [
            Self::rgb565(0, 0, 191),     // blue
            Self::rgb565(0, 0, 0),       // black
            Self::rgb565(191, 0, 191),   // magenta
            Self::rgb565(0, 0, 0),       // black
            Self::rgb565(0, 191, 191),   // cyan
            Self::rgb565(0, 0, 0),       // black
            Self::rgb565(96, 96, 96),    // gray (50%)
        ];
        let seg_w2 = w / mid_colors.len();
        for y in top_h..mid_h {
            let row = y * w;
            for i in 0..mid_colors.len() {
                let x0 = i * seg_w2;
                let x1 = if i + 1 == mid_colors.len() { w } else { (i + 1) * seg_w2 };
                for x in x0..x1 {
                    unsafe { fb.add(row + x).write_volatile(mid_colors[i]); }
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
                let x1 = if i + 1 == bot_colors.len() { w } else { (i + 1) * seg_w3 };
                for x in x0..x1 {
                    unsafe { fb.add(row + x).write_volatile(bot_colors[i]); }
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
        if x1 <= x0 || y1 <= y0 { return; }
        let w = (x1 - x0) as usize;
        let h = (y1 - y0) as usize;
        let src_stride = area.width as usize;
        let fb_ptr = self.fb_addr as *mut u16;
        // Write row by row converting ARGB8888 to RGB565
        for row in 0..h {
            let src_off = row * src_stride;
            let dst_off = ((y0 as usize + row) * self.width as usize) + x0 as usize;
            for col in 0..w {
                let Color(r, g, b, a) = colors[src_off + col];
                // Alpha ignore for now (assume opaque)
                let r5 = (r as u16) >> 3;
                let g6 = (g as u16) >> 2;
                let b5 = (b as u16) >> 3;
                let rgb565: u16 = (r5 << 11) | (g6 << 5) | b5;
                unsafe { fb_ptr.add(dst_off + col).write_volatile(rgb565); }
                let _ = a; // silence unused in no-alpha path
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
                Some(Event::PointerDown { x: x as i32, y: y as i32 })
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
