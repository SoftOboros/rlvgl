//! LPAR-16 pixel-golden conformance fixtures for LPAR-13 selection/navigation widgets (LPAR-16 §6).
//!
//! Each test:
//!   1. Renders through a software BufferDisplay renderer.
//!   2. Renders a second time into a fresh buffer and asserts byte-identical output (determinism).
//!   3. Asserts at least one concrete expected pixel value (anti-stably-wrong anchor).
//!   4. Asserts the buffer is non-trivially filled (not entirely the initial white).
//!
//! `draw_text_shaped` is a no-op so all anchors are fill_rect-based fills.
#![allow(clippy::erasing_op, clippy::identity_op)]

use rlvgl_core::font::ShapedText;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};

// ── Shared renderer ─────────────────────────────────────────────────────────

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl<'a> Renderer for DisplayRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}

    // Suppress glyph-extent rectangles so anchors target fill-based chrome only.
    fn draw_text_shaped(&mut self, _shaped: &ShapedText<'_>, _o: (i32, i32), _c: Color) {}
}

// ── Helper ──────────────────────────────────────────────────────────────────

fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ── 1. Dropdown ─────────────────────────────────────────────────────────────
// Anchor: trigger bg_color at (0,0); open-list selected_color at row 0 of list area.
// State drive: open() and set_options so the list area renders with the selected row fill.

#[test]
fn dropdown_pixel_golden() {
    use rlvgl_widgets::dropdown::Dropdown;

    let make = || {
        let mut d = Dropdown::new(r(0, 0, 80, 80));
        // Distinctive trigger background so the anchor is not default white.
        d.style.bg_color = Color(10, 20, 30, 255);
        d.item_style.bg_color = Color(40, 50, 60, 255);
        d.selected_color = Color(70, 80, 90, 255);
        d.set_options(&["Alpha", "Beta", "Gamma"]);
        d.open(); // exposes selected_color fill via the item list
        d
    };

    // First render.
    let mut display1 = BufferDisplay::new(80, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    // Second render — must be byte-identical (determinism contract).
    let mut display2 = BufferDisplay::new(80, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "dropdown: non-deterministic render"
    );

    // Anchor: trigger bg fills row 0, col 0.
    assert_eq!(
        display1.buffer[0 * 80 + 0],
        Color(10, 20, 30, 255),
        "dropdown: trigger bg_color anchor"
    );

    // Non-trivial: at least one pixel differs from initial white.
    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "dropdown: buffer is trivially empty"
    );
}

// ── 2. Keyboard ─────────────────────────────────────────────────────────────
// Keyboard delegates draw() to ButtonMatrix.  ButtonMatrix fills each cell with
// `button_color` (not `style.bg_color`).  Anchor: matrix button_color at (0,0).

#[test]
fn keyboard_pixel_golden() {
    use rlvgl_widgets::keyboard::Keyboard;

    let make = || {
        let mut kb = Keyboard::new(r(0, 0, 160, 80));
        // Distinctive cell fill via ButtonMatrix::button_color.
        kb.matrix_mut().button_color = Color(11, 22, 33, 255);
        kb
    };

    let mut display1 = BufferDisplay::new(160, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(160, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "keyboard: non-deterministic render"
    );

    // Anchor: button_color fills cell at origin.
    assert_eq!(
        display1.buffer[0 * 160 + 0],
        Color(11, 22, 33, 255),
        "keyboard: button_color anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "keyboard: buffer is trivially empty"
    );
}

// ── 3. Menu ──────────────────────────────────────────────────────────────────
// Anchor: header_color fills row 0; item_style.bg_color fills list area.
// State drive: one page with one navigable item highlighted (navigate_next).

#[test]
fn menu_pixel_golden() {
    use rlvgl_widgets::menu::{Menu, MenuItem};

    let make = || {
        let mut m = Menu::new(r(0, 0, 100, 80));
        m.header_color = Color(12, 34, 56, 255);
        m.item_style.bg_color = Color(78, 90, 12, 255);
        m.selected_color = Color(34, 56, 78, 255);

        let root = m.add_page("Root");
        m.add_item(root, MenuItem::Label("Item A".into()));
        m.add_item(root, MenuItem::Label("Item B".into()));
        m.set_root_page(root);
        m.navigate_next(); // highlight item 0 → selected_color fill
        m
    };

    let mut display1 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "menu: non-deterministic render"
    );

    // Anchor: header fills (0,0).
    assert_eq!(
        display1.buffer[0 * 100 + 0],
        Color(12, 34, 56, 255),
        "menu: header_color anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "menu: buffer is trivially empty"
    );
}

// ── 4. Roller ────────────────────────────────────────────────────────────────
// Anchor: selected_color fills the center row.
// With 3 visible rows of 16 px each the center row starts at y=16 (row 1).

#[test]
fn roller_pixel_golden() {
    use rlvgl_widgets::roller::{Roller, RollerMode};

    // 3 rows of 16 px = 48 px height; center row at y=[16..32).
    let make = || {
        let mut ro = Roller::new(r(0, 0, 60, 48));
        ro.style.bg_color = Color(5, 5, 5, 255);
        ro.items_color = Color(50, 50, 50, 255);
        ro.selected_color = Color(99, 100, 101, 255);
        ro.set_options(&["One", "Two", "Three"], RollerMode::Normal);
        // selected=0 by default; set_selected(1) to center row is non-trivially colored
        ro.set_selected(1, false);
        ro
    };

    let mut display1 = BufferDisplay::new(60, 48);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(60, 48);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "roller: non-deterministic render"
    );

    // Anchor: selected_color in the center row (y=16, x=0).
    assert_eq!(
        display1.buffer[16 * 60 + 0],
        Color(99, 100, 101, 255),
        "roller: selected_color center row anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "roller: buffer is trivially empty"
    );
}

// ── 5. Tabview ───────────────────────────────────────────────────────────────
// Anchor: active_tab_color at the first tab button pixel (0,0);
//         content area fill (style.bg_color) below bar.
// State drive: two tabs added; tab 0 is active by default.

#[test]
fn tabview_pixel_golden() {
    use rlvgl_widgets::tabview::{TabBarPos, Tabview};

    // Bar is 24 px tall; content area y=[24..80).
    let make = || {
        let mut tv = Tabview::new(r(0, 0, 100, 80), TabBarPos::Top);
        tv.active_tab_color = Color(13, 27, 41, 255);
        tv.tab_color = Color(55, 66, 77, 255);
        tv.style.bg_color = Color(88, 99, 110, 255);
        tv.add_tab("Alpha");
        tv.add_tab("Beta");
        // Tab 0 is active automatically; content rect will be filled with bg_color.
        tv
    };

    let mut display1 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "tabview: non-deterministic render"
    );

    // Anchor: active_tab_color at row 0, col 0 (first tab button).
    assert_eq!(
        display1.buffer[0 * 100 + 0],
        Color(13, 27, 41, 255),
        "tabview: active_tab_color anchor"
    );

    // Anchor: content bg_color at row 24 (first row below bar), col 0.
    assert_eq!(
        display1.buffer[24 * 100 + 0],
        Color(88, 99, 110, 255),
        "tabview: content bg_color anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "tabview: buffer is trivially empty"
    );
}

// ── 6. Tileview ──────────────────────────────────────────────────────────────
// Anchor: tile_color fills the entire viewport once a tile is active.

#[test]
fn tileview_pixel_golden() {
    use rlvgl_widgets::tileview::{TileDir, Tileview};

    let make = || {
        let mut tv = Tileview::new(r(0, 0, 60, 40));
        tv.style.bg_color = Color(15, 25, 35, 255);
        tv.tile_color = Color(45, 55, 65, 255);
        tv.add_tile(0, 0, TileDir::ALL);
        // First tile is active automatically; draw() fills with tile_color.
        tv
    };

    let mut display1 = BufferDisplay::new(60, 40);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(60, 40);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "tileview: non-deterministic render"
    );

    // Anchor: tile_color fills (0,0).
    assert_eq!(
        display1.buffer[0 * 60 + 0],
        Color(45, 55, 65, 255),
        "tileview: tile_color anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "tileview: buffer is trivially empty"
    );
}

// ── 7. Window ────────────────────────────────────────────────────────────────
// Anchor: header_color at (0,0); style.bg_color in content area (row 24, col 0).
// State drive: title set; one header button added so button_color appears at right edge.

#[test]
fn window_pixel_golden() {
    use rlvgl_widgets::window::{DEFAULT_HEADER_HEIGHT, Window};

    // DEFAULT_HEADER_HEIGHT = 24; content area y=[24..80).
    let make = || {
        let mut w = Window::new(r(0, 0, 100, 80), DEFAULT_HEADER_HEIGHT);
        w.header_color = Color(14, 28, 42, 255);
        w.button_color = Color(70, 80, 90, 255);
        w.style.bg_color = Color(110, 120, 130, 255);
        w.set_title("My Window");
        w.add_header_button(None, 20);
        w
    };

    let mut display1 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display1,
        };
        make().draw(&mut renderer);
    }

    let mut display2 = BufferDisplay::new(100, 80);
    {
        let mut renderer = DisplayRenderer {
            display: &mut display2,
        };
        make().draw(&mut renderer);
    }
    assert_eq!(
        display1.buffer, display2.buffer,
        "window: non-deterministic render"
    );

    // Anchor: header_color at (0,0).
    assert_eq!(
        display1.buffer[0 * 100 + 0],
        Color(14, 28, 42, 255),
        "window: header_color anchor"
    );

    // Anchor: content bg_color at row 24, col 0.
    assert_eq!(
        display1.buffer[24 * 100 + 0],
        Color(110, 120, 130, 255),
        "window: content bg_color anchor"
    );

    assert!(
        display1
            .buffer
            .iter()
            .any(|&c| c != Color(255, 255, 255, 255)),
        "window: buffer is trivially empty"
    );
}
