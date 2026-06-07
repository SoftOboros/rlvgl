//! Compile-fail: a `DsiRegs`-shaped struct whose layout drifts from the
//! silicon contract.
//!
//! `crate::hwcore::regs::dsi::DsiRegs` carries a `const _: () =
//! assert!(offset_of!(DsiRegs, lccr) == 0x064)` invariant. A struct
//! that omits the `0x01C..0x02B` reserved gap would put LCCR at `0x60`
//! and ship the panel-snow bug. This fixture demonstrates that the
//! compile-time offset assertion class catches that error before the
//! binary is flashed.
//!
//! Note: the fixture defines its own *broken* struct rather than mutating
//! the real `DsiRegs` — we want a stable trybuild snapshot that proves
//! the assertion mechanism works.

use core::mem::offset_of;

#[repr(C)]
struct DsiRegsBroken {
    vr: u32,        // 0x00
    cr: u32,        // 0x04
    ccr: u32,       // 0x08
    lvcidr: u32,    // 0x0C
    lcolcr: u32,    // 0x10
    lpcr: u32,      // 0x14
    lpmcr: u32,     // 0x18
    // BUG: missing `_reserved_01c: [u32; 4]` for the 0x01C..0x02B gap.
    pcr: u32,       // 0x1C — should be 0x2C
    gvcidr: u32,    // 0x20 — should be 0x30
    mcr: u32,       // 0x24 — should be 0x34
    vmcr: u32,      // 0x28 — should be 0x38
    vpcr: u32,      // 0x2C — should be 0x3C
    vccr: u32,      // 0x30 — should be 0x40
    vnpcr: u32,     // 0x34 — should be 0x44
    vhsacr: u32,    // 0x38 — should be 0x48
    vhbpcr: u32,    // 0x3C — should be 0x4C
    vlcr: u32,      // 0x40 — should be 0x50
    vvsacr: u32,    // 0x44 — should be 0x54
    vvbpcr: u32,    // 0x48 — should be 0x58
    vvfpcr: u32,    // 0x4C — should be 0x5C
    vvacr: u32,     // 0x50 — should be 0x60
    lccr: u32,      // 0x54 — SHOULD be 0x64
}

// This must fail to compile because LCCR ends up at 0x54, not 0x64.
const _: () = assert!(offset_of!(DsiRegsBroken, lccr) == 0x064);

fn main() {}
