//! Star Wars opening crawl effect with DMA2D-accelerated rendering.
//!
//! Yellow bold text scrolls bottom-to-top with perspective foreshortening.
//! A procedural starfield provides the background.
//!
//! ## Scroll buffer architecture
//!
//! A double-height ARGB8888 buffer in SDRAM holds pre-composited scanlines
//! (starfield + blended text). Each frame, only the NEW scanlines entering
//! the visible window are composited — typically 1-2 lines per frame.
//! The visible CRAWL_H-row window is rotated to the portrait FB via CPU copy.
//!
//! The buffer is 2×CRAWL_H rows so a contiguous CRAWL_H window can always
//! be read without wrap. When the write pointer passes CRAWL_H, rows
//! 0..CRAWL_H-1 are a copy of rows CRAWL_H..2*CRAWL_H-1 (kept in sync).

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

// ── Debug serial output ────────────────────────────────────────────────

fn dbg(s: &str) {
    const ISR: *const u32 = 0x4001_101C as *const u32;
    const TDR: *mut u32 = 0x4001_1028 as *mut u32;
    for b in s.bytes() {
        unsafe {
            while ISR.read_volatile() & (1 << 7) == 0 {}
            TDR.write_volatile(b as u32);
        }
    }
}

fn dbg_dec(mut v: u32) {
    if v == 0 { dbg("0"); return; }
    let mut buf = [0u8; 10];
    let mut i = 0usize;
    while v > 0 { buf[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    while i > 0 { i -= 1; dbg(unsafe { core::str::from_utf8_unchecked(core::slice::from_ref(&buf[i])) }); }
}

#[allow(dead_code)]
fn dbg_hex(val: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    const ISR: *const u32 = 0x4001_101C as *const u32;
    const TDR: *mut u32 = 0x4001_1028 as *mut u32;
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        unsafe {
            while ISR.read_volatile() & (1 << 7) == 0 {}
            TDR.write_volatile(HEX[nibble] as u32);
        }
    }
}

// ── Constants ───────────────────────────────────────────────────────────────

/// Portrait FB width (physical display columns).
const FB_W: u32 = 480;
/// Portrait rows used by the crawl (800 - 80 icon bar).
const FB_H: u32 = 720;
/// Landscape crawl width (maps to portrait Y = FB_H).
const CRAWL_W: u32 = FB_H;
/// Landscape crawl height (maps to portrait X = FB_W, the scroll axis).
const CRAWL_H: u32 = FB_W;
/// Pre-rendered text line width in pixels.
const TEXT_W: u32 = 600;
/// Perspective width at landscape top (vanishing point).
const TOP_W: u32 = 480;
/// Perspective width at landscape bottom (text entrance).
const BOT_W: u32 = 600;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Starfield in portrait orientation: FB_W wide × double-height mirrored.
/// Top half = random stars, bottom half = mirror of top (row-reversed).
const STAR_ROWS: u32 = FB_H * 2; // 1440 portrait rows
const STAR_STRIDE: u32 = FB_W * BPP; // 1920 bytes per portrait row
const STAR_SIZE: usize = (STAR_ROWS * STAR_STRIDE) as usize;
/// Line spacing multiplier for text layout (1.5x font height).
const LINE_SPACING_NUM: u32 = 3;
const LINE_SPACING_DEN: u32 = 2;
/// Target scroll speed in pixels per second.
const SCROLL_PX_PER_SEC: u32 = 40;
/// Yellow foreground color for DMA2D A8 blend (0x00RRGGBB).
const YELLOW: u32 = 0x00FF_D700;
/// Starfield background color (dark blue-black).
const BG_COLOR: u32 = 0xFF0A_0A20;
/// Number of stars to scatter.
const STAR_COUNT: usize = 200;

/// SDRAM base address for crawl buffers (Bank 2).
const CRAWL_BASE: usize = 0xD100_0000;

/// D2 SRAM base for FIR scanline (DMA2D-accessible).
const D2_SCANLINE: usize = 0x3000_0000;

// ── Public API ──────────────────────────────────────────────────────────────

pub struct StarCrawl {
    active: bool,
    /// Text scroll position (Q8). Negative = text off-screen below.
    scroll_q8: i32,
    scroll_speed_q8: i32,
    /// Starfield scroll position (Q8). Advances downward (opposite to text).
    star_scroll_q8: i32,

    // SDRAM pointers.
    starfield: *mut u8,   // CRAWL_W × STAR_ROWS × 4 (double-height mirrored)
    text_src: *mut u8,    // TEXT_W × text_h (A8 pre-rendered glyphs + logos)

    text_h: u32,

    // CPU-side FIR work buffer (DTCM, fast).
    scanline_buf: [u8; CRAWL_W as usize],

    lines: &'static [&'static str],
    font: &'static PackedFont,
}

impl StarCrawl {
    pub const fn new(font: &'static PackedFont, lines: &'static [&'static str], frame_hz: u32) -> Self {
        Self {
            active: false,
            scroll_q8: 0,
            scroll_speed_q8: ((SCROLL_PX_PER_SEC * 256) / frame_hz) as i32,
            star_scroll_q8: 0,
            starfield: core::ptr::null_mut(),
            text_src: core::ptr::null_mut(),
            text_h: 0,
            scanline_buf: [0u8; CRAWL_W as usize],
            lines,
            font,
        }
    }

    pub fn is_active(&self) -> bool { self.active }

    pub fn deactivate(&mut self) { self.active = false; }

    /// Advance scroll by one tick. Call AFTER present() so both
    /// double-buffer frames render at the same scroll position.
    /// Text scrolls up, starfield scrolls down (parallax).
    pub fn advance_scroll(&mut self) {
        self.scroll_q8 += self.scroll_speed_q8;
        // Starfield moves at 1/3 text speed, downward.
        self.star_scroll_q8 += self.scroll_speed_q8 / 3;
    }

    pub fn touch_deactivate(&mut self, _px: i32, _py: i32) -> bool {
        if self.active { self.active = false; true } else { false }
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn activate(&mut self, dma2d: &mut Dma2dBlitter) {
        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        let text_lines = self.lines.len() as u32;
        // Lead-in (120 rows starfield) + text + lead-out (full screen).
        self.text_h = 120 + text_lines * line_h + CRAWL_H;

        // Assign SDRAM pointers.
        self.starfield = CRAWL_BASE as *mut u8;
        self.text_src = (CRAWL_BASE + STAR_SIZE) as *mut u8;

        let text_end = self.text_src as u32 + TEXT_W * self.text_h;
        dbg("SC:sf="); dbg_hex(self.starfield as u32);
        dbg(" ts="); dbg_hex(self.text_src as u32);
        dbg(" te="); dbg_hex(text_end);
        dbg(" th="); dbg_dec(self.text_h);
        dbg("\r\n");

        // Render double-height mirrored starfield.
        self.render_starfield(dma2d);

        // Pre-render all text into A8 buffer.
        dbg("SC:text...\r\n");
        self.pre_render_text();

        // Text starts off-screen below. Starfield starts at 0.
        self.scroll_q8 = -((CRAWL_H as i32) << 8);
        self.star_scroll_q8 = 0;
        self.active = true;
        dbg("SC:ready\r\n");
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn tick(
        &mut self,
        _dma2d: &mut Dma2dBlitter,
        back_buf: *mut u8,
        fb_w: u32,
        _fb_h: u32,
    ) -> bool {
        if !self.active { return false; }

        let scroll_px = self.scroll_q8 >> 8;
        // Deactivate after last text scrolls fully off the top of screen.
        if scroll_px >= (self.text_h + CRAWL_H) as i32 {
            self.active = false;
            return false;
        }

        let t0 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // ── Step 1: DMA2D blit starfield → portrait back buffer ──────────
        // The starfield is portrait-oriented (FB_W × STAR_ROWS).
        // Blit FB_H rows starting at star_scroll offset. DMA2D handles
        // the row-major copy — zero CPU per-pixel work.
        let star_row = ((self.star_scroll_q8 >> 8) as u32) % STAR_ROWS;
        let rows_before_wrap = STAR_ROWS - star_row;
        if rows_before_wrap >= FB_H {
            // No wrap: one blit.
            let star_src = unsafe {
                self.starfield.add((star_row * STAR_STRIDE) as usize)
            };
            _dma2d.blit_raw(
                star_src as *const u8, STAR_STRIDE,
                back_buf, fb_w * BPP,
                FB_W, FB_H,
                PixelFmt::Argb8888,
            );
        } else {
            // Wraps: two blits.
            let src1 = unsafe {
                self.starfield.add((star_row * STAR_STRIDE) as usize)
            };
            _dma2d.blit_raw(
                src1 as *const u8, STAR_STRIDE,
                back_buf, fb_w * BPP,
                FB_W, rows_before_wrap,
                PixelFmt::Argb8888,
            );
            let dst2 = unsafe {
                back_buf.add((rows_before_wrap * fb_w * BPP) as usize)
            };
            _dma2d.blit_raw(
                self.starfield as *const u8, STAR_STRIDE,
                dst2, fb_w * BPP,
                FB_W, FB_H - rows_before_wrap,
                PixelFmt::Argb8888,
            );
        }

        let t1 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // ── Step 2: FIR text overlay + alpha blend onto portrait FB ──────
        // For each landscape row (vis_y), FIR the A8 text, then write
        // blended pixels into the portrait FB column. Only rows with
        // text data are processed; the starfield is already in the FB.
        let dst_stride = fb_w * BPP;

        for vis_y in 0..CRAWL_H {
            let text_row_i = scroll_px + vis_y as i32;
            if text_row_i < 0 || text_row_i as u32 >= self.text_h {
                continue; // starfield already in FB, nothing to overlay
            }

            // Perspective width.
            let target_w = TOP_W + (BOT_W - TOP_W) * vis_y / (CRAWL_H - 1);
            let dst_x_off = (CRAWL_W - target_w) / 2;

            // FIR resample (skips all-zero rows internally).
            self.fir_resample_text_row(text_row_i as u32, target_w);

            // Alpha-blend FIR'd text onto the starfield already in the FB.
            // Write to portrait column (fb_w - 1 - vis_y).
            let dst_col = fb_w - 1 - vis_y;
            for lx in dst_x_off..dst_x_off + target_w {
                let trap_x = (lx - dst_x_off) as usize;
                let alpha = self.scanline_buf[trap_x] as u32;
                if alpha == 0 { continue; } // skip transparent — starfield intact

                // Read existing starfield pixel from portrait FB.
                let dst_off = (lx * dst_stride + dst_col * BPP) as usize;
                let star = unsafe { (back_buf.add(dst_off) as *const u32).read() };

                let inv = 255 - alpha;
                let r = (((YELLOW >> 16) & 0xFF) * alpha + ((star >> 16) & 0xFF) * inv) / 255;
                let g = (((YELLOW >> 8) & 0xFF) * alpha + ((star >> 8) & 0xFF) * inv) / 255;
                let b = ((YELLOW & 0xFF) * alpha + (star & 0xFF) * inv) / 255;
                let blended = 0xFF00_0000 | (r << 16) | (g << 8) | b;

                unsafe { (back_buf.add(dst_off) as *mut u32).write(blended); }
            }
        }

        let t2 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // Timing report.
        static mut FRAME_CT: u32 = 0;
        let fc = unsafe { FRAME_CT };
        if fc < 5 || fc % 30 == 0 {
            dbg("SC:star="); dbg_dec(t1.wrapping_sub(t0) / 400);
            dbg(" txt="); dbg_dec(t2.wrapping_sub(t1) / 400);
            dbg(" tot="); dbg_dec(t2.wrapping_sub(t0) / 400);
            dbg("us s="); dbg_dec(scroll_px as u32);
            dbg("\r\n");
        }
        unsafe { FRAME_CT = fc + 1; }

        true
    }

    // ── Incremental composition ─────────────────────────────────────────

    /// FIR-resample one text source row from TEXT_W → target_w.
    /// Uses 7-tap raised-cosine kernel for anti-aliased downsampling.
    /// Skips runs of zero source pixels (transparent spans) for speed.
    /// Output: A8 alpha values in self.scanline_buf[0..target_w].
    #[inline(always)]
    fn fir_resample_text_row(&mut self, text_row: u32, target_w: u32) {
        if text_row >= self.text_h { return; }
        let src = unsafe { self.text_src.add((text_row * TEXT_W) as usize) };
        let step_q16 = (TEXT_W << 16) / target_w;

        // Check if entire row is zero (common for line spacing gaps).
        // Scan in u32 chunks (4 A8 bytes at a time) for speed.
        let mut all_zero = true;
        let words = TEXT_W as usize / 4;
        let src32 = src as *const u32;
        for i in 0..words {
            if unsafe { src32.add(i).read() } != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            for ox in 0..target_w as usize {
                self.scanline_buf[ox] = 0;
            }
            return;
        }

        let mut ox = 0usize;
        while ox < target_w as usize {
            let cx_q16 = ox as u32 * step_q16 + (step_q16 >> 1);
            let cx = (cx_q16 >> 16) as i32;

            // Check if center and neighbors are all zero → skip this span.
            if cx >= 3 && cx + 3 < TEXT_W as i32 {
                let center = unsafe { *src.add(cx as usize) };
                if center == 0 {
                    // Check tap range: if all 7 taps are zero, output is zero.
                    let any_nonzero = unsafe {
                        *src.add((cx - 3) as usize) != 0
                        || *src.add((cx - 2) as usize) != 0
                        || *src.add((cx - 1) as usize) != 0
                        || *src.add((cx + 1) as usize) != 0
                        || *src.add((cx + 2) as usize) != 0
                        || *src.add((cx + 3) as usize) != 0
                    };
                    if !any_nonzero {
                        self.scanline_buf[ox] = 0;
                        ox += 1;
                        continue;
                    }
                }
            }

            // Full 7-tap FIR.
            let mut acc: u32 = 0;
            let mut wsum: u32 = 0;
            const TAPS: [(i32, u32); 7] = [(-3,8),(-2,24),(-1,48),(0,64),(1,48),(2,24),(3,8)];
            for &(tap, w) in &TAPS {
                let sx = cx + tap;
                if sx >= 0 && sx < TEXT_W as i32 {
                    let v = unsafe { *src.add(sx as usize) } as u32;
                    acc += v * w;
                    wsum += w;
                }
            }
            self.scanline_buf[ox] = if wsum > 0 { (acc / wsum).min(255) as u8 } else { 0 };
            ox += 1;
        }
    }

    // ── Starfield ───────────────────────────────────────────────────────

    /// Render portrait-oriented double-height mirrored starfield.
    /// FB_W wide × STAR_ROWS tall. Top half = random stars,
    /// bottom half = mirror of top. DMA2D can blit directly to portrait FB.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn render_starfield(&mut self, dma2d: &mut Dma2dBlitter) {
        // Fill full buffer with BG_COLOR.
        dma2d.fill_raw(
            self.starfield, STAR_STRIDE, FB_W, STAR_ROWS,
            BG_COLOR, PixelFmt::Argb8888,
        );

        // Scatter stars in the top half (portrait rows 0..FB_H-1).
        let mut rng = 0xDEAD_BEEFu32;
        let pixel_count = FB_W * FB_H;
        for _ in 0..STAR_COUNT {
            rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
            let idx = rng % pixel_count;
            let brightness = 128 + (rng >> 24) as u8 / 2;
            let color = 0xFF00_0000
                | ((brightness as u32) << 16)
                | ((brightness as u32) << 8)
                | (brightness as u32);
            unsafe {
                (self.starfield.add((idx * BPP) as usize) as *mut u32).write_volatile(color);
            }
            rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
        }

        // Mirror: copy top half to bottom half in reverse row order.
        let row_bytes = STAR_STRIDE as usize;
        for i in 0..FB_H {
            let src = unsafe { self.starfield.add((i * STAR_STRIDE) as usize) };
            let dst_row = STAR_ROWS - 1 - i;
            let dst = unsafe { self.starfield.add((dst_row * STAR_STRIDE) as usize) };
            unsafe { core::ptr::copy_nonoverlapping(src, dst, row_bytes); }
        }
    }

    // ── Text pre-render ─────────────────────────────────────────────────

    fn pre_render_text(&mut self) {
        let total_bytes = (TEXT_W * self.text_h) as usize;
        unsafe { core::ptr::write_bytes(self.text_src, 0, total_bytes); }

        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        let mut cur_y = 120u32; // lead-in

        for &line in self.lines {
            let text_w = self.font.measure(line);
            let x_off = ((TEXT_W as i32 - text_w) / 2).max(0);
            let mut cx = x_off;
            for ch in line.chars() {
                if let Some(glyph) = self.font.glyph(ch) {
                    let gw = glyph.width as usize;
                    let gh = glyph.height as usize;
                    let data_off = glyph.offset as usize;
                    let gy = cur_y as i32 + self.font.ascent as i32 - glyph.ymin as i32 - gh as i32;
                    for row in 0..gh {
                        for col in 0..gw {
                            let src_idx = data_off + row * gw + col;
                            if let Some(&alpha) = self.font.data.get(src_idx) {
                                if alpha > 0 {
                                    let dx = cx + col as i32;
                                    let dy = gy + row as i32;
                                    if dx >= 0 && (dx as u32) < TEXT_W
                                        && dy >= 0 && (dy as u32) < self.text_h
                                    {
                                        unsafe {
                                            self.text_src.add((dy as u32 * TEXT_W + dx as u32) as usize)
                                                .write_volatile(alpha);
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
}

// Non-DMA2D stubs for compilation on non-ARM targets.
#[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
impl StarCrawl {
    pub fn activate(&mut self) {}
}
