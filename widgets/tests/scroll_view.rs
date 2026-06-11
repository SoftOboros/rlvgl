// SPDX-License-Identifier: MIT
//! REND-00 §12 acceptance: spike (unclipped tree path documented),
//! edge-crop goldens, the driving-case scroll-reveal, and the dirty-rect
//! contract.

use std::cell::RefCell;
use std::rc::Rc;

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::StyleBuilder;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_widgets::scroll_view::ScrollView;

const W: usize = 480;
const H: usize = 800;
const BG: u32 = 0xFF10_1018;

/// Headless ARGB8888 buffer renderer (the "D-dump" surface).
struct Buffer {
    pixels: Vec<u32>,
}

impl Buffer {
    fn new() -> Self {
        Self {
            pixels: vec![BG; W * H],
        }
    }
    fn at(&self, x: i32, y: i32) -> u32 {
        self.pixels[y as usize * W + x as usize]
    }
}

impl Renderer for Buffer {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let argb = color.to_argb8888();
        for y in rect.y.max(0)..(rect.y + rect.height).min(H as i32) {
            for x in rect.x.max(0)..(rect.x + rect.width).min(W as i32) {
                self.pixels[y as usize * W + x as usize] = argb;
            }
        }
    }
    fn draw_text(&mut self, _position: (i32, i32), _text: &str, _color: Color) {}
}

/// A plain colored cell widget positioned in whatever space its parent uses.
struct Cell {
    bounds: Rect,
    color: Color,
}

impl Cell {
    fn new(bounds: Rect, color: Color) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { bounds, color }))
    }
}

impl Widget for Cell {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.color);
    }
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

/// Distinct opaque color per row index so bleed/missing rows are visible.
fn row_color(row: i32) -> Color {
    Color(40 + row as u8 * 30, 80, 200 - row as u8 * 25, 255)
}

// ───────────────────────────────────────────────────────────────────────────
// Spike (REND-00 §12, deliverable 1)
// ───────────────────────────────────────────────────────────────────────────

/// Documents the gap REND closes: on the plain widget-tree draw path a
/// child straddling its parent's edge renders fully — nothing clips it.
/// (If this test ever fails, clipping appeared somewhere else; re-read
/// REND-00 §2 before touching it.)
#[test]
fn spike_unclipped_tree_path_bleeds_past_parent_bounds() {
    let parent = Rect {
        x: 100,
        y: 100,
        width: 200,
        height: 200,
    };
    // Child overhangs the parent's bottom edge by 100 px.
    let child = Cell::new(
        Rect {
            x: 120,
            y: 250,
            width: 50,
            height: 100,
        },
        Color(255, 0, 0, 255),
    );

    let mut frame = Buffer::new();
    // Parent background, then child — the bare tree path, no ScrollView.
    frame.fill_rect(parent, Color(0, 0, 0, 255));
    child.borrow().draw(&mut frame);

    // The child painted below the parent's bottom edge (y >= 300): bleed.
    assert_eq!(
        frame.at(130, 320),
        Color(255, 0, 0, 255).to_argb8888(),
        "spike: child bleeds past the parent edge on the unclipped path"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// Edge crops (acceptance a)
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn children_crop_exactly_at_all_four_viewport_edges() {
    let viewport = Rect {
        x: 100,
        y: 100,
        width: 200,
        height: 200,
    };
    let mut view = ScrollView::new(viewport, 200);
    view.style = StyleBuilder::new().bg_color(Color(0, 0, 0, 255)).build();
    let red = Color(255, 0, 0, 255);
    // Content space: viewport interior spans (0..200, 0..200).
    for bounds in [
        Rect {
            x: -50,
            y: 80,
            width: 100,
            height: 40,
        }, // left overhang
        Rect {
            x: 150,
            y: 80,
            width: 100,
            height: 40,
        }, // right overhang
        Rect {
            x: 20,
            y: -30,
            width: 40,
            height: 60,
        }, // top overhang
        Rect {
            x: 20,
            y: 170,
            width: 40,
            height: 60,
        }, // bottom overhang
    ] {
        view.add_child(Cell::new(bounds, red));
    }

    let mut frame = Buffer::new();
    view.draw(&mut frame);

    let inside = |x: i32, y: i32| {
        x >= viewport.x
            && x < viewport.x + viewport.width
            && y >= viewport.y
            && y < viewport.y + viewport.height
    };
    let red_argb = red.to_argb8888();
    // No bleed: every red pixel in the frame is inside the viewport.
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            if frame.at(x, y) == red_argb {
                assert!(inside(x, y), "bleed at ({x}, {y})");
            }
        }
    }
    // No missing interior: the visible part of each overhang is painted
    // right up to the edge.
    assert_eq!(frame.at(100, 190), red_argb, "left edge interior");
    assert_eq!(frame.at(299, 190), red_argb, "right edge interior");
    assert_eq!(frame.at(130, 100), red_argb, "top edge interior");
    assert_eq!(frame.at(130, 299), red_argb, "bottom edge interior");
    // And the first pixel past each edge is untouched background-of-frame.
    assert_eq!(frame.at(99, 190), BG, "no bleed past left");
    assert_eq!(frame.at(300, 190), BG, "no bleed past right");
    assert_eq!(frame.at(130, 99), BG, "no bleed past top");
    assert_eq!(frame.at(130, 300), BG, "no bleed past bottom");
}

// ───────────────────────────────────────────────────────────────────────────
// Driving case: 2-column grid of 200 px cells, scroll reveals a row
// (acceptance b)
// ───────────────────────────────────────────────────────────────────────────

/// Build the ticket's driving-case view: 5 rows × 2 columns of 200×200
/// cells (content 1000 px) behind a 400×500 viewport.
fn driving_case_view() -> (ScrollView, Rect) {
    let viewport = Rect {
        x: 40,
        y: 100,
        width: 400,
        height: 500,
    };
    let mut view = ScrollView::new(viewport, 1000);
    view.style = StyleBuilder::new().bg_color(Color(0, 0, 0, 255)).build();
    for row in 0..5 {
        for col in 0..2 {
            view.add_child(Cell::new(
                Rect {
                    x: col * 200,
                    y: row * 200,
                    width: 200,
                    height: 200,
                },
                row_color(row),
            ));
        }
    }
    (view, viewport)
}

/// Reference render: directly paint each cell's viewport intersection.
fn reference_frame(viewport: Rect, scroll_y: i32) -> Buffer {
    let mut frame = Buffer::new();
    frame.fill_rect(viewport, Color(0, 0, 0, 255));
    for row in 0..5 {
        for col in 0..2 {
            let screen = Rect {
                x: viewport.x + col * 200,
                y: viewport.y + row * 200 - scroll_y,
                width: 200,
                height: 200,
            };
            if let Some(visible) = screen.intersect(viewport) {
                frame.fill_rect(visible, row_color(row));
            }
        }
    }
    frame
}

#[test]
fn scroll_reveals_hidden_row_with_clean_partial_rows() {
    let (mut view, viewport) = driving_case_view();

    // Unscrolled: rows 0-1 fully visible, row 2 half visible, rows 3-4
    // hidden. Byte-identical to the reference.
    let mut frame = Buffer::new();
    view.draw(&mut frame);
    assert_eq!(frame.pixels, reference_frame(viewport, 0).pixels);
    assert_eq!(
        frame.at(viewport.x + 10, viewport.y + 499),
        row_color(2).to_argb8888(),
        "row 2 partially visible at the bottom edge"
    );

    // Scroll 350 px: row 1 now straddles the top edge, row 3 (previously
    // fully hidden) is revealed straddling the bottom edge.
    view.scroll_by(350);
    let mut frame = Buffer::new();
    view.draw(&mut frame);
    assert_eq!(frame.pixels, reference_frame(viewport, 350).pixels);

    let top_row_color = frame.at(viewport.x + 10, viewport.y);
    assert_eq!(
        top_row_color,
        row_color(1).to_argb8888(),
        "partial row 1 at top"
    );
    let bottom_row_color = frame.at(viewport.x + 10, viewport.y + 499);
    assert_eq!(
        bottom_row_color,
        row_color(4).to_argb8888(),
        "row 4 revealed at the bottom edge (350 + 500 = 850 ∈ row 4)"
    );
    // Clean edges: nothing painted outside the viewport.
    assert_eq!(frame.at(viewport.x + 10, viewport.y - 1), BG);
    assert_eq!(frame.at(viewport.x + 10, viewport.y + 500), BG);

    // scroll_to(absolute) renders byte-identically to the scroll_by chain.
    let (mut absolute, _) = driving_case_view();
    absolute.scroll_to(350);
    let mut frame_abs = Buffer::new();
    absolute.draw(&mut frame_abs);
    assert_eq!(frame.pixels, frame_abs.pixels);
}

#[test]
fn scroll_clamps_to_content_extent() {
    let (mut view, viewport) = driving_case_view();
    assert_eq!(view.max_scroll(), 500);
    view.scroll_by(10_000);
    assert_eq!(view.scroll_y(), 500);
    let mut frame = Buffer::new();
    view.draw(&mut frame);
    assert_eq!(frame.pixels, reference_frame(viewport, 500).pixels);
    view.scroll_by(-10_000);
    assert_eq!(view.scroll_y(), 0);
}

// ───────────────────────────────────────────────────────────────────────────
// Dirty-rect contract (acceptance c)
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn take_dirty_reports_viewport_only_on_offset_change() {
    let (mut view, viewport) = driving_case_view();
    assert_eq!(view.take_dirty(), None, "clean at construction");

    view.scroll_by(100);
    assert_eq!(
        view.take_dirty(),
        Some(viewport),
        "scroll dirties the viewport"
    );
    assert_eq!(view.take_dirty(), None, "drained");

    view.scroll_to(100);
    assert_eq!(view.take_dirty(), None, "same offset is a no-op");
    view.scroll_by(0);
    assert_eq!(view.take_dirty(), None, "zero delta is a no-op");

    view.scroll_to(500);
    view.scroll_by(10_000);
    assert_eq!(
        view.take_dirty(),
        Some(viewport),
        "one viewport rect per scroll burst"
    );
    view.scroll_by(1); // clamped at max: offset unchanged
    assert_eq!(view.take_dirty(), None, "clamped overshoot is a no-op");
}

// ───────────────────────────────────────────────────────────────────────────
// Event translation (REND-00 §6.7)
// ───────────────────────────────────────────────────────────────────────────

struct EventLog {
    bounds: Rect,
    seen: Vec<Event>,
}

impl Widget for EventLog {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, _renderer: &mut dyn Renderer) {}
    fn handle_event(&mut self, event: &Event) -> bool {
        self.seen.push(event.clone());
        false
    }
}

#[test]
fn pointer_events_translate_to_content_space_and_gate_on_viewport() {
    let viewport = Rect {
        x: 100,
        y: 100,
        width: 200,
        height: 200,
    };
    let child = Rc::new(RefCell::new(EventLog {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 1000,
        },
        seen: Vec::new(),
    }));
    let mut view = ScrollView::new(viewport, 1000);
    view.add_child(child.clone());
    view.scroll_by(300);

    // Inside the viewport: translated into content space (+scroll).
    view.handle_event(&Event::PressRelease { x: 150, y: 180 });
    // Outside the viewport: never delivered.
    view.handle_event(&Event::PressRelease { x: 10, y: 10 });
    // Non-pointer events pass through unchanged.
    view.handle_event(&Event::Tick);

    let seen = &child.borrow().seen;
    assert_eq!(
        seen.as_slice(),
        &[Event::PressRelease { x: 50, y: 380 }, Event::Tick,]
    );
}
