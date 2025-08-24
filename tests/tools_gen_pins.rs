//! Tests for the `gen_pins.py` helper script.

use std::{fs, path::Path, process::Command};
use serde_json::Value;

#[test]
fn aggregates_boards() {
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
    let data = fs::read_to_string(output.path().join("boards.json")).unwrap();
    let v: Value = serde_json::from_str(&data).unwrap();
    let boards = &v["boards"];
    let chip_f4 = &boards["STM32F4DISCOVERY"]["chip"];
    assert_eq!(chip_f4, "STM32F407");
    let chip_nucleo = &boards["NUCLEO-F401RE"]["chip"];
    assert_eq!(chip_nucleo, "STM32F401");
    let chip_f3 = &boards["STM32F3DISCOVERY"]["chip"];
    assert_eq!(chip_f3, "STM32F303");
}
