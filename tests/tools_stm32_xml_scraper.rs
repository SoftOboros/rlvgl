//! Tests for the `stm32_xml_scraper.py` utility.

use std::{fs, path::PathBuf, process::Command};
use tempfile::tempdir;

fn repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn scrapes_ip_and_mcu() {
    let repo = repo_path();
    let script = repo.join("tools/afdb/stm32_xml_scraper.py");

    let tmp = tempdir().unwrap();
    let src_root = tmp.path().join("src");
    let ip_dir = src_root.join("ip");
    let mcu_dir = src_root.join("mcu");
    fs::create_dir_all(&ip_dir).unwrap();
    fs::create_dir_all(&mcu_dir).unwrap();

    fs::write(
        ip_dir.join("usart.xml"),
        r#"<IP Name="USART"><Signal Name="TX"/><Signal Name="RX"/></IP>"#,
    )
    .unwrap();

    fs::write(
        mcu_dir.join("stm32f4.xml"),
        r#"<Mcu Name="STM32F4"><Pin Name="PA0"><Signal Name="USART2_TX" Instance="USART2" AlternateFunction="7"/></Pin><Pin Name="PA1"><Signal Name="USART2_RX" Instance="USART2" AlternateFunction="7"/></Pin></Mcu>"#,
    )
    .unwrap();

    let out = tmp.path().join("out");
    let status = Command::new("python3")
        .arg(&script)
        .arg("--root")
        .arg(&src_root)
        .arg("--output")
        .arg(&out)
        .status()
        .expect("run stm32_xml_scraper.py");
    assert!(status.success());

    let mcu_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out.join("mcu/STM32F4.json")).unwrap()).unwrap();
    // Accept either list-of-entries or map with `sigs` structure
    let af_val = mcu_json["pins"]["PA0"][0]["af"]
        .as_i64()
        .or_else(|| mcu_json["pins"]["PA0"]["sigs"]["USART2_TX"]["af"].as_i64());
    assert_eq!(af_val, Some(7));

    let ip_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(out.join("ip.json")).unwrap()).unwrap();
    assert_eq!(ip_json["USART"]["signals"][0], "TX");
}
