//! Tests the BSP generation pipeline: `.ioc` → IR → template.
#![cfg(all(feature = "creator", feature = "regression"))]
// The `#[path]` includes below pull in source modules from the
// `rlvgl-creator` binary; only a subset of their public surface is
// exercised by this test, so the unused helpers are expected.
#![allow(dead_code, clippy::too_many_arguments, clippy::duplicate_mod)]

#[path = "../src/bin/creator/bsp/af.rs"]
mod af;
#[path = "../src/bin/creator/bsp/ioc.rs"]
mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;

use af::AfProvider;
use ioc::ioc_to_ir;
use minijinja::{Environment, context};

#[test]
fn ioc_roundtrip_snapshot() {
    let ioc_text = include_str!("fixtures/simple.ioc");
    let afdb = DummyAf;
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

struct DummyAf;
impl AfProvider for DummyAf {
    fn lookup_af(&self, _mcu: &str, _pin: &str, func: &str) -> Option<u8> {
        Some(match func {
            "USART1_TX" | "USART1_RX" => 7,
            "SPI1_SCK" | "SPI1_MISO" | "SPI1_MOSI" => 5,
            "I2C1_SCL" | "I2C1_SDA" => 4,
            _ => 0,
        })
    }
}
