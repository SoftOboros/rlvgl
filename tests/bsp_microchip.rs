//! Round-trip test for the Microchip YAML adapter.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;
#[path = "../src/bin/creator/bsp/microchip.rs"]
mod microchip;

use minijinja::{Environment, context};

#[test]
fn microchip_roundtrip_snapshot() {
    let yaml = include_str!("fixtures/microchip.yaml");
    let spec = microchip::yaml_to_ir(yaml).unwrap();
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
