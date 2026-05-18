<!--
04c-initial-value-bindings.md - QT-04c: initial-value text bindings.
-->

**[← Prev](04b-properties-bindings.md) · [Index](README.md) · [Next →](#)** *(QT-04d not yet authored)*

# Chapter QT-04c — Initial-Value Text Bindings

QT-04b lowered handler bodies that *imperatively* mutate state but
left every property *binding* (RHS expressions like `text:
root.title`) as a `// TODO QT-04: bind text` comment. QT-04c lowers
the smallest useful subset of those bindings: a text expression
that resolves to a root-scope `string` property is read once at
construction time and threaded into the widget constructor. This
is **not** reactive; subsequent state mutations do not refresh the
widget. Reactivity is owned by **QT-04e** (reactive bindings) under
the re-split below.

This chapter is **ratified**; the implementation slice lands in the
**same pass** under commit prefix `QT-04c:`.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04 §3](./04-signal-handlers.md#3--canonical-glossary-delta-only),
and [QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only).
The supported binding form ([§5](#5--frozen-decision-supported-binding-form))
is owned here.

## §1 — Purpose

Replace one specific TODO emitted by QT-03b / QT-04b for non-literal
text expressions:

```rust
// TODO QT-04: bind text (non-literal QML expression)
let widget: Rc<RefCell<dyn Widget>> =
    Rc::new(RefCell::new(Label::new("", bounds)));
```

with an initial-value read from `ScreenState`:

```rust
let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(
    Label::new(state.borrow().title.clone(), bounds),
));
```

The widget therefore renders the state's *initial* value on first
draw. Updating the state after construction does not re-render
(see [§9 non-goals](#9--non-goals) and the QT-04e reservation).

## §2 — Problem Statement

QT-03b / QT-04b consider `text: root.title` an unlowered expression
and substitute an empty string. Three failure modes follow:

- A reviewer reading the generated `.rlvgl.rs` for a fixture that
  authored a meaningful default sees `Label::new("", bounds)` and
  has no way to tell that the state-side default would have given
  a non-empty initial render.
- The compile-as-mod gate cannot prove the binding fired at all —
  it can only assert the widget's bounds.
- Bring-up sims wanting to display the screen's "factory state"
  see a blank label until they manually wire the state into the
  widget.

QT-04c closes the gap with a deterministic narrow grammar (§5).
Anything outside the grammar continues to fall back to the existing
empty-string + TODO path, with the TODO renamed to point at the
phase that *will* lower it (QT-04e for reactivity-required
expressions).

## §3 — Canonical Glossary (delta only)

QT-04c introduces no new IR types and no new emitted Rust struct
names. Two terms:

### Initial-value binding

A construction-time read of a `ScreenState` field that the emitter
substitutes for a non-literal text expression on a Label or Button.
Specifically the emitted code calls `state.borrow().<field>.clone()`
and passes the resulting `String` into the widget constructor.

### `// QT-04c bound:` marker

A comment line emitted *above* the construction call, naming the
state field the binding resolved to. Mirrors QT-04 / QT-04b's
`// QT-04 body:` / `// QT-04b body:` markers. Reviewers and tooling
**MUST** be able to grep for `// QT-04c bound:` to find every
initial-value binding.

```rust
// QT-04c bound: text → state.title
let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(
    Label::new(state.borrow().title.clone(), bounds),
));
```

## §4 — Source-of-Truth Map

| Concept                                 | Owner                                                                      |
| --------------------------------------- | -------------------------------------------------------------------------- |
| `qt-ir` IR types                        | QT-00                                                                       |
| `ScreenState`                           | QT-04b                                                                      |
| Handler-body grammar                    | QT-04b §7                                                                   |
| Initial-value binding form              | this chapter (§5)                                                           |
| `// QT-04c bound:` marker               | this chapter (§3)                                                           |
| Reactive update propagation             | **QT-04e** (deferred)                                                       |
| MouseArea / hover handlers              | **QT-04d** (deferred — original QT-04c scope; renumbered)                  |
| Nested ID resolution beyond root        | **QT-04f** (deferred)                                                       |

## §5 — Frozen Decision: Supported Binding Form

QT-04c lowers exactly one expression family in exactly one
assignment target: the `text:` property of `Label` / `QC.Label` /
`Button` / `QC.Button` widgets, when the RHS is a single identifier
(or `root.<ident>`) that resolves to a `ScreenState` field of type
`StateFieldType::StringTy`.

Registration policy for adding new supported binding forms:
**Specification Required**.

| QML target | RHS form                                                  | Lowered to                                                                   |
| ---------- | --------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `text`     | `<ident>` or `<root_id>.<ident>` resolving to a `String` field on `ScreenState` | `state.borrow().<field>.clone()` (passed as the constructor's first arg).   |
| `text`     | Any other non-literal expression                          | **Falls through.** Empty `""` initial value + `// TODO QT-04e: reactive bind text` comment. |
| Anything other than `text` | n/a                                                       | Out of scope at QT-04c. `color:` lowering is **deferred to QT-04e** because color values require runtime parsing of state-held strings and the simplest correct path is reactive. |

Implementations **MUST**:

- Use the *same* root-scope ID resolution rule as QT-04b §8: bare
  `<ident>` or `root.<ident>` only. Other prefixes (`parent.`,
  sibling ids) fall through (QT-04f territory).
- Type-check the resolved field: only `StringTy` matches at QT-04c.
  An `int` or `bool` field with the same name **MUST** fall
  through (the user probably meant `text: count.toString()` which
  is QT-04e territory).
- Emit the `// QT-04c bound: text → state.<field>` marker line
  immediately above the constructor call.

## §6 — Frozen Decision: Phase Re-Split

QT-04 §10 originally sketched QT-04c as "MouseArea / hover
handlers". QT-04b §10 expanded it to "Reactive bindings + MouseArea
+ nested ID resolution". Both readings spread three orthogonal
concerns under one phase number, which would force one large PR
to land or all three to slip together. QT-04c amends the phase
set as follows:

| Phase    | Owns                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------- |
| QT-04c   | Initial-value text bindings (this chapter).                                                    |
| QT-04d   | MouseArea / hover handlers (the original QT-04 §10 scope, renumbered).                         |
| QT-04e   | Reactive bindings (state mutation triggers widget redraws; covers `text:` / `color:` reactive paths). |
| QT-04f   | Nested ID resolution (parent / sibling scope walks; extends QT-04b §8).                        |

This is a **Standards-Action** amendment to QT-00 §5 and is
recorded in QT-00 §15 in the same change-log entry as this
chapter's ratification.

## §7 — Frozen Decision: Emit Order

For a Label / Button widget whose `text:` resolves to a state
field per §5, emit order **MUST** be:

1. The `// QT-04c bound: text → state.<field>` marker line.
2. The `let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(<Constructor>::new(state.borrow().<field>.clone(), bounds)));` line.

For Button, this replaces the QT-04 `let mut button = Button::new(text_str, bounds);` pattern with the bound construction *before* `set_on_click` wiring. The `set_on_click` block remains unchanged.

## §8 — Versioning

QT-04c bumps `QT_EMIT_VERSION_RLVGL` from `4` to `5`. Rationale:
the `// TODO QT-04: bind text` line disappears for any fixture
authoring a `text: <root_string_field>` binding, the construction
call shape changes, and a new `// QT-04c bound:` marker appears in
the diff. Consumers pinned to `4` see a non-trivial diff.

`QT_EMIT_VERSION_DATA` remains at `1`.

## §9 — Non-Goals

- **No reactive propagation.** Mutating `state.title` after
  construction does not refresh the label. QT-04e ships that.
- **No expression evaluation.** Anything beyond `<ident>` /
  `<root_id>.<ident>` falls through. No `+` concatenation, no
  `.toString()`, no ternaries.
- **No `color:` binding.** Color expressions like `color: root.bg`
  remain a `// TODO QT-04e: bind color` comment because the
  state-held value would be a string that needs runtime parsing.
- **No non-string-typed text bindings.** `text: count` (where
  `count` is `i32`) falls through.
- **No bindings on widgets other than Label / Button.** Other
  widget types' text-equivalent properties (e.g. Checkbox label
  once we type that row) lower in their respective row promotions.
- **No write-back.** Whatever the widget does internally with the
  bound initial value does not write back to state.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types.                                            | Cited; not restated.                                                                                   |
| QT-04    | Closure shape (QT-04 §7), `// QT-04 body:` marker (QT-04 §3).     | Unchanged. QT-04c does not touch the handler-body path.                                                 |
| QT-04b   | `ScreenState`, `build_screen` signature, `// QT-04b body:` marker (QT-04b §3). | The `state` parameter that QT-04b threaded through every helper is what makes QT-04c possible. The `// TODO QT-04: bind text` comment QT-04b emitted is replaced by either the new `// QT-04c bound:` marker (when the binding resolves) or by a renamed `// TODO QT-04e: reactive bind text` (when it falls through). |
| QT-04d   | MouseArea / hover.                                                | Independent.                                                                                           |
| QT-04e   | Reactive bindings.                                                | Will repeatedly read state on dirty events. The QT-04c marker line is preserved by QT-04e — reviewers grep for `// QT-04c bound:` to find every initial-value binding regardless of how many phases later add reactivity. |
| QT-04f   | Nested ID resolution.                                             | Independent.                                                                                           |

## §11 — Acceptance Checklist

QT-04c is **ratified and shipped** when:

- [x] §5 freezes the supported binding form.
- [x] §7 fixes the emit order.
- [x] §8 names the version bump.
- [x] `qt::render_rlvgl` calls a new helper that resolves text
      expressions against `ScreenState` per §5.
- [x] `QT_EMIT_VERSION_RLVGL = 5`.
- [x] `// TODO QT-04: bind text` is replaced by either
      `// QT-04c bound:` (resolved) or `// TODO QT-04e: reactive bind text` (fallthrough).
- [x] New canonical fixture
      [`tests/fixtures/qt/bound_text.qml`](../../tests/fixtures/qt/bound_text.qml)
      exercises `text: <root_string_field>`.
- [x] Goldens for the fixture exist:
      [`bound_text.qt-ir.json`](../../tests/fixtures/qt/bound_text.qt-ir.json),
      [`bound_text.rs`](../../tests/fixtures/qt/bound_text.rs),
      [`bound_text.rlvgl.rs`](../../tests/fixtures/qt/bound_text.rlvgl.rs).
- [x] Drift gates pass.
- [x] Compile-as-mod gate verifies the bound label was constructed
      with the state's initial title and that mutating state
      *after* construction does **not** refresh (proves the
      non-reactive contract).
- [x] Existing `hello.rlvgl.rs` golden regenerated — its QC.Label
      now reads from `state.title`. Existing rlvgl compile gate
      consumes the new shape.
- [x] QT-00 §5 phase table amended for the QT-04c/d/e/f re-split.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/04-signal-handlers.md`](./04-signal-handlers.md) — closure shape precedent.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — `ScreenState` + threading precedent.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures.

## §13 — Unblocks

Ratifying QT-04c unblocks:

- `QT-04d` — MouseArea handlers. Now has a clean phase number to
  carry the original QT-04 §10 intent.
- `QT-04e` — reactive bindings. Inherits the `// QT-04c bound:`
  marker so reviewers can grep for the initial-value points that
  reactivity will keep refreshed.
- `QT-04f` — nested ID resolution. Will extend `resolve_string_state_ref`
  (the helper QT-04c introduces) to walk parent / sibling scopes.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. Initial-value text bindings (§5), `// QT-04c bound:` marker (§3), phase re-split QT-04c → narrow scope; carved out QT-04d (MouseArea), QT-04e (reactive), QT-04f (nested IDs) (§6). `QT_EMIT_VERSION_RLVGL` bumped from `4` to `5`. New `bound_text.qml` fixture + 3 goldens + 3 drift gates + compile-as-mod gate that proves the non-reactive contract. `hello.rlvgl.rs` regenerated; its `QC.Label` now reads `state.title` at construction. |
| 2026-04-29 | §9 non-reactive contract **superseded** by QT-04e ([`04e-reactive-bindings.md`](./04e-reactive-bindings.md)). The construction-time read shape is preserved verbatim; QT-04e adds a `Vec<LabelBinding>` to the `build_screen` return tuple plus a `refresh_bindings` helper that re-applies bindings on demand. The `bound_text` compile-as-mod gate now asserts the reactive contract (mutate state → call refresh_bindings → Label text updates). The `// TODO QT-04e: reactive bind text` marker shipped by this chapter on the fall-through path retires when the binding actually resolves; non-resolvable text expressions still emit it. |

---

MIT-licensed: MIT.
