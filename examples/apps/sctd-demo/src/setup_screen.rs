// SPDX-License-Identifier: MIT
//! Setup Screen — SCTD-03 §7/§8.
//!
//! Displayed when selector slot 0 (⚙) is active. Top row has two visually
//! separated touch **tabs** (`DP` / `MP`); tapping a tab switches the config
//! body without leaving the Setup screen. The body renders finger-sized touch
//! targets for the active tab's controls using the same button-rect +
//! `PressRelease` hit-test pattern as `MachinePanel` (SCTD-02 §6.2).
//!
//! SCTD-03 §8 config surface:
//! - **DP tab**: `Auto` checkbox (default on) + Speed selector x0.5/x1/x2.
//! - **MP tab**: Default-source selector USB/SD/AUX + Auto-Ready checkbox.

use alloc::{boxed::Box, vec::Vec};
use core::cell::RefCell;

use rlvgl_core::{
    bitmap_font::{BitmapFont, FONT_6X10},
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_ui::draw_helpers::{PANEL_RADIUS, draw_rounded_border, fill_rounded_rect};

const PANEL_BG: Color = Color(22, 29, 41, 255);
const PANEL_BORDER: Color = Color(75, 94, 122, 255);
const TITLE_COLOR: Color = Color(240, 244, 248, 255);
const BODY_COLOR: Color = Color(188, 201, 214, 255);
const ACTIVE_TAB_BG: Color = Color(52, 66, 92, 255);
const INACTIVE_TAB_BG: Color = Color(30, 38, 52, 255);
const ACTIVE_TAB_FG: Color = Color(240, 200, 80, 255);
const INACTIVE_TAB_FG: Color = Color(148, 200, 240, 255);
const BTN_ACTIVE_BG: Color = Color(52, 66, 92, 255);
const BTN_INACTIVE_BG: Color = Color(30, 38, 52, 255);
const BTN_ACTIVE_FG: Color = Color(240, 200, 80, 255);
const BTN_INACTIVE_FG: Color = Color(148, 200, 240, 255);
const PADDING: i32 = 12;
const TAB_HEIGHT: i32 = 32;
const TAB_GAP: i32 = 8;

/// Which tab is active in the Setup screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupTab {
    Dp,
    Mp,
}

/// DP configuration exposed by the Setup screen.
#[derive(Clone, Debug)]
pub struct DpConfig {
    /// Auto mode: when true the table auto-arrives / auto-departs philosophers.
    pub auto: bool,
    /// Speed setting index into `IDP_SPEED_LABELS` (0=x0.5, 1=x1, 2=x2).
    pub speed_idx: usize,
}

impl Default for DpConfig {
    fn default() -> Self {
        Self {
            auto: true,   // SCTD-03 §8 default
            speed_idx: 1, // x1 default
        }
    }
}

/// MP configuration exposed by the Setup screen.
#[derive(Clone, Debug)]
pub struct MpConfig {
    /// Default source index (0=USB, 1=SD, 2=AUX). Default USB per SCTD-03 §8.
    pub source_idx: usize,
    /// Auto-Ready on open: seed MP past idle on launch. Default on per SCTD-03 §8.
    pub auto_ready: bool,
}

impl Default for MpConfig {
    fn default() -> Self {
        Self {
            source_idx: 0,
            auto_ready: true,
        }
    }
}

impl MpConfig {
    /// Return the selected source name.
    #[allow(dead_code)]
    pub fn source_name(&self) -> &'static str {
        MP_SOURCE_LABELS[self.source_idx.min(MP_SOURCE_LABELS.len() - 1)]
    }
}

/// Source labels for the MP tab selector.
pub const MP_SOURCE_LABELS: [&str; 3] = ["USB", "SD", "AUX"];
/// Speed labels for the DP tab selector.
pub const DP_SPEED_LABELS: [&str; 3] = ["x0.5", "x1", "x2"];

// ---------------------------------------------------------------------------
// Tap callbacks
// ---------------------------------------------------------------------------

/// Callbacks wired by the controller into the Setup screen.
pub struct SetupCallbacks {
    /// Called when the user toggles the DP Auto checkbox.
    pub on_dp_auto: Option<Box<dyn FnMut(bool)>>,
    /// Called when the user cycles the DP Speed selector.
    pub on_dp_speed: Option<Box<dyn FnMut(usize)>>,
    /// Called when the user cycles the MP source selector.
    pub on_mp_source: Option<Box<dyn FnMut(usize)>>,
    /// Called when the user toggles the MP Auto-Ready checkbox.
    pub on_mp_auto_ready: Option<Box<dyn FnMut(bool)>>,
}

#[allow(clippy::derivable_impls)]
// Note: cannot use #[derive(Default)] because Box<dyn FnMut(...)> doesn't impl Default.
impl Default for SetupCallbacks {
    fn default() -> Self {
        Self {
            on_dp_auto: None,
            on_dp_speed: None,
            on_mp_source: None,
            on_mp_auto_ready: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SetupScreen widget
// ---------------------------------------------------------------------------

/// The Setup screen shown when selector slot 0 (⚙) is active.
///
/// Call [`set_visible(false)`](Self::set_visible) to hide the screen (e.g.
/// when a run-view slot is active); `draw` and `handle_event` are no-ops when
/// hidden.
pub struct SetupScreen {
    bounds: Rect,
    active_tab: SetupTab,
    dp_config: DpConfig,
    mp_config: MpConfig,
    font: &'static BitmapFont,
    /// Whether the screen is currently shown.
    visible: bool,
    /// Recorded hit-rects for the two tab buttons (index 0=DP, 1=MP).
    tab_rects: RefCell<[Option<Rect>; 2]>,
    /// Recorded hit-rects for the body controls (variable count per tab).
    body_rects: RefCell<Vec<Rect>>,
    callbacks: RefCell<SetupCallbacks>,
}

impl SetupScreen {
    /// Create the Setup screen with the given bounds.
    ///
    /// The screen is created visible. The controller calls
    /// [`set_visible(false)`](Self::set_visible) when a run-view slot is active,
    /// and [`set_visible(true)`](Self::set_visible) when slot 0 (Setup) is active.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            active_tab: SetupTab::Dp, // SCTD-03 §7 default
            dp_config: DpConfig::default(),
            mp_config: MpConfig::default(),
            font: &FONT_6X10,
            visible: true,
            tab_rects: RefCell::new([None; 2]),
            body_rects: RefCell::new(Vec::new()),
            callbacks: RefCell::new(SetupCallbacks::default()),
        }
    }

    /// Show or hide the Setup screen. When hidden, `draw` and `handle_event` are no-ops.
    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    /// Whether the Setup screen is currently shown.
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set the callbacks for config-change events.
    pub fn set_callbacks(&self, cbs: SetupCallbacks) {
        *self.callbacks.borrow_mut() = cbs;
    }

    /// Return the active tab.
    #[allow(dead_code)]
    pub fn active_tab(&self) -> SetupTab {
        self.active_tab
    }

    /// Set the active tab directly (for controller-driven switching).
    #[allow(dead_code)]
    pub fn set_active_tab(&mut self, tab: SetupTab) {
        self.active_tab = tab;
    }

    /// Update the DP config (called from the controller when the adapter changes).
    #[allow(dead_code)]
    pub fn set_dp_config(&mut self, cfg: DpConfig) {
        self.dp_config = cfg;
    }

    /// Update the MP config.
    #[allow(dead_code)]
    pub fn set_mp_config(&mut self, cfg: MpConfig) {
        self.mp_config = cfg;
    }

    /// Read the current DP config.
    #[allow(dead_code)]
    pub fn dp_config(&self) -> &DpConfig {
        &self.dp_config
    }

    /// Read the current MP config.
    #[allow(dead_code)]
    pub fn mp_config(&self) -> &MpConfig {
        &self.mp_config
    }

    fn line_height(&self) -> i32 {
        self.font.scaled_height() + 4
    }

    fn btn_height(&self) -> i32 {
        self.line_height() + 12
    }

    fn draw_button(
        &self,
        renderer: &mut dyn Renderer,
        label: &str,
        x: i32,
        y: i32,
        w: i32,
        active: bool,
    ) -> Rect {
        let h = self.btn_height();
        let (bg, fg) = if active {
            (BTN_ACTIVE_BG, BTN_ACTIVE_FG)
        } else {
            (BTN_INACTIVE_BG, BTN_INACTIVE_FG)
        };
        let face = Rect {
            x,
            y,
            width: w,
            height: h - 4,
        };
        fill_rounded_rect(renderer, face, bg, 4);
        self.font.draw_str(renderer, x + 10, y + 5, label, fg);
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn draw_tab_row(&self, renderer: &mut dyn Renderer, inner_x: i32, y: i32, inner_w: i32) {
        let tab_w = (inner_w - TAB_GAP) / 2;
        let dp_rect = Rect {
            x: inner_x,
            y,
            width: tab_w,
            height: TAB_HEIGHT,
        };
        let mp_rect = Rect {
            x: inner_x + tab_w + TAB_GAP,
            y,
            width: inner_w - tab_w - TAB_GAP,
            height: TAB_HEIGHT,
        };

        // Draw DP tab.
        let (dp_bg, dp_fg) = if self.active_tab == SetupTab::Dp {
            (ACTIVE_TAB_BG, ACTIVE_TAB_FG)
        } else {
            (INACTIVE_TAB_BG, INACTIVE_TAB_FG)
        };
        fill_rounded_rect(renderer, dp_rect, dp_bg, 4);
        draw_rounded_border(renderer, dp_rect, PANEL_BORDER, 1, 4);
        let dp_lx = inner_x + (tab_w - 2 * self.font.scaled_width()) / 2;
        let dp_ly = y + (TAB_HEIGHT - self.font.scaled_height()) / 2;
        self.font.draw_str(renderer, dp_lx, dp_ly, "DP", dp_fg);

        // Draw MP tab.
        let (mp_bg, mp_fg) = if self.active_tab == SetupTab::Mp {
            (ACTIVE_TAB_BG, ACTIVE_TAB_FG)
        } else {
            (INACTIVE_TAB_BG, INACTIVE_TAB_FG)
        };
        fill_rounded_rect(renderer, mp_rect, mp_bg, 4);
        draw_rounded_border(renderer, mp_rect, PANEL_BORDER, 1, 4);
        let mp_lx = inner_x
            + tab_w
            + TAB_GAP
            + (inner_w - tab_w - TAB_GAP - 2 * self.font.scaled_width()) / 2;
        let mp_ly = y + (TAB_HEIGHT - self.font.scaled_height()) / 2;
        self.font.draw_str(renderer, mp_lx, mp_ly, "MP", mp_fg);

        let mut tab_rects = self.tab_rects.borrow_mut();
        tab_rects[0] = Some(dp_rect);
        tab_rects[1] = Some(mp_rect);
    }

    /// Test-only: recorded tab rect for index (0=DP, 1=MP).
    #[cfg(test)]
    pub fn debug_tab_rect(&self, idx: usize) -> Option<Rect> {
        self.tab_rects.borrow()[idx.min(1)]
    }

    /// Test-only: recorded body control rect at index.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn debug_body_rect(&self, idx: usize) -> Option<Rect> {
        self.body_rects.borrow().get(idx).copied()
    }
}

impl Widget for SetupScreen {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        // Panel background.
        fill_rounded_rect(renderer, self.bounds, PANEL_BG, PANEL_RADIUS);
        draw_rounded_border(renderer, self.bounds, PANEL_BORDER, 1, PANEL_RADIUS);

        let inner_x = self.bounds.x + PADDING;
        let inner_w = (self.bounds.width - PADDING * 2).max(0);
        let mut y = self.bounds.y + PADDING;
        let lh = self.line_height();

        // Title.
        self.font
            .draw_str(renderer, inner_x, y, "Setup", TITLE_COLOR);
        y += lh + 4;

        // Divider.
        renderer.fill_rect(
            Rect {
                x: inner_x,
                y,
                width: inner_w,
                height: 1,
            },
            PANEL_BORDER,
        );
        y += 8;

        // Tab row: [ DP ]   [ MP ] — visually separated (SCTD-03 §7).
        self.draw_tab_row(renderer, inner_x, y, inner_w);
        y += TAB_HEIGHT + 8;

        // Divider below tabs.
        renderer.fill_rect(
            Rect {
                x: inner_x,
                y,
                width: inner_w,
                height: 1,
            },
            PANEL_BORDER,
        );
        y += 8;

        let bh = self.btn_height();
        let mut body_rects = self.body_rects.borrow_mut();
        body_rects.clear();

        match self.active_tab {
            SetupTab::Dp => {
                // DP tab body: Auto checkbox + Speed selector (SCTD-03 §8).
                self.font
                    .draw_str(renderer, inner_x, y, "DP Settings:", BODY_COLOR);
                y += lh + 4;

                // Auto checkbox: show checked state.
                let auto_label = if self.dp_config.auto {
                    "[x] Auto (table auto-runs)"
                } else {
                    "[ ] Auto (table auto-runs)"
                };
                let r = self.draw_button(
                    renderer,
                    auto_label,
                    inner_x,
                    y,
                    inner_w,
                    self.dp_config.auto,
                );
                body_rects.push(r);
                y += bh;

                // Speed selector — three buttons in a row.
                self.font
                    .draw_str(renderer, inner_x, y, "Speed:", BODY_COLOR);
                y += lh + 2;
                let spd_w = ((inner_w - TAB_GAP * 2) / 3).max(1);
                for (i, &lbl) in DP_SPEED_LABELS.iter().enumerate() {
                    let sx = inner_x + i as i32 * (spd_w + TAB_GAP);
                    let active = self.dp_config.speed_idx == i;
                    let r = self.draw_button(renderer, lbl, sx, y, spd_w, active);
                    body_rects.push(r);
                }
            }
            SetupTab::Mp => {
                // MP tab body: source selector + Auto-Ready checkbox (SCTD-03 §8).
                self.font
                    .draw_str(renderer, inner_x, y, "MP Settings:", BODY_COLOR);
                y += lh + 4;

                // Default source — three buttons in a row.
                self.font
                    .draw_str(renderer, inner_x, y, "Default Source:", BODY_COLOR);
                y += lh + 2;
                let src_w = ((inner_w - TAB_GAP * 2) / 3).max(1);
                for (i, &lbl) in MP_SOURCE_LABELS.iter().enumerate() {
                    let sx = inner_x + i as i32 * (src_w + TAB_GAP);
                    let active = self.mp_config.source_idx == i;
                    let r = self.draw_button(renderer, lbl, sx, y, src_w, active);
                    body_rects.push(r);
                }
                y += bh;

                // Auto-Ready checkbox.
                let ar_label = if self.mp_config.auto_ready {
                    "[x] Auto-Ready on open"
                } else {
                    "[ ] Auto-Ready on open"
                };
                let r = self.draw_button(
                    renderer,
                    ar_label,
                    inner_x,
                    y,
                    inner_w,
                    self.mp_config.auto_ready,
                );
                body_rects.push(r);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        if let Event::PressRelease { x, y } = event {
            // Check tab row first.
            {
                let tab_rects = self.tab_rects.borrow();
                if let Some(r) = tab_rects[0]
                    && *x >= r.x
                    && *x < r.x + r.width
                    && *y >= r.y
                    && *y < r.y + r.height
                {
                    drop(tab_rects);
                    self.active_tab = SetupTab::Dp;
                    return true;
                }
                if let Some(r) = tab_rects[1]
                    && *x >= r.x
                    && *x < r.x + r.width
                    && *y >= r.y
                    && *y < r.y + r.height
                {
                    drop(tab_rects);
                    self.active_tab = SetupTab::Mp;
                    return true;
                }
            }

            // Check body controls.
            let hit =
                self.body_rects.borrow().iter().position(|r| {
                    *x >= r.x && *x < r.x + r.width && *y >= r.y && *y < r.y + r.height
                });

            if let Some(idx) = hit {
                match self.active_tab {
                    SetupTab::Dp => {
                        // idx 0 = Auto toggle; idx 1/2/3 = Speed x0.5/x1/x2.
                        match idx {
                            0 => {
                                self.dp_config.auto = !self.dp_config.auto;
                                let new_val = self.dp_config.auto;
                                if let Some(cb) = self.callbacks.borrow_mut().on_dp_auto.as_mut() {
                                    cb(new_val);
                                }
                            }
                            1..=3 => {
                                self.dp_config.speed_idx = idx - 1;
                                let new_val = idx - 1;
                                if let Some(cb) = self.callbacks.borrow_mut().on_dp_speed.as_mut() {
                                    cb(new_val);
                                }
                            }
                            _ => {}
                        }
                    }
                    SetupTab::Mp => {
                        // idx 0/1/2 = Source USB/SD/AUX; idx 3 = Auto-Ready toggle.
                        match idx {
                            0..=2 => {
                                self.mp_config.source_idx = idx;
                                let new_val = idx;
                                if let Some(cb) = self.callbacks.borrow_mut().on_mp_source.as_mut()
                                {
                                    cb(new_val);
                                }
                            }
                            3 => {
                                self.mp_config.auto_ready = !self.mp_config.auto_ready;
                                let new_val = self.mp_config.auto_ready;
                                if let Some(cb) =
                                    self.callbacks.borrow_mut().on_mp_auto_ready.as_mut()
                                {
                                    cb(new_val);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                return true;
            }
        }
        false
    }
}
