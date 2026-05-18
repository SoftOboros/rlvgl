//! Compile-fail: using a `DbufCpu` handle after consuming it via
//! `start_double_buffer_*`.
//!
//! DCB-00 §5 transition: `DbufCpu::start_double_buffer_read(self, ...)`
//! consumes the CPU-owned typestate token. After this transition the
//! original `DbufCpu` value is moved-from and unusable. This is the
//! load-bearing soundness mechanism for `DcaDoubleBuf` (parallel to
//! `Cpu::lend_for_read` for the circular family).

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, DcaDoubleBuf, NullCache};

fn main() {
    let mut m0: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut m1: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = dca.cpu();
    let _active = cpu.start_double_buffer_read(&mut ctx);
    // The line below MUST fail: `cpu` was moved into
    // `start_double_buffer_read` and is no longer accessible.
    let _slice = cpu.as_m0_slice();
}
