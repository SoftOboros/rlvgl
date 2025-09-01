//! Tests the BSP generation pipeline: `.ioc` → IR → template.
#![cfg(all(feature = "creator", feature = "regression"))]

#[path = "../src/bin/creator/bsp/af.rs"]
mod af;
#[path = "../src/bin/creator/bsp/ioc.rs"]
mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;

use af::JsonAfDb;
use ioc::ioc_to_ir;
use minijinja::{Environment, context};
use std::{path::PathBuf, process::Command};
use tempfile::NamedTempFile;

#[test]
fn ioc_roundtrip_snapshot() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let csv = manifest_dir.join("tests/fixtures/stm32_af.csv");
    let script = manifest_dir.join("tools/afdb/st_extract_af.py");
    let tmp = NamedTempFile::new().unwrap();
    let status = Command::new("python3")
        .arg(&script)
        .arg("--db")
        .arg(&csv)
        .arg("--out")
        .arg(tmp.path())
        .status()
        .expect("failed to run st_extract_af.py");
    assert!(status.success());
    let afdb = JsonAfDb::from_path(tmp.path()).unwrap();

    let ioc_text = include_str!("fixtures/simple.ioc");
    let ir = ioc_to_ir(ioc_text, &afdb, false).unwrap();
    insta::assert_yaml_snapshot!("ir", &ir);

    let mut env = Environment::new();
    env.add_template(
        "gen",
        include_str!("../src/bin/creator/bsp/templates/simple.rs.jinja"),
    )
    .unwrap();
    let tmpl = env.get_template("gen").unwrap();
    let rendered = tmpl.render(context! { spec => &ir }).unwrap();
    insta::assert_snapshot!("generated", rendered);
}
