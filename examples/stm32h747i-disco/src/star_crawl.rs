//! Star Wars opening crawl effect with DMA2D-stepped rendering.
//!
//! The crawl is rendered as a round-robin task. Each call advances exactly
//! one small unit of work and then returns so the main loop can service touch,
//! serial, and new DMA completions without spin-waiting.

use rlvgl_core::packed_font::PackedFont;

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::blit::PixelFmt;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::dma2d::Dma2dBlitter;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};

/// Portrait framebuffer width.
const FB_W: u32 = 480;
/// Portrait rows used by the crawl.
const FB_H: u32 = 720;
/// Landscape crawl width.
const CRAWL_W: u32 = FB_H;
/// Landscape crawl height.
const CRAWL_H: u32 = FB_W;
/// Pre-rendered text line width in pixels.
const TEXT_W: u32 = 600;
/// Perspective width at the top of the crawl.
///
/// Keep this narrower than the source text width so the FIR pass has enough
/// decimation headroom to smooth distant glyph edges.
const TOP_W: u32 = 360;
/// Perspective width at the bottom of the crawl.
const BOT_W: u32 = 600;
/// ARGB8888 bytes per pixel.
const BPP: u32 = 4;
/// Double-height mirrored portrait starfield.
const STAR_ROWS: u32 = FB_H * 2;
const STAR_STRIDE: u32 = FB_W * BPP;
const STAR_SIZE: usize = (STAR_ROWS * STAR_STRIDE) as usize;
/// Line spacing multiplier for text layout.
const LINE_SPACING_NUM: u32 = 3;
const LINE_SPACING_DEN: u32 = 2;
/// Target scroll speed in pixels per second.
const SCROLL_PX_PER_SEC: u32 = 40;
/// Yellow DMA2D blend colour.
const YELLOW: u32 = 0x00FF_D700;
/// Background colour for the starfield.
const BG_COLOR: u32 = 0xFF0A_0A20;
/// Number of stars to scatter.
const STAR_COUNT: usize = 200;

/// Splash graphic crop: 384×384 from DESKTOP_PRISTINE.
/// Portrait coords in the framebuffer at 0xD030_0000.
const GRAPHIC_CROP_X: u32 = 48;
const GRAPHIC_CROP_Y: u32 = 208;
const GRAPHIC_SIZE: u32 = 384;
const GRAPHIC_GAP: u32 = 40; // gap below graphic before text

/// Softoboros letter logo: 250×64 RLVGLRAW.
const LOGO_GAP: u32 = 40; // gap above logo after text

/// SDRAM base address for crawl buffers.
const CRAWL_BASE: usize = 0xD100_0000;

/// D2 SRAM base for the portrait A8 text buffer.
///
/// 480 × 600 = 288,000 bytes. D2 SRAM is 288 KiB total; IPC mailbox at
/// 0x3004_7000 leaves 2,816 bytes headroom.
const A8_BUF: usize = 0x3000_0000;
const A8_WIDTH: u32 = FB_W; // 480 portrait columns
const A8_HEIGHT: u32 = BOT_W; // 600 rows (max text extent)
/// Minimum dst_x_off across all text rows (when target_w == BOT_W).
const A8_Y_BASE: u32 = (CRAWL_W - BOT_W) / 2; // 60
const A8_SIZE: usize = (A8_WIDTH * A8_HEIGHT) as usize; // 288,000

#[derive(Copy, Clone, Eq, PartialEq)]
enum RenderStage {
    Idle = 0,
    RenderFrame = 1,
    StartTextBlend = 2,
    WaitTextBlend = 3,
}

/// Result of advancing the crawl task by one step.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum StepResult {
    /// Crawl is inactive.
    Idle,
    /// More work remains before the frame can be presented.
    Pending,
    /// The back buffer is complete and ready to present.
    FrameReady,
    /// Crawl reached the end of the script and deactivated.
    Finished,
}

/// Star crawl renderer with non-blocking internal state.
pub struct StarCrawl {
    active: bool,
    scroll_q8: i32,
    scroll_speed_q8: i32,
    star_scroll_q8: i32,

    starfield: *mut u8,
    text_src: *mut u8,
    text_h: u32,

    frame_id: u32,
    frame_active: bool,
    frame_scroll_px: i32,
    frame_star_row: u32,
    back_buf: *mut u8,
    fb_w: u32,

    stage: RenderStage,
    bg_row: u32,
    text_row: u32,

    scanline_buf: [u8; CRAWL_W as usize],

    lines: &'static [&'static str],
    font: &'static PackedFont,
    diag_rows_with_text: u16,
    diag_rows_blended: u16,
    diag_last_blended_pixels: u16,
    diag_last_target_w: u16,
    diag_last_text_src_row: u16,
    diag_completed_frames: u16,
    diag_dropped_frames: u16,
    diag_last_error: u16,
}

impl StarCrawl {
    pub const fn new(
        font: &'static PackedFont,
        lines: &'static [&'static str],
        frame_hz: u32,
    ) -> Self {
        Self {
            active: false,
            scroll_q8: 0,
            scroll_speed_q8: ((SCROLL_PX_PER_SEC * 256) / frame_hz) as i32,
            star_scroll_q8: 0,
            starfield: core::ptr::null_mut(),
            text_src: core::ptr::null_mut(),
            text_h: 0,
            frame_id: 0,
            frame_active: false,
            frame_scroll_px: 0,
            frame_star_row: 0,
            back_buf: core::ptr::null_mut(),
            fb_w: FB_W,
            stage: RenderStage::Idle,
            bg_row: 0,
            text_row: 0,
            scanline_buf: [0u8; CRAWL_W as usize],
            lines,
            font,
            diag_rows_with_text: 0,
            diag_rows_blended: 0,
            diag_last_blended_pixels: 0,
            diag_last_target_w: 0,
            diag_last_text_src_row: 0,
            diag_completed_frames: 0,
            diag_dropped_frames: 0,
            diag_last_error: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.drop_frame();
    }

    /// Advance the logical scroll position after a successful present.
    pub fn advance_scroll(&mut self) {
        self.scroll_q8 += self.scroll_speed_q8;
        self.star_scroll_q8 += self.scroll_speed_q8 / 3;
    }

    /// Numeric stage identifier for telemetry.
    pub fn stage_code(&self) -> u32 {
        self.stage as u32
    }

    /// Current frame counter for telemetry.
    pub fn frame_id(&self) -> u32 {
        self.frame_id
    }

    /// Returns `true` when the crawl is parked on a DMA completion.
    pub fn waiting_for_dma(&self) -> bool {
        self.stage == RenderStage::WaitTextBlend
    }

    /// Packed crawl diagnostics for D3 SRAM / serial telemetry.
    pub fn diag_words(&self) -> (u32, u32, u32, u32) {
        let flags = ((self.active as u32) << 7)
            | ((self.frame_active as u32) << 6)
            | ((self.waiting_for_dma() as u32) << 4)
            | (((self.diag_last_error != 0) as u32) << 3);
        (
            ((self.stage as u32) << 24) | (flags << 16) | (self.frame_id & 0xFFFF),
            ((self.diag_last_text_src_row as u32) << 16) | self.text_row.min(u16::MAX as u32),
            ((self.diag_last_target_w as u32) << 16) | self.diag_last_blended_pixels as u32,
            ((self.diag_rows_with_text.min(0xFF) as u32) << 24)
                | ((self.diag_rows_blended.min(0xFF) as u32) << 16)
                | ((self.diag_completed_frames.min(0xFF) as u32) << 8)
                | (self.diag_dropped_frames.min(0xFF) as u32),
        )
    }

    fn reset_frame_state(&mut self) {
        self.frame_active = false;
        self.stage = RenderStage::Idle;
        self.bg_row = 0;
        self.text_row = 0;
    }

    fn finish_frame(&mut self) {
        self.diag_completed_frames = self.diag_completed_frames.saturating_add(1);
        self.reset_frame_state();
    }

    /// Drop the in-progress frame and restart on the next scheduler pass.
    pub fn drop_frame(&mut self) {
        if self.frame_active {
            self.diag_dropped_frames = self.diag_dropped_frames.saturating_add(1);
        }
        self.reset_frame_state();
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn activate(&mut self, dma2d: &mut Dma2dBlitter) {
        // Enable D2 SRAM1 + SRAM2 + SRAM3 clocks for the A8 portrait buffer.
        // RCC_AHB2ENR: bit 29 = SRAM1EN, 30 = SRAM2EN, 31 = SRAM3EN.
        unsafe {
            let ahb2enr = (0x5802_44DCu32) as *mut u32;
            ahb2enr.write_volatile(ahb2enr.read_volatile() | 0xE000_0000);
        }

        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        // Layout: margin(120) + graphic(384) + gap(40) + text + gap(40) + logo(64) + padding(CRAWL_H)
        let logo_h = Self::logo_height();
        self.text_h = 120
            + GRAPHIC_SIZE
            + GRAPHIC_GAP
            + self.lines.len() as u32 * line_h
            + LOGO_GAP
            + logo_h
            + CRAWL_H;
        self.starfield = CRAWL_BASE as *mut u8;
        self.text_src = (CRAWL_BASE + STAR_SIZE) as *mut u8;

        unsafe {
            core::ptr::write_bytes(self.text_src, 0, (TEXT_W * self.text_h) as usize);
        }

        self.render_starfield(dma2d);
        self.pre_render_text(line_h);
        self.scroll_q8 = -((CRAWL_H as i32) << 8);
        self.star_scroll_q8 = 0;
        self.frame_id = 0;
        self.active = true;
        self.drop_frame();
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        back_buf: *mut u8,
        fb_w: u32,
        _fb_h: u32,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        if !self.active {
            return StepResult::Idle;
        }

        if !self.frame_active {
            let scroll_px = self.scroll_q8 >> 8;
            if scroll_px >= (self.text_h + CRAWL_H) as i32 {
                self.deactivate();
                return StepResult::Finished;
            }

            self.frame_id = self.frame_id.wrapping_add(1);
            self.frame_active = true;
            self.frame_scroll_px = scroll_px;
            self.frame_star_row = ((self.star_scroll_q8 >> 8) as u32) % STAR_ROWS;
            self.back_buf = back_buf;
            self.fb_w = fb_w;
            self.bg_row = 0;
            self.text_row = 0;
            self.diag_rows_with_text = 0;
            self.diag_rows_blended = 0;
            self.diag_last_blended_pixels = 0;
            self.diag_last_target_w = 0;
            self.diag_last_text_src_row = 0;
            self.diag_last_error = 0;
            // Zero the portrait A8 buffer in D2 SRAM before FIR fills columns.
            unsafe {
                core::ptr::write_bytes(A8_BUF as *mut u8, 0, A8_SIZE);
            }
            self.stage = RenderStage::RenderFrame;
        }

        let dma_error = dma2d.read_error();
        if dma_error != 0 {
            self.diag_last_error = dma_error.min(u16::MAX as u32) as u16;
            self.drop_frame();
            return StepResult::Pending;
        }

        match self.stage {
            RenderStage::Idle => StepResult::Pending,
            RenderStage::RenderFrame => {
                // Gate: wait for LTDC scan to finish before first DMA2D burst.
                // Skip the gate on the very first frame after activation
                // (frame_id == 1) — no scan is pending in adapted command
                // mode so ERIF will never fire until we present().
                if self.bg_row == 0
                    && self.frame_id > 1
                    && !sync.erif_is_set()
                {
                    return StepResult::Pending;
                }

                // --- DMA2D starfield management ---
                // Use the ISR completion latch instead of poll_complete():
                // the DMA2D ISR clears TCIF before poll_complete() can
                // see it, causing a race that prevents bg_row from advancing.
                if !dma2d.is_in_flight() && sync.take_complete() {
                    sync.dma2d_idle();
                    self.bg_row += 1;
                }

                if self.bg_row < FB_H && !dma2d.is_in_flight() {
                    // Admission: each row blit is ~500 cycles.
                    // Don't start if we'd run into the next scan window.
                    if !sync.dma2d_admits(500) {
                        return StepResult::Pending;
                    }
                    let star_row = (self.frame_star_row + self.bg_row) % STAR_ROWS;
                    let src = unsafe { self.starfield.add((star_row * STAR_STRIDE) as usize) };
                    let dst =
                        unsafe { self.back_buf.add((self.bg_row * self.fb_w * BPP) as usize) };
                    sync.note_start();
                    sync.dma2d_active();
                    dma2d.start_blit_raw(
                        src as *const u8,
                        STAR_STRIDE,
                        dst,
                        self.fb_w * BPP,
                        FB_W,
                        1,
                        PixelFmt::Argb8888,
                    );
                }

                // --- CPU FIR: one text row → D2 SRAM A8 portrait buffer ---
                if self.text_row < CRAWL_H {
                    let text_row_i = self.frame_scroll_px + self.text_row as i32;
                    if text_row_i >= 0 && (text_row_i as u32) < self.text_h {
                        let src_row = text_row_i as u32;
                        let target_w = TOP_W + (BOT_W - TOP_W) * self.text_row / (CRAWL_H - 1);
                        let dst_x_off = (CRAWL_W - target_w) / 2;
                        self.diag_last_target_w = target_w.min(u16::MAX as u32) as u16;
                        self.diag_last_text_src_row = src_row.min(u16::MAX as u32) as u16;
                        if self.fir_resample_text_row(src_row, target_w) {
                            self.diag_rows_with_text = self.diag_rows_with_text.saturating_add(1);
                            // Copy FIR output into portrait A8 column in D2 SRAM.
                            let x_col = (self.fb_w - 1 - self.text_row) as usize;
                            let y_off = (dst_x_off - A8_Y_BASE) as usize;
                            for i in 0..target_w as usize {
                                unsafe {
                                    let a8_ptr = (A8_BUF + (y_off + i) * A8_WIDTH as usize + x_col)
                                        as *mut u8;
                                    a8_ptr.write_volatile(self.scanline_buf[i]);
                                }
                            }
                            self.diag_rows_blended = self.diag_rows_blended.saturating_add(1);
                        }
                    }
                    self.text_row += 1;
                }

                // --- Check completion ---
                let star_done = self.bg_row >= FB_H;
                let text_done = self.text_row >= CRAWL_H;
                let dma_done = !dma2d.is_in_flight();
                if star_done && text_done && dma_done {
                    self.stage = RenderStage::StartTextBlend;
                }
                StepResult::Pending
            }
            RenderStage::StartTextBlend => {
                // A8 blend is ~800K cycles. Don't start if budget is tight.
                if !sync.dma2d_admits(800_000) {
                    return StepResult::Pending;
                }

                // Flush D-cache for the A8 buffer so DMA2D sees all CPU
                // writes. D2 SRAM at 0x3000_0000 is Write-Back cached
                // under the default Cortex-M7 background map.
                dcache_clean_range(A8_BUF, A8_SIZE);

                // Single DMA2D A8→ARGB blend of the entire text layer.
                let dst_offset = (A8_Y_BASE * self.fb_w * BPP) as usize;
                let dst = unsafe { self.back_buf.add(dst_offset) };
                sync.note_start();
                sync.dma2d_active();
                dma2d.start_blend_a8_color(
                    A8_BUF as *const u8,
                    A8_WIDTH,
                    A8_HEIGHT,
                    YELLOW,
                    dst,
                    self.fb_w * BPP,
                );
                self.stage = RenderStage::WaitTextBlend;
                StepResult::Pending
            }
            RenderStage::WaitTextBlend => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                // ISR already cleared TCIF; use latch to confirm done.
                let _ = sync.take_complete();
                sync.dma2d_idle();
                self.finish_frame();
                StepResult::FrameReady
            }
        }
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn text_row_ptr(&self, text_row: u32) -> *mut u8 {
        unsafe { self.text_src.add((text_row * TEXT_W) as usize) }
    }

    /// Synchronously rasterize all text lines into `text_src` (A8 format).
    ///
    /// Uses `.get()` for bounds-checked font data access. Called once from
    /// `activate()` before the scroll begins.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn pre_render_text(&mut self, line_h: u32) {
        // Blit the splash graphic (384×384) as A8 alpha at the top.
        self.blit_splash_crop_a8(120);

        let mut cur_y = 120 + GRAPHIC_SIZE + GRAPHIC_GAP;
        for &line in self.lines {
            let text_w = self.font.measure(line);
            let mut cx = ((TEXT_W as i32 - text_w) / 2).max(0);
            for ch in line.chars() {
                if let Some(glyph) = self.font.glyph(ch) {
                    let gw = glyph.width as usize;
                    let gh = glyph.height as usize;
                    let data_off = glyph.offset as usize;
                    let gy = cur_y as i32 + self.font.ascent as i32 - glyph.ymin as i32 - gh as i32;
                    for row in 0..gh {
                        let dy = gy + row as i32;
                        if dy < 0 || dy as u32 >= self.text_h {
                            continue;
                        }
                        for col in 0..gw {
                            let src_idx = data_off + row * gw + col;
                            if let Some(&alpha) = self.font.data.get(src_idx) {
                                if alpha == 0 {
                                    continue;
                                }
                                let dx = cx + col as i32;
                                if dx < 0 || dx as u32 >= TEXT_W {
                                    continue;
                                }
                                unsafe {
                                    self.text_src
                                        .add(dy as usize * TEXT_W as usize + dx as usize)
                                        .write_volatile(alpha);
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

        // Blit the softoboros letter logo after the text.
        self.blit_logo_a8(cur_y + LOGO_GAP);
    }

    /// Parse logo RLVGLRAW header and return height.
    fn logo_height() -> u32 {
        let raw: &[u8] = include_bytes!("../assets/icons/softoboros-letter-logo.raw");
        if raw.len() < 24 {
            return 0;
        }
        u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]])
    }

    /// Blit the 384×384 splash crop from DESKTOP_PRISTINE into the text
    /// source buffer as A8 alpha. Converts white→transparent, color→opaque.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn blit_splash_crop_a8(&mut self, dst_y: u32) {
        const PRISTINE: *const u8 = 0xD030_0000 as *const u8;
        const PFB_W: u32 = 480; // portrait framebuffer width

        let x_off = ((TEXT_W - GRAPHIC_SIZE) / 2) as usize;

        for row in 0..GRAPHIC_SIZE {
            for col in 0..GRAPHIC_SIZE {
                let fb_off = ((GRAPHIC_CROP_Y + row) as usize * PFB_W as usize
                    + (GRAPHIC_CROP_X + col) as usize)
                    * 4;
                let (b, g, r) = unsafe {
                    (
                        *PRISTINE.add(fb_off),
                        *PRISTINE.add(fb_off + 1),
                        *PRISTINE.add(fb_off + 2),
                    )
                };
                // White background → alpha 0; colored → darker = more opaque
                let lum = (r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8;
                let alpha = 255u8.saturating_sub(lum as u8);
                if alpha > 0 {
                    // -90° CCW rotation: portrait (col, row) →
                    // landscape (row, GRAPHIC_SIZE-1-col) so the
                    // splash appears upright in the landscape crawl.
                    let dst_off = (dst_y + GRAPHIC_SIZE - 1 - col) as usize * TEXT_W as usize
                        + x_off
                        + row as usize;
                    if dst_off < (TEXT_W * self.text_h) as usize {
                        unsafe {
                            self.text_src.add(dst_off).write_volatile(alpha);
                        }
                    }
                }
            }
        }
    }

    /// Blit the softoboros letter logo (RLVGLRAW) into the text source
    /// buffer as A8 alpha.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn blit_logo_a8(&mut self, dst_y: u32) {
        let raw: &[u8] = include_bytes!("../assets/icons/softoboros-letter-logo.raw");
        if raw.len() < 24 {
            return;
        }
        let w = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
        let h = u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]);
        let pixels = &raw[24..];
        let x_off = ((TEXT_W - w) / 2) as usize;

        for row in 0..h {
            for col in 0..w {
                let off = (row * w + col) as usize * 4;
                if off + 3 >= pixels.len() {
                    break;
                }
                let a = pixels[off + 3];
                if a > 0 {
                    let dst_off = (dst_y + row) as usize * TEXT_W as usize + x_off + col as usize;
                    if dst_off < (TEXT_W * self.text_h) as usize {
                        unsafe {
                            self.text_src.add(dst_off).write_volatile(a);
                        }
                    }
                }
            }
        }
    }

    /// FIR-resample one text source row from `TEXT_W` to `target_w`.
    ///
    /// Returns `true` when the output contains at least one non-zero alpha.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn fir_resample_text_row(&mut self, text_row: u32, target_w: u32) -> bool {
        if text_row >= self.text_h || target_w == 0 {
            return false;
        }

        let src = self.text_row_ptr(text_row) as *const u8;
        let out = &mut self.scanline_buf[..CRAWL_W as usize];
        out[..target_w as usize].fill(0);

        let words = TEXT_W as usize / 4;
        let src32 = src as *const u32;
        let mut all_zero = true;
        for i in 0..words {
            if unsafe { src32.add(i).read_volatile() } != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            return false;
        }

        let step_q16 = (TEXT_W << 16) / target_w;
        let mut any_nonzero = false;
        let mut ox = 0usize;
        while ox < target_w as usize {
            let cx_q16 = ox as u32 * step_q16 + (step_q16 >> 1);
            let cx = (cx_q16 >> 16) as i32;

            if cx >= 3 && cx + 3 < TEXT_W as i32 {
                let center = unsafe { *src.add(cx as usize) };
                if center == 0 {
                    let any_neighbor = unsafe {
                        *src.add((cx - 3) as usize) != 0
                            || *src.add((cx - 2) as usize) != 0
                            || *src.add((cx - 1) as usize) != 0
                            || *src.add((cx + 1) as usize) != 0
                            || *src.add((cx + 2) as usize) != 0
                            || *src.add((cx + 3) as usize) != 0
                    };
                    if !any_neighbor {
                        ox += 1;
                        continue;
                    }
                }
            }

            let mut acc: u32 = 0;
            let mut wsum: u32 = 0;
            const TAPS: [(i32, u32); 7] = [
                (-3, 8),
                (-2, 24),
                (-1, 48),
                (0, 64),
                (1, 48),
                (2, 24),
                (3, 8),
            ];
            for &(tap, w) in &TAPS {
                let sx = cx + tap;
                if sx >= 0 && sx < TEXT_W as i32 {
                    let v = unsafe { *src.add(sx as usize) } as u32;
                    acc += v * w;
                    wsum += w;
                }
            }
            let alpha = if wsum > 0 {
                (acc / wsum).min(255) as u8
            } else {
                0
            };
            out[ox] = alpha;
            any_nonzero |= alpha != 0;
            ox += 1;
        }

        any_nonzero
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn render_starfield(&mut self, dma2d: &mut Dma2dBlitter) {
        dma2d.fill_raw(
            self.starfield,
            STAR_STRIDE,
            FB_W,
            STAR_ROWS,
            BG_COLOR,
            PixelFmt::Argb8888,
        );

        let mut rng = 0xDEAD_BEEFu32;
        let pixel_count = FB_W * FB_H;
        for _ in 0..STAR_COUNT {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let idx = rng % pixel_count;
            let brightness = 128 + (rng >> 24) as u8 / 2;
            let color = 0xFF00_0000
                | ((brightness as u32) << 16)
                | ((brightness as u32) << 8)
                | brightness as u32;
            unsafe {
                (self.starfield.add((idx * BPP) as usize) as *mut u32).write_volatile(color);
            }
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
        }

        let row_bytes = STAR_STRIDE as usize;
        for i in 0..FB_H {
            let src = unsafe { self.starfield.add((i * STAR_STRIDE) as usize) };
            let dst_row = STAR_ROWS - 1 - i;
            let dst = unsafe { self.starfield.add((dst_row * STAR_STRIDE) as usize) };
            unsafe {
                core::ptr::copy_nonoverlapping(src, dst, row_bytes);
            }
        }
    }
}

#[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
impl StarCrawl {
    pub fn activate(&mut self) {}

    pub fn tick(&mut self, _back_buf: *mut u8, _fb_w: u32, _fb_h: u32) -> StepResult {
        StepResult::Idle
    }
}

// ── D-cache maintenance ─────────────────────────────────────────────────────

/// Clean D-cache lines covering `[addr, addr+size)` so DMA2D sees CPU writes.
///
/// D2 SRAM at 0x3000_0000 is Write-Back Write-Allocate under the default
/// Cortex-M7 background map. Without a clean, DMA2D reads stale data.
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
fn dcache_clean_range(addr: usize, size: usize) {
    const DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32;
    const LINE_SIZE: usize = 32;
    let start = addr & !(LINE_SIZE - 1);
    let end = (addr + size + LINE_SIZE - 1) & !(LINE_SIZE - 1);
    let mut a = start;
    while a < end {
        unsafe {
            DCCMVAC.write_volatile(a as u32);
        }
        a += LINE_SIZE;
    }
    cortex_m::asm::dsb();
}
