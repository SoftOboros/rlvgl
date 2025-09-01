//! Round-trip test for the Espressif YAML adapter.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/espressif.rs"]
mod espressif;
#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;

use minijinja::{Environment, context};

#[test]
fn espressif_roundtrip_snapshot() {
    let yaml = include_str!("fixtures/espressif.yaml");
    let spec = espressif::yaml_to_ir(yaml).unwrap();
    insta::assert_yaml_snapshot!("ir", &spec);

    let mut env = Environment::new();
    env.add_template(
        "gen",
        include_str!("../src/bin/creator/bsp/templates/simple.rs.jinja"),
    )
    .unwrap();
    let tmpl = env.get_template("gen").unwrap();
    let rendered = tmpl.render(context! { spec => &spec }).unwrap();
    insta::assert_snapshot!("generated", rendered);
}
