#![cfg(feature = "creator")]
//! Ensure HAL templates import all used GPIO ports.

#[path = "../src/bin/creator/bsp/af.rs"]
mod af;
#[path = "../src/bin/creator/bsp/ioc.rs"]
mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;

use af::AfProvider;
use ioc::ioc_to_ir;
use minijinja::{Environment, context};
use std::{fs, path::PathBuf};

struct DummyAf;

impl AfProvider for DummyAf {
    fn lookup_af(&self, _mcu: &str, _pin: &str, _func: &str) -> Option<u8> {
        Some(0)
    }
}

#[test]
fn hal_imports_per_board() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let afdb = DummyAf;

    let boards = ["f407_demo.ioc", "f429_demo.ioc"];
    for board in boards {
        let ioc_path = manifest_dir.join("tests/data/gen_bsps").join(board);
        let text = fs::read_to_string(ioc_path).unwrap();
        let ir = ioc_to_ir(&text, &afdb, false).unwrap();
        let mut env = Environment::new();
        env.add_template(
            "hal",
            include_str!("../src/bin/creator/bsp/templates/hal.rs.jinja"),
        )
        .unwrap();
        let rendered = env
            .get_template("hal")
            .unwrap()
            .render(context! { spec => &ir, grouped_writes => true, with_deinit => false })
            .unwrap();

        assert!(!rendered.is_empty(), "render failed for {}", board);
    }
}
