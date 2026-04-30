<!--
08-multi-file-cli.md - QT-08: directory-mode ingest and emit.
-->

**[← Prev](04c-initial-value-bindings.md) · [Index](README.md) · [Next →](#)** *(QT-09 not yet authored)*

# Chapter QT-08 — Multi-File CLI

QT-01a / QT-03 / QT-03b / QT-04 / QT-04b / QT-04c / QT-03c built the
ingest + emit pipeline against a single `.qml` file at a time. Real
Qt projects are directories of `.qml` files (typically one screen
per file). QT-08 lets `rlvgl-creator qt {ingest,emit}` accept a
directory and process every `*.qml` file inside it in one
invocation.

This is the first slice of the broader **QT-08 CLI surface**
roadmap entry. Subsequent slices (`QT-08b`, `QT-08c`, …) handle
`.qmldir` resolution, `.qrc` resource manifests, and the asset
crate handoff. This chapter scopes only the directory walker.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary).
Per-file output shapes are owned by their respective phases
(QT-01a / QT-03 / QT-03b); QT-08 owns only the directory walk and
the per-file output naming convention in directory mode.

## §1 — Purpose

Replace the current "one fixture per `cargo run`" workflow with a
single command that processes a project tree:

```bash
# Before QT-08:
rlvgl-creator qt ingest screens/login.qml out/login/
rlvgl-creator qt ingest screens/dashboard.qml out/dashboard/
rlvgl-creator qt ingest screens/settings.qml out/settings/

# After QT-08:
rlvgl-creator qt ingest screens/ out/
# → out/login.qt-ir.json
# → out/dashboard.qt-ir.json
# → out/settings.qt-ir.json
```

Same shape for `qt emit --target {data,rlvgl}`.

## §2 — Problem Statement

Three failure modes today block real-project use:

- A user with a `screens/` directory has to invoke the binary
  once per file. For a project with 20 screens, that's 20
  invocations and 20 output dirs.
- The single-file mode's output naming (`qt-ir.json` rather than
  `<basename>.qt-ir.json`) does not aggregate cleanly: pointing
  three invocations at the same out dir overwrites the same file.
- There is no canonical iteration order. Build scripts have to
  shell out to `find` / `ls` and stitch the outputs together
  themselves.

QT-08 closes all three with a deterministic walker that:

1. Accepts either a file or a directory as `<input>`.
2. In directory mode, walks `*.qml` files in lexical order
   (sorted by `OsStr`) so the output is stable across filesystems.
3. Names every per-file output `<basename>.<suffix>` so multiple
   files can share an output directory.

## §3 — Canonical Glossary (delta only)

QT-08 introduces no new IR types, no new emitted-Rust types, and
no new emit-shape version. Two terms:

### Directory mode

The dispatch path triggered when `<input>` is a directory. Walks
the directory's immediate children (non-recursive at QT-08; nested
directory walking is deferred to QT-08b under `.qmldir` semantics).
Owned here.

### File mode

The pre-QT-08 single-file path. Untouched: invoking
`qt ingest <file.qml> <out>/` continues to write `<out>/qt-ir.json`
exactly as before. The naming asymmetry between file mode and
directory mode is **intentional** for back-compat; see §6.

## §4 — Source-of-Truth Map

| Concept                         | Owner                                                                  |
| ------------------------------- | ---------------------------------------------------------------------- |
| Per-file IR shape               | QT-01a                                                                  |
| Per-file data emit shape        | QT-03                                                                   |
| Per-file rlvgl emit shape       | QT-03b / QT-04 / QT-04b / QT-04c / QT-03c                               |
| File-mode output naming         | QT-01a / QT-03 (`qt-ir.json`, `<basename>.rs`, `<basename>.rlvgl.rs`)   |
| Directory-mode output naming    | this chapter (§5)                                                       |
| Directory walk order            | this chapter (§7)                                                       |
| `.qmldir` resolution            | **QT-08b** (deferred)                                                   |
| `.qrc` resource manifests       | **QT-08c** (deferred)                                                   |

## §5 — Frozen Decision: Directory-Mode Output Naming

For each `*.qml` file in the input directory, the per-output file
name is the QML basename plus a per-target suffix:

| Subcommand                   | File-mode output (unchanged) | Directory-mode output (new)                   |
| ---------------------------- | ---------------------------- | --------------------------------------------- |
| `qt ingest <input> <out>/`   | `<out>/qt-ir.json`           | `<out>/<basename>.qt-ir.json` per `*.qml`     |
| `qt emit --target data`      | `<out>/<basename>.rs`        | `<out>/<basename>.rs` per `*.qml`             |
| `qt emit --target rlvgl`     | `<out>/<basename>.rlvgl.rs`  | `<out>/<basename>.rlvgl.rs` per `*.qml`       |

`<basename>` is `Path::file_stem()` (the filename without the
trailing `.qml` extension). Implementations **MUST** error if two
input QML files in the same directory share a basename (impossible
on POSIX / Windows filesystems, but defensive).

`qt schema` and `qt check` are file-only at QT-08; directory-mode
support for them is deferred (no canonical aggregation use case
identified).

## §6 — Frozen Decision: File-Mode Asymmetry

`qt ingest <file.qml> <out>/` continues to write `<out>/qt-ir.json`
(no basename). This is **intentional** back-compat for fixtures and
scripts that pin the existing single-file flow. The asymmetry
between file mode (`qt-ir.json`) and directory mode
(`<basename>.qt-ir.json`) is **frozen** here; harmonising the two
would break the QT-02 golden roundtrip and is out of scope.

`qt emit` already used basename-prefixed output names in file mode,
so the directory-mode naming for emit is the natural extension and
no asymmetry exists for `--target {data,rlvgl}`.

## §7 — Frozen Decision: Walk Order and Filtering

Directory walking **MUST**:

1. Read the directory's immediate children only — no recursion.
   Subdirectory walking is reserved for QT-08b's `.qmldir` /
   sub-component semantics.
2. Filter to entries whose path matches `*.qml` (case-sensitive
   `.qml` extension). Files like `*.qmldir` / `*.QML` /
   `*.qml.bak` are skipped.
3. Sort the resulting paths by `OsStr` byte order
   (`Vec::sort_by_key(|p| p.file_name().to_owned())`). This makes
   output ordering reproducible across hosts.
4. Skip hidden files (filenames starting with `.`).
5. Process each file independently — a parse error on one file
   **MUST** report the file path and exit non-zero, *not* swallow
   the error and continue. Partial dir runs are surprising in
   build scripts; loud failure is preferred.

## §8 — Versioning

QT-08 does **not** bump `QT_EMIT_VERSION_RLVGL`. The per-file
emit shape is unchanged; only the CLI surface grows. Consumers
pinned to `QT_EMIT_VERSION_RLVGL = 6` see no diff inside any
generated `<basename>.rlvgl.rs`.

`QT_EMIT_VERSION_DATA` is unchanged for the same reason.

## §9 — Non-Goals

- **No recursive directory walking.** Nested QML projects are
  QT-08b territory (depends on `.qmldir` resolution).
- **No `.qmldir` ingest.** QT-08b will read `.qmldir` files to
  resolve user-defined types in the QML imports section.
- **No `.qrc` resource bundling.** QT-08c will pull `.qrc`
  manifests through the existing creator asset pipeline.
- **No multi-input concatenation.** Each input file produces its
  own independent output; there is no "merge into one big tree"
  mode.
- **No globbing.** The user passes a path, not a glob. Bash /
  PowerShell glob expansion is the user's responsibility.
- **No tracking-by-mtime / incremental rebuilds.** Every `qt`
  invocation reprocesses every file. Caching is left to
  cargo / make / similar build orchestrators.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                          | Resolution                                                                                            |
| -------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| QT-01a   | Single-file parser.                                              | Unchanged. Directory mode wraps it.                                                                    |
| QT-02    | IR schema artifact.                                              | Unchanged.                                                                                             |
| QT-03    | Data emit.                                                        | Unchanged per-file. Directory mode produces N `<basename>.rs` files.                                   |
| QT-03b   | rlvgl emit `build_screen` per file.                              | Unchanged. Each generated file has its own `build_screen` / `ScreenState`.                            |
| QT-04 / QT-04b / QT-04c | Handler / property / binding lowering.                | Per-file behaviour preserved.                                                                          |
| QT-08b   | `.qmldir` resolution.                                            | Will extend the directory walker to recognise the manifest file and load type aliases.                |
| QT-08c   | `.qrc` resource manifests.                                       | Will hand off discovered images / fonts to the creator asset pipeline.                                 |
| QT-09    | Desktop UI integration.                                          | Will wrap the directory walker behind an "Open project…" command.                                      |

## §11 — Acceptance Checklist

QT-08 is **ratified and shipped** when:

- [x] §5 freezes the directory-mode output naming.
- [x] §6 documents the file-mode / directory-mode asymmetry as
      intentional.
- [x] §7 freezes walk order, filtering, and error handling.
- [x] §8 confirms no version bump.
- [x] `qt::ingest` and `qt::emit` detect a directory `<input>`
      and walk it per §7.
- [x] New canonical fixture
      [`tests/fixtures/qt/multi/`](../../tests/fixtures/qt/multi/)
      contains at least two `*.qml` files exercising the dir-mode walk.
- [x] Drift gates verify dir-mode produces two outputs per
      subcommand (ingest, data emit, rlvgl emit).
- [x] CLI `--help` text describes both modes.
- [x] §15 carries a dated initial change-log entry.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `ingest` / `emit` dispatch.
- [`src/bin/creator/cli.rs`](../../src/bin/creator/cli.rs) — `--help` text.
- [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) — practical setup guide.
- [`docs/creator/CLI.md`](../creator/CLI.md) — full CLI reference.
- [`tests/fixtures/qt/multi/`](../../tests/fixtures/qt/multi/) — multi-file fixture (forthcoming).

## §13 — Unblocks

Ratifying QT-08 unblocks:

- Real-project bring-up. Users can point `qt ingest` at a
  `screens/` directory and get N IR files in one invocation.
- `QT-08b` — `.qmldir` resolution. Now has a directory walker to
  extend.
- `QT-08c` — `.qrc` resource bundling.
- `QT-09` — desktop-UI "Open project…" flow.
- CI orchestration for Qt-authored screens. Build scripts can run
  one `qt emit` per project and feed the output dir to cargo.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | Ratified and shipped. Directory-mode CLI for `qt ingest` and `qt emit` (§5), file-mode asymmetry frozen (§6), walk order / filtering / error handling frozen (§7), no version bump (§8). New `tests/fixtures/qt/multi/{a,b}.qml` fixture + 3 drift gates verifying two outputs per subcommand. `qt schema` / `qt check` remain file-only at QT-08; their dir-mode support is unscheduled. `.qmldir` and `.qrc` deferred to QT-08b / QT-08c. |

---

MIT-licensed: MIT.
