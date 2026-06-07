<!--
05d-emit-scjson.md - QT-05d: QML States{} → scjson emission.
-->

**[← Prev](05c-machine-bindings.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05d — QML `states:` / `transitions:` → scjson

QT-05a populates `UiModule.state_machine` from a hand-authored
sibling `.scjson`. QT-05d closes the **authoring** loop: when the
author declares states inline in QML via `states: [State { … }]`
and `transitions: [Transition { … }]` blocks, `rlvgl-creator qt
emit-scjson` walks them into a `qt_scjson::Scxml` document and
writes a sibling `.scjson` file. The next QT-05a ingest run picks
that file up and produces the same `UiStateMachine` the inline
QML declared.

QT-05d **does not** introduce a new IR. It introduces a new CLI
subcommand and a new walk algorithm; the existing `UiAssignment`
/ `UiItem` shapes from QT-01a carry the QML structure unchanged.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
and [QT-05a §3](./05a-scjson-ingest.md#3--canonical-glossary-delta-only).
The `States{}` / `Transition{}` QML idiom, the `qt emit-scjson`
CLI, and the round-trip parity contract are owned here.

## §1 — Purpose

After QT-05a/b/c, every Qt screen with a state machine requires a
hand-authored `.scjson` next door. That works, but:

- Authors prefer single-file declarations during early
  prototyping.
- istate-codegen consumes scjson, so the authoring tool would
  need to either translate or import. Skipping the translate step
  is brittle.
- Round-tripping through QT-05a's ingest is the cleanest way to
  verify that what the QML *says* matches what istate-codegen
  *receives*.

QT-05d gives a one-command bridge: write QML, run `qt emit-scjson`,
get an istate-ingestible `.scjson`. Subsequent edits to the QML
re-emit the file; conflicts are resolved by treating QML as the
source of truth and the `.scjson` as a regenerated artifact.

## §2 — Problem Statement

The QT-01a parser already accepts arbitrary `states: […]` and
`transitions: […]` lists as `UiAssignment` values — the structure
is captured, but no chapter promotes it to IR. Three concrete
gaps:

- **No CLI command.** `rlvgl-creator qt emit-scjson` does not
  exist; authors who want to generate a `.scjson` from QML have
  to round-trip through Qt Design Studio (which has its own
  state-machine editor) or write it by hand.
- **No QML-to-scjson lowering rules.** `Transition { from:
  "idle"; to: "running" }` could lower three different ways
  (event-attribute on `<transition>`, `<send>` payload,
  parameter-named transition); the chapter freezes one canonical
  form.
- **No round-trip verification.** Without a parity test, drift
  between the inline-QML walk and the QT-05a ingest walk would
  go undetected — the second pass through `qt ingest` (which
  reads the produced `.scjson`) and direct walk of the inline QML
  could disagree silently.

QT-05d closes all three by emitting a deterministic, scjson-subset
document and pinning the round-trip via a fixture-level test.

## §3 — Canonical Glossary (delta only)

QT-05d introduces no new IR types and no new emit-shape
constants. Three new internal helpers and one CLI subcommand.

### `qt emit-scjson <input> [<out>]`

CLI subcommand. File-mode: `<input>` is a `.qml`; writes the
`.scjson` to `<out>` (a path) or `<input_dir>/<basename>.scjson`
(default). Directory-mode: `<input>` is a directory; walks every
`*.qml` per QT-08 and emits `<basename>.scjson` next to each.

Idempotent: running twice produces byte-identical output.
Side-effect-free aside from filesystem writes.

### `walk_qml_state_machine(item)`

Pure function. Given a `UiItem`, returns
`Option<qt_scjson::Scxml>`:

- `None` if the item has no `states:` and no `transitions:`
  assignments.
- `Some(scxml)` populated per §5/§6 otherwise.

The walk is structural — it does not validate semantics
(reachability, deterministic transitions, etc.). istate's
codegen pipeline owns that.

### `// QT-05d emit-scjson:` marker

Emitted as the `_comment` field on the produced `Scxml` (lifted
through `Scxml.other_attributes` since the canonical pydantic
shape doesn't carry a `_comment` field). Names the source QML
path so reviewers can trace the `.scjson` back to its origin
without grepping git log.

## §4 — Source-of-Truth Map

| Concept                                    | Owner                                                                  |
| ------------------------------------------ | ---------------------------------------------------------------------- |
| QML structural parse (`UiItem` tree)       | QT-01a.                                                                 |
| `qt_scjson::Scxml` on-disk shape           | QT-05.                                                                  |
| `UiStateMachine` IR (consumer of scjson)   | QT-05.                                                                  |
| QT-05a side-file ingest                    | QT-05a.                                                                 |
| `states:` / `transitions:` QML idiom       | this chapter (§5).                                                      |
| `walk_qml_state_machine` algorithm         | this chapter (§6).                                                      |
| `qt emit-scjson` CLI                       | this chapter (§7).                                                      |
| `// QT-05d emit-scjson:` provenance marker | this chapter (§3).                                                      |
| Round-trip parity contract                 | this chapter (§8).                                                      |
| QML `StateMachine {}` framework form       | **deferred** — Qt's `import QtQml.StateMachine 1.0` blocks; QT-05d v2.   |

## §5 — Frozen Decision: Supported QML Idiom

Registration policy: **Specification Required**.

```qml
import QtQuick 2.15

Item {
    id: root

    states: [
        State {
            name: "idle"
            // optional — exactly one State MAY carry `initial: true`.
            initial: true
        },
        State { name: "running" }
    ]

    transitions: [
        Transition { from: "idle"; to: "running"; event: "start" },
        Transition { from: "running"; to: "idle"; event: "stop" }
    ]
}
```

| QML form                                 | Status                                                                                         |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `states: [State { name: "X" } …]`        | **shipped** — each `State.name` becomes `<state id="X">`.                                      |
| `State { name: "X"; initial: true }`     | **shipped** — sets `<scxml initial="X">`. Multiple `initial: true` → emit-time error.          |
| `transitions: [Transition { from: …; to: …; event: … } …]` | **shipped** — each becomes `<transition event="…" target="…">` nested in the matching `<state>`. |
| `Transition { from: "*"; to: "X" }`      | **deferred** — wildcard `from` requires SCXML interpretation we don't have at this layer.       |
| `Transition { signal: "…" }` (Qt animation form) | **deferred** — Qt animation framework, not SCXML; the `signal` attribute is silently ignored by QT-05d. |
| `State { PropertyChanges { … } }`        | **deferred** — these are visual state declarations, not SM semantics. Silently dropped.        |
| `StateMachine { import QtQml.StateMachine 1.0 … }` | **deferred** — separate authoring path; QT-05d v2.                                |
| `events:` block / `<datamodel>` inline   | **deferred** — for now, datamodel is owned by hand-authored scjson side-files.                |

The deferred rows are silently passed through the walker (so the
emitted `.scjson` is structurally minimal, not failing on
unrecognized fields). A future amendment may promote any of them.

## §6 — Frozen Decision: Walk Algorithm

For a `UiItem` (typically the QML root):

1. **Discover**. Look for two top-level assignments in
   `item.assignments`:
   - `target == "states"` with `UiAssignmentValue::List` of
     `UiAssignmentValue::Object { item }` whose
     `item.type_name == "State"`. Other shapes are silently
     ignored.
   - `target == "transitions"` similar with `type_name ==
     "Transition"`.
   If neither is present, return `None`.
2. **Collect states**. For each `State` item in declaration
   order:
   - Read `name` from a literal-string `name:` assignment.
   - If `name` is missing or non-literal, **emit-time error**.
   - Read `initial: true` if present.
   - Push a placeholder `Scxml.state[]` entry: `qt_scjson::State {
     id: Some(name), … }` with empty `transition` list (filled in
     step 4).
3. **Validate `initial`**. Zero or one `State` may carry
   `initial: true`:
   - Zero → `Scxml.initial = []` (no top-level initial; istate
     will pick the first state at runtime).
   - One → `Scxml.initial = [name]`.
   - Two or more → emit-time error.
4. **Distribute transitions**. For each `Transition` item:
   - Read `from`, `to`, `event` from literal-string assignments.
     Missing `from` or `to` → emit-time error. `event` is
     optional (eventless transition).
   - Find the matching state by `id == from`. If not found,
     emit-time error.
   - Append `qt_scjson::Transition { event: Some(event), target:
     vec![to.into()], … }` to that state's `transition` list.
5. **Build the `Scxml` document**:
   - `state` ← collected list (declaration order preserved).
   - `initial` ← per step 3.
   - `name` ← `None` (the SCXML `name` attribute is reserved for
     use by QT-05a's `<sm>` ID derivation; QT-05d does not write
     it).
   - `other_attributes` ← `{ "_comment": "QT-05d emit-scjson: <path>"
     }`.
6. **Serialize** via `serde_json::to_string_pretty`. Trailing
   newline included.

The walk is pure: re-running on the same `UiItem` produces
byte-identical bytes.

## §7 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt emit-scjson <INPUT> [<OUT>]

ARGS:
    <INPUT>    Path to a `.qml` file or a directory containing `.qml` files.
    <OUT>      Output path (file or directory). Defaults to the input directory.
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/to/foo.qml` | `dir/`             | (parent of input) | Writes `<OUT>/foo.scjson`. |
| File | `path/to/foo.qml` | `dir/foo.scjson`   | n/a               | Writes the named file. |
| Dir  | `path/to/screens/` | `dir/`            | (input itself)    | For every `*.qml` in the dir, writes `<OUT>/<basename>.scjson`. |

Exit codes:

| Code | Meaning |
| ---- | ------- |
| 0    | All `.qml` files yielded a state machine and were written. |
| 0    | A `.qml` file had no `states:`/`transitions:` blocks (silent skip). |
| Non-0 | Any QT-05d §6 emit-time error in any input file. The CLI prints the offending QML file path and the offending construct. |

## §8 — Frozen Decision: Round-Trip Parity

Registration policy: **Specification Required**.

For every `.qml` containing `states:` / `transitions:` blocks, the
following diagram MUST commute:

```
        QML
         │
         │ walk_qml_state_machine
         ▼
      Scxml (in memory)
         │
         │ serde_json::to_string_pretty
         ▼
      .scjson on disk        ──── byte-stable ────►   identical bytes on re-emit
         │
         │ qt::ingest (QT-05a side-file probe)
         ▼
      UiStateMachine ─── shape-equal ───►   walk_scxml_into_ui_state_machine(walk_qml_state_machine(qml))
```

Concretely: ingest of the emitted `.scjson` MUST produce a
`UiStateMachine` whose `id`/`states`/`transitions`/`initial` agree
with what direct in-memory walking of the QML would produce. The
QT-05d compile-as-mod gate enforces this on the
`tests/fixtures/qt/inline_states.qml` fixture.

Acceptable mismatches (recorded as deltas):

- `UiStateMachine.source` differs (QT-05d emit doesn't set it;
  QT-05a populates from the `.scjson` filename). Comparison
  ignores this field.
- `UiStateMachine.id` differs (QT-05a derives from QML basename;
  QT-05d direct walk doesn't set it). Comparison ignores this
  field.
- `UiStateMachine.scripts` empty in both paths (QT-05d doesn't
  emit `<script>` from inline QML; deferred per §5). Equal-empty.
- `UiStateMachine.datamodel` empty in both paths (same reason).
  Equal-empty.

## §9 — Non-Goals

- **No state-machine semantics enforcement.** Reachability,
  deterministic transitions, and parallel-region wellformedness
  remain istate's concern.
- **No `Transition` animation properties.** `from: "*"`, `signal:
  …`, `PathAnimation`, etc. are silently dropped.
- **No `<datamodel>` emission.** QT-05d v1 keeps datamodel
  authoring on the scjson side; users wanting a DM with their
  inline QML write a sibling scjson per QT-05a or wait for a
  QT-05d amendment.
- **No `.scjson` → `.qml` reverse path.** QT-05d is one-way:
  QML is canonical, `.scjson` is regenerated.
- **No live MCP integration.** `qt emit-scjson` writes the
  `.scjson` file but does not invoke istate-codegen; the user
  runs that step separately per QT-05 §7.
- **No partial emit.** A `.qml` either has the full
  `states:`/`transitions:` shape or doesn't. Mixed / malformed
  shapes are emit-time errors.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                | Resolution                                                                                                                                                                  |
| -------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| QT-01a   | Structural QML parse (`UiItem` tree, including `states:` / `transitions:` lists). | QT-05d consumes the existing `UiAssignmentValue::List` of `UiAssignmentValue::Object`. No parser changes.                                                                  |
| QT-05    | scjson element subset; `qt_scjson::Scxml` types.       | QT-05d uses the existing types verbatim. Deferred-element handling per QT-05 §5 applies.                                                                                   |
| QT-05a   | Side-file discovery + ingest walk.                     | QT-05a's discovery rule already finds `<basename>.scjson` next to `<basename>.qml`. After `qt emit-scjson`, the next `qt ingest` run sees the new file automatically.       |
| QT-05b/c | Emit-side handler dispatch + DM bindings.              | Independent. QT-05b/c read `UiModule.state_machine` regardless of whether QT-05a ingested a hand-authored `.scjson` or one emitted by QT-05d.                              |
| QT-05e   | Externals stub emission.                               | Independent. QT-05d-emitted scjson typically has no `<script>` callouts (per §5); a user wanting Externals authors them in the scjson directly or waits for an amendment. |
| QT-08    | Directory-mode CLI.                                    | QT-05d's directory-mode shares QT-08's `qt08_collect_qml_files` walker.                                                                                                    |

## §11 — Acceptance Checklist

QT-05d is **ratified and shipped** when:

- [x] §5 freezes the QML idiom.
- [x] §6 fixes the walk algorithm.
- [x] §7 fixes the CLI surface.
- [x] §8 fixes the round-trip parity contract.
- [x] `qt::walk_qml_state_machine` lands as a pure function.
- [x] `qt emit-scjson` CLI subcommand is wired (file mode +
      directory mode).
- [x] `tests/fixtures/qt/inline_states.qml` exists with the §5
      idiom.
- [x] `tests/fixtures/qt/inline_states.scjson` is the emitted
      golden; emit + re-emit produces byte-identical output.
- [x] A round-trip integration test asserts that ingest of the
      emitted `.scjson` produces the same `UiStateMachine`
      structure as direct walk of the QML.
- [x] No bumps to `QT_IR_VERSION` or `QT_EMIT_VERSION_RLVGL`
      (QT-05d emits a separate artifact, not part of the
      versioned emit-shapes).
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — IR types, scjson element subset.
- [`docs/qt-support/05a-scjson-ingest.md`](./05a-scjson-ingest.md) — round-trip consumer side.
- [`docs/qt-support/08-multi-file-cli.md`](./08-multi-file-cli.md) — directory walker shared with QT-05d.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter + walker.
- [`src/bin/creator/qt_scjson.rs`](../../src/bin/creator/qt_scjson.rs) — on-disk types.
- [`src/bin/creator/cli.rs`](../../src/bin/creator/cli.rs) — `qt emit-scjson` subcommand.
- [`tests/fixtures/qt/inline_states.qml`](../../tests/fixtures/qt/inline_states.qml) — canonical fixture.
- [`tests/fixtures/qt/inline_states.scjson`](../../tests/fixtures/qt/inline_states.scjson) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift / round-trip gates.

## §13 — Unblocks

Ratifying QT-05d unblocks:

- **QT-05e** (Externals stub emission) — once a QT-05d-authored
  state machine has `<script>` callouts (a future §5 amendment),
  QT-05e generates the matching `Externals` impl stubs.
- Real-project bring-up where authors prefer single-file QML
  declarations during prototyping. The `.scjson` becomes a
  generated artifact under `target/` or a regenerated checked-in
  file.
- Full QT-05 family: with QT-05a (ingest), QT-05b (handlers),
  QT-05c (bindings), QT-05d (authoring), only QT-05e
  (externals) remains for closeout.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | QT-05d ratified and shipped. New `qt emit-scjson` CLI subcommand (file mode + directory mode). New `walk_qml_state_machine(item) -> Option<Scxml>` pure walker recognising the §5 QML idiom — `states: [State { name: "…"; initial: true }]` and `transitions: [Transition { from: "…"; to: "…"; event: "…" }]` blocks at the QML root. Walker emits `qt_scjson::Scxml` with the `_comment: "QT-05d emit-scjson: <path>"` provenance attribute. Round-trip parity gate: emit produces byte-stable output; subsequent `qt ingest` of the emitted `.scjson` yields a `UiStateMachine` with `states`/`transitions`/`initial` shape-equal to direct walking of the QML. Multiple `initial: true` → emit-time error; missing `from`/`to`/`name` → emit-time error; transition referencing unknown state → emit-time error. Animation-flavoured `Transition` properties (`signal`, `PathAnimation`, etc.) and `PropertyChanges` blocks silently dropped per §5. Datamodel and `<script>` callouts deferred to scjson side-files (no inline-QML authoring path yet). New fixture `tests/fixtures/qt/inline_states.qml` + emitted golden `tests/fixtures/qt/inline_states.scjson`. Round-trip drift gate added to `tests/creator_qt_ingest.rs`. No bumps to `QT_IR_VERSION` or `QT_EMIT_VERSION_RLVGL` — QT-05d's artifact (`.scjson`) is separate from the versioned emit-shapes. QML `StateMachine{}` framework form (from `QtQml.StateMachine 1.0`), wildcard `from:`, `PropertyChanges`, and inline `<datamodel>` remain deferred under future Specification-Required amendments. |

---

MIT-licensed: MIT.
