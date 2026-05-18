//! Compile-fail: a `DcaDoubleBuf` cannot be lent to two DMA engines
//! simultaneously.
//!
//! DCB-00 §6 INV-D4 / INV-D14: at any instant a `DcaDoubleBuf` is in
//! exactly one typestate. The handle obtained from
//! `DcaDoubleBuf::cpu()` holds an `&mut DcaDoubleBuf`, so a second
//! `cpu()` call (or any reborrow) is rejected by the borrow checker.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, DcaDoubleBuf, NullCache};

fn main() {
    let mut m0: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut m1: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu_a = dca.cpu();
    // The line below MUST fail: `dca` is mutably borrowed by `cpu_a`,
    // so the second `cpu()` call is rejected as E0499.
    let cpu_b = dca.cpu();
    let _ = cpu_a.start_double_buffer_read(&mut ctx);
    let _ = cpu_b.start_double_buffer_write(&mut ctx);
}
