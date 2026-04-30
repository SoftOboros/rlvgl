<!--
09-desktop-ui.md - QT-09: desktop UI integration.
-->

**[← Prev](08c-qrc-resources.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-09 — Desktop UI Integration

QT-09 brings the 10 `qt …` CLI subcommands into the
`rlvgl-creator` desktop UI as a new top-level **Qt** menu group.
Each entry uses a file/folder picker dialog (matching the
existing scan/check/vendor pattern), invokes the underlying
`qt::*` function, and reports success/failure via the existing
toast feedback channel.

QT-09 introduces no new IR types, no new emit-shape constants,
no new fixtures, and no new behavioural tests beyond compile-clean.
UI behaviour is verified visually by the user (the codebase has no
existing UI test harness to extend).

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
existing UI dispatch convention (label-keyed `handle_action`,
`pick_file()` / `pick_folder()`, toast feedback) is owned by
[`docs/creator/UI-DESIGN.md`](../creator/UI-DESIGN.md). The
"Qt" menu group label set, per-subcommand dialog flow, and
file-picker filters are owned here.

## §1 — Purpose

After QT-09, the desktop UI's command surface mirrors the CLI:
every `qt …` subcommand is reachable from the Qt menu group with
the same input/output semantics. CLI users never need to think
about the UI; UI users never need to drop to a terminal for
Qt-specific work.

## §2 — Problem Statement

Today, the desktop UI menu groups (Assets / Build / Deploy /
Emulator) cover the asset pipeline, BSP generation, and
emulator launch but have **zero entries** for the 10 `qt …`
subcommands shipped under QT-01a / QT-05* / QT-06 / QT-07 / QT-08*.
Consequences:

- A QT-only user has to launch a separate terminal alongside
  the UI and copy paths between the two.
- The UI's "command parity with CLI" promise (per
  `UI-DESIGN.md`) silently regresses with every new QT
  subcommand we ship.
- Per-subcommand discovery suffers: the user can't see at a
  glance which Qt operations exist.

## §3 — Canonical Glossary (delta only)

QT-09 introduces no new IR types and no new modules. One menu
group, ten new `handle_qt_*` functions, ten new dispatch arms.

### "Qt" menu group

Adds to `MENU_GROUPS` in `src/bin/creator_ui/menus.rs`:

```rust
("Qt", &[
    "Qt Ingest",
    "Qt Check",
    "Qt Schema",
    "Qt Emit",
    "Qt Emit Scjson",
    "Qt Emit Externals",
    "Qt Emit Tokens",
    "Qt List Assets",
    "Qt List Qmldir",
    "Qt List Qrc",
]),
```

Order matches the CLI subcommand declaration order so reviewers
have a stable reference.

### `handle_qt_*` functions

Ten new functions in `commands.rs`, each following the existing
`handle_scan` / `handle_vendor` shape:

```rust
pub(crate) fn handle_qt_ingest(&mut self) {
    if let Some(input) = FileDialog::new().add_filter("QML", &["qml"]).pick_file() {
        if let Some(out) = FileDialog::new().pick_folder() {
            let res = qt::ingest(&input, &out);
            self.show_feedback("Qt Ingest", res);
        }
    }
}
```

File-picker filters follow §5.

## §4 — Source-of-Truth Map

| Concept                                       | Owner                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| Existing UI command-dispatch pattern          | `docs/creator/UI-DESIGN.md` + `src/bin/creator_ui/commands.rs`.         |
| File-picker / folder-picker convention        | `rfd::FileDialog` (existing dep).                                       |
| Toast feedback channel                        | `CreatorApp::show_feedback`.                                            |
| Qt-side CLI implementations                   | `src/bin/creator/qt.rs` + `src/bin/creator/qt_scjson.rs`.               |
| Qt menu group label set                       | this chapter (§3).                                                      |
| Per-subcommand dialog flow                    | this chapter (§5).                                                      |
| File-picker filters                           | this chapter (§5).                                                      |

## §5 — Frozen Decision: Per-Subcommand Dialog Flow

| Menu label              | CLI fn                  | Input picker                                   | Output picker (optional) | Notes                              |
| ----------------------- | ----------------------- | ---------------------------------------------- | ------------------------ | ---------------------------------- |
| `Qt Ingest`             | `qt::ingest`            | file (`*.qml`) or folder (cancellable to dir)  | folder (required)        | File mode preferred; folder mode via folder picker. |
| `Qt Check`              | `qt::check`             | file (`*.qml`)                                 | n/a                      | No output; only validation toast.  |
| `Qt Schema`             | `qt::schema`            | n/a                                            | file (optional `.json`)   | Cancel writes to stdout.           |
| `Qt Emit`               | `qt::emit`              | file (`*.qml`) or folder                       | folder (required)        | Target defaults to `rlvgl`; UI does not expose `--target data` at v1. |
| `Qt Emit Scjson`        | `qt::emit_scjson`       | file (`*.qml`) or folder                       | folder (optional)        | Default out: input parent.         |
| `Qt Emit Externals`     | `qt::emit_externals`    | file (`*.qml`) or folder                       | folder (optional)        | Same.                              |
| `Qt Emit Tokens`        | `qt::emit_tokens`       | file (`*.qml`) or folder                       | folder (optional)        | Same.                              |
| `Qt List Assets`        | `qt::list_assets`       | file (`*.qml`) or folder                       | folder (optional)        | Same.                              |
| `Qt List Qmldir`        | `qt::list_qmldir`       | folder (containing `qmldir`) or file (`qmldir`) | folder (optional)       | Folder picker preferred.           |
| `Qt List Qrc`           | `qt::list_qrc`          | file (`*.qrc`) or folder                       | folder (optional)        | File mode preferred.               |

For "input picker (file or folder)" the v1 UI uses **file picker
first**. If the user cancels file selection, the dispatch
attempts a folder picker. The same rule applies for "output
picker (optional)": cancelling the dialog uses the CLI's default
(input-parent for most subcommands).

The UI does **not** expose:

- `qt emit --target data` (deferred — `rlvgl` covers the
  primary path; advanced users use the CLI).
- Verbose / silent flags (UI runs silent by default).

These can be promoted to a §5 amendment if user pressure
justifies.

## §6 — Frozen Decision: Module Wiring

`creator_ui` already pulls each `creator/<x>.rs` module via
`#[path]` attributes in `mod.rs`. QT-09 adds two:

```rust
#[path = "../creator/qt.rs"]
mod qt;
#[path = "../creator/qt_scjson.rs"]
mod qt_scjson;
```

The `qt::*` functions are `pub(crate)` so the UI module can call
them directly. No public API surface changes.

## §7 — Versioning

| Constant                       | Before QT-09 | After QT-09 |
| ------------------------------ | ------------ | ----------- |
| All existing emit-shape consts | unchanged    | unchanged   |

QT-09 is a UI-side delta only; no emit-shape change.

## §8 — Non-Goals

- **No headless UI tests.** The codebase has no eframe test
  harness; QT-09 verifies via compile-clean only.
- **No advanced flag exposure.** `--target data`, verbose flags,
  and per-subcommand options beyond the input/output paths are
  deferred.
- **No drag-and-drop.** Existing UI uses file pickers
  exclusively; QT-09 follows.
- **No project-wide "run all qt subcommands" macro.** Each
  command is invoked individually. A future amendment may add a
  Qt-equivalent of the existing "Scan Convert Preview" wizard.
- **No qmldir/qrc preview.** The user opens the produced YAML in
  an external editor.

## §9 — Reconciliation with Adjacent Phases

| Phase    | Concern                                | Resolution                                                                                     |
| -------- | -------------------------------------- | ---------------------------------------------------------------------------------------------- |
| QT-01a   | `qt ingest` / `qt check`.              | UI exposes both via Qt Ingest / Qt Check entries.                                              |
| QT-02    | `qt schema`.                           | UI exposes via Qt Schema entry.                                                                |
| QT-03+   | `qt emit`.                             | UI defaults to `rlvgl` target; advanced users keep the CLI.                                    |
| QT-05*   | scjson + dispatch + bindings + emit + externals. | UI exposes the scjson / externals subcommands.                                          |
| QT-06    | Theme tokens.                          | UI exposes Qt Emit Tokens.                                                                     |
| QT-07    | Asset inventory.                       | UI exposes Qt List Assets.                                                                     |
| QT-08*   | Multi-file / qmldir / qrc.             | UI exposes Qt List Qmldir / Qt List Qrc.                                                       |

## §10 — Acceptance Checklist

QT-09 is **ratified and shipped** when:

- [x] §3 names the Qt menu group label set.
- [x] §5 freezes the per-subcommand dialog flow.
- [x] `creator_ui/mod.rs` imports `qt` and `qt_scjson` modules.
- [x] `creator_ui/menus.rs` adds the Qt menu group.
- [x] `creator_ui/commands.rs` adds 10 `handle_qt_*` functions
      and 10 dispatch arms in `handle_action`.
- [x] `cargo check --features creator` (without `creator_ui`)
      builds clean — confirms the QT-09 wiring is syntactically
      correct against the `qt::*` public surface.
- [x] `cargo check --features creator,creator_ui` builds clean.
      Initially blocked by a pre-existing breakage in
      `creator_ui/../creator/boards.rs` (5 `raw_db` lookup
      errors against chip vendor crates that had migrated to
      the per-spec YAML accessor convention). Resolved
      same-session by rewriting `boards.rs::load_ir` to use
      `<vendor>::board_yaml(name)` / `<vendor>::chip_yaml(name)`
      for nrf / esp / nxp / renesas / rp2040, returning a clean
      "not yet supported" error for the still-stm-style silabs
      / microchip / ti vendors (whose source data is empty
      anyway), and dropping the dead `parse_raw_db` helper
      along with the `zstd` / `std::io::Read` / `HashMap`
      imports it required. `test_vendor` updated to expose
      `board_yaml` / `chip_yaml` accessors matching the new
      convention. See §15 amendment.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §11 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/creator/UI-DESIGN.md`](../creator/UI-DESIGN.md) — desktop UI parity goal.
- [`src/bin/creator_ui/menus.rs`](../../src/bin/creator_ui/menus.rs) — menu group definitions.
- [`src/bin/creator_ui/commands.rs`](../../src/bin/creator_ui/commands.rs) — handler + dispatch.
- [`src/bin/creator_ui/mod.rs`](../../src/bin/creator_ui/mod.rs) — module imports.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — `qt::*` public functions.

## §12 — Unblocks

Ratifying QT-09 unblocks:

- UI users running an end-to-end Qt → rlvgl flow without leaving
  the desktop application.
- A future "Qt project wizard" similar to the existing "Scan
  Convert Preview" wizard.
- QT-10 (strict-mode + release tag) — the UI parity gate is one
  fewer SHOULD blocking promotion to MUST.

## §13 — Files Cited

(see [§11](#11--files-cited))

## §14 — Files Cited

(see [§11](#11--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-09 §10 amendment #2 (same-day): silabs / microchip / ti chipdb crates migrated to the YAML accessor convention, removing the typed "not yet supported" fallback in `boards.rs::load_ir`. Each crate gains `db/chips/<name>.yaml` + `db/boards/<name>.yaml` seeds matching the existing `BoardInfo` entries (silabs: `EFM32GG11`; microchip: `ATSAMD51J19A`; ti: `MSP432P401R` + `beaglebone_black_nhd_cape`/`AM335x`), a copy of nrf's build.rs that scans those dirs and emits `generated.rs` with `chip_yaml_impl` / `board_yaml_impl` / `CHIP_NAMES` / `BOARD_NAMES` / `BOARD_INFOS`, a new lib.rs mirroring nrf's accessor surface (`vendor()` / `boards()` / `find()` / `chip_names()` / `board_names()` / `chip_yaml()` / `board_yaml()`), and per-crate smoke tests that pin presence-and-shape of each seeded YAML (silabs: 4, microchip: 4, ti: 5 = 13 new tests). Cargo.toml gains `[features] default = ["std"]; std = ["dep:serde"]` to silence the `unexpected cfg condition value: "std"` warning. All eight chipdb crates now share one accessor convention. `boards.rs::load_ir` extends to dispatch silabs / microchip / ti through the new path; the legacy raw_db error branch is removed entirely. Both feature gates build clean; 263 creator tests + 13 chipdb tests all green. |
| 2026-04-30 | QT-09 §10 amendment (same-day): pre-existing `creator_ui` build break resolved. `creator/boards.rs::load_ir` rewritten to consume the new `<vendor>::board_yaml(name)` / `<vendor>::chip_yaml(name)` accessor convention introduced by the chipdb crates; serde-YAML is parsed directly into `serde_json::Value` (the YAML / JSON serde representation is interchangeable). nrf / esp / nxp / renesas / rp2040 vendors take the new path; silabs / microchip / ti return a clean "not yet supported by the YAML accessor convention" error (their chipdb crates still ship the legacy raw_db blob with no source data wired, so they were broken before this change too — only now the failure mode is a typed error rather than a panic on zstd-decoding an empty blob). Dead `parse_raw_db` helper removed along with `zstd` / `std::io::Read` / `HashMap` imports it required. `test_vendor` updated to expose `board_yaml` / `chip_yaml` accessors. `cargo check --features creator,creator_ui -p rlvgl --bin rlvgl-creator` now builds clean. 263/263 creator-feature tests still green (no regression). |
| 2026-04-30 | QT-09 ratified and shipped (UI wiring; see §10 caveat re: pre-existing `creator_ui` build break). Desktop UI now exposes all 10 `qt …` CLI subcommands via a new "Qt" menu group with 10 entries (Qt Ingest / Qt Check / Qt Schema / Qt Emit / Qt Emit Scjson / Qt Emit Externals / Qt Emit Tokens / Qt List Assets / Qt List Qmldir / Qt List Qrc). Each entry follows the existing `handle_scan` pattern: file/folder picker → `qt::*` invocation → toast via `show_feedback`. `creator_ui/mod.rs` gains `mod qt;` + `mod qt_scjson;` (reused via `#[path]` from `creator/`). New helper `CreatorApp::pick_qml_input()` tries `.qml` file picker first, falls through to folder picker — covers the file-or-dir input mode QT-08 introduced. UI does not expose advanced flags (`qt emit --target data`, verbose, silent) at v1; CLI remains the canonical surface for those. **§10 caveat**: the wider `cargo check --features creator,creator_ui` build is blocked by 5 pre-existing `raw_db` lookup errors in `creator_ui/../creator/boards.rs` against the chip vendor crates (verified via `git stash` to predate QT-09). QT-09 verification falls back to `cargo check --features creator` compile-clean confirming the `qt::*` call sites in `commands.rs` are syntactically correct against the public surface. No emit-shape changes. No headless UI tests added — the codebase has no eframe test harness. Drag-and-drop, project-wide "run all qt" macro, and qmldir/qrc preview tabs remain deferred under future Specification-Required §5 amendments. |

---

MIT-licensed: MIT.
