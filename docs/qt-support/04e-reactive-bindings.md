<!--
04e-reactive-bindings.md - QT-04e: reactive Label-text bindings.
-->

**[← Prev](04d-mousearea.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-04e — Reactive Bindings (Label Text)

QT-04c lowered `text: <state_field>` into a one-shot
`state.borrow().<field>.clone()` read at construction. After
construction, mutating `state.<field>` did not refresh the widget —
that "non-reactive contract" was the explicit QT-04c §9 promise.
QT-04e replaces it with a caller-driven refresh: build_screen now
exposes a `Vec<LabelBinding>` of concrete `Rc<RefCell<Label>>`
handles paired with state accessors, plus a generated
`refresh_bindings` helper that re-applies every binding in one call.

QT-04e is the **closing phase of the QT-04 family**: reactive text
bindings retire the last user-facing `// TODO QT-04e:` markers in
the canonical fixtures. Color bindings and handler-body expression
expansion remain deferred to later phases.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only),
and [QT-04c §3](./04c-initial-value-bindings.md#3--canonical-glossary-delta-only).
The `LabelBinding` shape and the new `build_screen` return tuple
are owned here.

## §1 — Purpose

Replace QT-04c's non-reactive contract. After QT-04e:

```rust
let (node, state, bindings) = build_screen(bounds);
// initial render — Label shows state.title's literal default
state.borrow_mut().title = String::from("New title");
refresh_bindings(&state, &bindings);
// Label now displays "New title".
```

The build_screen 2-tuple return shape from QT-04b §3 grows a third
element — `Vec<LabelBinding>` — carrying concrete Label handles for
every text-bound Label. Construction-time semantics are unchanged
(initial values still come from `state.borrow().<field>.clone()`).
The new behaviour is purely additive: callers that don't want
reactivity can ignore the `bindings` element with a `_`.

## §2 — Problem Statement

The QT-04c contract was deliberately narrow: "the value at build
time, then nothing." Three failure modes accumulated:

- A QML screen with `text: title` has no way to update the label
  after construction without manual widget surgery on the
  generated tree (which the user can't do without downcasting
  through `Rc<RefCell<dyn Widget>>`).
- The reactive-bindings TODO markers were the most numerous
  remaining QT-04e markers in the fixture goldens — `bound_text`
  and `hello` both carry them.
- The user-facing API drift between "QT-04c bindings exist but
  don't update" and "QT-04b handlers can mutate state" was
  confusing — handlers wrote to state with no observable effect.

QT-04e closes all three with the simplest sufficient design: keep
concrete handles, expose them publicly, ship a one-call refresh
helper.

## §3 — Canonical Glossary (delta only)

QT-04e introduces no new IR types. Two new emitted Rust types and
one helper function.

### `LabelBinding`

A `pub struct` emitted into every generated rlvgl-target module that
contains at least one text-bound Label (per QT-04c §5 resolution).
Carries a concrete `Rc<RefCell<Label>>` plus an accessor function
that reads the bound state field:

```rust
pub struct LabelBinding {
    pub label: Rc<RefCell<rlvgl_widgets::label::Label>>,
    pub accessor: fn(&ScreenState) -> alloc::string::String,
}

impl LabelBinding {
    pub fn refresh(&self, state: &ScreenState) {
        self.label.borrow_mut().set_text((self.accessor)(state));
    }
}
```

Owned here. Both fields are public so callers can inspect (e.g.
read the current label text, replace the accessor).

### `build_screen` return tuple (QT-04e amendment)

```rust
pub fn build_screen(bounds: rlvgl_core::widget::Rect)
    -> (
        rlvgl_core::WidgetNode,
        alloc::rc::Rc<core::cell::RefCell<ScreenState>>,
        alloc::vec::Vec<LabelBinding>,
    );
```

**Adapted** from QT-04b §3. Reason: reactive refresh requires
concrete widget handles for every binding; the tuple's third
element exposes them without restructuring the API into a struct.

The generated `refresh_bindings` helper reads the accessors:

```rust
pub fn refresh_bindings(
    state: &alloc::rc::Rc<core::cell::RefCell<ScreenState>>,
    bindings: &[LabelBinding],
) {
    let s = state.borrow();
    for b in bindings {
        b.refresh(&s);
    }
}
```

Every helper signature gains a third parameter:

```rust
fn build_<id>(
    bounds: Rect,
    state: Rc<RefCell<ScreenState>>,
    label_bindings: &mut Vec<LabelBinding>,
) -> WidgetNode;
```

Callers pass `&mut label_bindings` so each helper can push its own
bindings during the depth-first walk.

### `// QT-04e bound:` marker

Mirror of QT-04c's `// QT-04c bound:` marker, emitted inside the
helper that constructs a bound Label, immediately above the
`label_bindings.push(...)` line. Reviewers grep on this exact
prefix.

## §4 — Source-of-Truth Map

| Concept                                  | Owner                                                                  |
| ---------------------------------------- | ---------------------------------------------------------------------- |
| Initial-value binding (build-time read)  | QT-04c                                                                 |
| `// QT-04c bound:` construction marker   | QT-04c                                                                 |
| Reactive refresh helper                  | this chapter                                                            |
| `LabelBinding` shape                     | this chapter (§3)                                                       |
| `build_screen` return tuple              | this chapter (amends QT-04b §3 / QT-04c §10)                           |
| Per-helper `&mut Vec<LabelBinding>`      | this chapter                                                            |
| `refresh_bindings` free function         | this chapter (emitted into the generated module)                       |
| Color bindings                           | **deferred** — `// TODO QT-04e: bind color` lines stay; promotion tracked under a future amendment |
| Handler-body expression expansion        | **deferred** — `// TODO QT-04e: lower QML expression to Rust` lines stay |

## §5 — Frozen Decision: Supported Reactive Forms

Registration policy: **Specification Required**.

| QML form                                                    | QT-04c (build-time read)                       | QT-04e (reactive refresh)                                                  |
| ----------------------------------------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------- |
| `text: <state_string_field>` on a Label / Button mapped to `Label` | already supported                          | **shipped**: `Rc<RefCell<Label>>` retained as a `LabelBinding`.           |
| `text: <state_string_field>` on a `Button`-mapped widget    | already supported                              | **deferred** — Button bindings tracked under a future amendment.          |
| `color: <state_string_field>`                               | not supported (QT-04c §9)                      | **deferred** — color values require runtime `parse_qml_color_lit` on the bound string. Tracked under a future amendment. |
| Any other property → state ref                              | not supported                                  | **deferred**.                                                              |

QT-04e ships exactly one row: text-bound Labels. This narrow scope
matches the canonical fixture set (`bound_text.qml` and `hello.qml`
both use Label, neither uses Button text bindings nor color
bindings). Buttons and colors carry the same `// TODO QT-04e:`
comment shape; promoting them is a Specification-Required
amendment to this §5 table.

## §6 — Frozen Decision: Emit Order

For a Label whose `text:` resolves to a state field per QT-04c §5:

1. Construct the concrete Label and keep its `Rc<RefCell<Label>>`:
   ```rust
   // QT-04c bound: text → state.<field>
   let label_<i>: Rc<RefCell<Label>> = Rc::new(RefCell::new(
       Label::new(state.borrow().<field>.clone(), bounds),
   ));
   ```
2. Coerce to the generic dyn-Widget pointer for the WidgetNode:
   ```rust
   let widget: Rc<RefCell<dyn Widget>> = label_<i>.clone();
   ```
3. Push the binding:
   ```rust
   // QT-04e bound: refresh state.<field> → label_<i>.set_text
   label_bindings.push(LabelBinding {
       label: Rc::clone(&label_<i>),
       accessor: |s| s.<field>.clone(),
   });
   ```

The local handle name `label_<i>` is a per-helper counter (using
the existing per-helper node-index) so multiple bound labels in the
same helper do not shadow each other.

For Labels whose `text:` does **not** resolve (literal-default and
fall-through paths from QT-04c §5), the existing emission shape is
unchanged — no `Rc<RefCell<Label>>` handle, no `LabelBinding`.

## §7 — Frozen Decision: `refresh_bindings` Free Function

Every generated rlvgl-target module **MUST** emit:

```rust
/// Re-apply every QT-04e binding from the current state. Idempotent;
/// safe to call after every `state.borrow_mut()` mutation.
pub fn refresh_bindings(
    state: &Rc<RefCell<ScreenState>>,
    bindings: &[LabelBinding],
) {
    let s = state.borrow();
    for b in bindings {
        b.refresh(&s);
    }
}
```

This free function is emitted **even when `label_bindings` is
empty** — keeps the per-fixture API stable. Callers can always call
`refresh_bindings(&state, &bindings)` and it's a no-op when there
are no bindings.

## §8 — Versioning

QT-04e bumps `QT_EMIT_VERSION_RLVGL` from `10` to `11`. Rationale:

- `build_screen` return type changes from a 2-tuple to a 3-tuple.
- `pub struct LabelBinding` and `pub fn refresh_bindings` are new
  public emit-shape items.
- Helper signatures gain a `&mut Vec<LabelBinding>` parameter.
- Per-bound-Label emit acquires the `Rc::new(RefCell::new(...))`/clone
  pair plus the bindings push.

`QT_EMIT_VERSION_DATA` is unchanged.

## §9 — Non-Goals

- **No event-driven refresh.** Mutation does not auto-trigger
  refresh. Callers explicitly invoke `refresh_bindings` after the
  mutations they want propagated. This matches the spirit of
  rlvgl's existing event loop (caller-driven dispatch).
- **No fine-grained refresh.** `refresh_bindings` re-applies
  every binding; there is no "only-changed-fields" mode. Fine
  graining is a future amendment when fixture pressure justifies
  it.
- **No two-way binding.** Edits to the widget's text (e.g. via
  user input on a future Input widget) do not write back to
  state.
- **No reactive Button text.** Deferred to a Specification-Required
  amendment.
- **No reactive color bindings.** Same.
- **No reactive handler-body expansion.** Handler bodies that fall
  through to the QT-04 unlowered path remain unchanged.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types.                                            | Cited; not restated.                                                                                   |
| QT-04b   | `build_screen` 2-tuple return, ScreenState threading.            | **Amended here**: return type extended to a 3-tuple. Helper signatures gain `&mut Vec<LabelBinding>`. Recorded in QT-04b §15. |
| QT-04c   | Initial-value text bindings, "non-reactive contract".            | **Amended here**: the §9 non-reactive contract is superseded. The build-time read shape is preserved; QT-04e adds the refresh path on top. Recorded in QT-04c §15. |
| QT-04 / QT-04d | Handler closure / MouseArea wiring.                       | Independent. Closures still capture state via `Rc::clone(&state)` per QT-04 §7.                       |
| QT-04f   | Nested ID resolution.                                            | The `resolve_state_field_ref` shared resolver continues to back QT-04c bindings; QT-04e's reactive layer reads accessors over namespaced fields the same way it reads root-scope fields. |

## §11 — Acceptance Checklist

QT-04e is **ratified and shipped** when:

- [x] §3 names `LabelBinding`, the new `build_screen` return tuple,
      and the per-helper `label_bindings` parameter.
- [x] §5 freezes the supported reactive forms (text on Label only).
- [x] §6 fixes the emit order.
- [x] §7 fixes the `refresh_bindings` free function shape.
- [x] §8 names the version bump.
- [x] `qt::render_rlvgl` emits `pub struct LabelBinding`,
      `impl LabelBinding`, the new `build_screen` 3-tuple, and the
      `pub fn refresh_bindings` helper.
- [x] Helper signatures thread `label_bindings` through.
- [x] `QT_EMIT_VERSION_RLVGL = 11`.
- [x] `bound_text` compile-as-mod gate updated to **assert**
      reactivity: mutate `state.title`, call `refresh_bindings`,
      observe the bound Label's text changed.
- [x] Existing rlvgl-target compile-as-mod gates updated for the
      3-tuple return.
- [x] All existing rlvgl-target goldens regenerated for the
      version bump and the new struct / free function emissions.
- [x] QT-04b §15 records the `build_screen` signature change.
- [x] QT-04c §15 records the non-reactive-contract supersession.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — 2-tuple return, amended here.
- [`docs/qt-support/04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) — non-reactive contract, amended here.
- [`widgets/src/label.rs`](../../widgets/src/label.rs) — `Label::set_text`.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures.

## §13 — Unblocks

Ratifying QT-04e unblocks:

- Real-project bring-up where state mutates over time and bound
  text needs to follow. The `refresh_bindings` call is a single
  line at the end of each mutation batch.
- Future amendments — Button text bindings, color bindings,
  handler-body expression expansion — slot into the existing
  `Vec<LabelBinding>` framework with their own `<Kind>Binding`
  types.
- A future "auto-refresh on state change" optimisation. The
  current explicit-call model is a stable contract callers can
  pin against; auto-refresh would be additive (a new
  `subscribe(state, bindings)` helper).

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. Closes out the QT-04 family. `LabelBinding` struct, `build_screen` 3-tuple return, `refresh_bindings` free function, per-helper `&mut Vec<LabelBinding>` threading, `// QT-04e bound:` marker, scope (text on Label only) frozen. `QT_EMIT_VERSION_RLVGL` bumped `10 → 11`. `bound_text` compile-as-mod gate updated to assert reactive contract. All existing rlvgl-target goldens regenerated; existing compile-gate destructures updated from 2-tuple to 3-tuple. QT-04b §15 records the `build_screen` signature change; QT-04c §15 records the non-reactive-contract supersession. Color bindings, Button text bindings, and handler-body expression expansion remain deferred under future Specification-Required amendments. |

---

MIT-licensed: MIT.
