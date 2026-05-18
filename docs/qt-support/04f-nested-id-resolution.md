<!--
04f-nested-id-resolution.md - QT-04f: nested ID resolution beyond root.
-->

**[← Prev](04c-initial-value-bindings.md) · [Index](README.md) · [Next →](#)** *(QT-05 not yet authored)*

# Chapter QT-04f — Nested ID Resolution

QT-04b §8 froze ID resolution to a single scope: the QML root.
References shaped `<root_id>.<prop>` strip the root prefix and
look up against `ScreenState`. Anything else falls through.

QT-04f extends the resolver to handle properties declared on
non-root id'd items. A reference like `bg.alpha` (where `bg` is a
non-root `Rectangle { id: bg; property int alpha: 100 }`) now
resolves and lowers, both in handler bodies and in initial-value
bindings.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-04b §3](./04b-properties-bindings.md#3--canonical-glossary-delta-only),
and [QT-04c §3](./04c-initial-value-bindings.md#3--canonical-glossary-delta-only).
The namespacing rule ([§5](#5--frozen-decision-state-field-namespacing))
is owned here and amends QT-04b §8.

## §1 — Purpose

Replace the QT-04b §8 single-scope rule with a multi-scope walk:

```rust
// QT-04b §8: only `count` (root scope) resolves. `bg.alpha` falls
// through with `// TODO QT-04e: lower QML expression to Rust`.

// QT-04f §5/§7: `bg.alpha` resolves to `state.bg_alpha`; the
// handler body lowers under the existing §7 grammar.
```

Concretely, after QT-04f a fixture like:

```qml
Item {
    id: app
    Rectangle {
        id: bg
        property int alpha: 100
    }
    Button {
        text: "Dim"
        onClicked: bg.alpha -= 10
    }
}
```

emits a `ScreenState { pub bg_alpha: i32 }`, initialises
`bg_alpha: 100`, and lowers the Button's `onClicked` to
`s.bg_alpha = s.bg_alpha.saturating_sub(10);`.

## §2 — Problem Statement

Real QML codebases routinely declare properties on non-root id'd
items: visual state on backgrounds (`bg.alpha`), per-control
counters (`btn.clickCount`), per-section flags (`panel.expanded`).
Without QT-04f, every such property is invisible to the emitter:

- The property declaration is counted in
  `// emitter-skipped (QT-04c+):` summaries but does not become a
  `ScreenState` field.
- Handler bodies referencing the property fall through to the
  QT-04 unlowered path even if the body otherwise matches the
  QT-04b §7 grammar.
- Initial-value text bindings against non-root properties also
  fall through.

QT-04f closes the gap with a deterministic namespacing rule:
non-root id'd item properties become
`state.<sanitized_id>_<prop>`. Root continues to use the bare
field name for back-compat with the QT-04b precedent.

## §3 — Canonical Glossary (delta only)

QT-04f introduces no new IR types and no new emitted-Rust struct
names. Two terms:

### Namespaced state field

A `ScreenState` field whose name is `<sanitized_id>_<prop>` rather
than the bare `<prop>`. Emitted when the property's owning item
is **not** the QML root and has a declared `id`. Owned here.

`<sanitized_id>` is `id` with characters outside `[A-Za-z0-9_]`
replaced by `_` (per the existing `sanitize_ident` helper).

### Resolution scope walk

The QT-04f rule for resolving a `<owner>.<prop>` reference:

1. If `<owner>` matches the QML root's id (or the default `"root"`
   when the root has no id), strip the prefix and look up the
   bare `<prop>` field — preserves QT-04b §8 behaviour.
2. Else, attempt to find a `<sanitized_owner>_<prop>` field in
   `ScreenState`. If present, resolve to that.
3. Else, fall through (no resolution).

The walk is **non-hierarchical** at QT-04f: any registered id is
visible from any handler body. QML's lexical-scope rule (an id is
visible only to descendants of its declaring scope) is **not**
enforced here. Strict scope checking is deferred to a future
amendment when fixtures motivate it; for the canonical fixture
set, the laxer rule is correct.

## §4 — Source-of-Truth Map

| Concept                                 | Owner                                                                  |
| --------------------------------------- | ---------------------------------------------------------------------- |
| `qt-ir` IR types                        | QT-00                                                                   |
| `ScreenState` shape                     | QT-04b §3                                                               |
| Single-scope ID resolution rule         | QT-04b §8 (amended here)                                                |
| State-field namespacing                 | this chapter (§5)                                                       |
| Resolution scope walk                   | this chapter (§3 / §7)                                                  |
| `// QT-04b body:` / `// QT-04c bound:` markers | QT-04b / QT-04c (unchanged)                                       |

## §5 — Frozen Decision: State-Field Namespacing

Registration policy: **Specification Required**.

| Item position                                | Field naming               | Notes                                                    |
| -------------------------------------------- | -------------------------- | -------------------------------------------------------- |
| QML root (with or without `id:`)             | `<prop>` (bare)            | Preserves QT-04b §8.                                     |
| Non-root item with `id: <ident>`             | `<sanitized_ident>_<prop>` | Sanitisation per the existing `sanitize_ident` helper.   |
| Non-root item with no `id:`                  | **not collected**          | Without an id, references cannot disambiguate which item the property belongs to. The property still appears in the per-node `// emitter-skipped (QT-04c+):` summary. |
| Property name collision (`<id>_<prop>` already taken) | error                | Implementations **MUST** error at emit time. Same-id collisions are ruled out by QML; cross-id collisions (e.g. `id: foo, prop bar` plus `id: foo_bar` with no prop) are rare but defensively rejected. |

Field declaration order in `ScreenState`: depth-first traversal
order of the QML tree, matching the order QT-01a's parser
populated `UiItem::children`. This keeps the struct order
predictable and diff-stable.

## §6 — Frozen Decision: Default-Value Lowering

Identical to QT-04b §6: literal `int` / `f32` / `bool` / quoted
`string` defaults lower; non-literal defaults fall back to the
type-default with a `// QT-04f: non-literal default for
<sanitized_id>_<prop>: <expr>` comment (parallel to QT-04b's
own comment, with the `<id>` prefix included).

## §7 — Frozen Decision: Resolution Walk

`resolve_string_state_ref` and `lower_handler_statement`
implement the §3 walk:

```text
input: <ident>            → root scope, bare lookup (QT-04b §8 path)
input: <root_id>.<ident>  → strip prefix, root scope, bare lookup
input: <other_id>.<ident> → namespaced lookup (`<other_id>_<ident>`)
input: <a>.<b>.<c>...     → fall through (deeper nesting unsupported at QT-04f)
input: anything else      → fall through
```

The two helpers share a single resolver helper introduced by
this chapter: `resolve_state_field_ref(expr, state_fields, root_id) -> Option<&StateField>`.
Both call sites delegate; their existing type-checks (string-only
for QT-04c bindings, type-matched for QT-04b grammar) wrap the
shared resolver.

## §8 — Versioning

QT-04f bumps `QT_EMIT_VERSION_RLVGL` from `6` to `7`. Rationale:

- `ScreenState` may grow new namespaced fields for any fixture
  that has non-root id'd items with properties.
- Resolution that used to fall through may now lower, changing
  closure / construction shapes.
- New `// QT-04f resolved:` markers (mirror of the QT-04c
  marker shape) appear above lowered references.

The data-target version `QT_EMIT_VERSION_DATA` is unchanged.

## §9 — Non-Goals

- **No QML lexical scope enforcement.** Any registered id is
  visible from any handler. Strict descendant-only visibility is
  a future amendment if/when fixtures show it matters.
- **No multi-level dotted resolution.** `a.b.c` falls through.
  Resolving sub-properties (e.g. `bg.color.r`) requires a typed
  property model that QT-04f does not introduce.
- **No reactive propagation.** Mutating `state.bg_alpha` does not
  refresh widgets. QT-04e remains the home for reactivity.
- **No widget-pointer access.** A reference like `bg.width` does
  not resolve to the runtime widget's bounds. Only `ScreenState`
  fields participate in resolution.
- **No alias properties.** QML `property alias foo: bar.baz` is
  not lowered. The alias machinery requires runtime forwarding;
  deferred indefinitely.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-04b   | Single-scope ID rule (§8).                                        | **Amended here**: §8 expanded to "root scope (un-namespaced) + non-root id scopes (namespaced)". Recorded in QT-04b §15. |
| QT-04c   | Initial-value text bindings against root-scope strings.           | Now also resolves against namespaced non-root strings via the shared `resolve_state_field_ref` helper. |
| QT-04d   | MouseArea handlers.                                                | Independent.                                                                                           |
| QT-04e   | Reactive bindings.                                                 | Will subscribe to namespaced fields the same way it subscribes to root fields.                         |
| QT-03c   | Anchor resolver.                                                   | Sibling-relative anchors (e.g. `anchors.left: bg.right`) become technically resolvable now that `bg` is a known scope, but QT-03c has not promoted that row yet. Mechanical follow-up.                       |
| QT-08    | Multi-file CLI.                                                    | Independent. Per-file emit shape is unchanged otherwise.                                               |

## §11 — Acceptance Checklist

QT-04f is **ratified and shipped** when:

- [x] §5 freezes the namespacing rule.
- [x] §6 documents default-value lowering.
- [x] §7 freezes the resolution walk.
- [x] §8 names the version bump.
- [x] `qt::render_rlvgl` emits namespaced `ScreenState` fields per §5.
- [x] `resolve_string_state_ref` (QT-04c) and
      `lower_handler_statement` (QT-04b) call a shared resolver
      that handles both root-scope and namespaced lookups.
- [x] `QT_EMIT_VERSION_RLVGL = 7`.
- [x] New canonical fixture
      [`tests/fixtures/qt/nested.qml`](../../tests/fixtures/qt/nested.qml)
      exercises a non-root id'd item with a property and a
      handler body referencing it.
- [x] Goldens for the fixture exist:
      [`nested.qt-ir.json`](../../tests/fixtures/qt/nested.qt-ir.json),
      [`nested.rs`](../../tests/fixtures/qt/nested.rs),
      [`nested.rlvgl.rs`](../../tests/fixtures/qt/nested.rlvgl.rs).
- [x] Drift gates pass.
- [x] Compile-as-mod gate fires a synthetic click and asserts
      the namespaced field mutated as expected.
- [x] All existing rlvgl-target goldens regenerated for the
      version bump; existing compile-as-mod gates' version
      assertions updated.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — single-scope rule, amended here.
- [`docs/qt-support/04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) — text bindings now using the shared resolver.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures.

## §13 — Unblocks

Ratifying QT-04f unblocks:

- QT-03c sibling-relative anchor amendments (e.g. `anchors.left:
  bg.right`) — now have a known-id resolver.
- QT-04e reactive bindings — same subscribe path applies whether
  the field is root-scoped or namespaced.
- Real-project bring-up where state lives on multiple components
  (panels, palettes, badges).

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. State-field namespacing for non-root id'd items (§5), resolution walk extended to non-root scopes (§3 / §7), shared `resolve_state_field_ref` helper used by both QT-04b and QT-04c sites, `QT_EMIT_VERSION_RLVGL` bumped `6 → 7`. New `nested.qml` fixture + 3 goldens + 3 drift gates + synthetic-click compile-as-mod gate that asserts `state.bg_alpha == 90` after firing `Event::PressRelease`. All existing rlvgl-target goldens regenerated; compile-as-mod gates' version assertions bumped. QT-04b §8 amended via this chapter. |

---

MIT-licensed: MIT.
