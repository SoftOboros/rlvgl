//! Verifies reserved SWD pins are rejected unless explicitly allowed.
#![cfg(feature = "creator")]

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
