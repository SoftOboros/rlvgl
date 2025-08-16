//! Integration test verifying headless ASCII output from rlvgl-sim.
use std::{fs, process::Command};

use tempfile::tempdir;

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

#[test]
fn headless_renders_ascii_screen() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("screen.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-sim"))
        .arg("--headless")
        .arg(&path)
        .status()
        .expect("failed to run rlvgl-sim");
    assert!(status.success());
    let ascii = fs::read_to_string(&path).expect("read ascii");
    let lines: Vec<&str> = ascii.lines().collect();
    if lines.len() != HEIGHT {
        panic!("Unexpected line count: {}\n{}", lines.len(), ascii);
    }
    let expected_line = "@".repeat(WIDTH);
    for (i, line) in lines.iter().enumerate() {
        if *line != expected_line {
            panic!("Line {} mismatch\n{}", i, ascii);
        }
    }
}
