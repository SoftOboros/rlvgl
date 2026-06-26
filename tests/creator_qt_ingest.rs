// SPDX-License-Identifier: MIT
//! End-to-end test for the `rlvgl-creator qt ingest` subcommand.
//!
//! Drives the binary against `tests/fixtures/qt/hello.qml`, then asserts
//! the emitted `qt-ir.json` has the expected structural shape.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

#[test]
fn qt_ingest_emits_expected_ir() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qt/hello.qml");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(&fixture)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");

    let ir_path = out.path().join("qt-ir.json");
    assert!(ir_path.exists(), "qt-ir.json was not produced");
    let json: Value = serde_json::from_str(&std::fs::read_to_string(&ir_path).unwrap()).unwrap();

    // QT-05 bumped QT_IR_VERSION from 1 to 2 (additive `state_machine` field).
    assert_eq!(json["version"], 2);

    let imports = json["imports"].as_array().expect("imports array");
    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0]["module"], "QtQuick");
    assert_eq!(imports[0]["version"], "2.15");
    assert_eq!(imports[1]["module"], "QtQuick.Controls");
    assert_eq!(imports[1]["alias"], "QC");

    let root = &json["root"];
    assert_eq!(root["type_name"], "Item");
    assert_eq!(root["id"], "root");

    let props = root["properties"].as_array().unwrap();
    assert_eq!(props.len(), 3);
    assert_eq!(props[0]["name"], "title");
    assert_eq!(props[0]["ty"], "string");
    assert_eq!(props[2]["name"], "ratio");
    assert_eq!(props[2]["readonly"], true);

    let signals = root["signals"].as_array().unwrap();
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0]["name"], "pressed");
    assert_eq!(signals[0]["params"].as_array().unwrap().len(), 2);

    // Dotted assignment targets are preserved.
    let asn: Vec<&str> = root["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["target"].as_str().unwrap())
        .collect();
    assert!(asn.contains(&"width"));
    assert!(asn.contains(&"height"));
    assert!(asn.contains(&"anchors.fill"));
    assert!(asn.contains(&"anchors.margins"));
    assert!(asn.contains(&"font")); // grouped object
    assert!(asn.contains(&"transitions")); // object value

    // Children: Rectangle, QC.Label, MouseArea.
    let children = root["children"].as_array().unwrap();
    let names: Vec<&str> = children
        .iter()
        .map(|c| c["type_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Rectangle", "QC.Label", "MouseArea"]);

    // MouseArea handler bodies.
    let mouse = children
        .iter()
        .find(|c| c["type_name"] == "MouseArea")
        .unwrap();
    let handlers = mouse["handlers"].as_array().unwrap();
    let signal_names: Vec<&str> = handlers
        .iter()
        .map(|h| h["signal"].as_str().unwrap())
        .collect();
    assert!(signal_names.contains(&"onClicked"));
    assert!(signal_names.contains(&"onPressed"));
}

#[test]
fn qt_check_succeeds_on_valid_fixture() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qt/hello.qml");
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("check")
        .arg(&fixture)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt check should succeed on hello.qml");
}

#[test]
fn qt_check_fails_on_unterminated_block() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.qml");
    std::fs::write(&path, "Item {\n    width: 800\n").unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("check")
        .arg(&path)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(!status.success(), "qt check should fail on truncated input");
}

/// QT-03 golden-file gate: `qt emit --target data` against the
/// canonical fixture **MUST** produce byte-equivalent Rust to the
/// checked-in `tests/fixtures/qt/hello.rs`. `--target data` is
/// explicit since QT-03b flipped the default to `rlvgl`. The test
/// invokes the binary with a relative path (matching what a developer
/// types when regenerating the golden) so the source-path comments /
/// `QT_SOURCE` const stay stable. Compile-cleanness is enforced by
/// `tests/creator_qt_emit_compile.rs`, which `mod`s the same file.
#[test]
fn qt_emit_matches_canonical_golden_rs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/hello.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/hello.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("hello.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit output drifted from tests/fixtures/qt/hello.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit --target data tests/fixtures/qt/hello.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03 emit-shape policy \
             (docs/qt-support/03-rlvgl-emitter-widgets.md) before committing."
        );
    }
}

/// QT-04d fixture coverage: mousearea.qml exercises a typed
/// `MouseArea → ClickArea` lowering with a body that lowers via
/// QT-04b §7. Three gates pin every emit shape; the matching
/// compile-as-mod gate fires `Event::PressRelease` and asserts
/// `state.taps` mutated.
#[test]
fn qt_mousearea_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/mousearea.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/mousearea.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "mousearea ingest output drifted from golden"
    );
}

#[test]
fn qt_mousearea_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/mousearea.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/mousearea.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("mousearea.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "mousearea data emit drifted from golden"
    );
}

#[test]
fn qt_mousearea_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/mousearea.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/mousearea.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("mousearea.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/mousearea.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/mousearea.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-04d emit-shape policy \
             (docs/qt-support/04d-mousearea.md) before committing."
        );
    }
}

/// QT-03c §5 amendment #2 fixture coverage: corners.qml exercises
/// all four corner combinations (`left+top`, `right+top`,
/// `left+bottom`, `right+bottom`) on small badge Rectangles. Three
/// gates pin every emit shape; the matching compile-as-mod gate
/// asserts each badge's runtime bounds.
#[test]
fn qt_corners_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/corners.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/corners.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "corners ingest output drifted from golden"
    );
}

#[test]
fn qt_corners_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/corners.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/corners.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("corners.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "corners data emit drifted from golden"
    );
}

/// QT-03c §5 amendment #3 fixture coverage: siblings.qml exercises the
/// sibling-relative box-model anchor solver — children anchored to other
/// children (`<id>.<edge>`), axial fills (`left+right`, `top+bottom`), and
/// topological declaration ordering of the emitted `cb_<i>` Rects.
#[test]
fn qt_siblings_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/siblings.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/siblings.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("siblings.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/siblings.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/siblings.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03c §5 amendment #3 \
             (docs/qt-support/03c-anchor-resolver.md) before committing."
        );
    }
}

#[test]
fn qt_corners_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/corners.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/corners.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("corners.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/corners.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/corners.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03c §5 amendment #2 \
             (docs/qt-support/03c-anchor-resolver.md) before committing."
        );
    }
}

/// QT-03c §5 amendment fixture coverage: edges.qml exercises the
/// four single-edge anchors lowered in isolation. Three gates pin
/// every emit shape; the matching compile-as-mod gate asserts each
/// child's runtime bounds.
#[test]
fn qt_edges_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/edges.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/edges.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "edges ingest output drifted from golden"
    );
}

#[test]
fn qt_edges_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/edges.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/edges.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("edges.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "edges data emit drifted from golden"
    );
}

#[test]
fn qt_edges_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/edges.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/edges.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("edges.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/edges.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/edges.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03c §5 amendment \
             (docs/qt-support/03c-anchor-resolver.md) before committing."
        );
    }
}

/// QT-04f fixture coverage: nested.qml exercises a non-root id'd
/// `Rectangle { id: bg; property int alpha: 100 }` and a sibling
/// Button whose `onClicked: bg.alpha -= 10` lowers via the
/// QT-04f resolution walk into a namespaced `s.bg_alpha`
/// mutation. Three gates pin every emit shape; the matching
/// compile-as-mod gate verifies the runtime mutation.
#[test]
fn qt_nested_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/nested.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/nested.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "nested ingest output drifted from golden"
    );
}

#[test]
fn qt_nested_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/nested.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/nested.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("nested.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "nested data emit drifted from golden"
    );
}

#[test]
fn qt_nested_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/nested.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/nested.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("nested.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/nested.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/nested.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-04f emit-shape policy \
             (docs/qt-support/04f-nested-id-resolution.md) before committing."
        );
    }
}

/// QT-08 fixture coverage: `tests/fixtures/qt/multi/{a,b}.qml`
/// exercises the directory-mode CLI dispatch. Three gates check
/// that ingest / data emit / rlvgl emit each produce the expected
/// pair of `<basename>.<suffix>` outputs in lexical order.
#[test]
fn qt_multi_dir_ingest_produces_per_basename_outputs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let multi_dir = manifest_dir.join("tests/fixtures/qt/multi");
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(&multi_dir)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest <dir> failed");
    let a = out.path().join("a.qt-ir.json");
    let b = out.path().join("b.qt-ir.json");
    assert!(a.exists(), "missing a.qt-ir.json");
    assert!(b.exists(), "missing b.qt-ir.json");

    // Body sanity — each per-file IR carries its own root.
    let a_json: Value = serde_json::from_str(&std::fs::read_to_string(&a).unwrap()).unwrap();
    let b_json: Value = serde_json::from_str(&std::fs::read_to_string(&b).unwrap()).unwrap();
    assert_eq!(a_json["root"]["id"], "a");
    assert_eq!(b_json["root"]["id"], "b");
}

#[test]
fn qt_multi_dir_emit_data_produces_per_basename_outputs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let multi_dir = manifest_dir.join("tests/fixtures/qt/multi");
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg(&multi_dir)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data <dir> failed");
    assert!(out.path().join("a.rs").exists());
    assert!(out.path().join("b.rs").exists());
}

#[test]
fn qt_multi_dir_emit_rlvgl_produces_per_basename_outputs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let multi_dir = manifest_dir.join("tests/fixtures/qt/multi");
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg(&multi_dir)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl) <dir> failed");
    assert!(out.path().join("a.rlvgl.rs").exists());
    assert!(out.path().join("b.rlvgl.rs").exists());
}

/// QT-03c fixture coverage: centered.qml exercises a Rectangle
/// with literal `width: 50; height: 50; anchors.centerIn: parent`
/// inside a 200×200 parent. Three gates pin the IR / data emit /
/// rlvgl emit; the matching compile-as-mod gate asserts the child's
/// runtime bounds are (75, 75, 50, 50).
#[test]
fn qt_centered_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/centered.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/centered.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "centered ingest output drifted from golden"
    );
}

#[test]
fn qt_centered_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/centered.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/centered.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("centered.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "centered data emit drifted from golden"
    );
}

#[test]
fn qt_centered_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/centered.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/centered.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("centered.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/centered.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/centered.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03c emit-shape policy \
             (docs/qt-support/03c-anchor-resolver.md) before committing."
        );
    }
}

/// QT-04c fixture coverage: bound_text.qml exercises a Label
/// whose `text:` is bound to a root-scope `string` property,
/// lowered to a construction-time `state.borrow().<field>.clone()`
/// read.
#[test]
fn qt_bound_text_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/bound_text.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/bound_text.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "bound_text ingest output drifted from golden"
    );
}

#[test]
fn qt_bound_text_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/bound_text.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/bound_text.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("bound_text.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "bound_text data emit drifted from golden"
    );
}

#[test]
fn qt_bound_text_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/bound_text.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/bound_text.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("bound_text.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/bound_text.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/bound_text.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-04c emit-shape policy \
             (docs/qt-support/04c-initial-value-bindings.md) before committing."
        );
    }
}

/// QT-04b fixture coverage: counter.qml exercises a `property int
/// count: 0` declaration + `onClicked: count += 1` handler body
/// lowered to a state-mutating closure. Three gates — ingest IR,
/// data emit, rlvgl emit — pin every emit shape.
#[test]
fn qt_counter_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/counter.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/counter.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "counter ingest output drifted from golden"
    );
}

#[test]
fn qt_counter_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/counter.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/counter.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("counter.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "counter data emit drifted from golden"
    );
}

#[test]
fn qt_counter_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/counter.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/counter.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("counter.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/counter.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/counter.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-04b emit-shape policy \
             (docs/qt-support/04b-properties-bindings.md) before committing."
        );
    }
}

/// QT-04 fixture coverage: clickable.qml exercises Button +
/// onClicked. Three gates — ingest IR, data emit, rlvgl emit — share
/// the canonical fixture and pin every emit shape against drift.
#[test]
fn qt_clickable_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/clickable.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/clickable.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "qt ingest output drifted from clickable golden"
    );
}

#[test]
fn qt_clickable_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/clickable.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/clickable.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("clickable.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "qt emit --target data output drifted from clickable golden"
    );
}

#[test]
fn qt_clickable_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/clickable.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/clickable.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("clickable.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/clickable.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/clickable.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-04 emit-shape policy \
             (docs/qt-support/04-signal-handlers.md) before committing."
        );
    }
}

/// QT-03b golden-file gate: `qt emit --target rlvgl` (the default)
/// against the canonical fixture **MUST** produce byte-equivalent
/// Rust to the checked-in `tests/fixtures/qt/hello.rlvgl.rs`.
/// Compile-cleanness against `rlvgl-core` + `rlvgl-widgets` is
/// enforced by the same `creator_qt_emit_compile` target.
#[test]
fn qt_emit_rlvgl_matches_canonical_golden_rs() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/hello.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/hello.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("hello.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/hello.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/hello.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-03b emit-shape policy \
             (docs/qt-support/03b-rlvgl-widget-mapping.md) before committing."
        );
    }
}

/// QT-02 golden-file gate: `qt ingest` against the canonical fixture
/// **MUST** produce IR that matches the checked-in
/// `tests/fixtures/qt/hello.qt-ir.json` modulo the top-level `source`
/// field (which captures the input path verbatim and so naturally
/// varies between absolute and relative invocations). On drift, the
/// failure message names the regen command.
#[test]
fn qt_ingest_matches_canonical_golden() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qt/hello.qml");
    let golden =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qt/hello.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(&fixture)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");

    if canonical != produced {
        panic!(
            "qt-ir output drifted from tests/fixtures/qt/hello.qt-ir.json.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt ingest tests/fixtures/qt/hello.qml tests/fixtures/qt && \
             mv tests/fixtures/qt/qt-ir.json tests/fixtures/qt/hello.qt-ir.json\n\
             Verify the diff is intentional under the QT-02 bumping policy \
             (docs/qt-support/02-ir-schema.md) before committing."
        );
    }
}

#[test]
fn qt_schema_to_file_validates_emitted_ir() {
    // Emit the schema to a file, ingest the fixture, and check the
    // schema describes a UiModule object with the right top-level
    // properties. We don't pull in a JSON Schema validator crate —
    // structural smoke checks are enough to catch derive regressions.
    let dir = tempdir().unwrap();
    let schema_path = dir.path().join("qt-ir.schema.json");
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("schema")
        .arg("--out")
        .arg(&schema_path)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success());
    assert!(schema_path.exists());

    let schema: Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap()).unwrap();
    assert_eq!(schema["title"], "UiModule");
    let props = schema["properties"].as_object().unwrap();
    for required in ["version", "source", "imports", "pragmas", "root"] {
        assert!(
            props.contains_key(required),
            "schema missing top-level property `{required}`"
        );
    }
    let defs = schema["$defs"].as_object().unwrap();
    for ty in [
        "UiItem",
        "UiImport",
        "UiProperty",
        "UiAssignment",
        "UiAssignmentValue",
        "UiSignal",
        "UiSignalParam",
        "UiHandler",
        // QT-05: state-machine IR types per
        // `docs/qt-support/05-state-machines.md` §3.
        "UiStateMachine",
        "UiState",
        "UiTransition",
        "UiAction",
        "UiDmField",
        "UiScript",
        "UiScriptOrigin",
    ] {
        assert!(defs.contains_key(ty), "schema missing $defs/{ty}");
    }
}

// ============================================================================
// QT-05a: stopwatch fixture drift gates
// (`docs/qt-support/05a-scjson-ingest.md` §11)
// ============================================================================

/// QT-05a §11 acceptance gate: ingest of `stopwatch.qml` discovers
/// the sibling `stopwatch.scjson`, walks it per QT-05a §6, and the
/// resulting `qt-ir.json` is byte-equal to the checked-in golden.
/// Beyond byte equality, this test pins QT-05a's structural
/// guarantees: `state_machine` is `Some(_)`, the `id`/`initial`
/// derivations follow §8/§6, and every QT-05 §5 element appears
/// where the walk algorithm puts it.
#[test]
fn qt_stopwatch_fixture_ingest_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/stopwatch.qt-ir.json");
    let canonical_text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg("tests/fixtures/qt/stopwatch.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");
    let produced_text = std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap();

    let mut canonical: Value = serde_json::from_str(&canonical_text).unwrap();
    let mut produced: Value = serde_json::from_str(&produced_text).unwrap();
    canonical.as_object_mut().unwrap().remove("source");
    produced.as_object_mut().unwrap().remove("source");
    assert_eq!(
        canonical, produced,
        "stopwatch ingest output drifted from golden"
    );

    // QT-05a §11: structural assertions on the populated IR.
    let sm = produced
        .get("state_machine")
        .expect("state_machine field present")
        .as_object()
        .expect("state_machine is an object");
    assert_eq!(
        sm.get("id").and_then(|v| v.as_str()),
        Some("stopwatch"),
        "QT-05a §8: `<sm>` ID derivation"
    );
    assert_eq!(
        sm.get("source").and_then(|v| v.as_str()),
        Some("stopwatch.scjson"),
        "QT-05a §3: source field carries the scjson basename"
    );
    assert_eq!(
        sm.get("initial").and_then(|v| v.as_str()),
        Some("idle"),
        "QT-05a §6 step 1: initial state lifted from scjson"
    );
    let states = sm.get("states").and_then(|v| v.as_array()).unwrap();
    assert_eq!(states.len(), 2, "QT-05a §6 step 2: idle + running");
    assert_eq!(states[0].get("id").and_then(|v| v.as_str()), Some("idle"));
    assert_eq!(
        states[1].get("id").and_then(|v| v.as_str()),
        Some("running")
    );
    let transitions = sm.get("transitions").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        transitions.len(),
        3,
        "QT-05a §6 step 3: start/reset from idle, stop from running"
    );
    let dm = sm.get("datamodel").and_then(|v| v.as_array()).unwrap();
    assert_eq!(dm.len(), 2, "QT-05a §6 step 4: elapsed + lap");
    let scripts = sm.get("scripts").and_then(|v| v.as_array()).unwrap();
    assert_eq!(
        scripts.len(),
        2,
        "QT-05a §6 step 5: tick_start (onentry running) + tick_stop (onexit running)"
    );
    assert_eq!(
        scripts[0].get("name").and_then(|v| v.as_str()),
        Some("tick_start")
    );
    assert_eq!(
        scripts[1].get("name").and_then(|v| v.as_str()),
        Some("tick_stop")
    );
}

/// QT-05a §11 acceptance gate: the data-target Rust emit for
/// stopwatch.qml is byte-equal to its checked-in golden. The
/// emitter ignores `state_machine` for now (QT-05b/c/e own
/// emit-side glue), so this gate just confirms QT-05a does not
/// regress data-target emission.
#[test]
fn qt_stopwatch_fixture_data_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/stopwatch.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("--target")
        .arg("data")
        .arg("tests/fixtures/qt/stopwatch.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit --target data failed");
    let produced = std::fs::read_to_string(out.path().join("stopwatch.rs")).unwrap();
    assert_eq!(
        canonical.trim_end(),
        produced.trim_end(),
        "stopwatch data emit drifted from golden"
    );
}

/// QT-05a §11 acceptance gate: the rlvgl-target emit for
/// stopwatch.qml is byte-equal to its checked-in golden. Same
/// caveat: emit-side SM glue is QT-05b/c/e territory.
#[test]
fn qt_stopwatch_fixture_rlvgl_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/stopwatch.rlvgl.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit")
        .arg("tests/fixtures/qt/stopwatch.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit (rlvgl default) failed");
    let produced = std::fs::read_to_string(out.path().join("stopwatch.rlvgl.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit (rlvgl) output drifted from tests/fixtures/qt/stopwatch.rlvgl.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit tests/fixtures/qt/stopwatch.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-05a emit-shape policy \
             (docs/qt-support/05a-scjson-ingest.md) before committing."
        );
    }
}

/// QT-05a §7 error contract: a malformed `<basename>.scjson`
/// alongside the `.qml` causes `qt ingest` to exit non-zero with
/// the underlying serde_json error. Uses a tempdir to avoid
/// polluting the canonical fixture set.
#[test]
fn qt_malformed_scjson_side_file_is_a_hard_error() {
    let work = tempdir().unwrap();
    std::fs::write(
        work.path().join("foo.qml"),
        "import QtQuick 2.15\nItem { id: root }\n",
    )
    .unwrap();
    std::fs::write(
        work.path().join("foo.scjson"),
        "{ this is not valid JSON,, }",
    )
    .unwrap();

    let out = tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(work.path().join("foo.qml"))
        .arg(out.path())
        .output()
        .expect("failed to run rlvgl-creator");
    assert!(
        !output.status.success(),
        "qt ingest must fail on malformed scjson"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scjson") || stderr.contains("foo.scjson"),
        "stderr should name the offending side-file; got: {stderr}"
    );
}

// ============================================================================
// QT-05d: inline states/transitions → scjson round-trip gates
// (`docs/qt-support/05d-emit-scjson.md` §11)
// ============================================================================

/// QT-05d §11 acceptance gate: `qt emit-scjson` on the inline-
/// states fixture produces a byte-stable `.scjson` matching the
/// checked-in golden, AND a subsequent ingest of that `.scjson`
/// (via QT-05a's side-file probe) yields a `UiStateMachine` with
/// the same shape the inline QML declares.
#[test]
fn qt_inline_states_fixture_emit_scjson_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/inline_states.scjson");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit-scjson")
        .arg("tests/fixtures/qt/inline_states.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit-scjson failed");
    let produced = std::fs::read_to_string(out.path().join("inline_states.scjson")).unwrap();

    // The emitted file embeds the source QML path in `_comment`.
    // QT-05d §6: this is provenance-only; we drop it before
    // comparison so a `current_dir` change does not flake.
    let mut canonical_v: Value = serde_json::from_str(&canonical).unwrap();
    let mut produced_v: Value = serde_json::from_str(&produced).unwrap();
    if let Some(other) = canonical_v
        .get_mut("other_attributes")
        .and_then(|v| v.as_object_mut())
    {
        other.remove("_comment");
    }
    if let Some(other) = produced_v
        .get_mut("other_attributes")
        .and_then(|v| v.as_object_mut())
    {
        other.remove("_comment");
    }
    assert_eq!(
        canonical_v, produced_v,
        "QT-05d emit-scjson output drifted from golden"
    );
}

/// QT-05d §11 / §8 round-trip parity: emit-scjson, then ingest the
/// inline_states.qml (which now has a sibling `.scjson` from the
/// previous emit run) and assert the resulting `state_machine`
/// shape matches the QML's inline declarations.
#[test]
fn qt_inline_states_emit_then_ingest_roundtrip_parity() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let work = tempdir().unwrap();
    // Copy the .qml into the workdir so the emit-scjson output
    // lands next to it, then ingest from the workdir.
    let qml_src = manifest_dir.join("tests/fixtures/qt/inline_states.qml");
    let qml_dst = work.path().join("inline_states.qml");
    std::fs::copy(&qml_src, &qml_dst).unwrap();

    // Step 1: emit-scjson into the workdir.
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit-scjson")
        .arg(&qml_dst)
        .arg(work.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit-scjson failed");
    assert!(
        work.path().join("inline_states.scjson").exists(),
        "emit-scjson did not produce the .scjson next to the .qml"
    );

    // Step 2: ingest. The QT-05a discovery picks up the side-file.
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(&qml_dst)
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt ingest failed");

    // Step 3: assert the populated state_machine matches the
    // inline QML's declarations.
    let ir: Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap())
            .unwrap();
    let sm = ir
        .get("state_machine")
        .expect("state_machine populated after round-trip")
        .as_object()
        .expect("state_machine is an object");
    assert_eq!(
        sm.get("initial").and_then(|v| v.as_str()),
        Some("idle"),
        "QT-05d §6: State {{ initial: true }} round-trips to <scxml initial=\"…\">"
    );
    let states = sm.get("states").and_then(|v| v.as_array()).unwrap();
    let names: Vec<&str> = states
        .iter()
        .map(|s| s.get("id").and_then(|v| v.as_str()).unwrap())
        .collect();
    assert_eq!(names, vec!["idle", "running"]);
    let transitions = sm.get("transitions").and_then(|v| v.as_array()).unwrap();
    assert_eq!(transitions.len(), 2);
    let events: Vec<&str> = transitions
        .iter()
        .map(|t| t.get("event").and_then(|v| v.as_str()).unwrap())
        .collect();
    assert!(events.contains(&"start"));
    assert!(events.contains(&"stop"));
}

// ============================================================================
// QT-08c: .qrc resource manifest parsing — drift gate
// (`docs/qt-support/08c-qrc-resources.md` §11)
// ============================================================================

/// QT-08c §11 acceptance gate: `qt list-qrc` against the
/// resources.qrc fixture is byte-equal to the checked-in golden.
/// Pins the XML subset parser, qresource block ordering, and
/// alias-attribute surfacing.
#[test]
fn qt_resources_list_qrc_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/resources.qrc.yaml");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-qrc")
        .arg("tests/fixtures/qt/resources.qrc")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt list-qrc failed");
    let produced = std::fs::read_to_string(out.path().join("resources.qrc.yaml")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt list-qrc output drifted from tests/fixtures/qt/resources.qrc.yaml.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt list-qrc tests/fixtures/qt/resources.qrc tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-08c emit-shape policy \
             (docs/qt-support/08c-qrc-resources.md) before committing."
        );
    }
}

/// QT-08c §7: missing input file is a hard error.
#[test]
fn qt_list_qrc_missing_input_is_hard_error() {
    let work = tempdir().unwrap();
    let missing = work.path().join("does_not_exist.qrc");
    let output = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-qrc")
        .arg(&missing)
        .output()
        .expect("failed to run rlvgl-creator");
    assert!(
        !output.status.success(),
        "qt list-qrc must fail when the input is absent"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(".qrc") || stderr.contains("not found"),
        "stderr should name the missing qrc; got: {stderr}"
    );
}

// ============================================================================
// QT-08b: qmldir manifest parsing — drift gate
// (`docs/qt-support/08b-qmldir-resolution.md` §11)
// ============================================================================

/// QT-08b §11 acceptance gate: `qt list-qmldir` against the
/// sample_module fixture is byte-equal to the checked-in
/// `sample_module.qmldir.yaml` golden. Pins the directive
/// recognition + unrecognised-line passthrough.
#[test]
fn qt_sample_module_list_qmldir_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/sample_module.qmldir.yaml");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-qmldir")
        .arg("tests/fixtures/qt/sample_module")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt list-qmldir failed");
    let produced = std::fs::read_to_string(out.path().join("sample_module.qmldir.yaml")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt list-qmldir output drifted from tests/fixtures/qt/sample_module.qmldir.yaml.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt list-qmldir tests/fixtures/qt/sample_module tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-08b emit-shape policy \
             (docs/qt-support/08b-qmldir-resolution.md) before committing."
        );
    }
}

/// QT-08b §7: missing qmldir is a hard error (non-silent).
#[test]
fn qt_list_qmldir_missing_input_is_hard_error() {
    let work = tempdir().unwrap();
    // No qmldir created in the workdir.
    let output = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-qmldir")
        .arg(work.path())
        .output()
        .expect("failed to run rlvgl-creator");
    assert!(
        !output.status.success(),
        "qt list-qmldir must fail when qmldir is absent"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("qmldir") || stderr.contains("not found"),
        "stderr should name the missing qmldir; got: {stderr}"
    );
}

// ============================================================================
// QT-07: asset-handoff inventory — drift gate
// (`docs/qt-support/07-asset-handoff.md` §11)
// ============================================================================

/// QT-07 §11 acceptance gate: `qt list-assets` against the
/// image_refs fixture is byte-equal to the checked-in
/// `image_refs.assets.yaml` golden. Pins QT-07's `qrc:` prefix
/// stripping, dedup, lexical ordering, and YAML quoting rules
/// for whitespace-bearing scalars (e.g. `"FiraSans Bold"`).
#[test]
fn qt_image_refs_list_assets_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/image_refs.assets.yaml");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-assets")
        .arg("tests/fixtures/qt/image_refs.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt list-assets failed");
    let produced = std::fs::read_to_string(out.path().join("image_refs.assets.yaml")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt list-assets output drifted from tests/fixtures/qt/image_refs.assets.yaml.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt list-assets tests/fixtures/qt/image_refs.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-07 emit-shape policy \
             (docs/qt-support/07-asset-handoff.md) before committing."
        );
    }
}

/// QT-07 §7: a `.qml` with no asset references silently skips
/// (no `<basename>.assets.yaml` produced). Exercised against
/// `counter.qml` (no Image / no font.family).
#[test]
fn qt_list_assets_silent_skip_for_non_asset_qml() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("list-assets")
        .arg("tests/fixtures/qt/counter.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(
        status.success(),
        "qt list-assets should succeed (silent skip) on non-asset qml"
    );
    assert!(
        !out.path().join("counter.assets.yaml").exists(),
        "no .assets.yaml should be produced for a QML with no asset refs"
    );
}

// ============================================================================
// QT-06: theme-token emission — drift gate
// (`docs/qt-support/06-theme-tokens.md` §11)
// ============================================================================

/// QT-06 §11 acceptance gate: `qt emit-tokens` against the Theme.qml
/// fixture is byte-equal to the checked-in `Theme.tokens.yaml` golden.
/// Pins QT-06's name-to-category mapping (§6), lexical key
/// ordering, and the dark-mode `_dark` suffix convention.
#[test]
fn qt_theme_emit_tokens_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/Theme.tokens.yaml");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit-tokens")
        .arg("tests/fixtures/qt/Theme.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit-tokens failed");
    let produced = std::fs::read_to_string(out.path().join("Theme.tokens.yaml")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit-tokens output drifted from tests/fixtures/qt/Theme.tokens.yaml.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit-tokens tests/fixtures/qt/Theme.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-06 emit-shape policy \
             (docs/qt-support/06-theme-tokens.md) before committing."
        );
    }
}

/// QT-06 §6: a `.qml` with no recognised theme properties (e.g.
/// the existing widget fixtures) silently skips — no `tokens.yaml`
/// is produced.
#[test]
fn qt_emit_tokens_silent_skip_for_non_theme_qml() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit-tokens")
        .arg("tests/fixtures/qt/hello.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(
        status.success(),
        "qt emit-tokens should succeed (silent skip) on non-theme qml"
    );
    assert!(
        !out.path().join("hello.tokens.yaml").exists(),
        "no tokens.yaml should be produced for a QML with no theme properties"
    );
}

// ============================================================================
// QT-05e: externals stub emission — drift gate
// (`docs/qt-support/05e-externals-stubs.md` §11)
// ============================================================================

/// QT-05e §11 acceptance gate: `qt emit-externals` against the
/// stopwatch fixture is byte-equal to the checked-in golden.
#[test]
fn qt_stopwatch_externals_emit_matches_golden() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden = manifest_dir.join("tests/fixtures/qt/stopwatch_externals.rs");
    let canonical = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("missing golden at {} ({e})", golden.display()));

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("emit-externals")
        .arg("tests/fixtures/qt/stopwatch.qml")
        .arg(out.path())
        .current_dir(&manifest_dir)
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(status.success(), "qt emit-externals failed");
    let produced = std::fs::read_to_string(out.path().join("stopwatch_externals.rs")).unwrap();

    if canonical.trim_end() != produced.trim_end() {
        panic!(
            "qt emit-externals output drifted from tests/fixtures/qt/stopwatch_externals.rs.\n\
             Regenerate with:\n  \
             cargo run --features creator --bin rlvgl-creator -- \
             qt emit-externals tests/fixtures/qt/stopwatch.qml tests/fixtures/qt\n\
             Verify the diff is intentional under the QT-05e emit-shape policy \
             (docs/qt-support/05e-externals-stubs.md) before committing."
        );
    }
}

/// QT-05a §5 / §7: a `.qml` with no sibling `.scjson` ingests
/// silently (no error, `state_machine = None`). This is the
/// fall-through contract that keeps every pre-QT-05 fixture
/// stable.
#[test]
fn qt_missing_scjson_side_file_is_silent_fall_through() {
    let work = tempdir().unwrap();
    std::fs::write(
        work.path().join("noscjson.qml"),
        "import QtQuick 2.15\nItem { id: root }\n",
    )
    .unwrap();
    // No noscjson.scjson on purpose.

    let out = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("--silent")
        .arg("qt")
        .arg("ingest")
        .arg(work.path().join("noscjson.qml"))
        .arg(out.path())
        .status()
        .expect("failed to run rlvgl-creator");
    assert!(
        status.success(),
        "qt ingest must succeed when no scjson exists"
    );
    let produced: Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("qt-ir.json")).unwrap())
            .unwrap();
    assert!(
        produced
            .get("state_machine")
            .map(|v| v.is_null())
            .unwrap_or(true),
        "state_machine must be None / absent when no .scjson sibling exists; got: {produced:?}"
    );
}
