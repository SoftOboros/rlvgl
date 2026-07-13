// SPDX-License-Identifier: MIT
//! System information overlay panels — zero heap allocation.
//!
//! Two panels share the same visual style and formatting infrastructure:
//!
//! - **ChipInfoPanel**: static hardware info (MCU, Flash, SYSCLK) — read
//!   once on open, never refreshes.
//! - **LiveStatsPanel**: dynamic telemetry (FPS, Heap%, Ticks) — refreshes
//!   at ~2 Hz.

use rlvgl_core::event::Event;
use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_ui::draw_helpers::{draw_border, fill_rounded_rect};

// ── Shared constants ──────────��─────────────────────────────────────────

const PANEL_W: i32 = 340;
const PANEL_PADDING: i32 = 16;
const PANEL_RADIUS: u8 = 10;
const CLEAR_FRAMES: u8 = 3;
const MAX_LINES: usize = 6;
const VAL_BUF_LEN: usize = 32;

const BG_COLOR: Color = Color(30, 30, 30, 255);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TITLE_COLOR: Color = Color(80, 180, 255, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const DIM_COLOR: Color = Color(140, 140, 140, 255);

// ── InfoLine — shared stack-allocated line ──────────────────────────────

struct InfoLine {
    label: &'static [u8],
    buf: [u8; VAL_BUF_LEN],
    len: usize,
}

impl InfoLine {
    const EMPTY: Self = Self {
        label: b"",
        buf: [0; VAL_BUF_LEN],
        len: 0,
    };

    fn value(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

// ── Shared drawing helpers ──────────────────────────────────────────────

fn panel_height(font: &PackedFont, line_count: usize) -> i32 {
    let line_h = font.height as i32 + 6;
    PANEL_PADDING + line_h + PANEL_PADDING + line_count as i32 * line_h + PANEL_PADDING
}

fn draw_panel_common(
    renderer: &mut dyn Renderer,
    font: &PackedFont,
    bounds: Rect,
    title: &str,
    lines: &[InfoLine],
    line_count: usize,
) {
    fill_rounded_rect(renderer, bounds, BG_COLOR, PANEL_RADIUS);
    draw_border(renderer, bounds, BORDER_COLOR, 2);

    let line_h = font.height as i32 + 6;
    let mut y = bounds.y + PANEL_PADDING;
    let x = bounds.x + PANEL_PADDING;

    font.draw_str(renderer, x, y, title, TITLE_COLOR);
    let close_x = bounds.x + bounds.width - PANEL_PADDING - font.measure("X");
    font.draw_str(renderer, close_x, y, "X", Color(255, 80, 80, 255));
    y += line_h + PANEL_PADDING;

    for i in 0..line_count {
        let line = &lines[i];
        let label_str = unsafe { core::str::from_utf8_unchecked(line.label) };
        font.draw_str(renderer, x, y, label_str, DIM_COLOR);
        let val = line.value();
        let val_w = font.measure(val);
        let val_x = bounds.x + bounds.width - PANEL_PADDING - val_w;
        font.draw_str(renderer, val_x, y, val, TEXT_COLOR);
        y += line_h;
    }
}

#[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
fn draw_panel_hw(
    ctx: &mut rlvgl_platform::dma2d_draw::Dma2dOverlayCtx,
    scratch: &mut [u8],
    font: &PackedFont,
    bounds: Rect,
    title: &str,
    lines: &[InfoLine],
    line_count: usize,
) {
    ctx.fill_rounded_rect_hw(bounds, BG_COLOR, PANEL_RADIUS);
    ctx.draw_rounded_border_hw(bounds, BORDER_COLOR, 2, PANEL_RADIUS);

    let line_h = font.height as i32 + 6;
    let mut y = bounds.y + PANEL_PADDING;
    let x = bounds.x + PANEL_PADDING;

    ctx.draw_str_rotated(font, x, y, title, TITLE_COLOR, scratch);
    let close_x = bounds.x + bounds.width - PANEL_PADDING - font.measure("X");
    ctx.draw_str_rotated(font, close_x, y, "X", Color(255, 80, 80, 255), scratch);
    y += line_h + PANEL_PADDING;

    for i in 0..line_count {
        let line = &lines[i];
        let label_str = unsafe { core::str::from_utf8_unchecked(line.label) };
        ctx.draw_str_rotated(font, x, y, label_str, DIM_COLOR, scratch);
        let val = line.value();
        let val_w = font.measure(val);
        let val_x = bounds.x + bounds.width - PANEL_PADDING - val_w;
        ctx.draw_str_rotated(font, val_x, y, val, TEXT_COLOR, scratch);
        y += line_h;
    }
}

// ── Shared push helper ──────────────────────────────────────────────────

fn push_line(
    lines: &mut [InfoLine; MAX_LINES],
    count: &mut usize,
    label: &'static [u8],
    val: &[u8],
) {
    if *count < MAX_LINES {
        let line = &mut lines[*count];
        line.label = label;
        let n = val.len().min(VAL_BUF_LEN);
        line.buf[..n].copy_from_slice(&val[..n]);
        line.len = n;
        *count += 1;
    }
}

// ══════���═══════════════════════════════��═════════════════════════════════
// ChipInfoPanel — static hardware info (MCU, Flash, SYSCLK)
// ════════��═══════════════════════════════════════════════════════════════

pub struct ChipInfoPanel {
    visible: bool,
    bounds: Rect,
    lines: [InfoLine; MAX_LINES],
    line_count: usize,
    font: &'static PackedFont,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
    needs_collect: bool,
    frame_count: u32,
    fps_last_tick: u32,
    fps: u32,
}

impl ChipInfoPanel {
    pub fn new(font: &'static PackedFont) -> Self {
        let h = panel_height(font, 4); // MCU + Flash + SYSCLK + FPS
        Self {
            visible: false,
            bounds: Rect {
                x: (800 - PANEL_W) / 2,
                y: (480 - h) / 2,
                width: PANEL_W,
                height: h,
            },
            lines: [const { InfoLine::EMPTY }; MAX_LINES],
            line_count: 0,
            font,
            clear_countdown: 0,
            clear_bounds: None,
            needs_collect: false,
            frame_count: 0,
            fps_last_tick: 0,
            fps: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Call from the main loop (outside dispatch) to collect deferred info
    /// and update FPS. Returns `true` if content changed (needs a render).
    pub fn poll(&mut self, tick: u32) -> bool {
        if !self.visible {
            return false;
        }

        // FPS tracking
        self.frame_count += 1;
        let elapsed = tick.wrapping_sub(self.fps_last_tick);
        if elapsed >= 30 {
            self.fps = if elapsed > 0 {
                self.frame_count * 30 / elapsed
            } else {
                0
            };
            self.frame_count = 0;
            self.fps_last_tick = tick;
        }

        // Deferred initial collection
        if self.needs_collect {
            self.needs_collect = false;
            self.collect_info();
            self.resize_to_content();
            return true;
        }

        // Rebuild with updated FPS every ~2 Hz
        if elapsed >= 30 {
            self.collect_info();
            return true;
        }

        false
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.visible = true;
            self.needs_collect = true; // defer to next tick
        }
    }

    fn resize_to_content(&mut self) {
        let h = panel_height(self.font, self.line_count);
        self.bounds.height = h;
        self.bounds.y = (480 - h) / 2;
    }

    fn collect_info(&mut self) {
        self.line_count = 0;

        // Keep this panel display-only and side-effect-free. Earlier versions
        // sampled RCC/System Memory directly here; one bad RCC address hard
        // faulted when the Info wing opened.
        push_line(
            &mut self.lines,
            &mut self.line_count,
            b"MCU",
            b"STM32H747XIH6",
        );
        push_line(&mut self.lines, &mut self.line_count, b"Flash", b"2048 KB");
        push_line(&mut self.lines, &mut self.line_count, b"SYSCLK", b"480 MHz");

        // FPS (updated live)
        let mut buf = [0u8; VAL_BUF_LEN];
        let n = fmt_dec(self.fps, &mut buf);
        push_line(&mut self.lines, &mut self.line_count, b"FPS", &buf[..n]);
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn draw_hw(
        &self,
        ctx: &mut rlvgl_platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
    ) {
        if !self.visible {
            return;
        }
        draw_panel_hw(
            ctx,
            scratch,
            self.font,
            self.bounds,
            "System",
            &self.lines,
            self.line_count,
        );
    }
}

impl Widget for ChipInfoPanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        draw_panel_common(
            renderer,
            self.font,
            self.bounds,
            "System",
            &self.lines,
            self.line_count,
        );
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        if let Event::PressRelease { .. } = event {
            self.hide();
            return true;
        }
        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            self.clear_bounds
        } else {
            None
        }
    }
}

// ══════��═════════════════════════════════════════════════════════════════
// LiveStatsPanel — dynamic telemetry (FPS, Heap%, Ticks)
// ══���══════════════════��════════════════════════════════���═════════════════

/// Optional CPU utilisation snapshot passed to [`LiveStatsPanel::refresh`].
pub struct CpuSnapshot {
    pub cm7_pct: u32,
    pub cm4_pct: u32,
}

pub struct LiveStatsPanel {
    visible: bool,
    bounds: Rect,
    lines: [InfoLine; MAX_LINES],
    line_count: usize,
    font: &'static PackedFont,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
    refresh_divider: u8,
}

impl LiveStatsPanel {
    pub fn new(font: &'static PackedFont) -> Self {
        let h = panel_height(font, 3); // FPS + Heap + Ticks
        Self {
            visible: false,
            bounds: Rect {
                x: (800 - PANEL_W) / 2,
                y: (480 - h) / 2,
                width: PANEL_W,
                height: h,
            },
            lines: [const { InfoLine::EMPTY }; MAX_LINES],
            line_count: 0,
            font,
            clear_countdown: 0,
            clear_bounds: None,
            refresh_divider: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.visible = true;
            self.refresh_divider = 14; // force refresh on next tick
        }
    }

    /// Call every frame tick. Returns `true` if content changed.
    /// `temp_x10` is the cached junction temperature in tenths of °C.
    pub fn refresh(
        &mut self,
        tick: u32,
        heap_used: usize,
        heap_total: usize,
        temp_x10: i32,
        cpu: Option<&CpuSnapshot>,
    ) -> bool {
        if !self.visible {
            return false;
        }
        let _ = tick; // tick used only for throttle cadence

        self.refresh_divider = self.refresh_divider.wrapping_add(1);
        if self.refresh_divider % 15 != 0 {
            return false;
        }

        self.collect_info(heap_used, heap_total, temp_x10, cpu);
        self.resize_to_content();
        true
    }

    fn resize_to_content(&mut self) {
        let h = panel_height(self.font, self.line_count);
        self.bounds.height = h;
        self.bounds.y = (480 - h) / 2;
    }

    fn collect_info(
        &mut self,
        heap_used: usize,
        heap_total: usize,
        temp_x10: i32,
        cpu: Option<&CpuSnapshot>,
    ) {
        self.line_count = 0;

        // CPU utilisation (when cpu_stats feature is active)
        if let Some(cpu) = cpu {
            let mut buf = [0u8; VAL_BUF_LEN];
            let n = fmt_dec_into(cpu.cm7_pct, &mut buf);
            buf[n] = b'%';
            push_line(
                &mut self.lines,
                &mut self.line_count,
                b"CM7 CPU",
                &buf[..n + 1],
            );

            let mut buf = [0u8; VAL_BUF_LEN];
            let n = fmt_dec_into(cpu.cm4_pct, &mut buf);
            buf[n] = b'%';
            push_line(
                &mut self.lines,
                &mut self.line_count,
                b"CM4 CPU",
                &buf[..n + 1],
            );
        }

        // Junction temperature
        if temp_x10 != 0 {
            let whole = (temp_x10 / 10) as u32;
            let frac = ((temp_x10 % 10).unsigned_abs()) as u32;
            let mut buf = [0u8; VAL_BUF_LEN];
            let mut pos = 0;
            pos += fmt_dec_into(whole, &mut buf[pos..]);
            buf[pos] = b'.';
            pos += 1;
            pos += fmt_dec_into(frac, &mut buf[pos..]);
            buf[pos] = b' ';
            pos += 1;
            buf[pos] = b'C';
            pos += 1;
            push_line(&mut self.lines, &mut self.line_count, b"Temp", &buf[..pos]);
        }

        // Heap
        if heap_total > 0 {
            let pct = (heap_used * 100 / heap_total) as u32;
            let mut buf = [0u8; VAL_BUF_LEN];
            let mut pos = 0;
            pos += fmt_dec_into(heap_used as u32 / 1024, &mut buf[pos..]);
            buf[pos] = b'K';
            pos += 1;
            buf[pos] = b'/';
            pos += 1;
            pos += fmt_dec_into(heap_total as u32 / 1024, &mut buf[pos..]);
            buf[pos] = b'K';
            pos += 1;
            buf[pos] = b' ';
            pos += 1;
            buf[pos] = b'(';
            pos += 1;
            pos += fmt_dec_into(pct, &mut buf[pos..]);
            buf[pos] = b'%';
            pos += 1;
            buf[pos] = b')';
            pos += 1;
            push_line(&mut self.lines, &mut self.line_count, b"Heap", &buf[..pos]);
        }

        // Ticks
        // D3 SRAM event-count telemetry slot.
        let ticks = unsafe { (0x3800_0660u32 as *const u32).read_volatile() }; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        let mut buf = [0u8; VAL_BUF_LEN];
        let n = fmt_dec(ticks, &mut buf);
        push_line(&mut self.lines, &mut self.line_count, b"Ticks", &buf[..n]);
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn draw_hw(
        &self,
        ctx: &mut rlvgl_platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
    ) {
        if !self.visible {
            return;
        }
        draw_panel_hw(
            ctx,
            scratch,
            self.font,
            self.bounds,
            "Live Stats",
            &self.lines,
            self.line_count,
        );
    }
}

impl Widget for LiveStatsPanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        draw_panel_common(
            renderer,
            self.font,
            self.bounds,
            "Live Stats",
            &self.lines,
            self.line_count,
        );
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        if let Event::PressRelease { .. } = event {
            self.hide();
            return true;
        }
        false
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            self.clear_bounds
        } else {
            None
        }
    }
}

// ── Stack-only formatting (no heap) ─────────────���───────────────────────

fn fmt_dec(v: u32, buf: &mut [u8]) -> usize {
    fmt_dec_into(v, buf)
}

fn fmt_dec_into(mut v: u32, buf: &mut [u8]) -> usize {
    if v == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
        }
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    while v > 0 {
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    let n = i.min(buf.len());
    for j in 0..n {
        buf[j] = tmp[i - 1 - j];
    }
    n
}
