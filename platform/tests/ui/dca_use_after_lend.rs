//! Compile-fail: using a `Cpu` handle after lending it consumes the
//! handle by-value into a `DeviceRead` typestate.
//!
//! DCB-00 §5 transition table: `Cpu::lend_for_read(self, ...)` consumes
//! the CPU-owned typestate token. After this transition, the original
//! `Cpu` value is moved-from and unusable. This is the load-bearing
//! soundness mechanism: the CPU cannot continue to read or write the
//! buffer through the `Cpu` handle while the DMA engine reads from RAM.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = buf.cpu();
    let (_pending, _addr) = cpu.lend_for_read(&mut ctx);
    // The line below MUST fail: `cpu` was moved into `lend_for_read`
    // and is no longer accessible. Touching the buffer through it
    // would be a use-after-move that violates DCB §5 INV-D4.
    let _slice = cpu.as_slice();
}
