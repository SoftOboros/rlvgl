// SPDX-License-Identifier: MIT
//! QT-05b test fixture: mock istate-codegen output for the
//! `stopwatch.scjson` fixture.
//!
//! This crate hand-implements the **QT-05 §6 6-symbol linkage
//! surface** (`Machine`, `Machine::new`/`with_options`/`dispatch`,
//! `Event`, `State`, `DataModel`, `Externals`, `DefaultExternals`)
//! so the QT-05b compile-as-mod gate can build without the live
//! softoboros istate-codegen MCP. Its semantics are deliberately
//! incomplete — only enough to drive the `stopwatch.qml` /
//! `stopwatch.scjson` fixture's start/stop/reset transitions — but
//! the **shape** of every public symbol matches what the istate
//! Rust template (`backend/templates/codegen/rust/src/lib.rs.jinja2`)
//! emits at scaffold v1.
//!
//! Production users replace this crate with the actual
//! istate-codegen output. The QT-05 chapter §7 names the canonical
//! consumer location: `crates/<sm>_gen/`.
//!
//! Per QT-05 §6 the linkage v1 profile requires `std`, so this
//! crate is `std`-only — matching the istate Rust template's
//! `VecDeque` / `Box<dyn Externals>` shape.

#![allow(clippy::single_match)]

use std::collections::VecDeque;

/// State enum mirroring `<state id="…">` IDs from `stopwatch.scjson`,
/// PascalCased per istate's `to_rust_ident | capitalize` rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Idle,
    Running,
}

/// Event enum mirroring `<transition event="…">` names from
/// `stopwatch.scjson`, PascalCased per istate's
/// `to_rust_ident | capitalize` rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    Start,
    Stop,
    Reset,
    /// Internal-event raise from `<raise event="stopped"/>` on the
    /// running→idle transition.
    Stopped,
}

/// DataModel mirroring `<datamodel><data id="…"/>` entries.
/// Linkage v1 keeps every field as `f64` (the istate scaffold's
/// only supported type).
#[derive(Debug, Clone, Default)]
pub struct DataModel {
    pub elapsed: f64,
    pub lap: f64,
}

/// Trait emitted by istate-codegen for every discovered
/// `<script name="…"/>` callout. Default impls are no-op stubs.
pub trait Externals {
    fn tick_start(&mut self, _m: &mut Machine) {}
    fn tick_stop(&mut self, _m: &mut Machine) {}
}

/// Default no-op `Externals` impl. Matches the istate template's
/// `pub struct DefaultExternals` exactly.
pub struct DefaultExternals;
impl Externals for DefaultExternals {}

/// State machine value. `state` and `dm` fields are public per the
/// istate template; `queue` and the externals trait object are
/// internal.
pub struct Machine {
    pub state: State,
    pub dm: DataModel,
    queue: VecDeque<Event>,
    pub internal_events: bool,
    pub log_to_stderr: bool,
    pub externals: Box<dyn Externals>,
}

impl Machine {
    /// `Machine::new()` — `with_options(false, true)`, matching the
    /// istate template's defaults.
    pub fn new() -> Self {
        Self::with_options(false, true)
    }

    /// `Machine::with_options(internal_events, log_to_stderr)`.
    pub fn with_options(internal_events: bool, log_to_stderr: bool) -> Self {
        Self {
            state: State::Idle,
            dm: DataModel::default(),
            queue: VecDeque::new(),
            internal_events,
            log_to_stderr,
            externals: Box::new(DefaultExternals),
        }
    }

    /// `Machine::dispatch(ev) -> bool`. Fires guard → exit → effect
    /// → entry inline. Returns `true` if a transition fired.
    pub fn dispatch(&mut self, ev: Event) -> bool {
        let fired = match (self.state, ev) {
            (State::Idle, Event::Start) => {
                // running.onentry: <script name="tick_start"/>
                self.state = State::Running;
                true
            }
            (State::Idle, Event::Reset) => {
                self.dm.lap = 0.0;
                true
            }
            (State::Running, Event::Stop) => {
                // running.onexit: <script name="tick_stop"/>
                // transition: <raise event="stopped"/>
                self.state = State::Idle;
                if self.internal_events {
                    self.queue.push_back(Event::Stopped);
                }
                self.dm.elapsed = 0.0; // idle.onentry: assign elapsed = 0
                true
            }
            _ => false,
        };
        if fired && self.internal_events {
            while let Some(qev) = self.queue.pop_front() {
                let _ = self.dispatch(qev);
            }
        }
        fired
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}
