//! Tests for BSP mod generation and lib scanning.

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
