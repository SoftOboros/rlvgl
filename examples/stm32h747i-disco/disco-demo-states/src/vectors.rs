//! Verification vectors per chapter 04 §7.2 of the rlvgl app-schema spec.
//!
//! Each `vector_<id>` test drives the generated dispatch table through a
//! recorded event sequence (`vectors/sequences/<id>.events.txt`) and
//! asserts the resulting trace matches the committed golden
//! (`vectors/sequences/<id>.golden.trace.txt`). The naming pattern is
//! normative so CI can attribute test failures to specific cluster ids
//! without parsing rustc output.
//!
//! Generated — do not edit. Re-run the SM generator to refresh.

#![allow(unused_imports)]

use super::*;

fn vec_state_to_str(s: State) -> &'static str {
    match s {
        State::Idle => "idle",
        State::Menu => "menu",
        State::Settings => "settings",
        State::Playing => "playing",
    }
}

fn run_vector(events: &str, golden: &str) {
    let mut machine = Machine::with_options(false, false);
    let mut out: Vec<String> = Vec::new();
    out.push(format!("on_entry:{}", vec_state_to_str(machine.state)));
    for line in events.lines() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        let ev = match event_from_name(s) {
            Some(e) => e,
            None => panic!("unknown event in vector: {}", s),
        };
        let before = machine.state;
        if machine.dispatch(ev) {
            out.push(format!("on_exit:{}", vec_state_to_str(before)));
            out.push(format!(
                "transition:{}->{}",
                vec_state_to_str(before),
                vec_state_to_str(machine.state),
            ));
            out.push(format!("on_entry:{}", vec_state_to_str(machine.state)));
        } else {
            out.push(format!("no_transition:{} on {}", vec_state_to_str(before), s));
        }
    }
    let expected: Vec<&str> = golden.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        out.len(),
        expected.len(),
        "trace length mismatch: produced {}, golden {}\nproduced: {:#?}\ngolden:   {:#?}",
        out.len(),
        expected.len(),
        out,
        expected,
    );
    for (idx, (got, want)) in out.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            got, want,
            "trace mismatch at line {}: got {:?}, want {:?}",
            idx + 1,
            got,
            want,
        );
    }
}

#[test]
fn vector_back() {
    run_vector(
        include_str!("../vectors/sequences/back.events.txt"),
        include_str!("../vectors/sequences/back.golden.trace.txt"),
    );
}
#[test]
fn vector_open_menu() {
    run_vector(
        include_str!("../vectors/sequences/open_menu.events.txt"),
        include_str!("../vectors/sequences/open_menu.golden.trace.txt"),
    );
}
#[test]
fn vector_open_settings() {
    run_vector(
        include_str!("../vectors/sequences/open_settings.events.txt"),
        include_str!("../vectors/sequences/open_settings.golden.trace.txt"),
    );
}
#[test]
fn vector_play() {
    run_vector(
        include_str!("../vectors/sequences/play.events.txt"),
        include_str!("../vectors/sequences/play.golden.trace.txt"),
    );
}
#[test]
fn vector_stop() {
    run_vector(
        include_str!("../vectors/sequences/stop.events.txt"),
        include_str!("../vectors/sequences/stop.golden.trace.txt"),
    );
}
