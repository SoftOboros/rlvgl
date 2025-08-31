//! Round-trip test for the RP2040 YAML adapter.
#![cfg(all(feature = "creator", feature = "regression"))]

#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;
#[path = "../src/bin/creator/bsp/rp2040.rs"]
mod rp2040;

use minijinja::{Environment, context};

#[test]
fn rp2040_roundtrip_snapshot() {
    let yaml = include_str!("fixtures/rp2040.yaml");
    let spec = rp2040::yaml_to_ir(yaml).unwrap();
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
