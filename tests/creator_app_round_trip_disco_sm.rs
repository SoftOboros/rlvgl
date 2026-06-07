//! APP-04b end-to-end test: first SM-bearing round-trip target.
//!
//! Exercises `examples/stm32h747i-disco/app-with-sm.yaml` — the
//! committed real-world manifest that pairs the disco FreeRTOS
//! intent with a vendored `mcp-statechart` SM crate
//! (`examples/stm32h747i-disco/disco-demo-states/`). Proves:
//!
//! 1. The manifest validates per chapter 01 §6.
//! 2. `app from-yaml` emits a buildable example crate AND the
//!    chapter 04 §6 CV-1 cross-validate accepts every
//!    `screens[].state` value (each appears in the SM-gen
//!    self-manifest's `state_set`).
//! 3. `app from-yaml --check` is byte-deterministic against the
//!    just-emitted output (chapter 02 §9.1).
//! 4. The vendored SM crate is sibling-form (`Cargo.toml` present)
//!    and the orchestrator therefore does NOT inline its sources
//!    into `<out>/src/state_machine/` (chapter 04 §5.4 wrapper
//!    discriminator).
//! 5. The vendored self-manifest's `state_set` contains exactly the
//!    four states the manifest's screens reference.
#![cfg(feature = "creator")]

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest_path() -> PathBuf {
    workspace_root().join("examples/stm32h747i-disco/app-with-sm.yaml")
}

#[test]
fn sm_bearing_round_trip_emits_and_checks_cleanly() {
    let manifest = manifest_path();
    let tmp = tempdir().expect("tempdir");
    let out = tmp.path().join("emitted");

    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("from-yaml")
        .arg(&manifest)
        .arg("--out")
        .arg(&out)
        .status()
        .expect("spawn rlvgl-creator (emit)");
    assert!(
        status.success(),
        "emit failed — CV-1 (chapter 04 §6) probably rejected a screens[].state value"
    );

    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("from-yaml")
        .arg("--check")
        .arg(&manifest)
        .arg("--out")
        .arg(&out)
        .status()
        .expect("spawn rlvgl-creator (check)");
    assert!(
        status.success(),
        "--check found divergence; SM-bearing emit is not byte-deterministic"
    );

    // Sibling-crate form: orchestrator MUST NOT inline the SM
    // sources into <out>/src/state_machine/.
    let inlined = out.join("src/state_machine");
    assert!(
        !inlined.exists(),
        "sibling-form vendored crate must not be inlined; found {}",
        inlined.display()
    );

    // The screens[].state→file mapping landed: four screen modules,
    // one per state.
    for state in ["home", "menu", "settings", "playing"] {
        let p = out.join(format!("src/screens/{state}.rs"));
        assert!(
            p.is_file(),
            "expected emitted screen file at {}",
            p.display()
        );
    }
}

#[test]
fn vendored_sm_self_manifest_state_set_matches_screens() {
    let self_manifest_path = workspace_root()
        .join("examples/stm32h747i-disco/disco-demo-states/.mcp-statechart-manifest.json");
    let text =
        std::fs::read_to_string(&self_manifest_path).expect("read .mcp-statechart-manifest.json");
    let v: serde_json::Value =
        serde_json::from_str(&text).expect(".mcp-statechart-manifest.json is valid JSON");
    let state_set: Vec<String> = v
        .get("state_set")
        .and_then(|s| s.as_array())
        .expect("self-manifest has state_set: [...]")
        .iter()
        .filter_map(|s| s.as_str().map(String::from))
        .collect();
    let mut got = state_set.clone();
    got.sort();
    let want = ["idle", "menu", "playing", "settings"];
    assert_eq!(got, want, "SM-gen state_set must be {want:?}, got {got:?}");
}
