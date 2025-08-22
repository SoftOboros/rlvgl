//! Test the `st_extract_af.py` helper against sample CSV and IOC files.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn converts_csv_and_ioc() {
    let repo = repo_path();
    let script = repo.join("tools/afdb/st_extract_af.py");
    let data_dir = repo.join("tests/data/tools_st_extract_af");

    let tmp = tempdir().unwrap();

    // CSV input
    let csv_in = data_dir.join("pins.csv");
    let csv_out = tmp.path().join("csv.json");
    let status = Command::new("python3")
        .arg(&script)
        .arg("--input")
        .arg(&csv_in)
        .arg("--output")
        .arg(&csv_out)
        .status()
        .expect("run st_extract_af.py on csv");
    assert!(status.success());
    let csv_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&csv_out).unwrap()).unwrap();
    assert_eq!(csv_json["PA0"]["USART2_TX"], 7);

    // IOC input
    let ioc_in = data_dir.join("sample.ioc");
    let ioc_out = tmp.path().join("ioc.json");
    let status = Command::new("python3")
        .arg(&script)
        .arg("--input")
        .arg(&ioc_in)
        .arg("--output")
        .arg(&ioc_out)
        .status()
        .expect("run st_extract_af.py on ioc");
    assert!(status.success());
    let ioc_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ioc_out).unwrap()).unwrap();
    assert_eq!(ioc_json["PA1"]["USART2_RX"], 0);
}
