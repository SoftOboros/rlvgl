//! Compile-fail: using an `LtdcScan` handle after consuming it via
//! `stop_scan`.
//!
//! DCB-00 §5 transition table for `DeviceLtdcScan<T, N>`:
//! `LtdcScan::stop_scan(self, ctx)` consumes the typestate token.
//! After the consumer engine has been stopped and the typestate
//! returns to `Cpu`, the original `LtdcScan` value is moved-from
//! and unusable.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 64> = DcaBuf::new([0; 64]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = buf.cpu();
    let mut scan = cpu.start_ltdc_scan(&mut ctx);
    let _cpu_back = scan.stop_scan(&mut ctx);
    // The line below MUST fail: `scan` was moved into `stop_scan`
    // and is no longer accessible.
    let _slice = scan.paint_full();
}
