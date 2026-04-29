//! APP-02h integration tests: parallel stage 3 dispatch per
//! chapter 02 §5.1 + §5.2. The chapter's normative claim is
//! that running BSP-gen / asset-pipeline / SM-gen / i18n / theme
//! in parallel produces byte-identical output to running them
//! sequentially (§9.1 determinism). These tests build a fixture
//! manifest that exercises 4-of-5 eligible stages (asset, SM,
//! i18n, theme — BSP-gen is callback-only so we leave it
//! unwired and rely on the stub fallback) and assert
//! `jobs=1` and `jobs=4` produce identical inventories and
//! identical file contents.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::fs;
use std::path::{Path, PathBuf};

fn bsp_gen_unreachable(
    _vendor: &str,
    _board: &str,
    _chip: Option<&str>,
    _out_dir: &Path,
) -> anyhow::Result<String> {
    panic!("BSP-gen callback should not have been invoked in this test");
}

fn write_fixture_sm_crate(root: &Path, state_set: &[&str]) -> PathBuf {
    let dir = root.join("sm-crate");
    fs::create_dir_all(dir.join("src")).unwrap();
    let states_rs = format!(
        "// fixture states.rs\npub enum State {{ {} }}\n",
        state_set.join(", ")
    );
    fs::write(dir.join("src/states.rs"), states_rs).unwrap();
    fs::write(
        dir.join("src/vectors.rs"),
        "// fixture vectors.rs\n#[test]\nfn vector_idle() { assert!(true); }\n",
    )
    .unwrap();
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
    {{ "path": "src/states.rs", "hash": "blake3:0000000000000000000000000000000000000000000000000000000000000001" }},
    {{ "path": "src/vectors.rs", "hash": "blake3:0000000000000000000000000000000000000000000000000000000000000002" }}
  ],
  "state_set": [{state_set_json}]
}}
"#
    );
    fs::write(dir.join(".mcp-statechart-manifest.json"), manifest).unwrap();
    dir
}

fn write_full_manifest(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("states")).unwrap();
    fs::write(root.join("states/main.scxml"), "<scxml/>").unwrap();
    fs::create_dir_all(root.join("locales")).unwrap();
    fs::write(
        root.join("locales/en.json"),
        r#"{"demo.title":"Demo","hw.touch":"Touch"}
"#,
    )
    .unwrap();
    fs::write(
        root.join("locales/fr.json"),
        r#"{"demo.title":"Démo","hw.touch":"Tact"}
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("assets/splash.bin"), b"FAKE_SPLASH_BYTES").unwrap();
    fs::write(root.join("assets/inter-16.bin"), b"FAKE_FONT_BYTES").unwrap();
    fs::write(
        root.join("theme.json"),
        r##"{
  "colors": { "primary": "#3182CE", "background": "#FFFFFF" },
  "space": { "sp_4": 16, "sp_8": 32 },
  "radii": { "md": 6 }
}
"##,
    )
    .unwrap();
    fs::create_dir_all(root.join("layouts")).unwrap();
    fs::write(root.join("layouts/home.rs"), "// home layout\n").unwrap();

    let body = r#"schema: rlvgl-app/v0
name: parallel-fixture-app

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

state_machine:
  source: states/main.scxml
  generator: mcp-statechart
  vendored_crate: sm-crate

assets:
  - id: splash
    class: image_rgb565
    source: assets/splash.bin
  - id: inter-16
    class: font
    source: assets/inter-16.bin

theme:
  source: theme.json
  format: chakra_tokens_v1

i18n:
  bundle_dir: locales
  default_locale: en
  format: rlvgl_i18n_v1

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    state: idle
"#;
    let path = root.join("app.yaml");
    fs::write(&path, body).unwrap();
    path
}

fn run_with_jobs(
    manifest: &Path,
    ws_root: &Path,
    jobs: usize,
) -> (tempfile::TempDir, app::Inventory) {
    let m = app::validate(manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        ws_root.to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable)
    .with_jobs(jobs);
    let inv = orch.run().expect("orchestrator runs");
    (out, inv)
}

fn collect_files_relative(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                let bytes = fs::read(&p).unwrap();
                out.push((rel, bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ─── Determinism: sequential vs parallel produce identical output ───

#[test]
fn parallel_run_matches_sequential_byte_for_byte() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp.path(), &["idle", "menu"]);
    let manifest = write_full_manifest(tmp.path());

    let (out_seq, inv_seq) = run_with_jobs(&manifest, tmp.path(), 1);
    let (out_par, inv_par) = run_with_jobs(&manifest, tmp.path(), 4);

    // Inventory entry order MUST match (canonical merge in app.rs).
    let seq_paths: Vec<&str> = inv_seq.entries.iter().map(|e| e.path.as_str()).collect();
    let par_paths: Vec<&str> = inv_par.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(seq_paths, par_paths, "inventory order drifted");

    // Inventory hashes MUST match.
    for (s, p) in inv_seq.entries.iter().zip(&inv_par.entries) {
        assert_eq!(s.path, p.path);
        assert_eq!(
            s.hash, p.hash,
            "hash drift on {}: seq={} par={}",
            s.path, s.hash, p.hash
        );
        assert_eq!(s.stage, p.stage);
        assert_eq!(s.stub, p.stub);
    }

    // Every emitted file MUST match byte-for-byte.
    let seq_files = collect_files_relative(out_seq.path());
    let par_files = collect_files_relative(out_par.path());
    assert_eq!(seq_files.len(), par_files.len(), "file count differs");
    for (s, p) in seq_files.iter().zip(&par_files) {
        assert_eq!(s.0, p.0, "different paths");
        if s.0 == ".rlvgl-app-manifest.json" {
            // The inventory file may differ in `generated_at`
            // (different second). We've already checked entries
            // above; skip exact-byte comparison here.
            continue;
        }
        assert_eq!(s.1, p.1, "byte mismatch in {}", s.0);
    }
}

// ─── jobs=N for various N produces identical output to N=1 ──────────

#[test]
fn jobs_2_3_5_match_jobs_1() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture_sm_crate(tmp.path(), &["idle"]);
    let manifest = write_full_manifest(tmp.path());

    let (_out1, inv1) = run_with_jobs(&manifest, tmp.path(), 1);
    let baseline_hashes: Vec<String> = inv1.entries.iter().map(|e| e.hash.clone()).collect();
    let baseline_paths: Vec<String> = inv1.entries.iter().map(|e| e.path.clone()).collect();

    for n in [2, 3, 5, 8] {
        let (_out, inv) = run_with_jobs(&manifest, tmp.path(), n);
        let paths: Vec<String> = inv.entries.iter().map(|e| e.path.clone()).collect();
        let hashes: Vec<String> = inv.entries.iter().map(|e| e.hash.clone()).collect();
        assert_eq!(paths, baseline_paths, "jobs={n}: path order drifted");
        assert_eq!(hashes, baseline_hashes, "jobs={n}: hash drift");
    }
}

// ─── CV-1 still fires under parallel dispatch ───────────────────────

#[test]
fn parallel_cv1_rejects_unknown_state() {
    let tmp = tempfile::tempdir().unwrap();
    // SM emits state_set without "settings".
    write_fixture_sm_crate(tmp.path(), &["idle", "menu"]);
    // Manifest references a screen with state="settings".
    let manifest_body = r#"schema: rlvgl-app/v0
name: parallel-cv1-test

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

state_machine:
  source: states/main.scxml
  generator: mcp-statechart
  vendored_crate: sm-crate

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    state: settings
"#;
    fs::create_dir_all(tmp.path().join("states")).unwrap();
    fs::write(tmp.path().join("states/main.scxml"), "<scxml/>").unwrap();
    fs::create_dir_all(tmp.path().join("layouts")).unwrap();
    fs::write(tmp.path().join("layouts/home.rs"), "// layout\n").unwrap();
    let manifest = tmp.path().join("app.yaml");
    fs::write(&manifest, manifest_body).unwrap();

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable)
    .with_jobs(4);
    let err = orch
        .run()
        .expect_err("CV-1 must reject under parallel dispatch")
        .to_string();
    assert!(
        err.contains("CV-1") && err.contains("settings"),
        "got: {err}"
    );
}

// ─── Parallel error propagation: sub-gen failure surfaces ───────────

#[test]
fn parallel_dispatch_propagates_sub_gen_errors() {
    let tmp = tempfile::tempdir().unwrap();
    // No SM crate at all — emit_sm_vendored will fail finding the
    // self-manifest, but the validator will catch this earlier
    // with rule 5. So we test a different error: invalid theme hex.
    write_fixture_sm_crate(tmp.path(), &["idle"]);
    fs::create_dir_all(tmp.path().join("states")).unwrap();
    fs::write(tmp.path().join("states/main.scxml"), "<scxml/>").unwrap();
    fs::create_dir_all(tmp.path().join("layouts")).unwrap();
    fs::write(tmp.path().join("layouts/home.rs"), "// layout\n").unwrap();
    fs::write(
        tmp.path().join("theme.json"),
        r#"{"colors": {"primary": "not-a-color"}}"#,
    )
    .unwrap();

    let manifest_body = r#"schema: rlvgl-app/v0
name: parallel-error-test

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

state_machine:
  source: states/main.scxml
  generator: mcp-statechart
  vendored_crate: sm-crate

theme:
  source: theme.json
  format: chakra_tokens_v1

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    state: idle
"#;
    let manifest = tmp.path().join("app.yaml");
    fs::write(&manifest, manifest_body).unwrap();

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable)
    .with_jobs(4);
    let err = orch
        .run()
        .expect_err("invalid theme hex must propagate from parallel dispatch")
        .to_string();
    // The bad-hex error message must reach the caller even though
    // the failure happened inside a worker thread.
    assert!(err.contains("primary"), "got: {err}");
}
