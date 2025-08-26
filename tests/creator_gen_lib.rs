//! Tests for BSP mod generation and lib scanning.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/af.rs"]
pub mod af;
#[path = "../src/bin/creator/bsp/ioc.rs"]
pub mod ioc;
#[path = "../src/bin/creator/bsp/ir.rs"]
pub mod ir;
mod bsp {
    pub use super::af;
    pub use super::ioc;
    pub use super::ir;
}
#[path = "../src/bin/creator/bsp_gen.rs"]
mod bsp_gen;
#[path = "../src/bin/creator/gen_lib.rs"]
mod gen_lib;

use minijinja::{Environment, context};
use std::collections::BTreeMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn board_mod_respects_flags() {
    let tmp = tempdir().unwrap();
    bsp_gen::emit_board_mod(tmp.path(), true, false, true, false).unwrap();
    let text = fs::read_to_string(tmp.path().join("mod.rs")).unwrap();
    assert!(text.contains("pub mod hal;"));
    assert!(!text.contains("pub mod pac;"));
    assert!(text.contains("pub mod summary;"));
    assert!(!text.contains("pub mod pinreport;"));
}

#[test]
fn scan_tree_detects_forms() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("foo/hal")).unwrap();
    fs::create_dir_all(root.join("foo/pac")).unwrap();
    fs::write(root.join("foo/hal/split.rs"), "").unwrap();
    fs::write(root.join("foo/pac/flat.rs"), "").unwrap();
    fs::write(root.join("foo/summary.rs"), "").unwrap();
    let mcus = gen_lib::scan_tree(root).unwrap();
    assert_eq!(mcus.len(), 1);
    let (slug, forms) = &mcus[0];
    assert_eq!(slug, "foo");
    assert!(forms.hal_split);
    assert!(!forms.hal_flat);
    assert!(!forms.pac_split);
    assert!(forms.pac_flat);
    assert!(forms.summary);
    assert!(!forms.pinreport);
}

#[test]
fn emit_lib_rs_uses_mod_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("foo/hal")).unwrap();
    fs::write(root.join("foo/hal/split.rs"), "").unwrap();
    fs::write(root.join("foo/mod.rs"), "").unwrap();
    gen_lib::emit_lib_rs(root, root.join("lib.rs"), None, &[], None, false).unwrap();
    let text = fs::read_to_string(root.join("lib.rs")).unwrap();
    assert!(text.contains("pub mod foo;"));
}

#[test]
fn templates_skip_empty_pin_fns() {
    let mut env = Environment::new();
    env.add_template(
        "pac",
        include_str!("../src/bin/creator/bsp/templates/pac.rs.jinja"),
    )
    .unwrap();
    env.add_template(
        "hal",
        include_str!("../src/bin/creator/bsp/templates/hal.rs.jinja"),
    )
    .unwrap();
    let spec = context! {
        mcu => "STM32F0",
        pinctrl => Vec::<String>::new(),
        peripherals => BTreeMap::<String, String>::new(),
    };
    let pac = env
        .get_template("pac")
        .unwrap()
        .render(context! { spec => spec.clone(), grouped_writes => true, with_deinit => false })
        .unwrap();
    assert!(!pac.contains("pub fn configure_pins_pac"));
    let hal = env
        .get_template("hal")
        .unwrap()
        .render(context! { spec => spec, grouped_writes => true, with_deinit => false })
        .unwrap();
    assert!(!hal.contains("pub fn configure_pins_hal"));
}
