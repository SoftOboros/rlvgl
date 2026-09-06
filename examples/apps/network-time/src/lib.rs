//! Portable low-rate monochrome network-time application.
//!
//! The application owns only view state and rendering. Board runners own the
//! display flush cadence, I2C topology, network stack, persistent storage, and
//! sensor drivers.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::rc::Rc;
use core::{cell::RefCell, fmt::Write};

use heapless::String;
use rlvgl_core::{
    WidgetNode,
    application::{AppInfo, Application},
    event::Event,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
use rlvgl_network::{ConnectionState, unix_seconds_to_utc};

/// Preferred width of the initial monochrome clock face.
pub const TARGET_WIDTH: u32 = 128;
/// Preferred height of the initial monochrome clock face.
pub const TARGET_HEIGHT: u32 = 64;

const WHITE: Color = Color(255, 255, 255, 255);
const BLACK: Color = Color(0, 0, 0, 255);

/// A synchronized or holdover clock reading shown by the application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockReading {
    /// Whole seconds since 1970-01-01 00:00:00 UTC.
    pub unix_seconds: u64,
    /// Whether the network link is currently usable.
    pub connected: bool,
    /// Whole seconds since the most recent accepted network-time sample.
    pub sync_age_seconds: u64,
    /// Optional temperature in hundredths of a degree Celsius.
    pub temperature_centidegrees: Option<i16>,
}

/// One complete view state for the network-time screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayState {
    /// No stored or seed credentials were available.
    AwaitingCredentials,
    /// Persistent configuration could not be read or initialized.
    StorageFailure,
    /// A supplied provisioning seed failed credential validation.
    ConfigurationFailure,
    /// Radio-neutral connection progress.
    Connection {
        /// Current common connection lifecycle state.
        state: ConnectionState,
        /// Seconds elapsed in the current state.
        elapsed_seconds: u32,
    },
    /// DHCP completed and the SNTP exchange is starting.
    Synchronizing,
    /// An SNTP exchange failed and will be retried after the shown delay.
    SyncRetry {
        /// Delay before the next SNTP attempt.
        delay_seconds: u32,
    },
    /// Normal synchronized or holdover clock face.
    Clock(ClockReading),
}

/// Cloneable update handle retained by a board or simulator runtime.
#[derive(Clone)]
pub struct NetworkTimeModel {
    state: Rc<RefCell<DisplayState>>,
}

impl NetworkTimeModel {
    /// Replace the complete screen state.
    pub fn set(&self, state: DisplayState) {
        *self.state.borrow_mut() = state;
    }

    /// Return a snapshot of the current screen state.
    pub fn get(&self) -> DisplayState {
        *self.state.borrow()
    }
}

/// Portable network-time application.
pub struct NetworkTimeApp {
    model: NetworkTimeModel,
}

impl NetworkTimeApp {
    /// Construct an application initially waiting for credentials.
    pub fn new() -> Self {
        Self {
            model: NetworkTimeModel {
                state: Rc::new(RefCell::new(DisplayState::AwaitingCredentials)),
            },
        }
    }

    /// Return a cloneable model handle for the platform runner.
    pub fn model(&self) -> NetworkTimeModel {
        self.model.clone()
    }
}

impl Default for NetworkTimeApp {
    fn default() -> Self {
        Self::new()
    }
}

impl Application for NetworkTimeApp {
    fn info(&self) -> AppInfo {
        AppInfo {
            name: "rlvgl-network-time",
            version: env!("CARGO_PKG_VERSION"),
            preferred_width: TARGET_WIDTH,
            preferred_height: TARGET_HEIGHT,
        }
    }

    fn build(&mut self, width: u32, height: u32) -> WidgetNode {
        WidgetNode::new(Rc::new(RefCell::new(NetworkTimeView {
            bounds: Rect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
            model: self.model.clone(),
        })))
        .with_tag("network-time")
    }

    fn after_event(&mut self, _root: &Rc<RefCell<WidgetNode>>, _event: &Event) {}
}

struct NetworkTimeView {
    bounds: Rect,
    model: NetworkTimeModel,
}

impl Widget for NetworkTimeView {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        match self.model.get() {
            DisplayState::AwaitingCredentials => draw_lines(
                renderer,
                self.bounds,
                [
                    "NETWORK TIME",
                    "Wi-Fi not set",
                    "seed once with",
                    "RLVGL_WIFI_SSID",
                    "+ WIFI_PASSWORD",
                ],
            ),
            DisplayState::StorageFailure => draw_lines(
                renderer,
                self.bounds,
                [
                    "NETWORK TIME",
                    "storage error",
                    "NVS unavailable",
                    "credentials safe",
                    "see serial log",
                ],
            ),
            DisplayState::ConfigurationFailure => draw_lines(
                renderer,
                self.bounds,
                [
                    "NETWORK TIME",
                    "invalid Wi-Fi seed",
                    "check SSID/password",
                    "lengths",
                    "see serial log",
                ],
            ),
            DisplayState::Connection {
                state,
                elapsed_seconds,
            } => draw_connection(renderer, self.bounds, state, elapsed_seconds),
            DisplayState::Synchronizing => draw_lines(
                renderer,
                self.bounds,
                [
                    "NETWORK TIME",
                    "SNTP sync",
                    "Cloudflare UTC",
                    "",
                    "1 Hz I2C mono",
                ],
            ),
            DisplayState::SyncRetry { delay_seconds } => {
                let mut delay = String::<22>::new();
                let _ = write!(delay, "retry in {delay_seconds}s");
                draw_lines(
                    renderer,
                    self.bounds,
                    [
                        "NETWORK TIME",
                        "SNTP retry",
                        "no valid reply",
                        &delay,
                        "1 Hz I2C mono",
                    ],
                );
            }
            DisplayState::Clock(reading) => draw_clock(renderer, self.bounds, reading),
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}

fn draw_connection(
    renderer: &mut dyn Renderer,
    bounds: Rect,
    state: ConnectionState,
    elapsed_seconds: u32,
) {
    let mut detail = String::<22>::new();
    let second = match state {
        ConnectionState::Unprovisioned => "Wi-Fi not set",
        ConnectionState::Stored => "credentials stored",
        ConnectionState::RadioStarting => "radio init",
        ConnectionState::Associating { attempt } => {
            let _ = write!(detail, "attempt {attempt} / {elapsed_seconds}s");
            "Wi-Fi connect"
        }
        ConnectionState::AcquiringAddress => {
            let _ = write!(detail, "waiting {elapsed_seconds}s");
            "DHCP address"
        }
        ConnectionState::GotIp => "network ready",
        ConnectionState::Failed => "Wi-Fi failed",
    };
    draw_lines(
        renderer,
        bounds,
        ["NETWORK TIME", second, &detail, "", "1 Hz I2C mono"],
    );
}

fn draw_clock(renderer: &mut dyn Renderer, bounds: Rect, reading: ClockReading) {
    let utc = unix_seconds_to_utc(reading.unix_seconds);
    let mut date = String::<16>::new();
    let mut clock = String::<16>::new();
    let mut temperature = String::<22>::new();
    let mut status = String::<22>::new();
    let _ = write!(date, "{:04}-{:02}-{:02}", utc.year, utc.month, utc.day);
    let _ = write!(
        clock,
        "{:02}:{:02}:{:02} UTC",
        utc.hour, utc.minute, utc.second
    );
    match reading.temperature_centidegrees {
        Some(value) => {
            let value = i32::from(value);
            let magnitude = value.unsigned_abs();
            let sign = if value < 0 { "-" } else { "" };
            let _ = write!(
                temperature,
                "TEMP {sign}{}.{:02} C",
                magnitude / 100,
                magnitude % 100
            );
        }
        None => {
            let _ = temperature.push_str("TEMP --.-- C");
        }
    }
    let link = if reading.connected { "SYNC" } else { "HOLD" };
    let _ = write!(status, "{link} age {:>5}s", reading.sync_age_seconds);
    draw_lines(
        renderer,
        bounds,
        ["NETWORK TIME (UTC)", &date, &clock, &temperature, &status],
    );
}

fn draw_lines(renderer: &mut dyn Renderer, bounds: Rect, lines: [&str; 5]) {
    renderer.fill_rect(bounds, BLACK);
    for (offset, line) in [10, 22, 34, 46, 58].into_iter().zip(lines) {
        renderer.draw_text((bounds.x + 2, bounds.y + offset), line, WHITE);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::{string::String as AllocString, vec::Vec};

    use super::*;

    #[derive(Default)]
    struct Capture {
        fills: Vec<(Rect, Color)>,
        text: Vec<((i32, i32), AllocString)>,
    }

    impl Renderer for Capture {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            self.fills.push((rect, color));
        }

        fn draw_text(&mut self, position: (i32, i32), text: &str, _color: Color) {
            self.text.push((position, text.into()));
        }
    }

    #[test]
    fn app_renders_the_clock_from_portable_model_state() {
        let mut app = NetworkTimeApp::new();
        let model = app.model();
        let root = app.build(TARGET_WIDTH, TARGET_HEIGHT);
        model.set(DisplayState::Clock(ClockReading {
            unix_seconds: 951_827_696,
            connected: true,
            sync_age_seconds: 17,
            temperature_centidegrees: Some(2_283),
        }));

        let mut capture = Capture::default();
        root.draw(&mut capture);

        let lines: Vec<_> = capture.text.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(
            lines,
            [
                "NETWORK TIME (UTC)",
                "2000-02-29",
                "12:34:56 UTC",
                "TEMP 22.83 C",
                "SYNC age    17s",
            ]
        );
        assert_eq!(
            capture.fills,
            [(
                Rect {
                    x: 0,
                    y: 0,
                    width: 128,
                    height: 64
                },
                BLACK
            )]
        );
    }

    #[test]
    fn app_renders_bounded_connection_progress() {
        let mut app = NetworkTimeApp::new();
        let model = app.model();
        let root = app.build(TARGET_WIDTH, TARGET_HEIGHT);
        model.set(DisplayState::Connection {
            state: ConnectionState::Associating { attempt: 3 },
            elapsed_seconds: 7,
        });

        let mut capture = Capture::default();
        root.draw(&mut capture);
        assert_eq!(capture.text[1].1, "Wi-Fi connect");
        assert_eq!(capture.text[2].1, "attempt 3 / 7s");
    }
}
