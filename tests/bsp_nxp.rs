//! Round-trip test for the NXP YAML adapter.
#![cfg(all(feature = "creator", feature = "regression"))]

#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;
#[path = "../src/bin/creator/bsp/nxp.rs"]
mod nxp;

use minijinja::{Environment, context};

#[test]
fn nxp_roundtrip_snapshot() {
    let yaml = include_str!("fixtures/nxp.yaml");
    let spec = nxp::yaml_to_ir(yaml).unwrap();
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
