<!--
03-rlvgl-emitter-widgets.md - QT-03: data-only Rust emit shape, file naming, escape policy.
-->

**[← Prev](02-ir-schema.md) · [Index](README.md) · [Next →](#)** *(QT-03b not yet authored)*

# Chapter QT-03 — Rust Emitter (Data-Only Shape)

QT-03 lowers a `qt-ir` `UiModule` into a self-contained Rust module
describing the parsed tree as static data. It deliberately does **not**
yet wire the tree into `rlvgl-ui` widget constructors — that mapping
work is large enough to deserve its own chapter and is reserved for
**QT-03b**. This chapter freezes only the *output shape* and the
machinery around it (file naming, string escaping, drift gate, compile
gate).

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
emit-shape version is owned here ([§7](#7--frozen-decision-emit-shape-version))
and bumps under the same Specification-Required policy as the IR
schema in QT-00 §7.

## §1 — Purpose

Take a `UiModule` produced by QT-01a and produce a Rust source file
that:

1. Compiles `no_std`-clean with no external crate dependencies.
2. Captures every QML construct that QT-01a captured, either as
   typed data or as a `// emitter-skipped:` comment.
3. Is byte-stable across regenerations.
4. Is `mod`-includable so a downstream test target proves that
   "compiles" is part of the gate, not just "looks reasonable in a
   diff".

## §2 — Problem Statement

Without an emit step, `qt-ir.json` is a dead artifact: useful for
inspection, useless for actually building UI. A full lowering to
`rlvgl-ui` (widget constructors, theme tokens, signal wiring) is
multi-chapter work; landing it as one PR risks the
"rename / drift / fork" failure mode that QT-00 §2 cites.

QT-03 splits the problem in two:

- **QT-03 (this chapter)** — data-only shape. Lock the file layout,
  the comment headers, the `Node` / `Assignment` struct names, the
  string-escape rule. No widget mapping.
- **QT-03b (later)** — replace the static `Node` literal with calls
  into `rlvgl-ui` constructors, mapping QML type names per a
  ratified table.

This split matches the existing ESP BSP precedent
([`docs/creator/BSP-STATUS.md`](../creator/BSP-STATUS.md)): freeze
template names first, grow per-peripheral coverage after.

## §3 — Canonical Glossary (delta only)

QT-03 introduces no new IR types. The terms below are owned by this
chapter unless noted.

### Emit-shape

The structural layout of the emitted Rust file: header banner,
attribute lines, `Node` / `Assignment` struct definitions, the
top-level `pub static SCREEN: Node = …;` literal. Owned here. The
`render_rs` helper in
[`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) is the canonical
producer.

### `Node`, `Assignment` (emitted Rust types)

`Node` carries `type_name: &'static str`, `id: Option<&'static str>`,
`assignments: &'static [Assignment]`, `children: &'static [Node]`.
`Assignment` carries `target: &'static str`, `value: &'static str`.
Both `#[derive(Debug, Clone, Copy)]`. **Adapted** from the IR types
`UiItem` / `UiAssignment` (QT-00 §3) — they carry only the subset
representable as `&'static` data, with property declarations, signal
declarations, and signal handlers omitted (see §6).

### Emitter-skipped marker

A line of the form `// emitter-skipped (QT-NN…): <description>`
inside an emitted file. Surfaces a QML construct that the current
phase does not lower. Reviewers can grep for these to find what is
intentionally elided. Owned here; the prefix `// emitter-skipped`
is a stable string that future phases **MUST NOT** rename without
amending this chapter.

### Compile-as-mod gate

The integration test in
[`tests/creator_qt_emit_compile.rs`](../../tests/creator_qt_emit_compile.rs)
that includes the canonical golden via `#[path]` so that any
non-compiling output (unbalanced braces, invalid Rust escapes, dangling
type references) breaks the test binary's build.

### Golden Rust file

[`tests/fixtures/qt/hello.rs`](../../tests/fixtures/qt/hello.rs).
Owned by this chapter. Regenerated via the §5 command.

## §4 — Source-of-Truth Map

| Concept                                    | Owner                                                                |
| ------------------------------------------ | -------------------------------------------------------------------- |
| `qt-ir` IR types                           | QT-00                                                                 |
| `qt-ir` schema artifact                    | QT-02                                                                 |
| Emit-shape                                 | this chapter                                                          |
| `Node` / `Assignment` struct names         | this chapter                                                          |
| Emit-shape version constant                | this chapter (cites `QT_EMIT_VERSION` in [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)) |
| String-escape rule                         | this chapter (§8)                                                     |
| Emitter-skipped marker syntax              | this chapter (§3)                                                     |
| Golden Rust file                           | this chapter                                                          |
| Drift / compile-as-mod gates               | this chapter                                                          |
| Widget API mapping (QML → `rlvgl-ui`)      | **QT-03b** (not started)                                              |

## §5 — Frozen Decision: Regeneration Command

The canonical golden file **MUST** be regenerated with:

```bash
cargo run --features creator --bin rlvgl-creator -- \
    qt emit tests/fixtures/qt/hello.qml tests/fixtures/qt
```

Run from the workspace root. The command writes `hello.rs` directly
into `tests/fixtures/qt/`; no rename step (unlike the QT-02 ingest
flow). The drift-gate test panic message reproduces this string
verbatim.

## §6 — Frozen Decision: What Is Emitted vs. Skipped

| QML construct            | QT-03 treatment                                                                  |
| ------------------------ | -------------------------------------------------------------------------------- |
| Type instance (`Item { … }`) | `Node { type_name, id, assignments, children }` literal.                     |
| `id: <ident>`            | Stored on the parent `Node` as `id: Some(&"…")`.                                  |
| `target: <expression>`   | Single `Assignment { target, value }` entry. `value` is the opaque QML expression text. |
| `target.dotted: <expr>`  | Same; the dotted target is preserved verbatim per QT-00.                          |
| `target: SomeType { … }` (object value) | **Skipped** with `// emitter-skipped (QT-03b): <target>: <object>` comment immediately above the assignments slice. |
| `target: [ … ]` (list value) | **Skipped** with `// emitter-skipped (QT-03b): <target>: <list>` comment.   |
| `[default] [readonly] property …` declaration | **Skipped** with `// emitter-skipped (QT-04+): N property declaration(s)` summary on the parent `Node`. |
| `signal name(...)` declaration | **Skipped** with `// emitter-skipped (QT-04+): N signal declaration(s)`.    |
| `onSignal: …` handler binding | **Skipped** with `// emitter-skipped (QT-04+): N signal handler(s)`.        |
| `function name(...) { … }` | **Skipped** (lives in IR's `handlers` with `function:` prefix; reflected in the signal-handler count above). |
| `import …`               | Captured only in the JSON IR, not in the emitted Rust. (Imports become `use` decisions in QT-03b.) |
| `pragma …`               | Same — IR only.                                                                   |

The "what is skipped" set is **frozen**. Adding a new emit category
(e.g. lowering an object-valued assignment to an inline child node)
**MUST** be a Specification-Required amendment to this table and bump
[`QT_EMIT_VERSION`](../../src/bin/creator/qt.rs).

## §7 — Frozen Decision: Emit-Shape Version

The emit-shape version is **`1`** at QT-03. The constant lives at
[`src/bin/creator/qt.rs::QT_EMIT_VERSION`](../../src/bin/creator/qt.rs).

Bumping policy: **Specification Required**. The emit-shape version
**MUST** be incremented when:

- The `Node` or `Assignment` struct field set changes.
- A struct is renamed.
- The header banner format changes (lines reviewers grep for).
- The `// emitter-skipped` marker prefix is renamed.
- The string-escape strategy in §8 changes (e.g. raw strings instead
  of Debug-formatted literals).

Adding a *new* emitter-skipped category (e.g. lowering a previously
skipped construct) **SHOULD** bump the version, since it is a
behaviour change visible in the golden diff.

The bump **MUST** appear in this chapter's §15 change log together
with a regen of the canonical golden.

## §8 — Frozen Decision: String Escaping

Every QML-derived `&str` written into the emitted Rust **MUST** be
formatted via Rust's Debug formatter (`format!("{:?}", s)`). This
trivially produces a valid Rust string literal: outer double quotes,
escaped `"`/`\\`/control chars/non-ASCII unicode.

Implementations **MUST NOT** hand-build raw strings (`r#"…"#`) for
emission — choosing the right number of `#` markers for arbitrary
input is a recurring footgun that the Debug formatter avoids by
construction.

The `rust_str_lit` helper in
[`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) is the canonical
implementation; reviewers **SHOULD** verify all string-bearing emit
paths funnel through it.

## §9 — Non-Goals

- **No widget mapping in QT-03.** Calls into `rlvgl-ui` constructors
  are out of scope; that is QT-03b's job.
- **No layout solver.** QML's anchor / layout system is preserved as
  raw expression text in `Assignment.value`. Resolving it to
  `rlvgl-ui::layout` calls is QT-03b.
- **No multi-screen project support.** One `.qml` in, one `.rs`
  out. Multi-file projects, `.qmldir` resolution, and asset bundling
  are QT-07 / QT-08.
- **No emitted `mod` index file.** Each emitted file is independent.
  A future generated `mod.rs` aggregating multiple screens is QT-08.
- **No external crate dependency.** The emitted file **MUST** compile
  without pulling in `rlvgl-core`, `rlvgl-ui`, `serde`, or anything
  else. Self-containment is the QT-03 contract.

## §10 — Reconciliation with Adjacent QT-NN Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR types, schema-version policy.                     | Cited; not restated.                                                                                   |
| QT-01a   | Structural ingest. Produces the `UiModule` this chapter consumes. | Unchanged.                                                                                             |
| QT-01b   | Type-introspection ingest.                                       | Will populate fields the emitter currently passes through opaque (e.g. `Assignment.value`). Emit-shape version stays `1` if those fields keep the same type. |
| QT-02    | IR schema artifact.                                              | The golden Rust file is **secondary** to the JSON IR; the IR remains the canonical contract. Drift in the Rust output that has no IR-side counterpart is a QT-03 amendment. |
| QT-03b   | Widget API mapping (QML → `rlvgl-widgets`).                      | Ratified — see [`03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md). Splits `QT_EMIT_VERSION` into `QT_EMIT_VERSION_DATA` (stays `1`) and `QT_EMIT_VERSION_RLVGL` (`2`). Adds `--target {data,rlvgl}` flag with `data` default until QT-03b implementation ships, then flips to `rlvgl`. Keeps the `// emitter-skipped` / `// emitter-fallback` comment prefixes. |
| QT-04    | Bindings + handlers.                                             | Replaces the `// emitter-skipped (QT-04+):` comment lines with real lowered constructs. Same version-bump rules. |
| QT-08    | CLI surface growth.                                              | A future `qt emit` flag set (e.g. `--mod` for an aggregating index file) lives there.                  |

## §11 — Non-Goals (restated)

- No widget mapping yet.
- No layout solver.
- No multi-screen aggregation.
- No external crate dependency in the emitted file.

## §12 — Acceptance Checklist

QT-03 is **ratified** when:

- [x] [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) defines
      `pub fn render_rs(&UiModule) -> String` and `pub(crate) fn emit(input, out)`.
- [x] [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) defines
      `pub const QT_EMIT_VERSION: u32 = 1`.
- [x] [`tests/fixtures/qt/hello.rs`](../../tests/fixtures/qt/hello.rs) exists.
- [x] The drift gate (`qt_emit_matches_canonical_golden_rs` in
      [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs)) passes,
      and its failure message names the §5 regen command.
- [x] The compile-as-mod gate
      ([`tests/creator_qt_emit_compile.rs`](../../tests/creator_qt_emit_compile.rs))
      passes — the golden file is consumable as a Rust module.
- [x] The §6 emit-vs-skip table covers every QML construct currently
      captured by QT-01a's IR.
- [x] The §8 string-escape rule names the canonical `rust_str_lit` helper.
- [x] §15 carries a dated initial change-log entry.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/02-ir-schema.md`](./02-ir-schema.md) — IR schema gate.
- [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) — practical setup.
- [`docs/creator/BSP-STATUS.md`](../creator/BSP-STATUS.md) — emit-grow precedent.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `render_rs` / `emit` / `rust_str_lit`.
- [`tests/fixtures/qt/hello.rs`](../../tests/fixtures/qt/hello.rs) — canonical golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.
- [`tests/creator_qt_emit_compile.rs`](../../tests/creator_qt_emit_compile.rs) — compile-as-mod gate.

## §14 — Unblocks

Ratifying QT-03 unblocks:

- `QT-03b` — widget API mapping. Now has a stable file layout to grow
  inside, with both gates already in place to catch regressions.
- `QT-04` — bindings / handlers. Can amend the §6 table to lower the
  currently-skipped `// emitter-skipped (QT-04+):` comments into real
  Rust expressions.
- Project-side automation. Build scripts can shell out to
  `rlvgl-creator qt emit` and trust the output to compile.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-28 | Initial ratification. Emit-shape version `1`, golden `tests/fixtures/qt/hello.rs`, drift + compile-as-mod gates added. Widget API mapping deferred to QT-03b. |

---

MIT-licensed: MIT.
