//! APP-02f integration tests: real i18n generator per chapter 02
//! §7.5. Covers happy-path two-locale match table, missing-key
//! warnings (soft per §7.5), and byte-determinism across runs.
//!
//! Tests construct fixture i18n bundles inside a tempdir; the
//! existing rlvgl `i18n/locales/*.json` content shape is preserved
//! (flat `{"key":"value"}` JSON map per chapter 01 §5.8
//! `rlvgl_i18n_v1`).
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

fn write_bundle(dir: &Path, locale: &str, kv: &[(&str, &str)]) {
    let body = {
        let mut entries: Vec<String> = kv
            .iter()
            .map(|(k, v)| format!("  {:?}: {:?}", k, v))
            .collect();
        entries.sort();
        format!("{{\n{}\n}}\n", entries.join(",\n"))
    };
    fs::write(dir.join(format!("{locale}.json")), body).unwrap();
}

fn write_manifest(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("locales")).unwrap();
    fs::create_dir_all(root.join("layouts")).unwrap();
    fs::write(root.join("layouts/home.rs"), "// layout\n").unwrap();
    let body = r#"schema: rlvgl-app/v0
name: i18n-fixture-app

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hosted

i18n:
  bundle_dir: locales
  default_locale: en
  format: rlvgl_i18n_v1

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true
"#;
    let path = root.join("app.yaml");
    fs::write(&path, body).unwrap();
    path
}

fn run_orchestrator(manifest: &Path, ws_root: &Path) -> (tempfile::TempDir, app::Inventory) {
    let m = app::validate(manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        ws_root.to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let inv = orch.run().expect("orchestrator runs");
    (out, inv)
}

// ─── Happy path: two locales emit (locale, key) match arms ───────────

#[test]
fn i18n_emits_match_table_over_locale_and_key() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    let bundle_dir = manifest.parent().unwrap().join("locales");
    write_bundle(
        &bundle_dir,
        "en",
        &[
            ("demo.title", "rlvgl Demo v{version}"),
            ("hw.touch", "Touch: ({x}, {y})"),
        ],
    );
    write_bundle(
        &bundle_dir,
        "fr",
        &[
            ("demo.title", "Démo rlvgl v{version}"),
            ("hw.touch", "Tact : ({x}, {y})"),
        ],
    );

    let (out, inv) = run_orchestrator(&manifest, tmp.path());

    let entry = inv
        .entries
        .iter()
        .find(|e| e.path == "src/i18n_generated.rs")
        .expect("inventory entry for i18n_generated.rs");
    assert_eq!(entry.stage, "i18n");
    assert!(!entry.stub, "real i18n generator must not flag stub");
    assert!(entry.hash.starts_with("blake3:"));

    let body = fs::read_to_string(out.path().join("src/i18n_generated.rs")).unwrap();
    assert!(
        body.contains("pub const DEFAULT_LOCALE: &str = \"en\";"),
        "got:\n{body}"
    );
    assert!(
        body.contains("pub fn t(key: &str, locale: &str) -> &'static str"),
        "got:\n{body}"
    );
    assert!(body.contains("match (locale, key) {"), "got:\n{body}");
    assert!(
        body.contains(r#"("en", "demo.title") => "rlvgl Demo v{version}","#),
        "got:\n{body}"
    );
    assert!(
        body.contains(r#"("fr", "demo.title") => "Démo rlvgl v{version}","#),
        "got:\n{body}"
    );
    assert!(
        body.contains(r#"("en", "hw.touch") => "Touch: ({x}, {y})","#),
        "got:\n{body}"
    );
    assert!(
        body.contains("_ => key,"),
        "fallthrough preserved; got:\n{body}"
    );
}

// ─── Missing-key warning is soft, not fatal ──────────────────────────

#[test]
fn missing_key_in_one_locale_emits_warning_not_error() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    let bundle_dir = manifest.parent().unwrap().join("locales");
    write_bundle(
        &bundle_dir,
        "en",
        &[("demo.title", "Hello"), ("hw.touch", "Touch")],
    );
    // 'fr' is missing 'hw.touch'.
    write_bundle(&bundle_dir, "fr", &[("demo.title", "Bonjour")]);

    // Run must succeed (soft warning, not error).
    let (out, _inv) = run_orchestrator(&manifest, tmp.path());
    let body = fs::read_to_string(out.path().join("src/i18n_generated.rs")).unwrap();

    // 'en' has both arms; 'fr' has only the title arm; falling through
    // to `_ => key` for hw.touch under fr is acceptable per §7.5.
    assert!(
        body.contains(r#"("en", "hw.touch") => "Touch","#),
        "got:\n{body}"
    );
    assert!(
        !body.contains(r#"("fr", "hw.touch") =>"#),
        "missing key MUST NOT be invented; got:\n{body}"
    );
}

// ─── Empty bundle dir is rejected hard ───────────────────────────────

#[test]
fn empty_bundle_dir_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    fs::create_dir_all(manifest.parent().unwrap().join("locales")).unwrap();
    // No <locale>.json files.

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("empty bundle dir must be rejected")
        .to_string();
    assert!(err.contains("no <locale>.json"), "got: {err}");
}

// ─── Non-string value in bundle is rejected hard ─────────────────────

#[test]
fn non_string_bundle_value_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    let bundle_dir = manifest.parent().unwrap().join("locales");
    fs::create_dir_all(&bundle_dir).unwrap();
    fs::write(
        bundle_dir.join("en.json"),
        r#"{ "demo.count": 42 }
"#,
    )
    .unwrap();

    let m = app::validate(&manifest).expect("manifest validates");
    let out = tempfile::tempdir().unwrap();
    let mut orch = app::Orchestrator::new(
        m,
        manifest.parent().unwrap().to_path_buf(),
        tmp.path().to_path_buf(),
        out.path().to_path_buf(),
    )
    .with_bsp_gen(bsp_gen_unreachable);
    let err = orch
        .run()
        .expect_err("non-string value must be rejected")
        .to_string();
    assert!(
        err.contains("not a string") && err.contains("demo.count"),
        "got: {err}"
    );
}

// ─── Determinism: i18n emission is byte-identical across runs ────────

#[test]
fn i18n_emit_is_byte_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    let bundle_dir = manifest.parent().unwrap().join("locales");
    write_bundle(&bundle_dir, "en", &[("a.one", "1"), ("b.two", "2")]);
    write_bundle(&bundle_dir, "fr", &[("a.one", "un"), ("b.two", "deux")]);

    let (out1, inv1) = run_orchestrator(&manifest, tmp.path());
    let (out2, inv2) = run_orchestrator(&manifest, tmp.path());

    let body1 = fs::read_to_string(out1.path().join("src/i18n_generated.rs")).unwrap();
    let body2 = fs::read_to_string(out2.path().join("src/i18n_generated.rs")).unwrap();
    assert_eq!(body1, body2, "i18n_generated.rs drifts across runs");

    let h1 = inv1
        .entries
        .iter()
        .find(|e| e.path == "src/i18n_generated.rs")
        .unwrap()
        .hash
        .clone();
    let h2 = inv2
        .entries
        .iter()
        .find(|e| e.path == "src/i18n_generated.rs")
        .unwrap()
        .hash
        .clone();
    assert_eq!(h1, h2, "inventory hash drifts across runs");
}

// ─── Real bundle from the rlvgl repo's i18n/locales/ shape works ─────

#[test]
fn rlvgl_i18n_locales_shape_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = write_manifest(tmp.path());
    let bundle_dir = manifest.parent().unwrap().join("locales");

    // Mirror the real rlvgl i18n/locales/en.json shape — dotted-lowercase
    // keys, format-string-style placeholders, several namespaces.
    write_bundle(
        &bundle_dir,
        "en",
        &[
            ("demo.title", "rlvgl Demo v{version}"),
            ("hw.btn_press", "Btn: Press"),
            ("hw.cm4_waiting", "CM4: waiting"),
            ("hw.touch", "Touch: ({x}, {y})"),
        ],
    );

    let (out, _inv) = run_orchestrator(&manifest, tmp.path());
    let body = fs::read_to_string(out.path().join("src/i18n_generated.rs")).unwrap();
    assert!(body.contains(r#"("en", "demo.title") => "rlvgl Demo v{version}","#));
    assert!(body.contains(r#"("en", "hw.btn_press") => "Btn: Press","#));
    assert!(body.contains(r#"("en", "hw.cm4_waiting") => "CM4: waiting","#));
}
