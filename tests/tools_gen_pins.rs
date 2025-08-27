//! Tests for the `gen_pins.py` helper script.

use serde_json::Value;
use std::{fs, path::Path, process::Command};

#[test]
fn copies_mcu_definition() {
    let input = Path::new("tests/data/gen_pins");
    let output = tempfile::tempdir().unwrap();
    let status = Command::new("python3")
        .arg("tools/gen_pins.py")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output.path())
        .status()
        .expect("run gen_pins");
    assert!(status.success());
    let data = fs::read_to_string(output.path().join("mcu.json")).unwrap();
    let v: Value = serde_json::from_str(&data).unwrap();
    assert_eq!(v["chip"], "STM32F407");
}
