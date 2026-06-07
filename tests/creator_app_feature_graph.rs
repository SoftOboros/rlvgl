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

// ─── APP-05c: beetle bsp_pac creator-bsp-pac feature graph ─────────────

#[test]
fn app_05c_beetle_bsp_pac_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beetle-esp32c3/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    let emitted_exp =
        feature_expansion(&emitted, "bsp_pac").expect("emitted [features] missing bsp_pac");
    let reference_exp =
        feature_expansion(&reference, "bsp_pac").expect("reference [features] missing bsp_pac");
    assert_eq!(
        emitted_exp, reference_exp,
        "feature `bsp_pac` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
    );

    let emitted_default = feature_expansion(&emitted, "default").expect("emitted default");
    assert!(
        emitted_default.is_empty(),
        "expected default = [] for beetle bsp_pac, got {emitted_default:?}"
    );
}

#[test]
fn app_05c_beetle_bsp_pac_dependencies_subset_of_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/beetle-esp32c3/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    // bsp_pac is headless — no rlvgl runtime base deps expected.
    // The emit's [dependencies] block will contain at most the
    // controller (none in this manifest) so it's empty.
    let emitted_deps = dep_names(&emitted, "dependencies");
    assert!(
        emitted_deps.is_empty(),
        "bsp_pac is headless; emitted [dependencies] expected empty, got {emitted_deps:?}"
    );

    // [target.cfg(target_arch = "riscv32").dependencies] entries
    // gated by bsp_pac.
    let emitted_cfg_deps = target_cfg_dep_names(&emitted);
    let reference_cfg_deps = target_cfg_dep_names(&reference);
    let cfg_extra: Vec<_> = emitted_cfg_deps.difference(&reference_cfg_deps).collect();
    assert!(
        cfg_extra.is_empty(),
        "emitted [target.cfg.dependencies] introduces deps not in reference: {cfg_extra:?}"
    );
    for required in [
        "esp32c3",
        "esp-riscv-rt",
        "riscv-rt",
        "riscv",
        "panic-halt",
    ] {
        assert!(
            emitted_cfg_deps.contains(required),
            "emitted [target.cfg.dependencies] missing `{required}` (set: {emitted_cfg_deps:?})"
        );
    }
}

#[test]
fn app_05c_beetle_bsp_pac_no_template_tuning_todo() {
    let tmp = emit_to_tempdir("examples/beetle-esp32c3/app-bsp-pac.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    assert!(
        !emitted.contains("TODO(template-tuning)"),
        "beetle bsp_pac emit still carries TODO(template-tuning)"
    );
}

// ─── APP-05d: STM32H747I-DISCO freertos feature graph ──────────────────

const H747_FREERTOS_FEATURES: &[&str] =
    &["cm7", "freertos", "adapted_cmd", "dma2d", "splash", "desktop"];

fn assert_h747_freertos_features(emitted: &Table, reference: &Table) {
    for feat in H747_FREERTOS_FEATURES {
        let emitted_exp = feature_expansion(emitted, feat)
            .unwrap_or_else(|| panic!("emitted [features] missing {feat}"));
        let reference_exp = feature_expansion(reference, feat)
            .unwrap_or_else(|| panic!("reference [features] missing {feat}"));
        assert_eq!(
            emitted_exp, reference_exp,
            "feature `{feat}` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
        );
    }
    let emitted_default = feature_expansion(emitted, "default").expect("emitted default");
    assert!(
        emitted_default.is_empty(),
        "expected default = [] for H747 freertos, got {emitted_default:?}"
    );
}

fn assert_h747_freertos_deps(emitted: &Table, reference: &Table) {
    let emitted_deps = dep_names(emitted, "dependencies");
    let reference_deps = dep_names(reference, "dependencies");
    let extra: Vec<_> = emitted_deps.difference(&reference_deps).collect();
    assert!(
        extra.is_empty(),
        "emitted [dependencies] introduces deps not in reference: {extra:?}"
    );
    for required in [
        "rlvgl-core",
        "rlvgl-platform",
        "rlvgl-widgets",
        "rlvgl-ui",
        "rlvgl-i18n",
        "rlvgl-decomp",
        "rlvgl-playit",
        "rlvgl-app-disco-demo", // controller
        "cortex-m-rt",
        "cortex-m",
        "embedded-alloc",
        "panic-halt",
        "stm32h7",
        "critical-section",
    ] {
        assert!(
            emitted_deps.contains(required),
            "emitted [dependencies] missing `{required}` (set: {emitted_deps:?})"
        );
    }

    let emitted_cfg_deps = target_cfg_dep_names(emitted);
    let reference_cfg_deps = target_cfg_dep_names(reference);
    let cfg_extra: Vec<_> = emitted_cfg_deps.difference(&reference_cfg_deps).collect();
    assert!(
        cfg_extra.is_empty(),
        "emitted [target.cfg.dependencies] introduces deps not in reference: {cfg_extra:?}"
    );
    for required in [
        "stm32h7xx-hal",
        "embedded-hal",
        "embedded-hal-02",
        "embedded-sdmmc",
    ] {
        assert!(
            emitted_cfg_deps.contains(required),
            "emitted [target.cfg.dependencies] missing `{required}` (set: {emitted_cfg_deps:?})"
        );
    }
}

#[test]
fn app_05d_h747_freertos_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/stm32h747i-disco/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);
    assert_h747_freertos_features(&emitted, &reference);
}

#[test]
fn app_05d_h747_freertos_dependencies_subset_of_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/stm32h747i-disco/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);
    assert_h747_freertos_deps(&emitted, &reference);
}

#[test]
fn app_05d_h747_freertos_with_sm_shares_template() {
    // Both freertos manifests (app.yaml + app-with-sm.yaml) carry
    // the same target.features set and therefore must produce the
    // same feature-graph + dependencies shape from the same template.
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app-with-sm.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/stm32h747i-disco/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);
    assert_h747_freertos_features(&emitted, &reference);
    assert_h747_freertos_deps(&emitted, &reference);
}

#[test]
fn app_05d_h747_freertos_no_template_tuning_todo() {
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    assert!(
        !emitted.contains("TODO(template-tuning)"),
        "H747 freertos emit still carries TODO(template-tuning)"
    );
}

// ─── APP-05e: STM32H747I-DISCO zephyr feature graph ────────────────────

const H747_ZEPHYR_FEATURES: &[&str] = &["cm7", "zephyr", "splash", "desktop", "dma2d"];

#[test]
fn app_05e_h747_zephyr_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/stm32h747i-disco/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);

    for feat in H747_ZEPHYR_FEATURES {
        let emitted_exp = feature_expansion(&emitted, feat)
            .unwrap_or_else(|| panic!("emitted [features] missing {feat}"));
        let reference_exp = feature_expansion(&reference, feat)
            .unwrap_or_else(|| panic!("reference [features] missing {feat}"));
        assert_eq!(
            emitted_exp, reference_exp,
            "feature `{feat}` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
        );
    }
    let emitted_default = feature_expansion(&emitted, "default").expect("emitted default");
    assert!(
        emitted_default.is_empty(),
        "expected default = [] for H747 zephyr, got {emitted_default:?}"
    );
}

#[test]
fn app_05e_h747_zephyr_dependencies_subset_of_reference() {
    // Same H747 base + cross-compile deps as APP-05d (shared
    // statics in feature_graphs.rs).
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference_text =
        fs::read_to_string(ws.join("examples/stm32h747i-disco/Cargo.toml")).expect("reference");
    let emitted = parse_cargo_toml(&emitted_text);
    let reference = parse_cargo_toml(&reference_text);
    assert_h747_freertos_deps(&emitted, &reference);
}

#[test]
fn app_05e_h747_zephyr_emits_staticlib_lib_section() {
    // The zephyr prong's Rust side is a staticlib (chapter 02 §5.4.1).
    // Already handled by `emit_cargo_toml` pre-APP-05; assert the
    // template integration didn't regress that.
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let emitted_text =
        fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let emitted = parse_cargo_toml(&emitted_text);
    let lib = emitted
        .get("lib")
        .and_then(toml::Value::as_table)
        .expect("[lib] section present for zephyr prong");
    let crate_type = lib
        .get("crate-type")
        .and_then(toml::Value::as_array)
        .expect("[lib].crate-type is an array");
    assert!(
        crate_type
            .iter()
            .any(|v| v.as_str() == Some("staticlib")),
        "[lib].crate-type missing `staticlib` for zephyr prong: {crate_type:?}"
    );
}

#[test]
fn app_05e_h747_zephyr_no_template_tuning_todo() {
    let tmp = emit_to_tempdir("examples/stm32h747i-disco/app-zephyr.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    assert!(
        !emitted.contains("TODO(template-tuning)"),
        "H747 zephyr emit still carries TODO(template-tuning)"
    );
}

// ─── APP-05f: discipline scanner — round-trip cross-cut ────────────────

/// Every round-trip target manifest committed under `examples/`.
/// Per chapter 03 + APP-05-A §6, this set is closed at v0; adding
/// a new manifest requires both a §15 amendment to chapter 03 and
/// a corresponding APP-05x feature-graph template registration in
/// `src/bin/creator/app/feature_graphs.rs`.
const ROUND_TRIP_MANIFESTS: &[&str] = &[
    "examples/beaglebone-black/app.yaml",
    "examples/beetle-esp32c3/app.yaml",
    "examples/beetle-esp32c3/app-bsp-pac.yaml",
    "examples/stm32h747i-disco/app.yaml",
    "examples/stm32h747i-disco/app-with-sm.yaml",
    "examples/stm32h747i-disco/app-zephyr.yaml",
];

#[test]
fn app_05f_every_round_trip_manifest_has_a_template() {
    // Every committed manifest's (prong, generator, vendor, board)
    // tuple MUST resolve to a feature-graph template, and every
    // feature in the manifest's target.features MUST appear in
    // that template's `feature_expansions` table. This is the
    // initiative-wide acceptance gate per APP-05-A §8.
    let ws = workspace_root();
    let mut failures: Vec<String> = Vec::new();
    for rel in ROUND_TRIP_MANIFESTS {
        let manifest = match app::validate(&ws.join(rel)) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{rel}: validate failed: {e}"));
                continue;
            }
        };
        let prong = manifest.target.prong.as_str();
        let generator = manifest
            .target
            .generator
            .as_deref()
            .unwrap_or("creator-bsp-pac");
        let vendor = manifest.target.vendor.as_str();
        let board = manifest.target.board.as_str();

        let Some(template) = app::feature_graphs::lookup(prong, generator, vendor, board) else {
            failures.push(format!(
                "{rel}: no APP-05x template registered for (prong={prong}, generator={generator}, vendor={vendor}, board={board})"
            ));
            continue;
        };
        for feat in &manifest.target.features {
            let in_table = template
                .feature_expansions
                .iter()
                .any(|(k, _)| k == feat)
                || template.extra_features.iter().any(|(k, _)| k == feat);
            if !in_table {
                failures.push(format!(
                    "{rel}: target.features `{feat}` missing from template feature_expansions"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "APP-05f discipline scan: {} failure(s):\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

#[test]
fn app_05f_no_emitted_cargo_toml_carries_template_tuning_marker() {
    // Aggregate of the per-phase no_template_tuning_todo tests.
    // Emits every committed round-trip manifest and confirms none
    // of the emitted Cargo.tomls contain the placeholder marker.
    let mut offenders: Vec<&str> = Vec::new();
    for rel in ROUND_TRIP_MANIFESTS {
        let tmp = emit_to_tempdir(rel);
        let emitted =
            fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
        if emitted.contains("TODO(template-tuning)") {
            offenders.push(rel);
        }
    }
    assert!(
        offenders.is_empty(),
        "APP-05f: emitted Cargo.toml(s) carry TODO(template-tuning) marker: {offenders:?}"
    );
}
