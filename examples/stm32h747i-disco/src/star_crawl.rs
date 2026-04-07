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

/// Crawl frame width = landscape horizontal (portrait Y span).
const CRAWL_W: u32 = 720;
/// Crawl frame height = landscape vertical = scroll axis (portrait X span).
const CRAWL_H: u32 = 480;
/// Pre-rendered text line width in pixels.
const TEXT_W: u32 = 600;
/// Perspective width at landscape top (vanishing point).
const TOP_W: u32 = 360;
/// Perspective width at landscape bottom (text entrance).
const BOT_W: u32 = 600;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Scroll buffer: 2× CRAWL_H rows for contiguous window without wrap.
const SCROLL_BUF_ROWS: u32 = CRAWL_H * 2;
/// Scroll buffer size in bytes.
const SCROLL_BUF_SIZE: usize = (SCROLL_BUF_ROWS * CRAWL_W * BPP) as usize;
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
    scroll_q8: i32,
    scroll_speed_q8: i32,

    // SDRAM pointers.
    starfield: *mut u8,   // CRAWL_W × CRAWL_H × 4 (one screen of stars)
    scroll_buf: *mut u8,  // CRAWL_W × SCROLL_BUF_ROWS × 4 (double-height composited)
    text_src: *mut u8,    // TEXT_W × text_h (A8 pre-rendered glyphs)

    text_h: u32,
    /// How many scroll_buf rows have been composited so far.
    composed_up_to: u32,

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
            starfield: core::ptr::null_mut(),
            scroll_buf: core::ptr::null_mut(),
            text_src: core::ptr::null_mut(),
            text_h: 0,
            composed_up_to: 0,
            scanline_buf: [0u8; CRAWL_W as usize],
            lines,
            font,
        }
    }

    pub fn is_active(&self) -> bool { self.active }

    pub fn deactivate(&mut self) { self.active = false; }

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
        let starfield_size = (CRAWL_W * CRAWL_H * BPP) as usize;
        self.starfield = CRAWL_BASE as *mut u8;
        self.scroll_buf = (CRAWL_BASE + starfield_size) as *mut u8;
        self.text_src = (CRAWL_BASE + starfield_size + SCROLL_BUF_SIZE) as *mut u8;

        let text_end = self.text_src as u32 + TEXT_W * self.text_h;
        dbg("SC:sf="); dbg_hex(self.starfield as u32);
        dbg(" sb="); dbg_hex(self.scroll_buf as u32);
        dbg(" ts="); dbg_hex(self.text_src as u32);
        dbg(" te="); dbg_hex(text_end);
        dbg(" th="); dbg_dec(self.text_h);
        dbg("\r\n");

        // Render starfield (one screen).
        self.render_starfield(dma2d);

        // Pre-render all text into A8 buffer.
        dbg("SC:text...\r\n");
        self.pre_render_text();

        // Pre-compose the first CRAWL_H rows of the scroll buffer
        // (the initial visible window = pure starfield + early text).
        // Zero the scroll buffer so any un-composed rows show as background.
        dbg("SC:clear scroll buf...\r\n");
        dma2d.fill_raw(
            self.scroll_buf,
            CRAWL_W * BPP,
            CRAWL_W,
            SCROLL_BUF_ROWS,
            BG_COLOR,
            PixelFmt::Argb8888,
        );

        dbg("SC:compose initial...\r\n");
        self.composed_up_to = 0;
        self.compose_rows_up_to(dma2d, CRAWL_H);
        cortex_m::asm::dsb();

        self.scroll_q8 = 0;
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
        if scroll_px as u32 >= self.text_h + CRAWL_H {
            self.active = false;
            return false;
        }

        let t0 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // Compose any new rows needed (scroll_px + CRAWL_H is the bottom edge).
        // Compose up to the bottom of the visible window. Don't cap at text_h —
        // rows beyond text_h get pure starfield (the compose function handles this).
        // This prevents stale data from the ring buffer's first pass showing up.
        let need_up_to = scroll_px as u32 + CRAWL_H;
        if need_up_to > self.composed_up_to {
            self.compose_rows_up_to(_dma2d, need_up_to);
        }

        let t1 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // Read visible window from scroll_buf and rotate to portrait FB.
        // Visible rows: scroll_px .. scroll_px + CRAWL_H in the scroll buffer.
        // scroll_buf is double-height so we use (row % CRAWL_H) + page offset.
        let stride = CRAWL_W * BPP;
        let dst_stride = fb_w * BPP;

        for vis_y in 0..CRAWL_H {
            // vis_y=0 → landscape top (vanishing), vis_y=479 → landscape bottom.
            // Newer text enters at bottom. Read reversed to fix glyph orientation.
            let text_row = scroll_px as u32 + vis_y;
            let buf_row = text_row % SCROLL_BUF_ROWS;
            let src_row_ptr = unsafe {
                self.scroll_buf.add((buf_row * stride) as usize)
            };

            // Perspective: narrow at vis_y=0 (top), wide at vis_y=479 (bottom).
            let target_w = TOP_W + (BOT_W - TOP_W) * vis_y / (CRAWL_H - 1);
            let src_x_off = (CRAWL_W - target_w) / 2;

            let dst_col = fb_w - 1 - vis_y;
            for lx in 0..CRAWL_W {
                let src_lx = lx;
                let pixel = if src_lx >= src_x_off && src_lx < src_x_off + target_w {
                    unsafe { (src_row_ptr.add((src_lx * BPP) as usize) as *const u32).read() }
                } else {
                    BG_COLOR
                };
                unsafe {
                    let dst_off = (lx * dst_stride + dst_col * BPP) as usize;
                    (back_buf.add(dst_off) as *mut u32).write(pixel);
                }
            }
        }

        let t2 = unsafe { (0xE000_1004u32 as *const u32).read_volatile() };

        // Advance scroll.
        self.scroll_q8 += self.scroll_speed_q8;

        // Timing report.
        let frame = scroll_px as u32;
        if frame < 5 || frame % 60 == 0 {
            dbg("SC:s="); dbg_dec(frame);
            dbg(" comp="); dbg_dec(t1.wrapping_sub(t0) / 400);
            dbg(" rot="); dbg_dec(t2.wrapping_sub(t1) / 400);
            dbg(" tot="); dbg_dec(t2.wrapping_sub(t0) / 400);
            dbg("us\r\n");
        }

        true
    }

    // ── Incremental composition ─────────────────────────────────────────

    /// Compose scroll buffer rows from self.composed_up_to .. target.
    /// Each row gets: starfield background + A8 text blend (if text present).
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn compose_rows_up_to(&mut self, dma2d: &mut Dma2dBlitter, target: u32) {
        let stride = CRAWL_W * BPP;
        let star_stride = stride; // starfield has same dimensions

        for row in self.composed_up_to..target {
            let buf_row = row % SCROLL_BUF_ROWS;
            let dst = unsafe { self.scroll_buf.add((buf_row * stride) as usize) };

            // Step 1: Copy one starfield row (tiled: row % CRAWL_H).
            let star_row = row % CRAWL_H;
            let star_src = unsafe { self.starfield.add((star_row * star_stride) as usize) };
            unsafe {
                core::ptr::copy_nonoverlapping(star_src, dst, stride as usize);
            }

            // Step 2: If this row is within text range AND has glyph data, blend it.
            // Rows beyond text_h get pure starfield (no text overlay).
            if row < self.text_h {
                let a8_row_ptr = unsafe { self.text_src.add((row * TEXT_W) as usize) };
                // Check if row has any non-zero pixels (skip empty rows).
                let mut has_data = false;
                for i in 0..TEXT_W as usize {
                    if unsafe { *a8_row_ptr.add(i) } > 0 { has_data = true; break; }
                }
                if has_data {
                    // Nearest-neighbor resample TEXT_W → CRAWL_W into scanline_buf.
                    let step_q16 = (TEXT_W << 16) / CRAWL_W;
                    let mut sx_q16 = step_q16 >> 1;
                    for ox in 0..CRAWL_W as usize {
                        let sx = (sx_q16 >> 16) as usize;
                        self.scanline_buf[ox] = if sx < TEXT_W as usize {
                            unsafe { *a8_row_ptr.add(sx) }
                        } else {
                            0
                        };
                        sx_q16 += step_q16;
                    }

                    // Copy to D2 SRAM for DMA2D access.
                    let d2_ptr = D2_SCANLINE as *mut u8;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            self.scanline_buf.as_ptr(), d2_ptr, CRAWL_W as usize,
                        );
                    }

                    // DMA2D A8 blend onto the starfield row.
                    dma2d.blend_a8_color(d2_ptr, CRAWL_W, 1, YELLOW, dst, stride);
                }
            }
        }
        self.composed_up_to = target;
    }

    // ── Starfield ───────────────────────────────────────────────────────

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn render_starfield(&mut self, dma2d: &mut Dma2dBlitter) {
        let stride = CRAWL_W * BPP;
        dma2d.fill_raw(self.starfield, stride, CRAWL_W, CRAWL_H, BG_COLOR, PixelFmt::Argb8888);

        let mut rng = 0xDEAD_BEEFu32;
        let pixel_count = CRAWL_W * CRAWL_H;
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
