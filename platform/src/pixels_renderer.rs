//! Renderer for drawing rlvgl widgets into a pixel buffer.
//!
//! This helper is used by the desktop simulator to flush the widget tree into
//! the `pixels` frame buffer while providing optional text rendering via the
//! `fontdue` plugin.

use embedded_graphics::{Pixel, pixelcolor::Rgb888, prelude::*};
use rlvgl_core::{
    renderer::Renderer,
    widget::{Color, Rect},
};

#[cfg(feature = "fontdue")]
use rlvgl_core::fontdue::{FontdueRenderTarget, render_text};
#[cfg(feature = "fontdue")]
const FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
#[cfg(not(feature = "fontdue"))]
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    text::Text,
};

/// Renderer that writes RGBA pixels into a mutable frame buffer.
#[derive(Debug)]
pub struct PixelsRenderer<'a> {
    frame: &'a mut [u8],
    width: usize,
    height: usize,
}

impl<'a> PixelsRenderer<'a> {
    /// Create a new renderer for the given frame buffer.
    pub fn new(frame: &'a mut [u8], width: usize, height: usize) -> Self {
        Self {
            frame,
            width,
            height,
        }
    }

    fn put_pixel(&mut self, x: i32, y: i32, color: Rgb888) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let idx = ((y as usize) * self.width + x as usize) * 4;
            self.frame[idx] = color.r();
            self.frame[idx + 1] = color.g();
            self.frame[idx + 2] = color.b();
            self.frame[idx + 3] = 0xff;
        }
    }
}

#[cfg(feature = "fontdue")]
impl<'a> FontdueRenderTarget for PixelsRenderer<'a> {
    fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let idx = ((y as usize) * self.width + x as usize) * 4;
            let bg_r = self.frame[idx];
            let bg_g = self.frame[idx + 1];
            let bg_b = self.frame[idx + 2];
            let inv_alpha = 255 - alpha as u16;
            let r = ((color.0 as u16 * alpha as u16 + bg_r as u16 * inv_alpha) / 255) as u8;
            let g = ((color.1 as u16 * alpha as u16 + bg_g as u16 * inv_alpha) / 255) as u8;
            let b = ((color.2 as u16 * alpha as u16 + bg_b as u16 * inv_alpha) / 255) as u8;
            self.frame[idx] = r;
            self.frame[idx + 1] = g;
            self.frame[idx + 2] = b;
            self.frame[idx + 3] = 0xff;
        }
    }
}

impl<'a> Renderer for PixelsRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rgb = Rgb888::new(color.0, color.1, color.2);
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put_pixel(x, y, rgb);
            }
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        #[cfg(feature = "fontdue")]
        {
            let _ = render_text(self, FONT_DATA, position, text, color, 16.0);
        }
        #[cfg(not(feature = "fontdue"))]
        {
            let style = MonoTextStyle::new(&FONT_6X10, Rgb888::new(color.0, color.1, color.2));
            let _ = Text::new(text, Point::new(position.0, position.1), style).draw(self);
        }
    }
}

impl<'a> DrawTarget for PixelsRenderer<'a> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.put_pixel(point.x, point.y, color);
        }
        Ok(())
    }
}

impl<'a> OriginDimensions for PixelsRenderer<'a> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}
