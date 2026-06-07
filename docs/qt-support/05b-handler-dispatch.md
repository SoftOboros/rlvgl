<!--
05b-handler-dispatch.md - QT-05b: rlvgl emit — handler dispatch glue.
-->

**[← Prev](05a-scjson-ingest.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05b — Handler Dispatch Glue

QT-05a populated `UiModule.state_machine` from a sibling `.scjson`.
QT-05b consumes that field on the **emit** side: when a screen has
an attached state machine, generated rlvgl-target code threads a
`Rc<RefCell<<sm>_gen::Machine>>` through every helper and lowers QML
`on*: dispatch("<event>")` handlers into
`machine.borrow_mut().dispatch(<sm>_gen::Event::<Pascal>)`.

QT-05b is the first chapter in the QT-05 family that **changes
emitted Rust**. Prior chapters froze contracts and IR shapes; this
one cuts the actual glue.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only),
[QT-04e §3](./04e-reactive-bindings.md#3--canonical-glossary-delta-only),
and [QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only).
The `build_screen` 4-tuple, the `dispatch("…")` handler grammar
extension, and the `// QT-05b dispatch:` marker are owned here.

## §1 — Purpose

After QT-05a, `qt-ir.json` carries a populated `state_machine`
field. The rlvgl emitter currently ignores it. QT-05b makes the
emitter reactive: the presence of `state_machine` flips on a
narrow set of additive emit-shape changes:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
// initial state visible via machine.borrow().state
// callers fire events via machine.borrow_mut().dispatch(Event::Start);
// (handler-emitted code already does this for `dispatch("start")`
// QML bodies — the public `Rc<RefCell<Machine>>` is for callers
// that want to drive the SM externally, e.g. timer ticks.)
```

When `state_machine` is `None`, the existing 3-tuple shape from
QT-04e is preserved verbatim. Backwards compatibility is
non-negotiable: no pre-QT-05 fixture changes.

## §2 — Problem Statement

Three concrete gaps QT-05b closes:

- **No SM ownership.** Without a publicly exposed
  `Rc<RefCell<Machine>>`, callers cannot read state or fire events
  the SM was not designed to receive from the UI (e.g. a tick
  driven by an external timer). Forcing them to construct their
  own `Machine` defeats the whole istate-codegen pipeline.
- **No event dispatch from QML handlers.** `onClicked: dispatch("start")`
  is the most common QML state-machine handler form (used by Qt's
  StopWatch and DiningPhilosophers reference apps). Without
  QT-05b's lowering, the body falls through to QT-04 / QT-04b
  fallthrough and does nothing useful.
- **No version pin.** The 4-tuple return type is a breaking
  change for callers using the QT-04e 3-tuple. `QT_EMIT_VERSION_RLVGL`
  bumps so a consumer building against version 11 fails fast at
  the version assertion rather than the trait-bound mismatch.

## §3 — Canonical Glossary (delta only)

QT-05b introduces no new IR types. One new emit-shape change and
one new comment marker.

### `build_screen` return tuple (QT-05b amendment)

When `module.state_machine.is_some()`:

```rust
pub fn build_screen(bounds: rlvgl_core::widget::Rect)
    -> (
        rlvgl_core::WidgetNode,
        alloc::rc::Rc<core::cell::RefCell<ScreenState>>,
        alloc::rc::Rc<core::cell::RefCell<<sm>_gen::Machine>>,
        alloc::vec::Vec<LabelBinding>,
    );
```

When `module.state_machine.is_none()`: the QT-04e 3-tuple is
preserved unchanged.

**Adapted** from QT-04e §3. Reason: dispatch glue requires a
shared owner of the istate `Machine`, and exposing it publicly
lets callers read/dispatch outside the QML handler set.

Helper signatures gain a fourth parameter when SM is attached:

```rust
fn build_<id>(
    bounds: Rect,
    state: Rc<RefCell<ScreenState>>,
    machine: Rc<RefCell<<sm>_gen::Machine>>,
    label_bindings: &mut Vec<LabelBinding>,
) -> WidgetNode;
```

Pre-QT-05b helpers had three params. QT-05b adds `machine` between
`state` and `label_bindings` so the call-site ordering matches the
return-tuple ordering (state, machine, bindings).

### `// QT-05b dispatch:` marker

Emitted inside any `onClicked` (or other handler) closure that
lowered through QT-05b's `dispatch("…")` grammar. Lives directly
above the `machine.borrow_mut().dispatch(...)` call. Reviewers
grep on this exact prefix.

### `ISTATE_LINKAGE_VERSION` constant

Per QT-05 §6 the linkage surface is pinned. QT-05b is the first
emit chapter that materialises this — it emits

```rust
pub const ISTATE_LINKAGE_VERSION: u32 = 1;
```

at the top of every generated module that has an attached state
machine. Empty modules and modules without SM omit it.

### `QT_SM_NAME` constant

Reflects the `<sm>` ID derived per QT-05a §8. Useful for
diagnostic logging from the consumer side. Emitted alongside
`ISTATE_LINKAGE_VERSION`.

```rust
pub const QT_SM_NAME: &str = "stopwatch";
```

## §4 — Source-of-Truth Map

| Concept                                         | Owner                                                                  |
| ----------------------------------------------- | ---------------------------------------------------------------------- |
| `UiModule.state_machine` field shape            | QT-05 §3.                                                              |
| Population (`Scxml` → `UiStateMachine`)         | QT-05a §6.                                                             |
| 6-symbol istate linkage surface                 | QT-05 §6.                                                              |
| `build_screen` 3-tuple (no SM)                  | QT-04e §3.                                                             |
| `build_screen` 4-tuple (SM present)             | this chapter (§3).                                                      |
| Helper `&mut Vec<LabelBinding>` parameter       | QT-04e §3.                                                              |
| Helper `Rc<RefCell<Machine>>` parameter         | this chapter (§3).                                                      |
| `dispatch("…")` handler-body grammar            | this chapter (§5).                                                      |
| `// QT-05b dispatch:` marker                    | this chapter (§3).                                                      |
| `ISTATE_LINKAGE_VERSION` / `QT_SM_NAME` consts  | this chapter (§3).                                                      |
| `<sm>_gen::*` symbol set                        | upstream istate template (read-only here per QT-05 §6).                |
| Reactive label bindings (`refresh_bindings`)    | QT-04e §7; **not** extended by QT-05b. QT-05c amends to read DM.       |
| State-gated visibility / dm-read bindings       | **deferred** to QT-05c.                                                 |
| `Externals` stub emission (`<sm>_externals.rs`) | **deferred** to QT-05e.                                                 |

## §5 — Frozen Decision: `dispatch("…")` Handler Grammar

Registration policy: **Specification Required** (extends QT-04b §7).

| Handler body form                            | Lowering                                                                                                                                                  |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dispatch("<ident>")`                        | `machine.borrow_mut().dispatch(<sm>_gen::Event::<Pascal>);`                                                                                              |
| `dispatch('<ident>')`                        | Same. Single-quoted string literals accepted.                                                                                                            |
| `dispatch("<ident>"); dispatch("<ident2>")`  | Two consecutive `dispatch` lowerings, each under its own `// QT-05b dispatch:` marker. Multi-statement bodies fall through to QT-04 unlowered if any statement is not a `dispatch(…)` call. |
| `dispatch(some_expr)`                        | Falls through to QT-04 unlowered (`// QT-04 body:`). Expression-form events are deferred — they require a runtime `string -> Event` mapping which istate's `event_from_name` provides but which QT-05 §6 explicitly excludes from the linkage surface. |
| `dispatch()` (no arg)                        | Compile-time error: `dispatch must have exactly one string-literal argument`. |
| `dispatch("<ident>", …)` (multi-arg)         | Same error. SCXML `<send>` payload params are deferred.                                                                                                  |
| Any other handler body                       | Existing QT-04b §7 / QT-04 §6 fallthrough behaviour, unchanged.                                                                                          |

### `<Pascal>` event-name normalisation

The string passed to `dispatch("<ident>")` is normalised to a
PascalCase Rust enum variant matching istate's
`to_rust_ident | capitalize` rule:

- ASCII letter / digit / underscore characters survive.
- Underscores split words; first letter of each word becomes
  uppercase.
- Hyphens, dots, spaces split words the same way.
- Numeric leading characters are illegal; emit-time error.
- Empty string is illegal; emit-time error.

Examples:

| QML literal       | Rust enum variant        |
| ----------------- | ------------------------ |
| `"start"`         | `Event::Start`           |
| `"go"`            | `Event::Go`              |
| `"timer_tick"`    | `Event::TimerTick`       |
| `"timer-tick"`    | `Event::TimerTick`       |
| `"button.press"`  | `Event::ButtonPress`     |
| `"toggle"`        | `Event::Toggle`          |
| `"42invalid"`     | (error)                  |

The walker has no visibility into the istate-emitted `Event`
enum's actual variants — it can't validate that `Event::Start`
exists. Callers that mistype get a Rust **compile error** at
build time rather than a runtime panic. This is intentional:
istate-codegen is the source of truth for the `Event` set.

## §6 — Frozen Decision: Emit Order (per-screen)

For a generated rlvgl module whose IR has `state_machine: Some(_)`:

1. Top-of-module:
   ```rust
   pub const ISTATE_LINKAGE_VERSION: u32 = 1;
   pub const QT_SM_NAME: &str = "<sm>";
   use <sm>_gen::{Event, Machine};
   ```
   (`State` and `DataModel` imports are NOT emitted at QT-05b —
   they land with QT-05c when bindings start reading them.)
2. `pub struct ScreenState { … }` per QT-04b §3 (unchanged).
3. `pub struct LabelBinding { … }` + `impl LabelBinding` per
   QT-04e §3 (unchanged).
4. `pub fn build_screen(bounds) -> (WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<LabelBinding>)`:
   ```rust
   let state = Rc::new(RefCell::new(ScreenState { … }));
   let machine = Rc::new(RefCell::new(Machine::new()));
   let mut bindings: Vec<LabelBinding> = Vec::new();
   let node = build_<root_id>(bounds, state.clone(), machine.clone(), &mut bindings);
   (node, state, machine, bindings)
   ```
5. Each `build_<id>` helper signature is the QT-05b 4-param shape
   (§3); its body threads `machine.clone()` into recursive calls.
6. Inside any handler closure (per QT-04 §6), bodies that match
   the §5 `dispatch("…")` grammar emit:
   ```rust
   // QT-05b dispatch: <event> → Event::<Pascal>
   machine.borrow_mut().dispatch(Event::<Pascal>);
   ```
   under the existing `// QT-04 body:` marker is replaced.
7. `pub fn refresh_bindings(...)` from QT-04e is unchanged at
   QT-05b. (QT-05c will extend its signature.)

When `state_machine` is `None`, steps 1, 4 (return-tuple), 5
(machine param), and 6 (dispatch lowering) are all skipped. The
emit shape collapses to the QT-04e 3-tuple verbatim.

## §7 — Versioning

| Constant                       | Before QT-05b | After QT-05b |
| ------------------------------ | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL`        | 11            | 12           |
| `QT_IR_VERSION`                | 2             | unchanged    |
| `QT_EMIT_VERSION_DATA`         | 1             | unchanged    |
| `ISTATE_LINKAGE_VERSION`       | (constant in QT-05 chapter, not emitted) | 1 (now emitted in modules with SM) |

`QT_EMIT_VERSION_RLVGL` bumps because:

- `build_screen` return tuple grew from 3 to 4 elements when SM is
  attached.
- Every `build_<id>` helper signature gained a parameter.
- New `// QT-05b dispatch:` marker, new emit-shape constants
  (`ISTATE_LINKAGE_VERSION`, `QT_SM_NAME`).

When no SM is attached, the emit is byte-identical to QT-04e
output **except for** the `QT_EMIT_VERSION` constant. Existing
rlvgl-target goldens regenerate with `QT_EMIT_VERSION = 12`; no
other byte changes.

## §8 — `<sm>_gen` Crate Resolution

The emitted `use <sm>_gen::{Event, Machine};` statement assumes
`<sm>_gen` is reachable as an extern crate name. QT-05 §7 names
the canonical layout:

```toml
[dependencies]
<sm>_gen = { path = "crates/<sm>_gen" }
```

The consumer crate is responsible for adding this dependency; the
rlvgl emitter does not write to the consumer's `Cargo.toml`. (A
future amendment MAY add a `--with-codegen` flag that auto-adds
the dep alongside calling istate-codegen.)

For the QT-05b compile-as-mod gate, `tests/fixtures/qt/stopwatch_gen/`
is a hand-vendored crate satisfying the QT-05 §6 6-symbol surface;
it's wired into rlvgl's `[dev-dependencies]` so the gate compiles
without the live softoboros MCP. This is the test analog of the
production `crates/<sm>_gen/` path-dep.

## §9 — Non-Goals

- **No `match` over `State` in handlers.** State-gated handler
  bodies (`if (state == State.Running) dispatch("stop")`) are
  deferred to QT-05c (which owns `State` reads via bindings).
- **No `dm`-mutation handlers.** A QML handler body that wants to
  write `<sm>_gen::DataModel.<field>` directly cannot do so —
  istate's API is `dispatch` + entry/exit `<assign>`, not
  external write. This is a v1 linkage limitation.
- **No event coalescing or queueing across handlers.** Each
  `dispatch("…")` lowers to one `Machine::dispatch` call, fires
  guard/exit/effect/entry inline, returns. Callers wanting
  microstep/macrostep ordering use istate's
  `with_options(internal_events = true, …)` semantics — which
  the linkage surface exposes via `Machine::with_options`.
- **No automatic `dispatch` from non-QML contexts.** External
  timers, network events, etc. drive the Machine via the public
  `Rc<RefCell<Machine>>` returned by `build_screen` — there is no
  rlvgl-emitted glue for them.
- **No `Externals` plug-through.** That's QT-05e. QT-05b's
  emitted `Machine::new()` always installs `DefaultExternals`.
- **No Button-text / color / DM-text bindings.** QT-05c.
- **No reactive refresh on dispatch.** `refresh_bindings` is not
  called automatically after `dispatch`; the consumer (or a
  future QT-05c amendment) is responsible. This preserves QT-04e's
  caller-driven refresh contract.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                  | Resolution                                                                                                                                                                                                                                                  |
| -------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| QT-05    | IR types + 6-symbol linkage.                             | QT-05b is the first **consumer** of the linkage. Linkage v1's `Event` and `Machine` are imported; `State`/`DataModel`/`Externals` remain unimported until QT-05c/e.                                                                                          |
| QT-05a   | Side-file ingest.                                        | QT-05b reads `state_machine` only — populating it stays QT-05a's job.                                                                                                                                                                                       |
| QT-04b   | `build_screen` 2-tuple → 3-tuple, ScreenState threading. | **Amended here**: when SM is attached, return type extended to a 4-tuple. Helper signatures gain `Rc<RefCell<Machine>>` between `state` and `label_bindings`. QT-04b §3 / §15 to be amended once this lands.                                                |
| QT-04e   | Reactive refresh helper.                                 | `refresh_bindings` signature unchanged at QT-05b. QT-05c will widen it to also accept `&Rc<RefCell<Machine>>` once MachineBindings exist.                                                                                                                   |
| QT-04 / QT-04d | Handler closure shape, MouseArea wiring.            | The QT-05b `dispatch("…")` grammar slots in **before** the QT-04 / QT-04b §7 fallthrough — handlers matching it lower to dispatch glue; handlers not matching fall through to existing behaviour. Both `Button.onClicked` and `ClickArea.on_click` qualify. |
| QT-08    | Directory-mode CLI.                                      | Independent. QT-08's directory walker still triggers QT-05a side-file probes per file; QT-05b's emit just reads the resulting field.                                                                                                                        |

## §11 — Acceptance Checklist

QT-05b is **ratified and shipped** when:

- [x] §3 names the 4-tuple shape, the `// QT-05b dispatch:`
      marker, and the new `ISTATE_LINKAGE_VERSION` /
      `QT_SM_NAME` constants.
- [x] §5 freezes the `dispatch("…")` handler grammar +
      `<Pascal>` normalisation.
- [x] §6 fixes the per-screen emit order.
- [x] `tests/fixtures/qt/stopwatch_gen/` satisfies the 6-symbol
      linkage surface and is wired as a dev-dep of rlvgl.
- [x] `qt::render_rlvgl` emits the 4-tuple, `Event` import, and
      dispatch glue when `state_machine.is_some()`.
- [x] Pre-QT-05 fixtures regenerate with `QT_EMIT_VERSION = 12`
      and otherwise byte-identical Rust.
- [x] `stopwatch.rlvgl.rs` regenerates with the SM glue
      visible.
- [x] `tests/creator_qt_emit_stopwatch_compile.rs` consumes the
      emitted module, destructures the 4-tuple, fires
      `Event::PressRelease` on the start button, asserts
      `machine.borrow().state == State::Running`.
- [x] All existing rlvgl-target compile-as-mod gates updated for
      `QT_EMIT_VERSION = 12`.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — IR types, 6-symbol linkage.
- [`docs/qt-support/05a-scjson-ingest.md`](./05a-scjson-ingest.md) — ingest side; populates the field this chapter consumes.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — `build_screen` shape; amended here.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — refresh pump; unchanged at QT-05b.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/stopwatch.qml`](../../tests/fixtures/qt/stopwatch.qml) — canonical fixture.
- [`tests/fixtures/qt/stopwatch.scjson`](../../tests/fixtures/qt/stopwatch.scjson) — paired scjson side-file.
- [`tests/fixtures/qt/stopwatch_gen/`](../../tests/fixtures/qt/stopwatch_gen) — hand-vendored mock istate crate (matches QT-05 §6 surface).
- [`tests/creator_qt_emit_stopwatch_compile.rs`](../../tests/creator_qt_emit_stopwatch_compile.rs) — compile-as-mod gate (added by this chapter).

## §13 — Unblocks

Ratifying QT-05b unblocks:

- **QT-05c** (DM/State bindings): can amend `refresh_bindings` to
  read `machine.borrow().dm.<field>` and `machine.borrow().state`
  using the public `Rc<RefCell<Machine>>` exposed here.
- **QT-05e** (Externals stubs): can install a custom `Externals`
  impl by switching `Machine::new()` to
  `Machine::with_options(false, true)` + a generated
  `<screen>_externals` install path.
- Real-project bring-up where Qt screens with state machines need
  to dispatch events from QML buttons. After QT-05b, the
  authoring path is:
  ```qml
  Button { onClicked: dispatch("start") }
  ```
  paired with a hand-written or QT-05d-emitted `.scjson`, with no
  user-side glue.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | QT-05b ratified and shipped. `qt::render_rlvgl` is now reactive to `module.state_machine`: when `Some(_)`, `build_screen` returns a 4-tuple `(WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<<sm>_gen::Machine>>, Vec<LabelBinding>)` and every helper gains a `Rc<RefCell<Machine>>` parameter. New `dispatch("<event>")` handler grammar lowers to `machine.borrow_mut().dispatch(<sm>_gen::Event::<Pascal>)` under a `// QT-05b dispatch:` marker. New emit constants `ISTATE_LINKAGE_VERSION = 1` and `QT_SM_NAME = "<sm>"` appear at the top of every SM-attached module. PascalCase event-name normalisation matches istate's `to_rust_ident \| capitalize` rule (snake_case / kebab-case / dotted forms split on word boundaries). When `state_machine` is `None`, the QT-04e 3-tuple shape is preserved verbatim (full backwards-compat). `QT_EMIT_VERSION_RLVGL` bumped `11 → 12`. New fixture: `tests/fixtures/qt/stopwatch_gen/` mock istate crate (matches QT-05 §6 6-symbol linkage surface) wired as a dev-dep of rlvgl. New compile-as-mod gate `tests/creator_qt_emit_stopwatch_compile.rs` destructures the 4-tuple, fires synthetic clicks on Start/Stop/Reset, asserts `machine.borrow().state` flips between `State::Idle` and `State::Running`. All existing rlvgl-target compile-gates' version assertions bumped `11 → 12`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). QT-04b §3 / §15 records the 4-tuple amendment-when-SM-attached. State-gated handlers, `dm` mutation, and Button-text bindings remain deferred to QT-05c (DM/State bindings) or QT-05e (Externals). |

---

MIT-licensed: MIT.
