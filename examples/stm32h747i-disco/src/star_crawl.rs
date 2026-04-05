//! Star Wars opening crawl effect with DMA2D-accelerated rendering.
//!
//! Yellow bold text scrolls bottom-to-top with perspective foreshortening
//! (680px wide at the bottom, 400px centered at the top). A procedural
//! starfield provides the background. The active drawing area is 720x480
//! (80px right reserved for icon bar).
//!
//! ## SDRAM buffer layout (starting at [`CRAWL_BASE`])
//!
//! | Buffer       | Size          | Format   | Purpose                        |
//! |-------------|---------------|----------|--------------------------------|
//! | Starfield   | 720×480×4     | ARGB8888 | Rendered once at activation    |
//! | Crawl frame | 720×480×4     | ARGB8888 | Per-frame working buffer       |
//! | Text source | 680×H×1 byte  | A8       | Pre-rendered crawl text        |

#![allow(dead_code)]

use rlvgl::core::packed_font::PackedFont;

#[cfg(all(
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use rlvgl::platform::dma2d::Dma2dBlitter;
#[cfg(all(
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
))]
use rlvgl::platform::blit::PixelFmt;

// ── Constants ───────────────────────────────────────────────────────────────

/// Landscape width of the crawl drawing area (excluding icon bar).
const CRAWL_W: u32 = 720;
/// Landscape height of the crawl drawing area.
const CRAWL_H: u32 = 480;
/// Width of the text source buffer (full-width before perspective).
const TEXT_W: u32 = 680;
/// Width at the top of the perspective trapezoid.
const TOP_W: u32 = 400;
/// Width at the bottom of the perspective trapezoid.
const BOT_W: u32 = 680;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Starfield buffer size.
const STARFIELD_SIZE: usize = (CRAWL_W * CRAWL_H * BPP) as usize;
/// Crawl frame buffer size.
const CRAWL_FRAME_SIZE: usize = STARFIELD_SIZE;
/// Line spacing multiplier for text layout (1.5x font height).
const LINE_SPACING_NUM: u32 = 3;
const LINE_SPACING_DEN: u32 = 2;
/// Target scroll speed in pixels per second.
const SCROLL_PX_PER_SEC: u32 = 9;
/// Yellow foreground color for DMA2D A8 blend (0x00RRGGBB).
const YELLOW: u32 = 0x00FF_D700;
/// Starfield background color (dark blue-black).
const BG_COLOR: u32 = 0xFF0A_0A20;
/// Number of stars to scatter.
const STAR_COUNT: usize = 200;

/// SDRAM base address for crawl buffers. Placed after the two framebuffers
/// (2×1.5 MB) and the pristine desktop copy (1.5 MB) = 0xD048_0000.
const CRAWL_BASE: usize = 0xD048_0000;

// ── Public API ──────────────────────────────────────────────────────────────

/// State machine for the Star Wars opening crawl.
pub struct StarCrawl {
    active: bool,
    /// Scroll position in Q8 fixed-point (sub-pixel precision).
    scroll_q8: i32,
    /// Scroll speed in Q8 fixed-point pixels per tick.
    scroll_speed_q8: i32,

    // SDRAM buffer pointers (set once in activate).
    starfield: *mut u8,
    crawl_frame: *mut u8,
    text_src: *mut u8,

    // Pre-rendered text dimensions.
    text_h: u32,

    // Scanline FIR output buffer (stack-allocated).
    scanline_buf: [u8; TEXT_W as usize],

    // Content.
    lines: &'static [&'static str],
    font: &'static PackedFont,
}

impl StarCrawl {
    /// Create a new crawl state (inactive). Call [`activate`] to start.
    ///
    /// `frame_hz` is the render loop frame rate (e.g. 30). Scroll speed is
    /// derived from [`SCROLL_PX_PER_SEC`] so visual speed is independent of
    /// frame rate.
    pub const fn new(font: &'static PackedFont, lines: &'static [&'static str], frame_hz: u32) -> Self {
        Self {
            active: false,
            scroll_q8: 0,
            scroll_speed_q8: (SCROLL_PX_PER_SEC * 256 / frame_hz) as i32,
            starfield: core::ptr::null_mut(),
            crawl_frame: core::ptr::null_mut(),
            text_src: core::ptr::null_mut(),
            text_h: 0,
            scanline_buf: [0u8; TEXT_W as usize],
            lines,
            font,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the crawl: allocate SDRAM buffers, render starfield and
    /// pre-render all text into the A8 source buffer.
    #[cfg(all(
        feature = "dma2d",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    pub fn activate(&mut self, dma2d: &mut Dma2dBlitter) {
        // Compute text source height: 480 lead-in + text lines + 480 lead-out.
        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        let text_lines = self.lines.len() as u32;
        self.text_h = CRAWL_H + text_lines * line_h + CRAWL_H;

        // Assign SDRAM pointers.
        self.starfield = CRAWL_BASE as *mut u8;
        self.crawl_frame = (CRAWL_BASE + STARFIELD_SIZE) as *mut u8;
        self.text_src = (CRAWL_BASE + STARFIELD_SIZE + CRAWL_FRAME_SIZE) as *mut u8;

        // Render starfield.
        self.render_starfield(dma2d);

        // Pre-render text into A8 buffer.
        self.pre_render_text();

        // Reset scroll.
        self.scroll_q8 = 0;
        self.active = true;
    }

    /// Deactivate the crawl, freeing the rendering pipeline.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Advance one frame. Returns `true` if the back buffer was written
    /// and needs to be presented.
    ///
    /// `back_buf` is the portrait-oriented LTDC back buffer.
    /// `fb_w` / `fb_h` are the portrait framebuffer dimensions (480×800).
    #[cfg(all(
        feature = "dma2d",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    pub fn tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        back_buf: *mut u8,
        fb_w: u32,
        fb_h: u32,
    ) -> bool {
        if !self.active {
            return false;
        }

        let scroll_px = self.scroll_q8 >> 8;

        // Auto-deactivate when scroll has passed all text.
        if scroll_px as u32 >= self.text_h {
            self.active = false;
            return false;
        }

        // Step 1: Blit starfield → crawl frame (DMA2D M2M).
        let stride = CRAWL_W * BPP;
        dma2d.blit_raw(
            self.starfield,
            stride,
            self.crawl_frame,
            stride,
            CRAWL_W,
            CRAWL_H,
            PixelFmt::Argb8888,
        );

        // Steps 2+3: FIR decimate + DMA2D A8 blend, per scanline.
        for y in 0..CRAWL_H {
            let src_y = scroll_px + y as i32;
            if src_y < 0 || src_y as u32 >= self.text_h {
                continue;
            }

            // Perspective target width: TOP_W at y=0, BOT_W at y=479.
            let target_w = TOP_W + (BOT_W - TOP_W) * y / (CRAWL_H - 1);
            let x_off = (CRAWL_W - target_w) / 2;

            // FIR decimate: 680 → target_w.
            self.fir_decimate_line(src_y as u32, target_w);

            // DMA2D blend scanline onto crawl frame.
            let dst = unsafe {
                self.crawl_frame.add((y * stride) as usize + (x_off * BPP) as usize)
            };
            dma2d.blend_a8_color(
                self.scanline_buf.as_ptr(),
                target_w,
                1,
                YELLOW,
                dst,
                stride, // dst_stride (not used for 1 line, but required)
            );
        }

        // Step 4: Rotate landscape crawl frame → portrait back buffer.
        self.rotate_to_portrait(back_buf, fb_w, fb_h);

        // Advance scroll.
        self.scroll_q8 += self.scroll_speed_q8;

        true
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    /// Generate a procedural starfield (dark background + scattered white dots).
    #[cfg(all(
        feature = "dma2d",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn render_starfield(&mut self, dma2d: &mut Dma2dBlitter) {
        let stride = CRAWL_W * BPP;

        // Fill with dark blue-black.
        dma2d.fill_raw(self.starfield, stride, CRAWL_W, CRAWL_H, BG_COLOR, PixelFmt::Argb8888);

        // Scatter stars using xorshift32 PRNG.
        let mut rng = 0xDEAD_BEEFu32;
        let pixel_count = CRAWL_W * CRAWL_H;
        for _ in 0..STAR_COUNT {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let idx = rng % pixel_count;
            let brightness = 128 + (rng >> 24) as u8 / 2; // 128..255
            let color = 0xFF00_0000
                | ((brightness as u32) << 16)
                | ((brightness as u32) << 8)
                | (brightness as u32);
            unsafe {
                let ptr = self.starfield.add((idx * BPP) as usize) as *mut u32;
                ptr.write_volatile(color);
            }
            // Advance PRNG for next star.
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
        }

        // A few brighter 2x2 star clusters.
        for _ in 0..15 {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let x = rng % (CRAWL_W - 2);
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let y = rng % (CRAWL_H - 2);
            let color = 0xFFFF_FFFF; // bright white
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    unsafe {
                        let off = ((y + dy) * CRAWL_W + (x + dx)) * BPP;
                        let ptr = self.starfield.add(off as usize) as *mut u32;
                        ptr.write_volatile(color);
                    }
                }
            }
        }
    }

    /// Pre-render all crawl text into the A8 source buffer.
    fn pre_render_text(&mut self) {
        let total_bytes = (TEXT_W * self.text_h) as usize;

        // Zero the A8 buffer.
        unsafe {
            core::ptr::write_bytes(self.text_src, 0, total_bytes);
        }

        // Start rendering after the 480-line lead-in.
        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        let mut cur_y = CRAWL_H;

        for &line in self.lines {
            let text_w = self.font.measure(line);
            let x_off = ((TEXT_W as i32 - text_w) / 2).max(0);

            // Render each glyph directly into the A8 buffer.
            let mut cx = x_off;
            for ch in line.chars() {
                if let Some(glyph) = self.font.glyph(ch) {
                    let gw = glyph.width as usize;
                    let gh = glyph.height as usize;
                    let data_off = glyph.offset as usize;
                    // Center glyph vertically within line height.
                    let gy = cur_y as i32
                        + (self.font.height as i32 - gh as i32) / 2;

                    for row in 0..gh {
                        for col in 0..gw {
                            let src_idx = data_off + row * gw + col;
                            if let Some(&alpha) = self.font.data.get(src_idx) {
                                if alpha > 0 {
                                    let dx = cx + col as i32;
                                    let dy = gy + row as i32;
                                    if dx >= 0
                                        && (dx as u32) < TEXT_W
                                        && dy >= 0
                                        && (dy as u32) < self.text_h
                                    {
                                        unsafe {
                                            let dst = self
                                                .text_src
                                                .add((dy as u32 * TEXT_W + dx as u32) as usize);
                                            dst.write_volatile(alpha);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    cx += (glyph.advance_fp16 as i32 + 8) >> 4;
                } else {
                    cx += self.font.height as i32 / 2;
                }
            }
            cur_y += line_h;
        }
    }

    /// 7-tap FIR horizontal decimation: resample TEXT_W → target_w pixels.
    ///
    /// Uses Q8 fixed-point weights with a raised-cosine (Hann) window.
    /// Output is written to `self.scanline_buf[0..target_w]`.
    fn fir_decimate_line(&mut self, src_row: u32, target_w: u32) {
        let src_ptr = unsafe { self.text_src.add((src_row * TEXT_W) as usize) };
        let ratio_q16 = ((TEXT_W as u32) << 16) / target_w;

        for ox in 0..target_w {
            // Center position in source (Q16 fixed-point).
            let cx_q16 = ((ox as u32) << 16)
                .wrapping_add(ratio_q16 >> 1)
                .wrapping_mul(TEXT_W)
                / target_w;
            let cx_int = (cx_q16 >> 16) as i32;

            // Sample 7 taps centered at cx_int.
            let mut acc: u32 = 0;
            let mut weight_sum: u32 = 0;

            for tap in -3i32..=3 {
                let sx = cx_int + tap;
                if sx < 0 || sx >= TEXT_W as i32 {
                    continue;
                }
                // Raised-cosine weight: heavier at center, tapering.
                let w: u32 = match tap.unsigned_abs() {
                    0 => 64,
                    1 => 48,
                    2 => 24,
                    3 => 8,
                    _ => 0,
                };
                let val = unsafe { src_ptr.add(sx as usize).read_volatile() } as u32;
                acc += val * w;
                weight_sum += w;
            }

            let result = if weight_sum > 0 { acc / weight_sum } else { 0 };
            self.scanline_buf[ox as usize] = result.min(255) as u8;
        }
    }

    /// Copy landscape crawl frame → portrait back buffer with 90° rotation.
    ///
    /// Landscape (720×480 ARGB8888) → Portrait (480×800 ARGB8888).
    /// Only writes to the left 720 columns (portrait rows 0..719) of the
    /// framebuffer, leaving the icon bar untouched.
    fn rotate_to_portrait(&self, dst: *mut u8, fb_w: u32, _fb_h: u32) {
        let src_stride = CRAWL_W * BPP;
        let dst_stride = fb_w * BPP;

        for ly in 0..CRAWL_H {
            let src_row = unsafe { self.crawl_frame.add((ly * src_stride) as usize) };
            // Landscape y → portrait x. The display is rotated 90° CW:
            // landscape (lx, ly) → portrait (fb_w - 1 - ly, lx)
            let dst_col = fb_w - 1 - ly;

            for lx in 0..CRAWL_W {
                let dst_offset = (lx * dst_stride + dst_col * BPP) as usize;
                unsafe {
                    let pixel = (src_row.add((lx * BPP) as usize) as *const u32).read_volatile();
                    (dst.add(dst_offset) as *mut u32).write_volatile(pixel);
                }
            }
        }
    }
}

// Non-DMA2D stubs for compilation on non-ARM targets.
#[cfg(not(all(
    feature = "dma2d",
    any(target_arch = "arm", target_arch = "aarch64")
)))]
impl StarCrawl {
    pub fn activate(&mut self) {
        // No-op on non-DMA2D targets.
    }
}
