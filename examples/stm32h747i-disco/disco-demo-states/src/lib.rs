// Generated iState Rust runtime (scaffold)

use std::collections::VecDeque;



#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Idle,
    Menu,
    Settings,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    OpenMenu,
    Play,
    Back,
    OpenSettings,
    Stop,
}

#[derive(Debug, Clone)]
pub struct DataModel {
}

impl Default for DataModel {
    fn default() -> Self {
        Self {
        }
    }
}

pub struct Machine {
    pub state: State,
    pub dm: DataModel,
    queue: VecDeque<Event>,
    pub internal_events: bool,
    pub log_to_stderr: bool,
    pub externals: Box<dyn Externals>,
}

impl Machine {
    pub fn new() -> Self { Self::with_options(false, true) }
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

    pub fn dispatch(&mut self, ev: Event) -> bool {
        match (self.state, ev) {
            (State::Idle, Event::OpenMenu) => {
                if guard_t0(self) {
                    on_exit_idle(self);
                    effect_t0(self);
                    self.state = State::Menu;
                    on_entry_menu(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            (State::Idle, Event::Play) => {
                if guard_t1(self) {
                    on_exit_idle(self);
                    effect_t1(self);
                    self.state = State::Playing;
                    on_entry_playing(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            (State::Menu, Event::Back) => {
                if guard_t2(self) {
                    on_exit_menu(self);
                    effect_t2(self);
                    self.state = State::Idle;
                    on_entry_idle(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            (State::Menu, Event::OpenSettings) => {
                if guard_t3(self) {
                    on_exit_menu(self);
                    effect_t3(self);
                    self.state = State::Settings;
                    on_entry_settings(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            (State::Settings, Event::Back) => {
                if guard_t4(self) {
                    on_exit_settings(self);
                    effect_t4(self);
                    self.state = State::Menu;
                    on_entry_menu(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            (State::Playing, Event::Stop) => {
                if guard_t5(self) {
                    on_exit_playing(self);
                    effect_t5(self);
                    self.state = State::Idle;
                    on_entry_idle(self);
                    if self.internal_events {
                        while let Some(iev) = self.queue.pop_front() {
                            let _ = self.dispatch(iev);
                        }
                    }
                    true
                } else { false }
            }
            ,            _ => false,
        }
    }

    fn raise_name(&mut self, name: &str) {
        if !self.internal_events { return; }
        if let Some(ev) = event_from_name(name) {
            self.queue.push_back(ev);
        }
    }
}

// Guard stubs — emitted for every transition referenced by `dispatch`
// (those with both source and event). Transitions without `cond` get a
// `true` body so the dispatch table compiles without forcing every
// branch to special-case the guard call.
pub fn guard_t0(m: &Machine) -> bool {
    let _ = m;
    true
}
pub fn guard_t1(m: &Machine) -> bool {
    let _ = m;
    true
}
pub fn guard_t2(m: &Machine) -> bool {
    let _ = m;
    true
}
pub fn guard_t3(m: &Machine) -> bool {
    let _ = m;
    true
}
pub fn guard_t4(m: &Machine) -> bool {
    let _ = m;
    true
}
pub fn guard_t5(m: &Machine) -> bool {
    let _ = m;
    true
}

// Map string name to Event (for raise/send)
fn event_from_name(s: &str) -> Option<Event> {
    match s {
        "open_menu" => Some(Event::OpenMenu),
        "play" => Some(Event::Play),
        "back" => Some(Event::Back),
        "open_settings" => Some(Event::OpenSettings),
        "stop" => Some(Event::Stop),
        _ => None,
    }
}

// Effect stubs
pub fn effect_t0(m: &mut Machine) {
    let _ = m; // no-op
}
pub fn effect_t1(m: &mut Machine) {
    let _ = m; // no-op
}
pub fn effect_t2(m: &mut Machine) {
    let _ = m; // no-op
}
pub fn effect_t3(m: &mut Machine) {
    let _ = m; // no-op
}
pub fn effect_t4(m: &mut Machine) {
    let _ = m; // no-op
}
pub fn effect_t5(m: &mut Machine) {
    let _ = m; // no-op
}

// State lifecycle stubs
pub fn on_entry_idle(m: &mut Machine) {
    let _ = m;
}
pub fn on_exit_idle(m: &mut Machine) {
    let _ = m;
}
pub fn on_entry_menu(m: &mut Machine) {
    let _ = m;
}
pub fn on_exit_menu(m: &mut Machine) {
    let _ = m;
}
pub fn on_entry_settings(m: &mut Machine) {
    let _ = m;
}
pub fn on_exit_settings(m: &mut Machine) {
    let _ = m;
}
pub fn on_entry_playing(m: &mut Machine) {
    let _ = m;
}
pub fn on_exit_playing(m: &mut Machine) {
    let _ = m;
}

// Externals (callouts)
pub trait Externals {
}
pub struct DefaultExternals;
impl Externals for DefaultExternals {}