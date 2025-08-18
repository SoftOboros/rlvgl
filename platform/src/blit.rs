//! Basic graphics types and blitter traits for platform backends.
//!
//! These types describe pixel surfaces and operations that can be
//! accelerated by different platform implementations.

#[cfg(any(
    feature = "png",
    feature = "jpeg",
    feature = "qrcode",
    feature = "lottie",
    feature = "canvas",
    feature = "gif",
    feature = "apng",
    feature = "nes",
    feature = "pinyin",
    feature = "fatfs",
    test,
))]
use alloc::vec::Vec;
#[cfg(feature = "fontdue")]
use alloc::{collections::BTreeMap, vec};
use bitflags::bitflags;
use heapless::Vec as HVec;
#[cfg(feature = "fontdue")]
use rlvgl_core::fontdue::{Metrics, line_metrics, rasterize_glyph};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect as WidgetRect};

#[cfg(feature = "fontdue")]
const FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

#[cfg(feature = "fontdue")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Key identifying a cached glyph by font, size, and character.
struct GlyphKey {
    /// Pointer to the font data used to rasterize the glyph.
    font: *const u8,
    /// Font size in pixels, stored as raw bits for ordering.
    size: u32,
    /// Unicode codepoint of the glyph.
    ch: char,
}

/// Supported pixel formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFmt {
    /// 32-bit ARGB8888 format.
    Argb8888,
    /// 16-bit RGB565 format.
    Rgb565,
    /// 8-bit grayscale format.
    L8,
    /// 8-bit alpha-only format.
    A8,
    /// 4-bit alpha-only format.
    A4,
}

/// Rectangular region within a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left coordinate of the rectangle.
    pub x: i32,
    /// Top coordinate of the rectangle.
    pub y: i32,
    /// Width of the rectangle in pixels.
    pub w: u32,
    /// Height of the rectangle in pixels.
    pub h: u32,
}

/// A pixel buffer with dimension and format metadata.
pub struct Surface<'a> {
    /// Underlying pixel storage.
    pub buf: &'a mut [u8],
    /// Number of bytes between consecutive lines.
    pub stride: usize,
    /// Pixel format used by the buffer.
    pub format: PixelFmt,
    /// Width of the surface in pixels.
    pub width: u32,
    /// Height of the surface in pixels.
    pub height: u32,
}

impl<'a> Surface<'a> {
    /// Create a new surface from raw parts.
    pub fn new(
        buf: &'a mut [u8],
        stride: usize,
        format: PixelFmt,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            buf,
            stride,
            format,
            width,
            height,
        }
    }
}

bitflags! {
    /// Capabilities supported by a blitter implementation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BlitCaps: u32 {
        /// Ability to fill regions with a solid color.
        const FILL = 0b0001;
        /// Ability to copy pixels between surfaces.
        const BLIT = 0b0010;
        /// Ability to blend a source over a destination.
        const BLEND = 0b0100;
        /// Ability to convert between pixel formats.
        const PFC = 0b1000;
    }
}

/// Trait implemented by types capable of transferring pixel data.
pub trait Blitter {
    /// Return the capabilities supported by this blitter.
    fn caps(&self) -> BlitCaps;

    /// Fill `area` within `dst` with a solid `color`.
    fn fill(&mut self, dst: &mut Surface, area: Rect, color: u32);

    /// Copy pixels from `src` within `src_area` to `dst` at `dst_pos`.
    fn blit(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32));

    /// Blend pixels from `src` over `dst`.
    fn blend(&mut self, src: &Surface, src_area: Rect, dst: &mut Surface, dst_pos: (i32, i32));
}

/// Collects dirty rectangles for a frame and optionally coalesces them.
///
/// The planner stores up to `N` rectangles in a stack-allocated buffer. Call
/// [`add`] to register a region that changed during rendering and [`rects`] to
/// obtain the batched list for flushing. After presenting the frame, call
/// [`clear`] to reuse the planner for the next frame.
pub struct BlitPlanner<const N: usize> {
    rects: HVec<Rect, N>,
}

impl<const N: usize> BlitPlanner<N> {
    /// Create an empty planner.
    pub fn new() -> Self {
        Self { rects: HVec::new() }
    }

    /// Record a dirty rectangle.
    pub fn add(&mut self, rect: Rect) {
        let _ = self.rects.push(rect);
    }

    /// Return all accumulated rectangles.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Remove all stored rectangles.
    pub fn clear(&mut self) {
        self.rects.clear();
    }
}

impl<const N: usize> Default for BlitPlanner<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Renderer implementation backed by a [`Blitter`].
///
/// A `BlitterRenderer` owns a target [`Surface`] and batches dirty regions
/// using a [`BlitPlanner`]. Widgets interact with the generic [`Renderer`] trait
/// without being aware of the underlying blitter.
pub struct BlitterRenderer<'a, B: Blitter, const N: usize> {
    blitter: &'a mut B,
    surface: Surface<'a>,
    planner: BlitPlanner<N>,
    #[cfg(any(
        feature = "png",
        feature = "jpeg",
        feature = "qrcode",
        feature = "lottie",
        feature = "canvas",
        feature = "gif",
        feature = "apng",
        feature = "nes",
        feature = "pinyin",
        feature = "fatfs",
        test,
    ))]
    scratch: Option<Vec<u8>>,
    #[cfg(feature = "fontdue")]
    glyph_cache: BTreeMap<GlyphKey, (Metrics, Vec<u8>)>,
}

impl<'a, B: Blitter, const N: usize> BlitterRenderer<'a, B, N> {
    /// Create a new renderer targeting `surface` using `blitter`.
    pub fn new(blitter: &'a mut B, surface: Surface<'a>) -> Self {
        Self {
            blitter,
            surface,
            planner: BlitPlanner::new(),
            #[cfg(any(
                feature = "png",
                feature = "jpeg",
                feature = "qrcode",
                feature = "lottie",
                feature = "canvas",
                feature = "gif",
                feature = "apng",
                feature = "nes",
                feature = "pinyin",
                feature = "fatfs",
                test,
            ))]
            scratch: None,
            #[cfg(feature = "fontdue")]
            glyph_cache: BTreeMap::new(),
        }
    }

    /// Access the internal dirty-rectangle planner.
    pub fn planner(&mut self) -> &mut BlitPlanner<N> {
        &mut self.planner
    }

    #[cfg(any(
        feature = "png",
        feature = "jpeg",
        feature = "qrcode",
        feature = "lottie",
        feature = "canvas",
        feature = "gif",
        feature = "apng",
        feature = "nes",
        feature = "pinyin",
        feature = "fatfs",
        test,
    ))]
    fn blit_colors(&mut self, position: (i32, i32), pixels: &[Color], w: u32, h: u32) {
        let required = (w * h * 4) as usize;
        let buf = self.scratch.get_or_insert_with(Vec::new);
        if buf.len() < required {
            buf.resize(required, 0);
        }
        for (i, c) in pixels.iter().enumerate() {
            buf[i * 4..i * 4 + 4].copy_from_slice(&c.to_argb8888().to_le_bytes());
        }
        let src = Surface::new(
            &mut buf[..required],
            (w * 4) as usize,
            PixelFmt::Argb8888,
            w,
            h,
        );
        self.blitter
            .blit(&src, Rect { x: 0, y: 0, w, h }, &mut self.surface, position);
        self.planner.add(Rect {
            x: position.0,
            y: position.1,
            w,
            h,
        });
    }

    #[cfg(feature = "png")]
    /// Decode a PNG image and blit it onto the target surface.
    pub fn draw_png(
        &mut self,
        position: (i32, i32),
        data: &[u8],
    ) -> Result<(), rlvgl_core::png::DecodingError> {
        let (pixels, w, h) = rlvgl_core::png::decode(data)?;
        self.blit_colors(position, &pixels, w, h);
        Ok(())
    }

    #[cfg(feature = "jpeg")]
    /// Decode a JPEG image and blit it onto the target surface.
    pub fn draw_jpeg(
        &mut self,
        position: (i32, i32),
        data: &[u8],
    ) -> Result<(), rlvgl_core::jpeg::Error> {
        let (pixels, w, h) = rlvgl_core::jpeg::decode(data)?;
        self.blit_colors(position, &pixels, w as u32, h as u32);
        Ok(())
    }

    #[cfg(feature = "qrcode")]
    /// Generate a QR code from `data` and blit it onto the target surface.
    pub fn draw_qr(
        &mut self,
        position: (i32, i32),
        data: &[u8],
    ) -> Result<(), rlvgl_core::qrcode::QrError> {
        let (pixels, w, h) = rlvgl_core::qrcode::generate(data)?;
        self.blit_colors(position, &pixels, w, h);
        Ok(())
    }

    #[cfg(feature = "lottie")]
    /// Render a Lottie JSON animation frame and blit it onto the target surface.
    ///
    /// Returns `true` if the frame was rendered successfully.
    pub fn draw_lottie_frame(
        &mut self,
        position: (i32, i32),
        json: &str,
        frame: usize,
        width: u32,
        height: u32,
    ) -> bool {
        if let Some(pixels) =
            rlvgl_core::lottie::render_lottie_frame(json, frame, width as usize, height as usize)
        {
            self.blit_colors(position, &pixels, width, height);
            true
        } else {
            false
        }
    }

    #[cfg(feature = "canvas")]
    /// Blit an [`rlvgl_core::canvas::Canvas`] onto the target surface.
    pub fn draw_canvas(&mut self, position: (i32, i32), canvas: &rlvgl_core::canvas::Canvas) {
        let (w, h) = canvas.size();
        let pixels = canvas.pixels();
        self.blit_colors(position, &pixels, w, h);
    }

    #[cfg(feature = "gif")]
    /// Decode a GIF and blit the selected frame onto the target surface.
    pub fn draw_gif_frame(
        &mut self,
        position: (i32, i32),
        data: &[u8],
        frame: usize,
    ) -> Result<(), rlvgl_core::gif::DecodingError> {
        let (frames, w, h) = rlvgl_core::gif::decode(data)?;
        if let Some(f) = frames.get(frame) {
            self.blit_colors(position, &f.pixels, w as u32, h as u32);
        }
        Ok(())
    }

    #[cfg(feature = "apng")]
    /// Decode an APNG and blit the selected frame onto the target surface.
    pub fn draw_apng_frame(
        &mut self,
        position: (i32, i32),
        data: &[u8],
        frame: usize,
    ) -> Result<(), image::ImageError> {
        let (frames, w, h) = rlvgl_core::apng::decode(data)?;
        if let Some(f) = frames.get(frame) {
            self.blit_colors(position, &f.pixels, w, h);
        }
        Ok(())
    }

    #[cfg(all(feature = "pinyin", feature = "fontdue"))]
    /// Render Pinyin IME candidate characters via the blitter.
    ///
    /// Returns `true` if any candidates were rendered for `input`.
    pub fn draw_pinyin_candidates(
        &mut self,
        position: (i32, i32),
        ime: &rlvgl_core::pinyin::PinyinInputMethod,
        input: &str,
        color: Color,
    ) -> bool {
        if let Some(chars) = ime.candidates(input) {
            let text: alloc::string::String = chars.into_iter().collect();
            Renderer::draw_text(self, position, &text, color);
            true
        } else {
            false
        }
    }

    #[cfg(all(feature = "fatfs", feature = "fontdue"))]
    /// List a FAT directory and render the entries line by line.
    pub fn draw_fatfs_dir<T>(
        &mut self,
        position: (i32, i32),
        image: &mut T,
        dir: &str,
        color: Color,
    ) -> Result<(), std::io::Error>
    where
        T: std::io::Read + std::io::Write + std::io::Seek,
    {
        let names = rlvgl_core::fatfs::list_dir(image, dir)?;
        for (i, name) in names.iter().enumerate() {
            let y = position.1 + (i as i32) * 16;
            Renderer::draw_text(self, (position.0, y), name, color);
        }
        Ok(())
    }

    #[cfg(feature = "nes")]
    /// Blit an NES frame represented as ARGB8888 [`Color`] pixels.
    pub fn draw_nes_frame(
        &mut self,
        position: (i32, i32),
        pixels: &[Color],
        width: u32,
        height: u32,
    ) {
        self.blit_colors(position, pixels, width, height);
    }

    #[cfg(feature = "fontdue")]
    /// Draw UTF-8 text using the supplied font and size.
    pub fn draw_text(
        &mut self,
        position: (i32, i32),
        text: &str,
        color: Color,
        font_data: &[u8],
        px: f32,
    ) {
        let vm = line_metrics(font_data, px).unwrap();
        let ascent = vm.ascent.round() as i32;
        let baseline = position.1 + ascent;
        let mut x_cursor = position.0;
        for ch in text.chars() {
            let key = GlyphKey {
                font: font_data.as_ptr(),
                size: px.to_bits(),
                ch,
            };
            let (metrics, bitmap) = {
                let entry = self
                    .glyph_cache
                    .entry(key)
                    .or_insert_with(|| rasterize_glyph(font_data, ch, px).unwrap());
                (entry.0, entry.1.clone())
            };
            let w = metrics.width as i32;
            let h = metrics.height as i32;
            if w == 0 || h == 0 {
                x_cursor += metrics.advance_width.round() as i32;
                continue;
            }
            let mut argb = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let alpha = bitmap[(h - 1 - y) as usize * metrics.width + x as usize];
                    let idx = ((y * w + x) * 4) as usize;
                    argb[idx] = (color.0 as u16 * alpha as u16 / 255) as u8;
                    argb[idx + 1] = (color.1 as u16 * alpha as u16 / 255) as u8;
                    argb[idx + 2] = (color.2 as u16 * alpha as u16 / 255) as u8;
                    argb[idx + 3] = alpha;
                }
            }
            let src = Surface::new(
                argb.as_mut_slice(),
                (w * 4) as usize,
                PixelFmt::Argb8888,
                w as u32,
                h as u32,
            );
            let dst_pos = (
                x_cursor + metrics.xmin,
                baseline - ascent - metrics.ymin - (h - 1),
            );
            self.blitter.blend(
                &src,
                Rect {
                    x: 0,
                    y: 0,
                    w: w as u32,
                    h: h as u32,
                },
                &mut self.surface,
                dst_pos,
            );
            self.planner.add(Rect {
                x: dst_pos.0,
                y: dst_pos.1,
                w: w as u32,
                h: h as u32,
            });
            x_cursor += metrics.advance_width.round() as i32;
        }
    }

    #[cfg(not(feature = "fontdue"))]
    /// Stub text renderer when fontdue is disabled.
    pub fn draw_text(
        &mut self,
        position: (i32, i32),
        text: &str,
        color: Color,
        _font_data: &[u8],
        _px: f32,
    ) {
        let _ = (position, text, color);
    }
}

impl<B: Blitter, const N: usize> Renderer for BlitterRenderer<'_, B, N> {
    fn fill_rect(&mut self, rect: WidgetRect, color: Color) {
        let r = Rect {
            x: rect.x,
            y: rect.y,
            w: rect.width as u32,
            h: rect.height as u32,
        };
        self.planner.add(r);
        self.blitter.fill(&mut self.surface, r, color.to_argb8888());
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        #[cfg(feature = "fontdue")]
        {
            const PX: f32 = 16.0;
            BlitterRenderer::draw_text(self, position, text, color, FONT_DATA, PX);
        }
        #[cfg(not(feature = "fontdue"))]
        {
            let _ = (position, text, color);
        }
    }
}

#[cfg(test)]
mod scratch_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;

    #[test]
    fn blit_colors_reuses_scratch_buffer() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let pixels = [Color(0, 0, 0, 0)];
        renderer.blit_colors((0, 0), &pixels, 1, 1);
        let first_ptr = renderer.scratch.as_ref().unwrap().as_ptr();
        renderer.blit_colors((1, 1), &pixels, 1, 1);
        let second_ptr = renderer.scratch.as_ref().unwrap().as_ptr();
        assert_eq!(first_ptr, second_ptr);
    }
}

#[cfg(all(test, feature = "fontdue"))]
mod text_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;

    #[test]
    fn blitter_draws_text() {
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        Renderer::draw_text(&mut renderer, (0, 32), "A", Color(255, 255, 255, 255));
        assert!(buf.iter().any(|&p| p != 0));
    }

    #[test]
    fn cache_accounts_for_size() {
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_text((0, 32), "Hi", Color(255, 255, 255, 255), FONT_DATA, 16.0);
        let len_after_small = renderer.glyph_cache.len();
        renderer.draw_text((0, 32), "Hi", Color(255, 255, 255, 255), FONT_DATA, 16.0);
        assert_eq!(len_after_small, renderer.glyph_cache.len());
        renderer.draw_text((0, 32), "Hi", Color(255, 255, 255, 255), FONT_DATA, 24.0);
        assert!(renderer.glyph_cache.len() > len_after_small);
    }
}

#[cfg(all(test, feature = "png"))]
mod png_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;

    const RED_DOT_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

    #[test]
    fn blitter_draws_png() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_PNG)
            .unwrap();
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_png((0, 0), &data).unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "jpeg"))]
mod jpeg_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;

    const RED_DOT_JPEG: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDi6KKK+ZP3E//Z";

    #[test]
    fn blitter_draws_jpeg() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_JPEG)
            .unwrap();
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_jpeg((0, 0), &data).unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "gif"))]
mod gif_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;

    const RED_DOT_GIF: &str = "R0lGODdhAQABAPAAAP8AAP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==";

    #[test]
    fn blitter_draws_gif() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_GIF)
            .unwrap();
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_gif_frame((0, 0), &data, 0).unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "canvas"))]
mod canvas_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use embedded_graphics::prelude::Point;
    use rlvgl_core::canvas::Canvas;

    #[test]
    fn blitter_draws_canvas() {
        let mut canvas = Canvas::new(1, 1);
        canvas.draw_pixel(Point::new(0, 0), Color(255, 0, 0, 255));
        let mut buf = [0u8; 4];
        let surface = Surface::new(&mut buf, 4, PixelFmt::Argb8888, 1, 1);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_canvas((0, 0), &canvas);
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "qrcode"))]
mod qrcode_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;

    #[test]
    fn blitter_draws_qr() {
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_qr((0, 0), b"hi").unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "lottie"))]
mod lottie_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;

    const SIMPLE_JSON: &str =
        "{\"v\":\"5.7\",\"fr\":30,\"ip\":0,\"op\":0,\"w\":1,\"h\":1,\"layers\":[]}";

    #[test]
    fn blitter_draws_lottie() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        assert!(renderer.draw_lottie_frame((0, 0), SIMPLE_JSON, 0, 1, 1));
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "pinyin", feature = "fontdue"))]
mod pinyin_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use rlvgl_core::pinyin::PinyinInputMethod;

    #[test]
    fn blitter_draws_pinyin() {
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let ime = PinyinInputMethod;
        assert!(renderer.draw_pinyin_candidates((0, 0), &ime, "zhong", Color(255, 255, 255, 255)));
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "fatfs", feature = "fontdue"))]
mod fatfs_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use fatfs::{FileSystem, FormatVolumeOptions, FsOptions};
    use fscommon::BufStream;
    use std::io::{Cursor, Seek, SeekFrom, Write};

    #[test]
    fn blitter_draws_fatfs_listing() {
        let mut img = Cursor::new(vec![0u8; 1024 * 512]);
        fatfs::format_volume(&mut img, FormatVolumeOptions::new()).unwrap();
        img.seek(SeekFrom::Start(0)).unwrap();
        {
            let buf_stream = BufStream::new(&mut img);
            let fs = FileSystem::new(buf_stream, FsOptions::new()).unwrap();
            fs.root_dir()
                .create_file("foo.txt")
                .unwrap()
                .write_all(b"hi")
                .unwrap();
        }
        img.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer
            .draw_fatfs_dir((0, 0), &mut img, "/", Color(255, 255, 255, 255))
            .unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }
}

#[cfg(all(test, feature = "nes"))]
mod nes_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;

    #[test]
    fn blitter_draws_nes_frame() {
        let pixels = [Color(255, 0, 0, 255)];
        let mut buf = [0u8; 4];
        let surface = Surface::new(&mut buf, 4, PixelFmt::Argb8888, 1, 1);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_nes_frame((0, 0), &pixels, 1, 1);
        assert!(buf.iter().any(|&p| p != 0));
    }

    #[test]
    fn blitter_draws_full_nes_frame() {
        let mut pixels = [Color(0, 0, 0, 255); 256 * 240];
        for y in 0..240 {
            for x in 0..256 {
                pixels[y * 256 + x] = Color(x as u8, y as u8, 0, 255);
            }
        }
        let mut buf = [0u8; 256 * 240 * 4];
        let surface = Surface::new(&mut buf, 256 * 4, PixelFmt::Argb8888, 256, 240);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_nes_frame((0, 0), &pixels, 256, 240);
        let x = 128usize;
        let y = 120usize;
        let idx = (y * 256 + x) * 4;
        let actual = u32::from_le_bytes(buf[idx..idx + 4].try_into().unwrap());
        let expected = Color(x as u8, y as u8, 0, 255).to_argb8888();
        assert_eq!(actual, expected);
    }
}
