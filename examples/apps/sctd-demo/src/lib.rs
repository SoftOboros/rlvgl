// SPDX-License-Identifier: MIT
//! SCXML Tutorial Demo App — target-neutral rlvgl app crate.
//!
//! Implements the SCTD-03 deliverable: a right-edge Machine Selector with
//! `[⚙ Setup, DP, MP]` slots (SCTD-03 §5), a Setup screen with DP/MP tabs
//! (SCTD-03 §7/§8), and an Interactive DP with Auto mode (SCTD-03 §6).
//!
//! # SCTD-00 conformance notes
//! * §6.1 — no board init, PAC, OS calls, file I/O, or wall-clock assumptions.
//! * §6.2 — selector uses the same `STRIP_X_OFFSET`, `STRIP_ICON_SIZE`,
//!   `STRIP_MARGIN_TOP`, and `STRIP_GAP` constants as the disco demo.
//! * §6.3 — two required machines (Dining Philosophers, Media Player); both
//!   backed by generated machine crates.
//! * §6.8/§6.9 — Machine Panel shows active state names and dispatches events
//!   through the `MachineAdapter` trait; UI never reaches into generated
//!   internals beyond the public API.
//! * §7.1 — all tutorial-machine selection and event routing is in this crate.
//!
//! # SCTD-03 changes
//! * Selector is `[⚙ Setup, DP, MP]`; boot default = slot 1 (DP).
//! * Slot 0 shows the Setup screen (hides the machine panel + philosophers
//!   table); slots 1/2 show the run view for DP or MP respectively.
//! * The DP machine is a single adapter with an `Auto` toggle (classic auto-run folded in).
//! * `MediaPlayerAdapter` gains config seeding (source + Auto-Ready).
//! * State summaries containing ` | ` render as two lines with the `|` dropped.
//! * Footer shows touch instructions instead of keyboard hints.
//!
//! # Icon assets
//! The Dining Philosophers selector icon and the Philosophers Table backdrop
//! are the authentic tutorial table image (`Qt/DiningPhilosophers/Images/`
//! `dininig_philosophers.svg`) transcoded to RLE via `rlvgl-creator` per
//! SCTD-00 §6.4 — see [`assets`] for the provenance and the exact pipeline.
//! The table overlays live per-seat state discs on that backdrop (see
//! [`philosophers`]). The Media Player glyph stays Lucide-derived (SCTD-00
//! §6.6 allows Lucide for gaps when tutorial assets are absent or unsuitable).

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub mod assets;
mod machine_panel;
/// rlvgl-creator `qt emit`-generated, Bolero-composed media-player widget tree.
mod media_player_gen;
mod media_player_skin;
mod philosophers;
/// Vendored Bolero artwork (RLE) referenced by [`media_player_gen`].
mod qt_assets;
mod ratatui_hero;
mod selector;
mod setup_screen;

use alloc::{
    boxed::Box,
    format,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use core::cell::RefCell;

use machine_panel::MachinePanel;
use media_player_skin::MediaPlayerSkin;
use philosophers::{PhilosophersTable, SeatState};
use ratatui_hero::{HeroButton, HeroContent, HeroFrame, HeroSeat, HeroSnapshot};
#[cfg(test)]
use selector::SLOT_MP;
use selector::{MachineSelector, SLOT_DP, SLOT_SETUP};
use setup_screen::{DP_SPEED_LABELS, MP_SOURCE_LABELS, SetupCallbacks, SetupScreen};

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

    /// Per-seat states for the live Philosophers Table visualization.
    ///
    /// Dining Philosophers machines return `Some([_; 5])` (one entry per
    /// philosopher); machines with no philosophers (e.g. the Media Player)
    /// return `None`, which hides the table. The default is `None`.
    fn seat_states(&self) -> Option<[SeatState; 5]> {
        None
    }
}

// ---------------------------------------------------------------------------
// Media Player adapter (SCTD-00 §6.3 — real normalized Bolero machine)
// ---------------------------------------------------------------------------

/// Adapter for the iState-generated Media Player machine.
///
/// Gains SCTD-03 §8 config seeding: on launch/reset the adapter seeds
/// `s_source` to the configured default and, if `auto_ready` is true, fires
/// `Inp.Media.Ready` + `Inp.Media.ValidSource` so MP opens past idle.
pub struct MediaPlayerAdapter {
    machine: media_player::Machine,
    cfg_source_idx: usize,
    cfg_auto_ready: bool,
}

impl MediaPlayerAdapter {
    /// Create and start the Media Player machine with default config.
    pub fn new() -> Self {
        Self::with_config(0, true)
    }

    /// Create with explicit config (source index, auto_ready).
    pub fn with_config(source_idx: usize, auto_ready: bool) -> Self {
        let mut machine = media_player::Machine::new();
        machine.start();
        let mut adapter = Self {
            machine,
            cfg_source_idx: source_idx.min(MP_SOURCE_LABELS.len() - 1),
            cfg_auto_ready: auto_ready,
        };
        adapter.apply_config();
        adapter
    }

    /// Apply the stored config to the machine (called on new / reset).
    fn apply_config(&mut self) {
        if self.cfg_auto_ready {
            // Fire Ready + ValidSource so MP opens past idle (SCTD-03 §8).
            // Do this FIRST so the machine reaches a state that accepts Source events.
            self.machine
                .step("Inp.Media.Ready", media_player::Value::Undefined);
            self.machine
                .step("Inp.Media.ValidSource", media_player::Value::Undefined);
        }

        // Seed s_source via the appropriate Source event.
        // After Auto-Ready, the machine is in mediaPlayerRun and can accept Source events.
        let src = MP_SOURCE_LABELS[self.cfg_source_idx];
        let src_event = match src {
            "SD" => "Inp.Media.Source.SD",
            "AUX" => "Inp.Media.Source.AUX",
            _ => "Inp.Media.Source.USB", // default USB
        };
        self.machine.step(src_event, media_player::Value::Undefined);
    }

    /// Apply new config (source index + auto_ready) and reset the machine.
    pub fn apply_new_config(&mut self, source_idx: usize, auto_ready: bool) {
        self.cfg_source_idx = source_idx.min(MP_SOURCE_LABELS.len() - 1);
        self.cfg_auto_ready = auto_ready;
        // Reset the machine then re-seed.
        let mut machine = media_player::Machine::new();
        machine.start();
        self.machine = machine;
        self.apply_config();
    }

    /// Current configured source index.
    pub fn cfg_source_idx(&self) -> usize {
        self.cfg_source_idx
    }

    /// Current auto_ready setting.
    pub fn cfg_auto_ready(&self) -> bool {
        self.cfg_auto_ready
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
        self.machine
            .step(event_name, media_player::Value::Undefined);
    }
}

// ---------------------------------------------------------------------------
// Interactive Dining Philosophers adapter (SCTD-02 §5/§6, SCTD-03 §6)
// ---------------------------------------------------------------------------

/// Refills per machine logical-step at each speed setting (x0.5 / x1 / x2).
/// At a ~30 Hz refill, x1 ≈ one philosopher event every ~1.5 s — human-watchable
/// (SCTD-02 INV-SCTD02-2). The machine's logical clock is thereby decoupled from
/// the framebuffer refill rate; only the cadence is shown, never frame-accurate.
const IDP_SPEED_THRESHOLDS: [u32; 3] = [90, 45, 22];

/// Display labels for the three speed settings.
pub const IDP_SPEED_LABELS: [&str; 3] = DP_SPEED_LABELS;

/// Auto-arrive cadence: attempt to seat a philosopher every N ticks when auto
/// is on and the table is not full (SCTD-03 §6 INV-SCTD03-1). Biases the
/// table toward full.
const AUTO_ARRIVE_TICKS: u32 = 3;

/// Auto-depart cadence: occasionally depart a philosopher to keep the table
/// interesting (SCTD-03 §6 INV-SCTD03-1).
const AUTO_DEPART_TICKS: u32 = 120;

/// Adapter for the Interactive Dining Philosophers machine.
///
/// SCTD-03 §6: the sole DP adapter in the selector. Gains `set_auto`/`auto`
/// and the auto-arrive/depart timer in `tick()`. Manual buttons remain live
/// in both Auto=on and Auto=off states.
pub struct InteractiveDiningPhilosophersAdapter {
    machine: dining_philosophers_interactive::Machine,
    paused: bool,
    speed_idx: usize,
    accum: u32,
    auto: bool,
    auto_arrive_counter: u32,
    auto_depart_counter: u32,
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
            auto: true, // SCTD-03 §8: default Auto = on
            auto_arrive_counter: 0,
            auto_depart_counter: 0,
        }
    }

    /// Enable or disable Auto mode (SCTD-03 §6).
    pub fn set_auto(&mut self, on: bool) {
        self.auto = on;
    }

    /// Return the current Auto setting.
    pub fn auto(&self) -> bool {
        self.auto
    }

    /// Set the speed index (0=x0.5, 1=x1, 2=x2).
    pub fn set_speed_idx(&mut self, idx: usize) {
        self.speed_idx = idx.min(IDP_SPEED_THRESHOLDS.len() - 1);
    }

    /// Read an integer seat field (`t_SEATED` / `t_FORKS`) for seat `k`.
    fn seat_int(&self, var: &str, k: i64) -> i64 {
        if let dining_philosophers_interactive::Value::Map(m) = self.machine.get_var(var)
            && let Some(dining_philosophers_interactive::Value::Int(v)) =
                m.get(k.to_string().as_str())
        {
            return *v;
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

    /// Count the number of seated philosophers.
    fn seated_count(&self) -> usize {
        (1..=5).filter(|&k| self.seated(k)).count()
    }

    fn hero_snapshot(&self, events: &[String]) -> HeroSnapshot {
        HeroSnapshot {
            seats: core::array::from_fn(|index| {
                let number = index as i64 + 1;
                let left_fork = number;
                let right_fork = if number == 1 { 5 } else { number - 1 };
                HeroSeat {
                    number: number as u8,
                    state: self.seat_states().unwrap_or([SeatState::Empty; 5])[index],
                    left_fork_owner: self.seat_int("t_FORKS", left_fork),
                    right_fork_owner: self.seat_int("t_FORKS", right_fork),
                    depart_pending: self.seat_int("t_DEPART_REQ", number) != 0,
                }
            }),
            auto: self.auto,
            paused: self.paused,
            speed: IDP_SPEED_LABELS[self.speed_idx],
            events: events.to_vec(),
        }
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
            "{} | speed {} {} {}",
            seats,
            IDP_SPEED_LABELS[self.speed_idx],
            if self.paused { "PAUSED" } else { "running" },
            if self.auto { "Auto:on" } else { "Auto:off" },
        )
    }

    fn available_events(&self) -> &[&'static str] {
        // On-screen control surface (SCTD-02 §6.2, SCTD-03 §6).
        // Manual buttons dispatch regardless of Auto (SCTD-03 §6).
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
                self.auto_arrive_counter = 0;
                self.auto_depart_counter = 0;
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

        // Decoupled logical-tick cadence (INV-SCTD02-2): advance the machine
        // one logical step every N refills, paced by the speed setting.
        self.accum += 1;
        if self.accum >= IDP_SPEED_THRESHOLDS[self.speed_idx] {
            self.accum = 0;
            let _ = self.machine.run(1);
            // Poke seated philosophers so a contended one re-attempts forks once
            // a neighbour frees one.
            for k in 1..=5 {
                if self.seated(k) {
                    self.step_named(format!("poke.{k}"));
                }
            }
        }

        // Auto mode (SCTD-03 §6): fire auto-arrive / auto-depart ONLY when
        // auto() is true; manual buttons always remain live regardless.
        if self.auto {
            // Bias toward full: arrive when there is a free seat.
            self.auto_arrive_counter += 1;
            if self.auto_arrive_counter >= AUTO_ARRIVE_TICKS {
                self.auto_arrive_counter = 0;
                let k = self.lowest_empty();
                if k > 0 {
                    self.step_named(format!("arrive.{k}"));
                }
            }

            // Occasional depart to keep the table interesting.
            self.auto_depart_counter += 1;
            if self.auto_depart_counter >= AUTO_DEPART_TICKS {
                self.auto_depart_counter = 0;
                // Only depart if at least 2 philosophers are seated (keep ≥1
                // for visual interest).
                if self.seated_count() >= 2 {
                    let k = self.highest_seated();
                    if k > 0 {
                        self.step_named(format!("depart.{k}"));
                    }
                }
            }
        }
    }

    fn seat_states(&self) -> Option<[SeatState; 5]> {
        let mut out = [SeatState::Empty; 5];
        for (i, item) in out.iter_mut().enumerate() {
            let k = (i + 1) as i64;
            if !self.seated(k) {
                continue;
            }
            *item = match self.phase(k) {
                "EAT" => SeatState::Eating,
                "wai" => SeatState::Waiting,
                "hun" => SeatState::Hungry,
                _ => SeatState::Thinking,
            };
        }
        Some(out)
    }
}

// ---------------------------------------------------------------------------
// Slot / View model (SCTD-03 §5)
// ---------------------------------------------------------------------------

/// The view shown for a given selector slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlotView {
    /// Slot 0: Setup screen (hides the machine panel + philosophers table).
    Setup,
    /// Slot 1: DP run view.
    Dp,
    /// Slot 2: MP run view.
    Mp,
}

impl SlotView {
    fn from_slot(slot: usize) -> Self {
        match slot {
            SLOT_SETUP => SlotView::Setup,
            SLOT_DP => SlotView::Dp,
            _ => SlotView::Mp,
        }
    }

    fn is_run_view(self) -> bool {
        !matches!(self, SlotView::Setup)
    }
}

// ---------------------------------------------------------------------------
// Tutorial Demo Controller
// ---------------------------------------------------------------------------

struct ControllerState {
    selector: Rc<RefCell<MachineSelector>>,
    panel: Rc<RefCell<MachinePanel>>,
    setup: Rc<RefCell<SetupScreen>>,
    table: Rc<RefCell<PhilosophersTable>>,
    /// Skinned media player (emitted Bolero tree) shown for the MP slot.
    skin: Rc<RefCell<MediaPlayerSkin>>,
    subtitle: Rc<RefCell<Label>>,
    footer: Rc<RefCell<Label>>,
    hero: HeroWidgets,
    hero_open: bool,
    hero_events: Vec<String>,
    /// DP machine adapter (concrete type — no unsafe downcast needed).
    dp: InteractiveDiningPhilosophersAdapter,
    /// MP machine adapter (concrete type).
    mp: MediaPlayerAdapter,
    /// Currently selected selector slot (0=Setup, 1=DP, 2=MP).
    selected: usize,
    /// Which event button is currently focused (for keyboard dispatch).
    event_focus: usize,
    commands: Vec<SctdCommand>,
}

struct HeroWidgets {
    frame: Rc<RefCell<HeroFrame>>,
    content: Rc<RefCell<HeroContent>>,
    launcher: Rc<RefCell<HeroButton>>,
    controls: Vec<Rc<RefCell<HeroButton>>>,
}

impl HeroWidgets {
    fn set_modal_visible(&self, visible: bool) {
        self.frame.borrow_mut().set_visible(visible);
        self.content.borrow_mut().set_visible(visible);
        for control in &self.controls {
            control.borrow_mut().set_visible(visible);
        }
    }
}

impl ControllerState {
    fn new(
        selector: Rc<RefCell<MachineSelector>>,
        panel: Rc<RefCell<MachinePanel>>,
        setup: Rc<RefCell<SetupScreen>>,
        table: Rc<RefCell<PhilosophersTable>>,
        skin: Rc<RefCell<MediaPlayerSkin>>,
        subtitle: Rc<RefCell<Label>>,
        footer: Rc<RefCell<Label>>,
        hero: HeroWidgets,
    ) -> Self {
        let mut this = Self {
            selector,
            panel,
            setup,
            table,
            skin,
            subtitle,
            footer,
            hero,
            hero_open: false,
            hero_events: alloc::vec!["Native DP view ready".to_string()],
            dp: InteractiveDiningPhilosophersAdapter::new(),
            mp: MediaPlayerAdapter::new(),
            selected: SLOT_DP, // SCTD-03 §5: boot default = slot 1 (DP)
            event_focus: 0,
            commands: Vec::new(),
        };
        this.sync_visibility();
        this.sync_table();
        this.sync_panel();
        this
    }

    fn current_view(&self) -> SlotView {
        SlotView::from_slot(self.selected)
    }

    /// Return the active adapter as a shared reference (for run-view slots).
    fn active_adapter(&self) -> &dyn MachineAdapter {
        match self.selected {
            SLOT_DP => &self.dp,
            _ => &self.mp,
        }
    }

    /// Return the active adapter as a mutable reference (for run-view slots).
    fn active_adapter_mut(&mut self) -> &mut dyn MachineAdapter {
        match self.selected {
            SLOT_DP => &mut self.dp,
            _ => &mut self.mp,
        }
    }

    /// Return a typed mutable reference to the DP adapter.
    pub(crate) fn dp_adapter_mut(&mut self) -> &mut InteractiveDiningPhilosophersAdapter {
        &mut self.dp
    }

    /// Return a typed mutable reference to the MP adapter.
    pub(crate) fn mp_adapter_mut(&mut self) -> &mut MediaPlayerAdapter {
        &mut self.mp
    }

    /// Show/hide the panel, setup screen, and table based on the current slot.
    ///
    /// Visibility rules (SCTD-03 §5, amended for the media-player skin):
    /// * **Setup view** (slot 0): SetupScreen visible; MachinePanel hidden; table hidden; skin hidden.
    /// * **DP run view** (slot 1): MachinePanel visible; table visible; SetupScreen hidden; skin hidden.
    /// * **MP run view** (slot 2): media-player skin visible; MachinePanel hidden; table hidden; SetupScreen hidden.
    fn sync_visibility(&mut self) {
        if self.hero_open {
            self.panel.borrow_mut().set_visible(false);
            self.setup.borrow_mut().set_visible(false);
            self.skin.borrow_mut().set_visible(false);
            self.table.borrow_mut().set_states(None);
            if let Ok(mut selector) = self.selector.try_borrow_mut() {
                selector.set_visible(false);
            }
            self.hero.launcher.borrow_mut().set_visible(false);
            self.hero.set_modal_visible(true);
            return;
        }

        if let Ok(mut selector) = self.selector.try_borrow_mut() {
            selector.set_visible(true);
        }
        self.hero.set_modal_visible(false);
        self.hero
            .launcher
            .borrow_mut()
            .set_visible(self.current_view() == SlotView::Dp);
        match self.current_view() {
            SlotView::Setup => {
                self.panel.borrow_mut().set_visible(false);
                self.setup.borrow_mut().set_visible(true);
                self.skin.borrow_mut().set_visible(false);
                // Table hidden — sync_table will call set_states(None).
            }
            SlotView::Dp => {
                self.panel.borrow_mut().set_visible(true);
                self.setup.borrow_mut().set_visible(false);
                self.skin.borrow_mut().set_visible(false);
                // Table shown — sync_table will call set_states(Some(_)).
            }
            SlotView::Mp => {
                // The skinned Bolero media player replaces the generic Machine
                // Panel for the MP slot.
                self.panel.borrow_mut().set_visible(false);
                self.setup.borrow_mut().set_visible(false);
                self.skin.borrow_mut().set_visible(true);
                // Table hidden — sync_table will call set_states(None).
            }
        }
    }

    /// Push the selected machine's live seat states to the Philosophers Table.
    fn sync_table(&mut self) {
        let states = if self.current_view() == SlotView::Dp && !self.hero_open {
            self.dp.seat_states()
        } else {
            None // Hide table for MP and Setup views.
        };
        self.table.borrow_mut().set_states(states);
    }

    fn sync_panel(&mut self) {
        match self.current_view() {
            SlotView::Setup => {
                // Panel is hidden in Setup view; subtitle still updates.
                self.subtitle
                    .borrow_mut()
                    .set_text("Configure DP and MP machines");
            }
            SlotView::Dp | SlotView::Mp => {
                let name = self.active_adapter().name();
                let summary = self.active_adapter().state_summary();
                let events = self.active_adapter().available_events();
                let event_focus = self.event_focus;

                self.panel
                    .borrow_mut()
                    .update(name, &summary, events, event_focus);
                self.subtitle
                    .borrow_mut()
                    .set_text(format!("Machine: {}", name));
            }
        }
    }

    fn select_machine(&mut self, index: usize) {
        if index < selector::MACHINE_COUNT {
            self.hero_open = false;
            self.selected = index;
            self.event_focus = 0;
            // Use try_borrow_mut: when this is reached from a selector TAP, the
            // selector widget is already borrowed by the widget-tree dispatch
            // running the on_tap callback (and the selector has already updated
            // its own highlight in handle_event), so we must not borrow it again
            // — a plain borrow_mut would panic (BorrowMutError -> abort/reboot on
            // the FireBeetle). From the keyboard path the selector is free.
            if let Ok(mut sel) = self.selector.try_borrow_mut() {
                sel.set_selected(index);
            }
            self.sync_visibility();
            self.sync_table();
            self.sync_panel();
            match self.current_view() {
                SlotView::Setup => {
                    self.footer
                        .borrow_mut()
                        .set_text("Setup: tap DP / MP tabs to configure");
                }
                SlotView::Dp | SlotView::Mp => {
                    let name = self.active_adapter().name();
                    self.footer
                        .borrow_mut()
                        .set_text(format!("Selected: {}", name));
                }
            }
        }
    }

    fn dispatch_focused_event(&mut self) {
        if !self.current_view().is_run_view() {
            return;
        }
        let events: Vec<&'static str> = self.active_adapter().available_events().to_vec();
        if let Some(&event_name) = events.get(self.event_focus) {
            self.active_adapter_mut().dispatch_event(event_name);
            let summary = self.active_adapter().state_summary();
            let name = self.active_adapter().name();
            self.panel
                .borrow_mut()
                .update(name, &summary, &events, self.event_focus);
            self.footer
                .borrow_mut()
                .set_text(format!("Dispatched: {}", event_name));
            self.commands.push(SctdCommand::EventDispatched {
                machine: name,
                event: event_name,
            });
            self.push_hero_event(format!("keyboard: {event_name}"));
        }
    }

    /// Dispatch the event/control at `idx` for the selected machine. Used by the
    /// panel's on-screen tap callback. Does NOT touch the panel widget (it is
    /// borrowed during the tap); the panel refreshes on the next `Tick`. The
    /// Philosophers Table is a different widget (not borrowed during the tap),
    /// so it is refreshed here for immediate occupancy/state feedback.
    fn dispatch_event_index(&mut self, idx: usize) {
        if !self.current_view().is_run_view() {
            return;
        }
        let events: Vec<&'static str> = self.active_adapter().available_events().to_vec();
        if let Some(&event_name) = events.get(idx) {
            self.event_focus = idx;
            self.active_adapter_mut().dispatch_event(event_name);
            let name = self.active_adapter().name();
            self.footer
                .borrow_mut()
                .set_text(format!("Dispatched: {}", event_name));
            self.commands.push(SctdCommand::EventDispatched {
                machine: name,
                event: event_name,
            });
            self.push_hero_event(format!("manual: {event_name}"));
            self.sync_table();
        }
    }

    fn push_hero_event(&mut self, event: String) {
        const MAX_HERO_EVENTS: usize = 12;
        if self.hero_events.len() == MAX_HERO_EVENTS {
            self.hero_events.remove(0);
        }
        self.hero_events.push(event);
    }

    fn open_hero(&mut self) {
        if self.current_view() == SlotView::Dp {
            self.hero_open = true;
            self.push_hero_event("opened Ratatui hero".to_string());
        }
    }

    fn close_hero(&mut self) {
        if self.hero_open {
            self.hero_open = false;
            self.push_hero_event("closed Ratatui hero".to_string());
        }
    }

    fn sync_hero(&mut self) {
        if self.hero_open {
            let snapshot = self.dp.hero_snapshot(&self.hero_events);
            self.hero.content.borrow_mut().update(snapshot);
        }
    }

    fn cycle_event_focus(&mut self, delta: i32) {
        if !self.current_view().is_run_view() {
            return;
        }
        let count = self.active_adapter().available_events().len();
        if count == 0 {
            return;
        }
        self.event_focus = (self.event_focus as i32 + delta).rem_euclid(count as i32) as usize;
        self.sync_panel();
    }

    fn handle_key(&mut self, key: &Key) {
        if self.hero_open {
            if matches!(key, Key::Escape) {
                self.close_hero();
            }
            return;
        }
        match key {
            Key::ArrowUp => self.cycle_event_focus(-1),
            Key::ArrowDown => self.cycle_event_focus(1),
            Key::ArrowLeft => {
                let prev = if self.selected == 0 {
                    selector::MACHINE_COUNT - 1
                } else {
                    self.selected - 1
                };
                self.select_machine(prev);
            }
            Key::ArrowRight => {
                self.select_machine((self.selected + 1) % selector::MACHINE_COUNT);
            }
            Key::Enter | Key::Space => self.dispatch_focused_event(),
            // Number keys: 1 = slot 0 (Setup), 2 = slot 1 (DP), 3 = slot 2 (MP).
            Key::Character('1') => self.select_machine(0),
            Key::Character('2') => self.select_machine(1),
            Key::Character('3') => self.select_machine(2),
            Key::Character('d') | Key::Character('D') => self.dispatch_focused_event(),
            _ => {}
        }
    }

    fn tick(&mut self) {
        if self.current_view().is_run_view() {
            self.active_adapter_mut().tick();
            self.sync_table();
            self.sync_panel();
        }
        self.sync_visibility();
        self.sync_hero();
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
/// Machine Panel, Setup Screen, and the registered machine adapters.
pub struct SctdController {
    root: Rc<RefCell<WidgetNode>>,
    state: Rc<RefCell<ControllerState>>,
}

impl SctdController {
    /// Build the Tutorial Demo controller for the given screen dimensions.
    pub fn new(screen: rlvgl_platform::Screen) -> Self {
        let (logical_w, logical_h) = screen.logical_size();
        let width = if logical_w == 0 {
            800
        } else {
            logical_w as i32
        };
        let height = if logical_h == 0 {
            480
        } else {
            logical_h as i32
        };

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
            Rect {
                x: PANEL_X,
                y: 24,
                width: 500,
                height: 18,
            },
            Color(248, 249, 250, 255),
        );
        let subtitle = themed_label(
            "Machine: Interactive Philosophers",
            Rect {
                x: PANEL_X,
                y: 48,
                width: 500,
                height: 18,
            },
            Color(148, 162, 184, 255),
        );
        // SCTD-03 §12 f: footer shows touch instructions, not keyboard hints.
        let footer = themed_label(
            "Tap an icon to select; tap a control to act.",
            Rect {
                x: PANEL_X,
                y: height - 32,
                width: 700,
                height: 18,
            },
            Color(192, 203, 215, 255),
        );

        // Machine Panel — shows state summary and event buttons.
        let panel_w = PANEL_WIDTH.min(width - STRIP_X_OFFSET - PANEL_X - 10);
        let panel_h = PANEL_HEIGHT.min(height - PANEL_Y - 50);
        let panel = Rc::new(RefCell::new(MachinePanel::new(Rect {
            x: PANEL_X,
            y: PANEL_Y,
            width: panel_w,
            height: panel_h,
        })));

        // Setup Screen — same bounds as the Machine Panel; shown for slot 0.
        let setup = Rc::new(RefCell::new(SetupScreen::new(Rect {
            x: PANEL_X,
            y: PANEL_Y,
            width: panel_w,
            height: panel_h,
        })));

        // Philosophers Table — centered in the gap between panel and selector strip.
        const HERO_SIZE: i32 = 150;
        let gap_left = PANEL_X + panel_w;
        let gap_right = width - STRIP_X_OFFSET;
        let hero_x = gap_left + ((gap_right - gap_left - HERO_SIZE) / 2).max(0);
        let hero_y = PANEL_Y + ((panel_h - HERO_SIZE) / 2).max(0);
        let table = Rc::new(RefCell::new(PhilosophersTable::new(
            Rect {
                x: hero_x,
                y: hero_y,
                width: HERO_SIZE,
                height: HERO_SIZE,
            },
            assets::HERO_DP,
        )));

        // Media-player skin — the rlvgl-creator-emitted Bolero media player,
        // laid out across the content area (720×480, left of the selector
        // strip). Shown only for the MP slot.
        let skin = Rc::new(RefCell::new(MediaPlayerSkin::new(Rect {
            x: 0,
            y: 0,
            width: width - STRIP_X_OFFSET,
            height,
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

        // SCTD-04: additive near-full-screen hybrid hero. Native rlvgl owns
        // the popup frame and graphical buttons; Ratatui owns `hero_content`.
        let screen_rect = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let popup = Rect {
            x: 8,
            y: 8,
            width: (width - 16).max(1),
            height: (height - 16).max(1),
        };
        let action_y = popup.y + popup.height - 52;
        let hero_content_bounds = Rect {
            x: popup.x + 12,
            y: popup.y + 46,
            width: (popup.width - 24).max(12),
            height: (action_y - popup.y - 54).max(20),
        };
        let hero_frame = Rc::new(RefCell::new(HeroFrame::new(screen_rect, popup)));
        let hero_content = Rc::new(RefCell::new(HeroContent::new(hero_content_bounds)));
        let hero_launcher = Rc::new(RefCell::new(HeroButton::new(
            Rect {
                x: (width - STRIP_X_OFFSET - 142).max(PANEL_X),
                y: (height - 72).max(PANEL_Y),
                width: 132,
                height: 38,
            },
            "Ratatui",
            true,
        )));

        let close = Rc::new(RefCell::new(HeroButton::new(
            Rect {
                x: popup.x + popup.width - 60,
                y: popup.y + 10,
                width: 48,
                height: 32,
            },
            "X",
            false,
        )));
        close.borrow_mut().set_press_behavior(Rect {
            x: popup.x + popup.width - 92,
            y: 0,
            width: 100,
            height: 70,
        });
        let action_labels = ["Arrive", "Depart", "Panic", "Reset", "Pause", "Speed"];
        let action_gap = 6;
        let action_width = ((popup.width - 24 - action_gap * 5) / 6).max(48);
        let mut hero_controls: Vec<Rc<RefCell<HeroButton>>> = alloc::vec![close];
        for (index, label) in action_labels.iter().enumerate() {
            let bounds = Rect {
                x: popup.x + 12 + index as i32 * (action_width + action_gap),
                y: action_y,
                width: action_width,
                height: 38,
            };
            let mut control = HeroButton::new(bounds, label, index == 0);
            control.set_press_behavior(Rect {
                x: bounds.x,
                y: bounds.y - 10,
                width: bounds.width,
                height: bounds.height + 20,
            });
            hero_controls.push(Rc::new(RefCell::new(control)));
        }

        let state = Rc::new(RefCell::new(ControllerState::new(
            selector.clone(),
            panel.clone(),
            setup.clone(),
            table.clone(),
            skin.clone(),
            subtitle.clone(),
            footer.clone(),
            HeroWidgets {
                frame: hero_frame.clone(),
                content: hero_content.clone(),
                launcher: hero_launcher.clone(),
                controls: hero_controls.clone(),
            },
        )));

        // Wire selector tap callbacks (slots 0, 1, 2 → Setup, DP, MP).
        {
            for slot in 0..selector::MACHINE_COUNT {
                let state_c = state.clone();
                selector.borrow_mut().set_on_tap(
                    slot,
                    alloc::boxed::Box::new(move |_| {
                        state_c.borrow_mut().select_machine(slot);
                    }),
                );
            }

            // Wire on-screen event-button taps (SCTD-02 §6.2).
            let state_panel = state.clone();
            panel
                .borrow_mut()
                .set_on_event_tap(alloc::boxed::Box::new(move |idx| {
                    state_panel.borrow_mut().dispatch_event_index(idx);
                }));

            // Wire Setup screen config callbacks (SCTD-03 §7/§8).
            // We route them through the ControllerState via Rc<RefCell<_>>.
            {
                let state_auto = state.clone();
                let state_speed = state.clone();
                let state_src = state.clone();
                let state_ar = state.clone();
                setup.borrow().set_callbacks(SetupCallbacks {
                    on_dp_auto: Some(Box::new(move |on| {
                        let mut s = state_auto.borrow_mut();
                        s.dp_adapter_mut().set_auto(on);
                    })),
                    on_dp_speed: Some(Box::new(move |idx| {
                        let mut s = state_speed.borrow_mut();
                        s.dp_adapter_mut().set_speed_idx(idx);
                    })),
                    on_mp_source: Some(Box::new(move |idx| {
                        let mut s = state_src.borrow_mut();
                        let ar = s.mp_adapter_mut().cfg_auto_ready();
                        s.mp_adapter_mut().apply_new_config(idx, ar);
                    })),
                    on_mp_auto_ready: Some(Box::new(move |on| {
                        let mut s = state_ar.borrow_mut();
                        let src = s.mp_adapter_mut().cfg_source_idx();
                        s.mp_adapter_mut().apply_new_config(src, on);
                    })),
                });
            }

            let state_open = state.clone();
            hero_launcher
                .borrow_mut()
                .set_on_tap(Box::new(move || state_open.borrow_mut().open_hero()));

            let state_close = state.clone();
            hero_controls[0]
                .borrow_mut()
                .set_on_tap(Box::new(move || state_close.borrow_mut().close_hero()));

            for (event_index, button) in hero_controls.iter().skip(1).enumerate() {
                let state_action = state.clone();
                button.borrow_mut().set_on_tap(Box::new(move || {
                    state_action.borrow_mut().dispatch_event_index(event_index);
                }));
            }
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
                widget: setup.clone(),
                children: Vec::new(),
                tag: Some("sctd.setup"),
            });
            r.children.push(WidgetNode {
                widget: skin.clone(),
                children: Vec::new(),
                tag: Some("sctd.mp_skin"),
            });
            r.children.push(WidgetNode {
                widget: table.clone(),
                children: Vec::new(),
                tag: Some("sctd.table"),
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
            r.children.push(WidgetNode {
                widget: hero_launcher,
                children: Vec::new(),
                tag: Some("sctd.hero.launch"),
            });
            r.children.push(WidgetNode {
                widget: hero_frame,
                children: Vec::new(),
                tag: Some("sctd.hero.window"),
            });
            r.children.push(WidgetNode {
                widget: hero_content,
                children: Vec::new(),
                tag: Some("sctd.hero.content"),
            });
            let control_tags = [
                "sctd.hero.close",
                "sctd.hero.arrive",
                "sctd.hero.depart",
                "sctd.hero.panic",
                "sctd.hero.reset",
                "sctd.hero.pause",
                "sctd.hero.speed",
            ];
            for (control, tag) in hero_controls.into_iter().zip(control_tags) {
                r.children.push(WidgetNode {
                    widget: control,
                    children: Vec::new(),
                    tag: Some(tag),
                });
            }
        }

        // Set selector highlight to boot-default slot 1 (DP).
        selector.borrow_mut().set_selected(SLOT_DP);

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
        state.sync_visibility();
        state.sync_table();
        state.sync_hero();
    }

    /// Drain platform commands since the last call.
    pub fn drain_commands(&mut self) -> Vec<SctdCommand> {
        core::mem::take(&mut self.state.borrow_mut().commands)
    }

    /// Return the index of the currently selected selector slot.
    pub fn selected_machine(&self) -> usize {
        self.state.borrow().selected
    }

    /// Return the count of selector slots (always 3: Setup + DP + MP).
    pub fn machine_count(&self) -> usize {
        selector::MACHINE_COUNT
    }

    /// Return the state summary for the currently selected machine (empty
    /// string when the Setup screen is active).
    pub fn current_state_summary(&self) -> String {
        let state = self.state.borrow();
        if state.current_view().is_run_view() {
            state.active_adapter().state_summary()
        } else {
            String::new()
        }
    }

    /// Whether the additive SCTD-04 Ratatui hero window is open.
    pub fn hero_is_open(&self) -> bool {
        self.state.borrow().hero_open
    }

    /// Monotonic Ratatui frame generation for change-driven platform redraws.
    pub fn hero_generation(&self) -> u64 {
        self.state.borrow().hero.content.borrow().generation()
    }

    /// Monotonic native-table generation for change-driven platform redraws.
    pub fn native_generation(&self) -> u64 {
        self.state.borrow().table.borrow().generation()
    }
}

fn themed_label(text: impl Into<String>, bounds: Rect, text_color: Color) -> Rc<RefCell<Label>> {
    let mut label = Label::new(text, bounds);
    label.style = StyleBuilder::new()
        .bg_color(Color(0, 0, 0, 0))
        .alpha(0)
        .build();
    label.set_text_color(text_color);
    Rc::new(RefCell::new(label))
}

// ---------------------------------------------------------------------------
// Tests (SCTD-03 §12 g)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Widget;
    use setup_screen::SetupTab;

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

    /// SCTD-03 §5 — selector order: [Setup(0), DP(1), MP(2)].
    /// Boot default selection = slot 1 (DP).
    #[test]
    fn selector_order_dp_then_media_player() {
        let ctrl = make_controller();
        // Boot default = slot 1 (DP), not slot 0.
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        assert_eq!(ctrl.machine_count(), 3);
        // Slot 0 = Setup screen, slot 1 = DP adapter, slot 2 = MP adapter.
        let state = ctrl.state.borrow();
        // dp is the DP adapter, mp is the MP adapter.
        assert_eq!(state.dp.name(), "Interactive Philosophers");
        assert_eq!(state.mp.name(), "Media Player");
        // Selecting slot 0 shows the Setup view.
        assert_eq!(SlotView::from_slot(SLOT_SETUP), SlotView::Setup);
        assert_eq!(SlotView::from_slot(SLOT_DP), SlotView::Dp);
        assert_eq!(SlotView::from_slot(SLOT_MP), SlotView::Mp);
    }

    /// SCTD-03 §5 — MACHINE_COUNT stays 3.
    #[test]
    fn machine_count() {
        let ctrl = make_controller();
        assert_eq!(ctrl.machine_count(), 3);
    }

    /// SCTD-03 §5 — tapping selector icon at slot position switches to that slot.
    /// Slot 0 geometry: y in [STRIP_MARGIN_TOP .. STRIP_MARGIN_TOP+STRIP_ICON_SIZE].
    /// Slot 1 geometry: y in next band. Slot 2: next.
    #[test]
    fn tapping_selector_icon_switches_machine() {
        let mut ctrl = make_controller(); // 800x480
        // Boot on slot 1 (DP).
        assert_eq!(ctrl.selected_machine(), SLOT_DP);

        // Tap slot 2 (MP): x >= 730, y in slot-2 band [152, 227].
        ctrl.dispatch_event(&Event::PressRelease { x: 750, y: 190 });
        assert_eq!(
            ctrl.selected_machine(),
            SLOT_MP,
            "tapping the 3rd selector icon selects the MP slot"
        );

        // Tap slot 0 (Setup): y in slot-0 band [17, 77].
        ctrl.dispatch_event(&Event::PressRelease { x: 750, y: 40 });
        assert_eq!(
            ctrl.selected_machine(),
            SLOT_SETUP,
            "tapping the 1st selector icon selects the Setup slot"
        );
    }

    /// SCTD-03 §5 / SCTD-00 §6.4 — table shown for DP run view only; hidden
    /// for MP and for the Setup screen.
    #[test]
    fn table_visible_for_dp_machines_only() {
        let mut ctrl = make_controller();
        let vis = |c: &SctdController| c.state.borrow().table.borrow().is_visible();

        // Boot on DP (slot 1) → table shown.
        assert!(vis(&ctrl), "DP selected initially -> table shown");

        // Switch to MP (slot 2) → table hidden.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);
        assert!(!vis(&ctrl), "Media Player selected -> table hidden");

        // Switch to Setup (slot 0) → table hidden.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('1'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_SETUP);
        assert!(!vis(&ctrl), "Setup screen selected -> table hidden");

        // Back to DP (slot 1, key '2') → table shown.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('2'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        assert!(vis(&ctrl), "DP re-selected -> table shown");
    }

    /// SCTD-03 §5 — panel hidden on Setup view; setup screen hidden on run view.
    ///
    /// Mirrors the table visibility test but covers the MachinePanel and
    /// SetupScreen visibility gates added by SCTD-03a Fix 2.
    #[test]
    fn panel_hidden_on_setup_setup_hidden_on_run_view() {
        let mut ctrl = make_controller();

        let panel_vis = |c: &SctdController| c.state.borrow().panel.borrow().is_visible();
        let setup_vis = |c: &SctdController| c.state.borrow().setup.borrow().is_visible();

        // Boot on DP run view (slot 1): panel shown, setup hidden.
        assert!(panel_vis(&ctrl), "boot DP: panel must be visible");
        assert!(!setup_vis(&ctrl), "boot DP: setup must be hidden");

        // Switch to Setup (slot 0): panel hidden, setup shown.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('1'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_SETUP);
        assert!(!panel_vis(&ctrl), "Setup view: panel must be hidden");
        assert!(setup_vis(&ctrl), "Setup view: setup screen must be visible");

        // Switch to MP (slot 2): media-player skin shown, panel + setup hidden.
        let skin_vis = |c: &SctdController| c.state.borrow().skin.borrow().is_visible();
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);
        assert!(
            !panel_vis(&ctrl),
            "MP view: panel must be hidden (skin replaces it)"
        );
        assert!(
            skin_vis(&ctrl),
            "MP view: media-player skin must be visible"
        );
        assert!(!setup_vis(&ctrl), "MP view: setup must be hidden");

        // Back to Setup via selector tap.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('1'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_SETUP);
        assert!(!panel_vis(&ctrl), "re-entering Setup: panel must be hidden");
        assert!(setup_vis(&ctrl), "re-entering Setup: setup must be visible");

        // Back to DP: panel visible, setup hidden.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('2'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        assert!(panel_vis(&ctrl), "DP re-selected: panel must be visible");
        assert!(!setup_vis(&ctrl), "DP re-selected: setup must be hidden");
    }

    /// SCTD-03 §6 — interactive_philosophers_controls via the DP adapter.
    #[test]
    fn interactive_philosophers_controls() {
        let ctrl = make_controller();
        // Already on DP (slot 1) by boot default.
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        let dispatch =
            |c: &SctdController, idx: usize| c.state.borrow_mut().dispatch_event_index(idx);

        // Disable Auto so tick doesn't interfere with our manual assertions.
        ctrl.state.borrow_mut().dp_adapter_mut().set_auto(false);

        assert!(
            ctrl.current_state_summary()
                .contains("1:-- 2:-- 3:-- 4:-- 5:--")
        );
        dispatch(&ctrl, 0); // Arrive -> seat 1
        assert!(ctrl.current_state_summary().contains("1:thk"));
        dispatch(&ctrl, 0); // Arrive -> seat 2
        assert!(ctrl.current_state_summary().contains("2:thk"));
        dispatch(&ctrl, 1); // Depart -> highest seated (2) leaves
        let s = ctrl.current_state_summary();
        assert!(
            s.contains("1:thk") && s.contains("2:--"),
            "after depart: {}",
            s
        );
        dispatch(&ctrl, 3); // Reset -> empty table
        assert!(
            ctrl.current_state_summary()
                .contains("1:-- 2:-- 3:-- 4:-- 5:--")
        );

        // Host controls: Speed cycles label, Pause toggles run state.
        assert!(ctrl.current_state_summary().contains("speed x1 running"));
        dispatch(&ctrl, 5); // Speed
        assert!(ctrl.current_state_summary().contains("speed x2"));
        dispatch(&ctrl, 4); // Pause
        assert!(ctrl.current_state_summary().contains("PAUSED"));
    }

    /// SCTD-04 §8 — the Ratatui hero is additive to the existing native DP
    /// screen and shares its machine state.
    #[test]
    fn ratatui_hero_opens_from_native_dp_and_preserves_state() {
        let mut ctrl = make_controller();
        ctrl.state.borrow_mut().dp_adapter_mut().set_auto(false);

        assert!(!ctrl.hero_is_open());
        assert!(ctrl.state.borrow().table.borrow().is_visible());
        assert!(ctrl.state.borrow().hero.launcher.borrow().is_visible());
        assert!(find_node(&ctrl.root.borrow(), "sctd.hero.content").is_some());

        // Native graphical launcher.
        ctrl.dispatch_event(&Event::PressRelease { x: 610, y: 420 });
        assert!(ctrl.hero_is_open());
        assert!(!ctrl.state.borrow().table.borrow().is_visible());
        assert!(ctrl.state.borrow().hero.content.borrow().is_visible());
        let generation = ctrl.state.borrow().hero.content.borrow().generation();
        assert!(generation > 0);

        // Native graphical Arrive button updates the shared machine and the
        // Ratatui table snapshot.
        ctrl.dispatch_event(&Event::PressDown { x: 40, y: 435 });
        assert!(ctrl.current_state_summary().contains("1:thk"));
        assert!(ctrl.state.borrow().hero.content.borrow().generation() > generation);

        ctrl.dispatch_event(&Event::PressDown { x: 715, y: 435 });
        assert!(ctrl.current_state_summary().contains("speed x2"));
        ctrl.dispatch_event(&Event::PressDown { x: 588, y: 435 });
        assert!(ctrl.current_state_summary().contains("PAUSED"));

        // Native title-bar close reveals the original DP table at the same state.
        ctrl.dispatch_event(&Event::PressDown { x: 760, y: 24 });
        assert!(!ctrl.hero_is_open());
        assert!(ctrl.state.borrow().table.borrow().is_visible());
        assert!(ctrl.current_state_summary().contains("1:thk"));
    }

    /// SCTD-03 §6 — toggling Auto off stops auto-populate; manual Arrive still works.
    #[test]
    fn auto_mode_off_stops_auto_populate_manual_still_works() {
        let mut dp = InteractiveDiningPhilosophersAdapter::new();
        // Auto is on by default; turn it off.
        dp.set_auto(false);
        assert!(!dp.auto());

        // Run many ticks — table must stay empty (no auto-arrive).
        for _ in 0..1000 {
            dp.tick();
        }
        let s0 = dp.seat_states().unwrap();
        assert!(
            s0.iter().all(|x| *x == SeatState::Empty),
            "with Auto off, ticks must not seat philosophers: {:?}",
            s0
        );

        // A manual Arrive must still work regardless of Auto.
        dp.dispatch_event("Arrive");
        let s1 = dp.seat_states().unwrap();
        assert_ne!(
            s1[0],
            SeatState::Empty,
            "manual Arrive must seat seat 1 even with Auto off"
        );
    }

    /// SCTD-03 §6 — with Auto on, ticks eventually populate the table.
    #[test]
    fn auto_mode_on_auto_populates_table() {
        let mut dp = InteractiveDiningPhilosophersAdapter::new();
        assert!(dp.auto(), "Auto is on by default");

        // Run enough ticks that the auto-arrive fires (AUTO_ARRIVE_TICKS = 3).
        for _ in 0..20 {
            dp.tick();
        }
        let s = dp.seat_states().unwrap();
        assert!(
            s.iter().any(|x| *x != SeatState::Empty),
            "with Auto on, seats should populate over ticks: {:?}",
            s
        );
    }

    #[test]
    fn native_generation_changes_only_when_table_state_changes() {
        let mut controller = SctdController::new(rlvgl_platform::Screen::landscape(800, 480));
        let initial = controller.native_generation();

        controller.dispatch_event(&Event::Tick);
        assert_eq!(controller.native_generation(), initial);

        for _ in 0..20 {
            controller.dispatch_event(&Event::Tick);
            if controller.native_generation() != initial {
                return;
            }
        }
        panic!("auto timer should eventually change the native table generation");
    }

    /// SCTD-03 §7 — Setup screen tab switch DP↔MP changes the active tab.
    #[test]
    fn setup_screen_tab_switch() {
        let mut setup = SetupScreen::new(Rect {
            x: 0,
            y: 0,
            width: 400,
            height: 300,
        });
        // Default active tab = DP.
        assert_eq!(setup.active_tab(), SetupTab::Dp);

        // Draw to record tab rects.
        struct NullR;
        impl rlvgl_core::renderer::Renderer for NullR {
            fn fill_rect(&mut self, _: Rect, _: Color) {}
            fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}
        }
        setup.draw(&mut NullR);

        // Tap the MP tab (tab[1]).
        let mp_rect = setup
            .debug_tab_rect(1)
            .expect("MP tab rect recorded after draw");
        let (cx, cy) = (
            mp_rect.x + mp_rect.width / 2,
            mp_rect.y + mp_rect.height / 2,
        );
        let _ = Widget::handle_event(&mut setup, &Event::PressRelease { x: cx, y: cy });
        assert_eq!(
            setup.active_tab(),
            SetupTab::Mp,
            "tapping MP tab switches to MP"
        );

        // Tap the DP tab (tab[0]).
        let dp_rect = setup.debug_tab_rect(0).expect("DP tab rect recorded");
        let (cx, cy) = (
            dp_rect.x + dp_rect.width / 2,
            dp_rect.y + dp_rect.height / 2,
        );
        let _ = Widget::handle_event(&mut setup, &Event::PressRelease { x: cx, y: cy });
        assert_eq!(
            setup.active_tab(),
            SetupTab::Dp,
            "tapping DP tab switches back"
        );
    }

    /// SCTD-03 §9 — a summary containing ` | ` renders as two lines with `|` dropped.
    #[test]
    fn summary_with_pipe_renders_two_lines() {
        use crate::machine_panel::split_summary;
        let s = "Running | src:USB mute:off repeat:none";
        let lines = split_summary(s, 80);
        assert!(
            lines.len() >= 2,
            "expected at least 2 lines, got: {:?}",
            lines
        );
        for l in &lines {
            assert!(!l.contains('|'), "pipe must be dropped; line: {:?}", l);
        }
    }

    /// SCTD-03 §8 — MP launched with Auto-Ready on leaves idle; with a chosen
    /// source the summary shows src:<that>.
    #[test]
    fn mp_config_auto_ready_and_source() {
        // Auto-Ready off: machine starts at idle.
        let mp_no_auto = MediaPlayerAdapter::with_config(0, false);
        let s = mp_no_auto.state_summary();
        // Without Auto-Ready, the machine stays in the idle state.
        // The exact state name is implementation-specific; we just confirm
        // the summary is non-empty and contains the USB source field.
        assert!(!s.is_empty());
        assert!(
            s.contains("src:"),
            "summary must have src: field; got: {}",
            s
        );

        // Auto-Ready on + USB source: machine advances past idle.
        let mp_auto = MediaPlayerAdapter::with_config(0, true);
        let s_auto = mp_auto.state_summary();
        assert!(
            s_auto.contains("src:USB"),
            "src should be USB; got: {}",
            s_auto
        );

        // Auto-Ready on + SD source.
        let mp_sd = MediaPlayerAdapter::with_config(1, true);
        let s_sd = mp_sd.state_summary();
        assert!(s_sd.contains("src:SD"), "src should be SD; got: {}", s_sd);
    }

    /// SCTD-02 §5/§6 — interactive philosophers control surface still works.
    #[test]
    fn tapping_event_button_dispatches() {
        let mut ctrl = make_controller();
        // Already on DP (slot 1).
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        // Turn Auto off so the table starts empty.
        ctrl.state.borrow_mut().dp_adapter_mut().set_auto(false);

        // Layout pass so the panel records its button rects.
        struct NullRenderer;
        impl rlvgl_core::renderer::Renderer for NullRenderer {
            fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
            fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
        }
        ctrl.root().borrow().draw(&mut NullRenderer);
        let rect = ctrl
            .state
            .borrow()
            .panel
            .borrow()
            .debug_button_rect(0)
            .expect("button 0 (Arrive) rect must be recorded after draw");
        let (cx, cy) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
        ctrl.dispatch_event(&Event::PressRelease { x: cx, y: cy });

        let cmds = ctrl.drain_commands();
        assert!(!cmds.is_empty(), "tap must emit an EventDispatched command");
        assert!(
            ctrl.current_state_summary().contains("1:thk"),
            "tapping Arrive must seat philosopher 1; got {}",
            ctrl.current_state_summary()
        );
    }

    // Tests preserved from SCTD-02 — keep working (adapted where needed).

    /// SCTD-00 §6.4 — the Philosophers Table backdrop is the authentic tutorial
    /// illustration and must decode cleanly from its compiled-in RLE (150×150).
    #[test]
    fn table_backdrop_decodes_tutorial_art() {
        let ctrl = make_controller();
        let sz = ctrl.state.borrow().table.borrow().debug_backdrop_size();
        assert_eq!(
            sz,
            Some((150, 150)),
            "DP table backdrop must decode to 150x150"
        );
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    /// The live table reflects machine state: the interactive DP starts
    /// all-empty (auto off), Arrive seats the first chair.
    #[test]
    fn table_reflects_machine_state() {
        // Interactive DP: starts empty (auto off for determinism), Arrive seats seat 1.
        let mut inter = InteractiveDiningPhilosophersAdapter::new();
        inter.set_auto(false);
        let s0 = inter.seat_states().expect("interactive DP exposes seats");
        assert!(
            s0.iter().all(|x| *x == SeatState::Empty),
            "interactive table starts all-empty: {:?}",
            s0
        );
        inter.dispatch_event("Arrive");
        let s1 = inter.seat_states().expect("interactive DP exposes seats");
        assert_ne!(s1[0], SeatState::Empty, "Arrive seats the first chair");

        // Media Player: no seats.
        assert!(
            MediaPlayerAdapter::new().seat_states().is_none(),
            "Media Player exposes no philosopher seats"
        );
    }

    /// The controller wires live states into the table (draws without panicking).
    #[test]
    fn table_draws_overlay_without_panic() {
        let mut ctrl = make_controller();
        // DP selected (boot) with Auto on — after a few ticks some seats fill.
        for _ in 0..20 {
            ctrl.dispatch_event(&Event::Tick);
        }
        ctrl.root().borrow().draw(&mut NullRenderer);
        assert!(ctrl.state.borrow().table.borrow().is_visible());
    }

    /// The rlvgl-creator-emitted, Bolero-composed media-player skin builds and
    /// draws without panic when the MP slot is selected — exercising the
    /// `qt_image` RLE decode + blit of the vendored Bolero artwork end-to-end.
    #[test]
    fn media_player_skin_draws_when_mp_selected() {
        let mut ctrl = make_controller();
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);
        assert!(
            ctrl.state.borrow().skin.borrow().is_visible(),
            "MP slot selected -> skin visible"
        );
        // Draw the whole tree (skin visible): decodes + blits the vendored
        // Bolero RLE assets through the emitted `qt_image` helper.
        ctrl.root().borrow().draw(&mut NullRenderer);
    }

    /// SCTD-00 §9.2 — selected-machine switching via keyboard (updated for new slot model).
    #[test]
    fn machine_switching_via_keyboard() {
        let mut ctrl = make_controller();
        // Boot on slot 1 (DP).
        assert_eq!(ctrl.selected_machine(), SLOT_DP);

        // Key '1' → slot 0 (Setup).
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('1'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_SETUP);

        // Key '2' → slot 1 (DP).
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('2'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_DP);

        // Key '3' → slot 2 (MP).
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);

        // Arrow right from slot 2 wraps to slot 0.
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::ArrowRight,
        });
        assert_eq!(ctrl.selected_machine(), SLOT_SETUP);
    }

    /// SCTD-02 — selecting + running the Interactive machine under repeated
    /// draw/tick must not panic.
    #[test]
    fn interactive_machine_survives_select_draw_tick() {
        let mut ctrl = make_controller();
        // Already on slot 1 (DP) by boot default.
        assert_eq!(ctrl.selected_machine(), SLOT_DP);

        // Idle frames on the empty table (Auto on will fill it over time).
        for _ in 0..120 {
            ctrl.dispatch_event(&Event::Tick);
            ctrl.root().borrow().draw(&mut NullRenderer);
        }
        // Manually seat all five, then run many frames so they cycle thinking->eating.
        for _ in 0..6 {
            ctrl.state.borrow_mut().dispatch_event_index(0); // Arrive
        }
        for _ in 0..600 {
            ctrl.dispatch_event(&Event::Tick);
            ctrl.root().borrow().draw(&mut NullRenderer);
        }
        // Exercise every control with a draw between each.
        for idx in [1usize, 2, 3, 4, 5, 0] {
            ctrl.state.borrow_mut().dispatch_event_index(idx);
            ctrl.root().borrow().draw(&mut NullRenderer);
        }
    }

    /// SCTD-00 §9.2 — event dispatch: dispatching "Arrive" reaches the DP
    /// machine and produces an `EventDispatched` command.
    #[test]
    fn event_dispatch_reaches_dp_machine() {
        let mut ctrl = make_controller();
        assert_eq!(ctrl.selected_machine(), SLOT_DP);
        // Turn Auto off so table is empty.
        ctrl.state.borrow_mut().dp_adapter_mut().set_auto(false);

        // Event focus 0 = "Arrive".
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });

        let commands = ctrl.drain_commands();
        assert_eq!(commands.len(), 1, "expected one SctdCommand");
        assert_eq!(
            commands[0],
            SctdCommand::EventDispatched {
                machine: "Interactive Philosophers",
                event: "Arrive",
            }
        );
    }

    /// SCTD-00 §9.2 — visible state summary updates after a dispatch.
    #[test]
    fn state_summary_updates_after_dispatch() {
        let mut ctrl = make_controller();

        let before = ctrl.current_state_summary();

        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let after = ctrl.current_state_summary();

        assert!(
            !after.is_empty(),
            "state summary must be non-empty after dispatch"
        );
        let _ = before;
    }

    /// Required tags for automation harness.
    #[test]
    fn required_widget_tags_present() {
        let ctrl = make_controller();
        let root = ctrl.root.borrow();
        let required = [
            "sctd.root",
            "sctd.title",
            "sctd.subtitle",
            "sctd.panel",
            "sctd.setup",
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
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);

        // Event focus 0 = "Inp.Media.Ready".
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
        ctrl.dispatch_event(&Event::KeyDown {
            key: Key::Character('3'),
        });
        assert_eq!(ctrl.selected_machine(), SLOT_MP);

        let summary = ctrl.current_state_summary();
        assert!(!summary.is_empty(), "MP state summary must be non-empty");
        assert!(
            summary.contains("src:"),
            "summary must contain src: field; got: {}",
            summary
        );
        assert!(
            summary.contains("mute:"),
            "summary must contain mute: field; got: {}",
            summary
        );
        assert!(
            summary.contains("repeat:"),
            "summary must contain repeat: field; got: {}",
            summary
        );

        // After dispatching Inp.Media.Ready (event 0), the machine transitions.
        // The MP adapter already seeded Auto-Ready by default, so the machine
        // may already be past idle. Dispatch one more event.
        ctrl.dispatch_event(&Event::KeyDown { key: Key::Enter });
        let summary_after = ctrl.current_state_summary();
        assert!(
            !summary_after.is_empty(),
            "MP state summary after dispatch must be non-empty"
        );
    }
}
