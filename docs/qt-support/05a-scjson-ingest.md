<!--
05a-scjson-ingest.md - QT-05a: scjson side-file ingest.
-->

**[← Prev](05-state-machines.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05a — scjson Side-File Ingest

QT-05 froze the IR types and the istate-codegen linkage surface.
QT-05a wires up the **first read** end of the pipeline:
`rlvgl-creator qt ingest` now also reads `<basename>.scjson`
side-files next to `<basename>.qml` and walks them into
`UiModule.state_machine`. Subsequent chapters (QT-05b/c/d/e)
consume that field; this chapter only **populates** it.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary) and
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only).
The on-disk scjson element/attribute names continue to defer to
[`vendor/scjson/`](../../vendor/scjson) (BSD-1-Clause submodule).
The discovery rule, walk algorithm, and error contract are owned
here.

## §1 — Purpose

Without QT-05a, QT-05's IR field `UiModule.state_machine` is always
`None` — the IR shape is in place but no source ever populates it.
QT-05a closes that loop: a `.scjson` file authored or hand-written
next to a `.qml` becomes a populated `UiStateMachine` that
QT-05b-e can lower.

After QT-05a:

```bash
$ ls
stopwatch.qml
stopwatch.scjson

$ rlvgl-creator qt ingest stopwatch.qml stopwatch.qt-ir.json
$ jq .state_machine stopwatch.qt-ir.json
{
  "id": "stopwatch",
  "source": "stopwatch.scjson",
  "initial": "idle",
  "states": [...],
  "transitions": [...],
  "datamodel": [...],
  "scripts": [...]
}
```

## §2 — Problem Statement

QT-05's seed chapter intentionally landed concepts only — there is
no end-to-end story from a Qt project to populated IR until ingest
learns to merge scjson. Three failure modes are unblocked here:

- **No discoverability**. Without an automatic side-file lookup,
  the user must invoke a separate `qt link-sm` command (deferred
  to a follow-on amendment) — adding a step that's invisible from
  `qt ingest`'s help output.
- **No round-trip anchor**. Until `qt ingest` writes a populated
  `state_machine` field, the smoke test for QT-05's IR shape is
  the synthetic unit test from QT-05 §11. A real fixture pinned to
  on-disk scjson is the next strongest gate.
- **No fall-through path** for `.qml` files that don't have a
  state machine. The natural answer — `state_machine = None` when
  no `.scjson` is found — only works if QT-05a explicitly names
  it as the contract; otherwise reviewers might assume "missing
  scjson → ingest error".

QT-05a names the contract: discovery is opportunistic, missing
scjson is silent fall-through, malformed scjson is a hard error.

## §3 — Canonical Glossary (delta only)

QT-05a introduces no new IR types. Two new internal helpers and
one comment marker.

### `find_scjson_side_file(qml_path)`

A pure path helper that, given `/path/to/foo.qml`, returns
`/path/to/foo.scjson` iff that file exists. Returns `None`
otherwise. No I/O beyond `fs::metadata`.

### `walk_scxml_into_ui_state_machine(scxml, id, source)`

Consumes a parsed [`qt_scjson::Scxml`] (the QT-05 hand-rolled
subset) and produces a [`UiStateMachine`]. The walk is shallow per
the QT-05 §5 element subset: top-level `<state>` elements become
`UiState`s; their `<transition>` children become `UiTransition`s;
`<datamodel>/<data>` becomes `UiDmField`s; `<script name="…"/>`
discovered anywhere in the tree becomes a `UiScript` with the
correct `UiScriptOrigin`. Compound nesting (deferred per QT-05 §5)
is **flattened** at QT-05a — nested states surface as additional
top-level `UiState` entries with their own ID. Parallel and history
states (also deferred) are dropped silently and recorded in the
QT-05a §5 fall-through table for traceability.

### `// QT-05a scjson:` marker

Mirror of QT-04c's bound: marker. Emitted by future chapters
(QT-05b/c/e) at the top of any generated module that has an
attached state machine. **Reserved here**; not yet emitted.

## §4 — Source-of-Truth Map

| Concept                                       | Owner                                                       |
| --------------------------------------------- | ----------------------------------------------------------- |
| scjson on-disk element/attribute schema       | `vendor/scjson/` submodule (per QT-05 §0).                  |
| `qt_scjson::*` Rust types                     | QT-05 (`src/bin/creator/qt_scjson.rs`).                     |
| IR types (`UiStateMachine`, …)                | QT-05 §3.                                                   |
| Side-file discovery rule                      | this chapter (§5).                                          |
| Walk algorithm (Scxml → UiStateMachine)       | this chapter (§6).                                          |
| Error contract (missing / malformed)          | this chapter (§7).                                          |
| `<sm>` ID derivation                          | this chapter (§8).                                          |
| QT-05b/c/d/e consumption of the populated IR  | their respective chapters.                                  |

## §5 — Frozen Decision: Side-File Discovery

Registration policy: **Specification Required**.

| QML invocation                              | Discovery behaviour                                                                                |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `qt ingest path/to/foo.qml <out>`           | Probe `path/to/foo.scjson`. If present, attach. If absent, `state_machine = None` (no error).      |
| `qt ingest path/to/dir/ <out_dir>`          | For every `*.qml` discovered by QT-08's directory walker, apply the file-mode rule independently.  |
| `qt check path/to/foo.qml`                  | Same probe rule. A malformed scjson fails `qt check`; a missing scjson does not.                   |
| Symlinked `.scjson`                         | Resolved by `fs::metadata` — symlinks are followed, broken symlinks count as "absent".            |
| `.scjson` with mixed case (`Foo.SCJSON`)    | Not matched. Discovery is case-sensitive on the filesystem regardless of host OS conventions.      |
| Multi-screen QML referencing other QML      | Each `.qml` independently probes for its own sibling `.scjson`. No cross-file inheritance.         |

Deferred (Specification-Required amendments later):

- **`qmldir` / `.qrc` resource indirection** — when a `.qml` lives
  inside a Qt resource bundle and the side-file is referenced via
  `qrc:/`, that's QT-08c territory.
- **Explicit linker flag** (`qt link-sm --scjson <path> --as <id>`
  for non-side-file linkage) — deferred to a future QT-05a sub-amendment.
- **Multiple `.scjson` per `.qml`** — out of scope; one `.qml` ↔
  zero-or-one `.scjson`.

## §6 — Frozen Decision: Walk Algorithm

Per the [`qt_scjson::Scxml`] subset and the QT-05 §5 element list:

1. **Initial state**. `UiStateMachine.initial` ← first non-empty
   element of `Scxml.initial[]`, or `None`.
2. **States**. For each `Scxml.state[]`:
   - Push `UiState { id, on_entry, on_exit }` keyed by the
     `state.id` (defaulting to `"_anon_<index>"` when absent).
   - For nested states (`state.state[]`), recurse and append to
     the same flat list. Nested IDs are emitted verbatim — the
     scjson author is responsible for ID uniqueness.
3. **Transitions**. For each `state.transition[]` (across all
   states reachable in step 2):
   - Push `UiTransition { source, event, target, cond, actions }`.
   - `target` collapses scjson's `Vec<String>` into the first
     entry; multi-target transitions are deferred per QT-05 §5
     and are recorded in the `actions` field's surrounding
     comment if present (no semantic loss because the transition
     is still pushed).
4. **DataModel**. For each `Scxml.datamodel[].data[]`:
   - Push `UiDmField { id, initial }` where `initial` parses
     `data.expr` as `f64::from_str` if numeric, else `None`.
5. **Scripts**. Walk every `<script>` discovered in:
   - `Scxml.script[]` (top-level; `UiScriptOrigin::OnEntry { state: "_root" }`).
   - `state.onentry[].script[]` (`OnEntry { state }`).
   - `state.onexit[].script[]` (`OnExit { state }`).
   - `transition.script[]` (`Transition { index, from, to }`).
   - For unnamed `<script>` elements, synthesize a deterministic
     name per istate's `context.py::_extract_actions` convention:
     `script_<origin>_<state-or-source>_<dst-or-empty>_<index>`.
6. **Actions** (per `UiAction` enum):
   - `<assign location="…" expr="…"/>` → `UiAction::Assign`.
   - `<raise event="…"/>` → `UiAction::Raise`.
   - `<script name="…"/>` → `UiAction::Script` (the discovered
     `name` from step 5).

Steps 1–6 are byte-stable across regenerations: the walker MUST
emit IR fields in the order listed above. This is the QT-05a
golden-file anchor.

## §7 — Frozen Decision: Error Contract

| Condition                                    | Behaviour                                                                              |
| -------------------------------------------- | -------------------------------------------------------------------------------------- |
| `<base>.scjson` does not exist               | Silent fall-through; `state_machine = None`.                                           |
| `<base>.scjson` exists but is empty          | Hard error: "scjson side-file is empty". `qt ingest` exits non-zero.                   |
| `<base>.scjson` exists but is invalid JSON   | Hard error: forwards the underlying `serde_json` error with the file path attached.    |
| `<base>.scjson` is valid JSON but not scjson | Hard error: a missing `state` array AND a missing `datamodel` array AND a missing `initial` array all evaluate as "not a scjson document"; the user gets a descriptive error pointing at QT-05 §5. |
| `<base>.scjson` has deferred elements only   | Walk produces an empty `UiStateMachine` with `id`/`source` set. No error.              |
| Walk discovers an unnamed `<script>` and the synthesized name collides with a previously discovered script | Hard error: "duplicate script name". Authors fix by adding `name="…"` to one of them. |

The "deferred elements only" row is intentional: a user mid-port
may have a scjson rich in parallel/history/invoke that QT-05's
subset cannot lower yet. QT-05a still produces a populated (if
sparse) `UiStateMachine` — the rest of the pipeline can react to
that emptiness in its own way (QT-05b can refuse to emit dispatch
glue when there are no events; QT-05c can refuse bindings when
the datamodel is empty).

## §8 — Frozen Decision: `<sm>` ID Derivation

Registration policy: **Specification Required**.

The istate-codegen Rust crate name is `<sm>_gen` per QT-05 §6.
Where does `<sm>` come from?

1. **Default**: the `.qml` basename, snake_cased. `Stopwatch.qml`
   ↔ `Stopwatch.scjson` produces `id = "stopwatch"`,
   `<stopwatch>_gen`.
2. **Override**: if `Scxml.name` is set (the SCXML `name` attribute,
   surfaced via `qt_scjson::Scxml.name`), it wins after snake_casing.
3. **Collision**: if two `.qml` files in dir-mode produce the same
   `<sm>` (e.g. `Stopwatch.qml` and `stopwatch.qml`), `qt ingest`
   errors out before writing any artifact.

`<sm>` is NOT user-configurable via flag at QT-05a. A future
amendment MAY add `--sm-id <id>` for cases where the default
collides with an existing crate name.

## §9 — Non-Goals

- **No QML → scjson emit.** That's QT-05d.
- **No `<sm>_gen/` crate generation.** That stays out-of-process
  via the istate-codegen MCP.
- **No semantic validation of scjson.** The walker is structural
  only — guards/cond expressions, transition determinism, parallel
  region wellformedness are istate's concern.
- **No `qmldir` / `.qrc` resolution.** QT-08c.
- **No diff against istate-codegen output.** QT-05b/c/e will
  exercise the full pipeline once `<sm>_gen/` crates ship; QT-05a
  stops at populated IR.
- **No live MCP invocation.** The `--with-codegen` flag for
  automated istate-codegen invocation remains deferred to a future
  sub-amendment; QT-05a is purely local I/O.
- **No changes to existing fixtures.** All 9 pre-QT-05 fixtures
  continue to ingest with `state_machine = None` — silent
  fall-through is verified by their existing drift gates being
  unchanged.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                              | Resolution                                                                                                                                                                          |
| -------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| QT-05    | IR types + linkage surface.                          | QT-05a populates `UiModule.state_machine: Option<UiStateMachine>`; no new IR types and no linkage-version bump.                                                                     |
| QT-05b   | Handler dispatch glue.                               | QT-05b reads `state_machine` to decide whether to thread `Rc<RefCell<Machine>>` into `build_screen` and to emit `dispatch(Event::…)` calls. QT-05a is the unblocker.                |
| QT-05c   | DM/State bindings.                                   | Same. QT-05c reads `state_machine.datamodel` + `state_machine.states` to decide which bindings exist.                                                                              |
| QT-05d   | QML → scjson emit.                                   | QT-05d will write a `.scjson` next to a `.qml`; QT-05a's discovery rule then re-ingests on the next pass. The two phases form a closed loop without sharing code.                 |
| QT-05e   | Externals stub emission.                             | QT-05e reads `state_machine.scripts` to decide which `Externals` methods to stub.                                                                                                  |
| QT-08    | Directory-mode CLI.                                  | QT-08's directory walker is unchanged: each `.qml` it discovers triggers the QT-05a side-file probe independently.                                                                 |
| QT-04 family | `build_screen` tuple, refresh pump.              | Untouched here; emit-side amendments to those tuples land with QT-05b/c.                                                                                                            |

## §11 — Acceptance Checklist

QT-05a is **ratified and shipped** when:

- [x] §5 freezes the side-file discovery rule.
- [x] §6 freezes the walk algorithm.
- [x] §7 freezes the error contract.
- [x] §8 freezes the `<sm>` ID derivation.
- [x] `qt::ingest` (file mode + dir mode) probes
      `<basename>.scjson` and populates `UiModule.state_machine`.
- [x] `tests/fixtures/qt/stopwatch.qml` + `stopwatch.scjson` exist
      as the canonical QT-05a fixture.
- [x] `tests/fixtures/qt/stopwatch.qt-ir.json` golden carries a
      populated `state_machine` field; drift gate added.
- [x] `tests/fixtures/qt/stopwatch.rs` and
      `tests/fixtures/qt/stopwatch.rlvgl.rs` goldens are emitted
      (with the existing emit-shape — no QT-05b/c glue yet);
      drift gates added.
- [x] All 9 pre-QT-05 fixtures' goldens remain byte-equal —
      silent fall-through is enforced by drift.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended with QT-05a status and
      change-log entries.

QT-05a does NOT need a new compile-as-mod gate: the emitted Rust
is unchanged from QT-04e's shape (no SM glue yet). QT-05b will add
the first compile-as-mod gate that exercises the populated IR.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — IR types, scjson element subset, file layout.
- [`docs/qt-support/08-multi-file-cli.md`](./08-multi-file-cli.md) — directory-mode walker.
- [`vendor/scjson/`](../../vendor/scjson) — upstream scjson submodule (BSD-1-Clause).
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — ingest entry point + walk implementation.
- [`src/bin/creator/qt_scjson.rs`](../../src/bin/creator/qt_scjson.rs) — hand-rolled scjson subset.
- [`tests/fixtures/qt/stopwatch.qml`](../../tests/fixtures/qt/stopwatch.qml) — QT-05a canonical fixture.
- [`tests/fixtures/qt/stopwatch.scjson`](../../tests/fixtures/qt/stopwatch.scjson) — paired scjson side-file.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gates.

## §13 — Unblocks

Ratifying QT-05a unblocks:

- **QT-05b** (handler dispatch glue): the IR field is populated;
  the emitter can branch on `Some(sm)` vs `None` to decide
  whether to thread `Machine` through `build_screen`.
- **QT-05c** (DM/State bindings): the datamodel and state lists
  are now part of the IR — QT-05c can compute which Label /
  visibility bindings to lower.
- **QT-05e** (Externals stub emission): `UiStateMachine.scripts`
  enumerates the callouts QT-05e needs to stub.
- **QT-05d** (QML → scjson emit): the round-trip pivot —
  authoring as inline QML `States {}` in QT-05d round-trips
  through a QT-05d-emitted `.scjson` that QT-05a re-ingests.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | QT-05a ratified and shipped. `qt::ingest` (file mode + dir mode) now probes `<basename>.scjson` next to each `.qml` and populates `UiModule.state_machine` via the `qt_scjson::Scxml` → `UiStateMachine` walker. Discovery rule, walk algorithm, error contract, and `<sm>` ID derivation frozen in §5–§8. Missing scjson is silent fall-through (`state_machine = None`); malformed scjson is a hard error with the underlying serde_json message. Compound state nesting is flattened to a single-level `UiState` list (deferred-element entries dropped per QT-05 §5). New fixture: `tests/fixtures/qt/stopwatch.qml` + `tests/fixtures/qt/stopwatch.scjson` + 3 drift gates. All 9 pre-QT-05 fixture goldens unchanged — silent fall-through enforced by drift. No `QT_IR_VERSION` bump (the field is additive and was already added by QT-05). No `QT_EMIT_VERSION_RLVGL` bump (emit shape unchanged — QT-05b ships the first emit change). |

---

MIT-licensed: MIT.
