//! Test the `st_ioc_board.py` converter for user CubeMX projects.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn converts_user_ioc() {
    let repo = repo_path();
    let script = repo.join("tools/afdb/st_ioc_board.py");
    let data_dir = repo.join("tests/data/tools_st_extract_af");

    let tmp = tempdir().unwrap();
    let out = tmp.path().join("board.json");
    let status = Command::new("python3")
        .arg(&script)
        .arg("--ioc")
        .arg(data_dir.join("sample.ioc"))
        .arg("--mcu-root")
        .arg(data_dir.join("mcu"))
        .arg("--board")
        .arg("USER-BOARD")
        .arg("--output")
        .arg(&out)
        .status()
        .expect("run st_ioc_board.py");
    assert!(status.success());
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(json["board"], "USER-BOARD");
    assert_eq!(json["chip"], "STM32F4");
    assert_eq!(json["pins"]["PA0"]["USART2_TX"], 7);
}
