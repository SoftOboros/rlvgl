//! DMA2D-accelerated event viewer overlay.
//!
//! Replaces CPU-rendered text with a three-step DMA2D pipeline modeled on the
//! star crawl: CPU renders text to a D2 SRAM A8 buffer, then DMA2D fills the
//! background and blends the text in two hardware-timed transfers.  This
//! eliminates thousands of tiny CPU->SDRAM writes that cause AXI bus
//! contention with LTDC scanout.

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::blit::{PixelFmt, Rect as BlitRect};
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::dma2d::Dma2dBlitter;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::frame_sync::{Dma2dSync, FrameSync, ScopeProbe};
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::hwcore::addr::PhysAddr;
#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
use rlvgl_platform::hwcore::surface::{BackBuffer, FrameBuffer};

use rlvgl_core::bitmap_font::BitmapFont;
use rlvgl_ui::EventWindow;

// ── Portrait framebuffer constants ──────────────────────────────────────────
const FB_W: u32 = 480;
/// Portrait framebuffer height (NT35510 panel on the 747I-DISCO).
/// Used to synthesize a [`FrameBuffer`] handle around `self.back_buf`
/// when calling the typed DMA2D submission API.
const FB_H: u32 = 800;
const BPP: u32 = 4;

// ── Event window geometry (portrait FB coords) ─────────────────────────────
// Draw bounds: Rect { x: 210, y: 108, width: 380, height: 264 }
// Portrait transform (RotatedRenderer): fb_x = 480 - 108 - 264 = 108
//   fb_y = 210, fb_w = 264 (draw height), fb_h = 380 (draw width)
const EV_FB_X: u32 = 108;
const EV_FB_Y: u32 = 210;
const EV_FB_W: u32 = 264; // portrait width  (from draw height)
const EV_FB_H: u32 = 380; // portrait height (from draw width)

// ── A8 buffer in D2 SRAM ───────────────────────────────────────────────────
// Shared address with star crawl — mutually exclusive (crawl replaces
// normal rendering while active).
const A8_BUF: usize = 0x3000_0000;
const A8_WIDTH: u32 = EV_FB_W; // 264
const A8_HEIGHT: u32 = EV_FB_H; // 380
const A8_SIZE: usize = (A8_WIDTH * A8_HEIGHT) as usize; // 100,320 bytes

// ── Colors ──────────────────────────────────────────────────────────────────
const BG_COLOR: u32 = 0xFF19_1919; // Color(25, 25, 25, 255)
const BORDER_COLOR: u32 = 0xFF50_5050; // Color(80, 80, 80, 255)
const TEXT_FG: u32 = 0x00DC_DCDC; // Color(220, 220, 220) for A8 blend
const BORDER_W: u32 = 2;

// ── State machine ───────────────────────────────────────────────────────────

#[derive(Copy, Clone, Eq, PartialEq)]
enum Stage {
    Idle,
    FillBorder,
    WaitBorder,
    FillInterior,
    WaitInterior,
    BlendText,
    WaitBlend,
}

/// Result of a single `tick()` call.
pub enum StepResult {
    /// Overlay is inactive.
    Idle,
    /// DMA2D work in progress — call again next loop iteration.
    Pending,
    /// Frame complete, back buffer ready to present.
    FrameReady,
}

/// DMA2D-accelerated event viewer overlay.
pub struct EventOverlay {
    stage: Stage,
    back_buf: *mut u8,
    fb_w: u32,
}

impl EventOverlay {
    pub fn new() -> Self {
        Self {
            stage: Stage::Idle,
            back_buf: core::ptr::null_mut(),
            fb_w: FB_W,
        }
    }

    /// Returns true if the DMA2D pipeline is currently active.
    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    /// Begin a new frame: CPU-render text to the D2 SRAM A8 buffer,
    /// then transition to the DMA2D fill/blend stages.
    ///
    /// Call once per frame when the event window is visible.
    /// `back_buf` is the SDRAM back-buffer address.
    pub fn begin_frame(&mut self, event_win: &EventWindow, back_buf: *mut u8, fb_w: u32) {
        self.back_buf = back_buf;
        self.fb_w = fb_w;

        // Zero the A8 buffer in D2 SRAM.
        unsafe {
            core::ptr::write_bytes(A8_BUF as *mut u8, 0, A8_SIZE);
        }

        // CPU-render text entries to the A8 buffer (portrait orientation).
        // This writes to D2 SRAM, not SDRAM, so no AXI contention with LTDC.
        let font = event_win.font();
        let padding = event_win.padding();
        let line_h = event_win.line_height();

        event_win.for_each_visible(|i, text| {
            let draw_x = padding;
            let draw_y = padding + i as i32 * line_h;
            render_str_a8(font, draw_x, draw_y, text);
        });

        // D2 SRAM at 0x3000_0000 falls under the default MPU background map
        // (Write-Back Write-Allocate). CPU writes sit in D-cache; DMA2D
        // reads directly from physical SRAM. Clean the cache lines so
        // DMA2D sees the A8 text data we just wrote.
        dcache_clean_range(A8_BUF, A8_SIZE);

        self.stage = Stage::FillBorder;
    }

    /// Advance the DMA2D pipeline by one step.
    ///
    /// Returns `Pending` while DMA2D transfers are in flight, `FrameReady`
    /// when the overlay has been fully composited onto the back buffer.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn tick(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        // Check for DMA2D errors.
        let dma_error = dma2d.read_error();
        if dma_error != 0 {
            // Drop frame and return to idle on error.
            self.stage = Stage::Idle;
            return StepResult::Idle;
        }

        match self.stage {
            Stage::Idle => StepResult::Idle,

            Stage::FillBorder => {
                // Border fill: ~100K cycles. Check budget.
                if !sync.dma2d_admits(100_000) {
                    return StepResult::Pending;
                }
                sync.note_start();
                sync.dma2d_active();
                // SAFETY: `self.back_buf` was supplied by `begin_frame`
                // and points at the SDRAM back buffer the LTDC is not
                // currently scanning; `self` exclusively holds it for
                // the duration of this DMA2D submission. The synthesized
                // `BackBuffer<'_>` lives only across the typed call —
                // its borrow is released on `into_borrow`, restoring
                // the caller's view of `self.back_buf`.
                let mut fb = unsafe {
                    FrameBuffer::from_phys(
                        PhysAddr::new(self.back_buf as u32),
                        self.fb_w,
                        FB_H,
                        self.fb_w * BPP,
                        PixelFmt::Argb8888,
                    )
                };
                let mut back = BackBuffer::wrap(&mut fb);
                let inflight = dma2d.start_fill_typed(
                    back.dma_dst(),
                    BlitRect {
                        x: EV_FB_X as i32,
                        y: EV_FB_Y as i32,
                        w: EV_FB_W,
                        h: EV_FB_H,
                    },
                    BORDER_COLOR,
                );
                let _ = inflight.into_borrow();
                self.stage = Stage::WaitBorder;
                StepResult::Pending
            }

            Stage::WaitBorder => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                if !sync.take_complete() {
                    return StepResult::Pending;
                }
                sync.dma2d_idle();
                self.stage = Stage::FillInterior;
                // Fall through to start interior fill immediately.
                self.tick_fill_interior(dma2d, sync)
            }

            Stage::FillInterior => self.tick_fill_interior(dma2d, sync),

            Stage::WaitInterior => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                if !sync.take_complete() {
                    return StepResult::Pending;
                }
                sync.dma2d_idle();
                self.stage = Stage::BlendText;
                // Fall through to start text blend immediately.
                self.tick_blend_text(dma2d, sync)
            }

            Stage::BlendText => self.tick_blend_text(dma2d, sync),

            Stage::WaitBlend => {
                if dma2d.is_in_flight() {
                    return StepResult::Pending;
                }
                if !sync.take_complete() {
                    return StepResult::Pending;
                }
                sync.dma2d_idle();
                self.stage = Stage::Idle;
                StepResult::FrameReady
            }
        }
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn tick_fill_interior(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        if !sync.dma2d_admits(100_000) {
            return StepResult::Pending;
        }
        sync.note_start();
        sync.dma2d_active();
        // SAFETY: see `Stage::FillBorder` arm above.
        let mut fb = unsafe {
            FrameBuffer::from_phys(
                PhysAddr::new(self.back_buf as u32),
                self.fb_w,
                FB_H,
                self.fb_w * BPP,
                PixelFmt::Argb8888,
            )
        };
        let mut back = BackBuffer::wrap(&mut fb);
        let inflight = dma2d.start_fill_typed(
            back.dma_dst(),
            BlitRect {
                x: (EV_FB_X + BORDER_W) as i32,
                y: (EV_FB_Y + BORDER_W) as i32,
                w: EV_FB_W - 2 * BORDER_W,
                h: EV_FB_H - 2 * BORDER_W,
            },
            BG_COLOR,
        );
        let _ = inflight.into_borrow();
        self.stage = Stage::WaitInterior;
        StepResult::Pending
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn tick_blend_text(
        &mut self,
        dma2d: &mut Dma2dBlitter,
        sync: &(impl FrameSync + Dma2dSync + ScopeProbe),
    ) -> StepResult {
        if !sync.dma2d_admits(400_000) {
            return StepResult::Pending;
        }
        let dst = unsafe {
            self.back_buf
                .add((EV_FB_Y * self.fb_w * BPP + EV_FB_X * BPP) as usize)
        };
        sync.note_start();
        sync.dma2d_active();
        dma2d.start_blend_a8_color(
            A8_BUF as *const u8,
            A8_WIDTH,
            A8_HEIGHT,
            TEXT_FG,
            dst,
            self.fb_w * BPP,
        );
        self.stage = Stage::WaitBlend;
        StepResult::Pending
    }
}

// ── A8 text rendering (CPU → D2 SRAM) ──────────────────────────────────────

/// Render a string into the portrait A8 buffer at landscape draw coordinates
/// relative to the event window origin.
///
/// Each glyph pixel is written as a `scale × scale` block of alpha = 0xFF.
/// The RotatedRenderer coordinate transform is applied:
///   landscape (x, y, w, h) → portrait (EV_FB_W - y - h, x, h, w)
fn render_str_a8(font: &BitmapFont, draw_x: i32, draw_y: i32, text: &str) {
    let scale = font.scale as i32;
    let advance = font.scaled_width() + scale; // glyph width + gap
    let gw = font.glyph_width as usize;
    let gh = font.glyph_height as usize;
    let bits_per_glyph = gw * gh;

    let mut cx = draw_x;
    for ch in text.chars() {
        let idx = if (0x20..=0x7E).contains(&(ch as u32)) {
            (ch as u32 - 0x20) as usize
        } else {
            cx += advance;
            continue;
        };
        let bit_offset = idx * bits_per_glyph;

        for row in 0..gh {
            for col in 0..gw {
                let bit = bit_offset + row * gw + col;
                let byte_idx = bit / 8;
                let bit_idx = 7 - (bit % 8);
                if byte_idx < font.data.len() && (font.data[byte_idx] >> bit_idx) & 1 != 0 {
                    // Landscape pixel at (cx + col*scale, draw_y + row*scale)
                    // with size (scale, scale).
                    let lx = cx + col as i32 * scale;
                    let ly = draw_y + row as i32 * scale;

                    // RotatedRenderer transform → portrait A8 coords.
                    let a8_x = EV_FB_W as i32 - ly - scale;
                    let a8_y = lx;

                    // Write scale×scale block of alpha=0xFF.
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = (a8_x + dx) as u32;
                            let py = (a8_y + dy) as u32;
                            if px < A8_WIDTH && py < A8_HEIGHT {
                                unsafe {
                                    let ptr =
                                        (A8_BUF + py as usize * A8_WIDTH as usize + px as usize)
                                            as *mut u8;
                                    ptr.write_volatile(0xFF);
                                }
                            }
                        }
                    }
                }
            }
        }
        cx += advance;
    }
}

// ── D-cache maintenance ─────────────────────────────────────────────────────

/// Clean D-cache lines covering `[addr, addr+size)` so DMA2D sees CPU writes.
///
/// D2 SRAM at 0x3000_0000 falls under the default Cortex-M7 background map
/// (Write-Back Write-Allocate). Without an explicit cache clean, CPU writes
/// stay in the D-cache and DMA2D reads stale data from physical SRAM.
///
/// Uses DCCMVAC (Data Cache Clean by MVA to Point of Coherency) at
/// 0xE000_EF68, one write per 32-byte cache line.
fn dcache_clean_range(addr: usize, size: usize) {
    const DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
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
