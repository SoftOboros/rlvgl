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

#[path = "bsp/pac.rs"]
mod bsp_pac;
// HAL BSP module is not required for this bring-up path

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
        feature = "stm32h747i_disco",
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
            digital::{ErrorType as DigitalError, InputPin, OutputPin},
            i2c::{ErrorType as I2cError, I2c, Operation, SevenBitAddress},
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
        use stm32h7xx_hal::hal::digital::v2::OutputPin as OutputPin02;
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

        struct DummyI2c;
        impl I2cError for DummyI2c {
            type Error = Infallible;
        }
        impl I2c<SevenBitAddress> for DummyI2c {
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
        // Destructure PAC peripherals and switch to HAL for operation
        let dp = stm32h7::stm32h747cm7::Peripherals::take().unwrap();
        let stm32h7::stm32h747cm7::Peripherals {
            PWR,
            RCC,
            SYSCFG,
            GPIOJ,
            TIM8,
            DSIHOST: dsi,
            FMC: fmc,
            LTDC: ltdc,
            #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
            GPIOC,
            #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
            GPIOD,
            #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
            SDMMC1,
            ..
        } = dp;
        let pwr = PWR.constrain();
        let vos = pwr.freeze();
        let rcc = RCC.constrain();
        let mut syscfg = SYSCFG;
        let ccdr = rcc.freeze(vos, &mut syscfg);
        let gpioj = GPIOJ.split(ccdr.peripheral.GPIOJ);
        #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
        let gpioc = GPIOC.split(ccdr.peripheral.GPIOC);
        #[cfg(all(feature = "fatfs_nostd", feature = "sd_assets_demo"))]
        let gpiod = GPIOD.split(ccdr.peripheral.GPIOD);
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
        let mut panel_reset_hal = gpioj.pj12.into_push_pull_output();
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
        let mut _display = Stm32h747iDiscoDisplay::new(
            blitter,
            backlight,
            HalResetPin(panel_reset_hal),
            ltdc,
            dsi,
            fmc,
        );
        let i2c = DummyI2c;
        let mut input = Stm32h747iDiscoInput::new(i2c);
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
            if let Some(evt) = input.poll() {
                root.borrow_mut().dispatch_event(&evt);
                common_demo::flush_pending(&root, &pending, &to_remove);
            }
            if let Some(evt) = button_input.poll() {
                root.borrow_mut().dispatch_event(&evt);
                common_demo::flush_pending(&root, &pending, &to_remove);
            }
            cortex_m::asm::nop();
        }
    }

    #[cfg(not(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    )))]
    loop {
        cortex_m::asm::nop();
    }
}

#[cfg(doc)]
fn main() {}
