//! APP-05 — Cargo `[features]` graph + `[dependencies]` parity tests.
//!
//! Each test emits a round-trip target's `Cargo.toml` from its
//! `app.yaml` and asserts the per-prong feature-graph template
//! produces output that aligns with the existing hand-written
//! reference Cargo.toml for that example. Acceptance shape per
//! `docs/app-schema/APP-05-A.md` §8:
//!
//! - For each manifest feature in `target.features`, the emitted
//!   expansion equals the reference's expansion (set-equal,
//!   ordering tolerated).
//! - The emitted `[dependencies]` set is a subset of the reference
//!   `[dependencies]` (by dep name).
//!
//! Cross-reference: chapter 02 §8 preamble (frozen rule), §15
//! 2026-04-29 RATIFIED entry naming APP-05+ as the family.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use app::Orchestrator;

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

/// Extract the body of a `[<header>]` section from a Cargo.toml,
/// stopping at the next `[...]` line or EOF. The body retains its
/// blank lines and comments verbatim. Section header line itself is
/// not included.
fn section_body<'a>(text: &'a str, header: &str) -> Option<&'a str> {
    let needle = format!("[{header}]");
    let start = text.find(&needle)?;
    // Move to start of line containing the header.
    let after_header = &text[start + needle.len()..];
    // Skip the rest of the header line (newline).
    let body_start_off = after_header.find('\n').map(|i| i + 1).unwrap_or(0);
    let body = &after_header[body_start_off..];
    // Find the next `[` that starts a line (next section).
    let mut end = body.len();
    for (idx, line) in body.lines().enumerate() {
        if idx == 0 {
            continue;
        }
        if line.trim_start().starts_with('[') {
            // Locate the byte offset of this line.
            let mut byte_off = 0usize;
            for (i, l) in body.lines().enumerate() {
                if i == idx {
                    end = byte_off;
                    break;
                }
                byte_off += l.len() + 1; // +1 for the \n
            }
            break;
        }
    }
    Some(&body[..end])
}

/// Parse a `[features]` section body into a map of feature name →
/// set of expansion strings. Lines like `default = [...]` are kept
/// under the `default` key. Skips blank lines and comments.
fn parse_features_block(body: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some(eq) = t.find('=') else {
            continue;
        };
        let name = t[..eq].trim().to_string();
        let rhs = t[eq + 1..].trim();
        // rhs is a [..] list; tolerate a single string fallback.
        let mut entries = BTreeSet::new();
        let inner = rhs
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(rhs);
        for piece in inner.split(',') {
            let p = piece.trim().trim_matches('"').trim();
            if !p.is_empty() {
                entries.insert(p.to_string());
            }
        }
        out.insert(name, entries);
    }
    out
}

/// Parse a `[dependencies]` (or `[build-dependencies]` /
/// `[target.cfg.dependencies]`) section body into the set of dep
/// names that appear on the LHS. The RHS is intentionally NOT
/// parsed — APP-05 acceptance only requires set-subset by name at
/// v0; per-dep source-kind/version reconciliation is not in scope.
fn parse_dep_names(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(eq) = t.find('=') {
            out.insert(t[..eq].trim().to_string());
        }
    }
    out
}

// ─── APP-05a: BBB Linux feature graph ──────────────────────────────────

#[test]
fn app_05a_bbb_linux_feature_graph_matches_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference =
        fs::read_to_string(ws.join("examples/beaglebone-black/Cargo.toml")).expect("reference");

    let emitted_features = parse_features_block(
        section_body(&emitted, "features").expect("emitted has [features] section"),
    );
    let reference_features = parse_features_block(
        section_body(&reference, "features").expect("reference has [features] section"),
    );

    // Every feature the manifest declares (target.features) must
    // appear in the emitted [features] block with an expansion that
    // is set-equal to the reference's.
    let manifest_feats = ["linux", "splash", "desktop", "playit", "star_crawl"];
    for feat in manifest_feats {
        let emitted_exp = emitted_features
            .get(feat)
            .unwrap_or_else(|| panic!("emitted [features] missing {feat}"));
        let reference_exp = reference_features
            .get(feat)
            .unwrap_or_else(|| panic!("reference [features] missing {feat}"));
        assert_eq!(
            emitted_exp, reference_exp,
            "feature `{feat}` expansion mismatch:\n  emitted: {emitted_exp:?}\n  reference: {reference_exp:?}"
        );
    }

    // Default policy: BBB reference has the same default set as the
    // manifest features (AllManifestFeatures policy in the template).
    let emitted_default = emitted_features
        .get("default")
        .expect("emitted has default = ...");
    let reference_default = reference_features
        .get("default")
        .expect("reference has default = ...");
    assert_eq!(emitted_default, reference_default, "default = ... mismatch");
}

#[test]
fn app_05a_bbb_linux_dependencies_subset_of_reference() {
    let ws = workspace_root();
    let tmp = emit_to_tempdir("examples/beaglebone-black/app.yaml");
    let emitted = fs::read_to_string(tmp.path().join("Cargo.toml")).expect("emitted Cargo.toml");
    let reference =
        fs::read_to_string(ws.join("examples/beaglebone-black/Cargo.toml")).expect("reference");

    let emitted_deps = parse_dep_names(
        section_body(&emitted, "dependencies").expect("emitted has [dependencies] section"),
    );
    let reference_deps = parse_dep_names(
        section_body(&reference, "dependencies").expect("reference has [dependencies] section"),
    );

    // Every emitted dep must exist in the reference.
    let extra: Vec<_> = emitted_deps.difference(&reference_deps).collect();
    assert!(
        extra.is_empty(),
        "emitted [dependencies] introduces deps not in reference: {extra:?}"
    );

    // Specific dep-name expectations from the BBB template.
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

    // [build-dependencies] also expected.
    let emitted_build = parse_dep_names(
        section_body(&emitted, "build-dependencies")
            .expect("emitted has [build-dependencies] section"),
    );
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
