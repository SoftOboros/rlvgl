//! Compile-fail: a `DcaBuf` cannot be lent to two DMA engines
//! simultaneously.
//!
//! DCB-00 §6 INV-D4: at any instant a `DcaBuf` is in exactly one
//! typestate. The handle obtained from `DcaBuf::cpu()` holds an
//! `&mut DcaBuf`, so a second `cpu()` call (or any reborrow) is
//! rejected by the borrow checker — there is no path to "two
//! Device-Pending tokens for the same buffer at once".

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu_a = buf.cpu();
    // The line below MUST fail: `buf` is mutably borrowed by `cpu_a`,
    // so the second `cpu()` call is rejected as E0499.
    let cpu_b = buf.cpu();
    let _ = cpu_a.lend_for_read(&mut ctx);
    let _ = cpu_b.lend_for_read(&mut ctx);
}
