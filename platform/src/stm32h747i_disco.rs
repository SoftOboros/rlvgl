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
use stm32h7::stm32h747cm7::{DSIHOST, FMC, LTDC, RCC};

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
    ltdc: LTDC,
    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    dsi: DSIHOST,
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
        rcc: &mut RCC,
    ) -> Self
    where
        BL: SetDutyCycle,
        RST: OutputPin,
    {
        // Enable LTDC and DSI peripheral clocks
        rcc.apb3enr()
            .modify(|_, w| w.ltdcen().set_bit().dsien().set_bit());
        // Ensure the panel is held in reset and the backlight is off
        let _ = reset.set_low();
        let mut disp = Self {
            _blitter: blitter,
            backlight,
            reset,
            ltdc,
            dsi,
        };
        disp.set_backlight(0);
        disp.reset_panel();
        Otm8009a::init(&mut disp.dsi);
        let fb = Self::init_sdram(fmc, rcc);
        disp.setup_ltdc_layer(fb, 800, 480);
        disp
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

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn setup_ltdc_layer(&mut self, fb: u32, width: u16, height: u16) {
        use stm32h7::stm32h747cm7::ltdc::layer::pfcr::PF;
        let pitch = width * 2; // RGB565
        let layer0 = self.ltdc.layer(0);
        layer0.cfbar().write(|w| unsafe { w.cfbadd().bits(fb) });
        layer0
            .cfblr()
            .write(|w| unsafe { w.cfbll().bits(pitch + 3).cfbp().bits(pitch) });
        layer0
            .cfblnr()
            .write(|w| unsafe { w.cfblnbr().bits(height) });
        layer0.pfcr().write(|w| w.pf().variant(PF::Rgb565));
        layer0.cr().modify(|_, w| w.len().enabled());
        self.ltdc.srcr().write(|w| w.imr().reload());
    }

    #[cfg(all(
        feature = "stm32h747i_disco",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    /// Initialize the external SDRAM and return its base address.
    fn init_sdram(_fmc: FMC, rcc: &mut RCC) -> u32 {
        rcc.ahb3enr().modify(|_, w| w.fmcen().set_bit());
        0xC000_0000
    }
}

impl<B: Blitter> DisplayDriver for Stm32h747iDiscoDisplay<B> {
    fn flush(&mut self, _area: Rect, _colors: &[Color]) {
        todo!("MIPI-DSI flush not yet implemented");
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
