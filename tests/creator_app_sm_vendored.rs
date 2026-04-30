//! APP-04c integration tests: orchestrator consumption of a
//! pre-generated SM crate per chapter 04 §5.3 vendored-crate
//! offline model + §6 CV-1 cross-validate.
//!
//! These tests construct a minimal fixture vendored crate inside
//! a tempdir (with a hand-written `.mcp-statechart-manifest.json`
//! and stub `states.rs` / `vectors.rs`), then run the orchestrator
//! against a manifest that points at it. No external
//! `mcp-statechart` tool is invoked; we exercise the orchestrator
//! contract end-to-end against fixture data.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::fs;
use std::path::{Path, PathBuf};

/// SM-bearing tests don't trigger BSP-gen — but the type signature
/// requires a `BspGenFn`, so we plumb a panic-on-call stub.
fn bsp_gen_unreachable(
    _vendor: &str,
    _board: &str,
    _chip: Option<&str>,
    _out_dir: &Path,
) -> anyhow::Result<String> {
    panic!("BSP-gen callback should not have been invoked in this test");
}

/// Build a minimal vendored SM crate at `<root>/sm-crate/` with the
/// given state ids and a fixed states.rs / vectors.rs content. The
/// hashes inside the self-manifest are placeholders — chapter 04
/// §5.5 requires them to be present, not validated by the
/// orchestrator at v0.
fn write_fixture_sm_crate(root: &Path, state_set: &[&str], with_vectors: bool) -> PathBuf {
    let dir = root.join("sm-crate");
    fs::create_dir_all(dir.join("src")).unwrap();

    let states_rs = format!(
        "// fixture states.rs\npub enum State {{ {} }}\n",
        state_set.join(", ")
    );
    fs::write(dir.join("src/states.rs"), &states_rs).unwrap();

    let mut files_json = String::from(
        r#"    { "path": "src/states.rs", "hash": "blake3:0000000000000000000000000000000000000000000000000000000000000001" }"#,
    );
    if with_vectors {
        let vectors_rs =
            "// fixture vectors.rs\n#[test]\nfn vector_idle_to_menu() { assert!(true); }\n";
        fs::write(dir.join("src/vectors.rs"), vectors_rs).unwrap();
        files_json.push_str(",\n");
        files_json.push_str(
            r#"    { "path": "src/vectors.rs", "hash": "blake3:0000000000000000000000000000000000000000000000000000000000000002" }"#,
        );
    }

    let state_set_json = state_set
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        r#"{{
  "tool": "mcp-statechart",
  "version": "0.1.0-fixture",
  "source": "states/main.scxml",
  "files": [
{files_json}
  ],
  "state_set": [{state_set_json}]
}}
"#
    );
    fs::write(dir.join(".mcp-statechart-manifest.json"), manifest).unwrap();
    dir
}

/// Build a minimal manifest pointing at the fixture SM crate. Writes
/// alongside `app.yaml` and a fake `states/main.scxml` so the
/// validator's path-safety checks pass.
fn write_manifest(root: &Path, screen_state: Option<&str>, vectors: bool) -> PathBuf {
    fs::create_dir_all(root.join("states")).unwrap();
    fs::write(root.join("states/main.scxml"), "<scxml/>").unwrap();
    fs::create_dir_all(root.join("layouts")).unwrap();
    fs::write(root.join("layouts/home.rs"), "// fixture layout\n").unwrap();

    let state_yaml = match screen_state {
        Some(s) => format!("    state: {s}\n"),
        None => String::new(),
    };
    let vectors_yaml = if vectors {
        ""
    } else {
        "  verification_vectors: false\n"
    };

    let body = format!(
        r#"schema: rlvgl-app/v0
name: sm-fixture-app

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

state_machine:
  source: states/main.scxml
  generator: mcp-statechart
  vendored_crate: sm-crate
{vectors_yaml}
screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true
{state_yaml}"#
    );
    let path = root.join("app.yaml");
    fs::write(&path, body).unwrap();
    path
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ─── Happy path: orchestrator consumes the fixture SM crate ──────────

#[test]
fn vendored_sm_crate_is_copied_into_output() {
    let tmp_root = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp_root.path(), &["idle", "menu", "settings"], true);
    let manifest = write_manifest(tmp_root.path(), Some("menu"), true);

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        // Fixture lives outside the rlvgl repo's workspace; pass the
        // fixture root as the workspace root so path-safety scoping
        // works.
        tmp_root.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let inv = orch.run().expect("orchestrator runs");

    // Three SM-gen entries: states.rs, vectors.rs, mod.rs.
    let sm_entries: Vec<&app::InventoryEntry> =
        inv.entries.iter().filter(|e| e.stage == "sm-gen").collect();
    assert_eq!(
        sm_entries.len(),
        3,
        "expected 3 sm-gen entries; got {sm_entries:?}"
    );
    for e in &sm_entries {
        assert!(!e.stub, "vendored consumption must not flag stub: {e:?}");
        assert!(e.hash.starts_with("blake3:"), "got: {}", e.hash);
    }

    // Files exist on disk.
    assert!(out.path().join("src/state_machine/states.rs").is_file());
    assert!(out.path().join("src/state_machine/vectors.rs").is_file());
    assert!(out.path().join("src/state_machine/mod.rs").is_file());

    // mod.rs is child-module-shaped per chapter 02 §5.4.
    let mod_rs = fs::read_to_string(out.path().join("src/state_machine/mod.rs")).unwrap();
    assert!(mod_rs.contains("pub mod states;"), "got:\n{mod_rs}");
    assert!(
        mod_rs.contains("#[cfg(test)]\npub mod vectors;"),
        "vectors guarded by cfg(test); got:\n{mod_rs}"
    );

    // states.rs content copied byte-for-byte.
    let states_rs = fs::read_to_string(out.path().join("src/state_machine/states.rs")).unwrap();
    assert!(states_rs.contains("pub enum State"), "got:\n{states_rs}");
}

// ─── verification_vectors: false omits vectors.rs ────────────────────

#[test]
fn verification_vectors_false_omits_vectors_rs() {
    let tmp_root = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp_root.path(), &["idle"], false);
    let manifest = write_manifest(tmp_root.path(), Some("idle"), false);

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp_root.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let _ = orch.run().expect("orchestrator runs");

    assert!(
        out.path().join("src/state_machine/states.rs").is_file(),
        "states.rs always emitted"
    );
    assert!(
        !out.path().join("src/state_machine/vectors.rs").exists(),
        "vectors.rs MUST NOT exist when verification_vectors: false"
    );
    let mod_rs = fs::read_to_string(out.path().join("src/state_machine/mod.rs")).unwrap();
    assert!(
        !mod_rs.contains("pub mod vectors"),
        "mod.rs must not re-export vectors when absent; got:\n{mod_rs}"
    );
}

// ─── CV-1: screen.state must appear in self-manifest's state_set ─────

#[test]
fn cv1_rejects_screen_state_not_in_state_set() {
    let tmp_root = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp_root.path(), &["idle", "menu"], true);
    // Reference a state ('settings') that is NOT in the SM's emitted
    // state_set.
    let manifest = write_manifest(tmp_root.path(), Some("settings"), true);

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp_root.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("CV-1 must reject unknown state")
        .to_string();
    assert!(err.contains("CV-1"), "got: {err}");
    assert!(err.contains("settings"), "got: {err}");
    assert!(err.contains("idle") && err.contains("menu"), "got: {err}");
}

// ─── verification_vectors: true but vectors.rs absent → error ────────

#[test]
fn vectors_required_when_verification_vectors_true() {
    let tmp_root = tempfile::tempdir().unwrap();
    // SM crate without vectors.rs but manifest defaults to true.
    write_fixture_sm_crate(tmp_root.path(), &["idle"], false);
    let manifest = write_manifest(tmp_root.path(), Some("idle"), true);

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp_root.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("missing vectors.rs must be rejected")
        .to_string();
    assert!(
        err.contains("vectors.rs") && err.contains("verification_vectors"),
        "got: {err}"
    );
}

// ─── Validator: vendored_crate dir must exist ────────────────────────

#[test]
fn validator_rejects_missing_vendored_crate_dir() {
    let tmp_root = tempfile::tempdir().unwrap();
    // No SM crate at all.
    let manifest = write_manifest(tmp_root.path(), Some("idle"), true);

    let err = app::validate(&manifest)
        .expect_err("missing vendored_crate dir must be rejected")
        .to_string();
    assert!(
        err.contains("rule 5") && err.contains("vendored_crate"),
        "got: {err}"
    );
}

// ─── Validator: vendored_crate dir without self-manifest ─────────────

#[test]
fn validator_rejects_vendored_crate_without_self_manifest() {
    let tmp_root = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp_root.path().join("sm-crate/src")).unwrap();
    fs::write(
        tmp_root.path().join("sm-crate/src/states.rs"),
        "// no self-manifest beside us\n",
    )
    .unwrap();
    let manifest = write_manifest(tmp_root.path(), Some("idle"), true);

    let err = app::validate(&manifest)
        .expect_err("missing self-manifest must be rejected")
        .to_string();
    assert!(
        err.contains("rule 5") && err.contains("self-manifest"),
        "got: {err}"
    );
}

// ─── Determinism: vendored consumption produces identical inventories.

#[test]
fn vendored_sm_emit_is_byte_deterministic() {
    let tmp_root = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp_root.path(), &["idle", "menu"], true);
    let manifest = write_manifest(tmp_root.path(), Some("idle"), true);

    let manifest_dir = manifest.parent().unwrap().to_path_buf();
    let ws_root = tmp_root.path().to_path_buf();

    let m1 = app::validate(&manifest).unwrap();
    let out1 = tempfile::tempdir().unwrap();
    let mut o1 = app::Orchestrator::new(
        m1,
        manifest_dir.clone(),
        ws_root.clone(),
        out1.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let inv1 = o1.run().unwrap();

    let m2 = app::validate(&manifest).unwrap();
    let out2 = tempfile::tempdir().unwrap();
    let mut o2 = app::Orchestrator::new(m2, manifest_dir, ws_root, out2.path().to_path_buf())
        .with_bsp_gen(bsp_gen_unreachable);
    let inv2 = o2.run().unwrap();

    let hashes1: Vec<&str> = inv1
        .entries
        .iter()
        .filter(|e| e.stage == "sm-gen")
        .map(|e| e.hash.as_str())
        .collect();
    let hashes2: Vec<&str> = inv2
        .entries
        .iter()
        .filter(|e| e.stage == "sm-gen")
        .map(|e| e.hash.as_str())
        .collect();
    assert_eq!(hashes1, hashes2, "sm-gen hashes drift across runs");
}

// Suppress unused-helper warnings when the test binary is linked
// without all helpers exercised.
#[allow(dead_code)]
fn _ws_unused() -> PathBuf {
    workspace_root()
}
