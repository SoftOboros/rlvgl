//! Driver for the ST7789 LCD controller.
#![cfg(feature = "st7789")]
use crate::display::DisplayDriver;
use display_interface::{DataFormat, DisplayError, WriteOnlyDataCommand};
use display_interface_spi::SPIInterface;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;
use rlvgl_core::widget::{Color, Rect};

#[allow(dead_code)]
/// Display driver for the ST7789 LCD controller.
/// ***Note:*** width and height included for custom implementation.
pub struct St7789Display<SPI, DC> {
    interface: SPIInterface<SPI, DC>,
    width: u16,
    height: u16,
}

impl<SPI, DC> St7789Display<SPI, DC>
where
    SPI: SpiDevice,
    DC: OutputPin,
{
    /// Create a new driver instance.
    pub fn new(spi: SPI, dc: DC, width: u16, height: u16) -> Result<Self, DisplayError> {
        let interface = SPIInterface::new(spi, dc);
        Ok(Self {
            interface,
            width,
            height,
        })
    }

    /// Configure the address window for subsequent pixel writes.
    fn set_window(&mut self, area: Rect) -> Result<(), DisplayError> {
        // simplified set column/row addresses
        self.interface.send_commands(DataFormat::U8(&[
            0x2A,
            (area.x >> 8) as u8,
            area.x as u8,
            ((area.x + area.width as i32 - 1) >> 8) as u8,
            (area.x + area.width as i32 - 1) as u8,
        ]))?;
        self.interface.send_commands(DataFormat::U8(&[
            0x2B,
            (area.y >> 8) as u8,
            area.y as u8,
            ((area.y + area.height as i32 - 1) >> 8) as u8,
            (area.y + area.height as i32 - 1) as u8,
        ]))?;
        self.interface.send_commands(DataFormat::U8(&[0x2C]))
    }
}

impl<SPI, DC> DisplayDriver for St7789Display<SPI, DC>
where
    SPI: SpiDevice,
    DC: OutputPin,
{
    /// Write a pixel buffer to the display at the given rectangle.
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        if let Ok(()) = self.set_window(area) {
            let mut buf: [u8; 2] = [0; 2];
            for color in colors {
                buf[0] = ((color.0 as u16 >> 3) << 3 | (color.1 as u16 >> 5)) as u8;
                buf[1] = ((color.1 as u16 & 0b111_000) << 3 | (color.2 as u16 >> 3)) as u8;
                let _ = self.interface.send_data(DataFormat::U8(&buf));
            }
        }
    }
}
