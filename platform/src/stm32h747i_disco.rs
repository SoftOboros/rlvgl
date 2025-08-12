//! STM32H747I-DISCO display and touch driver stubs.
//!
//! Provides placeholder implementations of [`DisplayDriver`] and
//! [`InputDevice`] for the STM32H747I-DISCO discovery board. These stubs
//! establish the module structure for future integration with the board's
//! MIPI-DSI LCD and FT5336 capacitive touch controller.

use crate::{DisplayDriver, InputDevice};
use rlvgl_core::event::Event;
use rlvgl_core::widget::{Color, Rect};

/// Display driver for the STM32H747I-DISCO board.
///
/// This placeholder forwards pixel buffers to the board's MIPI-DSI LCD
/// controller once implemented.
pub struct Stm32h747iDiscoDisplay;

impl DisplayDriver for Stm32h747iDiscoDisplay {
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
