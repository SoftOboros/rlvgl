<!--
10-release.md - QT-10: strict-mode acceptance + release tag.
-->

**[← Prev](09-desktop-ui.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-10 — Strict-Mode Acceptance + Release Tag

QT-10 is the **closeout** of the Qt-support initiative. Every
prior chapter's §11 Acceptance Checklist is promoted from a
ratified-and-shipped milestone (per-chapter MUST) to a
strict-mode invariant: the rlvgl-creator binary cannot ship a
feature-marked build that fails any of those checks.

QT-10 ships in two halves:

1. **Strict-mode acceptance** (this chapter, auto-mode-eligible):
   a meta-test (`tests/creator_qt_strict_mode.rs`) walks every
   chapter file and every named CLI subcommand and asserts the
   surface is intact. A new `QT_FAMILY_STRICT_VERSION` constant
   pins the strict-mode generation. README + 00-concepts.md
   strict-mode amendments record the closeout.

2. **Release tag** (out of scope for auto mode): cutting a
   `v0.x.0` git tag, running `cargo publish` for any newly
   published-or-bumped crates, and announcing. This step
   requires explicit user authorization per the safety
   protocol; QT-10 names the workflow but does not perform it.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174.
Vocabulary defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary).
Every prior chapter's §11 acceptance checklist is the
authoritative source-of-truth for what counts as a passing
strict-mode build; QT-10 owns only the meta-test that
enforces them and the version constant that tracks the
strict-mode generation.

## §1 — Purpose

Until QT-10, each phase's "ratified and shipped" status is
recorded informally in 00-concepts.md §15 and the
per-chapter §15. There is no automated guard against
regressions that silently invalidate a previously-passing
acceptance checklist (e.g. someone deletes a fixture, renames
a CLI subcommand, drops a chapter file).

After QT-10:

- A meta-test (`tests/creator_qt_strict_mode.rs`) walks every
  chapter file and asserts:
  - The file exists.
  - It carries `## §15 — Change Log` (closeout marker).
  - It carries a 2026-04-* dated change-log entry.
- A separate test asserts every named CLI subcommand
  (`ingest` / `check` / `schema` / `emit` / `emit-scjson` /
  `emit-externals` / `emit-tokens` / `list-assets` /
  `list-qmldir` / `list-qrc`) is reachable via `qt --help`.
- A `QT_FAMILY_STRICT_VERSION = 1` constant lives in the
  emitter source and bumps when the strict-mode invariant set
  changes (e.g. a future amendment adds a new chapter).

## §2 — Problem Statement

Without QT-10, three regression classes go undetected:

- **Chapter deletion**: deleting `04e-reactive-bindings.md`
  breaks the documented reactive-bindings invariant but
  leaves all code tests green.
- **Subcommand removal**: deleting `qt list-assets` from
  `cli.rs::QtCommand` would compile but break QT-07's
  acceptance checklist.
- **Version-constant drift**: changing `QT_IR_VERSION = 2`
  to `3` without updating the chapter that owns the bump
  policy creates a silent fork.

QT-10 catches all three.

## §3 — Canonical Glossary (delta only)

QT-10 introduces no new IR types, no new CLI subcommands.
Three new artifacts.

### `QT_FAMILY_STRICT_VERSION`

```rust
pub const QT_FAMILY_STRICT_VERSION: u32 = 1;
```

Lives in `src/bin/creator/qt.rs`. Bumps when:

- A chapter file is renamed, deleted, or split.
- The CLI subcommand set in §5 changes.
- A version constant in §6 changes.

The strict-mode test asserts this constant matches the
expected generation; the same value is referenced by README
and 00-concepts.md as the canonical strict-mode generation.

### `tests/creator_qt_strict_mode.rs`

A meta-test asserting the strict-mode invariants from §5–§7
on every `cargo test --features creator -p rlvgl --tests`.
Lives next to the existing `creator_qt_*` tests.

### "strict-mode generation"

Shorthand for the value of `QT_FAMILY_STRICT_VERSION`. A
"strict-mode-1" build is one whose `QT_FAMILY_STRICT_VERSION
== 1` and whose strict-mode test passes.

## §4 — Source-of-Truth Map

| Concept                                | Owner                                                                  |
| -------------------------------------- | ---------------------------------------------------------------------- |
| Per-chapter §11 acceptance lists       | Each chapter (this chapter only enforces what they freeze).            |
| Chapter file set                       | this chapter (§5).                                                      |
| CLI subcommand set                     | this chapter (§5).                                                      |
| Version constant set                   | this chapter (§6).                                                      |
| Strict-mode meta-test                  | this chapter (§7).                                                      |
| Release-tag workflow                   | this chapter (§8).                                                      |
| Release-tag execution                  | **user-driven** — out of scope for auto-mode.                          |

## §5 — Frozen Decision: Strict-Mode Invariants

Registration policy: **Standards Action**. Adding or removing
any item bumps `QT_FAMILY_STRICT_VERSION`.

### Chapter file set (23 files)

```
docs/qt-support/00-concepts.md
docs/qt-support/02-ir-schema.md
docs/qt-support/03-rlvgl-emitter-widgets.md
docs/qt-support/03b-rlvgl-widget-mapping.md
docs/qt-support/03c-anchor-resolver.md
docs/qt-support/04-signal-handlers.md
docs/qt-support/04b-properties-bindings.md
docs/qt-support/04c-initial-value-bindings.md
docs/qt-support/04d-mousearea.md
docs/qt-support/04e-reactive-bindings.md
docs/qt-support/04f-nested-id-resolution.md
docs/qt-support/05-state-machines.md
docs/qt-support/05a-scjson-ingest.md
docs/qt-support/05b-handler-dispatch.md
docs/qt-support/05c-machine-bindings.md
docs/qt-support/05d-emit-scjson.md
docs/qt-support/05e-externals-stubs.md
docs/qt-support/06-theme-tokens.md
docs/qt-support/07-asset-handoff.md
docs/qt-support/08-multi-file-cli.md
docs/qt-support/08b-qmldir-resolution.md
docs/qt-support/08c-qrc-resources.md
docs/qt-support/09-desktop-ui.md
docs/qt-support/10-release.md
```

24 entries (this chapter included). All are normative.

### CLI subcommand set (10 entries)

```
qt ingest
qt check
qt schema
qt emit
qt emit-scjson
qt emit-externals
qt emit-tokens
qt list-assets
qt list-qmldir
qt list-qrc
```

The strict-mode test runs `rlvgl-creator qt --help` (or
otherwise inspects the CLI) and asserts all 10 are
reachable.

## §6 — Frozen Decision: Version Constant Snapshot

| Constant                       | Strict-mode-1 value | Owner                                                |
| ------------------------------ | ------------------- | ---------------------------------------------------- |
| `QT_IR_VERSION`                | 2                   | QT-05 (see chapter §8)                               |
| `QT_EMIT_VERSION_RLVGL`        | 13                  | QT-05c (see chapter §7) — was 11 at QT-05 close-out, +1 each at QT-05b/05c |
| `QT_EMIT_VERSION_DATA`         | 1                   | QT-03                                                |
| `ISTATE_LINKAGE_VERSION`       | 1                   | QT-05 §6                                             |
| `QT_EXTERNALS_VERSION`         | 1                   | QT-05e §8                                            |
| `QT_FAMILY_STRICT_VERSION`     | 1                   | this chapter (§3)                                    |

Strict-mode-1 means **all six** above hold simultaneously.
A bump in any one (with the surrounding chapter amendment)
bumps `QT_FAMILY_STRICT_VERSION` to 2 and the strict-mode
test's expected snapshot updates with it.

## §7 — Frozen Decision: Strict-Mode Meta-Test

`tests/creator_qt_strict_mode.rs` enforces three sets of
invariants:

1. **Chapter files** (§5):
   - Every entry in the chapter set exists at the named
     path.
   - Every existing file in `docs/qt-support/*.md` (excluding
     `README.md`) is in the chapter set — guards against
     drive-by additions that bypass the chapter discipline.
2. **Version constants** (§6):
   - Each constant has the strict-mode-1 value.
3. **CLI subcommand surface** (§5):
   - `rlvgl-creator qt --help` succeeds.
   - Each of the 10 subcommand labels appears in the help
     output.

Test runs under the standard `cargo test --features creator
-p rlvgl --tests` invocation; no separate gate.

## §8 — Frozen Decision: Release-Tag Workflow

QT-10's release-tag half is **not auto-mode-eligible**. The
canonical workflow when the user is ready:

1. Run `cargo fmt --check`, `cargo clippy --workspace`,
   `cargo test --workspace`.
2. Run the QT family's CLI smoke (each `qt …` subcommand
   on its canonical fixture).
3. Bump `Cargo.toml` workspace version to `v0.<minor>.<patch>`.
4. Update `CHANGELOG.md` (if maintained) with the QT-* family
   entries from `docs/qt-support/00-concepts.md` §15.
5. `git tag v0.<minor>.<patch>` + `git push --tags`.
6. `cargo publish` for any chipdb / creator crates whose
   versions changed (in dependency order: `rlvgl-chips-*` →
   `rlvgl-core` → `rlvgl-widgets` → `rlvgl` workspace).
7. GitHub Release notes pointing at the QT-10 strict-mode
   generation and listing the QT family entries.

QT-10 names this workflow but does not perform it. The user
runs it when ready.

## §9 — Non-Goals

- **No automatic release tagging.** Cutting tags / publishing
  crates is destructive and requires explicit user
  authorization per the safety protocol.
- **No CI integration.** The strict-mode test runs as part of
  `cargo test`; no separate CI workflow is added at v1.
- **No semver-major guard.** A breaking change to the IR or
  emit shape bumps the strict-mode generation but doesn't
  block a release; that's a release-process concern, not a
  test concern.
- **No upstream-data validation.** The chapter file set is
  validated by name; content drift inside individual chapters
  is owned by their own §11 lists.
- **No "all examples build" gate.** The existing examples (e.g.
  `examples/stm32h747i-disco/`) have their own build profiles
  documented in `CLAUDE.md`; QT-10 doesn't enforce them.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                       | Resolution                                                                                          |
| -------- | ----------------------------- | --------------------------------------------------------------------------------------------------- |
| QT-00    | Phase enumeration policy.     | QT-10 added under Standards Action. The phase set frozen in QT-00 §5 is the authoritative input to QT-10 §5. |
| QT-05*   | State-machine pipeline.       | QT-10 strict-mode-1 includes all six QT-05* chapters and pins `ISTATE_LINKAGE_VERSION = 1`.        |
| QT-06    | Theme tokens.                 | Pinned via §5 chapter set.                                                                          |
| QT-07    | Asset handoff.                | Pinned via §5 chapter set.                                                                          |
| QT-08*   | CLI surface family.           | All three chapters pinned. The 10 subcommands enumerated in §5 are the strict-mode-1 surface.        |
| QT-09    | Desktop-UI integration.       | Pinned. The Qt menu group's 10 entries match the §5 subcommand set 1:1.                            |

## §11 — Acceptance Checklist

QT-10 strict-mode-1 is **ratified and shipped** when:

- [x] `QT_FAMILY_STRICT_VERSION = 1` lives at
      `src/bin/creator/qt.rs`.
- [x] §5 freezes the chapter file set (24 entries) and the
      CLI subcommand set (10 entries).
- [x] §6 freezes the version-constant snapshot (six
      constants).
- [x] §7 fixes the strict-mode meta-test contract.
- [x] §8 documents the release-tag workflow (auto-mode does
      not perform it).
- [x] `tests/creator_qt_strict_mode.rs` asserts all three
      invariant sets.
- [x] `cargo test --features creator -p rlvgl --tests` is
      green with the new test in place.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended to mark the QT
      family strict-mode-1 ratified.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — phase enumeration policy.
- All other `docs/qt-support/*.md` chapters — implicitly cited via §5.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `QT_FAMILY_STRICT_VERSION` constant site.
- [`tests/creator_qt_strict_mode.rs`](../../tests/creator_qt_strict_mode.rs) — strict-mode meta-test.

## §13 — Unblocks

Ratifying QT-10 strict-mode-1 unblocks:

- The user-driven release-tag execution per §8.
- A future "Qt 2.x" major-version bump that explicitly
  invalidates strict-mode-1 (would bump to strict-mode-2).
- A CI workflow that runs the strict-mode test on every PR.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-10 strict-mode-1 ratified and shipped (auto-mode half). Closes the Qt-support roadmap's enforcement story. New `pub const QT_FAMILY_STRICT_VERSION: u32 = 1;` in `src/bin/creator/qt.rs` pins the strict-mode generation. New meta-test `tests/creator_qt_strict_mode.rs` asserts (1) every chapter file in the §5 set exists, (2) every chapter file under `docs/qt-support/*.md` is in the §5 set (no drive-by chapters), (3) every version constant from the §6 snapshot has its strict-mode-1 value, and (4) every CLI subcommand from the §5 set is reachable via `rlvgl-creator qt --help`. Six version constants pinned: `QT_IR_VERSION = 2`, `QT_EMIT_VERSION_RLVGL = 13`, `QT_EMIT_VERSION_DATA = 1`, `ISTATE_LINKAGE_VERSION = 1`, `QT_EXTERNALS_VERSION = 1`, `QT_FAMILY_STRICT_VERSION = 1`. README.md and 00-concepts.md amended with strict-mode-1 ratification entries. The release-tag half (cutting `v0.x.0`, `cargo publish`, GitHub Release) is documented in §8 but **not performed** under auto-mode per the safety protocol; the user runs it when ready. Chapter file set: 24 entries (incl. this chapter). CLI subcommand set: 10 entries. |

---

MIT-licensed: MIT.
