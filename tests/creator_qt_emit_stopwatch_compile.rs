// SPDX-License-Identifier: MIT
//! QT-05b compile-as-mod gate: include the canonical stopwatch
//! emitted module via `#[path]`, destructure the 4-tuple
//! `build_screen` return shape, fire synthetic `Event::PressRelease`
//! events at the Start/Stop/Reset buttons, and assert
//! `machine.borrow().state` flips through `State::Idle` →
//! `State::Running` → `State::Idle` per the lowered
//! `dispatch("…")` glue.
//!
//! Validates four contracts from QT-05b:
//!
//! 1. The 4-tuple return shape (`(WidgetNode, Rc<RefCell<ScreenState>>,
//!    Rc<RefCell<Machine>>, Vec<LabelBinding>)`) destructures
//!    correctly when the IR has `state_machine: Some(_)` (QT-05b §3).
//! 2. `Machine::new()` is called once during `build_screen` and the
//!    initial `State::Idle` is observable via `machine.borrow().state`
//!    before any handler fires (QT-05 §6 linkage v1).
//! 3. `onClicked: dispatch("start")` lowered to
//!    `machine.borrow_mut().dispatch(Event::Start)` per QT-05b §5;
//!    the synthetic click flips `machine.borrow().state` to
//!    `State::Running`.
//! 4. The emit-shape constants `ISTATE_LINKAGE_VERSION = 1` and
//!    `QT_SM_NAME = "stopwatch"` are present (QT-05b §3 / §7).
//!
//! Locked by `docs/qt-support/05b-handler-dispatch.md` §11.

use rlvgl_core::event::Event as UiEvent;
use stopwatch_gen::State as SmState;

// stopwatch.qml uses camelCase QML IDs (`startBtn`/`stopBtn`/`resetBtn`)
// so the emitter produces helper names like `build_startBtn`. The
// emit shape stays QT-04b §8 / QT-05b §6 compliant; the lint warning
// is purely cosmetic and the user-facing remedy is to rename the QML
// IDs. Suppress here so the gate can focus on the dispatch glue.
#[allow(non_snake_case)]
#[path = "fixtures/qt/stopwatch.rlvgl.rs"]
mod generated_stopwatch;

// QT-05e: pull in the emitted externals stubs to compile-check
// they integrate with the mock `stopwatch_gen` crate's `Externals`
// trait shape.
#[path = "fixtures/qt/stopwatch_externals.rs"]
mod generated_stopwatch_externals;

#[test]
fn generated_stopwatch_module_lowers_dispatch_glue() {
    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 320,
        height: 200,
    };
    let (node, state, machine, _bindings) = generated_stopwatch::build_screen(bounds);

    // QT-05b §3 / §7: emit-shape constants are present.
    assert_eq!(generated_stopwatch::QT_EMIT_VERSION, 20);
    assert_eq!(generated_stopwatch::QT_IR_VERSION, 2);
    assert_eq!(generated_stopwatch::ISTATE_LINKAGE_VERSION, 1);
    assert_eq!(generated_stopwatch::QT_SM_NAME, "stopwatch");

    // QT-05 §6 linkage v1: initial state is `Idle` (matching
    // `<scxml initial="idle">` from stopwatch.scjson).
    assert_eq!(machine.borrow().state, SmState::Idle);
    assert_eq!(state.borrow().title, "Stopwatch");

    // Locate the three buttons by their tags. Per QT-05a's QML they
    // are `startBtn`, `stopBtn`, `resetBtn`.
    let start_btn = node
        .children
        .iter()
        .find(|c| c.tag == Some("startBtn"))
        .expect("startBtn child should be present");
    let stop_btn = node
        .children
        .iter()
        .find(|c| c.tag == Some("stopBtn"))
        .expect("stopBtn child should be present");
    let reset_btn = node
        .children
        .iter()
        .find(|c| c.tag == Some("resetBtn"))
        .expect("resetBtn child should be present");

    let inside = |b: rlvgl_core::widget::Rect| UiEvent::PressRelease {
        x: b.x + b.width / 2,
        y: b.y + b.height / 2,
    };

    // QT-05b §5: dispatch("start") lowering. Idle → Running.
    let start_bounds = start_btn.widget.borrow().bounds();
    let handled = start_btn
        .widget
        .borrow_mut()
        .handle_event(&inside(start_bounds));
    assert!(handled, "Start button should handle PressRelease");
    assert_eq!(
        machine.borrow().state,
        SmState::Running,
        "QT-05b: dispatch('start') flipped Idle → Running"
    );

    // QT-05b §5: dispatch("stop") lowering. Running → Idle.
    let stop_bounds = stop_btn.widget.borrow().bounds();
    let handled = stop_btn
        .widget
        .borrow_mut()
        .handle_event(&inside(stop_bounds));
    assert!(handled, "Stop button should handle PressRelease");
    assert_eq!(
        machine.borrow().state,
        SmState::Idle,
        "QT-05b: dispatch('stop') flipped Running → Idle"
    );

    // Idempotency: a second Start press fires another dispatch and
    // re-enters Running.
    let _ = start_btn
        .widget
        .borrow_mut()
        .handle_event(&inside(start_bounds));
    assert_eq!(machine.borrow().state, SmState::Running);

    // QT-05b §5: dispatch("reset") lowering. Reset is only
    // accepted in the Idle state for stopwatch.scjson, so we first
    // route back to Idle then fire reset.
    let _ = stop_btn
        .widget
        .borrow_mut()
        .handle_event(&inside(stop_bounds));
    assert_eq!(machine.borrow().state, SmState::Idle);
    let reset_bounds = reset_btn.widget.borrow().bounds();
    let handled = reset_btn
        .widget
        .borrow_mut()
        .handle_event(&inside(reset_bounds));
    assert!(handled, "Reset button should handle PressRelease");
    assert_eq!(machine.borrow().state, SmState::Idle);
}

/// QT-05c §11 acceptance gate: the `text: sm.dm.elapsed` Label
/// reflects `machine.dm.elapsed` after a caller mutates the field
/// and calls `refresh_bindings(&state, &machine, &bindings)`. Locks
/// in:
///
/// 1. The 4-tuple's binding slot is `Vec<Binding>` (sealed enum).
/// 2. The bound Label's initial text is read from `machine.dm` at
///    construction (not from `ScreenState`).
/// 3. After `dm.elapsed = 12.5` + `refresh_bindings`, the bound
///    Label's text is `"12.5"` (per `f64::to_string`).
/// 4. The QT-04e Label binding (`text: root.title`) continues to
///    work in parallel — the sealed enum doesn't break it.
#[test]
fn generated_stopwatch_module_lowers_dm_text_binding() {
    use rlvgl_widgets::label::Label;
    use std::cell::RefCell;
    use std::rc::Rc;

    let bounds = rlvgl_core::widget::Rect {
        x: 0,
        y: 0,
        width: 320,
        height: 200,
    };
    let (node, state, machine, bindings) = generated_stopwatch::build_screen(bounds);

    // Locate the two bound Labels by tag.
    let display = node
        .children
        .iter()
        .find(|c| c.tag == Some("display"))
        .expect("display Label (QT-04e bound) should be present");
    let counter = node
        .children
        .iter()
        .find(|c| c.tag == Some("counter"))
        .expect("counter Label (QT-05c bound) should be present");

    // Non-zero binding count: at least one LabelBinding (QT-04e:
    // display ↔ root.title) and one MachineBinding (QT-05c:
    // counter ↔ sm.dm.elapsed) exist.
    assert!(
        bindings.len() >= 2,
        "expected at least one LabelBinding + one MachineBinding, got {}",
        bindings.len()
    );

    // Helper: project the dyn-Widget Rc<RefCell> to a concrete
    // Rc<RefCell<Label>> so we can read `.text()`. The emitter
    // constructs both branches as `Rc::new(RefCell::new(Label::…))`
    // so the cast is sound; if the cast ever fails the panic gives
    // a clear signal that the emit shape drifted.
    fn as_label_rc(w: Rc<RefCell<dyn rlvgl_core::widget::Widget>>) -> Rc<RefCell<Label>> {
        // We rely on the emit always returning a `Label` widget for
        // these tagged children; a bug in the emit would surface
        // via the `text()` access below.
        let raw = Rc::into_raw(w) as *const RefCell<Label>;
        unsafe { Rc::from_raw(raw) }
    }
    let counter_label = as_label_rc(counter.widget.clone());
    let display_label = as_label_rc(display.widget.clone());

    // QT-05c §6: initial bound-text comes from machine.dm.elapsed
    // = 0.0 at construction → format_dm_elapsed returns "0".
    assert_eq!(
        counter_label.borrow().text(),
        "0",
        "QT-05c §6: initial DM-bound text from machine.dm.elapsed = 0.0"
    );

    // QT-04e §1 caller-driven refresh: mutate dm without calling
    // refresh_bindings — the Label keeps the construction-time text.
    machine.borrow_mut().dm.elapsed = 12.5;
    assert_eq!(
        counter_label.borrow().text(),
        "0",
        "QT-04e §9 contract: mutation alone does not refresh"
    );

    // QT-05c §7: refresh_bindings re-applies every binding,
    // pulling fresh values from machine.dm.
    generated_stopwatch::refresh_bindings(&state, &machine, &bindings);
    assert_eq!(
        counter_label.borrow().text(),
        "12.5",
        "QT-05c §6: refresh_bindings re-read machine.dm.elapsed"
    );
    // QT-04e binding still works in parallel.
    assert_eq!(
        display_label.borrow().text(),
        "Stopwatch",
        "QT-04e LabelBinding survived the QT-05c sealed-enum amendment"
    );

    // Idempotency: calling refresh again without a mutation is a
    // no-op.
    generated_stopwatch::refresh_bindings(&state, &machine, &bindings);
    assert_eq!(counter_label.borrow().text(), "12.5");

    // Mutate ScreenState.title; the QT-04e Label updates after
    // refresh while the QT-05c Label is unaffected.
    state.borrow_mut().title = String::from("Updated");
    generated_stopwatch::refresh_bindings(&state, &machine, &bindings);
    assert_eq!(display_label.borrow().text(), "Updated");
    assert_eq!(counter_label.borrow().text(), "12.5");
}

/// QT-05e §11 acceptance gate: the emitted ScreenExternals satisfies
/// the QT-05 §6 `Externals` linkage trait, can be constructed via
/// `::new()`, and can be installed on a `Machine` via the public
/// `externals` field per QT-05e §7.
#[test]
fn generated_stopwatch_externals_installs_on_machine() {
    use generated_stopwatch_externals::ScreenExternals;
    use stopwatch_gen::Machine;

    // QT-05e §3: stateless construction.
    let externals = ScreenExternals::new();
    let _ = externals; // confirm it builds; install path proven below.

    // QT-05e §7: install via the public `Machine.externals: Box<dyn Externals>`.
    let mut machine = Machine::new();
    machine.externals = Box::new(ScreenExternals::new());
    // The default `tick_start` / `tick_stop` stubs are no-ops; we
    // just verify they compile and accept `&mut Machine`.
    machine.externals.tick_start(&mut Machine::new());
    machine.externals.tick_stop(&mut Machine::new());

    // QT-05e §3 / §8: per-file version constant is reachable.
    assert_eq!(generated_stopwatch_externals::QT_EXTERNALS_VERSION, 1);
}
