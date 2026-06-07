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

fn assert_lower_right(ascii: &str) {
    let lines: Vec<&str> = ascii.lines().collect();
    assert_eq!(lines.len(), HEIGHT);
    let mut found = false;
    for (y, line) in lines.iter().enumerate() {
        assert_eq!(line.len(), WIDTH);
        for (x, ch) in line.chars().enumerate() {
            if x >= WIDTH / 3 && y >= HEIGHT / 3 {
                if ch != '@' {
                    found = true;
                }
            } else {
                assert_eq!(ch, '@', "unexpected char at ({x},{y})");
            }
        }
    }
    assert!(found, "no content in lower-right region");
}

fn run_demo(flag: &str) -> String {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("screen.txt");
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-sim"))
        .arg("--headless")
        .arg(&path)
        .arg(flag)
        .status()
        .expect("failed to run rlvgl-sim");
    assert!(status.success());
    fs::read_to_string(&path).expect("read ascii")
}

#[test]
fn png_demo_in_lower_right() {
    let ascii = run_demo("--png");
    assert_lower_right(&ascii);
}

#[test]
fn qrcode_demo_in_lower_right() {
    let ascii = run_demo("--qrcode");
    assert_lower_right(&ascii);
}

#[test]
fn gif_demo_in_lower_right() {
    let ascii = run_demo("--gif");
    assert_lower_right(&ascii);
}
