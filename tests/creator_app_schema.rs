//! APP-02i smoke test: `rlvgl-creator app schema` emits a JSON
//! Schema describing the rlvgl-app/v0 manifest grammar from
//! chapter 01 §5.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::path::PathBuf;
use std::process::Command;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

#[test]
fn app_schema_function_emits_valid_json_with_expected_shape() {
    let body = app::app_schema_json().expect("schema emits");
    let json: serde_json::Value = serde_json::from_str(&body).expect("schema parses as JSON");

    // Top-level shape: an object schema named Manifest with the
    // chapter 01 §5.1 fields.
    assert_eq!(
        json.get("title").and_then(|v| v.as_str()),
        Some("Manifest"),
        "schema root should be titled Manifest; got: {body}"
    );
    assert_eq!(json.get("type").and_then(|v| v.as_str()), Some("object"));

    let props = json
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("schema has properties");
    for required in ["schema", "name", "target"] {
        assert!(
            props.contains_key(required),
            "missing required property '{required}' in schema; got: {body}"
        );
    }
    for optional in [
        "controller",
        "state_machine",
        "assets",
        "screens",
        "theme",
        "i18n",
        "metadata",
    ] {
        assert!(
            props.contains_key(optional),
            "missing optional property '{optional}' in schema; got: {body}"
        );
    }

    // Asset / Controller / StateMachine / Screen / Theme / I18n /
    // Target subtypes appear in $defs.
    let defs = json
        .get("$defs")
        .and_then(|v| v.as_object())
        .expect("schema has $defs");
    for ty in [
        "Asset",
        "Controller",
        "I18n",
        "Screen",
        "StateMachine",
        "Target",
        "Theme",
    ] {
        assert!(
            defs.contains_key(ty),
            "missing $defs.{ty} in schema; got: {body}"
        );
    }
}

#[test]
fn app_schema_subcommand_writes_to_stdout() {
    let output = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("schema")
        .output()
        .expect("spawn rlvgl-creator");
    assert!(output.status.success(), "rlvgl-creator exited non-zero");
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(
        stdout.contains("\"title\": \"Manifest\""),
        "stdout missing schema title; got first 200 chars:\n{}",
        &stdout.chars().take(200).collect::<String>()
    );
    assert!(
        stdout.contains("\"$schema\""),
        "stdout missing JSON Schema marker; got first 200 chars:\n{}",
        &stdout.chars().take(200).collect::<String>()
    );
}

#[test]
fn app_schema_subcommand_writes_to_file_when_out_given() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("app-schema.json");
    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("schema")
        .arg("--out")
        .arg(&path)
        .status()
        .expect("spawn rlvgl-creator");
    assert!(status.success(), "rlvgl-creator exited non-zero");
    let body = std::fs::read_to_string(&path).expect("schema file readable");
    let _: serde_json::Value = serde_json::from_str(&body).expect("written file parses as JSON");
    assert!(body.contains("\"Manifest\""), "got: {body}");
}
