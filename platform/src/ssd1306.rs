//! Driver adapter for SSD1306 monochrome OLEDs (128x64 / 128x32).
//!
//! Wraps a [`ssd1306::Ssd1306`] in `BufferedGraphicsMode` and exposes it as a
//! rlvgl [`DisplayDriver`] by threshold-quantizing incoming `Color` pixels
//! (RGBA → 1 bpp) before drawing them into the ssd1306 crate's framebuffer.
//! Flushing to the panel happens in [`DisplayDriver::vsync`].

use crate::display::DisplayDriver;
use crate::screen::Screen;
use display_interface::{DisplayError, WriteOnlyDataCommand};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::text::Text;
use embedded_graphics::{Pixel, pixelcolor::BinaryColor, prelude::*};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use ssd1306::Ssd1306;
use ssd1306::mode::{BufferedGraphicsMode, DisplayConfig};
use ssd1306::prelude::DisplaySize;

/// rlvgl display driver wrapping a buffered SSD1306.
///
/// The user is expected to construct the underlying [`Ssd1306`] in
/// [`BufferedGraphicsMode`] themselves (so the example crate owns the I2C /
/// SPI bus type), and hand it to [`Ssd1306Display::new`] which initializes
/// the panel.
pub struct Ssd1306Display<DI, SIZE>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
{
    display: Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>,
    width: u16,
    height: u16,
}

impl<DI, SIZE> Ssd1306Display<DI, SIZE>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
{
    /// Initialize the panel and wrap it in a rlvgl-compatible driver.
    pub fn new(
        mut display: Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>,
    ) -> Result<Self, DisplayError> {
        display.init()?;
        let (w, h) = display.dimensions();
        Ok(Self {
            display,
            width: w as u16,
            height: h as u16,
        })
    }

    /// Release the inner ssd1306 driver (e.g. to swap modes).
    pub fn release(self) -> Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>> {
        self.display
    }
}

fn quantize(c: Color) -> BinaryColor {
    if c.3 < 128 {
        return BinaryColor::Off;
    }
    let luma = (c.0 as u16 + c.1 as u16 + c.2 as u16) / 3;
    if luma >= 128 {
        BinaryColor::On
    } else {
        BinaryColor::Off
    }
}

impl<DI, SIZE> DisplayDriver for Ssd1306Display<DI, SIZE>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
{
    fn screen(&self) -> Screen {
        Screen::landscape(self.width as u32, self.height as u32)
    }

    fn flush(&mut self, area: Rect, colors: &[Color]) {
        let max_x = self.width as i32;
        let max_y = self.height as i32;
        for y in 0..area.height {
            for x in 0..area.width {
                let idx = (y * area.width + x) as usize;
                let Some(&c) = colors.get(idx) else {
                    continue;
                };
                let px = area.x + x;
                let py = area.y + y;
                if px < 0 || py < 0 || px >= max_x || py >= max_y {
                    continue;
                }
                let _ = Pixel(Point::new(px, py), quantize(c)).draw(&mut self.display);
            }
        }
    }

    fn vsync(&mut self) {
        let _ = self.display.flush();
    }
}

impl<DI, SIZE> Renderer for Ssd1306Display<DI, SIZE>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
{
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let c = quantize(color);
        let max_x = self.width as i32;
        let max_y = self.height as i32;
        for y in 0..rect.height {
            let py = rect.y + y;
            if py < 0 || py >= max_y {
                continue;
            }
            for x in 0..rect.width {
                let px = rect.x + x;
                if px < 0 || px >= max_x {
                    continue;
                }
                let _ = Pixel(Point::new(px, py), c).draw(&mut self.display);
            }
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        let style = MonoTextStyle::new(&FONT_6X10, quantize(color));
        let _ = Text::new(text, Point::new(position.0, position.1), style).draw(&mut self.display);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Color;

    #[test]
    fn quantize_transparent_is_off() {
        assert_eq!(quantize(Color(255, 255, 255, 0)), BinaryColor::Off);
        assert_eq!(quantize(Color(255, 255, 255, 127)), BinaryColor::Off);
    }

    #[test]
    fn quantize_bright_is_on() {
        assert_eq!(quantize(Color(255, 255, 255, 255)), BinaryColor::On);
        assert_eq!(quantize(Color(200, 200, 200, 200)), BinaryColor::On);
    }

    #[test]
    fn quantize_dark_is_off() {
        assert_eq!(quantize(Color(0, 0, 0, 255)), BinaryColor::Off);
        assert_eq!(quantize(Color(60, 60, 60, 255)), BinaryColor::Off);
    }
}
