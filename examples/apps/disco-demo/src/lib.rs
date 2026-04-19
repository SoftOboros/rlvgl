// SPDX-License-Identifier: MIT
//! Shared 747-style demo controller used by simulator, UEFI, and STM32 hosts.
//!
//! The controller owns the widget tree, shared assets, layout state, and a
//! small platform command queue so non-MCU runtimes can reuse the same
//! interaction model without board-specific register writes in the UI layer.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

mod assets;
mod dashboard_panel;
mod hotspot;
mod icon_strip;
mod wing;

use alloc::{format, rc::Rc, string::String, vec, vec::Vec};
use core::cell::RefCell;

use dashboard_panel::DashboardPanel;
use hotspot::ActionHotspot;
use icon_strip::{IconSlot, IconStrip};
use rlvgl_core::{
    WidgetNode,
    bitmap_font::FONT_6X10,
    event::{Event, Key},
    style::StyleBuilder,
    widget::{Color, Rect},
};
use rlvgl_ui::{EventWindow, EventWindowBuilder};
use rlvgl_widgets::{container::Container, label::Label};
use wing::Wing;

/// Runtime capabilities that shape which portions of the disco demo are active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscoCapabilities {
    /// True when platform audio demos can run.
    pub audio: bool,
    /// True when the runtime can surface storage summaries or browsers.
    pub storage: bool,
    /// True when diagnostic widgets should expose platform probes.
    pub diagnostics: bool,
    /// True when animation/effect demos such as the star crawl are available.
    pub effects: bool,
    /// True when pointer or touch input is supported.
    pub pointer: bool,
    /// Platform description (e.g. "Zephyr v4.1.0", "bare-metal", "simulator").
    pub platform: &'static str,
}

impl DiscoCapabilities {
    /// Capability preset for simulator hosts.
    ///
    /// `effects: true` so the simulator can run the Star Wars crawl and
    /// other motion demos through the shared motion engine in
    /// `rlvgl_widgets::motion`. The simulator runtime is responsible for
    /// actually building and driving those effects when it sees the
    /// matching `StartEffect` / `StopEffect` commands.
    pub const fn simulator() -> Self {
        Self {
            audio: false,
            storage: true,
            diagnostics: true,
            effects: true,
            pointer: true,
            platform: "simulator",
        }
    }

    /// Capability preset for the STM32H747I-DISCO hardware runtime.
    pub const fn stm32h747i_disco() -> Self {
        Self {
            audio: true,
            storage: true,
            diagnostics: true,
            effects: true,
            pointer: true,
            platform: "STM32H747I-DISCO bare-metal",
        }
    }

    /// Capability preset for the first AArch64 UEFI milestone.
    pub const fn uefi() -> Self {
        Self {
            audio: false,
            storage: false,
            diagnostics: true,
            effects: false,
            pointer: false,
            platform: "UEFI",
        }
    }

    /// Capability preset for the Zephyr RTOS runtime.
    pub const fn zephyr() -> Self {
        Self {
            audio: false,
            storage: true,
            diagnostics: true,
            effects: true,
            pointer: true,
            platform: "Zephyr RTOS",
        }
    }
}

impl Default for DiscoCapabilities {
    fn default() -> Self {
        Self::simulator()
    }
}

/// Effect hooks requested by the shared demo controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoEffect {
    /// Audio-scope or audio-reactive visualizations.
    AudioScope,
    /// The Star Wars style crawl used by the STM32 demo.
    StarCrawl,
}

/// Commands emitted by the shared demo controller for a runtime adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoCommand {
    /// Request a backlight change using an abstract `0..=100` level.
    SetBacklight(u8),
    /// Request that the runtime populate or refresh storage details.
    LoadStorageSummary,
    /// Request that a runtime-specific visual effect should start.
    StartEffect(DiscoEffect),
    /// Request that a runtime-specific visual effect should stop.
    StopEffect(DiscoEffect),
    /// Inform the runtime that the controller wants a status line surfaced.
    ShowStatus(String),
    /// Explicitly record that an action was intentionally ignored.
    NoOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WingKind {
    Settings,
    Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusState {
    Main(usize),
    Wing(WingKind, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainSlot {
    Settings = 0,
    Files = 1,
    Info = 2,
}

impl MainSlot {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Files,
            2 => Self::Info,
            _ => Self::Settings,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Settings => "Settings",
            Self::Files => "Storage",
            Self::Info => "Info",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSlot {
    Audio = 0,
    Camera = 1,
    Display = 2,
    Locale = 3,
    Backlight = 4,
    About = 5,
}

impl SettingsSlot {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Camera,
            2 => Self::Display,
            3 => Self::Locale,
            4 => Self::Backlight,
            5 => Self::About,
            _ => Self::Audio,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InfoSlot {
    Diagnostics = 0,
    LiveStats = 1,
    StarCrawl = 2,
    AudioScope = 3,
}

const STRIP_ICON_SIZE: i32 = 60;
const STRIP_MARGIN_TOP: i32 = 17;
const STRIP_GAP: i32 = 10;
const STRIP_X_OFFSET: i32 = 70;
const WING_X: i32 = 10;
const WING_ICON_SIZE: i32 = 60;
const WING_MARGIN_TOP: i32 = 17;
const WING_GAP: i32 = 10;

impl InfoSlot {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::LiveStats,
            2 => Self::StarCrawl,
            3 => Self::AudioScope,
            _ => Self::Diagnostics,
        }
    }
}

fn strip_slot_bounds(display_width: i32, index: usize) -> Rect {
    let step = STRIP_ICON_SIZE + STRIP_GAP;
    let top = if index == 0 {
        0
    } else {
        STRIP_MARGIN_TOP + index as i32 * step - STRIP_GAP / 2
    };
    let bottom = if index == icon_strip::SLOT_COUNT - 1 {
        STRIP_MARGIN_TOP + icon_strip::SLOT_COUNT as i32 * step
    } else {
        STRIP_MARGIN_TOP + (index as i32 + 1) * step - STRIP_GAP / 2
    };
    Rect {
        x: display_width - STRIP_X_OFFSET,
        y: top,
        width: STRIP_ICON_SIZE,
        height: bottom - top,
    }
}

fn wing_slot_bounds(index: usize, slot_count: usize) -> Rect {
    let step = WING_ICON_SIZE + WING_GAP;
    let top = if index == 0 {
        0
    } else {
        WING_MARGIN_TOP + index as i32 * step - WING_GAP / 2
    };
    let bottom = if index + 1 == slot_count {
        WING_MARGIN_TOP + slot_count as i32 * step
    } else {
        WING_MARGIN_TOP + (index as i32 + 1) * step - WING_GAP / 2
    };
    Rect {
        x: WING_X,
        y: top,
        width: WING_ICON_SIZE,
        height: bottom - top,
    }
}

struct ControllerState {
    capabilities: DiscoCapabilities,
    commands: Vec<DiscoCommand>,
    dashboard: Rc<RefCell<DashboardPanel>>,
    subtitle: Rc<RefCell<Label>>,
    footer: Rc<RefCell<Label>>,
    event_window: Rc<RefCell<EventWindow>>,
    icon_strip: Rc<RefCell<IconStrip>>,
    settings_wing: Rc<RefCell<Wing>>,
    info_wing: Rc<RefCell<Wing>>,
    focus: FocusState,
    tick_count: u64,
    backlight: u8,
    focus_dirty: bool,
    /// Info page currently rendered on the dashboard, if any.
    ///
    /// When set, the `Tick` handler re-renders the dashboard lines so
    /// live data (uptime ticker, FPS, heap, etc.) updates at frame
    /// rate instead of freezing at activation time.
    active_info: Option<InfoSlot>,
}

impl ControllerState {
    #[allow(clippy::too_many_arguments)]
    fn new(
        capabilities: DiscoCapabilities,
        dashboard: Rc<RefCell<DashboardPanel>>,
        subtitle: Rc<RefCell<Label>>,
        footer: Rc<RefCell<Label>>,
        event_window: Rc<RefCell<EventWindow>>,
        icon_strip: Rc<RefCell<IconStrip>>,
        settings_wing: Rc<RefCell<Wing>>,
        info_wing: Rc<RefCell<Wing>>,
    ) -> Self {
        let mut this = Self {
            capabilities,
            commands: Vec::new(),
            dashboard,
            subtitle,
            footer,
            event_window,
            icon_strip,
            settings_wing,
            info_wing,
            focus: FocusState::Main(0),
            tick_count: 0,
            backlight: 75,
            focus_dirty: false,
            active_info: None,
        };
        this.sync_focus_highlights();
        this
    }

    fn set_subtitle(&mut self, text: impl Into<String>) {
        self.subtitle.borrow_mut().set_text(text);
    }

    fn set_footer(&mut self, text: impl Into<String>) {
        self.footer.borrow_mut().set_text(text);
    }

    fn queue(&mut self, command: DiscoCommand) {
        self.commands.push(command);
    }

    fn push_status(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.set_footer(text.clone());
        self.event_window.borrow_mut().push_event(text.clone());
        self.queue(DiscoCommand::ShowStatus(text));
    }

    fn show_about(&mut self) {
        self.active_info = None;
        self.dashboard.borrow_mut().show();
        self.dashboard.borrow_mut().set_title("About");
        self.dashboard
            .borrow_mut()
            .set_caption("rlvgl demo application");
        self.dashboard
            .borrow_mut()
            .set_accent(Color(0x58, 0xB3, 0xF5, 0xFF));
        self.dashboard.borrow_mut().set_lines(vec![
            format!("Platform: {}", self.capabilities.platform),
            format!(
                "Audio: {}  Storage: {}  Effects: {}",
                if self.capabilities.audio { "yes" } else { "no" },
                if self.capabilities.storage {
                    "yes"
                } else {
                    "no"
                },
                if self.capabilities.effects {
                    "yes"
                } else {
                    "no"
                },
            ),
            format!(
                "Pointer: {}",
                if self.capabilities.pointer {
                    "yes"
                } else {
                    "no"
                },
            ),
            format!(
                "Diagnostics: {}",
                if self.capabilities.diagnostics {
                    "yes"
                } else {
                    "no"
                },
            ),
            format!("Backlight: {}%", self.backlight),
        ]);
        self.push_status("About");
    }

    fn show_home(&mut self) {
        self.active_info = None;
        self.dashboard.borrow_mut().show();
        self.dashboard.borrow_mut().set_title("About");
        self.dashboard
            .borrow_mut()
            .set_caption("rlvgl demo application");
        self.dashboard
            .borrow_mut()
            .set_accent(Color(0x58, 0xB3, 0xF5, 0xFF));
        self.dashboard.borrow_mut().set_lines(vec![
            "Use the right strip to open settings, storage, and info wings.".into(),
            "Arrow keys move focus, Enter activates, Escape closes a wing.".into(),
            format!(
                "Pointer input: {}  Storage: {}  Effects: {}",
                if self.capabilities.pointer {
                    "yes"
                } else {
                    "no"
                },
                if self.capabilities.storage {
                    "yes"
                } else {
                    "no"
                },
                if self.capabilities.effects {
                    "yes"
                } else {
                    "no"
                },
            ),
            format!(
                "Audio: {}  Diagnostics: {}  Backlight: {}%",
                if self.capabilities.audio { "yes" } else { "no" },
                if self.capabilities.diagnostics {
                    "yes"
                } else {
                    "no"
                },
                self.backlight,
            ),
        ]);
    }

    fn show_storage(&mut self) {
        self.dashboard.borrow_mut().show();
        self.dashboard.borrow_mut().set_title("Storage Browser");
        self.dashboard
            .borrow_mut()
            .set_caption("Shared file-browser placeholder");
        self.dashboard
            .borrow_mut()
            .set_accent(Color(0x7A, 0xD6, 0x8A, 0xFF));
        let mut lines = vec![
            "The shared controller reuses the 747 icon flow and layout.".into(),
            "Runtime adapters can replace these lines with real media summaries.".into(),
        ];
        if self.capabilities.storage {
            lines.push("Request queued: refresh storage summary".into());
            lines.push("Mock sources: onboard flash, SD card, host assets".into());
        } else {
            lines.push("Storage is disabled on this platform.".into());
        }
        self.dashboard.borrow_mut().set_lines(lines);
    }

    fn show_info(&mut self, title: &str, caption: &str, accent: Color, lines: Vec<String>) {
        self.dashboard.borrow_mut().show();
        self.dashboard.borrow_mut().set_title(title);
        self.dashboard.borrow_mut().set_caption(caption);
        self.dashboard.borrow_mut().set_accent(accent);
        self.dashboard.borrow_mut().set_lines(lines);
    }

    fn close_wings(&mut self) {
        self.active_info = None;
        self.dashboard.borrow_mut().hide();
        self.settings_wing.borrow_mut().close();
        self.info_wing.borrow_mut().close();
        let focus_index = match self.focus {
            FocusState::Main(index) | FocusState::Wing(_, index) => index,
        };
        self.focus = FocusState::Main(focus_index.min(2));
        self.refresh_focus_hint();
        self.sync_focus_highlights();
    }

    fn open_settings(&mut self) {
        self.info_wing.borrow_mut().close();
        self.settings_wing.borrow_mut().toggle_visible();
        let next_focus = if self.settings_wing.borrow().is_visible() {
            FocusState::Wing(WingKind::Settings, 0)
        } else {
            FocusState::Main(MainSlot::Settings as usize)
        };
        self.focus = next_focus;
        self.refresh_focus_hint();
        self.sync_focus_highlights();
    }

    fn open_info(&mut self) {
        self.settings_wing.borrow_mut().close();
        self.info_wing.borrow_mut().toggle_visible();
        let next_focus = if self.info_wing.borrow().is_visible() {
            FocusState::Wing(WingKind::Info, 0)
        } else {
            FocusState::Main(MainSlot::Info as usize)
        };
        self.focus = next_focus;
        self.refresh_focus_hint();
        self.sync_focus_highlights();
    }

    fn refresh_focus_hint(&mut self) {
        let text = match self.focus {
            FocusState::Main(index) => {
                format!("Focus: main {}", MainSlot::from_index(index).label())
            }
            FocusState::Wing(WingKind::Settings, index) => {
                format!("Focus: settings wing item {}", index + 1)
            }
            FocusState::Wing(WingKind::Info, index) => {
                format!("Focus: info wing item {}", index + 1)
            }
        };
        self.set_subtitle(text);
    }

    fn sync_focus_highlights(&mut self) {
        let (strip_slot, settings_slot, info_slot) = match self.focus {
            FocusState::Main(index) => (Some(index), None, None),
            FocusState::Wing(WingKind::Settings, index) => (None, Some(index), None),
            FocusState::Wing(WingKind::Info, index) => (None, None, Some(index)),
        };
        let strip = self.icon_strip.try_borrow_mut();
        let settings = self.settings_wing.try_borrow_mut();
        let info = self.info_wing.try_borrow_mut();
        if let (Ok(mut strip), Ok(mut settings), Ok(mut info)) = (strip, settings, info) {
            strip.set_focused_slot(strip_slot);
            settings.set_focused_slot(settings_slot);
            info.set_focused_slot(info_slot);
            self.focus_dirty = false;
        } else {
            self.focus_dirty = true;
        }
    }

    fn cycle_main_focus(&mut self, delta: i32) {
        let current = match self.focus {
            FocusState::Main(index) => index as i32,
            FocusState::Wing(_, _) => return,
        };
        let next = (current + delta).rem_euclid(3) as usize;
        self.focus = FocusState::Main(next);
        self.refresh_focus_hint();
        self.sync_focus_highlights();
    }

    fn cycle_wing_focus(&mut self, delta: i32) {
        self.focus = match self.focus {
            FocusState::Wing(WingKind::Settings, index) => FocusState::Wing(
                WingKind::Settings,
                (index as i32 + delta).rem_euclid(6) as usize,
            ),
            FocusState::Wing(WingKind::Info, index) => FocusState::Wing(
                WingKind::Info,
                (index as i32 + delta).rem_euclid(4) as usize,
            ),
            other => other,
        };
        self.refresh_focus_hint();
        self.sync_focus_highlights();
    }

    fn activate_main(&mut self, slot: MainSlot) {
        self.focus = FocusState::Main(slot as usize);
        self.sync_focus_highlights();
        match slot {
            MainSlot::Settings => self.open_settings(),
            MainSlot::Files => {
                self.close_wings();
                if self.capabilities.storage {
                    self.queue(DiscoCommand::LoadStorageSummary);
                    self.push_status("Storage");
                } else {
                    self.push_status("Storage: not available");
                }
            }
            MainSlot::Info => self.open_info(),
        }
    }

    fn activate_settings(&mut self, slot: SettingsSlot) {
        self.active_info = None;
        self.focus = FocusState::Wing(WingKind::Settings, slot as usize);
        self.sync_focus_highlights();
        match slot {
            SettingsSlot::Audio => {
                if self.capabilities.audio {
                    self.push_status("Queued audio scope effect");
                    self.queue(DiscoCommand::StartEffect(DiscoEffect::AudioScope));
                } else {
                    self.push_status("Audio scope is unavailable on this platform");
                    self.queue(DiscoCommand::NoOp);
                }
            }
            SettingsSlot::Camera => {
                self.push_status("Camera: not available");
            }
            SettingsSlot::Display => {
                self.push_status("Display: platform-managed");
            }
            SettingsSlot::Locale => {
                self.push_status(format!(
                    "Pointer: {} | Diag: {}",
                    if self.capabilities.pointer {
                        "on"
                    } else {
                        "off"
                    },
                    if self.capabilities.diagnostics {
                        "on"
                    } else {
                        "off"
                    },
                ));
            }
            SettingsSlot::Backlight => {
                self.backlight = match self.backlight {
                    100 => 25,
                    75 => 100,
                    50 => 75,
                    25 => 50,
                    _ => 75,
                };
                self.push_status(format!("Backlight {}%", self.backlight));
                self.queue(DiscoCommand::SetBacklight(self.backlight));
            }
            SettingsSlot::About => {
                self.show_about();
            }
        }
    }

    fn activate_info(&mut self, slot: InfoSlot) {
        self.focus = FocusState::Wing(WingKind::Info, slot as usize);
        self.sync_focus_highlights();
        match slot {
            InfoSlot::Diagnostics => {
                self.active_info = Some(slot);
                self.render_info_page(slot);
                self.push_status("Diagnostics page opened");
            }
            InfoSlot::LiveStats => {
                self.active_info = Some(slot);
                self.render_info_page(slot);
                self.push_status("Live stats panel refreshed");
            }
            InfoSlot::StarCrawl => {
                self.active_info = None;
                if self.capabilities.effects {
                    self.push_status("Queued star crawl effect");
                    self.queue(DiscoCommand::StartEffect(DiscoEffect::StarCrawl));
                } else {
                    self.push_status("Star crawl is unavailable on this platform");
                    self.queue(DiscoCommand::NoOp);
                }
            }
            InfoSlot::AudioScope => {
                self.active_info = None;
                if self.capabilities.audio {
                    self.push_status("Queued audio scope effect");
                    self.queue(DiscoCommand::StartEffect(DiscoEffect::AudioScope));
                } else {
                    self.push_status("Audio scope is unavailable on this platform");
                    self.queue(DiscoCommand::NoOp);
                }
            }
        }
    }

    /// Render the content for an info page using the current controller
    /// state. Called on activation and then every `Event::Tick` while the
    /// page is displayed, so live counters (uptime, FPS, heap) update at
    /// frame rate instead of freezing at activation time.
    fn render_info_page(&mut self, slot: InfoSlot) {
        match slot {
            InfoSlot::Diagnostics => {
                let lines = self.mock_diagnostics_lines();
                self.show_info(
                    "Diagnostics",
                    "STM32H747XIH6 — simulated telemetry",
                    Color(0xF2, 0x85, 0x85, 0xFF),
                    lines,
                );
            }
            InfoSlot::LiveStats => {
                let lines = self.mock_live_stats_lines();
                self.show_info(
                    "Live Stats",
                    "Frame-rate ticker + mock hw counters",
                    Color(0x58, 0xB3, 0xF5, 0xFF),
                    lines,
                );
            }
            InfoSlot::StarCrawl | InfoSlot::AudioScope => {
                // Effects don't render a page; intentionally no-op.
            }
        }
    }

    /// Produce the mock diagnostics lines shown on the Diagnostics page.
    ///
    /// These match the shape of the hardware ChipInfoPanel (MCU, flash,
    /// clocks, memory, capabilities) but with values that are
    /// representative rather than scraped from real registers. They are
    /// intentionally stable across ticks so the page reads calmly.
    fn mock_diagnostics_lines(&self) -> Vec<String> {
        vec![
            "MCU: STM32H747XIH6  (rev Y)".into(),
            "SYSCLK: 400 MHz    HCLK: 200 MHz".into(),
            "Flash: 2048 KB     SRAM: 1024 KB".into(),
            format!(
                "Diagnostics: {}   Audio: {}",
                yes_no(self.capabilities.diagnostics),
                yes_no(self.capabilities.audio),
            ),
            format!(
                "Storage: {}       Effects: {}",
                yes_no(self.capabilities.storage),
                yes_no(self.capabilities.effects),
            ),
            "LTDC: 800x480 ARGB8888  @ 60 Hz".into(),
            "Bus: AXI 200 MHz  QoS: high".into(),
        ]
    }

    /// Live stats: updates every tick.
    ///
    /// Uptime is derived from `tick_count` and the shared 60 Hz tick
    /// cadence. Heap and DMA counts are mocked deterministic functions
    /// of `tick_count` so the numbers visibly change without using a
    /// PRNG (which would be noisy).
    fn mock_live_stats_lines(&self) -> Vec<String> {
        const TICK_HZ: u64 = 60;
        let seconds_whole = self.tick_count / TICK_HZ;
        let seconds_frac = (self.tick_count % TICK_HZ) * 100 / TICK_HZ;
        let heap_used_kb = 48 + ((self.tick_count / 30) % 16) as u32;
        let heap_total_kb = 256;
        let heap_pct = heap_used_kb * 100 / heap_total_kb;
        let dma_frames = self.tick_count / 2;
        vec![
            format!("Uptime: {seconds_whole}.{seconds_frac:02} s"),
            format!("Ticks: {}", self.tick_count),
            format!("FPS: {TICK_HZ}   Frame: {:?} ms", 1000 / TICK_HZ),
            format!("Heap: {heap_used_kb}/{heap_total_kb} KB  ({heap_pct}%)"),
            format!("DMA2D frames: {dma_frames}"),
            format!("Backlight: {}%", self.backlight),
            format!(
                "Focus: {}",
                match self.focus {
                    FocusState::Main(i) => format!("main[{i}]"),
                    FocusState::Wing(WingKind::Settings, i) => format!("settings[{i}]"),
                    FocusState::Wing(WingKind::Info, i) => format!("info[{i}]"),
                }
            ),
        ]
    }

    fn handle_key(&mut self, key: &Key) {
        match key {
            Key::ArrowUp => match self.focus {
                FocusState::Main(_) => self.cycle_main_focus(-1),
                FocusState::Wing(_, _) => self.cycle_wing_focus(-1),
            },
            Key::ArrowDown => match self.focus {
                FocusState::Main(_) => self.cycle_main_focus(1),
                FocusState::Wing(_, _) => self.cycle_wing_focus(1),
            },
            Key::ArrowLeft => {
                if matches!(self.focus, FocusState::Wing(_, _)) {
                    self.close_wings();
                } else {
                    self.cycle_main_focus(-1);
                }
            }
            Key::ArrowRight => {
                if matches!(self.focus, FocusState::Wing(_, _)) {
                    self.close_wings();
                } else {
                    self.cycle_main_focus(1);
                }
            }
            Key::Enter | Key::Space => match self.focus {
                FocusState::Main(index) => self.activate_main(MainSlot::from_index(index)),
                FocusState::Wing(WingKind::Settings, index) => {
                    self.activate_settings(SettingsSlot::from_index(index))
                }
                FocusState::Wing(WingKind::Info, index) => {
                    self.activate_info(InfoSlot::from_index(index))
                }
            },
            Key::Escape => self.close_wings(),
            Key::Character('s') | Key::Character('S') => self.activate_main(MainSlot::Settings),
            Key::Character('f') | Key::Character('F') => self.activate_main(MainSlot::Files),
            Key::Character('i') | Key::Character('I') => self.activate_main(MainSlot::Info),
            Key::Character('b') | Key::Character('B') => {
                self.activate_settings(SettingsSlot::Backlight)
            }
            _ => {}
        }
    }
}

/// Shared controller that owns the 747-style demo widget tree and command queue.
pub struct DiscoController {
    root: Rc<RefCell<WidgetNode>>,
    state: Rc<RefCell<ControllerState>>,
}

impl DiscoController {
    /// Build a new shared demo controller for the requested screen.
    ///
    /// `screen` carries the logical width and height the application
    /// draws into; the controller never reads `screen.rotation`
    /// (that's the platform layer's concern).
    pub fn new(screen: rlvgl_platform::Screen, capabilities: DiscoCapabilities) -> Self {
        let (logical_w, logical_h) = screen.logical_size();
        let width = if logical_w == 0 {
            assets::DISPLAY_WIDTH
        } else {
            logical_w as i32
        };
        let height = if logical_h == 0 {
            assets::DISPLAY_HEIGHT
        } else {
            logical_h as i32
        };

        let mut root_container = Container::new(Rect {
            x: 0,
            y: 0,
            width,
            height,
        });
        // Transparent root — desktop/splash background shows through.
        // Alpha=0 means draw_widget_bg skips the fill entirely.
        root_container.style = StyleBuilder::new().bg_color(Color(0, 0, 0, 0)).build();
        let root = Rc::new(RefCell::new(WidgetNode {
            widget: Rc::new(RefCell::new(root_container)),
            children: Vec::new(),
            tag: Some("disco.root"),
        }));

        let title = themed_label(
            "STM32H747I-DISCO Runtime",
            Rect {
                x: 84,
                y: 24,
                width: 420,
                height: 18,
            },
            Color(248, 249, 250, 255),
        );
        let subtitle = themed_label(
            "Focus: main Settings",
            Rect {
                x: 84,
                y: 48,
                width: 420,
                height: 18,
            },
            Color(148, 162, 184, 255),
        );
        let footer = themed_label(
            "Ready",
            Rect {
                x: 84,
                y: height - 32,
                width: 620,
                height: 18,
            },
            Color(192, 203, 215, 255),
        );

        let dashboard = Rc::new(RefCell::new(DashboardPanel::new(
            Rect {
                x: assets::PANEL_X.min(width - assets::PANEL_WIDTH - 12),
                y: assets::PANEL_Y.min(height - assets::PANEL_HEIGHT - 24),
                width: assets::PANEL_WIDTH.min(width - 120),
                height: assets::PANEL_HEIGHT.min(height - 120),
            },
            "About",
            "rlvgl demo application",
        )));

        let event_window = Rc::new(RefCell::new(
            EventWindowBuilder::new(&FONT_6X10)
                .width(420)
                .center(width, height)
                .expire_ticks(180)
                .build(),
        ));

        let settings_wing = Rc::new(RefCell::new(Wing::new(&[
            (assets::ICON_AUDIO_48, capabilities.audio),
            (assets::ICON_CAMERA_48, false),
            (assets::ICON_MONITOR_48, true),
            (assets::ICON_GLOBE_48, true),
            (assets::ICON_BUG_48, true),
            (assets::ICON_INFO, true), // About
        ])));

        let info_wing = Rc::new(RefCell::new(Wing::new(&[
            (assets::ICON_CPU_48, true),
            (assets::ICON_MONITOR_48, true),
            (assets::ICON_PLAY_48, capabilities.effects),
            (assets::ICON_AUDIO_48, capabilities.audio),
        ])));

        let icon_strip = {
            let mut strip = IconStrip::new(
                width - STRIP_X_OFFSET,
                STRIP_ICON_SIZE,
                STRIP_MARGIN_TOP,
                STRIP_GAP,
            );
            strip.set_slot(
                0,
                IconSlot {
                    rle: assets::ICON_SETTINGS,
                    enabled: true,
                    on_tap: None,
                },
            );
            strip.set_slot(
                1,
                IconSlot {
                    rle: assets::ICON_FILE,
                    enabled: true,
                    on_tap: None,
                },
            );
            strip.set_slot(
                2,
                IconSlot {
                    rle: assets::ICON_INFO,
                    enabled: true,
                    on_tap: None,
                },
            );
            Rc::new(RefCell::new(strip))
        };

        let state = Rc::new(RefCell::new(ControllerState::new(
            capabilities,
            dashboard.clone(),
            subtitle.clone(),
            footer.clone(),
            event_window.clone(),
            icon_strip.clone(),
            settings_wing.clone(),
            info_wing.clone(),
        )));

        {
            let state_for_settings = state.clone();
            icon_strip.borrow_mut().slots_mut()[0]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                state_for_settings
                    .borrow_mut()
                    .activate_main(MainSlot::Settings);
            }));
            let state_for_files = state.clone();
            icon_strip.borrow_mut().slots_mut()[1]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                state_for_files.borrow_mut().activate_main(MainSlot::Files);
            }));
            let state_for_info = state.clone();
            icon_strip.borrow_mut().slots_mut()[2]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |_| {
                state_for_info.borrow_mut().activate_main(MainSlot::Info);
            }));
        }

        for index in 0..6 {
            let shared_state = state.clone();
            settings_wing.borrow_mut().slots_mut()[index]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |slot| {
                shared_state
                    .borrow_mut()
                    .activate_settings(SettingsSlot::from_index(slot));
            }));
        }

        for index in 0..4 {
            let shared_state = state.clone();
            info_wing.borrow_mut().slots_mut()[index]
                .as_mut()
                .unwrap()
                .on_tap = Some(alloc::boxed::Box::new(move |slot| {
                shared_state
                    .borrow_mut()
                    .activate_info(InfoSlot::from_index(slot));
            }));
        }

        root.borrow_mut().children.push(WidgetNode {
            widget: title,
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: subtitle.clone(),
            children: Vec::new(),
            tag: Some("disco.subtitle"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: dashboard,
            children: Vec::new(),
            tag: Some("disco.dashboard"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: footer.clone(),
            children: Vec::new(),
            tag: Some("disco.footer"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: event_window,
            children: Vec::new(),
            tag: Some("disco.events"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: settings_wing.clone(),
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: info_wing.clone(),
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: icon_strip.clone(),
            children: Vec::new(),
            tag: None,
        });

        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(ActionHotspot::new(
                strip_slot_bounds(width, 0),
                {
                    let state = state.clone();
                    move || state.borrow_mut().activate_main(MainSlot::Settings)
                },
            ))),
            children: Vec::new(),
            tag: Some("disco.main.settings"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(ActionHotspot::new(
                strip_slot_bounds(width, 1),
                {
                    let state = state.clone();
                    move || state.borrow_mut().activate_main(MainSlot::Files)
                },
            ))),
            children: Vec::new(),
            tag: Some("disco.main.files"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(ActionHotspot::new(
                strip_slot_bounds(width, 2),
                {
                    let state = state.clone();
                    move || state.borrow_mut().activate_main(MainSlot::Info)
                },
            ))),
            children: Vec::new(),
            tag: Some("disco.main.info"),
        });

        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(0, 5), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_settings(SettingsSlot::Audio)
                })
                .with_visibility({
                    let wing = settings_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.settings.audio"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(1, 5), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_settings(SettingsSlot::Camera)
                })
                .with_visibility({
                    let wing = settings_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.settings.camera"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(2, 5), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_settings(SettingsSlot::Display)
                })
                .with_visibility({
                    let wing = settings_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.settings.display"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(3, 5), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_settings(SettingsSlot::Locale)
                })
                .with_visibility({
                    let wing = settings_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.settings.locale"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(4, 5), {
                    let state = state.clone();
                    move || {
                        state
                            .borrow_mut()
                            .activate_settings(SettingsSlot::Backlight)
                    }
                })
                .with_visibility({
                    let wing = settings_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.settings.backlight"),
        });

        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(0, 4), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_info(InfoSlot::Diagnostics)
                })
                .with_visibility({
                    let wing = info_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.info.diagnostics"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(1, 4), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_info(InfoSlot::LiveStats)
                })
                .with_visibility({
                    let wing = info_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.info.live_stats"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(2, 4), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_info(InfoSlot::StarCrawl)
                })
                .with_visibility({
                    let wing = info_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.info.star_crawl"),
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(
                ActionHotspot::new(wing_slot_bounds(3, 4), {
                    let state = state.clone();
                    move || state.borrow_mut().activate_info(InfoSlot::AudioScope)
                })
                .with_visibility({
                    let wing = info_wing.clone();
                    move || wing.borrow().is_visible()
                }),
            )),
            children: Vec::new(),
            tag: Some("disco.info.audio_scope"),
        });

        let controller = Self { root, state };
        // Dashboard starts hidden — user navigates to it via settings/info.
        // Populate the home content so it's ready when shown.
        controller.state.borrow_mut().show_home();
        controller.state.borrow_mut().dashboard.borrow_mut().hide();
        controller
            .state
            .borrow_mut()
            .set_footer("Ready: tap the strip or use arrows + Enter");
        controller
    }

    /// Return a clone of the owned root widget tree handle.
    pub fn root(&self) -> Rc<RefCell<WidgetNode>> {
        self.root.clone()
    }

    /// Dispatch an event through the widget tree and internal controller logic.
    pub fn dispatch_event(&mut self, event: &Event) -> bool {
        let consumed = self.root.borrow_mut().dispatch_event(event);
        self.handle_event(event);
        consumed
    }

    /// Handle an event after widget dispatch.
    pub fn handle_event(&mut self, event: &Event) {
        let mut state = self.state.borrow_mut();
        if state.focus_dirty {
            state.sync_focus_highlights();
        }
        match event {
            Event::Tick => {
                state.tick_count = state.tick_count.wrapping_add(1);
                if state.tick_count.is_multiple_of(600) {
                    let tick_count = state.tick_count;
                    let backlight = state.backlight;
                    state.set_footer(format!(
                        "Ready: {tick_count} ticks | backlight {backlight}%"
                    ));
                }
                // Keep live info pages updating at frame rate. Only the
                // slots with dynamic mock data (Live Stats, Diagnostics)
                // are currently refreshable — effects clear active_info
                // when they activate.
                if let Some(slot) = state.active_info {
                    state.render_info_page(slot);
                }
            }
            Event::KeyDown { key } => state.handle_key(key),
            Event::PressRelease { x, y } if !state.capabilities.pointer => {
                state.push_status(format!("Pointer input ignored at ({x}, {y})"));
            }
            _ => {}
        }
    }

    /// Advance the widget tree by one shared demo tick.
    pub fn tick(&mut self) {
        self.root.borrow_mut().dispatch_event(&Event::Tick);
        self.handle_event(&Event::Tick);
    }

    /// Drain platform commands requested since the previous call.
    pub fn drain_commands(&mut self) -> Vec<DiscoCommand> {
        core::mem::take(&mut self.state.borrow_mut().commands)
    }

    /// Surface a runtime-produced status line through the shared UI.
    pub fn publish_status(&mut self, text: impl Into<String>) {
        self.state.borrow_mut().push_status(text.into());
    }
}

/// Format a bool as `"yes"` / `"no"` for status pages.
fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn themed_label(text: impl Into<String>, bounds: Rect, text_color: Color) -> Rc<RefCell<Label>> {
    let mut label = Label::new(text, bounds);
    label.style = StyleBuilder::new()
        .bg_color(Color(0, 0, 0, 0))
        .alpha(0)
        .build();
    label.text_color = text_color;
    Rc::new(RefCell::new(label))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_node<'a>(root: &'a WidgetNode, tag: &str) -> Option<&'a WidgetNode> {
        if root.tag == Some(tag) {
            return Some(root);
        }
        for child in &root.children {
            if let Some(found) = find_node(child, tag) {
                return Some(found);
            }
        }
        None
    }

    fn find_node_mut<'a>(root: &'a mut WidgetNode, tag: &str) -> Option<&'a mut WidgetNode> {
        if root.tag == Some(tag) {
            return Some(root);
        }
        for child in &mut root.children {
            if let Some(found) = find_node_mut(child, tag) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn controller_builds_shared_automation_surface_for_all_runtime_presets() {
        let sim = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let uefi = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::uefi(),
        );
        let stm = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::stm32h747i_disco(),
        );

        let required_tags = [
            "disco.root",
            "disco.dashboard",
            "disco.subtitle",
            "disco.footer",
            "disco.events",
            "disco.main.settings",
            "disco.main.files",
            "disco.main.info",
            "disco.settings.audio",
            "disco.settings.camera",
            "disco.settings.display",
            "disco.settings.locale",
            "disco.settings.backlight",
            "disco.info.diagnostics",
            "disco.info.live_stats",
            "disco.info.star_crawl",
            "disco.info.audio_scope",
        ];

        for controller in [&sim, &uefi, &stm] {
            let root = controller.root.borrow();
            for tag in required_tags {
                assert!(find_node(&root, tag).is_some(), "missing tag {tag}");
            }
        }
    }

    #[test]
    fn wing_hotspots_collapse_bounds_until_their_wing_is_open() {
        let mut controller = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let root = controller.root();

        {
            let root = root.borrow();
            let main = find_node(&root, "disco.main.settings").unwrap();
            assert!(main.widget.borrow().bounds().width > 0);
            assert!(main.widget.borrow().bounds().height > 0);

            let settings = find_node(&root, "disco.settings.audio").unwrap();
            assert_eq!(settings.widget.borrow().bounds().width, 0);
            assert_eq!(settings.widget.borrow().bounds().height, 0);

            let info = find_node(&root, "disco.info.live_stats").unwrap();
            assert_eq!(info.widget.borrow().bounds().width, 0);
            assert_eq!(info.widget.borrow().bounds().height, 0);
        }

        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });

        {
            let root = root.borrow();
            let settings = find_node(&root, "disco.settings.audio").unwrap();
            assert!(settings.widget.borrow().bounds().width > 0);
            assert!(settings.widget.borrow().bounds().height > 0);
        }

        controller.dispatch_event(&Event::KeyDown {
            key: Key::Character('i'),
        });

        {
            let root = root.borrow();
            let settings = find_node(&root, "disco.settings.audio").unwrap();
            assert_eq!(settings.widget.borrow().bounds().width, 0);
            assert_eq!(settings.widget.borrow().bounds().height, 0);

            let info = find_node(&root, "disco.info.live_stats").unwrap();
            assert!(info.widget.borrow().bounds().width > 0);
            assert!(info.widget.borrow().bounds().height > 0);
        }
    }

    #[test]
    fn tagged_hotspot_dispatch_triggers_the_same_controller_actions() {
        let mut controller = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let root = controller.root();

        {
            let mut root = root.borrow_mut();
            let hotspot = find_node_mut(&mut root, "disco.main.files").unwrap();
            assert!(hotspot.dispatch_event(&Event::PressRelease { x: 0, y: 0 }));
        }

        let commands = controller.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::LoadStorageSummary))
        );
    }

    #[test]
    fn unsupported_audio_action_neutralizes_without_panicking() {
        let mut controller = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::uefi(),
        );
        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });
        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let commands = controller.drain_commands();
        assert!(commands.iter().any(|cmd| matches!(cmd, DiscoCommand::NoOp)));
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::ShowStatus(_)))
        );
    }

    #[test]
    fn storage_command_is_emitted_from_main_strip_flow() {
        let mut controller = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        controller.dispatch_event(&Event::KeyDown {
            key: Key::ArrowDown,
        });
        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let commands = controller.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::LoadStorageSummary))
        );
    }

    #[test]
    fn effect_command_is_emitted_for_enabled_platforms() {
        let mut controller = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::stm32h747i_disco(),
        );
        controller.dispatch_event(&Event::KeyDown {
            key: Key::ArrowDown,
        });
        controller.dispatch_event(&Event::KeyDown {
            key: Key::ArrowDown,
        });
        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });
        controller.dispatch_event(&Event::KeyDown {
            key: Key::ArrowDown,
        });
        controller.dispatch_event(&Event::KeyDown {
            key: Key::ArrowDown,
        });
        controller.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let commands = controller.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| { matches!(cmd, DiscoCommand::StartEffect(DiscoEffect::StarCrawl)) })
        );
    }

    // ── Focus navigation tests ──────────────────────────────────────

    fn key_down(controller: &mut DiscoController, key: Key) {
        controller.dispatch_event(&Event::KeyDown { key });
    }

    fn focus(controller: &DiscoController) -> FocusState {
        controller.state.borrow().focus
    }

    #[test]
    fn arrow_down_cycles_main_focus() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        assert_eq!(focus(&c), FocusState::Main(0));
        key_down(&mut c, Key::ArrowDown);
        assert_eq!(focus(&c), FocusState::Main(1));
        key_down(&mut c, Key::ArrowDown);
        assert_eq!(focus(&c), FocusState::Main(2));
        key_down(&mut c, Key::ArrowDown);
        assert_eq!(focus(&c), FocusState::Main(0));
    }

    #[test]
    fn arrow_up_wraps_from_first_slot() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        assert_eq!(focus(&c), FocusState::Main(0));
        key_down(&mut c, Key::ArrowUp);
        assert_eq!(focus(&c), FocusState::Main(2));
    }

    #[test]
    fn enter_opens_wing_escape_closes() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let root = c.root();
        key_down(&mut c, Key::Enter);
        {
            let root = root.borrow();
            let audio = find_node(&root, "disco.settings.audio").unwrap();
            assert!(audio.widget.borrow().bounds().width > 0);
        }
        key_down(&mut c, Key::Escape);
        {
            let root = root.borrow();
            let audio = find_node(&root, "disco.settings.audio").unwrap();
            assert_eq!(audio.widget.borrow().bounds().width, 0);
        }
    }

    #[test]
    fn arrow_left_from_wing_returns_to_main() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Enter);
        assert!(matches!(focus(&c), FocusState::Wing(WingKind::Settings, _)));
        key_down(&mut c, Key::ArrowLeft);
        assert!(matches!(focus(&c), FocusState::Main(_)));
    }

    #[test]
    fn wing_focus_cycles_all_settings_items() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Enter);
        assert_eq!(focus(&c), FocusState::Wing(WingKind::Settings, 0));
        for i in 1..6 {
            key_down(&mut c, Key::ArrowDown);
            assert_eq!(focus(&c), FocusState::Wing(WingKind::Settings, i));
        }
        key_down(&mut c, Key::ArrowDown);
        assert_eq!(focus(&c), FocusState::Wing(WingKind::Settings, 0));
    }

    #[test]
    fn wing_focus_cycles_all_info_items() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Character('i'));
        assert_eq!(focus(&c), FocusState::Wing(WingKind::Info, 0));
        for i in 1..4 {
            key_down(&mut c, Key::ArrowDown);
            assert_eq!(focus(&c), FocusState::Wing(WingKind::Info, i));
        }
        key_down(&mut c, Key::ArrowDown);
        assert_eq!(focus(&c), FocusState::Wing(WingKind::Info, 0));
    }

    #[test]
    fn hotkey_s_activates_settings() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::ArrowDown); // move to Files
        key_down(&mut c, Key::Character('s'));
        let root = c.root();
        let root = root.borrow();
        let audio = find_node(&root, "disco.settings.audio").unwrap();
        assert!(audio.widget.borrow().bounds().width > 0);
    }

    #[test]
    fn hotkey_f_emits_storage_command() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Character('f'));
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::LoadStorageSummary))
        );
    }

    #[test]
    fn hotkey_i_activates_info_wing() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Character('i'));
        let root = c.root();
        let root = root.borrow();
        let diag = find_node(&root, "disco.info.diagnostics").unwrap();
        assert!(diag.widget.borrow().bounds().width > 0);
    }

    #[test]
    fn hotkey_b_cycles_backlight() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        // Default backlight is 75
        key_down(&mut c, Key::Character('b'));
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::SetBacklight(100)))
        );

        key_down(&mut c, Key::Character('b'));
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::SetBacklight(25)))
        );

        key_down(&mut c, Key::Character('b'));
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::SetBacklight(50)))
        );

        key_down(&mut c, Key::Character('b'));
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::SetBacklight(75)))
        );
    }

    #[test]
    fn audio_on_capable_platform_emits_start_effect() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::stm32h747i_disco(),
        );
        key_down(&mut c, Key::Enter); // open settings wing
        key_down(&mut c, Key::Enter); // activate audio (index 0)
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| { matches!(cmd, DiscoCommand::StartEffect(DiscoEffect::AudioScope)) })
        );
    }

    #[test]
    fn info_diagnostics_emits_show_status() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Character('i'));
        key_down(&mut c, Key::Enter); // activate diagnostics (index 0)
        let commands = c.drain_commands();
        assert!(
            commands
                .iter()
                .any(|cmd| matches!(cmd, DiscoCommand::ShowStatus(_)))
        );
    }

    #[test]
    fn pointer_ignored_on_uefi_caps() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::uefi(),
        );
        c.dispatch_event(&Event::PressRelease { x: 100, y: 100 });
        // Should not panic — UEFI has pointer: false, events go through normally
        // but no widget should consume them in the hotspot path
    }

    #[test]
    fn opening_info_closes_settings() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let root = c.root();
        key_down(&mut c, Key::Enter); // open settings
        {
            let root = root.borrow();
            assert!(
                find_node(&root, "disco.settings.audio")
                    .unwrap()
                    .widget
                    .borrow()
                    .bounds()
                    .width
                    > 0
            );
        }
        key_down(&mut c, Key::Character('i')); // open info
        {
            let root = root.borrow();
            assert_eq!(
                find_node(&root, "disco.settings.audio")
                    .unwrap()
                    .widget
                    .borrow()
                    .bounds()
                    .width,
                0
            );
            assert!(
                find_node(&root, "disco.info.diagnostics")
                    .unwrap()
                    .widget
                    .borrow()
                    .bounds()
                    .width
                    > 0
            );
        }
    }

    #[test]
    fn drain_commands_clears_queue() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Character('f'));
        let first = c.drain_commands();
        assert!(!first.is_empty());
        let second = c.drain_commands();
        assert!(second.is_empty());
    }

    #[test]
    fn custom_display_size_respected() {
        let c = DiscoController::new(
            rlvgl_platform::Screen::landscape(1024, 600),
            DiscoCapabilities::simulator(),
        );
        let root = c.root();
        let root = root.borrow();
        let bounds = root.widget.borrow().bounds();
        assert_eq!(bounds.width, 1024);
        assert_eq!(bounds.height, 600);
    }

    // ── Focus highlight tests ───────────────────────────────────────

    #[test]
    fn initial_focus_highlight_on_slot_zero() {
        let c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        let state = c.state.borrow();
        assert_eq!(state.icon_strip.borrow().focused_slot(), Some(0));
        assert_eq!(state.settings_wing.borrow().focused_slot(), None);
        assert_eq!(state.info_wing.borrow().focused_slot(), None);
    }

    #[test]
    fn arrow_down_moves_highlight_to_next_slot() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::ArrowDown);
        let state = c.state.borrow();
        assert_eq!(state.icon_strip.borrow().focused_slot(), Some(1));
    }

    #[test]
    fn enter_moves_highlight_to_wing() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Enter);
        let state = c.state.borrow();
        assert_eq!(state.icon_strip.borrow().focused_slot(), None);
        assert_eq!(state.settings_wing.borrow().focused_slot(), Some(0));
    }

    #[test]
    fn escape_restores_highlight_to_main() {
        let mut c = DiscoController::new(
            rlvgl_platform::Screen::landscape(800, 480),
            DiscoCapabilities::simulator(),
        );
        key_down(&mut c, Key::Enter);
        key_down(&mut c, Key::Escape);
        let state = c.state.borrow();
        assert!(state.icon_strip.borrow().focused_slot().is_some());
        assert_eq!(state.settings_wing.borrow().focused_slot(), None);
    }
}
