//! Compile-fail: two concurrent `paint_full` borrows on the same
//! `LtdcScan` are rejected by the borrow checker.
//!
//! Each `paint_full` call reborrows `LtdcScan` mutably; a second
//! concurrent call would alias the buffer's `&mut [T; N]` borrow.
//! Rust's `&mut`-uniqueness rule rejects this at compile time
//! (E0499) — the type system equivalent of "no two CPU painters
//! at once" without needing a runtime check.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 64> = DcaBuf::new([0; 64]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = buf.cpu();
    let mut scan = cpu.start_ltdc_scan(&mut ctx);

    let pixels1 = scan.paint_full();
    // The line below MUST fail: `scan` is mutably borrowed by
    // `pixels1`; the second `paint_full` is rejected as E0499.
    let pixels2 = scan.paint_full();
    let _ = pixels1[0];
    let _ = pixels2[0];
}
