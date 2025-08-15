//! STM32H747I-DISCO display and touch driver stubs.
//!
//! Provides placeholder implementations of [`DisplayDriver`] and
//! [`InputDevice`] for the STM32H747I-DISCO discovery board. These stubs
//! establish the module structure for future integration with the board's
//! MIPI-DSI LCD and FT5336 capacitive touch controller. Basic backlight PWM
//! and panel reset control are wired through `embedded-hal` traits.

use crate::{Blitter, DisplayDriver, InputDevice};
#[cfg(feature = "stm32h747i_disco")]
use embedded_hal::{digital::OutputPin, pwm::PwmPin};
use rlvgl_core::event::Event;
use rlvgl_core::widget::{Color, Rect};
#[cfg(all(
    feature = "stm32h747i_disco",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use stm32h7::stm32h747cm7::{DSIHOST, LTDC, RCC};

/// Display driver for the STM32H747I-DISCO board.
///
/// Wraps a [`Blitter`] and configures LTDC/DSI clocks. The actual flush path is
/// still unimplemented and will eventually transfer pixel data over MIPI-DSI.
pub struct Stm32h747iDiscoDisplay<B: Blitter, BL = (), RST = ()> {
    blitter: B,
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
        rcc: &mut RCC,
    ) -> Self
    where
        BL: PwmPin,
        RST: OutputPin,
    {
        // Enable LTDC and DSI peripheral clocks
        rcc.apb3enr
            .modify(|_, w| w.ltdcen().set_bit().dsien().set_bit());
        // Ensure the panel is held in reset and the backlight is off
        let _ = reset.set_low();
        let mut disp = Self {
            blitter,
            backlight,
            reset,
            ltdc,
            dsi,
        };
        disp.backlight.disable();
        disp.reset_panel();
        disp
    }

    #[cfg(feature = "stm32h747i_disco")]
    fn set_backlight(&mut self, level: BL::Duty)
    where
        BL: PwmPin,
    {
        self.backlight.set_duty(level);
        self.backlight.enable();
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
}

impl<B: Blitter> DisplayDriver for Stm32h747iDiscoDisplay<B> {
    fn flush(&mut self, _area: Rect, _colors: &[Color]) {
        todo!("MIPI-DSI flush not yet implemented");
    }
}

/// Touch input driver for the STM32H747I-DISCO board.
///
/// Polls the FT5336 capacitive controller over I²C when fully implemented.
pub struct Stm32h747iDiscoInput;

impl InputDevice for Stm32h747iDiscoInput {
    fn poll(&mut self) -> Option<Event> {
        todo!("touch polling not yet implemented");
    }
}
