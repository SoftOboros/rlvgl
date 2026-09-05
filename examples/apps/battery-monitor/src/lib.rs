//! Three-column Ratatui battery-monitor UI and portable polling contract.

#![cfg_attr(not(feature = "host-serial"), no_std)]

extern crate alloc;

use alloc::{format, rc::Rc, string::String};
use core::cell::RefCell;

use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect as TuiRect},
    style::{Color as TuiColor, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget as TuiWidget},
};
use ratatui_rlvgl::{CellMetrics, RatatuiTerminalFont, RatatuiView, RlvglBackend};
use rlvgl_core::{
    WidgetNode,
    application::{AppInfo, Application},
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget as RlvglWidget},
};

/// Physical display width of the initial ESP32 target.
pub const TARGET_WIDTH: u32 = 800;
/// Physical display height of the initial ESP32 target.
pub const TARGET_HEIGHT: u32 = 480;
/// The three addresses configured on the initial battery bank.
pub const DEFAULT_BATTERY_ADDRESSES: [u8; 3] = [0xF7, 0x04, 0x05];

/// Read-only telemetry used by the monitor screen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatteryTelemetry {
    /// Battery Modbus address.
    pub address: u8,
    /// Number of cell-voltage reports available from the BMS.
    pub cell_count: u16,
    /// The first four cell voltages in decivolts.
    pub cell_voltage_decivolts: [u16; 4],
    /// Number of cell-temperature reports available from the BMS.
    pub cell_temperature_count: u16,
    /// The first three cell temperatures in tenths of a degree Celsius.
    pub cell_temperature_decicelsius: [i16; 3],
    /// Module voltage in decivolts.
    pub module_voltage_decivolts: u16,
    /// Signed current in centiamps.
    pub current_centiamps: i16,
    /// Remaining capacity in milliamp-hours.
    pub remaining_capacity_milliamphours: u32,
    /// Total capacity in milliamp-hours.
    pub total_capacity_milliamphours: u32,
    /// Raw Status1 word.
    pub status1: u16,
    /// Raw Status2 word.
    pub status2: u16,
    /// Raw Status3 word.
    pub status3: u16,
    /// Raw charge/discharge-status word.
    pub charge_discharge_status: u16,
}

impl BatteryTelemetry {
    /// Return the state of charge as a percentage when total capacity is known.
    pub fn state_of_charge_percent(self) -> Option<u8> {
        (self.total_capacity_milliamphours != 0).then(|| {
            ((u64::from(self.remaining_capacity_milliamphours) * 100)
                / u64::from(self.total_capacity_milliamphours)) as u8
        })
    }
    /// Report whether charging is enabled by the BMS.
    pub fn charge_enabled(self) -> bool {
        self.charge_discharge_status & (1 << 7) != 0
    }
    /// Report whether discharging is enabled by the BMS.
    pub fn discharge_enabled(self) -> bool {
        self.charge_discharge_status & (1 << 6) != 0
    }
    /// Report whether the protection and warning bits are clear.
    pub fn alarms_clear(self) -> bool {
        self.status1 & !0x000e == 0 && self.status2 & 0x00ff == 0 && self.status3 == 0
    }

    /// Return the mean of the reported cell-temperature sensors.
    pub fn average_cell_temperature_decicelsius(self) -> Option<i16> {
        let count = usize::from(self.cell_temperature_count.min(3));
        (count != 0).then(|| {
            (self.cell_temperature_decicelsius[..count]
                .iter()
                .map(|value| i32::from(*value))
                .sum::<i32>()
                / count as i32) as i16
        })
    }
}

/// Result of asking a transport to service one battery address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PollResult {
    /// The transport is rate-limited and has no new result yet.
    Pending,
    /// Read-only telemetry was received and CRC-validated.
    Telemetry(BatteryTelemetry),
    /// The transport could not complete a read-only poll.
    Error(String),
}

/// Transport boundary used by the UI.
///
/// Implementations may return Pending until their poll interval elapses. They
/// must not issue configuration writes.
pub trait BatteryPoller {
    /// Service a poll for the supplied Modbus address.
    fn poll(&mut self, address: u8) -> PollResult;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DashboardSnapshot {
    readings: [Option<BatteryTelemetry>; 3],
    errors: [Option<String>; 3],
}

struct DashboardView {
    bounds: Rect,
    terminal: RefCell<Terminal<RlvglBackend>>,
    view: RatatuiView,
    snapshot: Option<DashboardSnapshot>,
}

impl DashboardView {
    fn new(bounds: Rect) -> Self {
        let metrics = CellMetrics::font_6x10();
        let cell_width = i32::from(metrics.width());
        let cell_height = i32::from(metrics.height());
        let columns = (bounds.width.max(cell_width) / cell_width).min(u16::MAX as i32) as u16;
        let rows = (bounds.height.max(cell_height) / cell_height).min(u16::MAX as i32) as u16;
        let (backend, surface) = RlvglBackend::new(columns, rows, metrics)
            .expect("battery-monitor bounds always produce a terminal grid");
        let view = RatatuiView::new(bounds, surface);
        view.set_font_family(RatatuiTerminalFont::Bitmap6x10);
        Self {
            bounds,
            terminal: RefCell::new(Terminal::new(backend).expect("rlvgl backend is infallible")),
            view,
            snapshot: None,
        }
    }

    fn update(&mut self, snapshot: DashboardSnapshot) {
        if self.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        self.terminal
            .borrow_mut()
            .draw(|frame| render_dashboard(frame, &snapshot))
            .expect("retained terminal rendering cannot fail");
        self.snapshot = Some(snapshot);
    }
}

impl RlvglWidget for DashboardView {
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn draw(&self, renderer: &mut dyn Renderer) {
        self.view.draw(renderer);
    }
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

/// Battery monitor application parameterized by its UART/RS-485 poller.
pub struct BatteryMonitorApp<P> {
    poller: P,
    addresses: [u8; 3],
    next_address: usize,
    readings: [Option<BatteryTelemetry>; 3],
    errors: [Option<String>; 3],
    dashboard: Option<Rc<RefCell<DashboardView>>>,
}

impl<P> BatteryMonitorApp<P> {
    /// Construct a monitor for the supplied addresses and polling transport.
    pub fn new(poller: P, addresses: [u8; 3]) -> Self {
        Self {
            poller,
            addresses,
            next_address: 0,
            readings: [None; 3],
            errors: [None, None, None],
            dashboard: None,
        }
    }
    fn refresh_dashboard(&mut self) {
        if let Some(dashboard) = &self.dashboard {
            dashboard.borrow_mut().update(DashboardSnapshot {
                readings: self.readings,
                errors: self.errors.clone(),
            });
        }
    }
}

impl<P: BatteryPoller> BatteryMonitorApp<P> {
    fn service_poll(&mut self) {
        let index = self.next_address;
        match self.poller.poll(self.addresses[index]) {
            PollResult::Pending => return,
            PollResult::Telemetry(reading) => {
                self.readings[index] = Some(reading);
                self.errors[index] = None;
            }
            PollResult::Error(error) => self.errors[index] = Some(error),
        }
        self.next_address = (index + 1) % self.addresses.len();
        self.refresh_dashboard();
    }
}

impl<P: BatteryPoller> Application for BatteryMonitorApp<P> {
    fn info(&self) -> AppInfo {
        AppInfo {
            name: "rlvgl-battery-monitor",
            version: "0.1.0",
            preferred_width: TARGET_WIDTH,
            preferred_height: TARGET_HEIGHT,
        }
    }

    fn build(&mut self, width: u32, height: u32) -> WidgetNode {
        let dashboard = Rc::new(RefCell::new(DashboardView::new(Rect {
            x: 0,
            y: 0,
            width: width as i32,
            height: height as i32,
        })));
        self.dashboard = Some(dashboard.clone());
        self.refresh_dashboard();
        WidgetNode::new(dashboard)
    }

    fn after_event(&mut self, _root: &Rc<RefCell<WidgetNode>>, _event: &Event) {}
    fn tick(&mut self, _root: &Rc<RefCell<WidgetNode>>) {
        self.service_poll();
    }
}

fn render_dashboard(frame: &mut ratatui::Frame<'_>, snapshot: &DashboardSnapshot) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(TuiColor::Rgb(9, 17, 30))),
        area,
    );
    let regions = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(18),
            Constraint::Length(2),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from("  BATTERY BUS MONITOR  /  RENOGY MODBUS RTU")).style(
            Style::default()
                .fg(TuiColor::LightCyan)
                .bg(TuiColor::Rgb(15, 43, 63)),
        ),
        regions[0],
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(regions[1]);
    for index in 0..3 {
        frame.render_widget(
            BatteryColumn {
                number: index + 1,
                address: DEFAULT_BATTERY_ADDRESSES[index],
                reading: snapshot.readings[index],
                error: snapshot.errors[index].as_deref(),
            },
            columns[index],
        );
    }
    let online = snapshot.readings.iter().flatten().count();
    let alerts = snapshot
        .readings
        .iter()
        .flatten()
        .filter(|reading| !reading.alarms_clear())
        .count();
    frame.render_widget(
        Paragraph::new(format!(
            "  9600 8N1  |  READ-ONLY  |  {online}/3 ONLINE  |  {alerts} ALERT(S)"
        ))
        .style(
            Style::default()
                .fg(TuiColor::Gray)
                .bg(TuiColor::Rgb(15, 43, 63)),
        ),
        regions[2],
    );
}

struct BatteryColumn<'a> {
    number: usize,
    address: u8,
    reading: Option<BatteryTelemetry>,
    error: Option<&'a str>,
}

impl TuiWidget for BatteryColumn<'_> {
    fn render(self, area: TuiRect, buffer: &mut Buffer) {
        let accent = match self.reading {
            Some(reading) if reading.alarms_clear() => TuiColor::Green,
            Some(_) => TuiColor::Yellow,
            None => TuiColor::DarkGray,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent))
            .title(format!(" Battery {}: {:02X} ", self.number, self.address))
            .style(Style::default().bg(TuiColor::Rgb(12, 25, 41)));
        let inner = block.inner(area);
        block.render(area, buffer);
        let (content, style) = match (self.reading, self.error) {
            (_, Some(error)) => (
                format!("STALE / LINK ERROR\n\n{error}"),
                Style::default().fg(TuiColor::Red),
            ),
            (Some(reading), _) => (
                format_reading(reading),
                Style::default().fg(TuiColor::White),
            ),
            (None, None) => (
                String::from("WAITING\n\nfirst poll pending"),
                Style::default().fg(TuiColor::Gray),
            ),
        };
        Paragraph::new(content).style(style).render(inner, buffer);
    }
}

fn format_reading(reading: BatteryTelemetry) -> String {
    let current_sign = if reading.current_centiamps < 0 {
        '-'
    } else {
        '+'
    };
    let state_of_charge = reading
        .state_of_charge_percent()
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| String::from("--"));
    let state_of_charge_bar = reading
        .state_of_charge_percent()
        .map(state_of_charge_bar)
        .unwrap_or_else(|| String::from("[............]"));
    let status = if reading.alarms_clear() {
        "CLEAR"
    } else {
        "CHECK"
    };
    let temperature = reading
        .average_cell_temperature_decicelsius()
        .map(format_temperature)
        .unwrap_or_else(|| String::from("--.- C"));
    let cells = format!(
        "Cells    {}/4\n{}\n{}\n{}\n{}",
        reading.cell_count,
        format_cell(1, reading.cell_count, reading.cell_voltage_decivolts[0]),
        format_cell(2, reading.cell_count, reading.cell_voltage_decivolts[1]),
        format_cell(3, reading.cell_count, reading.cell_voltage_decivolts[2]),
        format_cell(4, reading.cell_count, reading.cell_voltage_decivolts[3]),
    );
    format!(
        "Voltage  {:>4}.{:01} V\n\
         Current  {current_sign}{:>3}.{:02} A\n\
         Cell T   {temperature}\n\
         SOC      {state_of_charge}\n\
                  {state_of_charge_bar}\n\
         {cells}\n\
         Remain   {}.{:01} Ah\n\
         Total    {}.{:01} Ah\n\
         Charge   {}\n\
         Dischg   {}\n\
         Alarms   {status}\n\
         S1/S2    {:04X}/{:04X}\n\
         S3/Ctrl  {:04X}/{:04X}",
        reading.module_voltage_decivolts / 10,
        reading.module_voltage_decivolts % 10,
        reading.current_centiamps.unsigned_abs() / 100,
        reading.current_centiamps.unsigned_abs() % 100,
        reading.remaining_capacity_milliamphours / 1000,
        (reading.remaining_capacity_milliamphours % 1000) / 100,
        reading.total_capacity_milliamphours / 1000,
        (reading.total_capacity_milliamphours % 1000) / 100,
        if reading.charge_enabled() {
            "ON"
        } else {
            "OFF"
        },
        if reading.discharge_enabled() {
            "ON"
        } else {
            "OFF"
        },
        reading.status1,
        reading.status2,
        reading.status3,
        reading.charge_discharge_status,
    )
}

fn format_temperature(decicelsius: i16) -> String {
    let sign = if decicelsius < 0 { "-" } else { "" };
    let magnitude = decicelsius.unsigned_abs();
    format!("{sign}{}.{:01} C", magnitude / 10, magnitude % 10)
}

fn format_cell(index: u8, cell_count: u16, voltage_decivolts: u16) -> String {
    if u16::from(index) > cell_count {
        return format!("C{index}  --.- [........]");
    }
    format!(
        "C{index}  {}.{} {}",
        voltage_decivolts / 10,
        voltage_decivolts % 10,
        cell_bar(voltage_decivolts),
    )
}

fn cell_bar(voltage_decivolts: u16) -> String {
    const BAR_WIDTH: usize = 8;
    let filled = (((i32::from(voltage_decivolts) - 30) * BAR_WIDTH as i32) / 6)
        .clamp(0, BAR_WIDTH as i32) as usize;
    format!("[{}{}]", "=".repeat(filled), ".".repeat(BAR_WIDTH - filled))
}

fn state_of_charge_bar(percent: u8) -> String {
    const BAR_WIDTH: usize = 12;
    let filled = (usize::from(percent).min(100) * BAR_WIDTH) / 100;
    format!("[{}{}]", "=".repeat(filled), ".".repeat(BAR_WIDTH - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_decodes_status_bits_and_capacity() {
        let reading = BatteryTelemetry {
            cell_temperature_count: 3,
            cell_temperature_decicelsius: [210, 212, 213],
            remaining_capacity_milliamphours: 70_493,
            total_capacity_milliamphours: 99_996,
            charge_discharge_status: 0x00c0,
            ..BatteryTelemetry::default()
        };
        assert_eq!(reading.state_of_charge_percent(), Some(70));
        assert!(reading.charge_enabled());
        assert!(reading.discharge_enabled());
        assert!(reading.alarms_clear());
        assert_eq!(reading.average_cell_temperature_decicelsius(), Some(211));
    }

    #[test]
    fn dashboard_reading_keeps_capacity_precision() {
        let text = format_reading(BatteryTelemetry {
            cell_count: 4,
            cell_voltage_decivolts: [33; 4],
            cell_temperature_count: 3,
            cell_temperature_decicelsius: [210; 3],
            remaining_capacity_milliamphours: 70_493,
            total_capacity_milliamphours: 99_996,
            ..BatteryTelemetry::default()
        });
        assert!(text.contains("Remain   70.4 Ah"));
        assert!(text.contains("Total    99.9 Ah"));
        assert!(text.contains("[========....]"));
        assert!(text.contains("C1  3.3 [====....]"));
        assert!(text.contains("Cell T   21.0 C"));
    }

    #[test]
    fn cell_bar_centers_a_nominal_lfp_cell() {
        assert_eq!(cell_bar(33), "[====....]");
        assert_eq!(format_cell(4, 3, 33), "C4  --.- [........]");
    }

    #[test]
    fn state_of_charge_bar_scales_to_percentage() {
        assert_eq!(state_of_charge_bar(70), "[========....]");
        assert_eq!(state_of_charge_bar(100), "[============]");
    }

    #[test]
    fn temperature_formatting_preserves_sign_and_precision() {
        assert_eq!(format_temperature(-15), "-1.5 C");
    }
}
