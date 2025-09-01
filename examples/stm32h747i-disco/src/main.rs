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
#[path = "bsp/hal.rs"]
mod bsp_hal;

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

        struct GpioBacklight;
        impl PwmError for GpioBacklight {
            type Error = Infallible;
        }
        impl SetDutyCycle for GpioBacklight {
            fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
                unsafe {
                    let gpioj = &*stm32h7::stm32h747cm7::GPIOJ::ptr();
                    if duty == 0 {
                        gpioj.bsrr.write(|w| w.bits(1 << (6 + 16))); // PJ6 low
                    } else {
                        gpioj.bsrr.write(|w| w.bits(1 << 6)); // PJ6 high
                    }
                }
                Ok(())
            }
            fn max_duty_cycle(&self) -> u16 {
                u16::MAX
            }
        }

        struct DummyReset;
        impl DigitalError for DummyReset {
            type Error = Infallible;
        }
        impl OutputPin for DummyReset {
            fn set_high(&mut self) -> Result<(), Self::Error> {
                Ok(())
            }
            fn set_low(&mut self) -> Result<(), Self::Error> {
                Ok(())
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
        // Configure essential clocks and pins using the generated HAL-based BSP
        // prior to moving PAC peripherals.
        let mut p = stm32h7::stm32h747cm7::Peripherals::take().unwrap();
        use stm32h7xx_hal::prelude::*;
        let pwr = p.PWR.constrain();
        let vos = pwr.freeze();
        let rcc = p.RCC.constrain();
        let ccdr = rcc.freeze(vos, &mut p.SYSCFG);
        bsp_hal::enable_gpio_clocks(&p);
        bsp_hal::configure_pins_hal(&p, &ccdr);
        // Minimal panel reset (PJ12) and prepare backlight (PJ6 as PWM placeholder)
        unsafe {
            let gpioj = &*stm32h7::stm32h747cm7::GPIOJ::ptr();
            // PJ6 and PJ12 -> output mode (01)
            let mut moder = gpioj.moder.read().bits();
            moder &= !(0b11 << (6 * 2));
            moder |= 0b01 << (6 * 2);
            moder &= !(0b11 << (12 * 2));
            moder |= 0b01 << (12 * 2);
            gpioj.moder.write(|w| w.bits(moder));
            // push-pull
            let mut otyper = gpioj.otyper.read().bits();
            otyper &= !(1 << 6);
            otyper &= !(1 << 12);
            gpioj.otyper.write(|w| w.bits(otyper));
            // Panel reset sequence: PJ12 low → delay → high
            gpioj.bsrr.write(|w| w.bits(1 << (12 + 16))); // reset low
            cortex_m::asm::delay(10_000_00);
            gpioj.bsrr.write(|w| w.bits(1 << 12)); // reset high
        }
        let stm32h7::stm32h747cm7::Peripherals {
            DSIHOST: dsi,
            FMC: fmc,
            LTDC: ltdc,
            RCC: mut rcc,
            ..
        } = p;
        let blitter = CpuBlitter;
        let backlight = GpioBacklight;
        let reset = DummyReset;
        let mut _display =
            Stm32h747iDiscoDisplay::new(blitter, backlight, reset, ltdc, dsi, fmc, &mut rcc);
        let i2c = DummyI2c;
        let mut input = Stm32h747iDiscoInput::new(i2c);
        let button = DummyButton;
        let mut button_input = ButtonInput::new(button);

        let demo = common_demo::build_demo(800, 480);
        let root = demo.root;
        let pending = demo.pending;
        let to_remove = demo.to_remove;

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
