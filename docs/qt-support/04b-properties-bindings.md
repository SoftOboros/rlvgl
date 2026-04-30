<!--
04b-properties-bindings.md - QT-04b: properties + handler-body expression lowering.
-->

**[← Prev](04-signal-handlers.md) · [Index](README.md) · [Next →](#)** *(QT-04c not yet authored)*

# Chapter QT-04b — Properties + Bindings

QT-04 wired empty `set_on_click` closures with the QML body
preserved as a `// QT-04 body:` comment. QT-04b teaches the
plumbing what each property declaration and handler body actually
means: it generates a `ScreenState` struct from QML `property`
declarations, threads it through every builder helper, and lowers
a tightly-scoped subset of handler bodies into real state mutations.

This chapter is **ratified**; implementation is **scheduled for the
next pass** under commit prefix `QT-04b:`. Following the same
spec-before-code rhythm used by QT-03 → QT-03b, the design is
locked here so reviewers can redirect before code lands.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-03 §3](./03-rlvgl-emitter-widgets.md#3--canonical-glossary-delta-only),
[QT-03b §3](./03b-rlvgl-widget-mapping.md#3--canonical-glossary-delta-only),
and [QT-04 §3](./04-signal-handlers.md#3--canonical-glossary-delta-only).
The `ScreenState` shape ([§3](#3--canonical-glossary-delta-only)),
the supported property type set ([§5](#5--frozen-decision-supported-property-types)),
and the handler-body lowering grammar ([§7](#7--frozen-decision-handler-body-lowering-grammar))
are owned here.

## §1 — Purpose

QT-04 left every lowered closure with this body:

```rust
button.set_on_click(|_b| {
    // TODO QT-04b: lower QML expression to Rust.
});
```

QT-04b replaces it. After ratification + implementation, the same
closure for `onClicked: count += 1` reads:

```rust
{
    let state = Rc::clone(&state);
    button.set_on_click(move |_b| {
        // QT-04b body: count += 1
        state.borrow_mut().count += 1;
    });
}
```

That requires:

1. A `ScreenState` struct generated from root-level QML `property`
   declarations.
2. A `build_screen` signature change — it now returns
   `(WidgetNode, Rc<RefCell<ScreenState>>)` so callers can read /
   write state from outside.
3. Threading `Rc<RefCell<ScreenState>>` through every `build_<id>`
   helper.
4. A handler-body parser that recognises a small grammar of
   property mutations and lowers them; everything else stays as a
   `// TODO QT-04c:` comment with the verbatim QML body preserved.

## §2 — Problem Statement

QT-04's empty closures prove the wiring path but do not respond to
input. A button that registers an `on_click` callback that does
nothing is a regression risk: without state mutation, the only way
to verify a generated screen *behaves* correctly is to wire it up
manually outside the generated module.

Three failure modes follow:

- A reviewer reading the generated `.rlvgl.rs` cannot tell whether
  a handler will eventually do anything — the `// TODO QT-04b`
  marker is honest but uninformative.
- The compile-as-mod gate at QT-04 (`creator_qt_emit_clickable_compile.rs`)
  proves `set_on_click` linked but cannot prove the click *does
  anything*. End-to-end behavioural coverage is impossible without
  state mutation.
- Property declarations from QT-01a (`property int count: 0`)
  remain entirely elided at emit time. They appear only in the
  IR and as a `// emitter-skipped (QT-04+):` summary in the
  generated source.

QT-04b closes all three by generating typed state and lowering a
deterministic subset of mutations. The subset stays small on
purpose: a tiny grammar with a clear failure mode (`// TODO
QT-04c:` for anything outside it) is easier to audit than a
heuristic-rich expression evaluator.

## §3 — Canonical Glossary (delta only)

QT-04b introduces no new IR types and amends QT-03b's emitted-Rust
surface in two ways: a new `ScreenState` struct and a changed
`build_screen` signature.

### `ScreenState` struct

A `#[derive(Debug, Clone)]` struct emitted into every generated
rlvgl-target module that contains QML property declarations on the
root item. One field per declared property. Field types are the
Rust mapping from §5. Owned here.

```rust
#[derive(Debug, Clone)]
pub struct ScreenState {
    pub title: alloc::string::String,
    pub count: i32,
    pub ratio: f32,
}
```

If the root item declares no properties, the struct is **still
emitted**, with no fields. This keeps the `build_screen` signature
stable across fixtures and removes a feature-detection step from
every consumer. `#[derive(Debug, Clone)]` are the only derives at
QT-04b; QT-04c **MAY** add `Default` once a sensible "no-state"
default is defined.

### `build_screen` signature (QT-04b version)

```rust
pub fn build_screen(bounds: rlvgl_core::widget::Rect)
    -> (rlvgl_core::WidgetNode, alloc::rc::Rc<core::cell::RefCell<ScreenState>>);
```

**Adapted** from QT-03b §3. Reason: a `WidgetNode` alone cannot
expose state to the caller. The state handle is needed both for
external observers (a desktop sim might want to read `count`) and
for the closures that mutate it.

The `Rc<RefCell<ScreenState>>` is constructed once per call to
`build_screen` and threaded through every `build_<id>` helper as a
new second parameter. Every helper signature changes from QT-03b:

```rust
fn build_<id>(bounds: Rect, state: Rc<RefCell<ScreenState>>) -> WidgetNode;
```

Helpers that do not consume `state` use it only to pass to their
own children; clippy `unused_variables` is suppressed via the
existing `#![allow(dead_code)]` and `#![allow(unused_imports)]`
file-level allows (extended in §11 with `#![allow(unused_variables)]`).

### Handler-body lowering grammar

The grammar from §7. Owned here. Anything outside the grammar
falls through to a `// TODO QT-04c:` comment with the verbatim QML
body preserved on a `// QT-04 body:` line.

### `// QT-04b body:` marker

Mirrors QT-04's `// QT-04 body:` marker but indicates the handler
body **was lowered**. Reviewers / tooling **MUST** be able to
grep on this exact string. Lines using this prefix appear *above*
the lowered Rust statements inside the closure, so the original
QML and its lowering are visually adjacent:

```rust
// QT-04b body: count += 1
state.borrow_mut().count += 1;
```

The QT-04 `// QT-04 body:` prefix is reserved for the
**unlowered** path: it appears above the closure when the body
falls through to a `// TODO QT-04c:` placeholder. The two prefixes
together form a stable mini-language reviewers can grep on.

### ID resolution scope

A QML identifier (`count`, `title`, etc.) inside a handler body
**MUST** resolve to a property declared on the *closest enclosing
ancestor whose `id` matches a registered scope*. At QT-04b, the
only registered scope is the **screen root**: `ScreenState` carries
the root item's properties. Other scopes (per-item state, parent
chain walking) are **deferred to QT-04c**.

A handler body referencing an unresolvable identifier does not
abort the build; the body falls through to the `// TODO QT-04c:`
path with a `// QT-04b: unresolved <ident>` annotation.

## §4 — Source-of-Truth Map

| Concept                                    | Owner                                                                  |
| ------------------------------------------ | ---------------------------------------------------------------------- |
| `qt-ir` IR types                           | QT-00                                                                   |
| Mapping table                              | QT-03b §5 / QT-04 §5                                                   |
| Handler closure shape (QT-04 baseline)     | QT-04 §7                                                                |
| `ScreenState` struct                       | this chapter                                                            |
| `build_screen` signature                   | this chapter (amends QT-03b §3)                                         |
| Supported property types                   | this chapter (§5)                                                       |
| Default-value lowering rules               | this chapter (§6)                                                       |
| Handler-body lowering grammar              | this chapter (§7)                                                       |
| ID resolution scope                        | this chapter (§8)                                                       |
| Reactive property propagation              | **QT-04c** (deferred)                                                   |
| Multi-screen / cross-module state          | **QT-08** (CLI surface phase)                                           |

## §5 — Frozen Decision: Supported Property Types

Registration policy: **Specification Required**.

| QML type   | Rust type                          | Default literal example | Notes                                          |
| ---------- | ---------------------------------- | ----------------------- | ---------------------------------------------- |
| `int`      | `i32`                              | `42`                    | Negative literals supported (`-3`).             |
| `real`     | `f32`                              | `1.5`                   | Single-precision is sufficient for QT-04b's UI use cases; `f64` deferred. |
| `double`   | `f32`                              | `1.5`                   | QML `double` is identical to `real` for our purposes; both lower to `f32`. |
| `string`   | `alloc::string::String`            | `"Hello"`               | Owned `String` so closures can mutate.         |
| `bool`     | `bool`                             | `true` / `false`        |                                                |
| Anything else | **unsupported** — property is dropped from `ScreenState` with a `// emitter-skipped (QT-04c+): property <name>: <ty>` comment | n/a            | Includes `var`, `variant`, `Item`, user-defined types, and aliases. |

A property declaration with a non-literal default value (e.g.
`property int count: someExpression`) **MUST** be lowered with the
type-appropriate Rust default (`0`, `0.0`, `String::new()`, `false`)
plus a `// QT-04b: non-literal default for <name>: <expr>` comment
above the field initializer. The QML expression itself is not
evaluated at QT-04b.

## §6 — Frozen Decision: Default-Value Lowering Rules

For a property declaration `property <ty> <name>[: <expr>]`:

| Default expression form | Lowered to                                                          |
| ----------------------- | ------------------------------------------------------------------- |
| Absent                  | Type-appropriate Rust default.                                       |
| Integer literal         | The literal integer (cast to `i32`).                                 |
| Float literal           | The literal float (cast to `f32`).                                   |
| `true` / `false`        | The literal `bool`.                                                  |
| Quoted string literal   | `String::from("…")` (handled per QT-03 §8 escape rule).             |
| Anything else           | Type-appropriate Rust default + `// QT-04b: non-literal default` comment. |

Default-value parsing reuses the helpers established by QT-03b §6
(`parse_string_literal`, `parse_int_literal`, plus a new
`parse_float_literal` and a `parse_bool_literal`).

## §7 — Frozen Decision: Handler-Body Lowering Grammar

A QML handler body **lowers** under QT-04b when, after stripping
trailing semicolons and trimming whitespace, it matches one of the
patterns below. Anything else falls through to the QT-04 unlowered
path. Multiple matching statements separated by `;` lower
sequentially, preserving order.

| Pattern                                      | Lowered statement                                                                       |
| -------------------------------------------- | --------------------------------------------------------------------------------------- |
| `<ident> += <int_literal>`                   | `state.borrow_mut().<ident> = state.borrow().<ident>.saturating_add(<int>)` for `i32`; `+=` for `f32` / `String` (the latter only when the literal is a `string` and the property is `string`). |
| `<ident> -= <int_literal>`                   | Mirror of above with `saturating_sub` for `i32`.                                        |
| `<ident> = <int_literal>`                    | `state.borrow_mut().<ident> = <int>;` for `i32` properties.                              |
| `<ident> = <float_literal>`                  | Same for `f32` properties.                                                               |
| `<ident> = <quoted_string_literal>`          | `state.borrow_mut().<ident> = String::from(<lit>);` for `string` properties.            |
| `<ident> = true` / `<ident> = false`         | `state.borrow_mut().<ident> = <bool>;` for `bool` properties.                            |
| `<ident> = !<ident>`                         | Self-toggle for `bool` properties.                                                       |
| Anything else                                | **Falls through.** A `// QT-04 body:` comment carries the verbatim QML; the closure body itself remains the QT-04 `// TODO QT-04c:` placeholder. |

Type checking happens at emit time: the emitter looks up `<ident>`
in the resolved scope (§8); if the property's type does not match
the literal's type, the statement falls through and is annotated
with a `// QT-04b: type mismatch (<ident>: <ty>, expected <ty>)`
comment.

`saturating_add` / `saturating_sub` are chosen over wrapping
arithmetic because a UI counter overflowing silently is a worse
default than a stuck max value. Future amendments **MAY** introduce
opt-in wrapping via a per-property attribute, deferred.

## §8 — Frozen Decision: ID Resolution Scope

At QT-04b, exactly **one scope** is registered: the screen root.
A handler body's `<ident>` resolves to a `ScreenState` field if and
only if `<ident>` matches a property name declared on the QML root
item.

Examples (assuming `property int count: 0` on root):

| Handler body                                | Lowering                                                          |
| ------------------------------------------- | ------------------------------------------------------------------ |
| `count += 1`                                | Lowered (count is on root).                                        |
| `root.count += 1`                           | Lowered (the `root.` prefix matches the root id and is stripped). |
| `parent.count += 1`                         | **Falls through** — `parent` resolution is QT-04c.                |
| `someOther.count += 1`                      | **Falls through** — `someOther` is not the root id.               |
| `count = otherProp`                         | **Falls through** — RHS is not a literal.                         |

The root id is the `id:` value on the QML root item, defaulting to
`"root"` if unset. The `<root_id>.` prefix is stripped before
matching against `ScreenState` fields.

## §9 — Non-Goals

- **No reactive property propagation.** Mutating `state.count` does
  not automatically redraw widgets that bound to `count` at
  construction time. Reactivity is **QT-04c**. Until then,
  consumers manually call `node.dispatch_event(&Event::Tick)` or
  trigger a redraw out-of-band.
- **No JS engine.** Anything beyond the §7 grammar is not
  evaluated. Operators not in §7 (`*=`, `/=`, `%=`, `&&`, `||`,
  function calls, ternary, indexing) fall through.
- **No object literals or list literals as defaults.** Properties
  whose default is `[ ... ]` or `{ ... }` are unsupported in §5.
- **No multi-statement bodies with mixed lowered/unlowered
  statements.** If any statement in a body falls through, the
  *entire* body falls through. Reviewers **SHOULD** split mixed
  handlers in the QML source.
- **No nested ID resolution.** `parent.foo`, sibling references,
  `Component`/`StateGroup` lookups are all QT-04c+.
- **No string concatenation / interpolation.** `text: "x: " + count`
  is unsupported (text bindings remain literal-only at QT-04b).
- **No two-way binding.** A QML `text: count` does **not** update
  when `count` mutates, and edits to `text` do not write back.
- **No reactive RHS bindings.** The QT-03b "TODO QT-04: bind text"
  comment becomes "TODO QT-04c: bind text"; QT-04b does not lower
  reactive bindings, only the imperative-mutation handler subset.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types.                                            | Cited; not restated. The `UiProperty` IR shape already carries everything QT-04b needs.                |
| QT-01a   | Structural ingest.                                                | No change. Property declarations are already in the IR.                                                |
| QT-02    | IR schema artifact.                                              | Unchanged.                                                                                             |
| QT-03    | Data-only emit shape.                                            | Unchanged. QT-04b touches the rlvgl target only.                                                       |
| QT-03b   | Widget API mapping; `build_screen` signature.                     | **§3 amendment** required: `build_screen` returns `(WidgetNode, Rc<RefCell<ScreenState>>)`. Recorded in QT-03b §15. |
| QT-04    | Signal handler closure shape.                                    | The closure now captures `state` via a `move` closure + an outer `let state = Rc::clone(&state);` block. The `// QT-04 body:` marker is repurposed for the unlowered path; the new `// QT-04b body:` marker covers the lowered path. Recorded in QT-04 §15. |
| QT-04c   | Reactive bindings + MouseArea + nested ID resolution.            | Replaces the `// TODO QT-04c:` placeholders. Likely bumps `QT_EMIT_VERSION_RLVGL` to `5`.            |
| QT-05    | State machines.                                                   | Will compose with `ScreenState` rather than replace it.                                                |
| QT-06    | Theme-token reconciliation.                                       | Tokens flow into widget construction, orthogonal to `ScreenState`.                                     |

## §11 — Versioning

QT-04b bumps `QT_EMIT_VERSION_RLVGL` from `3` to `4`. Rationale:

- The `build_screen` return type changes from `WidgetNode` to a
  tuple. This is a breaking change for any consumer pinned to
  `QT_EMIT_VERSION_RLVGL = 3`.
- Generated files now include a `pub struct ScreenState { … }`
  declaration even when the root item has no properties.
- Helper signatures gain a second `state` parameter.
- Closures in handler-supported widgets become `move` closures
  capturing `Rc<RefCell<ScreenState>>`.

The data-target version `QT_EMIT_VERSION_DATA` remains at `1` —
QT-04b does not affect the data emit.

A new file-level allow `#![allow(unused_variables)]` is added to
the rlvgl-target emit so helpers that thread `state` without
consuming it do not trip clippy / rustc warnings.

## §12 — Acceptance Checklist

QT-04b is **ratified** when the items below are checked. The
implementation slice (struct emission, signature change, body
lowering, fixture, gates) lands in the next pass and ticks the
remaining boxes.

- [x] §3 names the `ScreenState` shape, its derives, and the new
      `build_screen` signature.
- [x] §5 freezes the supported property type set.
- [x] §6 freezes the default-value lowering rules.
- [x] §7 freezes the handler-body lowering grammar.
- [x] §8 freezes the ID resolution scope (root only at QT-04b).
- [x] §11 names the version bump and rationale.
- [x] `qt::render_rlvgl` emits the `ScreenState` struct + the new
      `build_screen` signature (see [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)
      `emit_screen_state_struct` / `emit_screen_state_init`).
- [x] Property defaults lower per §6 (helpers
      `lower_property_default`, `parse_float_literal`,
      `parse_bool_literal`).
- [x] Handler bodies lower per §7 / §8 (`lower_handler_body`,
      `lower_handler_statement`, `split_assignment`,
      `strip_root_prefix`).
- [x] `QT_EMIT_VERSION_RLVGL = 4`.
- [x] Existing rlvgl-target goldens
      ([`hello.rlvgl.rs`](../../tests/fixtures/qt/hello.rlvgl.rs),
      [`clickable.rlvgl.rs`](../../tests/fixtures/qt/clickable.rlvgl.rs))
      regenerated for the new signature.
- [x] Existing rlvgl compile-as-mod tests updated to consume the
      tuple return and assert state shape (`hello`'s `ScreenState`
      now carries the literal-default `title` / `count` / `ratio`
      fields; `clickable`'s `ScreenState` is empty by §3).
- [x] New canonical fixture
      [`tests/fixtures/qt/counter.qml`](../../tests/fixtures/qt/counter.qml).
- [x] Goldens exist:
      [`counter.qt-ir.json`](../../tests/fixtures/qt/counter.qt-ir.json),
      [`counter.rs`](../../tests/fixtures/qt/counter.rs),
      [`counter.rlvgl.rs`](../../tests/fixtures/qt/counter.rlvgl.rs).
- [x] Drift gates pass — `qt_counter_fixture_{ingest,data_emit,rlvgl_emit}_matches_golden`.
- [x] Compile-as-mod gate
      [`tests/creator_qt_emit_counter_compile.rs`](../../tests/creator_qt_emit_counter_compile.rs)
      calls `build_screen`, fires a synthetic
      `Event::PressRelease` inside the button's bounds, and asserts
      `state.borrow().count == 1` (and `== 2` after a second click).
- [x] QT-03b §3 / §15 amended to record the `build_screen`
      signature change. QT-04 §15 amended to record the
      `// QT-04 body:` repurposing.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — `build_screen` precedent (amended here).
- [`docs/qt-support/04-signal-handlers.md`](./04-signal-handlers.md) — closure shape precedent.
- [`core/src/lib.rs`](../../core/src/lib.rs) — `WidgetNode` declaration.
- [`core/src/event.rs`](../../core/src/event.rs) — `Event` shape used by the planned compile-as-mod gate.
- [`widgets/src/button.rs`](../../widgets/src/button.rs) — `set_on_click` signature.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures (existing `clickable.qml` plus forthcoming `counter.qml`).

## §14 — Unblocks

Ratifying QT-04b unblocks:

- The implementation pass under commit prefix `QT-04b:`.
- `QT-04c` — reactive bindings, MouseArea, nested ID resolution.
  Now has a stable `ScreenState` to wire reactivity through.
- `QT-05` — state machines. Can compose `ScreenState` with an
  emitted state-transition table without re-deciding where state
  lives.
- Project-side automation. Build scripts that wrap `qt emit` can
  read state from `Rc<RefCell<ScreenState>>` for bring-up sims.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified. `ScreenState` shape (§3), `build_screen` signature change (§3 / §10), supported property types (§5), default-value lowering (§6), handler-body grammar (§7), ID resolution scope (§8), version bump plan to `QT_EMIT_VERSION_RLVGL = 4` (§11) frozen. Implementation deferred to next pass under commit prefix `QT-04b:`. |
| 2026-04-29 | Implementation landed. `pub struct ScreenState` (always emitted, even when empty) + `build_screen(bounds) -> (WidgetNode, Rc<RefCell<ScreenState>>)` + helpers threading `state` per §3. Property literal defaults lower per §6 (i32 / f32 / bool / String). Handler bodies matching the §7 grammar (`<ident> += <int>`, `-= <int>`, `= <literal>`, `= !<bool_ident>`) lower under a single `state.borrow_mut()` per closure with `// QT-04b body:` per-line markers. Non-matching bodies fall through to QT-04's `// QT-04 body:` + `// TODO QT-04c:` placeholder. `QT_EMIT_VERSION_RLVGL` bumped to `4`. New `counter.qml` fixture + 3 goldens + 3 drift gates. Synthetic-click compile-as-mod gate ([`creator_qt_emit_counter_compile.rs`](../../tests/creator_qt_emit_counter_compile.rs)) fires `Event::PressRelease` and asserts `state.borrow().count == 1` (and `== 2` after a second click). The `// emitter-skipped (QT-04+):` property line is renamed to `// emitter-skipped (QT-04c+):` and now subtracts properties that were lowered to `ScreenState`. |
| 2026-04-29 | Note from QT-04c ([`04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) §6): the `// TODO QT-04c:` placeholder QT-04b emitted in fall-through closures has been **renamed to `// TODO QT-04e: lower QML expression to Rust.`** since QT-04c shipped narrowly (initial-value text bindings only) and reactive expression lowering is now QT-04e's responsibility. Existing fall-through bodies in `clickable.rlvgl.rs` carry the renamed marker. |
| 2026-04-29 | §8 amended by QT-04f ([`04f-nested-id-resolution.md`](./04f-nested-id-resolution.md)): the single-scope rule expanded to "root scope (un-namespaced) + non-root id scopes (namespaced as `<sanitized_id>_<prop>`)". Resolution walks both via the shared `resolve_state_field_ref` helper. Same-day implementation. |
| 2026-04-29 | §3 amended by QT-04e ([`04e-reactive-bindings.md`](./04e-reactive-bindings.md)): `build_screen` return type extended from `(WidgetNode, Rc<RefCell<ScreenState>>)` to `(WidgetNode, Rc<RefCell<ScreenState>>, Vec<LabelBinding>)`. Helper signatures gain `&mut Vec<LabelBinding>`. The state-threading model from this chapter is preserved verbatim; QT-04e adds the binding accumulator alongside. |

---

MIT-licensed: MIT.
