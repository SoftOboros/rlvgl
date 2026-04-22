//! Compile-fail: constructing an `MmioAddr<T>` outside an `unsafe` block.
//!
//! Register-block bases must be created via `unsafe const fn MmioAddr::new`
//! so the caller acknowledges the provenance / aliasing contract. A bare
//! safe call (here in const context) must fail with E0133.

use rlvgl_platform::MmioAddr;

const BAD: MmioAddr<u32> = MmioAddr::new(0x5200_1000);

fn main() {
    let _ = BAD;
}
