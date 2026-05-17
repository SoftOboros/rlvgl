//! Basic graphics types and blitter traits for platform backends.
//!
//! These types describe pixel surfaces and operations that can be
//! accelerated by different platform implementations.

#[cfg(any(
    feature = "canvas",
    feature = "gif",
    feature = "apng",
    feature = "nes",
    feature = "png",
    feature = "jpeg",
    feature = "qrcode",
    feature = "lottie",
    feature = "fontdue",
    test,
))]
use alloc::vec::Vec;
#[cfg(feature = "fontdue")]
use alloc::{collections::BTreeMap, vec};
use bitflags::bitflags;
use heapless::Vec as HVec;
use rlvgl_core::cmd::{BlendMode, Cmd, CommandList};
#[cfg(feature = "fontdue")]
use rlvgl_core::fontdue::{Metrics, line_metrics, rasterize_glyph};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect as WidgetRect};

#[cfg(feature = "fontdue")]
const FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

#[cfg(feature = "fontdue")]
fn round_to_i32(value: f32) -> i32 {
    if value.is_nan() {
        0
    } else if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}

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
/// [`Self::add`] to register a region that changed during rendering and
/// [`Self::rects`] to obtain the batched list for flushing. After presenting
/// the frame, call [`Self::clear`] to reuse the planner for the next frame.
///
/// Adds beyond `N` are silently dropped, but [`Self::overflowed`] flips to
/// `true` so callers driving a dirty-rect present path know the rect set is
/// incomplete and can fall back to a full-frame repaint for that frame.
pub struct BlitPlanner<const N: usize> {
    rects: HVec<Rect, N>,
    overflowed: bool,
}

impl<const N: usize> BlitPlanner<N> {
    /// Create an empty planner.
    pub fn new() -> Self {
        Self {
            rects: HVec::new(),
            overflowed: false,
        }
    }

    /// Record a dirty rectangle.
    pub fn add(&mut self, rect: Rect) {
        if self.rects.push(rect).is_err() {
            self.overflowed = true;
        }
    }

    /// Return all accumulated rectangles.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Whether [`Self::add`] dropped a rect because the planner was full.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// Remove all stored rectangles.
    pub fn clear(&mut self) {
        self.rects.clear();
        self.overflowed = false;
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
        feature = "canvas",
        feature = "gif",
        feature = "apng",
        feature = "nes",
        all(feature = "png", not(target_os = "none")),
        all(feature = "jpeg", not(target_os = "none")),
        all(feature = "qrcode", not(target_os = "none")),
        feature = "lottie",
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
                feature = "canvas",
                feature = "gif",
                feature = "apng",
                feature = "nes",
                all(feature = "png", not(target_os = "none")),
                all(feature = "jpeg", not(target_os = "none")),
                all(feature = "qrcode", not(target_os = "none")),
                feature = "lottie",
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
        feature = "canvas",
        feature = "gif",
        feature = "apng",
        feature = "nes",
        all(feature = "png", not(target_os = "none")),
        all(feature = "jpeg", not(target_os = "none")),
        all(feature = "qrcode", not(target_os = "none")),
        feature = "lottie",
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

    #[cfg(all(feature = "png", not(target_os = "none")))]
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

    #[cfg(all(feature = "jpeg", not(target_os = "none")))]
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

    #[cfg(all(feature = "qrcode", not(target_os = "none")))]
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
    /// Returns an error if the JSON data is invalid.
    pub fn draw_lottie_frame(
        &mut self,
        position: (i32, i32),
        json: &str,
        frame: usize,
        width: u32,
        height: u32,
    ) -> Result<(), rlvgl_core::lottie::Error> {
        let pixels =
            rlvgl_core::lottie::render_lottie_frame(json, frame, width as usize, height as usize)?;
        self.blit_colors(position, &pixels, width, height);
        Ok(())
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
            // Determine remaining space on the surface.
            let max_w = self.surface.width as i32 - position.0;
            let max_h = self.surface.height as i32 - position.1;
            if max_w <= 0 || max_h < 16 {
                return false;
            }

            // Truncate the candidate string to fit within the surface width.
            let text: alloc::string::String = chars.into_iter().collect();
            let max_chars = (max_w / 16) as usize;
            let clipped: alloc::string::String = text.chars().take(max_chars).collect();
            if clipped.is_empty() {
                return false;
            }
            Renderer::draw_text(self, position, &clipped, color);
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
        let max_w = self.surface.width as i32 - position.0;
        let max_h = self.surface.height as i32 - position.1;
        if max_w <= 0 || max_h <= 0 {
            return Ok(());
        }
        let line_h = 16;
        let max_lines = (max_h / line_h) as usize;
        let max_chars = (max_w / 16) as usize;
        let names = rlvgl_core::fatfs::list_dir(image, dir)?;
        for (i, name) in names.iter().take(max_lines).enumerate() {
            let y = position.1 + (i as i32) * line_h;
            let clipped: alloc::string::String = name.chars().take(max_chars).collect();
            if clipped.is_empty() {
                break;
            }
            Renderer::draw_text(self, (position.0, y), &clipped, color);
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
        let ascent = round_to_i32(vm.ascent);
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
                x_cursor += round_to_i32(metrics.advance_width);
                continue;
            }
            let mut argb = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let alpha = bitmap[(y) as usize * metrics.width + x as usize];
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
            x_cursor += round_to_i32(metrics.advance_width);
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
    fn blend_rect(&mut self, rect: WidgetRect, color: Color) {
        let alpha = color.3 as u16;
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_rect(rect, color);
            return;
        }
        // Inline source-over blending for ARGB8888 surfaces.
        if self.surface.format == PixelFmt::Argb8888 {
            let sw = self.surface.width as i32;
            let sh = self.surface.height as i32;
            let stride = self.surface.stride;
            let inv = 255 - alpha;
            let x0 = rect.x.max(0);
            let y0 = rect.y.max(0);
            let x1 = (rect.x + rect.width).min(sw);
            let y1 = (rect.y + rect.height).min(sh);
            for y in y0..y1 {
                for x in x0..x1 {
                    let off = y as usize * stride + x as usize * 4;
                    let bg_b = self.surface.buf[off] as u16;
                    let bg_g = self.surface.buf[off + 1] as u16;
                    let bg_r = self.surface.buf[off + 2] as u16;
                    self.surface.buf[off] = ((color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
                    self.surface.buf[off + 1] = ((color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
                    self.surface.buf[off + 2] = ((color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
                    self.surface.buf[off + 3] = 0xff;
                }
            }
            self.planner.add(Rect {
                x: rect.x,
                y: rect.y,
                w: rect.width as u32,
                h: rect.height as u32,
            });
        } else {
            // Fallback for non-ARGB8888 surfaces: just overwrite.
            self.fill_rect(rect, color);
        }
    }

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

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        // Fast path: write ARGB8888 pixels directly into the surface buffer,
        // then record the dirty rectangle. Avoids per-pixel fill_rect overhead.
        if self.surface.format == PixelFmt::Argb8888 {
            let sw = self.surface.width as i32;
            let sh = self.surface.height as i32;
            let stride = self.surface.stride;
            for y in 0..height as i32 {
                let dy = position.1 + y;
                if dy < 0 || dy >= sh {
                    continue;
                }
                for x in 0..width as i32 {
                    let dx = position.0 + x;
                    if dx < 0 || dx >= sw {
                        continue;
                    }
                    let src_idx = (y as u32 * width + x as u32) as usize;
                    if let Some(&c) = pixels.get(src_idx) {
                        let off = dy as usize * stride + dx as usize * 4;
                        self.surface.buf[off..off + 4]
                            .copy_from_slice(&c.to_argb8888().to_le_bytes());
                    }
                }
            }
            self.planner.add(Rect {
                x: position.0,
                y: position.1,
                w: width,
                h: height,
            });
        } else {
            // Fallback: per-pixel fill_rect for non-ARGB8888 surfaces
            for y in 0..height as i32 {
                for x in 0..width as i32 {
                    let idx = (y as u32 * width + x as u32) as usize;
                    if let Some(&c) = pixels.get(idx) {
                        self.fill_rect(
                            WidgetRect {
                                x: position.0 + x,
                                y: position.1 + y,
                                width: 1,
                                height: 1,
                            },
                            c,
                        );
                    }
                }
            }
        }
    }

    fn blend_row(&mut self, x: i32, y: i32, color: Color, coverage: &[u8]) {
        // AA inner-loop primitive. Direct ARGB8888 source-over with per-pixel
        // alpha = (color.alpha * coverage[i]) / 255. One planner.add per row
        // keeps DMA2D refresh granularity efficient versus per-pixel adds.
        if color.3 == 0 || coverage.is_empty() {
            return;
        }
        if self.surface.format != PixelFmt::Argb8888 {
            // Non-ARGB8888 surfaces: fall back through the trait default
            // (per-pixel blend_rect). This branch is rare on disco.
            for (i, &cov) in coverage.iter().enumerate() {
                if cov == 0 {
                    continue;
                }
                let alpha = ((color.3 as u16 * cov as u16) / 255) as u8;
                if alpha == 0 {
                    continue;
                }
                self.blend_rect(
                    WidgetRect {
                        x: x + i as i32,
                        y,
                        width: 1,
                        height: 1,
                    },
                    Color(color.0, color.1, color.2, alpha),
                );
            }
            return;
        }
        let sw = self.surface.width as i32;
        let sh = self.surface.height as i32;
        if y < 0 || y >= sh {
            return;
        }
        let stride = self.surface.stride;
        let src_a = color.3 as u16;
        let row_off = y as usize * stride;
        let mut written_x0 = i32::MAX;
        let mut written_x1 = i32::MIN;
        for (i, &cov) in coverage.iter().enumerate() {
            if cov == 0 {
                continue;
            }
            let px = x + i as i32;
            if px < 0 || px >= sw {
                continue;
            }
            let alpha = (src_a * cov as u16) / 255;
            if alpha == 0 {
                continue;
            }
            let inv = 255 - alpha;
            let off = row_off + (px as usize) * 4;
            let bg_b = self.surface.buf[off] as u16;
            let bg_g = self.surface.buf[off + 1] as u16;
            let bg_r = self.surface.buf[off + 2] as u16;
            self.surface.buf[off] = ((color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
            self.surface.buf[off + 1] = ((color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
            self.surface.buf[off + 2] = ((color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
            self.surface.buf[off + 3] = 0xff;
            if px < written_x0 {
                written_x0 = px;
            }
            if px > written_x1 {
                written_x1 = px;
            }
        }
        if written_x0 <= written_x1 {
            self.planner.add(Rect {
                x: written_x0,
                y,
                w: (written_x1 - written_x0 + 1) as u32,
                h: 1,
            });
        }
    }

    fn submit(&mut self, list: &CommandList) {
        submit_with_occlusion(self, list);
    }
}

/// `CommandSink` impl with occlusion pre-pass.
///
/// Backend-specialized submit path: walks the [`CommandList`] and, for
/// each cmd, checks whether any *later* opaque [`Cmd::FillRect`] fully
/// covers its AABB. If so, the cmd is skipped entirely — it would be
/// overwritten anyway. Survivors are dispatched via the normal
/// [`Renderer`] trait methods on `self` (existing
/// [`BlitterRenderer::blend_row`] override applies, so AA cmds get the
/// hardware-blend fast path).
///
/// The check is O(n²) but cmd counts are small (a typical clock frame
/// is 50–100 cmds). Only `FillRect` with `alpha == 255` qualifies as an
/// occluder — geometric primitives (OBB, disc, arc, line) don't fully
/// cover their AABBs and are never occluders even at full alpha.
///
/// Two-pass dispatch:
///
/// 1. **Occlusion pre-pass.** For each cmd, scan forward looking for a
///    later opaque [`Cmd::FillRect`] whose AABB fully covers it; if
///    found, drop the cmd. Markers (`Barrier`, `SetClip`) terminate
///    the search range — a `Barrier` declares ordering that must be
///    preserved (compositor sync, swap hold), and a `SetClip` change
///    means later cmds render under a different clip and can't be
///    trusted as occluders of earlier pixels.
///
/// 2. **Adjacent-FillRect coalescing.** Survivor cmds are dispatched
///    one at a time, but consecutive [`Cmd::FillRect`]s with identical
///    `color` + `blend` and edge-adjacent rectangles are merged into a
///    single wider FillRect. Cuts down on the number of DMA2D fill
///    ops and reduces dirty-rect tracking pressure when widgets
///    happen to emit sequential strips of the same fill (toolbars,
///    background bands, table cells).
///
/// The coalescing is conservative — only horizontal or vertical edge
/// adjacency is detected (no L-shapes, no overlap-into-union). For
/// the common case of a row of cells fixed-width and same-color, this
/// reduces N rects to 1.
fn submit_with_occlusion<B: Blitter, const N: usize>(
    r: &mut BlitterRenderer<'_, B, N>,
    list: &CommandList,
) {
    let mut pending: Option<(WidgetRect, Color, BlendMode)> = None;

    for (i, cmd) in list.iter().enumerate() {
        // Occlusion pre-pass.
        let occluded = match cmd.aabb() {
            None => false,
            Some(bb_i) => {
                let mut hit = false;
                for later in list.iter().skip(i + 1) {
                    if matches!(later, Cmd::Barrier | Cmd::SetClip { .. }) {
                        break;
                    }
                    if !is_opaque_filler(later) {
                        continue;
                    }
                    let Some(bb_j) = later.aabb() else { continue };
                    if rect_contains(bb_j, bb_i) {
                        hit = true;
                        break;
                    }
                }
                hit
            }
        };
        if occluded {
            // Skip the cmd entirely. Don't flush pending — the next cmd
            // may still coalesce with what's pending across this gap.
            continue;
        }

        // Coalescing dispatch.
        if let Cmd::FillRect { rect, color, blend } = cmd {
            if let Some((prect, pcolor, pblend)) = pending.as_ref() {
                if pcolor == color
                    && pblend == blend
                    && let Some(merged) = merge_rects_axis_aligned(*prect, *rect)
                {
                    pending = Some((merged, *color, *blend));
                    continue;
                }
                // Can't merge — flush pending and start a new pending.
                flush_pending(r, pending.take());
            }
            pending = Some((*rect, *color, *blend));
        } else {
            // Markers and non-FillRect cmds break the coalescing run.
            flush_pending(r, pending.take());
            cmd.dispatch_to(r);
        }
    }
    flush_pending(r, pending.take());
}

#[inline]
fn flush_pending<B: Blitter, const N: usize>(
    r: &mut BlitterRenderer<'_, B, N>,
    pending: Option<(WidgetRect, Color, BlendMode)>,
) {
    if let Some((rect, color, blend)) = pending {
        Cmd::FillRect { rect, color, blend }.dispatch_to(r);
    }
}

/// Try to merge two rectangles into a single rectangle representing
/// their union. Returns `Some(union)` only when one of the four
/// edge-adjacent cases applies; returns `None` for overlap, gap, or
/// L-shapes (which would require a multi-rect representation).
#[inline]
fn merge_rects_axis_aligned(a: WidgetRect, b: WidgetRect) -> Option<WidgetRect> {
    if a.y == b.y && a.height == b.height {
        if a.x + a.width == b.x {
            return Some(WidgetRect {
                x: a.x,
                y: a.y,
                width: a.width + b.width,
                height: a.height,
            });
        }
        if b.x + b.width == a.x {
            return Some(WidgetRect {
                x: b.x,
                y: b.y,
                width: a.width + b.width,
                height: a.height,
            });
        }
    }
    if a.x == b.x && a.width == b.width {
        if a.y + a.height == b.y {
            return Some(WidgetRect {
                x: a.x,
                y: a.y,
                width: a.width,
                height: a.height + b.height,
            });
        }
        if b.y + b.height == a.y {
            return Some(WidgetRect {
                x: b.x,
                y: b.y,
                width: a.width,
                height: a.height + b.height,
            });
        }
    }
    None
}

#[inline]
fn is_opaque_filler(cmd: &Cmd) -> bool {
    if let Cmd::FillRect { color, .. } = cmd {
        color.3 == 255
    } else {
        false
    }
}

#[inline]
fn rect_contains(outer: WidgetRect, inner: WidgetRect) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.width >= inner.x + inner.width
        && outer.y + outer.height >= inner.y + inner.height
}

/// Renderer wrapper that applies 90° CCW rotation for platforms where the
/// physical display is landscape but the framebuffer is portrait.
///
/// Maps logical coordinates (800×480 landscape) to framebuffer coordinates
/// (480×800 portrait):
///   fb_x = fb_width - logical_y - logical_height
///   fb_y = logical_x
///   fb_w = logical_height
///   fb_h = logical_width
pub struct RotatedRenderer<'a> {
    inner: &'a mut dyn Renderer,
    /// Portrait framebuffer width (the short dimension, e.g. 480).
    fb_width: i32,
}

impl<'a> RotatedRenderer<'a> {
    /// Create a rotated renderer wrapping `inner`.
    ///
    /// `fb_width` is the portrait framebuffer width (480 on STM32H747I-DISCO).
    pub fn new(inner: &'a mut dyn Renderer, fb_width: u32) -> Self {
        Self {
            inner,
            fb_width: fb_width as i32,
        }
    }
}

impl Renderer for RotatedRenderer<'_> {
    fn fill_rect(&mut self, rect: WidgetRect, color: Color) {
        let mut fb_x = self.fb_width - rect.y - rect.height;
        let fb_y = rect.x;
        let mut fb_w = rect.height;
        let fb_h = rect.width;

        if fb_w <= 0 || fb_h <= 0 {
            return;
        }

        // Clamp left edge: rect extends off-screen left
        if fb_x < 0 {
            fb_w += fb_x; // shrink width by the overshoot
            fb_x = 0;
        }
        // Clamp right edge
        if fb_x + fb_w > self.fb_width {
            fb_w = self.fb_width - fb_x;
        }
        if fb_w <= 0 {
            return;
        }

        self.inner.fill_rect(
            WidgetRect {
                x: fb_x,
                y: fb_y,
                width: fb_w,
                height: fb_h,
            },
            color,
        );
    }

    fn blend_rect(&mut self, rect: WidgetRect, color: Color) {
        let mut fb_x = self.fb_width - rect.y - rect.height;
        let fb_y = rect.x;
        let mut fb_w = rect.height;
        let fb_h = rect.width;

        if fb_w <= 0 || fb_h <= 0 {
            return;
        }

        if fb_x < 0 {
            fb_w += fb_x;
            fb_x = 0;
        }
        if fb_x + fb_w > self.fb_width {
            fb_w = self.fb_width - fb_x;
        }
        if fb_w <= 0 {
            return;
        }

        self.inner.blend_rect(
            WidgetRect {
                x: fb_x,
                y: fb_y,
                width: fb_w,
                height: fb_h,
            },
            color,
        );
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        let fx = self.fb_width - 1 - position.1;
        if fx >= 0 {
            self.inner.draw_text((fx, position.0), text, color);
        }
    }

    fn draw_pixels(&mut self, position: (i32, i32), pixels: &[Color], width: u32, height: u32) {
        // 2026-05-17: replace the per-pixel `fill_rect` slow path with a
        // rotate-then-delegate pattern. The previous implementation made
        // W*H calls to self.fill_rect (each a 1×1 logical rect) through
        // the full rotation + clip + blitter pipeline. For a 60×60 icon
        // that's 3600 single-pixel fill_rects per draw, and IconStrip
        // draws 3 icons per frame — pinning CM7 inside the blitter for
        // seconds at a time and making poll_joystick effectively
        // unreachable (probe-rs halt evidence: 3 sequential halts ~1s
        // apart all landed at byte-identical PC/LR/SP inside
        // CpuBlitter::fill's row loop, called from this site).
        //
        // The inner `BlitterRenderer::draw_pixels` (line 617) already has
        // an Argb8888 fast path that writes pixels directly into the
        // surface buffer + adds one dirty rect — H+W work per call, no
        // per-pixel fill_rect dispatch. Rotating the source buffer once
        // (W*H copies) and calling inner.draw_pixels once collapses
        // 3 × 60 × 60 = 10,800 fill_rect dispatches down to 3 fast blits.
        if width == 0 || height == 0 {
            return;
        }

        // 90° rotation mapping (mirrors `fill_rect` above):
        // physical (fb_x, fb_y) = (fb_width - rect.y - rect.height, rect.x)
        // physical (width, height) = (rect.height, rect.width)
        let fb_x = self.fb_width - position.1 - height as i32;
        let fb_y = position.0;
        let phys_w = height; // physical width = logical height
        let phys_h = width; // physical height = logical width

        // Re-lay-out the pixel buffer in physical orientation.
        // For each logical (lx, ly), the rotated buffer index is
        // (lx * phys_w) + (height - 1 - ly).
        //
        // 2026-05-17 (second pass): use a static scratch buffer instead
        // of allocating a Vec per call. The bench-9-snapshot setup
        // draws ~10-14 icons per render through this path
        // (IconStrip + Wing + spectrum overlay); at 9 Hz that was
        // ~200 KiB/sec of alloc/free through the 64 KiB linked-list
        // heap. After a few seconds of operation the heap fragments
        // and a subsequent alloc panics — observed as a "lock up after
        // navigating into the wing menu, recover via reset button"
        // pattern.
        //
        // The scratch buffer is sized for the worst case we draw in
        // practice (icons up to 64×64). If a caller requests a larger
        // pixels rect, we fall back to allocating one Vec for that
        // specific call — large icons are rare and the alloc still
        // makes progress.
        const SCRATCH_PIXELS: usize = 64 * 64;
        static mut SCRATCH: [Color; SCRATCH_PIXELS] = [Color(0, 0, 0, 0); SCRATCH_PIXELS];

        let len = (width * height) as usize;
        if len <= SCRATCH_PIXELS {
            // SAFETY: single-core, single-threaded — `draw_pixels` is
            // called only from the main render loop and not from any
            // ISR. The scratch buffer is unique to this call site and
            // not borrowed across calls.
            let scratch = unsafe { &mut SCRATCH[..len] };
            // Clear only the prefix we'll actually fill; pixels that
            // aren't written (the `pixels.get(src_idx)` None branch)
            // retain their prior value, but we touch every index in
            // the inner loop so that's safe.
            for ly in 0..height as i32 {
                for lx in 0..width as i32 {
                    let src_idx = (ly as u32 * width + lx as u32) as usize;
                    let dst_idx =
                        (lx as u32 * phys_w + (height as i32 - 1 - ly) as u32) as usize;
                    scratch[dst_idx] = pixels.get(src_idx).copied().unwrap_or(Color(0, 0, 0, 0));
                }
            }
            self.inner.draw_pixels((fb_x, fb_y), scratch, phys_w, phys_h);
        } else {
            // Oversize fallback: allocate once for this call. Rare.
            let mut rotated: alloc::vec::Vec<Color> = alloc::vec::Vec::with_capacity(len);
            rotated.resize(len, Color(0, 0, 0, 0));
            for ly in 0..height as i32 {
                for lx in 0..width as i32 {
                    let src_idx = (ly as u32 * width + lx as u32) as usize;
                    let dst_idx =
                        (lx as u32 * phys_w + (height as i32 - 1 - ly) as u32) as usize;
                    if let Some(&c) = pixels.get(src_idx) {
                        rotated[dst_idx] = c;
                    }
                }
            }
            self.inner.draw_pixels((fb_x, fb_y), &rotated, phys_w, phys_h);
        }
    }

    fn submit(&mut self, list: &CommandList) {
        // Delegate to inner — preserves any backend-specific
        // optimizations (e.g. BlitterRenderer's occlusion pre-pass)
        // through the rotation wrapper.
        self.inner.submit(list);
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

#[cfg(all(test, feature = "png", not(target_os = "none")))]
mod png_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;
    use rlvgl_core::png::DecodingError;

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

    #[test]
    fn blitter_rejects_invalid_png() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let err = renderer.draw_png((0, 0), b"not a png").unwrap_err();
        assert!(matches!(err, DecodingError::Format(_)));
    }
}

#[cfg(all(test, feature = "jpeg", not(target_os = "none")))]
mod jpeg_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;
    use rlvgl_core::jpeg::Error as JpegError;

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

    #[test]
    fn blitter_rejects_invalid_jpeg() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let err = renderer.draw_jpeg((0, 0), b"not a jpeg").unwrap_err();
        assert!(matches!(err, JpegError::Format(_)));
    }
}

#[cfg(all(test, feature = "gif"))]
mod gif_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;
    use rlvgl_core::gif::DecodingError as GifDecodingError;

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

    #[test]
    fn blitter_rejects_invalid_gif() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let err = renderer
            .draw_gif_frame((0, 0), b"not a gif", 0)
            .unwrap_err();
        assert!(matches!(err, GifDecodingError::Format(_)));
    }
}

#[cfg(all(test, feature = "apng"))]
mod apng_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use base64::Engine;

    const RED_DOT_APNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACGFjVEwAAAABAAAAALQt6aAAAAAaZmNUTAAAAAAAAAABAAAAAQAAAAAAAAAAAGQD6AEAqmVSjAAAAA1JREFUeJxj+M/A8B8ABQAB/4mZPR0AAAAASUVORK5CYII=";

    #[test]
    fn blitter_draws_apng() {
        let data = base64::engine::general_purpose::STANDARD
            .decode(RED_DOT_APNG)
            .unwrap();
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer.draw_apng_frame((0, 0), &data, 0).unwrap();
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

#[cfg(all(test, feature = "qrcode", not(target_os = "none")))]
mod qrcode_tests {
    use super::*;
    use crate::cpu_blitter::CpuBlitter;
    use rlvgl_core::qrcode::QrError;

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

    #[test]
    fn blitter_rejects_invalid_qr_data() {
        let mut buf = [0u8; 64 * 64 * 4];
        let surface = Surface::new(&mut buf, 64 * 4, PixelFmt::Argb8888, 64, 64);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let data = vec![0u8; 3000];
        let err = renderer.draw_qr((0, 0), &data).unwrap_err();
        assert!(matches!(err, QrError::DataTooLong));
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
        renderer
            .draw_lottie_frame((0, 0), SIMPLE_JSON, 0, 1, 1)
            .unwrap();
        assert!(buf.iter().any(|&p| p != 0));
    }

    #[test]
    fn blitter_rejects_invalid_lottie() {
        let mut buf = [0u8; 4 * 4 * 4];
        let surface = Surface::new(&mut buf, 4 * 4, PixelFmt::Argb8888, 4, 4);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        assert!(
            renderer
                .draw_lottie_frame((0, 0), "not json", 0, 1, 1)
                .is_err()
        );
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

    #[test]
    fn pinyin_candidates_clipped_to_surface() {
        let mut buf = [0u8; 32 * 16 * 4];
        let surface = Surface::new(&mut buf, 32 * 4, PixelFmt::Argb8888, 32, 16);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        let ime = PinyinInputMethod;
        assert!(renderer.draw_pinyin_candidates((0, 0), &ime, "zhong", Color(255, 255, 255, 255)));

        let mut expected = [0u8; 32 * 16 * 4];
        let surface_e = Surface::new(&mut expected, 32 * 4, PixelFmt::Argb8888, 32, 16);
        let mut blit_e = CpuBlitter;
        let mut renderer_e: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit_e, surface_e);
        let chars = ime.candidates("zhong").unwrap();
        let text: alloc::string::String = chars.into_iter().collect();
        let clipped: alloc::string::String = text.chars().take(2).collect();
        Renderer::draw_text(&mut renderer_e, (0, 0), &clipped, Color(255, 255, 255, 255));
        assert_eq!(buf[..], expected[..]);
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

    #[test]
    fn fatfs_listing_clipped_to_surface() {
        let mut img = Cursor::new(vec![0u8; 1024 * 512]);
        fatfs::format_volume(&mut img, FormatVolumeOptions::new()).unwrap();
        img.seek(SeekFrom::Start(0)).unwrap();
        {
            let buf_stream = BufStream::new(&mut img);
            let fs = FileSystem::new(buf_stream, FsOptions::new()).unwrap();
            fs.root_dir().create_file("first_long_name.txt").unwrap();
            fs.root_dir().create_file("second_long_name.txt").unwrap();
            fs.root_dir().create_file("third_long_name.txt").unwrap();
        }
        img.seek(SeekFrom::Start(0)).unwrap();
        let image_vec = img.get_ref().clone();
        let mut img_expected = Cursor::new(image_vec.clone());
        let mut img_actual = Cursor::new(image_vec);

        let mut buf = [0u8; 32 * 32 * 4];
        let surface = Surface::new(&mut buf, 32 * 4, PixelFmt::Argb8888, 32, 32);
        let mut blit = CpuBlitter;
        let mut renderer: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit, surface);
        renderer
            .draw_fatfs_dir((0, 0), &mut img_actual, "/", Color(255, 255, 255, 255))
            .unwrap();

        let names = rlvgl_core::fatfs::list_dir(&mut img_expected, "/").unwrap();
        let mut expected = [0u8; 32 * 32 * 4];
        let surface_e = Surface::new(&mut expected, 32 * 4, PixelFmt::Argb8888, 32, 32);
        let mut blit_e = CpuBlitter;
        let mut renderer_e: BlitterRenderer<'_, CpuBlitter, 4> =
            BlitterRenderer::new(&mut blit_e, surface_e);
        for (i, name) in names.iter().take(2).enumerate() {
            let clipped: alloc::string::String = name.chars().take(2).collect();
            Renderer::draw_text(
                &mut renderer_e,
                (0, (i as i32) * 16),
                &clipped,
                Color(255, 255, 255, 255),
            );
        }
        assert_eq!(buf[..], expected[..]);
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
