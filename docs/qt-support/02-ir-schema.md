<!--
02-ir-schema.md - QT-02: qt-ir schema freeze, regeneration workflow, drift gates.
-->

**[← Prev](00-concepts.md) · [Index](README.md) · [Next →](#)** *(QT-03 not yet authored)*

# Chapter QT-02 — `qt-ir` Schema Freeze

This chapter takes the IR types defined by the QT-01a MVP, locks them
behind a checked-in JSON Schema artifact, and ratifies the
regeneration workflow that earlier chapters defer to. It does not
introduce new IR types — that authority remains with QT-00.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary is
canonicalised in [QT-00 §3](./00-concepts.md#3--canonical-glossary).
The schema-bumping mechanics are owned here; the **bumping policy**
proper lives in [QT-00 §7](./00-concepts.md#7--frozen-decision-ir-schema-version)
and is referenced rather than restated.

## §1 — Purpose

QT-02 freezes three artifacts and the workflow that ties them
together:

1. The canonical JSON Schema at
   [`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json).
2. The canonical golden ingest at
   [`tests/fixtures/qt/hello.qt-ir.json`](../../tests/fixtures/qt/hello.qt-ir.json).
3. The regeneration command names embedded in the failure messages
   of the schema-drift and golden-file tests.

After ratification, the IR types in
[`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) **MUST NOT**
change without (a) following the bumping policy in QT-00 §7 and
(b) regenerating both artifacts in the same commit.

## §2 — Problem Statement

Without a checked-in schema, the IR contract exists only inside the
binary's source. Two failure modes follow:

- A field rename (say, `default_value` → `defaultValue`) compiles
  cleanly, passes the existing roundtrip test, and silently breaks
  every downstream consumer that hand-wrote types against the schema
  output of `rlvgl-creator qt schema`.
- An emitter PR claiming "no IR changes" can in fact change the IR
  output (e.g. a new field with `#[serde(default)]` shifts the JSON
  text), and there is no machine-readable diff a reviewer can
  point at.

Evidence from the existing schemars-emitted output shape
([`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json)
lines 1–30): the schema is byte-stable across regenerations of the
same source, so a checked-in canonical copy is a sound diff target.

The sibling `schemas/config.schema.json` and
`schemas/mcu_canonical.schema.json` already use this pattern. QT-02
mirrors it.

## §3 — Canonical Glossary (delta only)

QT-02 introduces no new IR types. The terms below are owned by QT-00
unless otherwise noted; the entries here record only QT-02-side
deltas.

### Canonical schema artifact

The file at
[`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json).
Owned by this chapter; emitted by `rlvgl-creator qt schema`.
Its `$id` is **`https://rlvgl.dev/schemas/qt-ir.schema.json`**
(canonical URL, not a fetched resource — the file is the source of
truth, the URL is an identifier).

### Canonical golden ingest

The file at
[`tests/fixtures/qt/hello.qt-ir.json`](../../tests/fixtures/qt/hello.qt-ir.json),
which is the byte-for-byte expected output of
`rlvgl-creator qt ingest tests/fixtures/qt/hello.qml`. Owned by this
chapter. The top-level `source` field is **excluded** from the
comparison — it captures the input path verbatim and naturally
varies between absolute and relative invocations.

### Schema-drift gate / golden-file gate

The two test cases in
[`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) and
[`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs)
that compare regenerated artifacts against the checked-in canonical
copies. Failure messages **MUST** name the regen command.

## §4 — Source-of-Truth Map

| Concept                                              | Owner                                                                  |
| ---------------------------------------------------- | ---------------------------------------------------------------------- |
| IR type definitions                                  | QT-00 (cites [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs))   |
| IR schema bumping policy                             | QT-00 §7                                                                |
| Canonical schema artifact                            | this chapter                                                            |
| Canonical golden ingest                              | this chapter                                                            |
| Schema `$id` and `$comment` strings                  | this chapter (cites [`QT_IR_SCHEMA_ID` / `QT_IR_SCHEMA_COMMENT`](../../src/bin/creator/qt.rs)) |
| Regeneration commands                                | this chapter (§5)                                                       |
| Drift / golden-file gates                            | this chapter                                                            |

## §5 — Frozen Decision: Regeneration Commands

The schema **MUST** be regenerated with:

```bash
cargo run --features creator --bin rlvgl-creator -- \
    qt schema --out schemas/qt-ir.schema.json
```

The canonical golden ingest **MUST** be regenerated with:

```bash
cargo run --features creator --bin rlvgl-creator -- \
    qt ingest tests/fixtures/qt/hello.qml tests/fixtures/qt
mv tests/fixtures/qt/qt-ir.json tests/fixtures/qt/hello.qt-ir.json
```

These exact strings appear inside the panic messages of the drift /
golden-file gates. Reviewers **SHOULD** copy them from the test
output rather than re-typing.

Both regen commands are pure local actions (no network, no homebrew,
no PySide6) — the QT-INGEST.md "external dependencies: none" entry
applies.

## §6 — Frozen Decision: Field Inclusions in Comparison

When comparing IR JSON for the **golden-file gate**, the following
fields are treated as informative metadata and **MUST** be excluded
from the equality check:

| Field           | Excluded? | Why                                                                          |
| --------------- | --------- | ---------------------------------------------------------------------------- |
| `source`        | yes       | Captures the input path verbatim; varies between absolute / relative paths. |
| `version`       | no        | Material to the bumping policy in QT-00 §7.                                 |
| `imports`       | no        | Structural.                                                                  |
| `pragmas`       | no        | Structural.                                                                  |
| `root` and below | no       | Structural.                                                                  |

Adding a new metadata field that should be excluded requires a
**Specification-Required** amendment to this table.

## §7 — Frozen Decision: Schema Determinism

The emitted schema **MUST** be byte-stable across regenerations on
the same toolchain. Concretely:

- `serde_json::Map` ordering is alphabetical (BTreeMap), so all keys
  emit in lexicographic order. Reviewers **SHOULD NOT** be surprised
  to see `$comment` before `$defs`.
- The `$id` and `$comment` fields are inserted by the `decorate_schema`
  helper in [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs); their
  exact strings are owned by this chapter (§3, §5).

If a future schemars upgrade changes the emit shape, the drift gate
will fail loudly — that is the intended early-warning. Resolution
**MUST** be a Specification-Required amendment to this chapter.

## §8 — Reconciliation with Adjacent Schemas

[`schemas/`](../../schemas/) currently houses three other JSON Schema
files: `config.schema.json`, `mcu_canonical.schema.json`, and
`ip_canonical.schema.json`. They all carry `$schema`, `$id`, and a
`$comment` naming the file. `qt-ir.schema.json` follows the same
convention via the `decorate_schema` helper — there is no namespace
overlap (Qt schema is `qt-ir`, the others are board / chip overlays)
and no need to factor a shared decorator.

If the BSP / chipdb generator ever exports its own typed IR schema,
the `decorate_schema` helper **SHOULD** be lifted into a shared
`creator::schema` module at that point. Until then, the duplication
cost is one ~25-line helper.

## §9 — Non-Goals

- **No external schema validator.** A JSON Schema validator crate
  (e.g. `jsonschema`) is **not** added. Reviewers and CI rely on the
  byte-equivalence drift gate, not on validating arbitrary JSON
  against the schema. If a future phase needs validator-side
  guarantees, that is a separate Specification-Required addition.
- **No upstream schema publication.** The `$id` URL is an
  identifier, not a fetched resource. Nothing outside the repo is
  expected to resolve it.
- **No multi-version schema retention.** When the version bumps,
  the old schema file is replaced. Earlier IR text is recoverable
  from git history; we do not maintain a `qt-ir-v1.schema.json`
  alongside a `qt-ir-v2.schema.json`.

## §10 — Reconciliation with Adjacent QT-NN Phases

| Phase    | Concern                                                              | Resolution                                                                       |
| -------- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, IR type set, schema-version bumping policy.              | Owned by QT-00. QT-02 cites without restatement.                                 |
| QT-01a   | Structural ingest. Produces the IR this chapter freezes.             | QT-01a's behavior is unchanged. The golden file pins its current output.         |
| QT-01b   | Type-introspection ingest (PySide6 / `qmlplugindump`).               | **MUST** produce IR that validates under the same schema. Bumps **SHOULD** be additive. |
| QT-03    | rlvgl emitter — widgets.                                             | **MUST** consume the canonical schema. Adding emit-only fields is a QT-02 amendment. |
| QT-04+   | Bindings, state machines, theme tokens, CLI growth.                  | Same. New IR shapes go through QT-02's bumping policy.                           |

## §11 — Non-Goals (restated)

- No external validator dependency.
- No URL-resolved schema.
- No multi-version retention.

## §12 — Acceptance Checklist

QT-02 is **ratified** when:

- [x] [`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json) exists,
      decorated with `$schema` / `$id` / `$comment`, byte-stable across regenerations.
- [x] [`tests/fixtures/qt/hello.qt-ir.json`](../../tests/fixtures/qt/hello.qt-ir.json) exists.
- [x] The schema-drift unit test in
      [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) (`schema_matches_checked_in_canonical`) passes,
      and its failure message names the §5 regen command.
- [x] The golden-file integration test in
      [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) (`qt_ingest_matches_canonical_golden`) passes,
      and its failure message names the §5 regen command.
- [x] The roundtrip test (`canonical_fixture_roundtrips`) passes.
- [x] `pub const QT_IR_SCHEMA_ID` and `pub const QT_IR_SCHEMA_COMMENT`
      exist in [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) and are cited from §3.
- [x] [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) names the canonical schema location.
- [x] §15 carries a dated initial change-log entry.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) — practical setup.
- [`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json) — canonical schema.
- [`schemas/config.schema.json`](../../schemas/config.schema.json) — sibling schema convention.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — IR types + `render_schema` helper.
- [`tests/fixtures/qt/hello.qml`](../../tests/fixtures/qt/hello.qml) — canonical fixture.
- [`tests/fixtures/qt/hello.qt-ir.json`](../../tests/fixtures/qt/hello.qt-ir.json) — canonical golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — golden-file gate.

## §14 — Unblocks

Ratifying QT-02 unblocks:

- `QT-01b` — type-introspection ingest, which now has a stable
  schema target to add fields to (under the bumping policy).
- `QT-03` — rlvgl emitter, which can wire its codegen against the
  canonical schema and regression-test by ingesting `hello.qml` and
  consuming the golden.
- External tools and editors — anything that wants typed completion
  on a `qt-ir.json` blob can pull the checked-in schema directly.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-28 | Initial ratification. Schema artifact, golden ingest, drift / golden-file gates, regen commands frozen. Schema version remains `1` per QT-00 §7. |

---

MIT-licensed: MIT.
