<!--
05c-machine-bindings.md - QT-05c: rlvgl emit — DataModel-driven Label bindings.
-->

**[← Prev](05b-handler-dispatch.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05c — Machine Bindings (DataModel → Label Text)

QT-05b lowered `dispatch("…")` into `Machine::dispatch(Event::…)` —
that closes the **input** loop from UI to SM. QT-05c closes the
**output** loop: a Label whose `text:` resolves to a state-machine
datamodel field is bound through the QT-04e refresh pump, so calling
`refresh_bindings` after `Machine::dispatch` updates the visible text.

QT-05c renames the existing `Vec<LabelBinding>` slot in the
`build_screen` 4-tuple to `Vec<Binding>` (a sealed enum over
`LabelBinding` / `MachineBinding`) and extends `refresh_bindings`'
signature to accept `&Rc<RefCell<Machine>>`. Pre-QT-05 modules keep
their QT-04e shape verbatim.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04e §3](./04e-reactive-bindings.md#3--canonical-glossary-delta-only),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
and [QT-05b §3](./05b-handler-dispatch.md#3--canonical-glossary-delta-only).
The `Binding` sealed enum, `MachineBinding` shape, and the
`text: sm.dm.<field>` grammar are owned here.

## §1 — Purpose

After QT-05b a stopwatch's Start button changes
`machine.borrow().state` from `Idle` to `Running`. After QT-05c a
sibling Label can carry `text: sm.dm.elapsed` and reflect the
running counter on the next `refresh_bindings` call:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
// initial render
machine.borrow_mut().dispatch(Event::Start);
// machine.dm.elapsed mutated by external timer:
machine.borrow_mut().dm.elapsed = 12.5;
refresh_bindings(&state, &machine, &bindings);
// the bound Label now shows "12.5"
```

QT-05c **MUST NOT** auto-refresh on dispatch — the QT-04e
caller-driven refresh contract is preserved. Bindings update
exactly when `refresh_bindings` is called.

## §2 — Problem Statement

Three gaps QT-05c closes:

- **No DM-driven Label text.** Without QT-05c, a Label whose
  `text:` is `sm.dm.elapsed` falls through to the QT-04e
  unsupported-form fallback (literal `""`). The most common Qt
  state-machine pattern — running clock, score counter, latched
  reading — is unreachable.
- **No type discipline at the binding boundary.** The istate
  template's `DataModel` is `f64`-only at linkage v1; a binding
  needs to format the float into a String for `Label::set_text`.
  Doing this once per binding site at emit time keeps the runtime
  zero-format-string and reproducible.
- **No clean coexistence with QT-04e.** Adding a parallel
  `Vec<MachineBinding>` 5th tuple element doubles destructure
  pressure on every caller. A sealed `Binding` enum that
  encapsulates both source kinds preserves the 4-tuple shape from
  QT-05b.

## §3 — Canonical Glossary (delta only)

QT-05c introduces no new IR types. Two new emitted types and one
helper amendment.

### `Binding`

```rust
#[derive(Debug, Clone)]
pub enum Binding {
    Label(LabelBinding),
    Machine(MachineBinding),
}
```

Owned here. Replaces the QT-04e `Vec<LabelBinding>` 4-tuple slot
**only when** `module.state_machine.is_some()`. Pre-QT-05 fixtures
(no SM) keep `Vec<LabelBinding>` unchanged — the migration is
opt-in by IR shape.

### `MachineBinding`

```rust
pub struct MachineBinding {
    pub label: Rc<RefCell<rlvgl_widgets::label::Label>>,
    pub accessor: fn(&<sm>_gen::DataModel) -> alloc::string::String,
}

impl MachineBinding {
    pub fn refresh(&self, dm: &<sm>_gen::DataModel) {
        self.label.borrow_mut().set_text((self.accessor)(dm));
    }
}
```

Owned here. Mirrors `LabelBinding` in shape; the only difference is
the source type the accessor reads from.

### `build_screen` return tuple (QT-05c amendment)

Tuple cardinality is unchanged from QT-05b. The slot type changes:

```rust
// QT-05b shape (when SM attached):
pub fn build_screen(bounds)
    -> (WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<LabelBinding>);

// QT-05c shape (when SM attached):
pub fn build_screen(bounds)
    -> (WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<Binding>);
```

When `state_machine` is `None`, the QT-04e 3-tuple
(`(WidgetNode, Rc<RefCell<ScreenState>>, Vec<LabelBinding>)`) is
preserved verbatim.

### `refresh_bindings` (QT-05c amendment)

```rust
// SM attached:
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    machine: &Rc<RefCell<<sm>_gen::Machine>>,
    bindings: &[Binding],
) { … }

// No SM (QT-04e shape):
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    bindings: &[LabelBinding],
) { … }
```

Amends QT-04e §7. The `Rc<RefCell<Machine>>` argument is the same
handle returned as the 3rd element of `build_screen`'s 4-tuple;
callers pass it through after firing dispatches.

### `// QT-05c machine-bound:` marker

Mirror of QT-04e's `// QT-04e bound:`. Emitted directly above each
`bindings.push(Binding::Machine(...))` call so reviewers grep on
this exact prefix.

### `<sm>_gen::DataModel` import

QT-05b's emit added `use <sm>_gen::{Event, Machine};`. QT-05c
extends to `use <sm>_gen::{DataModel, Event, Machine};` whenever
at least one `MachineBinding::Machine` is emitted. (When the IR
has SM but no DM-bound Labels, `DataModel` is omitted to keep the
emit minimal.)

## §4 — Source-of-Truth Map

| Concept                                            | Owner                                                                  |
| -------------------------------------------------- | ---------------------------------------------------------------------- |
| `UiModule.state_machine.datamodel: Vec<UiDmField>` | QT-05 §3.                                                              |
| `Vec<LabelBinding>` (no SM)                        | QT-04e §3.                                                              |
| `Binding` sealed enum (SM)                         | this chapter (§3).                                                      |
| `MachineBinding` shape                             | this chapter (§3).                                                      |
| `text: sm.dm.<field>` grammar                      | this chapter (§5).                                                      |
| `refresh_bindings` signature (SM-attached)         | this chapter (§3); amends QT-04e §7.                                   |
| `// QT-05c machine-bound:` marker                  | this chapter (§3).                                                      |
| `<sm>_gen::DataModel` import                       | this chapter (§6).                                                      |
| Visibility-from-state (`visible: sm.state == …`)   | **deferred** — `// TODO QT-05c: bind visibility` lines reserved.       |
| Color-from-DM                                      | **deferred** — same reason as QT-04e color-deferral.                   |
| Button-text from DM                                | **deferred** — Button-text bindings are still QT-04e territory.        |

## §5 — Frozen Decision: Supported Source Forms

Registration policy: **Specification Required**.

| QML form                                      | Status                                                                           |
| --------------------------------------------- | -------------------------------------------------------------------------------- |
| `text: sm.dm.<dm_field>` on a Label / QC.Label | **shipped** — lowered to `MachineBinding::Machine` per §6.                       |
| `text: <state_field>` on a Label / QC.Label    | unchanged — QT-04e text bindings still apply.                                   |
| `text: sm.dm.<dm_field>` on a Button          | **deferred** — `// TODO QT-05c: bind Button text from DM` line emitted.          |
| `visible: sm.state == State::<Variant>`       | **deferred** — VisibilityFromState reserved as a future §5 amendment.             |
| `color: sm.dm.<dm_field>`                     | **deferred** — same trajectory as QT-04e color bindings.                          |
| `text: dm.<field>` (no `sm.` prefix)          | **not supported**. Authors MUST use the explicit `sm.dm.…` qualified form so QT-05a's nested ID resolution path stays unambiguous against QT-04f's `<other_id>.<prop>` pattern. |
| `text: sm.dm.<missing_field>`                 | **emit-time error**. The walker checks `<missing_field>` against `state_machine.datamodel[*].id`; an unknown field aborts emit with the offending QML expression. |
| Other property → `sm.dm.…`                    | **deferred**.                                                                     |

The unknown-field check is QT-05c's first foray into validating
binding referents at emit time. QT-04e bindings against
`ScreenState` rely on the same rule indirectly (via `resolve_state_field_ref`);
QT-05c brings the same discipline to DM references.

## §6 — Frozen Decision: Emit Order

For a Label whose `text:` matches `sm.dm.<field>`:

1. Construct the concrete Label and keep its `Rc<RefCell<Label>>`:
   ```rust
   // QT-05c machine-bound: text → sm.dm.<field>
   let label_<i>: Rc<RefCell<Label>> = Rc::new(RefCell::new(
       Label::new(
           {
               let m = machine.borrow();
               format_dm_<field>(&m.dm)
           },
           bounds,
       ),
   ));
   ```
   The initial-value read is **machine-driven**, mirroring
   QT-04c's state-driven initial-value contract.
2. Coerce to the generic dyn-Widget pointer for the WidgetNode:
   ```rust
   let widget: Rc<RefCell<dyn Widget>> = label_<i>.clone();
   ```
3. Emit a free function that formats the DM field once per binding
   site:
   ```rust
   #[inline]
   fn format_dm_<field>(dm: &DataModel) -> alloc::string::String {
       use alloc::string::ToString;
       dm.<field>.to_string()
   }
   ```
   For `f64` fields, `ToString` produces a deterministic decimal
   representation (matches `format!("{}")` output). Multi-field
   formatters and locale-aware formatting are deferred.
4. Push the binding:
   ```rust
   bindings.push(Binding::Machine(MachineBinding {
       label: Rc::clone(&label_<i>),
       accessor: format_dm_<field>,
   }));
   ```

For a Label whose `text:` is **already** a `ScreenState` reference
(QT-04c §5 / QT-04e §6), the existing emit shape is preserved —
`Binding::Label(LabelBinding { … })` wraps the QT-04e push.

## §7 — Frozen Decision: `refresh_bindings` Body

For SM-attached modules:

```rust
/// Re-apply every QT-04e / QT-05c binding from the current state
/// and machine. Idempotent; safe to call after every mutation.
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    machine: &Rc<RefCell<Machine>>,
    bindings: &[Binding],
) {
    let s = state.borrow();
    let m = machine.borrow();
    for b in bindings {
        match b {
            Binding::Label(lb) => lb.refresh(&s),
            Binding::Machine(mb) => mb.refresh(&m.dm),
        }
    }
}
```

For non-SM modules: the QT-04e signature
(`fn refresh_bindings(state, &[LabelBinding])`) is preserved.

## §8 — Versioning

| Constant                       | Before QT-05c | After QT-05c |
| ------------------------------ | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL`        | 12            | 13           |
| `QT_IR_VERSION`                | 2             | unchanged    |
| `QT_EMIT_VERSION_DATA`         | 1             | unchanged    |
| `ISTATE_LINKAGE_VERSION`       | 1             | unchanged    |

`QT_EMIT_VERSION_RLVGL` bumps because:

- New `pub enum Binding` and `pub struct MachineBinding` types
  emitted on SM-attached modules.
- `Vec<LabelBinding>` slot of `build_screen`'s 4-tuple becomes
  `Vec<Binding>` for SM-attached modules.
- `refresh_bindings` gains a `&Rc<RefCell<Machine>>` parameter for
  SM-attached modules.
- Helper signatures' `&mut Vec<LabelBinding>` becomes
  `&mut Vec<Binding>` for SM-attached modules.
- New `// QT-05c machine-bound:` marker, new
  `format_dm_<field>` free functions when DM-bound Labels exist.

When no SM is attached, the emit is byte-identical to QT-05b
output **except** for `QT_EMIT_VERSION = 13`. Existing
rlvgl-target goldens regenerate for the version bump only.

## §9 — Non-Goals

- **No automatic refresh on dispatch.** Caller-driven refresh
  remains the contract.
- **No bidirectional binding.** Edits to the Label's text do not
  write back to `dm.<field>`.
- **No multi-field formatters.** A single binding reads one DM
  field; combining (`text: sm.dm.elapsed + " ms"`) is deferred.
- **No type widening of `DataModel`.** `f64` only, matching
  istate's linkage v1.
- **No `visible:` / `color:` bindings.** QT-05c amendments may
  promote them later.
- **No Button-text bindings from DM.** Deferred under the same
  pattern as QT-04e's Button-text deferral.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                         | Resolution                                                                                                                                                            |
| -------- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| QT-05    | `state_machine.datamodel` IR field.             | QT-05c is its first emit-side consumer.                                                                                                                              |
| QT-05a   | scjson side-file walk.                          | QT-05c reads the populated `datamodel` table; no walk changes.                                                                                                       |
| QT-05b   | `build_screen` 4-tuple, dispatch glue.          | **Amended here**: 4th tuple slot type changes from `Vec<LabelBinding>` to `Vec<Binding>` for SM-attached modules. QT-05b §3 / §15 to be amended once this lands.       |
| QT-04c   | Initial-value text bindings (build-time read).  | The QT-04c read shape is preserved when the source is `ScreenState`. QT-05c bindings get their own initial read against `machine.borrow().dm`.                       |
| QT-04e   | Reactive refresh helper.                        | **Amended here**: `refresh_bindings` signature gains `&Rc<RefCell<Machine>>` for SM-attached modules. The QT-04e Vec<LabelBinding> contract is preserved for non-SM. |
| QT-05e   | Externals stub emission.                        | Unblocked by §5's deferred VisibilityFromState row — when QT-05e callouts mutate `dm` from outside `Machine::dispatch`, the same `refresh_bindings` call applies.    |

## §11 — Acceptance Checklist

QT-05c is **ratified and shipped** when:

- [x] §3 names `Binding`, `MachineBinding`, the new `build_screen`
      slot type, and the `refresh_bindings` signature change.
- [x] §5 freezes the supported source forms (Label `text` from
      `sm.dm.<field>` only).
- [x] §6 fixes the emit order.
- [x] §7 fixes the `refresh_bindings` body.
- [x] §8 names the version bump (`12 → 13`).
- [x] `qt::render_rlvgl` emits `pub enum Binding` and
      `pub struct MachineBinding` on SM-attached modules.
- [x] `qt::render_rlvgl` rewrites `Vec<LabelBinding>` slot of
      `build_screen` to `Vec<Binding>` on SM-attached modules.
- [x] Helper signatures' `&mut Vec<LabelBinding>` becomes
      `&mut Vec<Binding>` on SM-attached modules.
- [x] `refresh_bindings` signature widened with `&Rc<RefCell<Machine>>`
      on SM-attached modules.
- [x] QT-05c emits `format_dm_<field>` per DM-bound site.
- [x] Unknown DM field → emit-time error.
- [x] `QT_EMIT_VERSION_RLVGL = 13`.
- [x] `tests/fixtures/qt/stopwatch.qml` extended with a Label
      whose `text: sm.dm.elapsed` exercises §5.
- [x] `tests/creator_qt_emit_stopwatch_compile.rs` extended to
      mutate `machine.borrow_mut().dm.elapsed`, call
      `refresh_bindings`, observe the bound Label's text changed.
- [x] All existing rlvgl-target compile-as-mod gates updated for
      `QT_EMIT_VERSION = 13`.
- [x] All existing rlvgl-target goldens regenerated for the
      version bump (otherwise byte-equal).
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.
- [x] QT-05b §3 / §15 records the 4-tuple slot retype.
- [x] QT-04e §15 records the `refresh_bindings` signature
      widening.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — IR types, 6-symbol linkage.
- [`docs/qt-support/05a-scjson-ingest.md`](./05a-scjson-ingest.md) — populates `state_machine.datamodel`.
- [`docs/qt-support/05b-handler-dispatch.md`](./05b-handler-dispatch.md) — dispatch glue; 4-tuple slot type retyped here.
- [`docs/qt-support/04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) — initial-read shape; mirrored.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — refresh pump; signature widened here.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter.
- [`tests/fixtures/qt/stopwatch.qml`](../../tests/fixtures/qt/stopwatch.qml) — extended for QT-05c.
- [`tests/fixtures/qt/stopwatch_gen/`](../../tests/fixtures/qt/stopwatch_gen) — mock istate crate (DataModel.elapsed).
- [`tests/creator_qt_emit_stopwatch_compile.rs`](../../tests/creator_qt_emit_stopwatch_compile.rs) — extended.

## §13 — Unblocks

Ratifying QT-05c unblocks:

- **QT-05d** (QML `States {}` → scjson emit): the round-trip
  consumer side is now feature-complete (handlers + bindings),
  so QT-05d has a target to validate authoring against.
- **QT-05e** (Externals stubs): an externals callout that mutates
  `m.dm.<field>` is observable via the QT-05c refresh path
  without further emit changes.
- VisibilityFromState (`visible: sm.state == …`): a §5 amendment
  away.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | QT-05c ratified and shipped. New emitted `pub enum Binding { Label(LabelBinding), Machine(MachineBinding) }` sealed enum and new `pub struct MachineBinding` with `accessor: fn(&DataModel) -> String` shape. `build_screen`'s 4-tuple slot retyped from `Vec<LabelBinding>` to `Vec<Binding>` on SM-attached modules; non-SM modules keep the QT-04e 3-tuple verbatim. Helper signatures' `&mut Vec<LabelBinding>` becomes `&mut Vec<Binding>` on SM-attached modules. `refresh_bindings` signature widened with `&Rc<RefCell<Machine>>` between `state` and `bindings` on SM-attached modules; QT-04e shape preserved on non-SM. New `text: sm.dm.<field>` Label-text grammar lowers under a `// QT-05c machine-bound:` marker. Per-binding-site `format_dm_<field>` free functions emitted (`f64::to_string` representation; locale-aware deferred). Unknown DM field → emit-time error. `<sm>_gen::DataModel` joins the `Event`/`Machine` import set when DM bindings exist. `QT_EMIT_VERSION_RLVGL` bumped `12 → 13`. `tests/fixtures/qt/stopwatch.qml` extended with a Label whose `text: sm.dm.elapsed` exercises the grammar; `tests/creator_qt_emit_stopwatch_compile.rs` extended to mutate `machine.borrow_mut().dm.elapsed`, call `refresh_bindings(&state, &machine, &bindings)`, and assert the bound Label's text changed. All 9 existing rlvgl-target compile-gate version assertions bumped `12 → 13`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). QT-05b §3 / §15 records the 4-tuple slot retype. QT-04e §15 records the `refresh_bindings` signature widening on SM-attached modules. VisibilityFromState, color-from-DM, Button-text-from-DM, and multi-field formatters remain deferred under future Specification-Required amendments. |

---

MIT-licensed: MIT.
