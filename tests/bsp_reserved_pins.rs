//! Verifies reserved SWD pins are rejected unless explicitly allowed.
#![cfg(feature = "creator")]
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

struct StubAf;

impl AfProvider for StubAf {
    fn lookup_af(&self, _mcu: &str, _pin: &str, _func: &str) -> Option<u8> {
        Some(0)
    }
}

#[test]
fn reject_reserved_pins_by_default() {
    let ioc_text = include_str!("fixtures/reserved.ioc");
    let err = ioc_to_ir(ioc_text, &StubAf, false).unwrap_err();
    assert!(err.to_string().contains("reserved pin"));
}

#[test]
fn allow_reserved_pins_with_override() {
    let ioc_text = include_str!("fixtures/reserved.ioc");
    let ir = ioc_to_ir(ioc_text, &StubAf, true).unwrap();
    assert!(ir.pinctrl.iter().any(|p| p.pin == "PA13"));
}
