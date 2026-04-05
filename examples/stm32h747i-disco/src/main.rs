#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

extern crate alloc;

#[cfg(not(feature = "c_hal"))]
use core::arch::asm;
use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
#[cfg(not(doc))]
use panic_halt as _;

// The demo app crate provides flush_pending and Application trait for widget
// tree management. The c_hal path uses a server-mode widget tree driven by
// CM4 via IPC and does not need it.

// Auto-generated board support — pin constants and PAC helpers are a reference
// library; not all are consumed in every build configuration.
#[allow(dead_code, unused_imports, unused_macros, unused_unsafe, unknown_lints)]
#[path = "bsp/cm7/pac.rs"]
mod bsp_pac;
mod config_menu;
mod fonts;
mod icon_strip;
mod ipc;
mod star_crawl;
// HAL BSP module is not required for this bring-up path

#[cfg(feature = "splash")]
static SPLASH_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");

/// Desktop background image — decoded into the framebuffer and restored
/// behind widgets when they hide.  Independent of the splash boot screen.
#[cfg(feature = "desktop")]
static DESKTOP_RLE: &[u8] = include_bytes!("../assets/media/splash.rle");


// Optional: route BSP log messages to semihosting when enabled.
#[cfg(feature = "bsp_log")]
#[no_mangle]
fn _bsp_log(args: core::fmt::Arguments) {
    #[cfg(feature = "semihosting")]
    {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "{}", args);
        }
    }
}

/// Global allocator backed by a fixed-size heap in RAM.
#[global_allocator]
static ALLOC: Heap = Heap::empty();

#[cfg(not(feature = "c_hal"))]
fn mpu_rasr(
    size_field: u32,
    ap: u32,
    tex: u32,
    shareable: u32,
    cacheable: u32,
    bufferable: u32,
    execute_never: u32,
) -> u32 {
    let enable = 1u32;
    let size_bits = size_field << 1;
    let tex_bits = tex << 19;
    let s_bits = shareable << 18;
    let c_bits = cacheable << 17;
    let b_bits = bufferable << 16;
    let xn_bits = execute_never << 28;
    enable | size_bits | ap | tex_bits | s_bits | c_bits | b_bits | xn_bits
}

#[cfg(not(feature = "c_hal"))]
fn configure_mpu_regions(cp: &mut cortex_m::Peripherals) {
    const AP_FULL_ACCESS: u32 = 0b011 << 24;

    unsafe {
        set_mpu_trace(0xFACE_0001);
        cp.MPU.ctrl.write(0);
        barrier_dsb();
        barrier_isb();
    }

    #[inline(always)]
    fn configure_slot(
        mpu: &mut cortex_m::peripheral::MPU,
        number: u32,
        base: u32,
        rasr: u32,
        slot: usize,
    ) {
        unsafe {
            mpu.rnr.write(number);
            mpu.rbar.write(base);
            mpu.rasr.write(rasr);
        }
        record_region(slot, base, rasr);
    }

    let mpu = &mut cp.MPU;

    unsafe {
        configure_slot(
            mpu,
            0,
            0x0800_0000,
            mpu_rasr(20, AP_FULL_ACCESS, 0, 0, 1, 1, 0),
            0,
        );
        set_mpu_trace(0xDEAD_0010);

        configure_slot(
            mpu,
            1,
            0x2000_0000,
            mpu_rasr(16, AP_FULL_ACCESS, 0, 0, 1, 1, 1),
            1,
        );
        set_mpu_trace(0xDEAD_0020);

        configure_slot(
            mpu,
            2,
            0x2400_0000,
            mpu_rasr(18, AP_FULL_ACCESS, 0, 1, 1, 1, 1),
            2,
        );
        set_mpu_trace(0xDEAD_0030);

        configure_slot(
            mpu,
            3,
            0x3004_7000,
            mpu_rasr(11, AP_FULL_ACCESS, 0, 1, 0, 0, 1),
            3,
        );
        set_mpu_trace(0xDEAD_0040);

        configure_slot(
            mpu,
            4,
            0x3800_0000,
            mpu_rasr(15, AP_FULL_ACCESS, 0, 1, 1, 1, 1),
            4,
        );
        set_mpu_trace(0xDEAD_0050);

        configure_slot(
            mpu,
            5,
            0xC000_0000,
            mpu_rasr(24, AP_FULL_ACCESS, 1, 1, 0, 0, 1),
            5,
        );
        set_mpu_trace(0xDEAD_0060);

        const MPU_CTRL_ENABLE: u32 = 1;
        const MPU_CTRL_PRIVDEFENA: u32 = 1 << 2;
        mpu.ctrl.write(MPU_CTRL_ENABLE | MPU_CTRL_PRIVDEFENA);
        single_nop();
        barrier_dsb();
        barrier_isb();
        set_mpu_trace(0xDEAD_0003);
    }
}
#[cfg(not(feature = "c_hal"))]
#[allow(unknown_lints, unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_TRACE: u32 = 0;

#[cfg(not(feature = "c_hal"))]
#[allow(unknown_lints, unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_DUMP: [u32; 12] = [0; 12];

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn set_mpu_trace(val: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(MPU_TRACE), val);
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn record_region(slot: usize, base: u32, rasr: u32) {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(MPU_DUMP[slot * 2]);
        core::ptr::write_volatile(ptr, base);
        core::ptr::write_volatile(ptr.add(1), rasr);
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn single_nop() {
    unsafe {
        asm!("nop", options(nomem, nostack, preserves_flags));
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn barrier_dsb() {
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(not(feature = "c_hal"))]
#[inline(always)]
fn barrier_isb() {
    unsafe {
        asm!("isb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
const SDRAM_REFRESH_COUNT: u16 = 566;
#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
const SDRAM_MODE_REGISTER: u16 = 0x0230;

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn wait_for_sdram_ready(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    while fmc.sdsr.read().bits() & (1 << 5) != 0 {
        cortex_m::asm::nop();
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn issue_sdram_command(
    fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock,
    mode: u8,
    auto_refresh: u8,
    mode_register: u16,
) {
    unsafe {
        fmc.sdcmr.write(|w| {
            w.mode()
                .bits(mode)
                .ctb1()
                .clear_bit()
                .ctb2()
                .set_bit()  // Bank 2 (SDNE1/SDCKE1 on H747I-DISCO)
                .nrfs()
                .bits(auto_refresh)
                .mrd()
                .bits(mode_register)
        });
    }
    wait_for_sdram_ready(fmc);
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn configure_fmc_sdram(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    unsafe {
        fmc.bcr1.modify(|_, w| w.fmcen().set_bit());
        // SDCR1: shared bits only (SDCLK, RBURST, RPIPE)
        fmc.sdbank1().sdcr.write(|w| {
            w.sdclk()
                .bits(0b01) // Reserved per RM0399, but required on this silicon
                .rburst()
                .set_bit()
                .rpipe()
                .bits(0)
        });
        // SDCR2: bank-specific config (NC, NR, MWID, NB, CAS, WP)
        // H747I-DISCO SDRAM is on Bank 2 (SDNE1=PH6, SDCKE1=PH7)
        fmc.sdbank2().sdcr.write(|w| {
            w.nc()
                .bits(0b01)
                .nr()
                .bits(0b01)
                .mwid()
                .bits(0b10)
                .nb()
                .set_bit()
                .cas()
                .bits(0b11)
                .wp()
                .clear_bit()
        });
        // SDTR1: shared timing (TRP, TRC must be in SDTR1)
        // PAC sdbank1().sdtr offset = 0x144 = SDCR2 (known PAC bug).
        // Use raw write to SDTR1 at 0x148.
        let sdtr1 = 0x5200_4148u32 as *mut u32;
        sdtr1.write_volatile(
            (1 << 20) // TRP = 2 cycles
            | (6 << 12) // TRC = 7 cycles
        );
        // SDTR2: bank-specific timing
        // PAC sdbank2().sdtr offset = 0x148 = SDTR1 (same PAC bug pattern).
        // Use raw write to SDTR2 at 0x14C.
        let sdtr2 = 0x5200_414Cu32 as *mut u32;
        sdtr2.write_volatile(
            (1 << 24)   // TRCD = 2 cycles
            | (1 << 16) // TWR = 2 cycles
            | (4 << 8)  // TRAS = 5 cycles
            | (6 << 4)  // TXSR = 7 cycles
            | (1 << 0)  // TMRD = 2 cycles
        );
    }

    issue_sdram_command(fmc, 0b001, 0, 0);
    cortex_m::asm::delay(100_000);
    issue_sdram_command(fmc, 0b010, 0, 0);
    issue_sdram_command(fmc, 0b011, 7, 0);
    issue_sdram_command(fmc, 0b100, 0, SDRAM_MODE_REGISTER);
    issue_sdram_command(fmc, 0b000, 0, 0);

    unsafe {
        fmc.sdrtr.write(|w| w.count().bits(SDRAM_REFRESH_COUNT));
    }

    wait_for_sdram_ready(fmc);
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn configure_pin_alt12(gpio: &stm32h7::stm32h747cm7::gpioa::RegisterBlock, pin: u8) {
    let shift2 = (pin as u32) * 2;
    unsafe {
        gpio.moder.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            bits |= 0b10 << shift2;
            w.bits(bits)
        });
        gpio.ospeedr.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            bits |= 0b11 << shift2;
            w.bits(bits)
        });
        gpio.pupdr.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(0b11 << shift2);
            w.bits(bits)
        });
        gpio.otyper.modify(|r, w| {
            let mut bits = r.bits();
            bits &= !(1 << pin);
            w.bits(bits)
        });
        if pin < 8 {
            let shift4 = (pin as u32) * 4;
            gpio.afrl.modify(|r, w| {
                let mut bits = r.bits();
                bits &= !(0xF << shift4);
                bits |= 12 << shift4;
                w.bits(bits)
            });
        } else {
            let shift4 = ((pin as u32) - 8) * 4;
            gpio.afrh.modify(|r, w| {
                let mut bits = r.bits();
                bits &= !(0xF << shift4);
                bits |= 12 << shift4;
                w.bits(bits)
            });
        }
    }
}

#[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
fn early_fmc_setup() {
    use stm32h7::stm32h747cm7::{
        GPIOD, GPIOE, GPIOF, GPIOG, GPIOH, GPIOI, RCC, gpioa::RegisterBlock as GpioRegs,
    };

    let rcc = unsafe { &*RCC::ptr() };

    unsafe {
        // Enable clocks for GPIO D through I so alternate functions can be programmed.
        let mask = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8);
        rcc.ahb4enr.modify(|r, w| w.bits(r.bits() | mask));
        rcc.ahb4enr.read();
    }

    let gpiod = unsafe { &*GPIOD::ptr() as &GpioRegs };
    for &pin in &[0, 1, 8, 9, 10, 14, 15] {
        configure_pin_alt12(gpiod, pin);
    }
    let gpioe = unsafe { &*GPIOE::ptr() as &GpioRegs };
    for &pin in &[0, 1, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpioe, pin);
    }
    let gpiof = unsafe { &*GPIOF::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpiof, pin);
    }
    let gpiog = unsafe { &*GPIOG::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 4, 5, 8, 15] {
        configure_pin_alt12(gpiog, pin);
    }
    let gpioh = unsafe { &*GPIOH::ptr() as &GpioRegs };
    for &pin in &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        configure_pin_alt12(gpioh, pin);
    }
    let gpioi = unsafe { &*GPIOI::ptr() as &GpioRegs };
    for &pin in &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10] {
        configure_pin_alt12(gpioi, pin);
    }

    unsafe {
        // Enable FMC clocks in both the combined and core 1 domains.
        rcc.ahb3enr.modify(|r, w| w.bits(r.bits() | (1 << 12)));
        rcc.ahb3enr.read();
        rcc.c1_ahb3enr.modify(|r, w| w.bits(r.bits() | (1 << 12)));
        rcc.c1_ahb3enr.read();
    }

    let fmc = unsafe { &*stm32h7::stm32h747cm7::FMC::ptr() };
    // D3 SRAM telemetry for early FMC init
    unsafe { (0x3800_0200u32 as *mut u32).write_volatile(0xF0C0_0001u32); }
    configure_fmc_sdram(fmc);
    // Capture SDCR1, SDTR1, SDSR after init
    unsafe {
        let sdcr1 = (0x5200_4140u32 as *const u32).read_volatile();
        let sdtr1 = (0x5200_4148u32 as *const u32).read_volatile();
        let sdsr  = (0x5200_4158u32 as *const u32).read_volatile();
        (0x3800_0204u32 as *mut u32).write_volatile(sdcr1);
        (0x3800_0208u32 as *mut u32).write_volatile(sdtr1);
        (0x3800_020Cu32 as *mut u32).write_volatile(sdsr);
        (0x3800_0200u32 as *mut u32).write_volatile(0xF0C0_0002u32);
    }
}

/// Heap size in bytes.
const HEAP_SIZE: usize = 64 * 1024;

/// Static memory region used to service heap allocations.
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// Application entry point.
#[cfg(not(doc))]
#[entry]
fn main() -> ! {
    // Heap must be ready before any Rust allocation (including rlvgl_app_main).
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    // ── C HAL path ──────────────────────────────────────────────────────────
    // All MCU init (MPU, power, clocks, GPIO, SDRAM) is handled by c_bsp_init,
    // which calls back into rlvgl_app_main() when hardware is ready.
    #[cfg(all(
        feature = "c_hal",
        feature = "stm32h747i_disco_cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // Force-link the BSP crate so its native C library is included.
        extern crate rlvgl_bsps_stm;
        unsafe extern "C" {
            fn c_bsp_init() -> !;
        }
        unsafe { c_bsp_init() }
    }

    // ── Rust HAL path (no c_hal feature) ────────────────────────────────────
    #[cfg(all(
        not(feature = "c_hal"),
        feature = "stm32h747i_disco_cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // D3 breadcrumb: very first thing in Rust HAL path
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0001u32); }
        // Early spin delay to give debuggers time to attach before
        // peripheral clocks and pin configuration. This is a coarse, cycle-based
        // busy-wait that does not rely on any timers being configured yet.
        // Adjust the iteration count as needed for your CPU clock.
        // Rough guide: 10 × 100M cycles ≈ ~2.5s @ 400 MHz, ~10s @ 100 MHz.
        for _ in 0..2 {
            cortex_m::asm::delay(10_000_000);
        }

        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0002u32); } // post-delay
        let mut cp = cortex_m::Peripherals::take().unwrap();
        configure_mpu_regions(&mut cp);
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0003u32); } // post-MPU

        use core::convert::Infallible;
        use embedded_hal::{
            digital::InputPin,
            i2c::{I2c as EhI2c, Operation, SevenBitAddress},
            pwm::{ErrorType as PwmError, SetDutyCycle},
        };
        use rlvgl::core::event::{Event, Key};
        use rlvgl::platform::{
            CpuBlitter, InputDevice, Stm32h747iDiscoDisplay, Stm32h747iDiscoInput,
        };
        #[cfg(feature = "sd_storage")]
        use rlvgl::platform::SdMmcBlockDev;
        use stm32h7xx_hal::prelude::*;

        // Backlight adapter using a HAL GPIO pin as a stand-in for PWM
        use stm32h7xx_hal::gpio::{Output, Pin, PushPull};
        // Backlight control on PJ6 (GPIO fallback); touch INT uses PK7
        #[allow(dead_code)]
        type HalBacklightPin = Pin<'J', 6, Output<PushPull>>;
        #[allow(dead_code)]
        struct HalGpioBacklight(HalBacklightPin);
        impl PwmError for HalGpioBacklight {
            type Error = Infallible;
        }
        impl SetDutyCycle for HalGpioBacklight {
            fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                if duty == 0 {
                    let _ = self.0.set_low();
                } else {
                    let _ = self.0.set_high();
                }
                Ok(())
            }
            fn max_duty_cycle(&self) -> u16 {
                u16::MAX
            }
        }

        // Adapter to bridge HAL v0.2 input pin to embedded-hal 1.0 InputPin
        struct HalInputPin<P>(P);
        impl<P> embedded_hal::digital::ErrorType for HalInputPin<P> {
            type Error = Infallible;
        }
        impl<P: stm32h7xx_hal::hal::digital::v2::InputPin<Error = Infallible>>
            embedded_hal::digital::InputPin for HalInputPin<P>
        {
            fn is_high(&mut self) -> Result<bool, Self::Error> {
                self.0.is_high()
            }
            fn is_low(&mut self) -> Result<bool, Self::Error> {
                self.0.is_low()
            }
        }

        struct ButtonInput<B: InputPin> {
            button: B,
            last: bool,
        }
        impl<B: InputPin> ButtonInput<B> {
            fn new(button: B) -> Self {
                Self {
                    button,
                    last: false,
                }
            }
        }
        impl<B: InputPin> InputDevice for ButtonInput<B> {
            fn poll(&mut self) -> Option<Event> {
                let pressed = self.button.is_low().ok()?;
                match (pressed, self.last) {
                    (true, false) => {
                        self.last = true;
                        Some(Event::KeyDown { key: Key::Enter })
                    }
                    (false, true) => {
                        self.last = false;
                        Some(Event::KeyUp { key: Key::Enter })
                    }
                    _ => None,
                }
            }
        }
        /// Joystick input: polls 5 GPIO pins (SEL, DOWN, LEFT, RIGHT, UP)
        /// and generates KeyDown/KeyUp events on edge transitions.
        struct JoystickInput<S: InputPin, D: InputPin, L: InputPin, R: InputPin, U: InputPin> {
            sel: S, down: D, left: L, right: R, up: U,
            last: [bool; 5],
        }
        impl<S: InputPin, D: InputPin, L: InputPin, R: InputPin, U: InputPin>
            JoystickInput<S, D, L, R, U>
        {
            fn new(sel: S, down: D, left: L, right: R, up: U) -> Self {
                Self { sel, down, left, right, up, last: [false; 5] }
            }
            fn poll(&mut self) -> Option<Event> {
                let pins: [bool; 5] = [
                    self.sel.is_low().unwrap_or(false),
                    self.down.is_low().unwrap_or(false),
                    self.left.is_low().unwrap_or(false),
                    self.right.is_low().unwrap_or(false),
                    self.up.is_low().unwrap_or(false),
                ];
                const KEYS: [Key; 5] = [
                    Key::Enter, Key::ArrowDown, Key::ArrowLeft,
                    Key::ArrowRight, Key::ArrowUp,
                ];
                for i in 0..5 {
                    if pins[i] != self.last[i] {
                        self.last[i] = pins[i];
                        return Some(if pins[i] {
                            Event::KeyDown { key: KEYS[i].clone() }
                        } else {
                            Event::KeyUp { key: KEYS[i].clone() }
                        });
                    }
                }
                None
            }
        }
        // Destructure PAC peripherals and switch to HAL for operation
        let dp = stm32h7::stm32h747cm7::Peripherals::take().unwrap();

        #[cfg(all(feature = "pac_sdram_init", not(feature = "c_hal")))]
        early_fmc_setup();
        // Ensure the PWR peripheral clock is enabled before touching PWR regs.
        // On H7, PWR sits on APB4; without PWREN the VOSRDY poll can hang.
        // Some PACs don’t expose a typed `pwren()`; set the bit position directly.
        dp.RCC
            .apb4enr
            .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 9)) });

        // PWR clock now enabled. Skip PAC-based clock init for bring-up.

        // Now split out PAC peripherals and hand PWR to the HAL.
        let stm32h7::stm32h747cm7::Peripherals {
            PWR,
            RCC,
            SYSCFG,
            GPIOJ,
            GPIOG,
            GPIOK,
            GPIOD,
            GPIOE,
            GPIOF,
            GPIOH,
            GPIOI,
            I2C4,
            #[cfg(feature = "backlight_pwm")]
            TIM8,
            DSIHOST: dsi,
            FMC: _fmc,
            LTDC: ltdc,
            #[cfg(feature = "dma2d")]
            DMA2D,
            GPIOC,
            #[cfg(feature = "qspi_flash")]
            GPIOB,
            #[cfg(feature = "qspi_flash")]
            QUADSPI,
            #[cfg(feature = "sd_storage")]
            SDMMC1,
            ..
        } = dp;
        // Configure SMPS supply + VOS1 via HAL (requires `stm32h7xx-hal` feature `smps`).
        let pwr = PWR.constrain();
        let vos = pwr.smps().vos1().freeze();
        use stm32h7xx_hal::rcc::{PllConfigStrategy, ResetEnable};
        let rcc = RCC.constrain();
        let mut syscfg = SYSCFG;
        // HAL RCC: derive SYSCLK and LTDC pixel clock (via PLL3R)
        // Assumes HSE=25 MHz on H747I-DISCO. Adjust if using HSI or a different crystal.
        let ccdr = rcc
            .use_hse(25.MHz())
            .sys_ck(400.MHz())
            .hclk(200.MHz())
            .pll1_strategy(PllConfigStrategy::Iterative)
            .pll2_r_ck(150.MHz())
            // Target ~33 MHz pixel clock for 800x480 panel bring-up
            .pll3_r_ck(32.MHz())
            .freeze(vos, &mut syscfg);
        // Enable display-related peripherals in D1 domain
        let _ = ccdr.peripheral.LTDC.enable();
        let _ = ccdr.peripheral.DMA2D.enable();
        let _ = ccdr.peripheral.DSI.enable();
        let _ = ccdr.peripheral.FMC.enable();
        // HAL bug: pll3_r_ck() configures PLL3 dividers but never sets PLL3ON.
        // Without PLL3R running, LTDC register reads hang (no pixel clock domain).
        // Force PLL3ON and wait for PLL3RDY.
        unsafe {
            const RCC_CR: *mut u32 = 0x5802_4400u32 as *mut u32;
            RCC_CR.write_volatile(RCC_CR.read_volatile() | (1 << 28)); // PLL3ON
            while RCC_CR.read_volatile() & (1 << 29) == 0 {} // wait PLL3RDY
        }
        // Signal clocks ready to CM4 via shared mailbox flag
        #[allow(clippy::let_unit_value)]
        {
            // Safe to call; function is a no-op in unified builds
            let _ = bsp_pac::signal_clocks_ready();
        }
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0005u32); } // pre-gpio-split
        let gpioj = GPIOJ.split(ccdr.peripheral.GPIOJ);
        let gpiog = GPIOG.split(ccdr.peripheral.GPIOG);
        let gpiok = GPIOK.split(ccdr.peripheral.GPIOK);
        let gpiod = GPIOD.split(ccdr.peripheral.GPIOD);
        let gpioe = GPIOE.split(ccdr.peripheral.GPIOE);
        let gpiof = GPIOF.split(ccdr.peripheral.GPIOF);
        let gpioh = GPIOH.split(ccdr.peripheral.GPIOH);
        let gpioi = GPIOI.split(ccdr.peripheral.GPIOI);
        let gpioc = GPIOC.split(ccdr.peripheral.GPIOC);
        #[cfg(feature = "qspi_flash")]
        let gpiob = GPIOB.split(ccdr.peripheral.GPIOB);
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0006u32); } // post-gpio-split
        // Panel reset via HAL + adapter to embedded-hal 1.0 OutputPin
        struct HalResetPin<P>(P);
        impl<P> embedded_hal::digital::ErrorType for HalResetPin<P> {
            type Error = Infallible;
        }
        impl<P: stm32h7xx_hal::hal::digital::v2::OutputPin<Error = Infallible>>
            embedded_hal::digital::OutputPin for HalResetPin<P>
        {
            fn set_high(&mut self) -> Result<(), Self::Error> {
                let _ = self.0.set_high();
                Ok(())
            }
            fn set_low(&mut self) -> Result<(), Self::Error> {
                let _ = self.0.set_low();
                Ok(())
            }
        }
        // Configure FMC SDRAM pin mux (AF12 + VeryHigh speed)
        use stm32h7xx_hal::gpio::Speed;
        macro_rules! af12_high {
            ($pin:expr) => {{
                let mut pin = $pin.into_alternate::<12>();
                pin.set_speed(Speed::VeryHigh);
            }};
        }
        af12_high!(gpiof.pf0);
        af12_high!(gpiof.pf1);
        af12_high!(gpiof.pf2);
        af12_high!(gpiof.pf3);
        af12_high!(gpiof.pf4);
        af12_high!(gpiof.pf5);
        af12_high!(gpiof.pf12);
        af12_high!(gpiof.pf13);
        af12_high!(gpiof.pf14);
        af12_high!(gpiof.pf15);
        af12_high!(gpiog.pg0);
        af12_high!(gpiog.pg1);
        af12_high!(gpiog.pg2);
        af12_high!(gpiog.pg4);
        af12_high!(gpiof.pf11);
        af12_high!(gpiog.pg15);
        af12_high!(gpioh.ph5);
        af12_high!(gpiog.pg8);
        af12_high!(gpioh.ph6);
        af12_high!(gpioh.ph7);
        af12_high!(gpioe.pe0);
        af12_high!(gpioe.pe1);
        af12_high!(gpioi.pi4);
        af12_high!(gpioi.pi5);
        af12_high!(gpiod.pd14);
        af12_high!(gpiod.pd15);
        af12_high!(gpiod.pd0);
        af12_high!(gpiod.pd1);
        af12_high!(gpioe.pe7);
        af12_high!(gpioe.pe8);
        af12_high!(gpioe.pe9);
        af12_high!(gpioe.pe10);
        af12_high!(gpioe.pe11);
        af12_high!(gpioe.pe12);
        af12_high!(gpioe.pe13);
        af12_high!(gpioe.pe14);
        af12_high!(gpioe.pe15);
        af12_high!(gpiod.pd8);
        af12_high!(gpiod.pd9);
        af12_high!(gpiod.pd10);
        af12_high!(gpioh.ph8);
        af12_high!(gpioh.ph9);
        af12_high!(gpioh.ph10);
        af12_high!(gpioh.ph11);
        af12_high!(gpioh.ph12);
        af12_high!(gpioh.ph13);
        af12_high!(gpioh.ph14);
        af12_high!(gpioh.ph15);
        af12_high!(gpioi.pi0);
        af12_high!(gpioi.pi1);
        af12_high!(gpioi.pi2);
        af12_high!(gpioi.pi3);
        af12_high!(gpioi.pi6);
        af12_high!(gpioi.pi7);
        af12_high!(gpioi.pi9);
        af12_high!(gpioi.pi10);
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0007u32); } // post-FMC-pins

        // ── QSPI flash init (MT25TL01G Bank 1) ──────────────────────────
        #[cfg(feature = "qspi_flash")]
        let qspi_flash = {
            use rlvgl::platform::Mt25tlFlash;
            use stm32h7xx_hal::xspi;

            // Errata 2.8.5: Select PLL2R (150 MHz) as QSPI kernel clock
            // D1CCIPR QSPISEL bits [5:4]: 00=HCLK, 01=PLL1Q, 10=PLL2R, 11=PER
            unsafe {
                let d1ccipr = 0x5802_4C18u32 as *mut u32;
                let val = d1ccipr.read_volatile();
                d1ccipr.write_volatile((val & !(0b11 << 4)) | (0b10 << 4));
            }

            // QSPI Bank 1 GPIO pins (AF numbers verified against DS12930 Table 9)
            let qspi_clk = gpiob.pb2.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io0 = gpiod.pd11.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io1 = gpiof.pf9.into_alternate::<10>().speed(Speed::VeryHigh);
            let qspi_io2 = gpiof.pf7.into_alternate::<9>().speed(Speed::VeryHigh);
            let qspi_io3 = gpiof.pf6.into_alternate::<9>().speed(Speed::VeryHigh);
            // NCS on PG6 (AF10) is managed by the HAL internally

            let qspi = QUADSPI.bank1(
                (qspi_clk, qspi_io0, qspi_io1, qspi_io2, qspi_io3),
                xspi::Config::new(50.MHz()).fifo_threshold(4),
                &ccdr.clocks,
                ccdr.peripheral.QSPI,
            );

            let mut flash = Mt25tlFlash::new(qspi);

            // Read and verify JEDEC ID
            match flash.read_id() {
                Ok(id) => {
                    unsafe {
                        // Breadcrumb: write JEDEC ID to D3 SRAM for debug
                        let bc = 0x3800_0320u32 as *mut u32;
                        bc.write_volatile(
                            0x0F00_0000
                                | (id[0] as u32) << 16
                                | (id[1] as u32) << 8
                                | id[2] as u32,
                        );
                    }
                }
                Err(_) => {
                    unsafe {
                        (0x3800_0320u32 as *mut u32).write_volatile(0xDEAD_DEAD);
                    }
                }
            }
            flash
        };
        #[cfg(feature = "qspi_flash")]
        let _ = &qspi_flash; // suppress unused warning when not consumed yet

        // Panel reset GPIO on PG3 (LCD_RESET)
        let mut panel_reset_hal = gpiog.pg3.into_push_pull_output();
        let _ = panel_reset_hal.set_low();
        cortex_m::asm::delay(10_000_00);
        let _ = panel_reset_hal.set_high();
        // Backlight via HAL PWM (feature) or GPIO fallback
        #[cfg(feature = "backlight_pwm")]
        let backlight = {
            use stm32h7xx_hal::hal::PwmPin as HalPwmPin02;
            // Configure PJ6 as TIM8_CH2 with AF3 and start PWM at ~10kHz
            let pj6_ch2 = gpioj.pj6.into_alternate::<3>();
            let ch = TIM8.pwm(pj6_ch2, 10.kHz(), ccdr.peripheral.TIM8, &ccdr.clocks);
            // Adapter from HAL 0.2 PwmPin to embedded-hal 1.0 SetDutyCycle
            struct TimBacklight<T: HalPwmPin02<Duty = u16>>(T);
            impl<T: HalPwmPin02<Duty = u16>> PwmError for TimBacklight<T> {
                type Error = Infallible;
            }
            impl<T: HalPwmPin02<Duty = u16>> SetDutyCycle for TimBacklight<T> {
                fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                    let max = self.0.get_max_duty();
                    let d = if duty == 0 { 0 } else { max.min(duty) };
                    self.0.set_duty(d);
                    if d == 0 {
                        self.0.disable();
                    } else {
                        self.0.enable();
                    }
                    Ok(())
                }
                fn max_duty_cycle(&self) -> u16 {
                    self.0.get_max_duty()
                }
            }
            TimBacklight(ch)
        };
        #[cfg(not(feature = "backlight_pwm"))]
        let backlight = {
            let bl_pin = gpioj.pj6.into_push_pull_output();
            HalGpioBacklight(bl_pin)
        };
        let blitter = CpuBlitter;
        // Configure a SysTick timer to flip buffers at ~60 Hz
        use cortex_m::peripheral::syst::SystClkSource;
        cp.SYST.set_clock_source(SystClkSource::Core);
        let sys_hz = ccdr.clocks.sys_ck().to_Hz();
        let flip_hz = 6u32; // loose 6 Hz flip for bring-up
        let reload = (sys_hz / flip_hz).saturating_sub(1);
        cp.SYST.set_reload(reload);
        cp.SYST.clear_current();
        cp.SYST.enable_counter();
        // ── USART1 VCP init (PA9=TX AF7, 115200 8N1) ──────────────────────
        // Addresses from C HAL path (RCC C1 domain registers at 0x5802_44xx)
        unsafe {
            // Enable GPIOA clock (AHB4ENR at RCC+0xE0)
            let ahb4 = 0x5802_44E0u32 as *mut u32; // global AHB4ENR
            ahb4.write_volatile(ahb4.read_volatile() | (1 << 0));
            let _ = (ahb4 as *const u32).read_volatile();
            // PA9 = AF7: AFRH bits 7:4 = 7, MODER bits 19:18 = 10 (AF)
            let gpioa = 0x5802_0000u32;
            let afrh = (gpioa + 0x24) as *mut u32;
            afrh.write_volatile((afrh.read_volatile() & !(0xFu32 << 4)) | (7u32 << 4));
            let moder = gpioa as *mut u32;
            moder.write_volatile((moder.read_volatile() & !(3u32 << 18)) | (2u32 << 18));
            // Enable USART1 clock (C1_APB2ENR bit 4)
            let apb2 = 0x5802_44F0u32 as *mut u32;
            apb2.write_volatile(apb2.read_volatile() | (1 << 4));
            let _ = (apb2 as *const u32).read_volatile();
            // USART1 config: BRR=868 (100 MHz / 115200), TE+UE
            let usart1 = 0x4001_1000u32;
            ((usart1 + 0x0C) as *mut u32).write_volatile(868); // BRR
            ((usart1 + 0x00) as *mut u32).write_volatile((1 << 3) | (1 << 0)); // CR1: TE + UE
        }

        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0010u32); } // pre-display::new
        let mut display = Stm32h747iDiscoDisplay::new(
            blitter,
            backlight,
            HalResetPin(panel_reset_hal),
            ltdc,
            dsi,
            #[cfg(feature = "dma2d")]
            DMA2D,
            #[cfg(feature = "splash")]
            Some(SPLASH_RLE),
        );
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0011u32); } // post-display::new
        // No splash delay — splash is the desktop background.
        // Optional: SDRAM RAM test (feature-gated). Writes a few patterns per MB
        // and prints progress via semihosting if enabled.
        #[cfg(feature = "sdram_ramtest")]
        {
            #[cfg(feature = "semihosting")]
            fn logln(args: core::fmt::Arguments) {
                use core::fmt::Write;
                if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                    let _ = writeln!(out, "{}", args);
                }
            }
            #[cfg(not(feature = "semihosting"))]
            fn logln(_args: core::fmt::Arguments) {}
            macro_rules! log {
                ($($arg:tt)*) => {
                    logln(format_args!($($arg)*));
                }
            }
            unsafe {
                const BASE: usize = 0xC000_0000;
                const SIZE_MB: usize = 32; // H747I-DISCO typical SDRAM size
                // stride controls test density per MB (words touched per MB)
                const STRIDE: usize = 256; // higher = denser test, slower
                for mb in 0..SIZE_MB {
                    let mb_base = BASE + (mb << 20);
                    let mut errs = 0usize;

                    // Pattern 1: solid zeros
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8);
                        p.write_volatile(0x0000_0000);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8);
                        if p.read_volatile() != 0x0000_0000 {
                            errs += 1;
                        }
                    }

                    // Pattern 2: solid ones
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8 + 1);
                        p.write_volatile(0xFFFF_FFFF);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8 + 1);
                        if p.read_volatile() != 0xFFFF_FFFF {
                            errs += 1;
                        }
                    }

                    // Pattern 3: address-based
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8 + 2);
                        let v = (mb_base as u32).wrapping_add((i as u32) << 4);
                        p.write_volatile(v);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8 + 2);
                        let v = (mb_base as u32).wrapping_add((i as u32) << 4);
                        if p.read_volatile() != v {
                            errs += 1;
                        }
                    }

                    // Pattern 4: checkerboard
                    for i in 0..STRIDE {
                        let p0 = (mb_base as *mut u32).add(i * 8 + 3);
                        let p1 = (mb_base as *mut u32).add(i * 8 + 4);
                        p0.write_volatile(0xAAAA_AAAA);
                        p1.write_volatile(0x5555_5555);
                    }
                    for i in 0..STRIDE {
                        let p0 = (mb_base as *const u32).add(i * 8 + 3);
                        let p1 = (mb_base as *const u32).add(i * 8 + 4);
                        if p0.read_volatile() != 0xAAAA_AAAA {
                            errs += 1;
                        }
                        if p1.read_volatile() != 0x5555_5555 {
                            errs += 1;
                        }
                    }

                    // Pattern 5: pseudo-random (xorshift)
                    let mut seed: u32 = 0xC0FF_EE11 ^ (mb as u32 * 0x9E37_79B9);
                    for i in 0..STRIDE {
                        // xorshift32
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let p = (mb_base as *mut u32).add(i * 8 + 5);
                        p.write_volatile(seed);
                    }
                    let mut seed2: u32 = 0xC0FF_EE11 ^ (mb as u32 * 0x9E37_79B9);
                    for i in 0..STRIDE {
                        seed2 ^= seed2 << 13;
                        seed2 ^= seed2 >> 17;
                        seed2 ^= seed2 << 5;
                        let p = (mb_base as *const u32).add(i * 8 + 5);
                        if p.read_volatile() != seed2 {
                            errs += 1;
                        }
                    }

                    log!("SDRAM test: MB {} -> {} errors\n", mb, errs);
                }
            }
        }
        // Main loop: handle IPC commands (from CM4) and real inputs
        ipc::init();

        // ── I2C4 for FT5336 touch controller (PD12=SCL, PD13=SDA, AF4 OD) ──
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0020u32); } // pre-I2C4
        let _scl = gpiod.pd12.into_alternate_open_drain::<4>();
        let _sda = gpiod.pd13.into_alternate_open_drain::<4>();
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0021u32); } // post-I2C4-pins
        let i2c4 = stm32h7xx_hal::i2c::I2c::i2c4(
            I2C4, 400.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks,
        );
        unsafe { (0x3800_0300u32 as *mut u32).write_volatile(0xA11C_0022u32); } // post-I2C4-init
        // Wrap for embedded-hal 1.0 (stm32h7xx-hal I2c implements eh 0.2 I2C)
        struct HalI2c<I>(I);
        impl<I> embedded_hal::i2c::ErrorType for HalI2c<I> {
            type Error = embedded_hal::i2c::ErrorKind;
        }
        impl<I> EhI2c<SevenBitAddress> for HalI2c<I>
        where
            I: stm32h7xx_hal::hal::blocking::i2c::WriteRead
                + stm32h7xx_hal::hal::blocking::i2c::Write
                + stm32h7xx_hal::hal::blocking::i2c::Read,
        {
            fn read(&mut self, addr: SevenBitAddress, buf: &mut [u8]) -> Result<(), Self::Error> {
                self.0.read(addr, buf).map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn write(&mut self, addr: SevenBitAddress, bytes: &[u8]) -> Result<(), Self::Error> {
                self.0.write(addr, bytes).map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn write_read(&mut self, addr: SevenBitAddress, bytes: &[u8], buf: &mut [u8]) -> Result<(), Self::Error> {
                self.0.write_read(addr, bytes, buf).map_err(|_| embedded_hal::i2c::ErrorKind::Other)
            }
            fn transaction(&mut self, _addr: SevenBitAddress, _ops: &mut [Operation<'_>]) -> Result<(), Self::Error> {
                Err(embedded_hal::i2c::ErrorKind::Other)
            }
        }
        // ── Audio codec init (before touch claims I2C4) ──
        #[cfg(feature = "audio")]
        let i2c4 = {
            use rlvgl::platform::{Sai1Audio, Wm8994};

            // SAI1 peripheral clock + kernel clock source = PLL2_P
            let sai = Sai1Audio::new();
            sai.enable_clock(1); // 1 = PLL2_P

            // SAI1 GPIO pins (AF6, VeryHigh speed)
            let _sai1_mclk = gpiog.pg7.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sck  = gpioe.pe5.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_fs   = gpioe.pe4.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sd_a = gpioe.pe6.into_alternate::<6>().speed(Speed::VeryHigh);
            let _sai1_sd_b = gpioe.pe3.into_alternate::<6>().speed(Speed::VeryHigh);

            // Configure SAI1 sub-block A as I2S master TX
            // MCKDIV=0 means /1; the WM8994 FLL handles exact audio frequency
            sai.configure_tx(0);

            // Init WM8994 codec over I2C4 (temporary ownership, then release)
            let codec_i2c = HalI2c(i2c4);
            let mut codec = Wm8994::new(codec_i2c);
            // init_playback performs a software reset, verifies chip ID,
            // configures FLL for exact audio clocking, and sets up DAC routing.
            // PLL2_P provides the SAI1 kernel clock; MCKDIV=0 means MCLK = kernel_ck.
            // The WM8994 FLL locks to whatever MCLK we provide.
            let _ = codec.init_playback(
                48_000,
                150_000_000, // approximate MCLK from PLL2_P
                rlvgl::platform::wm8994::OutputDevice::Headphone,
            );

            // Enable SAI1 TX — codec is now receiving I2S frames
            sai.enable_tx();

            // Release I2C4 back so touch can use it
            codec.release().0
        };
        #[cfg(not(feature = "audio"))]
        let i2c4 = i2c4;

        let touch_i2c = HalI2c(i2c4);
        let touch_int = HalInputPin(gpiok.pk7.into_floating_input());
        let mut input = Stm32h747iDiscoInput::new_with_int(
            touch_i2c, touch_int, display.dimensions().0 as u16,
        );

        // ── Real button: PC13 wakeup button (active-low, external pull-up) ──
        let button = HalInputPin(gpioc.pc13.into_floating_input());
        let mut button_input = ButtonInput::new(button);

        // ── Joystick: PK2=SEL, PK3=DOWN, PK4=LEFT, PK5=RIGHT, PK6=UP ──
        // Use pull-up inputs to prevent floating pin noise on boot
        let mut joystick = JoystickInput::new(
            HalInputPin(gpiok.pk2.into_pull_up_input()),
            HalInputPin(gpiok.pk3.into_pull_up_input()),
            HalInputPin(gpiok.pk4.into_pull_up_input()),
            HalInputPin(gpiok.pk5.into_pull_up_input()),
            HalInputPin(gpiok.pk6.into_pull_up_input()),
        );

        // Build a minimal root widget tree. The demo app tree has a white
        // root container that paints over the SDRAM splash. We use an invisible
        // root that produces no pixels — the splash survives in the framebuffer
        // and the EventWindow draws on top when visible.
        use rlvgl::core::WidgetNode;

        /// Root widget that draws nothing (splash stays in the framebuffer).
        struct InvisibleRoot;
        impl rlvgl::core::widget::Widget for InvisibleRoot {
            fn bounds(&self) -> rlvgl::core::widget::Rect {
                // Landscape widget space: 800 wide × 480 tall
                rlvgl::core::widget::Rect { x: 0, y: 0, width: 800, height: 480 }
            }
            fn draw(&self, _renderer: &mut dyn rlvgl::core::renderer::Renderer) {}
            fn handle_event(&mut self, _event: &Event) -> bool { false }
        }

        let root = Rc::new(RefCell::new(WidgetNode {
            widget: Rc::new(RefCell::new(InvisibleRoot)),
            children: alloc::vec![],
        }));
        #[cfg(feature = "sd_storage")]
        {
            use alloc::rc::Rc;
            use core::cell::RefCell;
            let pending: Rc<RefCell<alloc::vec::Vec<WidgetNode>>> =
                Rc::new(RefCell::new(alloc::vec::Vec::new()));
            let to_remove: Rc<RefCell<alloc::vec::Vec<Rc<RefCell<dyn rlvgl::core::widget::Widget>>>>> =
                Rc::new(RefCell::new(alloc::vec::Vec::new()));
            use rlvgl::core::widget::Rect;
            use rlvgl::widgets::label::Label;
            use rlvgl_i18n::t;
            use stm32h7xx_hal::gpio::Alternate;

            // Card detect: PI8 is active-low (low = card inserted)
            let sd_detect = gpioi.pi8.into_pull_up_input();
            let card_present = sd_detect.is_low();

            // SDMMC1 pins: PC12=CK, PD2=CMD, PC8..PC11=D0..D3 (AF12)
            use stm32h7xx_hal::sdmmc::SdmmcExt;
            let ck: stm32h7xx_hal::gpio::Pin<'C', 12, Alternate<12>> = gpioc.pc12.into_alternate();
            let cmd: stm32h7xx_hal::gpio::Pin<'D', 2, Alternate<12>> = gpiod.pd2.into_alternate();
            let d0: stm32h7xx_hal::gpio::Pin<'C', 8, Alternate<12>> = gpioc.pc8.into_alternate();
            let d1: stm32h7xx_hal::gpio::Pin<'C', 9, Alternate<12>> = gpioc.pc9.into_alternate();
            let d2: stm32h7xx_hal::gpio::Pin<'C', 10, Alternate<12>> = gpioc.pc10.into_alternate();
            let d3: stm32h7xx_hal::gpio::Pin<'C', 11, Alternate<12>> = gpioc.pc11.into_alternate();
            let sdmmc = SDMMC1.sdmmc(
                (ck, cmd, d0, d1, d2, d3),
                ccdr.peripheral.SDMMC1,
                &ccdr.clocks,
            );
            let bd = SdMmcBlockDev::new(sdmmc);

            let sd_msg: &str = if !card_present {
                t!("hw.sd_no_card")
            } else {
                use rlvgl::platform::sd_emmc_adapter as sda;
                let volume_mgr = embedded_sdmmc::VolumeManager::new(bd, sda::DummyTimeSource);
                match volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)) {
                    Ok(volume) => {
                        match volume.open_root_dir() {
                            Ok(root_dir) => {
                                let mut count = 0u32;
                                root_dir.iterate_dir(|entry| {
                                    count += 1;
                                    let _ = entry;
                                }).ok();
                                if count > 0 { t!("hw.sd_mounted_ok") } else { t!("hw.sd_empty") }
                            }
                            Err(_) => t!("hw.sd_root_dir_failed"),
                        }
                    }
                    Err(_) => t!("hw.sd_mount_failed"),
                }
            };

            let label = Label::new(
                sd_msg,
                Rect { x: 10, y: 70, width: 260, height: 16 },
            );
            let node = rlvgl::core::WidgetNode {
                widget: Rc::new(RefCell::new(label)),
                children: alloc::vec![],
            };
            pending.borrow_mut().push(node);
            rlvgl_app_demo::flush_pending(&root, &pending, &to_remove);
        }

        // ── USART1 serial helper (115200 8N1 already configured above) ──
        fn serial_puts(s: &str) {
            const USART1_ISR: *const u32 = 0x4001_101C as *const u32;
            const USART1_TDR: *mut u32 = 0x4001_1028 as *mut u32;
            for b in s.bytes() {
                unsafe {
                    while USART1_ISR.read_volatile() & (1 << 7) == 0 {} // TXE
                    USART1_TDR.write_volatile(b as u32);
                }
            }
        }

        // ── EventWindow widget (replaces direct-framebuffer toasts) ──────
        use alloc::rc::Rc;
        use core::cell::RefCell;
        use rlvgl::core::bitmap_font::FONT_6X10;
        use rlvgl::ui::EventWindowBuilder;
        use rlvgl::platform::blit::{BlitterRenderer, RotatedRenderer, Surface, PixelFmt};
        use rlvgl_i18n::t;

        let event_win = Rc::new(RefCell::new(
            EventWindowBuilder::new(&FONT_6X10).build(),
        ));

        root.borrow_mut().children.push(rlvgl::core::WidgetNode {
            widget: event_win.clone(),
            children: alloc::vec![],
        });

        let mut render_blitter = CpuBlitter;

        // ── Fix double-buffering ──────────────────────────────────────────
        // The sdram_alloc may have given both framebuffers the same address.
        // Force a second buffer at a known SDRAM offset and copy the front
        // buffer into it.
        let (w_fb, h_fb) = display.dimensions();

        unsafe { (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0001); }
        // ── RLE decode helper ─────────────────────────────────────────────
        fn decode_rle(blob: &[u8]) -> (u32, u32, alloc::vec::Vec<u8>) {
            let (w, h, pal_bytes, stream) =
                rlvgl_decomp::parse_rle_blob(blob).expect("RLE parse");
            let pal_count = pal_bytes.len() / 2;
            let mut palette = alloc::vec![0u16; pal_count];
            for i in 0..pal_count {
                palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
            }
            let rgba = rlvgl_decomp::decode_rgba(
                w as usize, h as usize, &palette, stream,
            ).expect("RLE decode");
            (w as u32, h as u32, rgba)
        }

        fn decode_rle_colors(blob: &[u8]) -> (u32, u32, alloc::vec::Vec<rlvgl::core::widget::Color>) {
            let (w, h, rgba) = decode_rle(blob);
            let colors = rgba.chunks_exact(4)
                .map(|c| rlvgl::core::widget::Color(c[0], c[1], c[2], c[3]))
                .collect();
            (w, h, colors)
        }

        // ── Config menu (created first so icon strip can reference it) ────
        let config_menu = {
            use crate::config_menu::ConfigMenu;
            use rlvgl::core::widget::Rect;
            use rlvgl::core::packed_font::PackedFont;

            static CLOSE_RLE: &[u8] = include_bytes!("../assets/icons/close28.rle");
            let (cw, ch, close_rgba) = decode_rle(CLOSE_RLE);

            static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-24.bin");
            static UI_FONT: PackedFont = PackedFont {
                height: 24,
                glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                data: FONT_DATA,
            };

            let event_win_for_cfg = event_win.clone();
            Rc::new(RefCell::new(
                ConfigMenu::new(
                    Rect { x: 730, y: 17, width: 60, height: 60 },
                    rlvgl_i18n::locale() as u8,
                    &UI_FONT,
                )
                .close_icon(&close_rgba, cw, ch)
                .on_change(|idx| {
                    let locale = rlvgl_i18n::locale_from_u8(idx);
                    rlvgl_i18n::set_locale(locale);
                })
                .on_preview(|idx| {
                    let locale = rlvgl_i18n::locale_from_u8(idx);
                    rlvgl_i18n::set_locale(locale);
                })
                .on_event_viewer_change(move |enabled| {
                    event_win_for_cfg.borrow_mut().set_enabled(enabled);
                }),
            ))
        };

        // ── Icon strip (right edge, 6 slots) ────────────────────────────────
        {
            use crate::icon_strip::{IconStrip, IconSlot};

            let mut strip = IconStrip::new(
                730, // x position
                60,  // icon size
                17,  // margin top
                17,  // gap between icons
            );

            let icons: [(&[u8], bool); 6] = [
                (include_bytes!("../assets/icons/settings.rle"), true),
                (include_bytes!("../assets/icons/file.rle"), true),
                (include_bytes!("../assets/icons/audio.rle"), false),
                (include_bytes!("../assets/icons/video.rle"), false),
                (include_bytes!("../assets/icons/camera.rle"), false),
                (include_bytes!("../assets/icons/info.rle"), true),
            ];

            for (i, (rle, enabled)) in icons.iter().enumerate() {
                strip.set_slot(i, IconSlot {
                    rle,
                    enabled: *enabled,
                    on_tap: None,
                });
            }

            // Settings tap (slot 0) → toggle config menu
            let cm = config_menu.clone();
            strip.slots_mut()[0].as_mut().unwrap().on_tap = Some(alloc::boxed::Box::new(move |_| {
                cm.borrow_mut().toggle_visible();
            }));

            root.borrow_mut().children.push(rlvgl::core::WidgetNode {
                widget: Rc::new(RefCell::new(strip)),
                children: alloc::vec![],
            });
        }

        // Config menu draws on top of everything (last child)
        root.borrow_mut().children.push(rlvgl::core::WidgetNode {
            widget: config_menu.clone(),
            children: alloc::vec![],
        });

        unsafe { (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0003); }
        let fb_bytes = (w_fb * h_fb * 4) as usize;
        const FB2_ADDR: u32 = 0xD018_0000; // 1.5MB into 32MB SDRAM
        if display.back_buffer_addr() == display.front_buffer_addr() {
            serial_puts("FIX: double-buffer was single — setting FB2\r\n");
            unsafe {
                core::ptr::copy_nonoverlapping(
                    display.front_buffer_addr() as *const u8,
                    FB2_ADDR as *mut u8,
                    fb_bytes,
                );
                cortex_m::asm::dsb();
            }
            display.set_back_buffer(FB2_ADDR);
        }

        // ── Desktop background ────────────────────────────────────────────
        // When the `desktop` feature is enabled, decode the desktop image
        // into both framebuffers.  This is independent of `splash` — you
        // can have a splash boot animation, a desktop background, both
        // (with the same or different assets), or neither.
        #[cfg(feature = "desktop")]
        {
            let (dw, dh, pal_bytes, stream) =
                rlvgl_decomp::parse_rle_blob(DESKTOP_RLE).expect("desktop RLE parse");
            let pal_count = pal_bytes.len() / 2;
            let mut palette = [0u16; 192];
            for i in 0..pal_count {
                palette[i] = u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
            }
            let fb0 = unsafe {
                core::slice::from_raw_parts_mut(
                    display.front_buffer_addr() as *mut u8, fb_bytes,
                )
            };
            let _ = rlvgl_decomp::decode_argb_into(
                dw as usize, dh as usize, &palette[..pal_count], stream, fb0,
            );
            let fb1 = unsafe {
                core::slice::from_raw_parts_mut(
                    display.back_buffer_addr() as *mut u8, fb_bytes,
                )
            };
            let _ = rlvgl_decomp::decode_argb_into(
                dw as usize, dh as usize, &palette[..pal_count], stream, fb1,
            );
            cortex_m::asm::dsb();
            serial_puts("DESKTOP: decoded into both FBs\r\n");
        }

        // Telemetry: write both fb addresses
        unsafe {
            (0x3800_0620u32 as *mut u32).write_volatile(display.front_buffer_addr());
            (0x3800_0624u32 as *mut u32).write_volatile(display.back_buffer_addr());
        }

        // Save a pristine copy of the desktop framebuffer so we can restore
        // pixels under the EventWindow when it hides (the front buffer gets
        // EventWindow pixels painted on it, so we can't copy from there).
        // Place pristine copy at 0xD030_0000 (after the two 1.5MB framebuffers).
        const DESKTOP_PRISTINE: u32 = 0xD030_0000;
        // When desktop feature is off, the pristine copy is still taken so
        // that the solid-black background can be restored correctly.
        let pristine_ref = display.back_buffer_addr();
        unsafe {
            core::ptr::copy_nonoverlapping(
                pristine_ref as *const u8,
                DESKTOP_PRISTINE as *mut u8,
                fb_bytes,
            );
            cortex_m::asm::dsb();
        }

        // D3 breadcrumb: entering main loop
        unsafe { (0x3800_0600u32 as *mut u32).write_volatile(0x1C1C_0001); }
        unsafe { (0x3800_0664u32 as *mut u32).write_volatile(0xA0A0_0004); }
        serial_puts("rlvgl: input proof loop started\r\n");

        // No boot discard — splash delay removed, pins are stable by now.
        let _btn_discard: u32 = 0;

        // ── Tap gesture recognizer ────────────────────────────────────────
        use rlvgl::platform::gesture::TapRecognizer;
        let mut tap = TapRecognizer::new(2); // 2 ticks (~330ms at 6Hz)

        // ── Event telemetry ring buffer ──────────────────────────────────
        // 16-entry ring at D3 SRAM 0x3800_0700, each entry = 4 words:
        //   [0] tick_count  [1] event_code  [2] x  [3] y
        // Event codes: 0x01=PointerDown, 0x02=PointerUp, 0x03=PressDown,
        //              0x04=PressRelease, 0x10=GestureProcess, 0x11=GestureTick
        const TELEM_BASE: u32 = 0x3800_0700;
        const TELEM_ENTRIES: u32 = 16;
        const TELEM_ENTRY_WORDS: u32 = 4;
        // Ring index at 0x3800_06F0, dump tick counter at 0x3800_06F4
        const TELEM_IDX_ADDR: u32 = 0x3800_06F0;
        const TELEM_DUMP_TICK: u32 = 0x3800_06F4;

        unsafe {
            (TELEM_IDX_ADDR as *mut u32).write_volatile(0);
            (TELEM_DUMP_TICK as *mut u32).write_volatile(0);
        }

        fn telem_log(tick: u32, code: u32, x: i32, y: i32) {
            unsafe {
                let idx = (TELEM_IDX_ADDR as *const u32).read_volatile();
                let slot = idx % TELEM_ENTRIES;
                let base = TELEM_BASE + slot * TELEM_ENTRY_WORDS * 4;
                (base as *mut u32).write_volatile(tick);
                ((base + 4) as *mut u32).write_volatile(code);
                ((base + 8) as *mut u32).write_volatile(x as u32);
                ((base + 12) as *mut u32).write_volatile(y as u32);
                (TELEM_IDX_ADDR as *mut u32).write_volatile(idx + 1);
            }
        }

        // Double-buffer sync: render for 2 frames after any visual change
        // so both ping-pong buffers match.
        let mut dirty_frames: u8 = 4; // force initial render
        let mut was_visible = false;
        let mut render_count: u32 = 0;
        let mut tick_count: u32 = 0;

        // Save-under compositor: saves fb pixels when overlays open,
        // restores when they close.
        use rlvgl::platform::compositor::Compositor;
        let mut compositor = Compositor::new(w_fb, h_fb, DESKTOP_PRISTINE);

        // Event counter written to D3 SRAM for probe-rs inspection
        let mut evt_count: u32 = 0;

        // ── Star Wars opening crawl ─────────────────────────────────────
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let mut star_crawl = {
            use rlvgl::core::packed_font::PackedFont;

            static BOLD_FONT_DATA: &[u8] =
                include_bytes!("../assets/fonts/DejaVuSans-Bold-32.bin");
            static BOLD_FONT: PackedFont = PackedFont {
                height: 32,
                glyphs: &crate::fonts::DEJAVU_SANS_BOLD_32_GLYPHS,
                data: BOLD_FONT_DATA,
            };

            static CRAWL_LINES: &[&str] = &[
                "RLVGL",
                "",
                "Episode I",
                "THE EMBEDDED MENACE",
                "",
                "It is a period of civil war.",
                "Rebel firmware engineers,",
                "striking from a hidden lab,",
                "have won their first victory",
                "against the evil Proprietary",
                "RTOS Empire.",
                "",
                "During the battle, Rebel",
                "spies managed to steal",
                "secret plans to the Empire's",
                "ultimate weapon, the",
                "DEATH BLOB, a binary",
                "firmware image with enough",
                "power to destroy an entire",
                "product line.",
                "",
                "Pursued by the Empire's",
                "sinister vendor lock-in,",
                "Princess Ferris races home",
                "aboard her starship,",
                "custodian of the stolen",
                "plans that can save her",
                "people and restore freedom",
                "to the galaxy....",
            ];

            star_crawl::StarCrawl::new(&BOLD_FONT, CRAWL_LINES)
        };

        // Info icon tap flag — set by PressRelease in icon slot 5 bounds.
        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
        let mut crawl_toggle_pending = false;

        loop {
            // Loop heartbeat
            unsafe {
                let prev = (0x3800_0660u32 as *const u32).read_volatile();
                (0x3800_0660u32 as *mut u32).write_volatile(prev.wrapping_add(1));
            }
            // Handle CM4 commands
            if let Some(cmd) = ipc::cmd_pop() {
                if cmd.kind == ipc::CmdKind::SetBacklight as u32 {
                    let duty = (cmd.a & 0xFFFF) as u16;
                    let level = if duty < 512 { 0 } else { u16::MAX };
                    display.set_brightness(level);
                }
            }

            // ── Poll touch ──
            // Poll touch — coords already in landscape widget space
            // (portrait→landscape transform done inside InputDevice::poll)
            if let Some(evt) = input.poll() {
                // Log to telemetry + event window
                match &evt {
                    Event::PointerDown { x, y } => {
                        telem_log(tick_count, 0x01, *x, *y);
                        event_win.borrow_mut().push_event(
                            t!("hw.touch", x = *x, y = *y),
                        );
                        dirty_frames = dirty_frames.max(2);
                        evt_count += 1;
                    }
                    Event::PointerUp { x, y } => {
                        telem_log(tick_count, 0x02, *x, *y);
                        evt_count += 1;
                    }
                    _ => {}
                }

                // Feed to gesture recognizer → dispatch gestures to widgets
                if let Some(gesture) = tap.process(&evt) {
                    match &gesture {
                        Event::PressDown { x, y } => telem_log(tick_count, 0x03, *x, *y),
                        Event::PressRelease { x, y } => {
                            telem_log(tick_count, 0x04, *x, *y);
                            // Info icon (slot 5): x=730, y=402, 60×60
                            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                            if *x >= 730 && *x < 790 && *y >= 402 && *y < 462 {
                                crawl_toggle_pending = true;
                            }
                        }
                        _ => {}
                    }
                    dirty_frames = 2;
                    root.borrow_mut().dispatch_event(&gesture);
                }
            }

            // ── Poll button (PC13 — the one with the pole) ──
            if let Some(evt) = button_input.poll() {
                unsafe {
                    let code: u32 = match &evt {
                        Event::KeyDown { .. } => 0x4200_0001,
                        Event::KeyUp { .. } => 0x4200_8000,
                        _ => 0x4200_FFFF,
                    };
                    (0x3800_0630u32 as *mut u32).write_volatile(code);
                }
                if matches!(evt, Event::KeyDown { .. }) {
                    serial_puts("BTN: PRESS\r\n");
                    event_win.borrow_mut().push_event(
                        alloc::string::String::from(t!("hw.btn_press")),
                    );
                    dirty_frames = 2;
                    evt_count += 1;
                }
                root.borrow_mut().dispatch_event(&evt);
            }

            // ── Poll joystick (PK2-PK6 — the flat pad) ──
            if let Some(evt) = joystick.poll() {
                unsafe {
                    let code: u32 = match &evt {
                        Event::KeyDown { key } => 0x4A00_0000 | match key {
                            Key::Enter => 1, Key::ArrowUp => 2, Key::ArrowDown => 3,
                            Key::ArrowLeft => 4, Key::ArrowRight => 5, _ => 0xFF,
                        },
                        Event::KeyUp { .. } => 0x4A00_8000,
                        _ => 0x4A00_FFFF,
                    };
                    (0x3800_0634u32 as *mut u32).write_volatile(code);
                }
                if let Event::KeyDown { ref key } = evt {
                    let label = match key {
                        Key::ArrowUp => t!("hw.joy_up"),
                        Key::ArrowDown => t!("hw.joy_down"),
                        Key::ArrowLeft => t!("hw.joy_left"),
                        Key::ArrowRight => t!("hw.joy_right"),
                        Key::Enter => t!("hw.joy_sel"),
                        _ => t!("hw.joy_unknown"),
                    };
                    serial_puts(label);
                    serial_puts("\r\n");
                    event_win.borrow_mut().push_event(
                        alloc::string::String::from(label),
                    );
                    dirty_frames = 2;
                    evt_count += 1;
                }
                root.borrow_mut().dispatch_event(&evt);
            }

            // ── SysTick: tick widgets, render, present ──
            if cp.SYST.has_wrapped() {
                // Advance gesture settle timer → may emit PressRelease
                if let Some(gesture) = tap.tick() {
                    if let Event::PressRelease { x, y } = &gesture {
                        telem_log(tick_count, 0x14, *x, *y);
                        // Log settled tap to EventWindow (PointerDown may have
                        // been missed for very fast taps)
                        event_win.borrow_mut().push_event(
                            t!("hw.touch", x = *x, y = *y),
                        );
                    }
                    dirty_frames = 4;
                    root.borrow_mut().dispatch_event(&gesture);
                }

                // Keep rendering while config menu is clearing stale pixels
                if config_menu.borrow().clear_active() {
                    dirty_frames = dirty_frames.max(2);
                }

                // Dispatch Tick to age EventWindow entries
                root.borrow_mut().dispatch_event(&Event::Tick);

                let vis = event_win.borrow().is_visible();
                let entry_count = event_win.borrow().entry_count();

                // Only render when something visually changed:
                // - visibility transition (show or hide)
                // - entry count changed (new event or expiry)
                // - dirty_frames > 0 (second buffer needs sync)
                // Track overlay visibility transitions — restore from
                // pristine desktop when overlays hide.
                use rlvgl::core::widget::Widget as _;
                if vis != was_visible {
                    dirty_frames = 4;
                    if !vis {
                        // EventWindow just hid — restore from pristine
                        compositor.mark_pristine_restore(event_win.borrow().bounds());
                    }
                    was_visible = vis;
                }
                let cm_vis = config_menu.borrow().is_visible();
                static mut CM_WAS_VISIBLE: bool = false;
                if cm_vis != unsafe { CM_WAS_VISIBLE } {
                    dirty_frames = 4;
                    if !cm_vis {
                        // ConfigMenu just hid — restore panel region from pristine
                        if let Some(panel) = config_menu.borrow().last_panel_bounds() {
                            compositor.mark_pristine_restore(panel);
                        }
                        // Also restore gear area (it was part of the visible bounds)
                        compositor.mark_pristine_restore(config_menu.borrow().bounds());
                    }
                    unsafe { CM_WAS_VISIBLE = cm_vis; }
                }
                // Detect entry count change (expiry or new push)
                static mut LAST_ENTRY_COUNT: usize = 0;
                let ec = entry_count;
                if ec != unsafe { LAST_ENTRY_COUNT } {
                    unsafe { LAST_ENTRY_COUNT = ec; }
                    dirty_frames = 2;
                }
                // Keep rendering while restores are pending
                if compositor.has_pending() {
                    dirty_frames = dirty_frames.max(2);
                }

                // ── Star crawl toggle + render override ─────────────────
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                if crawl_toggle_pending {
                    crawl_toggle_pending = false;
                    if star_crawl.is_active() {
                        star_crawl.deactivate();
                        dirty_frames = 4; // restore desktop
                    } else if let Some(raw) = display.take_dma2d_raw() {
                        let mut blitter = rlvgl::platform::Dma2dBlitter::new(raw);
                        star_crawl.activate(&mut blitter);
                        display.return_dma2d_raw(blitter.into_inner());
                    }
                }

                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                let crawl_active = star_crawl.is_active();
                #[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
                let crawl_active = false;

                if crawl_active {
                    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                    if let Some(raw) = display.take_dma2d_raw() {
                        let mut blitter = rlvgl::platform::Dma2dBlitter::new(raw);
                        let back = display.back_buffer_addr();
                        let (w, h) = display.dimensions();
                        if star_crawl.tick(&mut blitter, back as *mut u8, w, h) {
                            render_count += 1;
                            display.present();
                        }
                        display.return_dma2d_raw(blitter.into_inner());
                        // Auto-deactivation: crawl finished scrolling
                        if !star_crawl.is_active() {
                            dirty_frames = 4;
                        }
                    }
                } else {
                    let need_render = dirty_frames > 0;
                    if need_render {
                        let back = display.back_buffer_addr();
                        let (w, h) = display.dimensions();
                        let fb_bytes = (w * h * 4) as usize;
                        let stride = (w * 4) as usize;

                        // Restore saved pixels under dismissed overlays
                        compositor.restore(back as *mut u8);

                        let fb_slice = unsafe {
                            core::slice::from_raw_parts_mut(back as *mut u8, fb_bytes)
                        };
                        let surface = Surface::new(
                            fb_slice, stride, PixelFmt::Argb8888, w, h,
                        );
                        let mut blit_renderer: BlitterRenderer<'_, CpuBlitter, 32> =
                            BlitterRenderer::new(&mut render_blitter, surface);
                        let mut renderer = RotatedRenderer::new(&mut blit_renderer, w);

                        // Draw widget tree
                        root.borrow().draw(&mut renderer);

                        render_count += 1;
                        if dirty_frames > 0 { dirty_frames -= 1; }
                        display.present();
                    }
                }

                tick_count += 1;
                // Telemetry at 0x3800_0604..0x3800_0640
                unsafe {
                    (0x3800_0604u32 as *mut u32).write_volatile(evt_count);
                    (0x3800_0608u32 as *mut u32).write_volatile(tick_count);
                    (0x3800_060Cu32 as *mut u32).write_volatile(render_count);
                    (0x3800_0610u32 as *mut u32).write_volatile(
                        ((dirty_frames as u32) << 16)
                        | ((was_visible as u32) << 8)
                        | (event_win.borrow().is_visible() as u32)
                    );
                    (0x3800_0614u32 as *mut u32).write_volatile(
                        display.back_buffer_addr()
                    );
                    (0x3800_0618u32 as *mut u32).write_volatile(
                        (0x5000_10ACu32 as *const u32).read_volatile()
                    );
                    // Cortex-M fault registers
                    (0x3800_0638u32 as *mut u32).write_volatile(
                        (0xE000_ED28u32 as *const u32).read_volatile() // CFSR
                    );
                    (0x3800_063Cu32 as *mut u32).write_volatile(
                        (0xE000_ED38u32 as *const u32).read_volatile() // MMFAR/BFAR
                    );
                    // LTDC ISR — check for underrun (bit 1) or error flags
                    (0x3800_0640u32 as *mut u32).write_volatile(
                        (0x5000_1010u32 as *const u32).read_volatile() // LTDC ISR
                    );
                    // EventWindow entry count for debugging
                    (0x3800_0644u32 as *mut u32).write_volatile(
                        event_win.borrow().entry_count() as u32
                    );

                    // Dump event telemetry ring over serial every ~1s (6 ticks)
                    let last_dump = (TELEM_DUMP_TICK as *const u32).read_volatile();
                    if tick_count - last_dump >= 180 { // ~30s at 6Hz
                        let idx = (TELEM_IDX_ADDR as *const u32).read_volatile();
                        if idx > 0 {
                            let dump_count = idx.min(TELEM_ENTRIES);
                            let start = if idx > TELEM_ENTRIES { idx - TELEM_ENTRIES } else { 0 };
                            serial_puts("TELEM:");
                            for i in start..start + dump_count {
                                let slot = i % TELEM_ENTRIES;
                                let base = TELEM_BASE + slot * TELEM_ENTRY_WORDS * 4;
                                let t = (base as *const u32).read_volatile();
                                let code = ((base + 4) as *const u32).read_volatile();
                                let x = ((base + 8) as *const u32).read_volatile();
                                let y = ((base + 12) as *const u32).read_volatile();
                                // Format: " T:code:x:y"
                                use core::fmt::Write;
                                let mut buf = alloc::string::String::new();
                                let _ = write!(buf, " {}:{:02x}:{},{}", t, code, x as i32, y as i32);
                                serial_puts(&buf);
                            }
                            serial_puts("\r\n");
                            // Reset ring
                            (TELEM_IDX_ADDR as *mut u32).write_volatile(0);
                        }
                        (TELEM_DUMP_TICK as *mut u32).write_volatile(tick_count);
                    }
                }
            }
            cortex_m::asm::nop();
        }
    }

    // Fallback: non-ARM / non-disco / doc builds
    #[cfg(not(any(
        all(feature = "c_hal",       feature = "stm32h747i_disco_cm7", any(target_arch = "arm", target_arch = "aarch64")),
        all(not(feature = "c_hal"),  feature = "stm32h747i_disco_cm7", any(target_arch = "arm", target_arch = "aarch64"))
    )))]
    loop {
        cortex_m::asm::nop();
    }
}

// ── c_hal application entry ─────────────────────────────────────────────────
//
// Called by c_bsp_init() after all C hardware init completes.  No Rust HAL
// clock configuration is needed here — clocks are already running at 400 MHz.
// PAC peripherals are obtained via steal() since the C side never called
// Peripherals::take().
#[cfg(all(
    feature = "c_hal",
    feature = "stm32h747i_disco_cm7",
    any(target_arch = "arm", target_arch = "aarch64")
))]
#[unsafe(no_mangle)]
pub extern "C" fn rlvgl_app_main() -> ! {
    use core::convert::Infallible;
    use embedded_hal::{
        digital::{ErrorType as DigitalError, InputPin, OutputPin},
        i2c::{ErrorType as I2cError, I2c as EhI2c, Operation, SevenBitAddress},
        pwm::{ErrorType as PwmError, SetDutyCycle},
    };
    use rlvgl::core::event::{Event, Key};
    use rlvgl::platform::{
        CpuBlitter, InputDevice, Stm32h747iDiscoDisplay, Stm32h747iDiscoInput,
    };

    // ── Disable D-cache to ensure SDRAM coherency with LTDC ─────────────────
    unsafe {
        let ccr = (0xE000_ED14u32 as *const u32).read_volatile();
        (0xE000_ED14u32 as *mut u32).write_volatile(ccr & !(1 << 16));
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    // ── Signal clocks ready to CM4 ──────────────────────────────────────────
    #[allow(clippy::let_unit_value)]
    let _ = bsp_pac::signal_clocks_ready();

    // ── Steal PAC peripherals (clocks/GPIO already configured by C) ─────────
    let dp = unsafe { stm32h7::stm32h747cm7::Peripherals::steal() };
    let mut cp = unsafe { cortex_m::Peripherals::steal() };

    // ── Direct GPIO output pin (BSRR-based, no HAL ownership chain) ─────────
    struct GpioOut { base: u32, pin: u8 }
    impl DigitalError for GpioOut { type Error = Infallible; }
    impl OutputPin for GpioOut {
        fn set_high(&mut self) -> Result<(), Infallible> {
            unsafe { ((self.base + 0x18) as *mut u32).write_volatile(1u32 << self.pin) }
            Ok(())
        }
        fn set_low(&mut self) -> Result<(), Infallible> {
            unsafe { ((self.base + 0x18) as *mut u32).write_volatile(1u32 << (self.pin + 16)) }
            Ok(())
        }
    }

    // ── Backlight: GPIO output wrapped as SetDutyCycle ───────────────────────
    struct GpioBacklight(GpioOut);
    impl PwmError for GpioBacklight { type Error = Infallible; }
    impl SetDutyCycle for GpioBacklight {
        fn max_duty_cycle(&self) -> u16 { u16::MAX }
        fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Infallible> {
            if duty == 0 { self.0.set_low() } else { self.0.set_high() }
        }
    }

    // ── Direct GPIO input pin ────────────────────────────────────────────────
    struct GpioIn { base: u32, pin: u8 }
    impl DigitalError for GpioIn { type Error = Infallible; }
    impl InputPin for GpioIn {
        fn is_high(&mut self) -> Result<bool, Infallible> {
            let idr = unsafe { ((self.base + 0x10) as *const u32).read_volatile() };
            Ok((idr >> self.pin) & 1 != 0)
        }
        fn is_low(&mut self) -> Result<bool, Infallible> {
            self.is_high().map(|v| !v)
        }
    }

    // ── Dummy I2C (touch controller not yet wired) ───────────────────────────
    struct DummyI2c;
    impl I2cError for DummyI2c { type Error = Infallible; }
    impl EhI2c<SevenBitAddress> for DummyI2c {
        fn read(&mut self, _a: SevenBitAddress, _b: &mut [u8]) -> Result<(), Infallible> { Ok(()) }
        fn write(&mut self, _a: SevenBitAddress, _b: &[u8]) -> Result<(), Infallible> { Ok(()) }
        fn write_read(&mut self, _a: SevenBitAddress, _b: &[u8], _r: &mut [u8]) -> Result<(), Infallible> { Ok(()) }
        fn transaction(&mut self, _a: SevenBitAddress, _ops: &mut [Operation<'_>]) -> Result<(), Infallible> { Ok(()) }
    }

    // ── Dummy button ─────────────────────────────────────────────────────────
    struct DummyButton;
    impl DigitalError for DummyButton { type Error = Infallible; }
    impl InputPin for DummyButton {
        fn is_high(&mut self) -> Result<bool, Infallible> { Ok(false) }
        fn is_low(&mut self)  -> Result<bool, Infallible> { Ok(true)  }
    }

    struct ButtonInput<B: InputPin> { button: B, last: bool }
    impl<B: InputPin> ButtonInput<B> {
        fn new(b: B) -> Self { Self { button: b, last: false } }
    }
    impl<B: InputPin> InputDevice for ButtonInput<B> {
        fn poll(&mut self) -> Option<Event> {
            let pressed = self.button.is_low().ok()?;
            match (pressed, self.last) {
                (true,  false) => { self.last = true;  Some(Event::KeyDown { key: Key::Enter }) }
                (false, true)  => { self.last = false; Some(Event::KeyUp   { key: Key::Enter }) }
                _ => None,
            }
        }
    }

    // GPIO base addresses (must match stm32h747xi.h)
    const GPIOG: u32 = 0x58021800;
    const GPIOJ: u32 = 0x58022400;
    const GPIOK: u32 = 0x58022800;

    // PG3: panel reset — ensure it's in GPIO output mode (C BSP may
    // have left it in AF mode, which prevents BSRR from toggling the pin).
    unsafe {
        let moder = (GPIOG as *mut u32).read_volatile();
        // Clear bits 7:6 (pin 3 MODER) and set to 01 (GP output)
        (GPIOG as *mut u32).write_volatile((moder & !(3u32 << 6)) | (1u32 << 6));
    }
    let panel_reset = GpioOut { base: GPIOG, pin: 3 };

    // PJ12: LCD backlight control (DSI_BL_CTRL per UM2411 CN15 pin 53)
    // Configure PJ12 as GP output (clear bits 25:24, set to 01)
    unsafe {
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 24)) | (1u32 << 24));
    }
    let backlight = GpioBacklight(GpioOut { base: GPIOJ, pin: 12 });

    // PJ6: debug toggle probe — Arduino D9 on CN5, scope-accessible
    // Configure PJ6 as GP output (MODER bits 13:12 = 01)
    unsafe {
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 12)) | (1u32 << 12));
    }
    /// Toggle PJ6 high then low as a scope breadcrumb.
    #[inline(always)]
    fn dbg_pulse() {
        const GPIOJ_BSRR: *mut u32 = (0x58022400 + 0x18) as *mut u32;
        unsafe {
            GPIOJ_BSRR.write_volatile(1u32 << 6);       // set PJ6
            cortex_m::asm::delay(40);                     // ~100ns pulse
            GPIOJ_BSRR.write_volatile(1u32 << (6 + 16)); // reset PJ6
        }
    }
    // Quick triple-pulse to confirm probe is alive
    for _ in 0..3 { dbg_pulse(); cortex_m::asm::delay(4_000); }

    // ── UART8 debug serial (PJ8=TX on Arduino D1/CN6, 115200 8N1) ─────
    // PJ8 = UART8_TX (AF8) — Port J clock already enabled
    const UART8: u32 = 0x4000_7C00;
    const RCC_APB1LENR: u32 = 0x5802_44E8;
    unsafe {
        // Enable UART8 clock (RCC_APB1LENR bit 31)
        let apb1 = (RCC_APB1LENR as *mut u32).read_volatile();
        (RCC_APB1LENR as *mut u32).write_volatile(apb1 | (1 << 31));
        (RCC_APB1LENR as *const u32).read_volatile(); // readback fence
        // PJ8 = AF8: MODER bits 17:16 = 10 (AF), AFRH bits 3:0 = 0x8
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 16)) | (2u32 << 16));
        let afrh = ((GPIOJ + 0x24) as *mut u32).read_volatile();
        ((GPIOJ + 0x24) as *mut u32).write_volatile((afrh & !(0xFu32)) | 8);
        // UART8: BRR = APB1_clk / baud = 100_000_000 / 115200 ≈ 868
        ((UART8 + 0x0C) as *mut u32).write_volatile(868); // BRR
        ((UART8 + 0x00) as *mut u32).write_volatile(
            (1 << 3)  // TE (transmitter enable)
            | (1 << 0) // UE (USART enable)
        );
    }

    // ── USART1 debug serial via ST-LINK VCP (PA9=TX AF7, 115200 8N1) ────
    const USART1: u32 = 0x4001_1000;
    const GPIOA: u32 = 0x5802_0000;
    unsafe {
        // Enable GPIOA clock (AHB4ENR bit 0)
        let ahb4 = (0x5802_44E0u32 as *mut u32).read_volatile();
        (0x5802_44E0u32 as *mut u32).write_volatile(ahb4 | (1 << 0));
        (0x5802_44E0u32 as *const u32).read_volatile();
        // PA9 = AF7: AFRH bits 7:4 = 7, MODER bits 19:18 = 10 (AF)
        let afrh = ((GPIOA + 0x24) as *mut u32).read_volatile();
        ((GPIOA + 0x24) as *mut u32).write_volatile((afrh & !(0xFu32 << 4)) | (7u32 << 4));
        let moder = (GPIOA as *mut u32).read_volatile();
        (GPIOA as *mut u32).write_volatile((moder & !(3u32 << 18)) | (2u32 << 18));
        // Enable USART1 clock (APB2ENR bit 4)
        let apb2 = (0x5802_44F0u32 as *mut u32).read_volatile();
        (0x5802_44F0u32 as *mut u32).write_volatile(apb2 | (1 << 4));
        (0x5802_44F0u32 as *const u32).read_volatile();
        // BRR = APB2_clk / baud = 100_000_000 / 115200 ≈ 868
        ((USART1 + 0x0C) as *mut u32).write_volatile(868);
        ((USART1 + 0x00) as *mut u32).write_volatile((1 << 3) | (1 << 0)); // TE + UE
    }

    /// Send a string over UART8 + USART1 VCP (blocking, dual output).
    fn dbg_print(s: &str) {
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
    dbg_print("rlvgl: UART8+VCP alive\r\n");
    dbg_pulse();

    // PK7: touch interrupt input
    let touch_int = GpioIn { base: GPIOK, pin: 7 };

    // ── SysTick: loose ~6 Hz flip timer ─────────────────────────────────────
    use cortex_m::peripheral::syst::SystClkSource;
    cp.SYST.set_clock_source(SystClkSource::Core);
    // 400 MHz / 6 Hz — truncates to 24-bit SysTick register; actual rate
    // will be higher, which is fine for a bring-up display flip.
    const SYS_HZ: u32 = 400_000_000;
    const FLIP_HZ: u32 = 6;
    cp.SYST.set_reload((SYS_HZ / FLIP_HZ).saturating_sub(1));
    cp.SYST.clear_current();
    cp.SYST.enable_counter();

    // ── Display ──────────────────────────────────────────────────────────────
    dbg_print("rlvgl: DSI+LTDC init start\r\n");
    dbg_pulse();
    let mut display = Stm32h747iDiscoDisplay::new(
        CpuBlitter,
        backlight,
        panel_reset,
        dp.LTDC,
        dp.DSIHOST,
        #[cfg(feature = "dma2d")]
        dp.DMA2D,
        #[cfg(feature = "splash")]
        Some(SPLASH_RLE),
    );
    dbg_print("rlvgl: DSI+LTDC init done\r\n");
    dbg_pulse();

    // Hold splash for ~2s
    #[cfg(feature = "splash")]
    for _ in 0..200u32 { cortex_m::asm::delay(4_000_000); }

    // Re-assert PJ12 as GP output and PG3 as GP output — the display
    // constructor or PAC peripheral take() may reset GPIO MODER.
    unsafe {
        // PJ12 backlight: MODER bits 25:24 = 01
        let moder = (GPIOJ as *mut u32).read_volatile();
        (GPIOJ as *mut u32).write_volatile((moder & !(3u32 << 24)) | (1u32 << 24));
        // Drive PJ12 high (backlight on)
        ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12);
        // PG3 panel reset: MODER bits 7:6 = 01, drive high
        let moder = (GPIOG as *mut u32).read_volatile();
        (GPIOG as *mut u32).write_volatile((moder & !(3u32 << 6)) | (1u32 << 6));
        ((GPIOG + 0x18) as *mut u32).write_volatile(1u32 << 3);
    }

    // ── IPC + input ──────────────────────────────────────────────────────────
    ipc::init();
    let mut input = Stm32h747iDiscoInput::new_with_int(
        DummyI2c, touch_int, display.dimensions().0 as u16,
    );
    let mut _button_input = ButtonInput::new(DummyButton);

    // ── Display server widget tree ───────────────────────────────────────────
    // Static widget tree with well-known IDs that CM4 drives via IPC commands.
    use alloc::{rc::Rc, vec::Vec};
    use core::cell::RefCell;
    use rlvgl::core::WidgetNode;
    use rlvgl::widgets::{button::Button, container::Container, label::Label};
    use rlvgl::core::widget::Rect;
    use rlvgl_i18n::t;

    let title_label = Rc::new(RefCell::new(Label::new(
        t!("hw.title", version = env!("CARGO_PKG_VERSION")),
        Rect { x: 10, y: 10, width: 200, height: 20 },
    )));

    let counter_button = Rc::new(RefCell::new(Button::new(
        t!("demo.clicks_zero"),
        Rect { x: 10, y: 40, width: 120, height: 30 },
    )));

    // When the button is tapped locally, forward ButtonPressed to CM4
    {
        counter_button.borrow_mut().set_on_click(|_btn: &mut Button| {
            let _ = ipc::event_push(ipc::evt_button_pressed(ipc::widget_id::CLICK_COUNTER));
        });
    }

    let status_label = Rc::new(RefCell::new(Label::new(
        t!("hw.cm4_waiting"),
        Rect { x: 10, y: 80, width: 300, height: 20 },
    )));

    let root = Rc::new(RefCell::new(WidgetNode {
        widget: Rc::new(RefCell::new(Container::new(Rect {
            x: 0, y: 0, width: 800, height: 480,
        }))),
        children: Vec::new(),
    }));
    root.borrow_mut().children.push(WidgetNode {
        widget: title_label.clone(),
        children: Vec::new(),
    });
    root.borrow_mut().children.push(WidgetNode {
        widget: counter_button.clone(),
        children: Vec::new(),
    });
    root.borrow_mut().children.push(WidgetNode {
        widget: status_label.clone(),
        children: Vec::new(),
    });

    // ── Semihosting SDRAM inspector ──────────────────────────────────────────
    // CM7 reads SDRAM perfectly; semihosting passes data via BKPT trap to the
    // debugger console, bypassing the AHB-AP bus width issues that corrupt
    // probe-rs direct reads.
    #[cfg(feature = "semihosting")]
    fn sh_hexdump(label: &str, addr: u32, words: usize) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "\n── {} @ 0x{:08X} ({} words) ──", label, addr, words);
            for i in 0..words {
                let a = addr + (i as u32) * 4;
                let val = unsafe { (a as *const u32).read_volatile() };
                if i % 4 == 0 {
                    let _ = write!(out, "  {:08X}:", a);
                }
                let _ = write!(out, " {:08X}", val);
                if i % 4 == 3 || i == words - 1 {
                    let _ = writeln!(out);
                }
            }
        }
    }

    #[cfg(feature = "semihosting")]
    #[allow(dead_code)]
    fn sh_print(msg: &str) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = write!(out, "{}", msg);
        }
    }

    #[cfg(feature = "semihosting")]
    fn sh_println(msg: &str) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let _ = writeln!(out, "{}", msg);
        }
    }

    #[cfg(feature = "semihosting")]
    fn sh_reg(label: &str, addr: u32) {
        use core::fmt::Write;
        if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
            let val = unsafe { (addr as *const u32).read_volatile() };
            let _ = writeln!(out, "  {} (0x{:08X}) = 0x{:08X}", label, addr, val);
        }
    }

    // Post-init semihosting dump: LTDC, DSI, and framebuffer contents
    #[cfg(feature = "semihosting")]
    {
        sh_println("╔══════════════════════════════════════════════════╗");
        sh_println("║   rlvgl semihosting SDRAM/register inspector    ║");
        sh_println("╚══════════════════════════════════════════════════╝");

        // Key DSI wrapper registers
        sh_println("\n── DSI wrapper ──");
        sh_reg("WCFGR ", 0x5000_0400);
        sh_reg("WCR   ", 0x5000_0404);
        sh_reg("WIER  ", 0x5000_0408);
        sh_reg("WISR  ", 0x5000_040C);
        sh_reg("WIFCR ", 0x5000_0410);
        sh_reg("WPCR0 ", 0x5000_0418);

        // DSI host registers (RM0399 §34.15: VR=0x00, CR=0x04, CCR=0x08)
        sh_println("\n── DSI host ──");
        sh_reg("VR    ", 0x5000_0000); // Version register
        sh_reg("CR    ", 0x5000_0004); // Control: bit0=EN
        sh_reg("CCR   ", 0x5000_0008); // Clock control
        sh_reg("LVCIDR", 0x5000_000C);
        sh_reg("LCOLCR", 0x5000_0010);
        sh_reg("LPCR  ", 0x5000_0014);
        sh_reg("LPMCR ", 0x5000_0018);
        sh_reg("PCR   ", 0x5000_002C);
        sh_reg("MCR   ", 0x5000_0034);
        sh_reg("VMCR  ", 0x5000_0038);
        sh_reg("CMCR  ", 0x5000_0068);
        sh_reg("GHCR  ", 0x5000_006C);
        sh_reg("GPSR  ", 0x5000_0074);

        // DSI PHY registers — ST CMSIS header offsets (matches PAC)
        sh_println("\n── DSI PHY (CMSIS/PAC offsets) ──");
        sh_reg("CLCR  ", 0x5000_0094);
        sh_reg("CLTCR ", 0x5000_0098); // Clock lane timer
        sh_reg("DLTCR ", 0x5000_009C);
        sh_reg("PCTLR ", 0x5000_00A0);
        sh_reg("PCONFR", 0x5000_00A4);
        sh_reg("PUCR  ", 0x5000_00A8); // ULPS control
        sh_reg("WRPCR ", 0x5000_0430);

        // LTDC and DSI error flags
        const LTDC_BASE: u32 = 0x5000_1000;
        sh_println("\n── Error flags ──");
        sh_reg("LTDC_ISR ", LTDC_BASE + 0x38); // bit1=FUIF (FIFO underrun)
        sh_reg("LTDC_GCR ", LTDC_BASE + 0x18);
        sh_reg("DSI_ISR0 ", 0x5000_00BC); // ACK errors, PHY errors
        sh_reg("DSI_ISR1 ", 0x5000_00C0); // Payload errors

        // LTDC pre-LTDCEN values (comprehensive dump at 0x24070140)
        sh_println("\n── LTDC pre-LTDCEN snapshot (0x24070140) ──");
        sh_hexdump("Pre-en full", 0x2407_0140, 16);
        // Layout: [sentinel, L1CR, WHPCR, WVPCR, PFCR, CACR,
        //          BFCR, CFBAR, CFBLR, CFBLNR,
        //          SSCR, BPCR, AWCR, TWCR, GCR, end]

        // Also read SRAM diagnostic dump
        sh_hexdump("SRAM diag", 0x2407_0000, 27);

        // Framebuffer content: read from pre-stored CFBAR (0x24070128)
        // (Live LTDC reads are aliased to GCR after LTDCEN)
        let cfbar = unsafe { (0x2407_0128u32 as *const u32).read_volatile() };
        if cfbar >= 0x2400_0000 && cfbar < 0x2408_0000 {
            sh_hexdump("Framebuffer (AXI SRAM)", cfbar, 64);
        } else if cfbar >= 0xC000_0000 {
            sh_hexdump("Framebuffer (SDRAM)", cfbar, 64);
        } else {
            use core::fmt::Write;
            if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                let _ = writeln!(out, "  CFBAR=0x{:08X} — unexpected range!", cfbar);
            }
        }

        // SDRAM sanity: read/write test at 0xC000_0000
        sh_println("\n── SDRAM read/write test ──");
        let test_addr: u32 = 0xC000_0000;
        unsafe {
            let before = (test_addr as *const u32).read_volatile();
            (test_addr as *mut u32).write_volatile(0xDEAD_BEEF);
            cortex_m::asm::dsb();
            let after = (test_addr as *const u32).read_volatile();
            (test_addr as *mut u32).write_volatile(before); // restore
            use core::fmt::Write;
            if let Ok(mut out) = cortex_m_semihosting::hio::hstdout() {
                let _ = writeln!(out, "  [0xC0000000] before=0x{:08X} wrote=0xDEADBEEF readback=0x{:08X} {}",
                    before, after, if after == 0xDEAD_BEEF { "OK" } else { "FAIL" });
            }
        }

        sh_println("\n── Initial dump complete ──\n");
    }

    dbg_print("rlvgl: entering main loop\r\n");
    dbg_pulse();

    // ── Backlight blink test: 3 visible blinks on PJ12 ─────────────────────
    for _ in 0..3 {
        unsafe {
            // PJ12 HIGH
            ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12);
            cortex_m::asm::delay(80_000_000); // ~200ms at 400 MHz
            // PJ12 LOW
            ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << (12 + 16));
            cortex_m::asm::delay(80_000_000);
        }
    }
    // Leave backlight ON
    unsafe { ((GPIOJ + 0x18) as *mut u32).write_volatile(1u32 << 12); }

    // ── Display server main loop ─────────────────────────────────────────────
    let mut tap2 = rlvgl::platform::gesture::TapRecognizer::new(2);
    let mut frame_counter: u32 = 0;

    loop {
        // 1. Drain command queue from CM4
        while let Some(cmd) = ipc::cmd_pop() {
            match ipc::CmdKind::from_u32(cmd.kind) {
                ipc::CmdKind::SetBacklight => {
                    let duty = (cmd.a & 0xFFFF) as u16;
                    let level = if duty < 512 { 0 } else { u16::MAX };
                    display.set_brightness(level);
                }
                ipc::CmdKind::UpdateLabel => {
                    let mut buf = [0u8; 12];
                    let len = ipc::extract_label_text(&cmd, &mut buf);
                    if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                        let text_owned = alloc::string::String::from(text);
                        match cmd.a {
                            ipc::widget_id::TITLE => {
                                title_label.borrow_mut().set_text(text_owned);
                            }
                            ipc::widget_id::CLICK_COUNTER => {
                                counter_button.borrow_mut().set_text(text_owned);
                            }
                            ipc::widget_id::STATUS_LABEL => {
                                status_label.borrow_mut().set_text(text_owned);
                            }
                            _ => {}
                        }
                    }
                }
                ipc::CmdKind::Navigate => {
                    // Screen navigation — placeholder for future screens
                }
                ipc::CmdKind::UpdateValue => {
                    // Numeric value update — placeholder
                }
                ipc::CmdKind::ShowWidget => {
                    // Widget visibility — placeholder
                }
                ipc::CmdKind::None => {}
            }
        }

        // 2. Poll touch → gesture → dispatch to widget tree → forward to CM4
        if let Some(evt) = input.poll() {
            let transformed = match &evt {
                Event::PointerDown { x, y } => Event::PointerDown {
                    x: *y, y: w_fb as i32 - 1 - *x,
                },
                Event::PointerUp { x, y } => Event::PointerUp {
                    x: *y, y: w_fb as i32 - 1 - *x,
                },
                Event::PointerMove { x, y } => Event::PointerMove {
                    x: *y, y: w_fb as i32 - 1 - *x,
                },
                other => other.clone(),
            };
            if let Some(gesture) = tap2.process(&transformed) {
                root.borrow_mut().dispatch_event(&gesture);
            }
            // Forward touch events to CM4 (primary point only for IPC)
            let ipc_evt = match &evt {
                Event::PointerDown { x, y } => Some(ipc::evt_pointer_down(*x, *y)),
                Event::PointerMove { x, y } => Some(ipc::evt_pointer_move(*x, *y)),
                Event::PointerUp { x, y } => Some(ipc::evt_pointer_up(*x, *y)),
                Event::Touch { count, points } if *count > 0 => {
                    let tp = &points[0];
                    Some(ipc::evt_pointer_down(tp.x, tp.y))
                }
                _ => None,
            };
            if let Some(e) = ipc_evt {
                let _ = ipc::event_push(e);
            }
        }

        // 3. SysTick → render frame → notify CM4
        if cp.SYST.has_wrapped() {
            // Heartbeat toggle on PJ6 (CN5 D9)
            unsafe {
                const GPIOJ_ODR: *mut u32 = (0x58022400 + 0x14) as *mut u32;
                let odr = GPIOJ_ODR.read_volatile();
                GPIOJ_ODR.write_volatile(odr ^ (1 << 6));
            }
            display.present();
            // Periodic UART status (~1 Hz)
            if frame_counter % 25 == 0 {
                dbg_print(".");
            }
            // Periodic semihosting SDRAM dump (~30s = every 180 frames at 6 Hz)
            // Periodic semihosting SDRAM dump (~30s = every 180 frames at 6 Hz)
            #[cfg(feature = "semihosting")]
            if frame_counter % 180 == 30 {
                sh_println("\n── Periodic SDRAM check ──");
                // CFBAR is at LTDC+0xAC (aliased after LTDCEN — use pre-stored value)
                let cfbar = unsafe { (0x2407_0128u32 as *const u32).read_volatile() };
                sh_hexdump("FB snapshot", cfbar, 16);
                sh_reg("WISR  ", 0x5000_040C);
                sh_reg("WCR   ", 0x5000_0404);
                sh_reg("CR    ", 0x5000_0004);
            }
            frame_counter = frame_counter.wrapping_add(1);
            let _ = ipc::event_push(ipc::evt_frame_rendered(frame_counter));
        }

        cortex_m::asm::nop();
    }
}

#[cfg(doc)]
fn main() {}
