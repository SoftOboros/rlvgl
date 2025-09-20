#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

extern crate alloc;

use core::arch::asm;
use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
#[cfg(not(doc))]
use panic_halt as _;

#[path = "../../common_demo/lib.rs"]
mod common_demo;

// Use the split-core generated PAC BSP for CM7
#[path = "bsp/cm7/pac.rs"]
mod bsp_pac;
mod ipc;
// HAL BSP module is not required for this bring-up path

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
#[allow(unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_TRACE: u32 = 0;

#[allow(unsafe_attributes)]
#[unsafe(link_section = ".noinit")]
#[unsafe(no_mangle)]
static mut MPU_DUMP: [u32; 12] = [0; 12];

#[inline(always)]
fn set_mpu_trace(val: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(MPU_TRACE), val);
    }
}

#[inline(always)]
fn record_region(slot: usize, base: u32, rasr: u32) {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(MPU_DUMP[slot * 2]);
        core::ptr::write_volatile(ptr, base);
        core::ptr::write_volatile(ptr.add(1), rasr);
    }
}

#[inline(always)]
fn single_nop() {
    unsafe {
        asm!("nop", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
fn barrier_dsb() {
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[inline(always)]
fn barrier_isb() {
    unsafe {
        asm!("isb sy", options(nostack, preserves_flags));
    }
}

#[cfg(feature = "pac_sdram_init")]
const SDRAM_REFRESH_COUNT: u16 = 566;
#[cfg(feature = "pac_sdram_init")]
const SDRAM_MODE_REGISTER: u16 = 0x0230;

#[cfg(feature = "pac_sdram_init")]
fn wait_for_sdram_ready(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    while fmc.sdsr.read().bits() & (1 << 5) != 0 {
        cortex_m::asm::nop();
    }
}

#[cfg(feature = "pac_sdram_init")]
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
                .set_bit()
                .ctb2()
                .clear_bit()
                .nrfs()
                .bits(auto_refresh)
                .mrd()
                .bits(mode_register)
        });
    }
    wait_for_sdram_ready(fmc);
}

#[cfg(feature = "pac_sdram_init")]
fn configure_fmc_sdram(fmc: &stm32h7::stm32h747cm7::fmc::RegisterBlock) {
    unsafe {
        fmc.bcr1.modify(|_, w| w.fmcen().set_bit());
        fmc.sdbank1().sdcr.write(|w| {
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
                .sdclk()
                .bits(0b01)
                .rburst()
                .set_bit()
                .rpipe()
                .bits(0)
        });
        fmc.sdbank1().sdtr.write(|w| {
            w.tmrd()
                .bits(1)
                .txsr()
                .bits(6)
                .tras()
                .bits(4)
                .trc()
                .bits(6)
                .twr()
                .bits(1)
                .trp()
                .bits(1)
                .trcd()
                .bits(1)
        });
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

#[cfg(feature = "pac_sdram_init")]
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

#[cfg(feature = "pac_sdram_init")]
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
    configure_fmc_sdram(fmc);
}

/// Heap size in bytes.
const HEAP_SIZE: usize = 64 * 1024;

/// Static memory region used to service heap allocations.
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// Application entry point.
#[cfg(not(doc))]
#[entry]
fn main() -> ! {
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    #[cfg(all(
        feature = "stm32h747i_disco_cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // Early spin delay to give debuggers time to attach before
        // peripheral clocks and pin configuration. This is a coarse, cycle-based
        // busy-wait that does not rely on any timers being configured yet.
        // Adjust the iteration count as needed for your CPU clock.
        // Rough guide: 10 × 100M cycles ≈ ~2.5s @ 400 MHz, ~10s @ 100 MHz.
        for _ in 0..10 {
            cortex_m::asm::delay(100_000_000);
        }

        let mut cp = cortex_m::Peripherals::take().unwrap();
        configure_mpu_regions(&mut cp);

        use core::convert::Infallible;
        use embedded_hal::{
            digital::{ErrorType as DigitalError, InputPin},
            i2c::{ErrorType as I2cError, I2c as EhI2c, Operation, SevenBitAddress},
            pwm::{ErrorType as PwmError, SetDutyCycle},
        };
        use rlvgl::core::event::{Event, Key};
        use rlvgl::platform::{
            CpuBlitter, InputDevice, Stm32h747iDiscoDisplay, Stm32h747iDiscoInput,
        };
        #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
        use rlvgl::platform::{DiscoSdBlockDevice, mount_and_list_assets};
        use stm32h7xx_hal::prelude::*;

        // Backlight adapter using a HAL GPIO pin as a stand-in for PWM
        use stm32h7xx_hal::gpio::{Output, Pin, PushPull};
        // Backlight control on PJ6 (GPIO fallback); touch INT uses PK7
        type HalBacklightPin = Pin<'J', 6, Output<PushPull>>;
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

        struct DummyButton;
        impl DigitalError for DummyButton {
            type Error = Infallible;
        }
        impl InputPin for DummyButton {
            fn is_high(&mut self) -> Result<bool, Self::Error> {
                Ok(false)
            }
            fn is_low(&mut self) -> Result<bool, Self::Error> {
                Ok(true)
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
        // Destructure PAC peripherals and switch to HAL for operation
        let dp = stm32h7::stm32h747cm7::Peripherals::take().unwrap();

        #[cfg(feature = "pac_sdram_init")]
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
            I2C4: _i2c4,
            TIM8,
            DSIHOST: dsi,
            FMC: fmc,
            LTDC: ltdc,
            #[cfg(feature = "dma2d")]
            DMA2D,
            #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
            GPIOC,
            #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
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
        // Signal clocks ready to CM4 via shared mailbox flag
        #[allow(clippy::let_unit_value)]
        {
            // Safe to call; function is a no-op in unified builds
            let _ = bsp_pac::signal_clocks_ready();
        }
        let gpioj = GPIOJ.split(ccdr.peripheral.GPIOJ);
        let gpiog = GPIOG.split(ccdr.peripheral.GPIOG);
        let gpiok = GPIOK.split(ccdr.peripheral.GPIOK);
        let gpiod = GPIOD.split(ccdr.peripheral.GPIOD);
        let gpioe = GPIOE.split(ccdr.peripheral.GPIOE);
        let gpiof = GPIOF.split(ccdr.peripheral.GPIOF);
        let gpioh = GPIOH.split(ccdr.peripheral.GPIOH);
        let gpioi = GPIOI.split(ccdr.peripheral.GPIOI);
        #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
        let gpioc = GPIOC.split(ccdr.peripheral.GPIOC);
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
            let mut ch = TIM8.pwm(pj6_ch2, 10.kHz(), ccdr.peripheral.TIM8, &ccdr.clocks);
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
        let mut display = Stm32h747iDiscoDisplay::new(
            blitter,
            backlight,
            HalResetPin(panel_reset_hal),
            ltdc,
            dsi,
            #[cfg(feature = "dma2d")]
            DMA2D,
        );
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
        // Main loop: handle IPC commands (from CM4) and input stubs
        ipc::init();
        // Touch I2C placeholder: provide a dummy I²C until the HAL 0.2 → EH1.0 adapter is ready
        struct DummyI2c;
        impl I2cError for DummyI2c {
            type Error = Infallible;
        }
        impl EhI2c<SevenBitAddress> for DummyI2c {
            fn read(
                &mut self,
                _address: SevenBitAddress,
                _buf: &mut [u8],
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            fn write(
                &mut self,
                _address: SevenBitAddress,
                _bytes: &[u8],
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            fn write_read(
                &mut self,
                _address: SevenBitAddress,
                _bytes: &[u8],
                _buf: &mut [u8],
            ) -> Result<(), Self::Error> {
                Ok(())
            }
            fn transaction(
                &mut self,
                _address: SevenBitAddress,
                _ops: &mut [Operation<'_>],
            ) -> Result<(), Self::Error> {
                Ok(())
            }
        }
        let i2c = DummyI2c;
        let touch_int = HalInputPin(gpiok.pk7.into_floating_input());
        let mut input = Stm32h747iDiscoInput::new_with_int(i2c, touch_int);
        let button = DummyButton;
        let mut button_input = ButtonInput::new(button);

        let demo = common_demo::build_demo(800, 480);
        let root = demo.root;
        let pending = demo.pending;
        let to_remove = demo.to_remove;

        #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
        {
            use alloc::{format, rc::Rc};
            use core::cell::RefCell;
            use rlvgl::core::widget::Rect;
            use rlvgl::widgets::label::Label;
            use stm32h7xx_hal::gpio::Alternate;
            // SDMMC1 pins: PC12=CK, PD2=CMD, PC8..PC11=D0..D3 (AF12)
            let ck: stm32h7xx_hal::gpio::Pin<'C', 12, Alternate<12>> = gpioc.pc12.into_alternate();
            let cmd: stm32h7xx_hal::gpio::Pin<'D', 2, Alternate<12>> = gpiod.pd2.into_alternate();
            let d0: stm32h7xx_hal::gpio::Pin<'C', 8, Alternate<12>> = gpioc.pc8.into_alternate();
            let d1: stm32h7xx_hal::gpio::Pin<'C', 9, Alternate<12>> = gpioc.pc9.into_alternate();
            let d2: stm32h7xx_hal::gpio::Pin<'C', 10, Alternate<12>> = gpioc.pc10.into_alternate();
            let d3: stm32h7xx_hal::gpio::Pin<'C', 11, Alternate<12>> = gpioc.pc11.into_alternate();
            let pins = (ck, cmd, d0, d1, d2, d3);
            let sdmmc = stm32h7xx_hal::sdmmc::Sdmmc::new(
                SDMMC1,
                pins,
                ccdr.peripheral.SDMMC1,
                &ccdr.clocks,
            );
            let mut bd = DiscoSdBlockDevice::new(sdmmc);
            match mount_and_list_assets(&mut bd) {
                Ok(names) => {
                    if names.is_empty() {
                        let label = Label::new(
                            "SD: no assets",
                            Rect {
                                x: 10,
                                y: 70,
                                width: 180,
                                height: 16,
                            },
                        );
                        let node = rlvgl::core::WidgetNode {
                            widget: Rc::new(RefCell::new(label)),
                            children: alloc::vec![],
                        };
                        pending.borrow_mut().push(node);
                    } else {
                        for (i, name) in names.into_iter().take(4).enumerate() {
                            let label = Label::new(
                                format!("asset: {}", name),
                                Rect {
                                    x: 10,
                                    y: 70 + (i as i32 * 18),
                                    width: 260,
                                    height: 16,
                                },
                            );
                            let node = rlvgl::core::WidgetNode {
                                widget: Rc::new(RefCell::new(label)),
                                children: alloc::vec![],
                            };
                            pending.borrow_mut().push(node);
                        }
                    }
                    common_demo::flush_pending(&root, &pending, &to_remove);
                }
                Err(_) => {
                    let label = Label::new(
                        "SD: mount/list failed",
                        Rect {
                            x: 10,
                            y: 70,
                            width: 220,
                            height: 16,
                        },
                    );
                    let node = rlvgl::core::WidgetNode {
                        widget: Rc::new(RefCell::new(label)),
                        children: alloc::vec![],
                    };
                    pending.borrow_mut().push(node);
                    common_demo::flush_pending(&root, &pending, &to_remove);
                }
            }
        }

        loop {
            // Handle CM4 commands
            if let Some(cmd) = ipc::pop() {
                if cmd.kind == ipc::CmdKind::SetBacklight as u32 {
                    let duty = (cmd.a & 0xFFFF) as u16;
                    // Map 16-bit duty to simple on/off for GPIO fallback
                    let level = if duty < 512 { 0 } else { u16::MAX };
                    display.set_brightness(level);
                }
            }
            if let Some(evt) = input.poll() {
                root.borrow_mut().dispatch_event(&evt);
                common_demo::flush_pending(&root, &pending, &to_remove);
            }
            if let Some(evt) = button_input.poll() {
                root.borrow_mut().dispatch_event(&evt);
                common_demo::flush_pending(&root, &pending, &to_remove);
            }
            // Flip on SysTick wrap (approx. 60 Hz)
            if cp.SYST.has_wrapped() {
                display.present();
            }
            cortex_m::asm::nop();
        }
    }

    #[cfg(not(all(
        feature = "stm32h747i_disco_cm7",
        any(target_arch = "arm", target_arch = "aarch64")
    )))]
    loop {
        cortex_m::asm::nop();
    }
}

#[cfg(doc)]
fn main() {}
