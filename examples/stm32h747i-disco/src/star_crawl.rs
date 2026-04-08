//! Star Wars opening crawl effect with DMA2D-stepped rendering.
//!
//! The crawl is rendered as a round-robin task. Each call advances exactly
//! one small unit of work and then returns so the main loop can service touch,
//! serial, and new DMA completions without spin-waiting.

use rlvgl::core::packed_font::PackedFont;

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl::platform::blit::PixelFmt;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl::platform::dma2d::Dma2dBlitter;

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

/// SDRAM base address for crawl buffers.
const CRAWL_BASE: usize = 0xD100_0000;

/// D2 SRAM address for FIR output — DMA2D-accessible, CPU-writable.
const D2_SCANLINE: usize = 0x3000_0000;

#[derive(Copy, Clone, Eq, PartialEq)]
enum RenderStage {
    Idle = 0,
    StartStarRow = 1,
    WaitStarRow = 2,
    ProcessTextRow = 3,
    WaitTextBlend = 4,
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
        self.stage == RenderStage::WaitStarRow || self.stage == RenderStage::WaitTextBlend
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
        let line_h = (self.font.height as u32 * LINE_SPACING_NUM) / LINE_SPACING_DEN;
        self.text_h = 120 + self.lines.len() as u32 * line_h + CRAWL_H;
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
            self.stage = RenderStage::StartStarRow;
        }

        let dma_error = dma2d.read_error();
        if dma_error != 0 {
            self.diag_last_error = dma_error.min(u16::MAX as u32) as u16;
            self.drop_frame();
            return StepResult::Pending;
        }

        match self.stage {
            RenderStage::Idle => StepResult::Pending,
            RenderStage::StartStarRow => {
                // Don't start DMA2D burst transfers until LTDC scan is done.
                // ERIF_FLAG is set by ISR when scan completes; present() clears
                // it, so this naturally waits one scan period after present.
                if !crate::ERIF_FLAG.load(core::sync::atomic::Ordering::Acquire) {
                    return StepResult::Pending;
                }
                let star_row = (self.frame_star_row + self.bg_row) % STAR_ROWS;
                let src = unsafe { self.starfield.add((star_row * STAR_STRIDE) as usize) };
                let dst = unsafe { self.back_buf.add((self.bg_row * self.fb_w * BPP) as usize) };
                #[cfg(not(feature = "c_hal"))]
                crate::dma2d_irq::note_start();
                crate::scope_probe::dma2d_active();
                dma2d.start_blit_raw(
                    src as *const u8,
                    STAR_STRIDE,
                    dst,
                    self.fb_w * BPP,
                    FB_W,
                    1,
                    PixelFmt::Argb8888,
                );
                self.stage = RenderStage::WaitStarRow;
                StepResult::Pending
            }
            RenderStage::WaitStarRow => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                if dma2d.poll_complete() {
                    dma2d.ack_complete();
                }
                crate::scope_probe::dma2d_idle();
                self.bg_row += 1;
                if self.bg_row >= FB_H {
                    // DMA2D wrote starfield directly to SDRAM, bypassing D-cache.
                    // Invalidate the entire D-cache so the CPU text-blend loop
                    // reads fresh starfield pixels instead of stale cached data.
                    unsafe {
                        cortex_m::asm::dsb();
                        // CM7 D-cache: 32KB, 4-way, 32B lines → 256 sets
                        // DCISW at 0xE000_EF60: way[31:30], set[12:5]
                        for way in 0..4u32 {
                            for set in 0..256u32 {
                                (0xE000_EF60u32 as *mut u32)
                                    .write_volatile((way << 30) | (set << 5));
                            }
                        }
                        cortex_m::asm::dsb();
                        cortex_m::asm::isb();
                    }
                    self.stage = RenderStage::ProcessTextRow;
                    self.text_row = 0;
                } else {
                    self.stage = RenderStage::StartStarRow;
                }
                StepResult::Pending
            }
            RenderStage::ProcessTextRow => {
                if self.text_row >= CRAWL_H {
                    self.finish_frame();
                    return StepResult::FrameReady;
                }
                let text_row_i = self.frame_scroll_px + self.text_row as i32;
                if text_row_i >= 0 && (text_row_i as u32) < self.text_h {
                    let src_row = text_row_i as u32;
                    let target_w = TOP_W + (BOT_W - TOP_W) * self.text_row / (CRAWL_H - 1);
                    let dst_x_off = (CRAWL_W - target_w) / 2;
                    let dst_col = self.fb_w - 1 - self.text_row;
                    self.diag_last_target_w = target_w.min(u16::MAX as u32) as u16;
                    self.diag_last_text_src_row = src_row.min(u16::MAX as u32) as u16;
                    if self.fir_resample_text_row(src_row, target_w) {
                        self.diag_rows_with_text = self.diag_rows_with_text.saturating_add(1);
                        // CPU blend (reference path — known working).
                        let dst_stride = (self.fb_w * BPP) as usize;
                        let mut blended = 0u32;
                        for i in 0..target_w as usize {
                            let alpha = self.scanline_buf[i] as u32;
                            if alpha == 0 {
                                continue;
                            }
                            let dst_off = (dst_x_off as usize + i) * dst_stride
                                + dst_col as usize * BPP as usize;
                            let dst_ptr =
                                unsafe { self.back_buf.add(dst_off) as *mut u32 };
                            let star = unsafe { dst_ptr.read_volatile() };
                            let inv = 255 - alpha;
                            let r = (((YELLOW >> 16) & 0xFF) * alpha
                                + ((star >> 16) & 0xFF) * inv)
                                / 255;
                            let g = (((YELLOW >> 8) & 0xFF) * alpha
                                + ((star >> 8) & 0xFF) * inv)
                                / 255;
                            let b =
                                ((YELLOW & 0xFF) * alpha + (star & 0xFF) * inv) / 255;
                            unsafe {
                                dst_ptr.write_volatile(
                                    0xFF00_0000 | (r << 16) | (g << 8) | b,
                                );
                            }
                            blended += 1;
                        }
                        self.diag_last_blended_pixels =
                            blended.min(u16::MAX as u32) as u16;
                        if blended != 0 {
                            self.diag_rows_blended =
                                self.diag_rows_blended.saturating_add(1);
                        }
                    }
                }
                self.text_row += 1;
                StepResult::Pending
            }
            RenderStage::WaitTextBlend => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                if dma2d.poll_complete() {
                    dma2d.ack_complete();
                }
                crate::scope_probe::dma2d_idle();
                self.text_row += 1;
                self.stage = RenderStage::ProcessTextRow;
                StepResult::Pending
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
        let mut cur_y = 120u32;
        for &line in self.lines {
            let text_w = self.font.measure(line);
            let mut cx = ((TEXT_W as i32 - text_w) / 2).max(0);
            for ch in line.chars() {
                if let Some(glyph) = self.font.glyph(ch) {
                    let gw = glyph.width as usize;
                    let gh = glyph.height as usize;
                    let data_off = glyph.offset as usize;
                    let gy = cur_y as i32 + self.font.ascent as i32
                        - glyph.ymin as i32
                        - gh as i32;
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

    pub fn stage_code(&self) -> u32 {
        0
    }

    pub fn frame_id(&self) -> u32 {
        0
    }

    pub fn waiting_for_dma(&self) -> bool {
        false
    }

    pub fn drop_frame(&mut self) {}

    pub fn diag_words(&self) -> (u32, u32, u32, u32) {
        (0, 0, 0, 0)
    }
}
