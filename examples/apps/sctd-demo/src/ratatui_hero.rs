// SPDX-License-Identifier: MIT
//! SCTD-04 hybrid hero window: native rlvgl chrome around Ratatui content.

use alloc::{boxed::Box, string::String, vec::Vec};
use core::cell::RefCell;

use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::Rect as TuiRect,
    style::{Color as TuiColor, Style},
    widgets::Widget as TuiWidget,
};
use ratatui_rlvgl::{CellMetrics, RatatuiView, RlvglBackend};
use rlvgl_core::{
    bitmap_font::FONT_6X10,
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget as RlvglWidget},
};
use rlvgl_ui::draw_helpers::{draw_rounded_border, fill_rounded_rect};

use crate::philosophers::SeatState;

const WINDOW_BG: Color = Color(13, 19, 30, 255);
const TITLE_BG: Color = Color(29, 42, 61, 255);
const WINDOW_BORDER: Color = Color(82, 192, 224, 255);
const TITLE_COLOR: Color = Color(241, 247, 252, 255);
const BUTTON_BG: Color = Color(42, 58, 79, 255);
const BUTTON_BORDER: Color = Color(91, 119, 151, 255);
const BUTTON_TEXT: Color = Color(225, 235, 244, 255);
const BUTTON_ACCENT: Color = Color(31, 112, 146, 255);

/// Snapshot of one philosopher row rendered by Ratatui.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeroSeat {
    pub(crate) number: u8,
    pub(crate) state: SeatState,
    pub(crate) left_fork_owner: i64,
    pub(crate) right_fork_owner: i64,
    pub(crate) depart_pending: bool,
}

/// Presentation-only snapshot consumed by the hero terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeroSnapshot {
    pub(crate) seats: [HeroSeat; 5],
    pub(crate) auto: bool,
    pub(crate) paused: bool,
    pub(crate) speed: &'static str,
    pub(crate) events: Vec<String>,
}

/// Native rlvgl popup background and title bar.
pub(crate) struct HeroFrame {
    screen: Rect,
    popup: Rect,
    visible: bool,
}

impl HeroFrame {
    pub(crate) const fn new(screen: Rect, popup: Rect) -> Self {
        Self {
            screen,
            popup,
            visible: false,
        }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl RlvglWidget for HeroFrame {
    fn bounds(&self) -> Rect {
        self.screen
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        renderer.blend_rect(self.screen, Color(0, 0, 0, 210));
        fill_rounded_rect(renderer, self.popup, WINDOW_BG, 12);
        let title = Rect {
            x: self.popup.x,
            y: self.popup.y,
            width: self.popup.width,
            height: 40,
        };
        fill_rounded_rect(renderer, title, TITLE_BG, 12);
        renderer.fill_rect(
            Rect {
                x: title.x,
                y: title.y + 28,
                width: title.width,
                height: 12,
            },
            TITLE_BG,
        );
        draw_rounded_border(renderer, self.popup, WINDOW_BORDER, 2, 12);
        FONT_6X10.draw_str(
            renderer,
            self.popup.x + 16,
            self.popup.y + (title.height - FONT_6X10.scaled_height()) / 2,
            "Dining Philosophers - Ratatui on rlvgl",
            TITLE_COLOR,
        );
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

/// Native graphical button used by the launcher and modal controls.
pub(crate) struct HeroButton {
    bounds: Rect,
    hit_bounds: Rect,
    label: &'static str,
    accent: bool,
    activate_on_press: bool,
    visible: bool,
    on_tap: Option<Box<dyn FnMut()>>,
}

impl HeroButton {
    pub(crate) const fn new(bounds: Rect, label: &'static str, accent: bool) -> Self {
        Self {
            bounds,
            hit_bounds: bounds,
            label,
            accent,
            activate_on_press: false,
            visible: false,
            on_tap: None,
        }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    #[cfg(test)]
    pub(crate) const fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_on_tap(&mut self, callback: Box<dyn FnMut()>) {
        self.on_tap = Some(callback);
    }

    pub(crate) fn set_press_behavior(&mut self, hit_bounds: Rect) {
        self.hit_bounds = hit_bounds;
        self.activate_on_press = true;
    }
}

impl RlvglWidget for HeroButton {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if !self.visible {
            return;
        }
        fill_rounded_rect(
            renderer,
            self.bounds,
            if self.accent {
                BUTTON_ACCENT
            } else {
                BUTTON_BG
            },
            6,
        );
        draw_rounded_border(renderer, self.bounds, BUTTON_BORDER, 1, 6);
        let (x, y) = centered_bitmap_label(self.bounds, self.label);
        FONT_6X10.draw_str(renderer, x, y, self.label, BUTTON_TEXT);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        let point = match event {
            Event::PressRelease { x, y } => Some((*x, *y)),
            Event::PressDown { x, y } if self.activate_on_press => Some((*x, *y)),
            _ => None,
        };
        if let Some((x, y)) = point
            && x >= self.hit_bounds.x
            && x < self.hit_bounds.x + self.hit_bounds.width
            && y >= self.hit_bounds.y
            && y < self.hit_bounds.y + self.hit_bounds.height
        {
            if let Some(mut callback) = self.on_tap.take() {
                callback();
                self.on_tap = Some(callback);
            }
            return true;
        }
        false
    }
}

fn centered_bitmap_label(bounds: Rect, label: &str) -> (i32, i32) {
    let glyph_count = label.chars().count() as i32;
    let advance = FONT_6X10.scaled_width() + i32::from(FONT_6X10.scale);
    let text_width = if glyph_count == 0 {
        0
    } else {
        FONT_6X10.scaled_width() + (glyph_count - 1) * advance
    };
    (
        bounds.x + (bounds.width - text_width) / 2,
        bounds.y + (bounds.height - FONT_6X10.scaled_height()) / 2,
    )
}

/// Ratatui-owned content surface within the native popup.
pub(crate) struct HeroContent {
    bounds: Rect,
    terminal: RefCell<Terminal<RlvglBackend>>,
    view: RatatuiView,
    snapshot: Option<HeroSnapshot>,
    visible: bool,
}

impl HeroContent {
    pub(crate) fn new(bounds: Rect) -> Self {
        let metrics = CellMetrics::font_6x10();
        let cell_width = i32::from(metrics.width());
        let cell_height = i32::from(metrics.height());
        let columns = (bounds.width.max(cell_width) / cell_width).min(u16::MAX as i32) as u16;
        let rows = (bounds.height.max(cell_height) / cell_height).min(u16::MAX as i32) as u16;
        let (backend, surface) = RlvglBackend::new(columns, rows, metrics)
            .expect("hero bounds always produce a valid bitmap-font terminal grid");
        let terminal = Terminal::new(backend).expect("rlvgl backend construction is infallible");
        Self {
            bounds,
            terminal: RefCell::new(terminal),
            view: RatatuiView::new(bounds, surface),
            snapshot: None,
            visible: false,
        }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    #[cfg(test)]
    pub(crate) const fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn update(&mut self, snapshot: HeroSnapshot) {
        if self.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        self.terminal
            .borrow_mut()
            .draw(|frame| render_snapshot(frame, &snapshot))
            .expect("rendering to a retained rlvgl surface cannot fail");
        self.snapshot = Some(snapshot);
    }

    pub(crate) fn generation(&self) -> u64 {
        self.view.surface().generation()
    }
}

impl RlvglWidget for HeroContent {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.visible {
            self.view.draw(renderer);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

fn render_snapshot(frame: &mut ratatui::Frame<'_>, snapshot: &HeroSnapshot) {
    frame.render_widget(DiningTableScene { snapshot }, frame.area());
}

/// Ratatui-owned spatial rendering of the same five-seat dining table shown by
/// the native graphical view. All geometry is expressed in terminal cells;
/// rlvgl only rasterizes the resulting retained Ratatui buffer.
struct DiningTableScene<'a> {
    snapshot: &'a HeroSnapshot,
}

impl TuiWidget for DiningTableScene<'_> {
    fn render(self, area: TuiRect, buffer: &mut Buffer) {
        if area.width < 20 || area.height < 8 {
            return;
        }

        let background = Style::default()
            .fg(TuiColor::Gray)
            .bg(TuiColor::Rgb(13, 19, 30));
        buffer.set_style(area, background);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_symbol(" ");
            }
        }

        let center_x = area.x + area.width / 2;
        let center_y = area.y + area.height / 2;
        let radius_x = (area.width * 29 / 100).max(8);
        let radius_y = (area.height * 34 / 100).max(3);
        let table_style = Style::default()
            .fg(TuiColor::Rgb(214, 176, 118))
            .bg(TuiColor::Rgb(92, 57, 32));

        let rx2 = i64::from(radius_x) * i64::from(radius_x);
        let ry2 = i64::from(radius_y) * i64::from(radius_y);
        for y in center_y.saturating_sub(radius_y)..=center_y.saturating_add(radius_y) {
            for x in center_x.saturating_sub(radius_x)..=center_x.saturating_add(radius_x) {
                if x < area.left() || x >= area.right() || y < area.top() || y >= area.bottom() {
                    continue;
                }
                let dx = i64::from(x) - i64::from(center_x);
                let dy = i64::from(y) - i64::from(center_y);
                let distance = dx * dx * ry2 + dy * dy * rx2;
                if distance <= rx2 * ry2 {
                    buffer[(x, y)].set_symbol(" ").set_style(table_style);
                }
            }
        }

        let status = if self.snapshot.paused {
            "PAUSED"
        } else if self.snapshot.auto {
            "AUTO"
        } else {
            "MANUAL"
        };
        let center_label = "DINING TABLE";
        let label_x = center_x.saturating_sub(center_label.len() as u16 / 2);
        buffer.set_stringn(
            label_x,
            center_y,
            center_label,
            center_label.len(),
            table_style.fg(TuiColor::White),
        );
        let run_label = alloc::format!("{}  {}", status, self.snapshot.speed);
        let run_x = center_x.saturating_sub(run_label.len() as u16 / 2);
        buffer.set_stringn(
            run_x,
            center_y.saturating_add(1),
            &run_label,
            run_label.len(),
            table_style.fg(TuiColor::LightCyan),
        );

        let seat_positions = [(24u16, 10u16), (66, 10), (82, 48), (18, 48), (50, 82)];
        for (seat, &(x_percent, y_percent)) in self.snapshot.seats.iter().zip(seat_positions.iter())
        {
            let x = area.x + area.width.saturating_mul(x_percent) / 100;
            let y = area.y + area.height.saturating_mul(y_percent) / 100;
            paint_seat(buffer, area, x, y, seat);
        }

        let fork_positions = [(45u16, 22u16), (70, 37), (64, 69), (36, 69), (30, 37)];
        for (index, &(x_percent, y_percent)) in fork_positions.iter().enumerate() {
            let x = area.x + area.width.saturating_mul(x_percent) / 100;
            let y = area.y + area.height.saturating_mul(y_percent) / 100;
            // Seat N's left fork is fork N in the statechart datamodel.
            let owner = self.snapshot.seats[index].left_fork_owner;
            let label = if owner == 0 {
                alloc::format!("F{}", index + 1)
            } else {
                alloc::format!("F{}>P{}", index + 1, owner)
            };
            buffer.set_stringn(
                x,
                y,
                &label,
                label.len(),
                table_style.fg(if owner == 0 {
                    TuiColor::LightYellow
                } else {
                    TuiColor::LightMagenta
                }),
            );
        }
    }
}

fn paint_seat(buffer: &mut Buffer, area: TuiRect, center_x: u16, top: u16, seat: &HeroSeat) {
    const WIDTH: u16 = 9;
    const HEIGHT: u16 = 2;
    let left = center_x.saturating_sub(WIDTH / 2).max(area.left());
    let top = top.min(area.bottom().saturating_sub(HEIGHT));
    let right = left.saturating_add(WIDTH).min(area.right());
    let style = Style::default()
        .fg(TuiColor::Black)
        .bg(seat_color(seat.state));
    for y in top..top.saturating_add(HEIGHT).min(area.bottom()) {
        for x in left..right {
            buffer[(x, y)].set_symbol(" ").set_style(style);
        }
    }
    let seat_label = alloc::format!(
        "P{}{}",
        seat.number,
        if seat.depart_pending { "!" } else { "" }
    );
    buffer.set_stringn(left + 1, top, &seat_label, WIDTH as usize - 2, style);
    buffer.set_stringn(
        left + 1,
        top + 1,
        seat_state_label(seat.state),
        WIDTH as usize - 2,
        style,
    );
}

const fn seat_state_label(state: SeatState) -> &'static str {
    match state {
        SeatState::Empty => "EMPTY",
        SeatState::Thinking => "THINK",
        SeatState::Hungry => "HUNGRY",
        SeatState::Waiting => "WAIT",
        SeatState::Eating => "EATING",
    }
}

const fn seat_color(state: SeatState) -> TuiColor {
    match state {
        SeatState::Empty => TuiColor::DarkGray,
        SeatState::Thinking => TuiColor::LightBlue,
        SeatState::Hungry => TuiColor::LightYellow,
        SeatState::Waiting => TuiColor::LightRed,
        SeatState::Eating => TuiColor::LightGreen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> HeroSnapshot {
        HeroSnapshot {
            seats: core::array::from_fn(|index| HeroSeat {
                number: index as u8 + 1,
                state: if index == 0 {
                    SeatState::Eating
                } else {
                    SeatState::Empty
                },
                left_fork_owner: if index == 0 { 1 } else { 0 },
                right_fork_owner: 0,
                depart_pending: false,
            }),
            auto: true,
            paused: false,
            speed: "x1",
            events: alloc::vec![String::from("Arrive")],
        }
    }

    #[test]
    fn unchanged_snapshot_does_not_render_another_terminal_frame() {
        let mut content = HeroContent::new(Rect {
            x: 20,
            y: 50,
            width: 720,
            height: 340,
        });
        content.update(snapshot());
        let generation = content.generation();
        content.update(snapshot());
        assert_eq!(content.generation(), generation);
    }

    #[test]
    fn terminal_grid_uses_scaled_bitmap_font_metrics() {
        let content = HeroContent::new(Rect {
            x: 20,
            y: 54,
            width: 760,
            height: 358,
        });

        let size = content.terminal.borrow().size().unwrap();
        assert_eq!((size.width, size.height), (63, 17));
    }

    #[test]
    fn native_button_labels_use_scaled_font_centering() {
        let bounds = Rect {
            x: 588,
            y: 408,
            width: 132,
            height: 38,
        };

        assert_eq!(centered_bitmap_label(bounds, "Ratatui"), (606, 417));
        assert_eq!(centered_bitmap_label(bounds, "X"), (648, 417));
    }

    #[test]
    fn edge_close_accepts_press_without_waiting_for_release() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        let mut close = HeroButton::new(
            Rect {
                x: 732,
                y: 18,
                width: 48,
                height: 32,
            },
            "X",
            false,
        );
        close.set_visible(true);
        close.set_press_behavior(Rect {
            x: 700,
            y: 0,
            width: 100,
            height: 70,
        });
        let activations = Rc::new(Cell::new(0));
        let callback_activations = activations.clone();
        close.set_on_tap(Box::new(move || {
            callback_activations.set(callback_activations.get() + 1);
        }));

        assert!(close.handle_event(&Event::PressDown { x: 712, y: 8 }));
        assert_eq!(activations.get(), 1);
    }

    #[test]
    fn popup_is_a_spatial_ascii_dining_table_not_a_text_grid() {
        let mut content = HeroContent::new(Rect {
            x: 20,
            y: 50,
            width: 720,
            height: 340,
        });
        content.update(snapshot());
        let surface = content.view.surface();
        let size = surface.size();
        let mut symbols = String::new();
        let mut table_cells = 0usize;
        let mut eating_cells = 0usize;
        for y in 0..size.height {
            for x in 0..size.width {
                let cell = surface.cell(x, y).expect("cell within terminal grid");
                symbols.push_str(cell.symbol());
                if cell.bg == TuiColor::Rgb(92, 57, 32) {
                    table_cells += 1;
                }
                if cell.bg == TuiColor::LightGreen {
                    eating_cells += 1;
                }
            }
        }
        assert!(symbols.contains("DINING TABLE"));
        assert!(symbols.contains("P1"));
        assert!(symbols.contains("F1>P1"));
        assert!(symbols.chars().all(|character| character.is_ascii()));
        assert!(table_cells > 100, "table must dominate the popup content");
        assert!(eating_cells >= 12, "seat state must be a visible block");
    }
}
