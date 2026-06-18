//! LPAR-16 pixel-golden conformance fixtures for LPAR-14 data-rich widgets (LPAR-16 §6).
//!
//! Each test is a §5.B kind-3 pixel golden:
//!   1. Render into a pixel buffer.
//!   2. Render again into a fresh buffer; assert byte-identical (determinism).
//!   3. Assert at least one concrete expected pixel value (anti-"stably-wrong" anchor).
//!   4. Assert the render is non-trivial (buffer not entirely initial fill).
//!
// clippy: 0 * N and 1 * N appear as deliberate symmetrical 2D index computations.
#![allow(clippy::erasing_op, clippy::identity_op)]

use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect, Widget};
use rlvgl_platform::display::{BufferDisplay, DisplayDriver};

// ---------------------------------------------------------------------------
// Shared renderer wrapper (mirrors golden_slider.rs / golden_button.rs house style)
// ---------------------------------------------------------------------------

struct DisplayRenderer<'a> {
    display: &'a mut BufferDisplay,
}

impl Renderer for DisplayRenderer<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let colors = vec![color; (rect.width * rect.height) as usize];
        self.display.flush(rect, &colors);
    }

    fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
}

// ---------------------------------------------------------------------------
// Helper: render widget twice, return both buffers.
// ---------------------------------------------------------------------------

fn render_twice<W: Widget>(w: &W, width: usize, height: usize) -> (Vec<Color>, Vec<Color>) {
    let mut d1 = BufferDisplay::new(width, height);
    w.draw(&mut DisplayRenderer { display: &mut d1 });

    let mut d2 = BufferDisplay::new(width, height);
    w.draw(&mut DisplayRenderer { display: &mut d2 });

    (d1.buffer, d2.buffer)
}

// ---------------------------------------------------------------------------
// 1. Calendar — pixel golden
//
// Anchor: the selected day-cell background (selected_color = Color(100,160,230,255))
// is painted via fill_rect inside draw() for the selected day.
// January 2026: first weekday = Thursday (col 4 in 0=Sun scheme).
// Day 1 lands at flat_idx = 4 → row 0, col 4.
// ---------------------------------------------------------------------------

#[test]
fn calendar_golden() {
    use rlvgl_widgets::calendar::{Calendar, CalendarDate};

    // 7 columns × 30 px each = 210 px wide.
    // Layout: header_h ≈ 14, name_row_h ≈ 14 → grid_y ≈ 28.
    // cell_h = (height - 28) / 6 = (200 - 28) / 6 = 28 (approx).
    const W: usize = 210;
    const H: usize = 200;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut cal = Calendar::new(bounds);
    cal.set_displayed_month(2026, 1); // January 2026; first_weekday = Thursday = 4
    cal.set_today(CalendarDate {
        year: 2026,
        month: 1,
        day: 15,
    });
    // Select day 1 (flat_idx = 4, row 0, col 4).
    cal.set_selected(CalendarDate {
        year: 2026,
        month: 1,
        day: 1,
    });
    // Override style bg so we can anchor the background too.
    cal.style.bg_color = Color(20, 20, 20, 255);

    let (buf1, buf2) = render_twice(&cal, W, H);

    // Gate 2: determinism — two renders are byte-identical.
    assert_eq!(buf1, buf2, "calendar: non-deterministic render");

    // Gate 3: concrete anchor — top-left corner is the widget background.
    // draw_widget_bg fills bounds with bg_color (radius=0, alpha=255 → fill_rect).
    assert_eq!(
        buf1[0 * W + 0],
        Color(20, 20, 20, 255),
        "calendar: bg_color not painted at (0,0)"
    );

    // Anchor: selected_color (for day 1's cell) must appear somewhere in the buffer.
    // draw() calls fill_rect(cell_rect, selected_color) for the selected day's cell
    // before drawing the day-number text on top.  The cell background survives at
    // pixels the glyph bitmask does not cover.
    assert!(
        buf1.contains(&Color(100, 160, 230, 255)),
        "calendar: selected_color not found anywhere in buffer"
    );

    // Gate 4: non-trivial — not all pixels are the initial fill (zero).
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "calendar: buffer is empty"
    );
}

// ---------------------------------------------------------------------------
// 2. Chart — pixel golden
//
// Widget: Chart with ChartType::Bar, one series, point_count=4, all points
// at y_max (fills bars from top to bottom of the plot area).
// Anchor: the bar series color (Color(0,200,0,255)) appears in the buffer
// via fill_rect inside draw_bar_series.
// ---------------------------------------------------------------------------

#[test]
fn chart_golden() {
    use rlvgl_widgets::chart::{Chart, ChartAxis, ChartType};

    const W: usize = 100;
    const H: usize = 100;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut chart = Chart::new(bounds);
    chart.set_type(ChartType::Bar);
    chart.set_axis_range(ChartAxis::Primary, 0, 100);
    chart.set_point_count(4);
    // Override bg so it is distinct from bar color.
    chart.style.bg_color = Color(10, 10, 10, 255);
    chart.grid_color = Color(50, 50, 50, 180); // partially transparent; falls back to fill_rect
    let bar_color = Color(0, 200, 0, 255);
    let s = chart.add_series(bar_color, ChartAxis::Primary);
    // All 4 points at y_max → each bar extends from bottom to top of the plot.
    chart.set_points(s, &[100, 100, 100, 100]);

    let (buf1, buf2) = render_twice(&chart, W, H);

    // Gate 2: determinism.
    assert_eq!(buf1, buf2, "chart: non-deterministic render");

    // Gate 3a: bg_color painted at top-left.
    assert_eq!(
        buf1[0 * W + 0],
        Color(10, 10, 10, 255),
        "chart: bg_color not painted at (0,0)"
    );

    // Gate 3b: bar series color present somewhere in the buffer.
    // draw_bar_series calls fill_rect directly with bar_color.
    assert!(
        buf1.contains(&bar_color),
        "chart: bar series color not found in buffer"
    );

    // Gate 4: non-trivial.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "chart: buffer is empty"
    );
}

// ---------------------------------------------------------------------------
// 3. Span (Spangroup) — pixel golden
//
// Spangroup only paints a fill_rect for its background (draw_widget_bg).
// Glyph rendering goes through draw_text_shaped → blend_rect → fill_rect
// so glyph pixels also land in the buffer.  We anchor on bg_color which is
// guaranteed by draw_widget_bg when alpha=255 and radius=0.
// ---------------------------------------------------------------------------

#[test]
fn span_golden() {
    use rlvgl_widgets::span::{SpanStyle, Spangroup};

    const W: usize = 120;
    const H: usize = 40;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut sg = Spangroup::new(bounds);
    sg.style.bg_color = Color(60, 80, 100, 255);
    sg.add_span("Hello", SpanStyle::with_color(Color(255, 255, 0, 255)));
    sg.add_span(" world", SpanStyle::with_color(Color(200, 200, 200, 255)));

    let (buf1, buf2) = render_twice(&sg, W, H);

    // Gate 2: determinism.
    assert_eq!(buf1, buf2, "span: non-deterministic render");

    // Gate 3: bg_color painted at top-left (draw_widget_bg, radius=0, alpha=255).
    assert_eq!(
        buf1[0 * W + 0],
        Color(60, 80, 100, 255),
        "span: bg_color not painted at (0,0)"
    );

    // Gate 4: non-trivial.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "span: buffer is empty"
    );
}

// ---------------------------------------------------------------------------
// 4. Table — pixel golden
//
// Table.draw_cells calls fill_rect for every cell background.
// Anchor: cell_bg_color (Color(240,240,240,255)) appears in the buffer,
// and the selected cell uses selected_color (Color(100,160,230,255)).
// ---------------------------------------------------------------------------

#[test]
fn table_golden() {
    use rlvgl_widgets::table::Table;

    const W: usize = 120;
    const H: usize = 80;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut table = Table::new(bounds);
    // Use distinct colors for clear anchoring.
    table.cell_bg_color = Color(240, 240, 240, 255);
    table.selected_color = Color(100, 160, 230, 255);
    table.style.bg_color = Color(30, 30, 30, 255);

    table.set_row_count(2);
    table.set_column_count(2);
    table.set_cell_value(0, 0, "A");
    table.set_cell_value(0, 1, "B");
    table.set_cell_value(1, 0, "C");
    table.set_cell_value(1, 1, "D");
    // Select cell (0,0) so selected_color is painted there.
    table.set_selected_cell(0, 0);

    let (buf1, buf2) = render_twice(&table, W, H);

    // Gate 2: determinism.
    assert_eq!(buf1, buf2, "table: non-deterministic render");

    // Gate 3a: selected_color at (0,0) — draw_cells paints selected cell (0,0)
    // with selected_color on top of the widget bg; this is the first fill at that
    // position because cell (0,0) starts at (bounds.x, bounds.y) = (0,0).
    assert_eq!(
        buf1[0 * W + 0],
        Color(100, 160, 230, 255),
        "table: selected_color not painted at selected cell (0,0)"
    );

    // Gate 3b: cell_bg_color appears somewhere (unselected cells fill with it).
    assert!(
        buf1.contains(&Color(240, 240, 240, 255)),
        "table: cell_bg_color not found in buffer"
    );

    // Gate 4: non-trivial.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "table: buffer is empty"
    );
}

// ---------------------------------------------------------------------------
// 5. Textarea — pixel golden
//
// When active with text, draw_widget_bg paints bg_color and draw_caret
// paints a CARET_WIDTH=2 rectangle using style.border_color.
// Anchor: bg_color fill (draw_widget_bg) and caret pixel (border_color).
// ---------------------------------------------------------------------------

#[test]
fn textarea_golden() {
    use rlvgl_widgets::textarea::Textarea;

    const W: usize = 100;
    const H: usize = 40;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut ta = Textarea::new(bounds);
    ta.style.bg_color = Color(245, 245, 245, 255);
    // border_color is used for the caret rectangle.
    ta.style.border_color = Color(255, 0, 128, 255);
    ta.set_active(true);
    ta.set_text("Hi");

    let (buf1, buf2) = render_twice(&ta, W, H);

    // Gate 2: determinism.
    assert_eq!(buf1, buf2, "textarea: non-deterministic render");

    // Gate 3a: caret at (0,0) — EditCore.new("",…) sets caret=0; set_text("Hi")
    // clamps caret to min(0, 2)=0, so the caret is at col=0, x=0.
    // CARET_WIDTH=2, so pixels (0,0)..(1,line_height-1) = border_color.
    assert_eq!(
        buf1[0 * W + 0],
        Color(255, 0, 128, 255),
        "textarea: caret not painted at (0,0) with border_color"
    );

    // Gate 3b: bg_color appears at a pixel beyond CARET_WIDTH=2 (e.g. x=10).
    // draw_widget_bg fills the entire bounds first; only the caret (2px wide at x=0)
    // overwrites a narrow strip on the left.
    assert_eq!(
        buf1[0 * W + 10],
        Color(245, 245, 245, 255),
        "textarea: bg_color not found at x=10 (beyond caret)"
    );

    // Gate 4: non-trivial.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "textarea: buffer is empty"
    );
}

// ---------------------------------------------------------------------------
// 6. MessageBox — pixel golden
//
// MessageBox.draw paints:
//   • outer bg via draw_widget_bg (fill_rect with style.bg_color),
//   • button-matrix cells via ButtonMatrix.draw (fill_rect with button_color).
// Anchor: outer bg_color at (0,0) and the button_color in the footer area.
// ButtonMatrix default button_color = Color(200,200,200,255).
// ---------------------------------------------------------------------------

#[test]
fn message_box_golden() {
    use rlvgl_widgets::message_box::MessageBox;

    const W: usize = 200;
    const H: usize = 150;

    let bounds = Rect {
        x: 0,
        y: 0,
        width: W as i32,
        height: H as i32,
    };

    let mut mb = MessageBox::new(bounds, "Alert", "Something happened", &["OK", "Cancel"]);
    mb.style.bg_color = Color(240, 235, 220, 255);

    let (buf1, buf2) = render_twice(&mb, W, H);

    // Gate 2: determinism.
    assert_eq!(buf1, buf2, "message_box: non-deterministic render");

    // Gate 3a: bg_color painted at top-left.
    assert_eq!(
        buf1[0 * W + 0],
        Color(240, 235, 220, 255),
        "message_box: bg_color not painted at (0,0)"
    );

    // Gate 3b: button-matrix button_color appears in the footer region.
    // Footer top = H - 40 = 110; default button_color = Color(200,200,200,255).
    let footer_top = H - 40;
    let footer_pixels = &buf1[footer_top * W..];
    assert!(
        footer_pixels.contains(&Color(200, 200, 200, 255)),
        "message_box: button_color not found in footer area"
    );

    // Gate 4: non-trivial.
    assert!(
        buf1.iter().any(|&c| c != Color(0, 0, 0, 0)),
        "message_box: buffer is empty"
    );
}
