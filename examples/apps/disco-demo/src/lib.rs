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
mod icon_strip;
mod wing;

use alloc::{format, rc::Rc, string::String, vec, vec::Vec};
use core::cell::RefCell;

use dashboard_panel::DashboardPanel;
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
}

impl DiscoCapabilities {
    /// Capability preset for simulator hosts.
    pub const fn simulator() -> Self {
        Self {
            audio: false,
            storage: true,
            diagnostics: true,
            effects: false,
            pointer: true,
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
}

impl SettingsSlot {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Camera,
            2 => Self::Display,
            3 => Self::Locale,
            4 => Self::Backlight,
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

struct ControllerState {
    capabilities: DiscoCapabilities,
    commands: Vec<DiscoCommand>,
    dashboard: Rc<RefCell<DashboardPanel>>,
    subtitle: Rc<RefCell<Label>>,
    footer: Rc<RefCell<Label>>,
    event_window: Rc<RefCell<EventWindow>>,
    settings_wing: Rc<RefCell<Wing>>,
    info_wing: Rc<RefCell<Wing>>,
    focus: FocusState,
    tick_count: u64,
    backlight: u8,
}

impl ControllerState {
    fn new(
        capabilities: DiscoCapabilities,
        dashboard: Rc<RefCell<DashboardPanel>>,
        subtitle: Rc<RefCell<Label>>,
        footer: Rc<RefCell<Label>>,
        event_window: Rc<RefCell<EventWindow>>,
        settings_wing: Rc<RefCell<Wing>>,
        info_wing: Rc<RefCell<Wing>>,
    ) -> Self {
        Self {
            capabilities,
            commands: Vec::new(),
            dashboard,
            subtitle,
            footer,
            event_window,
            settings_wing,
            info_wing,
            focus: FocusState::Main(0),
            tick_count: 0,
            backlight: 75,
        }
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

    fn show_home(&mut self) {
        self.dashboard.borrow_mut().set_title("Flight Deck");
        self.dashboard
            .borrow_mut()
            .set_caption("Shared 747-style demo controller");
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
        self.dashboard.borrow_mut().set_title(title);
        self.dashboard.borrow_mut().set_caption(caption);
        self.dashboard.borrow_mut().set_accent(accent);
        self.dashboard.borrow_mut().set_lines(lines);
    }

    fn close_wings(&mut self) {
        self.settings_wing.borrow_mut().close();
        self.info_wing.borrow_mut().close();
        let focus_index = match self.focus {
            FocusState::Main(index) | FocusState::Wing(_, index) => index,
        };
        self.focus = FocusState::Main(focus_index.min(2));
        self.refresh_focus_hint();
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
        self.show_info(
            "Settings Wing",
            "Shared settings actions without board registers",
            Color(0xE4, 0xA8, 0x40, 0xFF),
            vec![
                "Audio scope, locale, and backlight commands are queued here.".into(),
                "Unsupported entries stay neutral instead of panicking.".into(),
                format!("Audio supported: {}", self.capabilities.audio),
            ],
        );
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
        self.show_info(
            "Info Wing",
            "Diagnostics and effect hooks shared across runtimes",
            Color(0xA7, 0x84, 0xF7, 0xFF),
            vec![
                "Diagnostics and live stats are platform-capability driven.".into(),
                "Star crawl and audio scope remain runtime-owned effects.".into(),
            ],
        );
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

    fn cycle_main_focus(&mut self, delta: i32) {
        let current = match self.focus {
            FocusState::Main(index) => index as i32,
            FocusState::Wing(_, _) => return,
        };
        let next = (current + delta).rem_euclid(3) as usize;
        self.focus = FocusState::Main(next);
        self.refresh_focus_hint();
    }

    fn cycle_wing_focus(&mut self, delta: i32) {
        self.focus = match self.focus {
            FocusState::Wing(WingKind::Settings, index) => FocusState::Wing(
                WingKind::Settings,
                (index as i32 + delta).rem_euclid(5) as usize,
            ),
            FocusState::Wing(WingKind::Info, index) => FocusState::Wing(
                WingKind::Info,
                (index as i32 + delta).rem_euclid(4) as usize,
            ),
            other => other,
        };
        self.refresh_focus_hint();
    }

    fn activate_main(&mut self, slot: MainSlot) {
        self.focus = FocusState::Main(slot as usize);
        match slot {
            MainSlot::Settings => self.open_settings(),
            MainSlot::Files => {
                self.close_wings();
                self.show_storage();
                if self.capabilities.storage {
                    self.queue(DiscoCommand::LoadStorageSummary);
                    self.push_status("Queued storage summary refresh");
                } else {
                    self.push_status("Storage browser is unavailable on this platform");
                    self.queue(DiscoCommand::NoOp);
                }
            }
            MainSlot::Info => self.open_info(),
        }
    }

    fn activate_settings(&mut self, slot: SettingsSlot) {
        self.focus = FocusState::Wing(WingKind::Settings, slot as usize);
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
                self.push_status("Camera pipeline is intentionally stubbed in the shared demo");
                self.queue(DiscoCommand::NoOp);
            }
            SettingsSlot::Display => {
                self.show_info(
                    "Display Controls",
                    "Replaceable per-platform display hooks",
                    Color(0x57, 0xC2, 0xD8, 0xFF),
                    vec![
                        "Backends own present, backlight, and pixel format translation.".into(),
                        "The shared controller only emits abstract display commands.".into(),
                    ],
                );
                self.push_status("Display status panel refreshed");
            }
            SettingsSlot::Locale => {
                self.show_info(
                    "Locale + Platform",
                    "Shared controller status for non-MCU runtimes",
                    Color(0x7A, 0xD6, 0x8A, 0xFF),
                    vec![
                        format!("Pointer enabled: {}", self.capabilities.pointer),
                        format!("Diagnostics enabled: {}", self.capabilities.diagnostics),
                    ],
                );
                self.push_status("Platform summary updated");
            }
            SettingsSlot::Backlight => {
                self.backlight = match self.backlight {
                    100 => 25,
                    75 => 100,
                    50 => 75,
                    25 => 50,
                    _ => 75,
                };
                self.show_home();
                self.push_status(format!("Queued backlight level {}%", self.backlight));
                self.queue(DiscoCommand::SetBacklight(self.backlight));
            }
        }
    }

    fn activate_info(&mut self, slot: InfoSlot) {
        self.focus = FocusState::Wing(WingKind::Info, slot as usize);
        match slot {
            InfoSlot::Diagnostics => {
                let mut lines = vec![
                    "Diagnostics page extracted from the board demo.".into(),
                    "Runtimes decide how much hardware detail to expose.".into(),
                ];
                lines.push(format!(
                    "Capability flag: diagnostics = {}",
                    self.capabilities.diagnostics
                ));
                self.show_info(
                    "Diagnostics",
                    "Board-neutral controller page",
                    Color(0xF2, 0x85, 0x85, 0xFF),
                    lines,
                );
                self.push_status("Diagnostics page opened");
            }
            InfoSlot::LiveStats => {
                self.show_info(
                    "Live Stats",
                    "Shared update loop placeholder",
                    Color(0x58, 0xB3, 0xF5, 0xFF),
                    vec![
                        format!("Ticks observed: {}", self.tick_count),
                        format!("Backlight target: {}%", self.backlight),
                        "STM32 can replace this with board telemetry.".into(),
                    ],
                );
                self.push_status("Live stats panel refreshed");
            }
            InfoSlot::StarCrawl => {
                if self.capabilities.effects {
                    self.push_status("Queued star crawl effect");
                    self.queue(DiscoCommand::StartEffect(DiscoEffect::StarCrawl));
                } else {
                    self.push_status("Star crawl is unavailable on this platform");
                    self.queue(DiscoCommand::NoOp);
                }
            }
            InfoSlot::AudioScope => {
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
    /// Build a new shared demo controller for the requested display size.
    pub fn new(width: u32, height: u32, capabilities: DiscoCapabilities) -> Self {
        let width = if width == 0 {
            assets::DISPLAY_WIDTH
        } else {
            width as i32
        };
        let height = if height == 0 {
            assets::DISPLAY_HEIGHT
        } else {
            height as i32
        };

        let mut root_container = Container::new(Rect {
            x: 0,
            y: 0,
            width,
            height,
        });
        root_container.style = StyleBuilder::new().bg_color(Color(13, 19, 30, 255)).build();
        let root = Rc::new(RefCell::new(WidgetNode {
            widget: Rc::new(RefCell::new(root_container)),
            children: Vec::new(),
            tag: None,
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
            "Flight Deck",
            "Shared 747-style demo controller",
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
        ])));

        let info_wing = Rc::new(RefCell::new(Wing::new(&[
            (assets::ICON_CPU_48, true),
            (assets::ICON_MONITOR_48, true),
            (assets::ICON_PLAY_48, capabilities.effects),
            (assets::ICON_AUDIO_48, capabilities.audio),
        ])));

        let state = Rc::new(RefCell::new(ControllerState::new(
            capabilities,
            dashboard.clone(),
            subtitle.clone(),
            footer.clone(),
            event_window.clone(),
            settings_wing.clone(),
            info_wing.clone(),
        )));

        let mut icon_strip = IconStrip::new(width - 70, 60, 17, 10);
        icon_strip.set_slot(
            0,
            IconSlot {
                rle: assets::ICON_SETTINGS,
                enabled: true,
                on_tap: None,
            },
        );
        icon_strip.set_slot(
            1,
            IconSlot {
                rle: assets::ICON_FILE,
                enabled: true,
                on_tap: None,
            },
        );
        icon_strip.set_slot(
            2,
            IconSlot {
                rle: assets::ICON_INFO,
                enabled: true,
                on_tap: None,
            },
        );

        {
            let state_for_settings = state.clone();
            icon_strip.slots_mut()[0].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    state_for_settings
                        .borrow_mut()
                        .activate_main(MainSlot::Settings);
                }));
            let state_for_files = state.clone();
            icon_strip.slots_mut()[1].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    state_for_files.borrow_mut().activate_main(MainSlot::Files);
                }));
            let state_for_info = state.clone();
            icon_strip.slots_mut()[2].as_mut().unwrap().on_tap =
                Some(alloc::boxed::Box::new(move |_| {
                    state_for_info.borrow_mut().activate_main(MainSlot::Info);
                }));
        }

        for index in 0..5 {
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
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: dashboard,
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: footer.clone(),
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: event_window,
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: settings_wing,
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: info_wing,
            children: Vec::new(),
            tag: None,
        });
        root.borrow_mut().children.push(WidgetNode {
            widget: Rc::new(RefCell::new(icon_strip)),
            children: Vec::new(),
            tag: None,
        });

        let controller = Self { root, state };
        controller.state.borrow_mut().show_home();
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

    #[test]
    fn controller_builds_for_all_runtime_presets() {
        let sim = DiscoController::new(800, 480, DiscoCapabilities::simulator());
        let uefi = DiscoController::new(800, 480, DiscoCapabilities::uefi());
        let stm = DiscoController::new(800, 480, DiscoCapabilities::stm32h747i_disco());
        assert_eq!(sim.root.borrow().children.len(), 8);
        assert_eq!(uefi.root.borrow().children.len(), 8);
        assert_eq!(stm.root.borrow().children.len(), 8);
    }

    #[test]
    fn unsupported_audio_action_neutralizes_without_panicking() {
        let mut controller = DiscoController::new(800, 480, DiscoCapabilities::uefi());
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
        let mut controller = DiscoController::new(800, 480, DiscoCapabilities::simulator());
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
        let mut controller = DiscoController::new(800, 480, DiscoCapabilities::stm32h747i_disco());
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
}
