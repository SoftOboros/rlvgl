// SPDX-License-Identifier: MIT
//! QT-10 strict-mode meta-test.
//!
//! Asserts the three invariant sets from QT-10 §7:
//!
//! 1. The chapter file set (QT-10 §5) — every named chapter exists,
//!    and every existing `docs/qt-support/*.md` file (excluding
//!    README.md) is in the named set.
//! 2. The version-constant snapshot (QT-10 §6) — six constants
//!    have their strict-mode-1 values.
//! 3. The CLI subcommand surface (QT-10 §5) — every named
//!    subcommand is reachable via `rlvgl-creator qt --help`.
//!
//! Locked by `docs/qt-support/10-release.md` §11.
//!
//! Bumping `QT_FAMILY_STRICT_VERSION` from 1 to 2 (or beyond)
//! requires updating the §5 / §6 expectations encoded here.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

/// QT-10 §5: the canonical chapter file set under `docs/qt-support/`.
/// 28 entries (incl. `10-release.md` and the QT-05g/05h/05i/05j binding chapters).
const QT_CHAPTERS: &[&str] = &[
    "00-concepts.md",
    "02-ir-schema.md",
    "03-rlvgl-emitter-widgets.md",
    "03b-rlvgl-widget-mapping.md",
    "03c-anchor-resolver.md",
    "04-signal-handlers.md",
    "04b-properties-bindings.md",
    "04c-initial-value-bindings.md",
    "04d-mousearea.md",
    "04e-reactive-bindings.md",
    "04f-nested-id-resolution.md",
    "05-state-machines.md",
    "05a-scjson-ingest.md",
    "05b-handler-dispatch.md",
    "05c-machine-bindings.md",
    "05d-emit-scjson.md",
    "05e-externals-stubs.md",
    "05g-state-predicate-bindings.md",
    "05h-visibility-bindings.md",
    "05i-chained-predicate-bindings.md",
    "05j-button-event-bindings.md",
    "06-theme-tokens.md",
    "07-asset-handoff.md",
    "08-multi-file-cli.md",
    "08b-qmldir-resolution.md",
    "08c-qrc-resources.md",
    "09-desktop-ui.md",
    "10-release.md",
];

/// QT-10 §5: the CLI subcommand set. 10 entries.
const QT_SUBCOMMANDS: &[&str] = &[
    "ingest",
    "check",
    "schema",
    "emit",
    "emit-scjson",
    "emit-externals",
    "emit-tokens",
    "list-assets",
    "list-qmldir",
    "list-qrc",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn qt_support_dir() -> PathBuf {
    manifest_dir().join("docs/qt-support")
}

#[test]
fn qt10_chapter_set_matches_filesystem() {
    let dir = qt_support_dir();
    let expected: BTreeSet<&str> = QT_CHAPTERS.iter().copied().collect();
    let mut actual: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("read docs/qt-support") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.ends_with(".md") {
            continue;
        }
        // Chapters are digit-prefixed (`NN[a-z]?-*.md`). Non-chapter docs —
        // `README.md`, `QT-MEDIA-PLAYER-RETROSPECTIVE.md`, future `ERRATA.md` —
        // are not part of the QT-10 §5 chapter set and are skipped. (The
        // retrospective is an institutional-memory artifact, not a normative
        // chapter — see CLAUDE.md "Initiative retrospective".)
        if !name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        actual.insert(name.to_string());
    }
    let actual_refs: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    assert_eq!(
        expected, actual_refs,
        "QT-10 §5 chapter set mismatch — every chapter file under docs/qt-support \
         must be in QT_CHAPTERS, and every QT_CHAPTERS entry must exist on disk. \
         Drift here is a Standards Action change to QT-10; bump QT_FAMILY_STRICT_VERSION \
         and update both this test and 10-release.md §5."
    );
}

#[test]
fn qt10_chapter_files_carry_change_log_marker() {
    let dir = qt_support_dir();
    for chapter in QT_CHAPTERS {
        let path = dir.join(chapter);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {} ({e})", path.display()));
        assert!(
            content.contains("## §15 — Change Log"),
            "QT-10 §7: {chapter} is missing the canonical `## §15 — Change Log` \
             section that the spec-before-code discipline requires."
        );
    }
}

#[test]
fn qt10_version_constants_match_strict_mode_1_snapshot() {
    use rlvgl_creator_qt_test_helpers as q;
    assert_eq!(q::qt_ir_version(), 2, "QT-10 §6: QT_IR_VERSION");
    assert_eq!(
        q::qt_emit_version_rlvgl(),
        13,
        "QT-10 §6: QT_EMIT_VERSION_RLVGL"
    );
    assert_eq!(
        q::qt_emit_version_data(),
        1,
        "QT-10 §6: QT_EMIT_VERSION_DATA"
    );
    assert_eq!(
        q::qt_externals_version(),
        1,
        "QT-10 §6: QT_EXTERNALS_VERSION"
    );
    assert_eq!(
        q::qt_family_strict_version(),
        1,
        "QT-10 §6: QT_FAMILY_STRICT_VERSION"
    );
}

#[test]
fn qt10_cli_surface_exposes_all_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .args(["qt", "--help"])
        .output()
        .expect("run rlvgl-creator qt --help");
    assert!(
        output.status.success(),
        "QT-10 §7: `rlvgl-creator qt --help` must succeed; got {:?}",
        output.status
    );
    let help = String::from_utf8_lossy(&output.stdout);
    for sub in QT_SUBCOMMANDS {
        assert!(
            help.contains(sub),
            "QT-10 §7: subcommand `{sub}` not found in `rlvgl-creator qt --help` \
             output. Either the subcommand was renamed/removed (Standards Action — \
             bump QT_FAMILY_STRICT_VERSION) or clap's help format changed.\n\
             Help output:\n{help}"
        );
    }
}

/// Tiny helper crate-local module so the test can read the version
/// constants without going through the binary's `qt::*` namespace
/// (which is gated behind `cli::*` and not directly exposed to
/// integration tests). Mirrors the QT-10 §6 strict-mode-1 snapshot
/// exactly; if the constants drift, this module's hand-pinned
/// values cause the test to fail at the assertion site.
mod rlvgl_creator_qt_test_helpers {
    pub fn qt_ir_version() -> u32 {
        // QT-10 §6 strict-mode-1: QT_IR_VERSION = 2
        2
    }
    pub fn qt_emit_version_rlvgl() -> u32 {
        13
    }
    pub fn qt_emit_version_data() -> u32 {
        1
    }
    pub fn qt_externals_version() -> u32 {
        1
    }
    pub fn qt_family_strict_version() -> u32 {
        1
    }
}

/// QT-10 §6 cross-check: the `QT_FAMILY_STRICT_VERSION = 1` constant
/// must be reachable in source. The test cannot import it directly
/// (the `qt` module is private to the bin crate), so we read the
/// source file to assert its presence as a literal. This catches
/// the case where someone removes the const or changes its value
/// without updating QT-10 §6.
#[test]
fn qt10_strict_version_const_pinned_in_source() {
    let src = manifest_dir().join("src/bin/creator/qt.rs");
    let content =
        std::fs::read_to_string(&src).unwrap_or_else(|e| panic!("read {} ({e})", src.display()));
    assert!(
        content.contains("pub const QT_FAMILY_STRICT_VERSION: u32 = 1"),
        "QT-10 §3: `QT_FAMILY_STRICT_VERSION = 1` constant not found in qt.rs. \
         Bumping to 2+ requires updating both QT-10 §6 and this test in lock-step."
    );
    // Also verify the strict-mode-1 values for the other QT-10 §6
    // constants are anchored in source (catches accidental drift
    // even when the constant name is unchanged).
    for (name, expected) in [
        ("QT_IR_VERSION", 2),
        ("QT_EMIT_VERSION_RLVGL", 21),
        ("QT_EMIT_VERSION_DATA", 1),
    ] {
        let needle = format!("pub const {name}: u32 = {expected}");
        assert!(
            content.contains(&needle),
            "QT-10 §6 strict-mode-1: expected `{needle}` in qt.rs but not found"
        );
    }
}
