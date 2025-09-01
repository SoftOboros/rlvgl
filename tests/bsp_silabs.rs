//! Round-trip test for the Silicon Labs YAML adapter.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;
#[path = "../src/bin/creator/bsp/silabs.rs"]
mod silabs;

use minijinja::{Environment, context};

#[test]
fn silabs_roundtrip_snapshot() {
    let yaml = include_str!("fixtures/silabs.yaml");
    let spec = silabs::yaml_to_ir(yaml).unwrap();
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
