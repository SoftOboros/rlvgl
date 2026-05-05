//! APP-05 — Cargo `[features]` graph + `[dependencies]` parity tests.
//!
//! Each test emits a round-trip target's `Cargo.toml` from its
//! `app.yaml` and asserts the per-prong feature-graph template
//! produces output that aligns with the existing hand-written
//! reference Cargo.toml for that example. Acceptance shape per
//! `docs/app-schema/APP-05-A.md` §8:
//!
//! - For each manifest feature in `target.features`, the emitted
//!   expansion is set-equal to the reference's expansion.
//! - The emitted `[dependencies]` set is a subset of the reference
//!   `[dependencies]` (by dep name).
//!
//! Cross-reference: chapter 02 §8 preamble (frozen rule), §15
//! 2026-04-29 RATIFIED entry naming APP-05+ as the family.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use app::Orchestrator;
use toml::{Table, Value};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn emit_to_tempdir(manifest_rel: &str) -> tempfile::TempDir {
    let ws = workspace_root();
    let manifest_path = ws.join(manifest_rel);
    let m = app::validate(&manifest_path).expect("manifest validates");
    let manifest_dir = manifest_path.parent().unwrap().to_path_buf();
    let ws_root = app::find_workspace_root(&manifest_dir);
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut orch = Orchestrator::new(m, manifest_dir, ws_root, tmp.path().to_path_buf());
    orch.run().expect("orchestrator runs");
    tmp
}

fn parse_cargo_toml(text: &str) -> Table {
    text.parse::<Table>().expect("Cargo.toml parses as TOML")
}

/// Pull a feature's expansion list from a parsed Cargo.toml's
/// `[features]` table. Missing key returns an empty set.
fn feature_expansion(t: &Table, name: &str) -> Option<BTreeSet<String>> {
    let arr = t.get("features")?.as_table()?.get(name)?.as_array()?;
    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
}

/// Names of all keys under `[dependencies]` (or another dep section
/// when `header` is e.g. `"build-dependencies"`).
fn dep_names(t: &Table, header: &str) -> BTreeSet<String> {
    t.get(header)
        .and_then(|v| v.as_table())
        .map(|tbl| tbl.keys().cloned().collect())
        .unwrap_or_default()
}

/// Names of all keys under `[target.<cfg>.dependencies]` for any cfg.
fn target_cfg_dep_names(t: &Table) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(target) = t.get("target").and_then(Value::as_table) else {
        return out;
    };
    for (_cfg, body) in target {
        if let Some(deps) = body.get("dependencies").and_then(Value::as_table) {
            for k in deps.keys() {
                out.insert(k.clone());
            }
        }
    }
    out
}

// ─── APP-05a: BBB Linux feature graph ──────────────────────────────────

#[test]
fn app_05a_bbb_linux_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beaglebone-black/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    for feat in ["linux", "splash", "desktop", "playit", "star_crawl"] {
        let emitted_exp = feature_expansion(&emitted, feat)
            .unwrap_or_else(|| panic!("emitted [features] missing {feat}"));
        let reference_exp = feature_expansion(&reference, feat)
            .unwrap_or_else(|| panic!("reference [features] missing {feat}"));
        assert_eq!(
            emitted_exp, reference_exp,
            "feature `{feat}` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
        );
    }

    // Default policy: BBB reference has the same default set as the
    // manifest features (AllManifestFeatures policy in the template).
    let emitted_default = feature_expansion(&emitted, "default").expect("emitted default");
    let reference_default = feature_expansion(&reference, "default").expect("reference default");
    assert_eq!(emitted_default, reference_default, "default = ... mismatch");
}

#[test]
fn app_05a_bbb_linux_dependencies_subset_of_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beaglebone-black/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    let emitted_deps = dep_names(&emitted, "dependencies");
    let reference_deps = dep_names(&reference, "dependencies");
    let extra: Vec<_> = emitted_deps.difference(&reference_deps).collect();
    assert!(
        extra.is_empty(),
        "emitted [dependencies] introduces deps not in reference: {extra:?}"
    );
    for required in [
        "rlvgl-core",
        "rlvgl-platform",
        "rlvgl-app-disco-demo",
        "rlvgl-decomp",
        "rlvgl-playit",
        "rlvgl-widgets",
        "libc",
        "heapless",
    ] {
        assert!(
            emitted_deps.contains(required),
            "emitted [dependencies] missing `{required}` (emitted set: {emitted_deps:?})"
        );
    }

    let emitted_build = dep_names(&emitted, "build-dependencies");
    assert!(
        emitted_build.contains("cc"),
        "emitted [build-dependencies] missing `cc`"
    );
}

#[test]
fn app_05a_bbb_linux_no_template_tuning_todo() {
    // APP-05f preview: ensure the BBB emit has shed the placeholder
    // `TODO(template-tuning)` marker now that a template is wired.
    let tmp = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    assert!(
        !emitted.contains("TODO(template-tuning)"),
        "BBB linux emit still carries TODO(template-tuning); APP-05a template should have replaced it"
    );
}

// ─── APP-05b: beetle esp_hal hosted feature graph ──────────────────────

#[test]
fn app_05b_beetle_esp_hal_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beetle-esp32c3/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    let emitted_exp =
        feature_expansion(&emitted, "esp_hal").expect("emitted [features] missing esp_hal");
    let reference_exp =
        feature_expansion(&reference, "esp_hal").expect("reference [features] missing esp_hal");
    assert_eq!(
        emitted_exp, reference_exp,
        "feature `esp_hal` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
    );

    // Beetle reference has default = []; the emit must match.
    let emitted_default = feature_expansion(&emitted, "default").expect("emitted default");
    assert!(
        emitted_default.is_empty(),
        "expected default = [] for beetle esp_hal, got {emitted_default:?}"
    );
}

#[test]
fn app_05b_beetle_esp_hal_dependencies_subset_of_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beetle-esp32c3/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    let emitted_deps = dep_names(&emitted, "dependencies");
    let reference_deps = dep_names(&reference, "dependencies");
    let extra: Vec<_> = emitted_deps.difference(&reference_deps).collect();
    assert!(
        extra.is_empty(),
        "emitted [dependencies] introduces deps not in reference: {extra:?}"
    );
    for required in ["rlvgl-core", "rlvgl-platform", "rlvgl-widgets", "ssd1306"] {
        assert!(
            emitted_deps.contains(required),
            "emitted [dependencies] missing `{required}` (emitted set: {emitted_deps:?})"
        );
    }

    // [target.cfg(target_arch = "riscv32").dependencies] entries gated
    // by esp_hal.
    let emitted_cfg_deps = target_cfg_dep_names(&emitted);
    let reference_cfg_deps = target_cfg_dep_names(&reference);
    let cfg_extra: Vec<_> = emitted_cfg_deps.difference(&reference_cfg_deps).collect();
    assert!(
        cfg_extra.is_empty(),
        "emitted [target.cfg.dependencies] introduces deps not in reference: {cfg_extra:?}"
    );
    for required in ["esp-hal", "esp-backtrace", "esp-println", "esp-alloc"] {
        assert!(
            emitted_cfg_deps.contains(required),
            "emitted [target.cfg.dependencies] missing `{required}` (set: {emitted_cfg_deps:?})"
        );
    }
}

#[test]
fn app_05b_beetle_esp_hal_no_template_tuning_todo() {
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    assert!(
        !emitted.contains("TODO(template-tuning)"),
        "beetle esp_hal emit still carries TODO(template-tuning)"
    );
}
