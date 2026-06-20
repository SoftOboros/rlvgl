//! SCTD-02 §5.8 behavioral vectors for the Interactive Dining Philosophers
//! machine. The emitter fires one transition per event, so each region listens
//! for DISTINCT per-seat events and the host fans out (mirrored here):
//!   arrive.N / depart.N / break.N / poke.N. Reset is host-side (re-instantiate,
//! §5.7). `tick` = advance the next timer then poke seated philosophers.

use dining_philosophers_interactive::{Machine, Value};

fn vi(m: &Machine, var: &str, k: &str) -> i64 {
    match m.get_var(var) {
        Value::Map(mm) => match mm.get(k) {
            Some(Value::Int(v)) => *v,
            _ => -999,
        },
        _ => -1000,
    }
}
fn vs(m: &Machine, var: &str, k: &str) -> String {
    match m.get_var(var) {
        Value::Map(mm) => match mm.get(k) {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
fn seated(m: &Machine, k: i64) -> bool {
    vi(m, "t_SEATED", &k.to_string()) == 1
}
fn lowest_empty(m: &Machine) -> i64 {
    (1..=5).find(|&k| !seated(m, k)).unwrap_or(0)
}
fn highest_seated(m: &Machine) -> i64 {
    (1..=5).rev().find(|&k| seated(m, k)).unwrap_or(0)
}

// Host-side fan-out (the adapter will do exactly this).
fn arrive(m: &mut Machine) {
    let k = lowest_empty(m);
    if k > 0 {
        m.step(&format!("arrive.{k}"), Value::Undefined);
    }
}
fn depart(m: &mut Machine) {
    let k = highest_seated(m);
    if k > 0 {
        m.step(&format!("depart.{k}"), Value::Undefined);
    }
}
fn panic_break(m: &mut Machine) {
    for k in 1..=5 {
        if seated(m, k) {
            m.step(&format!("break.{k}"), Value::Undefined);
        }
    }
}
fn tick(m: &mut Machine) {
    m.run(1);
    for k in 1..=5 {
        if seated(m, k) {
            m.step(&format!("poke.{k}"), Value::Undefined);
        }
    }
}

fn fresh() -> Machine {
    let mut m = Machine::new();
    m.start();
    m
}

#[test]
fn starts_with_empty_table() {
    let m = fresh();
    for n in ["1", "2", "3", "4", "5"] {
        assert_eq!(vi(&m, "t_SEATED", n), 0);
        assert_eq!(vs(&m, "t_PHASE", n), "empty");
    }
}

#[test]
fn arrive_fills_lowest_empty_and_caps_at_five() {
    let mut m = fresh();
    arrive(&mut m);
    assert_eq!(vi(&m, "t_SEATED", "1"), 1);
    assert_eq!(vs(&m, "t_PHASE", "1"), "thinking");
    assert_eq!(vi(&m, "t_SEATED", "2"), 0);
    for _ in 0..4 {
        arrive(&mut m);
    }
    for n in ["1", "2", "3", "4", "5"] {
        assert_eq!(vi(&m, "t_SEATED", n), 1, "seat {n} filled");
    }
    arrive(&mut m); // full -> no-op
    for n in ["1", "2", "3", "4", "5"] {
        assert_eq!(vi(&m, "t_SEATED", n), 1);
    }
}

#[test]
fn depart_removes_highest_seated_from_thinking() {
    let mut m = fresh();
    arrive(&mut m);
    arrive(&mut m);
    arrive(&mut m);
    depart(&mut m); // highest = 3, Thinking -> leaves now
    assert_eq!(vi(&m, "t_SEATED", "3"), 0);
    assert_eq!(vs(&m, "t_PHASE", "3"), "empty");
    assert_eq!(vi(&m, "t_SEATED", "2"), 1);
    assert_eq!(vi(&m, "t_SEATED", "1"), 1);
}

#[test]
fn single_philosopher_cycles_thinking_to_eating() {
    let mut m = fresh();
    arrive(&mut m);
    assert_eq!(vs(&m, "t_PHASE", "1"), "thinking");
    tick(&mut m);
    assert_eq!(vs(&m, "t_PHASE", "1"), "eating");
    assert_eq!(vi(&m, "t_FORKS", "1"), 1);
    assert_eq!(vi(&m, "t_FORKS", "5"), 1);
    tick(&mut m);
    assert_eq!(vs(&m, "t_PHASE", "1"), "thinking");
    assert_eq!(vi(&m, "t_FORKS", "1"), 0);
    assert_eq!(vi(&m, "t_FORKS", "5"), 0);
}

#[test]
fn depart_while_eating_defers_then_leaves_after_cycle() {
    let mut m = fresh();
    arrive(&mut m);
    tick(&mut m); // -> eating
    assert_eq!(vs(&m, "t_PHASE", "1"), "eating");
    depart(&mut m); // eating -> deferred
    assert_eq!(vi(&m, "t_SEATED", "1"), 1);
    assert_eq!(vi(&m, "t_DEPART_REQ", "1"), 1);
    tick(&mut m); // full -> Thinking -> deferred depart fires
    assert_eq!(vi(&m, "t_SEATED", "1"), 0);
    assert_eq!(vs(&m, "t_PHASE", "1"), "empty");
    assert_eq!(vi(&m, "t_DEPART_REQ", "1"), 0);
}

#[test]
fn deadlock_break_releases_forks_and_returns_to_thinking() {
    let mut m = fresh();
    for _ in 0..5 {
        arrive(&mut m);
    }
    for _ in 0..4 {
        tick(&mut m);
    }
    panic_break(&mut m);
    for n in ["1", "2", "3", "4", "5"] {
        assert_eq!(vi(&m, "t_FORKS", n), 0, "fork {n} released");
        assert_eq!(vi(&m, "t_SEATED", n), 1, "stays seated");
        assert_eq!(vs(&m, "t_PHASE", n), "thinking", "seat {n} thinking");
    }
}

#[test]
fn reset_is_host_reinstantiation() {
    // SCTD-02 §5.7: the host re-instantiates the machine for SIMULATION_RESET.
    let mut m = fresh();
    arrive(&mut m);
    arrive(&mut m);
    tick(&mut m);
    m = fresh(); // <- the reset
    for n in ["1", "2", "3", "4", "5"] {
        assert_eq!(vi(&m, "t_SEATED", n), 0, "seat {n} empty after reset");
        assert_eq!(vs(&m, "t_PHASE", n), "empty");
        assert_eq!(vi(&m, "t_FORKS", n), 0);
    }
}
