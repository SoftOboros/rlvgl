//! Tests for Svelte token and source validation.
#![cfg(feature = "creator")]
// The `#[path]` include below pulls in source modules from the
// `rlvgl-creator` binary; only a subset of their public surface is
// exercised by this test, so the unused helpers are expected.
#![allow(dead_code, clippy::too_many_arguments)]

#[path = "../src/bin/creator/svelte.rs"]
mod svelte;

use std::path::{Path, PathBuf};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/svelte")
        .join(path)
}

#[test]
fn svelte_check_accepts_valid_tokens_and_component() {
    let tokens = fixture("tokens-valid.yaml");
    let component = fixture("valid/button.svelte");
    svelte::check(&component, Some(&tokens), None).unwrap();
}

#[test]
fn svelte_check_rejects_raw_html() {
    let tokens = fixture("tokens-valid.yaml");
    let component = fixture("invalid/raw_html.svelte");
    let err = svelte::check(&component, Some(&tokens), None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("raw HTML tag 'div'"));
}

#[test]
fn svelte_check_rejects_await_block() {
    let tokens = fixture("tokens-valid.yaml");
    let component = fixture("invalid/await.svelte");
    let err = svelte::check(&component, Some(&tokens), None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("{#await}"));
}

#[test]
fn svelte_check_rejects_named_slots() {
    let tokens = fixture("tokens-valid.yaml");
    let component = fixture("invalid/named_slot.svelte");
    let err = svelte::check(&component, Some(&tokens), None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("named slots are not allowed"));
}

#[test]
fn svelte_check_rejects_invalid_tokens() {
    let tokens = fixture("tokens-invalid.yaml");
    let err = svelte::check(&tokens, None, None).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("token key 'Primary'"));
}

#[test]
fn svelte_check_rejects_unknown_mode() {
    let tokens = fixture("tokens-modes.yaml");
    let err = svelte::check(&tokens, None, Some("missing")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown token mode"));
}
