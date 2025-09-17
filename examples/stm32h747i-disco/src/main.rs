#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

extern crate alloc;

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

        use core::convert::Infallible;
        use embedded_hal::{
            digital::{ErrorType as DigitalError, InputPin},
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
        impl embedded_hal::digital::ErrorType for HalInputPin<P> {
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
            TIM8,
            DSIHOST: dsi,
            FMC: fmc,
            LTDC: ltdc,
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
        use stm32h7xx_hal::prelude::*;
        let rcc = RCC.constrain();
        let mut syscfg = SYSCFG;
        // HAL RCC: derive SYSCLK and LTDC pixel clock (via PLL3R)
        // Assumes HSE=25 MHz on H747I-DISCO. Adjust if using HSI or a different crystal.
        let ccdr = rcc
            .use_hse(25.MHz())
            .sys_ck(400.MHz())
            .pll1_strategy(PllConfigStrategy::Iterative)
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
        // Configure FMC SDRAM pin mux (subset necessary for SDRAM Bank1)
        use stm32h7xx_hal::gpio::Alternate;
        // Address lines PF0..PF5 (A0..A5)
        let _ = gpiof.pf0.into_alternate::<0>();
        let _ = gpiof.pf1.into_alternate::<0>();
        let _ = gpiof.pf2.into_alternate::<0>();
        let _ = gpiof.pf3.into_alternate::<0>();
        let _ = gpiof.pf4.into_alternate::<0>();
        let _ = gpiof.pf5.into_alternate::<0>();
        // Address lines PF12..PF15 (A6..A9)
        let _ = gpiof.pf12.into_alternate::<0>();
        let _ = gpiof.pf13.into_alternate::<0>();
        let _ = gpiof.pf14.into_alternate::<0>();
        let _ = gpiof.pf15.into_alternate::<0>();
        // Address lines PG0..PG1 (A10..A11), PG2 (A12), PG4 (BA0)
        let _ = gpiog.pg0.into_alternate::<0>();
        let _ = gpiog.pg1.into_alternate::<0>();
        let _ = gpiog.pg2.into_alternate::<0>();
        let _ = gpiog.pg4.into_alternate::<0>();
        // Control lines PF11 (SDNRAS), PG15 (SDNCAS), PH5 (SDNWE)
        let _ = gpiof.pf11.into_alternate::<0>();
        let _ = gpiog.pg15.into_alternate::<0>();
        let _ = gpioh.ph5.into_alternate::<0>();
        // Clock and enable: PG8 (SDCLK), PH6 (SDNE1), PH7 (SDCKE1)
        let _ = gpiog.pg8.into_alternate::<0>();
        let _ = gpioh.ph6.into_alternate::<0>();
        let _ = gpioh.ph7.into_alternate::<0>();
        // Byte lane enables: PE0 (NBL0), PE1 (NBL1), PI4 (NBL2), PI5 (NBL3)
        let _ = gpioe.pe0.into_alternate::<0>();
        let _ = gpioe.pe1.into_alternate::<0>();
        let _ = gpioi.pi4.into_alternate::<0>();
        let _ = gpioi.pi5.into_alternate::<0>();
        // Data lines D0..D15 on PD/PE
        let _ = gpiod.pd14.into_alternate::<0>(); // D0
        let _ = gpiod.pd15.into_alternate::<0>(); // D1
        let _ = gpiod.pd0.into_alternate::<0>();  // D2
        let _ = gpiod.pd1.into_alternate::<0>();  // D3
        let _ = gpioe.pe7.into_alternate::<0>();  // D4
        let _ = gpioe.pe8.into_alternate::<0>();  // D5
        let _ = gpioe.pe9.into_alternate::<0>();  // D6
        let _ = gpioe.pe10.into_alternate::<0>(); // D7
        let _ = gpioe.pe11.into_alternate::<0>(); // D8
        let _ = gpioe.pe12.into_alternate::<0>(); // D9
        let _ = gpioe.pe13.into_alternate::<0>(); // D10
        let _ = gpioe.pe14.into_alternate::<0>(); // D11
        let _ = gpioe.pe15.into_alternate::<0>(); // D12
        let _ = gpiod.pd8.into_alternate::<0>();  // D13
        let _ = gpiod.pd9.into_alternate::<0>();  // D14
        let _ = gpiod.pd10.into_alternate::<0>(); // D15
        // Data lines D16..D23 on PH
        let _ = gpioh.ph8.into_alternate::<0>();  // D16
        let _ = gpioh.ph9.into_alternate::<0>();  // D17
        let _ = gpioh.ph10.into_alternate::<0>(); // D18
        let _ = gpioh.ph11.into_alternate::<0>(); // D19
        let _ = gpioh.ph12.into_alternate::<0>(); // D20
        let _ = gpioh.ph13.into_alternate::<0>(); // D21
        let _ = gpioh.ph14.into_alternate::<0>(); // D22
        let _ = gpioh.ph15.into_alternate::<0>(); // D23
        // Data lines D24..D31 on PI
        let _ = gpioi.pi0.into_alternate::<0>();  // D24
        let _ = gpioi.pi1.into_alternate::<0>();  // D25
        let _ = gpioi.pi2.into_alternate::<0>();  // D26
        let _ = gpioi.pi3.into_alternate::<0>();  // D27
        let _ = gpioi.pi6.into_alternate::<0>();  // D28
        let _ = gpioi.pi7.into_alternate::<0>();  // D29
        let _ = gpioi.pi9.into_alternate::<0>();  // D30
        let _ = gpioi.pi10.into_alternate::<0>(); // D31

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
        let mut cp = cortex_m::Peripherals::take().unwrap();
        use cortex_m::peripheral::syst::SystClkSource;
        cp.SYST.set_clock_source(SystClkSource::Core);
        let sys_hz = ccdr.clocks.sys_ck().0;
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
            fmc,
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
                        if p.read_volatile() != 0x0000_0000 { errs += 1; }
                    }

                    // Pattern 2: solid ones
                    for i in 0..STRIDE {
                        let p = (mb_base as *mut u32).add(i * 8 + 1);
                        p.write_volatile(0xFFFF_FFFF);
                    }
                    for i in 0..STRIDE {
                        let p = (mb_base as *const u32).add(i * 8 + 1);
                        if p.read_volatile() != 0xFFFF_FFFF { errs += 1; }
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
                        if p.read_volatile() != v { errs += 1; }
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
                        if p0.read_volatile() != 0xAAAA_AAAA { errs += 1; }
                        if p1.read_volatile() != 0x5555_5555 { errs += 1; }
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
                        if p.read_volatile() != seed2 { errs += 1; }
                    }

                    log!("SDRAM test: MB {} -> {} errors\n", mb, errs);
                }
            }
        }
        // Main loop: handle IPC commands (from CM4) and input stubs
        ipc::init();
        // Touch I2C4 on PD12/PD13 @ 400 kHz and INT on PK7
        let i2c = rlvgl::platform::stm32h747i_disco::init_touch_i2c(
            I2C4,
            gpiod,
            ccdr.peripheral.I2C4,
            &ccdr.clocks,
        );
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
