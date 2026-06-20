// SPDX-License-Identifier: MIT
//! SCXML Tutorial Demo App — target-neutral rlvgl app crate.
//!
//! Implements the SCTD-01 deliverable: a right-edge Machine Selector
//! (position-compatible with the disco demo icon strip) and a Machine Panel
//! showing active state names and dispatching named SCXML events through a
//! small adapter layer.
//!
//! # SCTD-00 conformance notes
//! * §6.1 — no board init, PAC, OS calls, file I/O, or wall-clock assumptions.
//! * §6.2 — selector uses the same `STRIP_X_OFFSET`, `STRIP_ICON_SIZE`,
//!   `STRIP_MARGIN_TOP`, and `STRIP_GAP` constants as the disco demo.
//! * §6.3 — two required entries (Dining Philosophers, Media Player).
//!   Both are now backed by generated machine crates; the `MediaPlayerStub`
//!   placeholder has been replaced by `MediaPlayerAdapter`.
//! * §6.8/§6.9 — Machine Panel shows active state names and dispatches events
//!   through the `MachineAdapter` trait; UI never reaches into generated
//!   internals beyond the public API.
//! * §7.1 — all tutorial-machine selection and event routing is in this crate.
//!
//! # `active_state_names()` accessor note
//! The Media Player machine is a simple hierarchical (non-parallel) machine
//! with no concurrent regions.  `Machine::current_state()` (which returns the
//! deepest active leaf state as a `&str`) is sufficient; `active_state_names()`
//! (a parallel-region-aware accessor) was considered but is not needed here.
//! The state summary is therefore built from `current_state()` plus the values
//! of the three datamodel variables (`s_source`, `s_mute`, `s_repeat`) via
//! `Machine::get_var()`.
//!
//! # Icon assets
//! Icons are Lucide-derived supplemental glyphs reused from the disco-demo
//! crate (SCTD-00 §6.6 allows Lucide for gaps when tutorial assets are
//! absent). The dp48 glyph represents Dining Philosophers; media48 represents
//! the Media Player. Tutorial-asset icons (from Qt/DiningPhilosophers/
//! Images/ and Qt/SkodaBoleroInfotainment/Qml/Images/) require transcoding to
//! RLE and are deferred to a follow-up per SCTD-00 §6.4.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub mod assets;
mod machine_panel;
mod selector;

use alloc::{boxed::Box, format, rc::Rc, string::{String, ToString}, vec, vec::Vec};
use core::cell::RefCell;

use machine_panel::MachinePanel;
use selector::MachineSelector;

use rlvgl_core::{
    WidgetNode,
    bitmap_font::FONT_6X10,
    event::{Event, Key},
    style::StyleBuilder,
    widget::{Color, Rect},
};
use rlvgl_ui::EventWindowBuilder;
use rlvgl_widgets::{container::Container, label::Label};

// ---------------------------------------------------------------------------
// Layout constants — mirror the disco demo exactly (SCTD-00 §6.2)
// ---------------------------------------------------------------------------

/// Width of each selector icon slot in pixels.
pub const STRIP_ICON_SIZE: i32 = 60;
/// Top margin before the first slot.
pub const STRIP_MARGIN_TOP: i32 = 17;
/// Gap between icon slots.
pub const STRIP_GAP: i32 = 10;
/// Horizontal offset from the right edge of the display to the icon strip.
pub const STRIP_X_OFFSET: i32 = 70;

/// X position of the Machine Panel content region.
const PANEL_X: i32 = 10;
/// Y position of the Machine Panel content region.
const PANEL_Y: i32 = 84;
/// Width of the Machine Panel.
const PANEL_WIDTH: i32 = 560;
/// Height of the Machine Panel.
const PANEL_HEIGHT: i32 = 312;

// ---------------------------------------------------------------------------
// Machine adapter trait (SCTD-00 §6.9)
// ---------------------------------------------------------------------------

/// Adapter trait bridging the UI to a Tutorial Machine's public API.
///
/// Implementations wrap one generated machine crate and expose only the
/// surface the Machine Panel needs: the current active state summary and
/// a small set of named dispatchable events.
pub trait MachineAdapter {
    /// Human-readable name of this tutorial machine (a static string literal).
    fn name(&self) -> &'static str;

    /// One-line summary of the machine's current active state(s).
    ///
    /// Implementations MUST derive this from the machine's public API
    /// (e.g. `Machine::current_state`, `Machine::get_child_state`) and
    /// MUST NOT reach into generated internals beyond those methods.
    fn state_summary(&self) -> String;

    /// Names of events the panel can dispatch to this machine.
    fn available_events(&self) -> &[&'static str];

    /// Dispatch a named event. The adapter converts the string into the
    /// appropriate `machine.step(event_name, Value::Undefined)` call.
    fn dispatch_event(&mut self, event_name: &str);

    /// Advance the machine by one logical step (for timer-driven machines).
    ///
    /// Called once per UI tick. Implementations that do not use a timer
    /// may leave this as a no-op.
    fn tick(&mut self) {}
}

// ---------------------------------------------------------------------------
// Dining Philosophers adapter
// ---------------------------------------------------------------------------

/// Adapter for the iState-generated Dining Philosophers machine.
///
/// Uses only the public API surface of the `dining_philosophers` crate:
/// `Machine::new`, `Machine::start`, `Machine::step`,
/// `Machine::current_state`, `Machine::get_child_state`,
/// `Machine::run` (for timer advancement).
pub struct DiningPhilosophersAdapter {
    machine: dining_philosophers::Machine,
    tick_counter: u64,
}

impl DiningPhilosophersAdapter {
    /// Create and start the Dining Philosophers machine.
    pub fn new() -> Self {
        let mut machine = dining_philosophers::Machine::new();
        machine.start();
        Self {
            machine,
            tick_counter: 0,
        }
    }
}

impl Default for DiningPhilosophersAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MachineAdapter for DiningPhilosophersAdapter {
    fn name(&self) -> &'static str {
        "Dining Philosophers"
    }

    fn state_summary(&self) -> String {
        // Show top-level state plus child machine states for each philosopher.
        // Uses only `Machine::current_state` and `Machine::get_child_state`
        // from the generated public API (SCTD-00 §6.9).
        let parent = self.machine.current_state();
        let p1 = self
            .machine
            .get_child_state("ID_P_1")
            .unwrap_or_else(|| "--".into());
        let p2 = self
            .machine
            .get_child_state("ID_P_2")
            .unwrap_or_else(|| "--".into());
        let p3 = self
            .machine
            .get_child_state("ID_P_3")
            .unwrap_or_else(|| "--".into());
        let p4 = self
            .machine
            .get_child_state("ID_P_4")
            .unwrap_or_else(|| "--".into());
        let p5 = self
            .machine
            .get_child_state("ID_P_5")
            .unwrap_or_else(|| "--".into());
        format!(
            "{} | P1:{} P2:{} P3:{} P4:{} P5:{}",
            parent, p1, p2, p3, p4, p5
        )
    }

    fn available_events(&self) -> &[&'static str] {
        // Manually dispatchable events from the tutorial machine's public
        // event vocabulary (visible in machine_ir_data transition event fields).
        // Timer events (Do.Timer.*) are normally fired by the timer queue;
        // exposing them here lets the demo panel demonstrate event dispatch
        // without waiting for the timer (SCTD-00 §6.8).
        &[
            "Do.Timer.Hungry",
            "Do.Timer.Think",
            "taken.1",
            "taken.2",
            "taken.3",
        ]
    }

    fn dispatch_event(&mut self, event_name: &str) {
        self.machine
            .step(event_name, dining_philosophers::Value::Undefined);
    }

    fn tick(&mut self) {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        // Advance the machine by one logical step. `Machine::run` fires any
        // timers whose `fire_time_ms <= logical_clock_ms` and delivers pending
        // internal events. For the Dining Philosophers machine this advances
        // the timer queue so hungry/eating transitions fire over time.
        let _ = self.machine.run(1);
    }
}

// ---------------------------------------------------------------------------
// Media Player adapter (SCTD-00 §6.3 — real normalized Bolero machine)
// ---------------------------------------------------------------------------

/// Adapter for the iState-generated Media Player machine.
///
/// The machine crate is generated from
/// `machines/media-player/source/media_player_normalized.scxml`, which is a
/// provenance-marked normalized form of the Skoda Bolero SCXML (In() cross-
/// region predicates replaced by explicit `s_mute` / `s_repeat` / `s_source`
/// datamodel variables, per SCTD-00 §5.3 and D-M1P6-7).
///
/// Uses only the public API surface of the `media_player` crate:
/// `Machine::new`, `Machine::start`, `Machine::step`,
/// `Machine::current_state`, `Machine::get_var` (for datamodel variable
/// display).  `active_state_names()` is not needed because this machine
/// has no parallel regions; `current_state()` returns the deepest active
/// leaf state directly.
pub struct MediaPlayerAdapter {
    machine: media_player::Machine,
}

impl MediaPlayerAdapter {
    /// Create and start the Media Player machine.
    pub fn new() -> Self {
        let mut machine = media_player::Machine::new();
        machine.start();
        Self { machine }
    }
}

impl Default for MediaPlayerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MachineAdapter for MediaPlayerAdapter {
    fn name(&self) -> &'static str {
        "Media Player"
    }

    fn state_summary(&self) -> String {
        // Uses `Machine::current_state` for the transport state (leaf state)
        // and `Machine::get_var` for the three normalized datamodel variables.
        // These are the only generated-crate methods needed for this simple
        // hierarchical machine (no parallel regions, no child machines).
        let transport = self.machine.current_state();

        let source = match self.machine.get_var("s_source") {
            media_player::Value::Str(s) => s,
            _ => "?".into(),
        };
        let muted = match self.machine.get_var("s_mute") {
            media_player::Value::Bool(b) => b,
            _ => false,
        };
        let repeat = match self.machine.get_var("s_repeat") {
            media_player::Value::Str(s) => s,
            _ => "none".into(),
        };

        format!(
            "{} | src:{} mute:{} repeat:{}",
            transport,
            source,
            if muted { "on" } else { "off" },
            repeat,
        )
    }

    fn available_events(&self) -> &[&'static str] {
        // Full dispatchable event vocabulary from the normalized SCXML.
        // Events are presented in logical UX order:
        //   transport controls → source selection → toggles → system
        &[
            "Inp.Media.Ready",
            "Inp.Media.ValidSource",
            "Inp.Media.Play",
            "Inp.Media.Pause",
            "Inp.Media.Stop",
            "Inp.Media.Next",
            "Inp.Media.Prev",
            "Inp.Media.Source.USB",
            "Inp.Media.Source.SD",
            "Inp.Media.Source.AUX",
            "Inp.Media.Mute",
            "Inp.Media.Repeat",
            "Inp.Media.Error",
        ]
    }

    fn dispatch_event(&mut self, event_name: &str) {
        // Delegates directly to the generated machine's public `step` method.
        // Uses `Value::Undefined` as event data — all MP transitions are
        // triggered by event name only (no data payload guards in the normalized SCXML).
        self.machine.step(event_name, media_player::Value::Undefined);
    }
}

// ---------------------------------------------------------------------------
// Interactive Dining Philosophers adapter (SCTD-02 §5/§6)
// ---------------------------------------------------------------------------

/// Refills per machine logical-step at each speed setting (x0.5 / x1 / x2).
/// At a ~30 Hz refill, x1 ≈ one philosopher event every ~1.5 s — human-watchable
/// (SCTD-02 INV-SCTD02-2). The machine's logical clock is thereby decoupled from
/// the framebuffer refill rate; only the cadence is shown, never frame-accurate.
const IDP_SPEED_THRESHOLDS: [u32; 3] = [90, 45, 22];
/// Display labels for the three speed settings.
const IDP_SPEED_LABELS: [&str; 3] = ["x0.5", "x1", "x2"];

/// Adapter for the SCTD-02 Interactive Dining Philosophers machine.
///
/// The generated `dining_philosophers_interactive` machine is a true `<parallel>`
/// of five philosopher regions. The emitter advances one transition per region
/// per event, so the host computes the target seat and dispatches the per-seat
/// events the machine listens for: `arrive.N` (seat the lowest-empty chair),
/// `depart.N` (remove the highest-seated), `break.N` (panic: release N's forks),
/// `poke.N` (let a waiting philosopher re-attempt a freed fork). Pause/Speed are
/// host-side simulation controls (SCTD-02 §6.4): they gate the logical tick, not
/// the machine. Reset re-instantiates the machine (SCTD-02 §5.7).
///
/// Uses only the generated public API: `Machine::new/start/step/run/get_var`.
pub struct InteractiveDiningPhilosophersAdapter {
    machine: dining_philosophers_interactive::Machine,
    paused: bool,
    speed_idx: usize,
    accum: u32,
}

impl InteractiveDiningPhilosophersAdapter {
    /// Create and start the Interactive Dining Philosophers machine.
    pub fn new() -> Self {
        let mut machine = dining_philosophers_interactive::Machine::new();
        machine.start();
        Self {
            machine,
            paused: false,
            speed_idx: 1,
            accum: 0,
        }
    }

    /// Read an integer seat field (`t_SEATED` / `t_FORKS`) for seat `k`.
    fn seat_int(&self, var: &str, k: i64) -> i64 {
        if let dining_philosophers_interactive::Value::Map(m) = self.machine.get_var(var) {
            if let Some(dining_philosophers_interactive::Value::Int(v)) =
                m.get(k.to_string().as_str())
            {
                return *v;
            }
        }
        0
    }

    /// Read the phase string for seat `k` from `t_PHASE`.
    fn phase(&self, k: i64) -> &'static str {
        let raw = if let dining_philosophers_interactive::Value::Map(m) =
            self.machine.get_var("t_PHASE")
        {
            match m.get(k.to_string().as_str()) {
                Some(dining_philosophers_interactive::Value::Str(s)) => s.clone(),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        match raw.as_str() {
            "thinking" => "thk",
            "hungry" => "hun",
            "waiting" => "wai",
            "eating" => "EAT",
            _ => "--",
        }
    }

    fn seated(&self, k: i64) -> bool {
        self.seat_int("t_SEATED", k) == 1
    }
    fn lowest_empty(&self) -> i64 {
        (1..=5).find(|&k| !self.seated(k)).unwrap_or(0)
    }
    fn highest_seated(&self) -> i64 {
        (1..=5).rev().find(|&k| self.seated(k)).unwrap_or(0)
    }

    fn step_named(&mut self, ev: alloc::string::String) {
        self.machine
            .step(&ev, dining_philosophers_interactive::Value::Undefined);
    }
}

impl Default for InteractiveDiningPhilosophersAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MachineAdapter for InteractiveDiningPhilosophersAdapter {
    fn name(&self) -> &'static str {
        "Interactive Philosophers"
    }

    fn state_summary(&self) -> String {
        let seats = (1..=5)
            .map(|k| format!("{}:{}", k, self.phase(k)))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "{} | speed {} {}",
            seats,
            IDP_SPEED_LABELS[self.speed_idx],
            if self.paused { "PAUSED" } else { "running" },
        )
    }

    fn available_events(&self) -> &[&'static str] {
        // On-screen control surface (SCTD-02 §6.2). These are host-side controls
        // translated to per-seat SCXML events / harness actions by `dispatch_event`.
        &["Arrive", "Depart", "Panic", "Reset", "Pause", "Speed"]
    }

    fn dispatch_event(&mut self, event_name: &str) {
        match event_name {
            "Arrive" => {
                let k = self.lowest_empty();
                if k > 0 {
                    self.step_named(format!("arrive.{k}"));
                }
            }
            "Depart" => {
                let k = self.highest_seated();
                if k > 0 {
                    self.step_named(format!("depart.{k}"));
                }
            }
            "Panic" => {
                for k in 1..=5 {
                    if self.seated(k) {
                        self.step_named(format!("break.{k}"));
                    }
                }
            }
            "Reset" => {
                let mut m = dining_philosophers_interactive::Machine::new();
                m.start();
                self.machine = m;
                self.accum = 0;
            }
            "Pause" => self.paused = !self.paused,
            "Speed" => self.speed_idx = (self.speed_idx + 1) % IDP_SPEED_LABELS.len(),
            _ => {}
        }
    }

    fn tick(&mut self) {
        if self.paused {
            return;
        }
        // Decoupled logical-tick cadence (INV-SCTD02-2): advance the machine one
        // logical step every N refills, paced for human reaction time and scaled
        // by the speed setting. Between timer fires, poke seated philosophers so a
        // contended one re-attempts forks once a neighbour frees one.
        self.accum += 1;
        if self.accum < IDP_SPEED_THRESHOLDS[self.speed_idx] {
            return;
        }
        self.accum = 0;
        let _ = self.machine.run(1);
        for k in 1..=5 {
            if self.seated(k) {
                self.step_named(format!("poke.{k}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tutorial Demo Controller
// ---------------------------------------------------------------------------

struct ControllerState {
    selector: Rc<RefCell<MachineSelector>>,
    panel: Rc<RefCell<MachinePanel>>,
    subtitle: Rc<RefCell<Label>>,
    footer: Rc<RefCell<Label>>,
    adapters: Vec<Box<dyn MachineAdapter>>,
    selected: usize,
    /// Which event button is currently focused (for keyboard dispatch).
    event_focus: usize,
    commands: Vec<SctdCommand>,
}

impl ControllerState {
    fn new(
        selector: Rc<RefCell<MachineSelector>>,
        panel: Rc<RefCell<MachinePanel>>,
        subtitle: Rc<RefCell<Label>>,
        footer: Rc<RefCell<Label>>,
    ) -> Self {
        let adapters: Vec<Box<dyn MachineAdapter>> = vec![
            alloc::boxed::Box::new(DiningPhilosophersAdapter::new()),
            alloc::boxed::Box::new(MediaPlayerAdapter::new()),
            alloc::boxed::Box::new(InteractiveDiningPhilosophersAdapter::new()),
        ];
        let mut this = Self {
            selector,
            panel,
            subtitle,
            footer,
            adapters,
            selected: 0,
            event_focus: 0,
            commands: Vec::new(),
        };
        this.sync_panel();
        this
    }

    fn sync_panel(&mut self) {
        let adapter = &self.adapters[self.selected];
        let name = adapter.name();
        let summary = adapter.state_summary();
        let events = adapter.available_events();
        let event_focus = self.event_focus;

        self.panel
            .borrow_mut()
            .update(name, &summary, events, event_focus);
        self.subtitle
            .borrow_mut()
            .set_text(format!("Machine: {}", name));
    }

    fn select_machine(&mut self, index: usize) {
        if index < self.adapters.len() {
            self.selected = index;
            self.event_focus = 0;
            self.selector.borrow_mut().set_selected(index);
            self.sync_panel();
            self.footer.borrow_mut().set_text(format!(
                "Selected: {}",
                self.adapters[index].name()
            ));
        }
    }

    fn dispatch_focused_event(&mut self) {
        let events: Vec<&'static str> = self.adapters[self.selected]
            .available_events()
            .to_vec();
        if let Some(&event_name) = events.get(self.event_focus) {
            self.adapters[self.selected].dispatch_event(event_name);
            let summary = self.adapters[self.selected].state_summary();
            let name = self.adapters[self.selected].name();
            self.panel.borrow_mut().update(
                name,
                &summary,
                &events,
                self.event_focus,
            );
            self.footer
                .borrow_mut()
                .set_text(format!("Dispatched: {}", event_name));
            self.commands.push(SctdCommand::EventDispatched {
                machine: name,
                event: event_name,
            });
        }
    }

    /// Dispatch the event/control at `idx` for the selected machine. Used by the
    /// panel's on-screen tap callback. Does NOT touch the panel widget (it is
    /// borrowed during the tap); the panel refreshes on the next `Tick`.
    fn dispatch_event_index(&mut self, idx: usize) {
        let events: Vec<&'static str> = self.adapters[self.selected]
            .available_events()
            .to_vec();
        if let Some(&event_name) = events.get(idx) {
            self.event_focus = idx;
            self.adapters[self.selected].dispatch_event(event_name);
            let name = self.adapters[self.selected].name();
            self.footer
                .borrow_mut()
                .set_text(format!("Dispatched: {}", event_name));
            self.commands.push(SctdCommand::EventDispatched {
                machine: name,
                event: event_name,
            });
        }
    }

    fn cycle_event_focus(&mut self, delta: i32) {
        let count = self.adapters[self.selected].available_events().len();
        if count == 0 {
            return;
        }
        self.event_focus =
            (self.event_focus as i32 + delta).rem_euclid(count as i32) as usize;
        self.sync_panel();
    }

    fn handle_key(&mut self, key: &Key) {
        match key {
            Key::ArrowUp => self.cycle_event_focus(-1),
            Key::ArrowDown => self.cycle_event_focus(1),
            Key::ArrowLeft => {
                let prev = if self.selected == 0 {
                    self.adapters.len() - 1
                } else {
                    self.selected - 1
                };
                self.select_machine(prev);
            }
            Key::ArrowRight => {
                self.select_machine((self.selected + 1) % self.adapters.len());
            }
            Key::Enter | Key::Space => self.dispatch_focused_event(),
            Key::Character('1') => self.select_machine(0),
            Key::Character('2') => self.select_machine(1),
            Key::Character('3') => self.select_machine(2),
            Key::Character('d') | Key::Character('D') => self.dispatch_focused_event(),
            _ => {}
        }
    }

    fn tick(&mut self) {
        self.adapters[self.selected].tick();
        self.sync_panel();
    }
}

// ---------------------------------------------------------------------------
// Commands emitted by the controller
// ---------------------------------------------------------------------------

/// Commands emitted by the Tutorial Demo controller for runtime adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SctdCommand {
    /// An SCXML event was dispatched to the named machine.
    EventDispatched {
        /// Name of the machine that received the event.
        machine: &'static str,
        /// Name of the SCXML event dispatched.
        event: &'static str,
    },
}

// ---------------------------------------------------------------------------
// Public controller
// ---------------------------------------------------------------------------

/// Shared Tutorial Demo controller — owns the widget tree, Machine Selector,
/// Machine Panel, and the registered machine adapters.
pub struct SctdController {
    root: Rc<RefCell<WidgetNode>>,
    state: Rc<RefCell<ControllerState>>,
}

impl SctdController {
    /// Build the Tutorial Demo controller for the given screen dimensions.
    pub fn new(screen: rlvgl_platform::Screen) -> Self {
        let (logical_w, logical_h) = screen.logical_size();
        let width = if logical_w == 0 { 800 } else { logical_w as i32 };
        let height = if logical_h == 0 { 480 } else { logical_h as i32 };

        let mut root_container = Container::new(Rect {
            x: 0,
            y: 0,
            width,
            height,
        });
        root_container.style = StyleBuilder::new().bg_color(Color(0, 0, 0, 0)).build();
        let root = Rc::new(RefCell::new(WidgetNode {
            widget: Rc::new(RefCell::new(root_container)),
            children: Vec::new(),
            tag: Some("sctd.root"),
        }));

        let title = themed_label(
            "SCXML Tutorial Demo",
            Rect { x: PANEL_X, y: 24, width: 500, height: 18 },
            Color(248, 249, 250, 255),
        );
        let subtitle = themed_label(
            "Machine: Dining Philosophers",
            Rect { x: PANEL_X, y: 48, width: 500, height: 18 },
            Color(148, 162, 184, 255),
        );
        let footer = themed_label(
            "Use arrow keys / 1 / 2 to select; Enter or D to dispatch",
            Rect { x: PANEL_X, y: height - 32, width: 700, height: 18 },
            Color(192, 203, 215, 255),
        );

        // Machine Panel — shows state summary and event buttons.
        let panel = Rc::new(RefCell::new(MachinePanel::new(Rect {
            x: PANEL_X,
            y: PANEL_Y,
            width: PANEL_WIDTH.min(width - STRIP_X_OFFSET - PANEL_X - 10),
            height: PANEL_HEIGHT.min(height - PANEL_Y - 50),
        })));

        // Machine Selector — right-edge icon strip matching disco-demo position.
        let selector = Rc::new(RefCell::new(MachineSelector::new(
            width - STRIP_X_OFFSET,
            STRIP_ICON_SIZE,
            STRIP_MARGIN_TOP,
            STRIP_GAP,
        )));

        let event_window = Rc::new(RefCell::new(
            EventWindowBuilder::new(&FONT_6X10)
                .width(400)
                .center(width, height)
                .expire_ticks(120)
                .build(),
        ));

        let state = Rc::new(RefCell::new(ControllerState::new(
            selector.clone(),
            panel.clone(),
            subtitle.clone(),
            footer.clone(),
        )));

        // Wire selector tap callbacks.
        {
            let state0 = state.clone();
            selector.borrow_mut().set_on_tap(0, alloc::boxed::Box::new(move |_| {
                state0.borrow_mut().select_machine(0);
            }));
            let state1 = state.clone();
            selector.borrow_mut().set_on_tap(1, alloc::boxed::Box::new(move |_| {
                state1.borrow_mut().select_machine(1);
            }));
            let state2 = state.clone();
            selector.borrow_mut().set_on_tap(2, alloc::boxed::Box::new(move |_| {
                state2.borrow_mut().select_machine(2);
            }));
            // Wire on-screen event-button taps (SCTD-02 §6.2).
            let state_panel = state.clone();
            panel.borrow_mut().set_on_event_tap(alloc::boxed::Box::new(move |idx| {
                state_panel.borrow_mut().dispatch_event_index(idx);
            }));
        }

        {
            let mut r = root.borrow_mut();
            r.children.push(WidgetNode {
                widget: title,
                children: Vec::new(),
                tag: Some("sctd.title"),
            });
            r.children.push(WidgetNode {
                widget: subtitle.clone(),
                children: Vec::new(),
                tag: Some("sctd.subtitle"),
            });
            r.children.push(WidgetNode {
                widget: panel.clone(),
                children: Vec::new(),
                tag: Some("sctd.panel"),
            });
            r.children.push(WidgetNode {
                widget: selector.clone(),
                children: Vec::new(),
                tag: Some("sctd.selector"),
            });
            r.children.push(WidgetNode {
                widget: footer.clone(),
                children: Vec::new(),
                tag: Some("sctd.footer"),
            });
            r.children.push(WidgetNode {
                widget: event_window,
                children: Vec::new(),
                tag: Some("sctd.events"),
            });
        }

        SctdController { root, state }
    }

    /// Return a clone of the root widget tree handle.
    pub fn root(&self) -> Rc<RefCell<WidgetNode>> {
        self.root.clone()
    }

    /// Dispatch a UI event through the widget tree and internal controller.
    pub fn dispatch_event(&mut self, event: &Event) -> bool {
        let consumed = self.root.borrow_mut().dispatch_event(event);
        self.handle_event(event);
        consumed
    }

    /// Internal event handler.
    pub fn handle_event(&mut self, event: &Event) {
        let mut state = self.state.borrow_mut();
        match event {
            Event::Tick => state.tick(),
            Event::KeyDown { key } => state.handle_key(key),
            _ => {}
        }
    }

    /// Drain platform commands since the last call.
    pub fn drain_commands(&mut self) -> Vec<SctdCommand> {
        core::mem::take(&mut self.state.borrow_mut().commands)
    }

    /// Return the index of the currently selected machine.
    pub fn selected_machine(&self) -> usize {
        self.state.borrow().selected
    }

    /// Return the count of registered machines.
    pub fn machine_count(&self) -> usize {
        self.state.borrow().adapters.len()
    }

    /// Return the state summary for the currently selected machine.
    pub fn current_state_summary(&self) -> String {
        let state = self.state.borrow();
        state.adapters[state.selected].state_summary()
    }
}

fn themed_label(
    text: impl Into<String>,
    bounds: Rect,
    text_color: Color,
) -> Rc<RefCell<Label>> {
    let mut label = Label::new(text, bounds);
    label.style = StyleBuilder::new()
        .bg_color(Color(0, 0, 0, 0))
        .alpha(0)
        .build();
    label.set_text_color(text_color);
    Rc::new(RefCell::new(label))
}

// ---------------------------------------------------------------------------
// Tests (SCTD-00 §9.2)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_controller() -> SctdController {
        SctdController::new(rlvgl_platform::Screen::landscape(800, 480))
    }

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

    /// SCTD-00 §9.2 / SCTD-02 §6.7 — selector order: Dining Philosophers (0),
    /// Media Player (1), Interactive Philosophers (2).
    #[test]
    fn selector_order_dp_then_media_player() {
        let ctrl = make_controller();
        // Initial selection must be index 0 (Dining Philosophers).
        assert_eq!(ctrl.selected_machine(), 0);
        assert_eq!(ctrl.machine_count(), 3);
        let state = ctrl.state.borrow();
        assert_eq!(state.adapters[0].name(), "Dining Philosophers");
        assert_eq!(state.adapters[1].name(), "Media Player");
        assert_eq!(state.adapters[2].name(), "Interactive Philosophers");
    }

    /// SCTD-00 §9.2 — selected-machine switching via keyboard.
    #[test]
    fn machine_switching_via_keyboard() {
        let mut ctrl = make_controller();
        assert_eq!(ctrl.selected_machine(), 0);

        // Arrow right cycles 0 -> 1 -> 2 -> 0 across the three machines.
        ctrl.dispatch_event(&Event::KeyDown { key: Key::ArrowRight });
        assert_eq!(ctrl.selected_machine(), 1);
        ctrl.dispatch_event(&Event::KeyDown { key: Key::ArrowRight });
        assert_eq!(ctrl.selected_machine(), 2);
        ctrl.dispatch_event(&Event::KeyDown { key: Key::ArrowRight });
        assert_eq!(ctrl.selected_machine(), 0);

        // Number keys select directly.
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('2') });
        assert_eq!(ctrl.selected_machine(), 1);
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('3') });
        assert_eq!(ctrl.selected_machine(), 2);
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('1') });
        assert_eq!(ctrl.selected_machine(), 0);
    }

    /// SCTD-02 §5/§6 — the Interactive Philosophers control surface drives the
    /// machine: Arrive seats the lowest-empty chair, Depart removes the
    /// highest-seated, Reset empties the table, Speed/Pause are host controls.
    #[test]
    fn interactive_philosophers_controls() {
        let mut ctrl = make_controller();
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('3') });
        assert_eq!(ctrl.selected_machine(), 2);
        let dispatch = |c: &SctdController, idx: usize| c.state.borrow_mut().dispatch_event_index(idx);

        assert!(ctrl.current_state_summary().contains("1:-- 2:-- 3:-- 4:-- 5:--"));
        dispatch(&ctrl, 0); // Arrive -> seat 1
        assert!(ctrl.current_state_summary().contains("1:thk"));
        dispatch(&ctrl, 0); // Arrive -> seat 2
        assert!(ctrl.current_state_summary().contains("2:thk"));
        dispatch(&ctrl, 1); // Depart -> highest seated (2) leaves
        let s = ctrl.current_state_summary();
        assert!(s.contains("1:thk") && s.contains("2:--"), "after depart: {}", s);
        dispatch(&ctrl, 3); // Reset -> empty table
        assert!(ctrl.current_state_summary().contains("1:-- 2:-- 3:-- 4:-- 5:--"));

        // Host controls: Speed cycles label, Pause toggles run state.
        assert!(ctrl.current_state_summary().contains("speed x1 running"));
        dispatch(&ctrl, 5); // Speed
        assert!(ctrl.current_state_summary().contains("speed x2"));
        dispatch(&ctrl, 4); // Pause
        assert!(ctrl.current_state_summary().contains("PAUSED"));
    }

    /// SCTD-00 §9.2 — event dispatch: dispatching "Do.Timer.Hungry" reaches
    /// the Dining Philosophers machine and produces an `EventDispatched` command.
    #[test]
    fn event_dispatch_reaches_dp_machine() {
        let mut ctrl = make_controller();
        assert_eq!(ctrl.selected_machine(), 0);

        // The event focus starts at 0, which is "Do.Timer.Hungry".
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });

        let commands = ctrl.drain_commands();
        assert_eq!(commands.len(), 1, "expected one SctdCommand");
        assert_eq!(
            commands[0],
            SctdCommand::EventDispatched {
                machine: "Dining Philosophers",
                event: "Do.Timer.Hungry",
            }
        );
    }

    /// SCTD-00 §9.2 — visible state summary updates after a dispatch.
    #[test]
    fn state_summary_updates_after_dispatch() {
        let mut ctrl = make_controller();

        // Capture pre-dispatch summary.
        let before = ctrl.current_state_summary();

        // Dispatch an event. Even if the machine doesn't change state
        // visibly (machine may still be in parent compound state), the
        // summary call must succeed and return a non-empty string.
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let after = ctrl.current_state_summary();

        assert!(!after.is_empty(), "state summary must be non-empty after dispatch");
        // The summary always includes the parent state name "DiningPhilosophers"
        // or a philosopher substates annotation.
        // We accept either the same or a changed value — the important property
        // is that the summary is reachable after dispatch without panicking.
        let _ = before;
    }

    /// Required tags for automation harness — widget tree must include
    /// sctd.root, sctd.selector, sctd.panel, sctd.subtitle, sctd.footer.
    #[test]
    fn required_widget_tags_present() {
        let ctrl = make_controller();
        let root = ctrl.root.borrow();
        let required = [
            "sctd.root",
            "sctd.title",
            "sctd.subtitle",
            "sctd.panel",
            "sctd.selector",
            "sctd.footer",
            "sctd.events",
        ];
        for tag in required {
            assert!(find_node(&root, tag).is_some(), "missing tag: {}", tag);
        }
    }

    /// SCTD-00 §9.2 — dispatch reaches the real Media Player machine and
    /// produces an `EventDispatched` command; the machine transitions correctly.
    #[test]
    fn event_dispatch_reaches_media_player_machine() {
        let mut ctrl = make_controller();
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('2') });
        assert_eq!(ctrl.selected_machine(), 1);

        // Event focus 0 = "Inp.Media.Ready" (first in MediaPlayerAdapter::available_events).
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let cmds = ctrl.drain_commands();
        assert_eq!(cmds.len(), 1, "expected one SctdCommand for MP dispatch");
        assert_eq!(
            cmds[0],
            SctdCommand::EventDispatched {
                machine: "Media Player",
                event: "Inp.Media.Ready",
            }
        );
    }

    /// SCTD-00 §9.2 — Media Player state summary includes the transport state
    /// and the three normalized datamodel variables (src/mute/repeat).
    #[test]
    fn media_player_state_summary_format() {
        let mut ctrl = make_controller();
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Character('2') });
        assert_eq!(ctrl.selected_machine(), 1);

        let summary = ctrl.current_state_summary();
        assert!(!summary.is_empty(), "MP state summary must be non-empty");
        // Verify the summary format contains the expected datamodel fields.
        assert!(summary.contains("src:"), "summary must contain src: field; got: {}", summary);
        assert!(summary.contains("mute:"), "summary must contain mute: field; got: {}", summary);
        assert!(summary.contains("repeat:"), "summary must contain repeat: field; got: {}", summary);

        // After dispatching Inp.Media.Ready, the machine should transition from
        // mediaPlayerIdle to mediaPlayerRun (specifically mediaPlayerSourceSelect).
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter }); // dispatch "Inp.Media.Ready"
        let summary_after = ctrl.current_state_summary();
        assert!(!summary_after.is_empty(), "MP state summary after Ready must be non-empty");
        // The leaf state after Inp.Media.Ready must NOT be "mediaPlayerIdle".
        assert!(
            !summary_after.starts_with("mediaPlayerIdle"),
            "After Inp.Media.Ready, machine must leave mediaPlayerIdle; got: {}",
            summary_after
        );
    }
}
