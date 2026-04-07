// SPDX-License-Identifier: MIT
//! Settings gear icon with language config menu overlay.
//!
//! Draws a gear icon button that toggles a config panel when tapped.
//! The panel contains checkboxes for language selection with mutually
//! exclusive behavior. OK applies the change, Cancel/X reverts it.

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl::core::event::Event;
use rlvgl::core::packed_font::PackedFont;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};
use rlvgl::ui::GridCalc;
use rlvgl::ui::draw_helpers::{draw_border, draw_rounded_border, fill_rounded_rect};

// ── D3 SRAM debug slots (readable via probe-rs) ──────────────────
// ConfigMenu writes to 0x3800_0640..0x3800_066F for live debugging.
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
mod d3 {
    // Touch coordinates captured while panel is open
    pub const TOUCH_X: *mut u32 = 0x3800_0640 as *mut u32;
    pub const TOUCH_Y: *mut u32 = 0x3800_0644 as *mut u32;
    // Action code identifying which UI element was hit
    pub const HIT_CODE: *mut u32 = 0x3800_0648 as *mut u32;
    // Clear-countdown state on menu close
    pub const CLEAR_STATE: *mut u32 = 0x3800_0658 as *mut u32;
    // Menu visibility toggle event
    pub const VISIBILITY: *mut u32 = 0x3800_065C as *mut u32;
    // PressRelease event counter + last coords + visible state
    pub const EVENT_COUNT: *mut u32 = 0x3800_0660 as *mut u32;
    pub const LAST_X: *mut u32 = 0x3800_0664 as *mut u32;
    pub const LAST_Y: *mut u32 = 0x3800_0668 as *mut u32;
    pub const MENU_VIS: *mut u32 = 0x3800_066C as *mut u32;

    // Action-code sentinels written to HIT_CODE
    pub const ACT_CLOSE: u32 = 0xC105_E000;
    pub const ACT_OK: u32 = 0x0000_0111;
    pub const ACT_CANCEL: u32 = 0x0CAA_CE10;
    pub const ACT_ENGLISH: u32 = 0xE000_0000;
    pub const ACT_FRENCH: u32 = 0xF000_0001;
    pub const ACT_OUTSIDE: u32 = 0x0000_DEAD;
    pub const ACT_MISS: u32 = 0xFFFF_FFFF;
}

/// A self-contained settings menu widget.
pub struct ConfigMenu {
    /// Hit area and draw position for the gear icon.
    gear_bounds: Rect,
    /// Whether the config panel is currently visible.
    visible: bool,
    /// Number of frames to clear stale pixels after closing.
    clear_countdown: u8,
    /// Panel bounds to clear (saved when menu closes).
    clear_bounds: Option<Rect>,
    /// Currently applied locale index.
    applied: u8,
    /// Pending selection (may differ from applied while menu is open).
    pending: u8,
    /// Number of frames to keep redrawing after visibility change.
    dirty_frames: u8,
    /// Callback invoked when the selected locale is applied (OK).
    on_change: Option<Box<dyn FnMut(u8)>>,
    /// Callback invoked when the pending locale changes (live preview).
    on_preview: Option<Box<dyn FnMut(u8)>>,
    /// Whether the event viewer is enabled (applied state).
    event_viewer_applied: bool,
    /// Pending event viewer state (may differ while menu is open).
    event_viewer_pending: bool,
    /// Callback invoked when the event viewer setting is applied (OK).
    on_event_viewer_change: Option<Box<dyn FnMut(bool)>>,
    /// Last received touch coords (for debug display).
    last_touch: Option<(i32, i32)>,
    /// Pre-decoded gear icon pixels (RGBA → Color), or empty if not provided.
    gear_pixels: Vec<Color>,
    /// Gear icon pixel dimensions (width, height).
    gear_icon_size: (u32, u32),
    /// Pre-decoded close (X) icon pixels.
    close_pixels: Vec<Color>,
    /// Close icon pixel dimensions.
    close_icon_size: (u32, u32),
    /// Bitmap font for text rendering.
    font: &'static PackedFont,
}

// Layout constants for the config panel.
const PANEL_W: i32 = 340;
const PANEL_H: i32 = 340;
const PANEL_RADIUS: u8 = 10;
const PANEL_PADDING: i32 = 16;
const CHECK_SIZE: i32 = 24;
const ROW_HEIGHT: i32 = 40;
const BTN_W: i32 = 120;
const BTN_H: i32 = 44;
const CLOSE_SIZE: i32 = 40;
const TITLE_HEIGHT: i32 = 32;

// Colors
const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const CHECK_COLOR: Color = Color(80, 180, 255, 255);
const GEAR_COLOR: Color = Color(200, 200, 200, 255);
const BTN_BG: Color = Color(60, 60, 60, 255);
const BTN_OK_BG: Color = Color(40, 100, 180, 255);
const CLOSE_COLOR: Color = Color(180, 80, 80, 255);

impl ConfigMenu {
    /// Create a new config menu with the gear icon at the given bounds.
    pub fn new(gear_bounds: Rect, initial_locale: u8, font: &'static PackedFont) -> Self {
        Self {
            gear_bounds,
            visible: false,
            clear_countdown: 0,
            clear_bounds: None,
            applied: initial_locale,
            pending: initial_locale,
            dirty_frames: 0,
            on_change: None,
            on_preview: None,
            event_viewer_applied: true,
            event_viewer_pending: true,
            on_event_viewer_change: None,
            last_touch: None,
            gear_pixels: Vec::new(),
            gear_icon_size: (0, 0),
            close_pixels: Vec::new(),
            close_icon_size: (0, 0),
            font,
        }
    }

    /// Whether the config panel is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggle the config panel visibility.
    pub fn toggle_visible(&mut self) {
        if self.visible {
            self.close_menu();
        } else {
            self.visible = true;
            self.pending = self.applied;
            self.event_viewer_pending = self.event_viewer_applied;
        }
    }

    /// Whether the menu is actively clearing stale pixels.
    pub fn clear_active(&self) -> bool {
        self.clear_countdown > 0
    }

    /// Return the full region that was covered when the menu was last open.
    /// Includes the panel bounds (not just the gear).
    pub fn last_panel_bounds(&self) -> Option<Rect> {
        self.clear_bounds
    }

    /// Set the gear icon from pre-decoded RGBA pixel data.
    ///
    /// `rgba` is a flat `[R, G, B, A, ...]` byte slice. Pixels with alpha == 0
    /// are treated as transparent and skipped during drawing.
    pub fn gear_icon(mut self, rgba: &[u8], width: u32, height: u32) -> Self {
        self.gear_pixels = rgba
            .chunks_exact(4)
            .map(|c| Color(c[0], c[1], c[2], c[3]))
            .collect();
        self.gear_icon_size = (width, height);
        self
    }

    /// Set the close (X) icon from pre-decoded RGBA pixel data.
    pub fn close_icon(mut self, rgba: &[u8], width: u32, height: u32) -> Self {
        self.close_pixels = rgba
            .chunks_exact(4)
            .map(|c| Color(c[0], c[1], c[2], c[3]))
            .collect();
        self.close_icon_size = (width, height);
        self
    }

    /// Attach a callback invoked when the selected locale is applied (OK).
    pub fn on_change<F: FnMut(u8) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Attach a callback invoked when the pending locale changes (live preview).
    pub fn on_preview<F: FnMut(u8) + 'static>(mut self, handler: F) -> Self {
        self.on_preview = Some(Box::new(handler));
        self
    }

    /// Set the initial event viewer enabled state (default: true).
    pub fn event_viewer_default(mut self, enabled: bool) -> Self {
        self.event_viewer_applied = enabled;
        self.event_viewer_pending = enabled;
        self
    }

    /// Attach a callback invoked when the event viewer setting is applied (OK).
    pub fn on_event_viewer_change<F: FnMut(bool) + 'static>(mut self, handler: F) -> Self {
        self.on_event_viewer_change = Some(Box::new(handler));
        self
    }

    /// The panel rect: 10px below the gear, right edge at gear's left edge.
    fn panel_bounds(&self) -> Rect {
        Rect {
            x: self.gear_bounds.x - PANEL_W,
            y: self.gear_bounds.y + self.gear_bounds.height + 10,
            width: PANEL_W,
            height: PANEL_H,
        }
    }

    /// Close button (X) in the upper-right of the panel.
    fn close_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - CLOSE_SIZE - 6,
            y: panel.y + 6,
            width: CLOSE_SIZE,
            height: CLOSE_SIZE,
        }
    }

    /// Grid calculator for the content area below the title bar.
    ///
    /// Single-column grid with rows of `ROW_HEIGHT`. Row indices:
    ///   0 = English, 1 = Français, 2 = (separator), 3 = Event Viewer.
    fn content_grid(&self) -> GridCalc {
        let panel = self.panel_bounds();
        GridCalc::new(
            panel.x + PANEL_PADDING,
            panel.y + TITLE_HEIGHT + PANEL_PADDING,
            1,
            PANEL_W - 2 * PANEL_PADDING,
            ROW_HEIGHT,
        )
    }

    /// OK button bounds.
    fn ok_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - PANEL_PADDING - BTN_W,
            y: panel.y + panel.height - PANEL_PADDING - BTN_H,
            width: BTN_W,
            height: BTN_H,
        }
    }

    /// Cancel button bounds.
    fn cancel_bounds(&self) -> Rect {
        let panel = self.panel_bounds();
        Rect {
            x: panel.x + panel.width - PANEL_PADDING - BTN_W - 10 - BTN_W,
            y: panel.y + panel.height - PANEL_PADDING - BTN_H,
            width: BTN_W,
            height: BTN_H,
        }
    }

    fn inside(rect: Rect, x: i32, y: i32) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    /// Close the menu and start the clear countdown to erase stale pixels.
    fn close_menu(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.panel_bounds());
            self.clear_countdown = 3;
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            unsafe {
                d3::CLEAR_STATE.write_volatile(0xD0_000000 | self.clear_countdown as u32);
            }
        }
        self.visible = false;
    }

    fn fire_on_change(&mut self) {
        if let Some(cb) = self.on_change.as_mut() {
            cb(self.applied);
        }
    }

    fn fire_on_preview(&mut self) {
        if let Some(cb) = self.on_preview.as_mut() {
            cb(self.pending);
        }
    }

    fn fire_on_event_viewer_change(&mut self) {
        if let Some(cb) = self.on_event_viewer_change.as_mut() {
            cb(self.event_viewer_applied);
        }
    }

    fn draw_checkbox(
        renderer: &mut dyn Renderer,
        font: &PackedFont,
        row: Rect,
        label: &str,
        checked: bool,
    ) {
        // Box outline
        let box_rect = Rect {
            x: row.x,
            y: row.y + (ROW_HEIGHT - CHECK_SIZE) / 2,
            width: CHECK_SIZE,
            height: CHECK_SIZE,
        };
        renderer.fill_rect(box_rect, BORDER_COLOR);

        if checked {
            let inner = Rect {
                x: box_rect.x + 4,
                y: box_rect.y + 4,
                width: box_rect.width - 8,
                height: box_rect.height - 8,
            };
            renderer.fill_rect(inner, CHECK_COLOR);
        } else {
            let inner = Rect {
                x: box_rect.x + 2,
                y: box_rect.y + 2,
                width: box_rect.width - 4,
                height: box_rect.height - 4,
            };
            renderer.fill_rect(inner, BG_COLOR);
        }

        // Label text
        let text_x = row.x + CHECK_SIZE + 12;
        let text_y = row.y + (ROW_HEIGHT - font.height as i32) / 2;
        font.draw_str(renderer, text_x, text_y, label, TEXT_COLOR);
    }

    fn draw_button(
        renderer: &mut dyn Renderer,
        font: &PackedFont,
        bounds: Rect,
        label: &str,
        bg: Color,
    ) {
        fill_rounded_rect(renderer, bounds, bg, 6);
        draw_rounded_border(renderer, bounds, BORDER_COLOR, 1, 6);
        let text_w = font.measure(label);
        let text_x = bounds.x + (bounds.width - text_w) / 2;
        let text_y = bounds.y + (bounds.height - font.height as i32) / 2;
        font.draw_str(renderer, text_x, text_y, label, TEXT_COLOR);
    }

    /// Draw a simple gear/cog icon using fill_rect calls.
    fn draw_gear(renderer: &mut dyn Renderer, bounds: Rect, color: Color) {
        let cx = bounds.x + bounds.width / 2;
        let cy = bounds.y + bounds.height / 2;
        let s = bounds.width.min(bounds.height);
        let r = s / 2;
        let ir = r * 2 / 5;
        let tw = r / 3;

        // Center hub
        renderer.fill_rect(
            Rect {
                x: cx - ir,
                y: cy - ir,
                width: ir * 2,
                height: ir * 2,
            },
            color,
        );
        // Horizontal bar (left-right teeth)
        renderer.fill_rect(
            Rect {
                x: cx - r,
                y: cy - tw,
                width: r * 2,
                height: tw * 2,
            },
            color,
        );
        // Vertical bar (top-bottom teeth)
        renderer.fill_rect(
            Rect {
                x: cx - tw,
                y: cy - r,
                width: tw * 2,
                height: r * 2,
            },
            color,
        );
        // Diagonal teeth
        let d = r * 5 / 8;
        let ts = tw + 1;
        for &(dx, dy) in &[(d, d), (d, -d), (-d, d), (-d, -d)] {
            renderer.fill_rect(
                Rect {
                    x: cx + dx - ts / 2,
                    y: cy + dy - ts / 2,
                    width: ts,
                    height: ts,
                },
                color,
            );
        }
        // Center hole
        let hole = ir * 2 / 3;
        renderer.fill_rect(
            Rect {
                x: cx - hole / 2,
                y: cy - hole / 2,
                width: hole,
                height: hole,
            },
            BG_COLOR,
        );
    }
}

impl ConfigMenu {
    /// DMA2D-accelerated draw, bypassing the Renderer pipeline.
    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    pub fn draw_hw(
        &self,
        ctx: &mut rlvgl::platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
    ) {
        if !self.visible {
            return;
        }

        let panel = self.panel_bounds();
        ctx.fill_rounded_rect_hw(panel, BG_COLOR, PANEL_RADIUS);
        ctx.draw_rounded_border_hw(panel, BORDER_COLOR, 1, PANEL_RADIUS);

        // Title
        ctx.draw_str_rotated(
            self.font,
            panel.x + PANEL_PADDING,
            panel.y + (TITLE_HEIGHT - self.font.height as i32) / 2,
            "Settings",
            TEXT_COLOR,
            scratch,
        );

        // Close X (text fallback)
        let cb = self.close_bounds();
        ctx.draw_str_rotated(self.font, cb.x + 8, cb.y + 4, "X", CLOSE_COLOR, scratch);

        // Language checkboxes
        self.draw_checkbox_hw(
            ctx,
            scratch,
            self.content_grid().cell(0, 0),
            "English",
            self.pending == 0,
        );
        self.draw_checkbox_hw(
            ctx,
            scratch,
            self.content_grid().cell(1, 0),
            "Francais",
            self.pending == 1,
        );

        // Separator
        let sep_y = self.content_grid().cell(2, 0).y + ROW_HEIGHT / 2;
        ctx.fill_rect_rotated(
            panel.x + PANEL_PADDING,
            sep_y,
            PANEL_W - 2 * PANEL_PADDING,
            1,
            BORDER_COLOR,
        );

        // Event Viewer toggle
        self.draw_checkbox_hw(
            ctx,
            scratch,
            self.content_grid().cell(3, 0),
            "Event Viewer",
            self.event_viewer_pending,
        );

        // OK / Cancel buttons
        self.draw_button_hw(ctx, scratch, self.ok_bounds(), "OK", BTN_OK_BG);
        self.draw_button_hw(ctx, scratch, self.cancel_bounds(), "Cancel", BTN_BG);
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn draw_checkbox_hw(
        &self,
        ctx: &mut rlvgl::platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
        row: Rect,
        label: &str,
        checked: bool,
    ) {
        let box_rect = Rect {
            x: row.x,
            y: row.y + (ROW_HEIGHT - CHECK_SIZE) / 2,
            width: CHECK_SIZE,
            height: CHECK_SIZE,
        };
        ctx.fill_rect_rotated(
            box_rect.x,
            box_rect.y,
            box_rect.width,
            box_rect.height,
            BORDER_COLOR,
        );
        if checked {
            let inner = Rect {
                x: box_rect.x + 4,
                y: box_rect.y + 4,
                width: box_rect.width - 8,
                height: box_rect.height - 8,
            };
            ctx.fill_rect_rotated(inner.x, inner.y, inner.width, inner.height, CHECK_COLOR);
        } else {
            let inner = Rect {
                x: box_rect.x + 2,
                y: box_rect.y + 2,
                width: box_rect.width - 4,
                height: box_rect.height - 4,
            };
            ctx.fill_rect_rotated(inner.x, inner.y, inner.width, inner.height, BG_COLOR);
        }
        let text_x = row.x + CHECK_SIZE + 12;
        let text_y = row.y + (ROW_HEIGHT - self.font.height as i32) / 2;
        ctx.draw_str_rotated(self.font, text_x, text_y, label, TEXT_COLOR, scratch);
    }

    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
    fn draw_button_hw(
        &self,
        ctx: &mut rlvgl::platform::dma2d_draw::Dma2dOverlayCtx,
        scratch: &mut [u8],
        bounds: Rect,
        label: &str,
        bg: Color,
    ) {
        ctx.fill_rounded_rect_hw(bounds, bg, 6);
        ctx.draw_rounded_border_hw(bounds, BORDER_COLOR, 1, 6);
        let tw = self.font.measure(label);
        let tx = bounds.x + (bounds.width - tw) / 2;
        let ty = bounds.y + (bounds.height - self.font.height as i32) / 2;
        ctx.draw_str_rotated(self.font, tx, ty, label, TEXT_COLOR, scratch);
    }
}

impl Widget for ConfigMenu {
    fn bounds(&self) -> Rect {
        if self.visible {
            let panel = self.panel_bounds();
            let min_x = self.gear_bounds.x.min(panel.x);
            let min_y = self.gear_bounds.y;
            let max_x = (self.gear_bounds.x + self.gear_bounds.width).max(panel.x + panel.width);
            let max_y = panel.y + panel.height;
            Rect {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        } else {
            self.gear_bounds
        }
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        // Gear icon is drawn by the IconStrip — config menu only draws the panel

        if !self.visible {
            return;
        }

        // Panel background
        let panel = self.panel_bounds();
        fill_rounded_rect(renderer, panel, BG_COLOR, PANEL_RADIUS);
        draw_rounded_border(renderer, panel, BORDER_COLOR, 1, PANEL_RADIUS);

        // Title
        self.font.draw_str(
            renderer,
            panel.x + PANEL_PADDING,
            panel.y + (TITLE_HEIGHT - self.font.height as i32) / 2,
            "Settings",
            TEXT_COLOR,
        );

        // Close icon (X) — pixel icon or fallback text
        let cb = self.close_bounds();
        if !self.close_pixels.is_empty() {
            let (cw, ch) = self.close_icon_size;
            let cx = cb.x + (cb.width - cw as i32) / 2;
            let cy = cb.y + (cb.height - ch as i32) / 2;
            renderer.draw_pixels((cx, cy), &self.close_pixels, cw, ch);
        } else {
            renderer.draw_text((cb.x + 8, cb.y + CLOSE_SIZE - 6), "X", CLOSE_COLOR);
        }

        // Language checkboxes
        Self::draw_checkbox(
            renderer,
            self.font,
            self.content_grid().cell(0, 0),
            "English",
            self.pending == 0,
        );
        Self::draw_checkbox(
            renderer,
            self.font,
            self.content_grid().cell(1, 0),
            "Francais",
            self.pending == 1,
        );

        // Separator line between language and options sections
        let sep_y = self.content_grid().cell(2, 0).y + ROW_HEIGHT / 2;
        renderer.fill_rect(
            Rect {
                x: panel.x + PANEL_PADDING,
                y: sep_y,
                width: PANEL_W - 2 * PANEL_PADDING,
                height: 1,
            },
            BORDER_COLOR,
        );

        // Event Viewer toggle
        Self::draw_checkbox(
            renderer,
            self.font,
            self.content_grid().cell(3, 0),
            "Event Viewer",
            self.event_viewer_pending,
        );

        // OK / Cancel buttons
        Self::draw_button(renderer, self.font, self.ok_bounds(), "OK", BTN_OK_BG);
        Self::draw_button(renderer, self.font, self.cancel_bounds(), "Cancel", BTN_BG);

        // Debug: draw hit area outlines in red for close, ok, cancel
        draw_border(renderer, self.close_bounds(), Color(255, 0, 0, 255), 1);
        draw_border(renderer, self.ok_bounds(), Color(0, 255, 0, 255), 1);
        draw_border(renderer, self.cancel_bounds(), Color(255, 255, 0, 255), 1);
        draw_border(
            renderer,
            self.content_grid().cell(0, 0),
            Color(0, 255, 255, 255),
            1,
        );
        draw_border(
            renderer,
            self.content_grid().cell(1, 0),
            Color(255, 0, 255, 255),
            1,
        );
        draw_border(
            renderer,
            self.content_grid().cell(3, 0),
            Color(0, 255, 255, 255),
            1,
        );
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if matches!(event, Event::Tick) {
            if self.clear_countdown > 0 {
                self.clear_countdown -= 1;
            }
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            self.last_touch = Some((*x, *y));

            // Log every PressRelease received by config menu
            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            unsafe {
                let prev = d3::EVENT_COUNT.read_volatile();
                d3::EVENT_COUNT.write_volatile(prev.wrapping_add(1));
                d3::LAST_X.write_volatile(*x as u32);
                d3::LAST_Y.write_volatile(*y as u32);
                d3::MENU_VIS.write_volatile(self.visible as u32);
            }

            // Gear toggle is handled by the IconStrip callback.
            // Config menu only handles taps when the panel is visible.
            if self.visible {
                // Write touch coords + hit result to D3 SRAM for probe-rs
                #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                unsafe {
                    d3::TOUCH_X.write_volatile(*x as u32);
                    d3::TOUCH_Y.write_volatile(*y as u32);
                }

                // Close X
                if Self::inside(self.close_bounds(), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_CLOSE);
                    }
                    self.pending = self.applied;
                    self.fire_on_preview();
                    self.event_viewer_pending = self.event_viewer_applied;
                    self.close_menu();
                    self.dirty_frames = 2;
                    return true;
                }

                // OK button — apply and close
                if Self::inside(self.ok_bounds(), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_OK);
                    }
                    if self.pending != self.applied {
                        self.applied = self.pending;
                        self.fire_on_change();
                    }
                    if self.event_viewer_pending != self.event_viewer_applied {
                        self.event_viewer_applied = self.event_viewer_pending;
                        self.fire_on_event_viewer_change();
                    }
                    self.close_menu();
                    self.dirty_frames = 2;
                    return true;
                }

                // Cancel button — revert and close
                if Self::inside(self.cancel_bounds(), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_CANCEL);
                    }
                    self.pending = self.applied;
                    self.fire_on_preview();
                    self.event_viewer_pending = self.event_viewer_applied;
                    self.close_menu();
                    self.dirty_frames = 2;
                    return true;
                }

                // English checkbox
                if Self::inside(self.content_grid().cell(0, 0), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_ENGLISH);
                    }
                    self.pending = 0;
                    self.fire_on_preview();
                    self.dirty_frames = 2;
                    return true;
                }

                // French checkbox
                if Self::inside(self.content_grid().cell(1, 0), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_FRENCH);
                    }
                    self.pending = 1;
                    self.fire_on_preview();
                    self.dirty_frames = 2;
                    return true;
                }

                // Event Viewer checkbox (toggle)
                if Self::inside(self.content_grid().cell(3, 0), *x, *y) {
                    self.event_viewer_pending = !self.event_viewer_pending;
                    self.dirty_frames = 2;
                    return true;
                }

                // Tap outside panel: cancel
                if !Self::inside(self.panel_bounds(), *x, *y) {
                    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                    unsafe {
                        d3::HIT_CODE.write_volatile(d3::ACT_OUTSIDE);
                    }
                    self.pending = self.applied;
                    self.fire_on_preview();
                    self.event_viewer_pending = self.event_viewer_applied;
                    self.close_menu();
                    self.dirty_frames = 2;
                    return true;
                }

                // Inside panel but no specific hit
                #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                unsafe {
                    d3::HIT_CODE.write_volatile(d3::ACT_MISS);
                }

                return true;
            }
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
