//! APP-02e end-to-end test: real BSP-gen invocation through the
//! `rlvgl-creator app from-yaml` orchestrator.
//!
//! `tests/creator_app_emit.rs` includes `app.rs` directly via `#[path]`
//! and therefore can't reach the binary-private `bsp/` tree, so the
//! orchestrator's BSP-gen stage falls back to a stub there. This test
//! invokes the compiled `rlvgl-creator` binary as a subprocess so the
//! real `BspGenFn` (defined in `src/bin/rlvgl_creator/main.rs`) is
//! wired up. It exercises the chapter 02 §7.2 + §7.2.1 path end to
//! end against the `examples/beetle-esp32c3/app-bsp-pac.yaml`
//! manifest (the only ratified round-trip target with
//! `target.generator: creator-bsp-pac`).
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

#[test]
fn app_from_yaml_emits_real_chipdb_bsp_for_beetle_bsp_pac() {
    let manifest = workspace_root().join("examples/beetle-esp32c3/app-bsp-pac.yaml");
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
        .expect("spawn rlvgl-creator");
    assert!(
        status.success(),
        "rlvgl-creator app from-yaml exited non-zero"
    );

    // Six files in src/bsp_generated/, all real (not the stub fallback).
    let bsp_dir = out.join("src/bsp_generated");
    for name in [
        "mod.rs",
        "pac.rs",
        "clocks.rs",
        "io_mux.rs",
        "peripherals.rs",
        "board.rs",
    ] {
        let p = bsp_dir.join(name);
        assert!(
            p.is_file(),
            "expected real BSP-gen output at {}",
            p.display()
        );
    }

    // mod.rs is child-module-shaped per chapter 02 §7.2 — the
    // synthesised index, NOT the crate-root-shaped one BSP-gen
    // emits at its native top level.
    let mod_rs = std::fs::read_to_string(bsp_dir.join("mod.rs")).unwrap();
    assert!(
        mod_rs.contains("pub mod board;")
            && mod_rs.contains("pub mod clocks;")
            && mod_rs.contains("pub mod io_mux;")
            && mod_rs.contains("pub mod pac;")
            && mod_rs.contains("pub mod peripherals;"),
        "child-module mod.rs should re-export the five sibling files; got:\n{mod_rs}"
    );
    assert!(
        !mod_rs.contains("#![no_std]"),
        "child-module mod.rs must not carry inner crate-root attrs; got:\n{mod_rs}"
    );
    assert!(
        !mod_rs.contains("orchestrator stub"),
        "real callback path must not emit the stub fallback; got:\n{mod_rs}"
    );

    // pac.rs is the real chipdb-rendered file and references esp32c3.
    let pac_rs = std::fs::read_to_string(bsp_dir.join("pac.rs")).unwrap();
    assert!(
        pac_rs.contains("esp32c3"),
        "pac.rs from real BSP-gen should reference the esp32c3 PAC crate; got first 200:\n{}",
        &pac_rs.chars().take(200).collect::<String>()
    );

    // Inventory marks each emitted file as stage="bsp-gen", stub=false.
    let inv_path = out.join(".rlvgl-app-manifest.json");
    let inv_text = std::fs::read_to_string(&inv_path).expect("inventory at <out>");
    let inv: serde_json::Value = serde_json::from_str(&inv_text).expect("inventory parses");
    let entries = inv
        .get("entries")
        .and_then(|v| v.as_array())
        .expect("inventory has entries[]");
    let bsp_entries: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| {
            e.get("path")
                .and_then(|p| p.as_str())
                .is_some_and(|p| p.starts_with("src/bsp_generated/"))
        })
        .collect();
    assert_eq!(
        bsp_entries.len(),
        6,
        "expected 6 inventory entries under src/bsp_generated/; got {}",
        bsp_entries.len()
    );
    for e in &bsp_entries {
        assert_eq!(
            e.get("stage").and_then(|s| s.as_str()),
            Some("bsp-gen"),
            "every src/bsp_generated/ entry must be stage=bsp-gen; got {e:?}"
        );
        assert_eq!(
            e.get("stub").and_then(|s| s.as_bool()),
            Some(false),
            "real BSP-gen path must not flag entries as stubs; got {e:?}"
        );
        let hash = e.get("hash").and_then(|h| h.as_str()).unwrap_or_default();
        assert!(
            hash.starts_with("blake3:") && hash.len() > 64,
            "every entry must carry a blake3 hash; got '{hash}'"
        );
    }
}

/// `--check` against the just-emitted output is clean — proves the
/// real BSP-gen path is byte-deterministic between two independent
/// runs (chapter 02 §9.1).
#[test]
fn app_from_yaml_real_bsp_gen_is_byte_deterministic() {
    let manifest = workspace_root().join("examples/beetle-esp32c3/app-bsp-pac.yaml");
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
    assert!(status.success(), "first emit exited non-zero");

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
        "--check against emitted output must be clean; \
         BSP-gen output is non-deterministic across runs"
    );
}
