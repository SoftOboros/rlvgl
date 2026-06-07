//! Compile-fail: an active `paint_full` borrow excludes a concurrent
//! `present` call.
//!
//! `LtdcScan::paint_full(&mut self)` returns a `&mut [T; N]` whose
//! lifetime is the `&mut self` borrow on the `LtdcScan`. Calling
//! `present` while that borrow is alive is rejected by the borrow
//! checker — the `&mut self` reborrow inside `present` would
//! conflict with the outstanding `paint_full` slice borrow. This
//! is the load-bearing soundness mechanism for the
//! "no-concurrent-paint-during-present" contract in DCB-00 §5.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 64> = DcaBuf::new([0; 64]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = buf.cpu();
    let mut scan = cpu.start_ltdc_scan(&mut ctx);

    let pixels = scan.paint_full();
    // The line below MUST fail: `scan` is mutably borrowed by
    // `pixels`, so the `&mut self` reborrow inside `present` is
    // rejected as E0499. The post-borrow read of `pixels[0]` keeps
    // the borrow alive past the `present` call.
    scan.present(&mut ctx);
    let _ = pixels[0];
}
