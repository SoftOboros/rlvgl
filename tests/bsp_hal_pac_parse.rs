#![cfg(feature = "creator")]
//! Render HAL and PAC BSP templates and ensure the output parses.

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
use syn::parse_file;

struct DummyAf;

impl AfProvider for DummyAf {
    fn lookup_af(&self, _mcu: &str, _pin: &str, _func: &str) -> Option<u8> {
        Some(0)
    }
}

#[test]
fn hal_pac_parse_per_board() {
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
        env.add_template(
            "pac",
            include_str!("../src/bin/creator/bsp/templates/pac.rs.jinja"),
        )
        .unwrap();

        let rendered_hal = env
            .get_template("hal")
            .unwrap()
            .render(context! { spec => &ir, grouped_writes => true, with_deinit => false })
            .unwrap();
        parse_file(&rendered_hal).expect("HAL parse failed");

        let rendered_pac = env
            .get_template("pac")
            .unwrap()
            .render(context! { spec => &ir, grouped_writes => true, with_deinit => false })
            .unwrap();
        parse_file(&rendered_pac).expect("PAC parse failed");
    }
}
