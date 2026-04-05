// SPDX-License-Identifier: MIT
//! System information overlay panel.
//!
//! Displays chip identification, clock frequencies, and memory usage
//! in a dark rounded-rect panel using PackedFont rendering.

use alloc::string::String;
use alloc::vec::Vec;

use rlvgl::core::event::Event;
use rlvgl::core::packed_font::PackedFont;
use rlvgl::core::renderer::Renderer;
use rlvgl::core::widget::{Color, Rect, Widget};
use rlvgl::ui::draw_helpers::{draw_border, fill_rounded_rect};

/// Panel constants.
const PANEL_W: i32 = 340;
const PANEL_PADDING: i32 = 16;
const PANEL_RADIUS: u8 = 10;
const CLEAR_FRAMES: u8 = 3;

/// Colors matching the dark overlay theme.
const BG_COLOR: Color = Color(30, 30, 30, 240);
const BORDER_COLOR: Color = Color(100, 100, 100, 255);
const TITLE_COLOR: Color = Color(80, 180, 255, 255);
const TEXT_COLOR: Color = Color(220, 220, 220, 255);
const DIM_COLOR: Color = Color(140, 140, 140, 255);

/// A line of system info text.
struct InfoLine {
    label: &'static str,
    value: String,
}

/// System information overlay.
pub struct SysInfoPanel {
    visible: bool,
    bounds: Rect,
    lines: Vec<InfoLine>,
    font: &'static PackedFont,
    clear_countdown: u8,
    clear_bounds: Option<Rect>,
}

impl SysInfoPanel {
    /// Create a new system info panel centered on the display.
    pub fn new(font: &'static PackedFont) -> Self {
        let line_h = font.height as i32 + 6;
        // Reserve space for title + up to 10 info lines
        let panel_h = PANEL_PADDING + line_h + PANEL_PADDING + 10 * line_h + PANEL_PADDING;
        // Center horizontally in the 720px content area
        let panel_x = (720 - PANEL_W) / 2;
        let panel_y = (480 - panel_h) / 2;

        Self {
            visible: false,
            bounds: Rect {
                x: panel_x,
                y: panel_y,
                width: PANEL_W,
                height: panel_h,
            },
            lines: Vec::new(),
            font,
            clear_countdown: 0,
            clear_bounds: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the panel, refreshing system info.
    pub fn show(&mut self) {
        self.lines.clear();
        self.collect_info();
        // Shrink panel height to fit actual content
        let line_h = self.font.height as i32 + 6;
        let content_h = PANEL_PADDING + line_h + PANEL_PADDING
            + self.lines.len() as i32 * line_h + PANEL_PADDING;
        self.bounds.height = content_h;
        self.bounds.y = (480 - content_h) / 2;
        self.visible = true;
    }

    /// Hide the panel.
    pub fn hide(&mut self) {
        if self.visible {
            self.clear_bounds = Some(self.bounds);
            self.clear_countdown = CLEAR_FRAMES;
            self.visible = false;
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Collect system information from hardware registers.
    fn collect_info(&mut self) {
        // MCU identification
        self.lines.push(InfoLine {
            label: "MCU",
            value: String::from("STM32H747XIH6"),
        });

        // DBGMCU_IDCODE at 0x5C001000
        let idcode = unsafe { (0x5C00_1000u32 as *const u32).read_volatile() };
        let dev_id = idcode & 0xFFF;
        let rev_id = (idcode >> 16) & 0xFFFF;
        self.lines.push(InfoLine {
            label: "DEV_ID",
            value: format_hex_u16(dev_id as u16),
        });
        self.lines.push(InfoLine {
            label: "REV_ID",
            value: format_hex_u16(rev_id as u16),
        });

        // Flash size at 0x1FF1E880
        let flash_kb = unsafe { (0x1FF1_E880u32 as *const u16).read_volatile() };
        self.lines.push(InfoLine {
            label: "Flash",
            value: format_kb(flash_kb as u32),
        });

        // RCC PLL1 config → approximate SYSCLK
        let pll1divr = unsafe { (0x5802_4830u32 as *const u32).read_volatile() };
        let pll_n = ((pll1divr & 0x1FF) + 1) as u32;
        let pll_p = (((pll1divr >> 9) & 0x7F) + 1) as u32;
        // HSE = 25 MHz on H747I-DISCO, PLL1_P = HSE/M * N / P
        // M is in RCC_PLLCKSELR bits 7:4
        let pllckselr = unsafe { (0x5802_4828u32 as *const u32).read_volatile() };
        let pll_m = ((pllckselr >> 4) & 0x3F) as u32;
        let sysclk_mhz = if pll_m > 0 { 25 * pll_n / pll_m / pll_p } else { 0 };
        self.lines.push(InfoLine {
            label: "SYSCLK",
            value: format_mhz(sysclk_mhz),
        });

        // Heap usage estimate (write marker, read back)
        // Use alloc::vec to probe — allocate and immediately free
        let heap_marker = unsafe { (0x3800_0690u32 as *const u32).read_volatile() };
        self.lines.push(InfoLine {
            label: "Heap",
            value: format_hex_u32(heap_marker),
        });

        // D3 SRAM telemetry: tick count
        let ticks = unsafe { (0x3800_0660u32 as *const u32).read_volatile() };
        self.lines.push(InfoLine {
            label: "Ticks",
            value: format_dec(ticks),
        });
    }
}

impl Widget for SysInfoPanel {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }

        fill_rounded_rect(renderer, self.bounds, BG_COLOR, PANEL_RADIUS);
        draw_border(renderer, self.bounds, BORDER_COLOR, 2);

        let line_h = self.font.height as i32 + 6;
        let mut y = self.bounds.y + PANEL_PADDING;
        let x = self.bounds.x + PANEL_PADDING;

        // Title
        self.font.draw_str(renderer, x, y, "System Information", TITLE_COLOR);
        y += line_h + PANEL_PADDING;

        // Info lines
        for line in &self.lines {
            self.font.draw_str(renderer, x, y, line.label, DIM_COLOR);
            // Right-align value
            let val_w = self.font.measure(&line.value);
            let val_x = self.bounds.x + self.bounds.width - PANEL_PADDING - val_w;
            self.font.draw_str(renderer, val_x, y, &line.value, TEXT_COLOR);
            y += line_h;
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            // Any tap closes the panel
            let _ = (x, y);
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

// ── Formatting helpers (no std::fmt available) ──────────────────────────

fn format_hex_u16(v: u16) -> String {
    let mut s = String::from("0x");
    for i in (0..4).rev() {
        let nib = (v >> (i * 4)) & 0xF;
        s.push(char::from(if nib < 10 { b'0' + nib as u8 } else { b'A' + nib as u8 - 10 }));
    }
    s
}

fn format_hex_u32(v: u32) -> String {
    let mut s = String::from("0x");
    for i in (0..8).rev() {
        let nib = (v >> (i * 4)) & 0xF;
        s.push(char::from(if nib < 10 { b'0' + nib as u8 } else { b'A' + nib as u8 - 10 }));
    }
    s
}

fn format_kb(kb: u32) -> String {
    let mut s = format_dec(kb);
    s.push_str(" KB");
    s
}

fn format_mhz(mhz: u32) -> String {
    let mut s = format_dec(mhz);
    s.push_str(" MHz");
    s
}

fn format_dec(mut v: u32) -> String {
    if v == 0 {
        return String::from("0");
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while v > 0 {
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        i += 1;
    }
    let mut s = String::with_capacity(i);
    for &b in buf[..i].iter().rev() {
        s.push(b as char);
    }
    s
}
