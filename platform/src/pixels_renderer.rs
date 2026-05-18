//! Renderer for drawing rlvgl widgets into a pixel buffer.
//!
//! This helper is used by the desktop simulator to flush the widget tree into
//! an RGBA frame buffer while providing optional text rendering via the
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
    fn blend_rect(&mut self, rect: Rect, color: Color) {
        let alpha = color.3 as u16;
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_rect(rect, color);
            return;
        }
        let inv = 255 - alpha;
        let x0 = rect.x.max(0) as usize;
        let y0 = rect.y.max(0) as usize;
        let x1 = (rect.x + rect.width).max(0) as usize;
        let y1 = (rect.y + rect.height).max(0) as usize;
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y * self.width + x) * 4;
                let bg_r = self.frame[idx] as u16;
                let bg_g = self.frame[idx + 1] as u16;
                let bg_b = self.frame[idx + 2] as u16;
                self.frame[idx] = ((color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
                self.frame[idx + 1] = ((color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
                self.frame[idx + 2] = ((color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
                self.frame[idx + 3] = 0xff;
            }
        }
    }

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

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        if y < 0 || (y as usize) >= self.height || color.3 == 0 {
            return;
        }
        let row_y = y as usize;
        let row_base = row_y * self.width * 4;
        let src_a = color.3 as u16;
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let px = x + i as i32;
            if px < 0 || (px as usize) >= self.width {
                continue;
            }
            let alpha = (src_a * cov as u16) / 255;
            if alpha == 0 {
                continue;
            }
            let inv = 255 - alpha;
            let idx = row_base + (px as usize) * 4;
            let bg_r = self.frame[idx] as u16;
            let bg_g = self.frame[idx + 1] as u16;
            let bg_b = self.frame[idx + 2] as u16;
            self.frame[idx] = ((color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
            self.frame[idx + 1] = ((color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
            self.frame[idx + 2] = ((color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
            self.frame[idx + 3] = 0xff;
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
