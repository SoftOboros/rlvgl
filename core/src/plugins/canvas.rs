//! Canvas wrapper based on `embedded_canvas`.
use crate::widget::Color;
use alloc::vec;
use alloc::vec::Vec;
use core::iter;
use embedded_canvas::Canvas as EcCanvas;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, Pixel, Point, RgbColor, Size};

/// In-memory pixel buffer that can be drawn on using [`embedded_canvas`].
pub struct Canvas {
    inner: EcCanvas<Rgb888>,
}

impl Canvas {
    /// Create a new canvas with the given dimensions in pixels.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: EcCanvas::new(Size::new(width, height)),
        }
    }

    /// Draw a single pixel at the provided point using the given color.
    pub fn draw_pixel(&mut self, point: Point, color: Color) {
        let rgb = Rgb888::new(color.0, color.1, color.2);
        let _ = self.inner.draw_iter(iter::once(Pixel(point, rgb)));
    }

    /// Return the raw color buffer of the canvas.
    pub fn pixels(&self) -> Vec<Color> {
        self.inner
            .pixels
            .iter()
            .map(|p| match p {
                Some(c) => Color(c.r(), c.g(), c.b()),
                None => Color(0, 0, 0),
            })
            .collect()
    }

    /// Encode the canvas contents into a PNG image.
    #[cfg(feature = "png")]
    pub fn to_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        use png::{ColorType, Encoder};
        let (w, h) = (self.inner.size().width, self.inner.size().height);
        let mut buf = Vec::new();
        {
            let mut encoder = Encoder::new(&mut buf, w, h);
            encoder.set_color(ColorType::Rgb);
            let mut writer = encoder.write_header()?;
            let data: Vec<u8> = self
                .pixels()
                .into_iter()
                .flat_map(|c| vec![c.0, c.1, c.2])
                .collect();
            writer.write_image_data(&data)?;
        }
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_and_get_pixels() {
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_pixel(Point::new(0, 0), Color(255, 0, 0));
        assert_eq!(canvas.pixels(), vec![Color(255, 0, 0)]);
    }

    #[test]
    fn blank_canvas_pixels() {
        let canvas = Canvas::new(1, 1);
        assert_eq!(canvas.pixels(), vec![Color(0, 0, 0)]);
    }

    #[cfg(feature = "png")]
    #[test]
    fn export_png() {
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_pixel(Point::new(0, 0), Color(255, 0, 0));
        let data = canvas.to_png().unwrap();
        let (pixels, w, h) = crate::plugins::png::decode(&data).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(pixels, vec![Color(255, 0, 0)]);
    }
}
