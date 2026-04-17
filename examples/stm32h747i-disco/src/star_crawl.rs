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
/// Portrait rows used by the crawl. Sized to match the full portrait
/// FB height (480×800 on STM32H747I-DISCO NT35510), so direct blits
/// from the crawl scratch cover the entire display with no rotation
/// step. Previously 720, leaving 80 rows of BG at the bottom of
/// portrait (= 80 landscape cols of BG at one edge).
const FB_H: u32 = 800;
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
    /// Starfield has been rendered into the persistent `layer_buf`; issue
    /// a single full-frame M2M blit from layer into `back_buf` so text
    /// can blend on top of a clean stars-only background.
    ComposeLayer = 4,
    /// Wait for the ComposeLayer M2M blit to finish before starting the
    /// text A8 blend into `back_buf`.
    WaitCompose = 5,
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

/// How the per-frame background (starfield) is rendered into back_buf.
///
/// See the per-platform setter call in the host's `rlvgl_init` /
/// bare-metal main for how each build selects a mode.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum RenderMode {
    /// Full re-blit: every frame writes all FB_H rows of starfield
    /// via per-row DMA2D ops. Interleaves with CPU FIR text work for
    /// parallelism. Works regardless of whether back_buf is the same
    /// pointer across frames (bare-metal alternates front/back FBs).
    Full,
    /// Incremental: DMA2D M2M-shifts existing back_buf up by the
    /// frame-to-frame scroll delta, then re-blits only the newly
    /// exposed bottom rows. Requires back_buf to be a persistent
    /// buffer across frames (same pointer every frame) — typically a
    /// dedicated scratch like 0xD180_0000. Falls back to Full
    /// automatically on the first frame after activation or when the
    /// delta exceeds FB_H (full re-render cheaper than partial shift).
    Incremental,
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
    /// Persistent starfield layer buffer. When non-null, starfield rows
    /// render into this buffer instead of directly into `back_buf`, and
    /// an internal compose stage (`ComposeLayer` / `WaitCompose`) blits
    /// layer → back_buf before text blends on top. This decouples
    /// the starfield (opaque, scrolls at 1/3 rate, safe to incremental-
    /// shift) from the text A8 blend (alpha, scrolls at full rate —
    /// shifting a blended buffer causes alpha accumulation / flicker).
    ///
    /// When null, legacy behavior: stars and text both render into
    /// `back_buf` (matches bare-metal `RenderMode::Full` today).
    layer_buf: *mut u8,
    fb_w: u32,

    stage: RenderStage,
    bg_row: u32,
    text_row: u32,

    /// Host-configured rendering strategy for the starfield per frame.
    /// See [`RenderMode`]. Defaults to [`RenderMode::Full`] for
    /// bare-metal compatibility.
    render_mode: RenderMode,
    /// Previous frame's `frame_star_row`. Used by Incremental mode to
    /// compute the scroll delta (how many rows to shift + re-fill).
    /// `u32::MAX` sentinel = no prior frame (first frame after
    /// activate; Incremental falls back to Full).
    last_frame_star_row: u32,
    /// In Incremental mode, set to true after the shift DMA2D is
    /// issued at new-frame-start so we don't issue it again during
    /// the same frame.
    incr_shift_done: bool,

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

    // Two-phase pipeline state (used by compose_and_blend / prep_next_frame)
    prep_bg_row: u32,
    prep_text_row: u32,
    prep_started: bool,
    prep_shift_done: bool,
    prep_frame_scroll_px: i32,
    prep_frame_star_row: u32,

    // Resumable compose state — survives across back porches
    compose_row: u32,         // starfield row progress
    compose_blend_y: u32,     // A8 blend stripe progress
    compose_star_row: u32,    // starfield scroll offset for this compose
    compose_active: bool,     // compose in progress (not yet buf_ready)
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
            layer_buf: core::ptr::null_mut(),
            fb_w: FB_W,
            stage: RenderStage::Idle,
            bg_row: 0,
            text_row: 0,
            render_mode: RenderMode::Full,
            last_frame_star_row: u32::MAX,
            incr_shift_done: false,
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
            prep_bg_row: 0,
            prep_text_row: 0,
            prep_started: false,
            prep_shift_done: false,
            prep_frame_scroll_px: 0,
            prep_frame_star_row: 0,
            compose_row: 0,
            compose_blend_y: 0,
            compose_star_row: 0,
            compose_active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Select between [`RenderMode::Full`] (default) and
    /// [`RenderMode::Incremental`]. Safe to call any time; takes
    /// effect at the next frame start. Incremental mode requires the
    /// host to pass the SAME `back_buf` pointer on every call to
    /// [`Self::tick`]; otherwise the shift-from-previous-frame
    /// assumption is violated.
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.render_mode = mode;
        // Reset the delta-tracking sentinel so the next frame-start
        // picks the Full path (no prior frame to shift from).
        self.last_frame_star_row = u32::MAX;
    }

    /// Configure a persistent starfield layer buffer, distinct from the
    /// per-frame `back_buf`. When set, starfield rows render into `layer`
    /// and an internal compose stage blits layer → back_buf before the
    /// text A8 blend. This is the recommended configuration when the
    /// host alternates `back_buf` between `fb_front` and `fb_back`
    /// (e.g. Zephyr ACM), because it preserves `RenderMode::Incremental`'s
    /// "same buffer every frame" contract without contaminating the
    /// persistent buffer with alpha-blended text.
    ///
    /// Passing `core::ptr::null_mut()` restores legacy behaviour (stars
    /// and text both render into `back_buf`).
    pub fn set_layer_buf(&mut self, layer: *mut u8) {
        self.layer_buf = layer;
        // Buffer identity change invalidates any prior frame's shift
        // anchor; force the next frame to take the Full path.
        self.last_frame_star_row = u32::MAX;
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
        matches!(
            self.stage,
            RenderStage::WaitTextBlend | RenderStage::WaitCompose
        )
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
        // Remember this frame's starfield scroll position so the NEXT
        // frame can compute the delta for Incremental mode.
        self.last_frame_star_row = self.frame_star_row;
        self.reset_frame_state();
    }

    /// Drop the in-progress frame and restart on the next scheduler pass.
    pub fn drop_frame(&mut self) {
        if self.frame_active {
            self.diag_dropped_frames = self.diag_dropped_frames.saturating_add(1);
        }
        // Invalidate the Incremental-mode delta anchor: back_buf may
        // have been partially written or the host may switch to a
        // different buffer next frame, so we must fall back to Full
        // for the next frame.
        self.last_frame_star_row = u32::MAX;
        self.incr_shift_done = false;
        self.prep_started = false;
        self.prep_shift_done = false;
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
            // Incremental mode: if we have a valid prior frame, shift
            // the existing back_buf up by the scroll delta and only
            // re-fill the bottom N rows this frame. Skip shift and fall
            // back to Full re-blit on:
            //   - first frame after activate (no prior)
            //   - delta >= FB_H (no overlap worth preserving)
            //   - Full mode (bare-metal)
            self.incr_shift_done = false;
            let mut start_row: u32 = 0;
            // Starfield DMA destination: layer buffer when set (Zephyr
            // ACM / any host that alternates back_buf), else back_buf.
            // Incremental shift must target the SAME buffer every frame,
            // which is why the layer path is preferred with alternating
            // framebuffers.
            let stars_target = if self.layer_buf.is_null() {
                back_buf
            } else {
                self.layer_buf
            };
            if self.render_mode == RenderMode::Incremental && self.last_frame_star_row != u32::MAX {
                // Delta is signed in modular STAR_ROWS space. We only
                // scroll forward, so expect a small positive delta.
                // Negative or huge deltas (e.g. STAR_ROWS wrap) → full.
                let last = self.last_frame_star_row as i32;
                let cur = self.frame_star_row as i32;
                let raw = cur - last;
                let delta_rows = if raw >= 0 && (raw as u32) < FB_H {
                    raw as u32
                } else {
                    FB_H // marker: fall back to full re-blit
                };
                if delta_rows > 0 && delta_rows < FB_H {
                    // Issue DMA2D M2M shift: copy stars_target rows
                    // [delta..FB_H] → rows [0..FB_H-delta]. Source is
                    // strictly above destination so linear-in-memory
                    // DMA2D has no aliasing.
                    let row_bytes = (FB_W * BPP) as u32;
                    let keep_rows = FB_H - delta_rows;
                    let src_rows = unsafe { stars_target.add((delta_rows * row_bytes) as usize) };
                    dma2d.start_blit_raw(
                        src_rows as *const u8,
                        row_bytes,
                        stars_target,
                        row_bytes,
                        FB_W,
                        keep_rows,
                        PixelFmt::Argb8888,
                    );
                    self.incr_shift_done = true;
                    start_row = keep_rows;
                }
            }
            self.bg_row = start_row;
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
                if self.bg_row == 0 && self.frame_id > 1 && !sync.erif_is_set() {
                    return StepResult::Pending;
                }

                // --- DMA2D starfield management ---
                // Use the ISR completion latch instead of poll_complete():
                // the DMA2D ISR clears TCIF before poll_complete() can
                // see it, causing a race that prevents bg_row from advancing.
                if !dma2d.is_in_flight() && sync.take_complete() {
                    sync.dma2d_idle();
                    if self.incr_shift_done {
                        // That completion was the Incremental-mode shift,
                        // not a per-row blit. Consume the flag without
                        // advancing bg_row — the bottom `delta` rows
                        // still need filling, starting from the current
                        // bg_row value set at new-frame-start.
                        self.incr_shift_done = false;
                    } else {
                        self.bg_row += 1;
                    }
                }

                if self.bg_row < FB_H && !dma2d.is_in_flight() {
                    // Admission: each row blit is ~500 cycles.
                    // Don't start if we'd run into the next scan window.
                    if !sync.dma2d_admits(500) {
                        return StepResult::Pending;
                    }
                    let star_row = (self.frame_star_row + self.bg_row) % STAR_ROWS;
                    let src = unsafe { self.starfield.add((star_row * STAR_STRIDE) as usize) };
                    // Stars render into the persistent layer buffer when
                    // one is configured, else into back_buf (bare-metal).
                    let stars_dst = if self.layer_buf.is_null() {
                        self.back_buf
                    } else {
                        self.layer_buf
                    };
                    let dst = unsafe { stars_dst.add((self.bg_row * self.fb_w * BPP) as usize) };
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
                    // If a persistent starfield layer is configured, the
                    // stars just landed in `layer_buf` — compose it into
                    // `back_buf` before blending text on top. Otherwise
                    // (legacy bare-metal) go straight to text blend.
                    self.stage = if self.layer_buf.is_null() {
                        RenderStage::StartTextBlend
                    } else {
                        RenderStage::ComposeLayer
                    };
                }
                StepResult::Pending
            }
            RenderStage::ComposeLayer => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                // One full-frame M2M blit layer → back_buf. Budget
                // ~400K cycles at 400 MHz (1.5 MB / ~4 bytes per cycle).
                if !sync.dma2d_admits(500_000) {
                    return StepResult::Pending;
                }
                let row_bytes = self.fb_w * BPP;
                sync.note_start();
                sync.dma2d_active();
                dma2d.start_blit_raw(
                    self.layer_buf as *const u8,
                    row_bytes,
                    self.back_buf,
                    row_bytes,
                    self.fb_w,
                    FB_H,
                    PixelFmt::Argb8888,
                );
                self.stage = RenderStage::WaitCompose;
                StepResult::Pending
            }
            RenderStage::WaitCompose => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                // Consume the completion latch and drive PJ6 low before
                // the next DMA2D burst.
                let _ = sync.take_complete();
                sync.dma2d_idle();
                self.stage = RenderStage::StartTextBlend;
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

    // ── Two-phase API (FreeRTOS display driver) ─────────────────────────────

    /// Timer-gated resumable compose. Call once per back porch.
    ///
    /// Runs starfield chunks + A8 blend stripes until `porch_cyc`
    /// cycles have elapsed since the porch started, then returns
    /// `false` (more work next porch). Returns `true` when the
    /// entire compose + blend is done — caller should give buf_ready.
    ///
    /// `porch_start` = DWT_CYCCNT snapshot at ERIF (porch start).
    /// `porch_cyc`   = budget in cycles (e.g. 4_800_000 = 12 ms).
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn compose_tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        fb: *mut u8,
        fb_w: u32,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
        porch_start: u32,
        porch_cyc: u32,
    ) -> bool {
        if !self.active || self.starfield.is_null() {
            return false;
        }

        // First call for a new frame: latch scroll position
        if !self.compose_active {
            self.compose_row = 0;
            self.compose_blend_y = 0;
            self.compose_star_row =
                ((self.star_scroll_q8 >> 8) as u32) % STAR_ROWS;
            self.compose_active = true;
            dcache_clean_range(A8_BUF, A8_SIZE);
        }

        let row_bytes = fb_w * BPP;
        const CHUNK_ROWS: u32 = 50;

        // ── Starfield chunks ────────────────────────────────────
        while self.compose_row < FB_H {
            let elapsed = cyccnt().wrapping_sub(porch_start);
            if elapsed >= porch_cyc {
                return false; // porch budget expired
            }
            let batch = CHUNK_ROWS.min(FB_H - self.compose_row);
            let src_start =
                (self.compose_star_row + self.compose_row) % STAR_ROWS;

            if src_start + batch <= STAR_ROWS {
                let src = unsafe {
                    self.starfield
                        .add((src_start * STAR_STRIDE) as usize)
                };
                let dst = unsafe {
                    fb.add((self.compose_row * row_bytes) as usize)
                };
                sync.dma2d_active();
                dma2d.start_blit_raw(
                    src as *const u8,
                    STAR_STRIDE,
                    dst,
                    row_bytes,
                    fb_w,
                    batch,
                    PixelFmt::Argb8888,
                );
                while dma2d.is_in_flight() {
                    cortex_m::asm::nop();
                }
                sync.dma2d_idle();
            } else {
                let top = STAR_ROWS - src_start;
                let bot = batch - top;
                let src_top = unsafe {
                    self.starfield
                        .add((src_start * STAR_STRIDE) as usize)
                };
                let dst_top = unsafe {
                    fb.add((self.compose_row * row_bytes) as usize)
                };
                sync.dma2d_active();
                dma2d.start_blit_raw(
                    src_top as *const u8,
                    STAR_STRIDE,
                    dst_top,
                    row_bytes,
                    fb_w,
                    top,
                    PixelFmt::Argb8888,
                );
                while dma2d.is_in_flight() {
                    cortex_m::asm::nop();
                }
                sync.dma2d_idle();
                let dst_bot = unsafe {
                    fb.add(
                        ((self.compose_row + top) * row_bytes) as usize,
                    )
                };
                sync.dma2d_active();
                dma2d.start_blit_raw(
                    self.starfield as *const u8,
                    STAR_STRIDE,
                    dst_bot,
                    row_bytes,
                    fb_w,
                    bot,
                    PixelFmt::Argb8888,
                );
                while dma2d.is_in_flight() {
                    cortex_m::asm::nop();
                }
                sync.dma2d_idle();
            }
            cortex_m::asm::dsb();
            self.compose_row += batch;
        }

        // ── A8 text blend stripes ───────────────────────────────
        let a8_h = A8_HEIGHT;
        const BLEND_ROWS: u32 = 50;
        while self.compose_blend_y < a8_h {
            let elapsed = cyccnt().wrapping_sub(porch_start);
            if elapsed >= porch_cyc {
                return false; // porch expired
            }
            let h = BLEND_ROWS.min(a8_h - self.compose_blend_y);
            let src_off = (self.compose_blend_y * A8_WIDTH) as usize;
            let src = (A8_BUF + src_off) as *const u8;
            let dst_off = ((A8_Y_BASE + self.compose_blend_y) * fb_w * BPP)
                as usize;
            let dst = unsafe { fb.add(dst_off) };
            sync.dma2d_active();
            dma2d.start_blend_a8_color(
                src,
                A8_WIDTH,
                h,
                YELLOW,
                dst,
                fb_w * BPP,
            );
            while dma2d.is_in_flight() {
                cortex_m::asm::nop();
            }
            sync.dma2d_idle();
            cortex_m::asm::dsb();
            self.compose_blend_y += h;
        }

        // All done — reset for next frame
        let _ = sync.take_complete();
        self.compose_active = false;
        self.diag_completed_frames =
            self.diag_completed_frames.saturating_add(1);
        true
    }

    /// Blocking compose + blend for pre-fill (no ERIF cycle running).
    /// Reads directly from starfield, row-by-row, no admission gating.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn compose_and_blend_blocking(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        back_buf: *mut u8,
        fb_w: u32,
    ) {
        if self.starfield.is_null() {
            return;
        }
        let row_bytes = fb_w * BPP;
        let star_row = ((self.star_scroll_q8 >> 8) as u32) % STAR_ROWS;
        for row in 0..FB_H {
            let src_row = (star_row + row) % STAR_ROWS;
            let src = unsafe {
                self.starfield.add((src_row * STAR_STRIDE) as usize)
                    as *const u8
            };
            let dst = unsafe { back_buf.add((row * row_bytes) as usize) };
            dma2d.start_blit_raw(
                src,
                STAR_STRIDE,
                dst,
                row_bytes,
                fb_w,
                1,
                PixelFmt::Argb8888,
            );
            while dma2d.is_in_flight() {
                cortex_m::asm::nop();
            }
        }
        dcache_clean_range(A8_BUF, A8_SIZE);
        let a8_h = A8_HEIGHT;
        const STRIPE_H: u32 = 16;
        let mut y = 0u32;
        while y < a8_h {
            let h = STRIPE_H.min(a8_h - y);
            let src_off = (y * A8_WIDTH) as usize;
            let src = (A8_BUF + src_off) as *const u8;
            let dst_off = ((A8_Y_BASE + y) * fb_w * BPP) as usize;
            let dst = unsafe { back_buf.add(dst_off) };
            dma2d.start_blend_a8_color(
                src,
                A8_WIDTH,
                h,
                YELLOW,
                dst,
                fb_w * BPP,
            );
            while dma2d.is_in_flight() {
                cortex_m::asm::nop();
            }
            y += h;
        }
    }

    /// Prepare the next frame's A8 text buffer via CPU FIR.
    ///
    /// Call repeatedly until it returns `FrameReady`. Pure CPU work —
    /// no DMA2D, no AXI contention. Starfield compose reads directly
    /// from the pre-rendered double-height buffer, so no starfield
    /// shift or layer_buf update is needed.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn prep_next_frame(
        &mut self,
        _dma2d: &mut Dma2dBlitter,
        _sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        if !self.active {
            return StepResult::Idle;
        }

        if !self.prep_started {
            self.advance_scroll();
            self.frame_id = self.frame_id.wrapping_add(1);

            let scroll_px = self.scroll_q8 >> 8;
            if scroll_px >= (self.text_h + CRAWL_H) as i32 {
                self.deactivate();
                return StepResult::Finished;
            }

            self.prep_frame_scroll_px = scroll_px;
            unsafe {
                core::ptr::write_bytes(A8_BUF as *mut u8, 0, A8_SIZE);
            }
            self.prep_text_row = 0;
            self.prep_started = true;
        }

        // CPU FIR text — one row per call
        if self.prep_text_row < CRAWL_H {
            let text_row_i =
                self.prep_frame_scroll_px + self.prep_text_row as i32;
            if text_row_i >= 0 && (text_row_i as u32) < self.text_h {
                let src_row = text_row_i as u32;
                let target_w = TOP_W
                    + (BOT_W - TOP_W) * self.prep_text_row / (CRAWL_H - 1);
                let dst_x_off = (CRAWL_W - target_w) / 2;
                if self.fir_resample_text_row(src_row, target_w) {
                    let x_col = (FB_W - 1 - self.prep_text_row) as usize;
                    let y_off = (dst_x_off - A8_Y_BASE) as usize;
                    for i in 0..target_w as usize {
                        unsafe {
                            let a8_ptr = (A8_BUF
                                + (y_off + i) * A8_WIDTH as usize
                                + x_col)
                                as *mut u8;
                            a8_ptr.write_volatile(self.scanline_buf[i]);
                        }
                    }
                }
            }
            self.prep_text_row += 1;
        }

        if self.prep_text_row >= CRAWL_H {
            dcache_clean_range(A8_BUF, A8_SIZE);
            self.prep_started = false;
            return StepResult::FrameReady;
        }

        StepResult::Pending
    }

    /// One-shot: fill `layer_buf` from the starfield source buffer.
    /// Blocking — called during pre-fill before the render loop starts.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn render_stars_to_layer(&mut self, dma2d: &mut Dma2dBlitter) {
        if self.layer_buf.is_null() || self.starfield.is_null() {
            return;
        }
        let star_row = ((self.star_scroll_q8 >> 8) as u32) % STAR_ROWS;
        self.last_frame_star_row = star_row;
        let row_bytes = FB_W * BPP;

        // Row-by-row blit from starfield → layer_buf (wrapping)
        for r in 0..FB_H {
            let src_row = (star_row + r) % STAR_ROWS;
            let src = unsafe {
                self.starfield.add((src_row * STAR_STRIDE) as usize)
            };
            let dst = unsafe {
                self.layer_buf.add((r * row_bytes) as usize)
            };
            dma2d.start_blit_raw(
                src as *const u8,
                row_bytes,
                dst,
                row_bytes,
                FB_W,
                1,
                PixelFmt::Argb8888,
            );
            while dma2d.is_in_flight() {
                cortex_m::asm::nop();
            }
        }
        let layer_size = (FB_W * FB_H * BPP) as usize;
        dcache_clean_range(self.layer_buf as usize, layer_size);
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

        // Flush D-cache for the entire starfield so DMA2D (which bypasses
        // L1 D-cache) sees both the per-pixel CPU star writes above and
        // the row-mirror copies. On bare-metal the larger cache-vs-buffer
        // ratio usually guaranteed eventual eviction before DMA2D read;
        // under Zephyr the timing is tighter and stars remained in cache,
        // leaving the DMA2D row blits to copy "blue only" from SDRAM.
        let starfield_addr = self.starfield as usize;
        let starfield_size = (STAR_ROWS * STAR_STRIDE) as usize;
        dcache_clean_range(starfield_addr, starfield_size);
    }
}

#[cfg(not(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64"))))]
impl StarCrawl {
    pub fn activate(&mut self) {}

    pub fn tick(&mut self, _back_buf: *mut u8, _fb_w: u32, _fb_h: u32) -> StepResult {
        StepResult::Idle
    }
}

// ── DWT cycle counter ───────────────────────────────────────────────────────

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
fn cyccnt() -> u32 {
    const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;
    unsafe { DWT_CYCCNT.read_volatile() }
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

// ── Effect trait impl ───────────────────────────────────────────────────────
//
// Lightweight adapter so `StarCrawl` satisfies the crate-local `Effect`
// trait (see `effect.rs`). `tick` delegates straight through; state-only
// methods are thin wrappers. Keeps the public crawl API stable while
// letting future multi-effect code treat the crawl as one instance among
// many.

impl crate::effect::Effect for StarCrawl {
    fn is_active(&self) -> bool {
        StarCrawl::is_active(self)
    }

    fn advance(&mut self) {
        self.advance_scroll();
    }

    fn dirty_bounds(&self) -> Option<crate::effect::Rect> {
        if !self.active {
            return None;
        }
        Some(crate::effect::Rect {
            x: 0,
            y: 0,
            w: FB_W as u16,
            h: FB_H as u16,
        })
    }

    fn reset(&mut self) {
        self.deactivate();
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        target: *mut u8,
        target_w: u32,
        target_h: u32,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        StarCrawl::tick(self, dma2d, target, target_w, target_h, sync)
    }
}
