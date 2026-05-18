//! Compile-fail: a circular DMA buffer cannot have two `HalfGuard`s
//! alive at once.
//!
//! DCB-00 §5 transition row for `DeviceActiveCirc<DIR>`: HalfGuard
//! reborrows the parent typestate handle mutably. A second
//! `half_guard` call before the first is dropped is rejected by the
//! borrow checker, which guarantees that "the inactive half" is
//! single-owner at any instant.

use rlvgl_platform::hwcore::dca::{DcaBuf, DcaCacheCtx, Half, NullCache};

fn main() {
    let mut buf: DcaBuf<u8, 64> = DcaBuf::new([0; 64]);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = buf.cpu();
    let mut circ = cpu.start_circular_read(&mut ctx);

    let g1 = circ.half_guard(&mut ctx, Half::First);
    // The line below MUST fail: `circ` is reborrowed mutably by `g1`,
    // so the second `half_guard` call is rejected as E0499. NLL would
    // otherwise drop `g1` early; the use of `g1` past the second call
    // forces the borrow to extend.
    let _g2 = circ.half_guard(&mut ctx, Half::Second);
    let _ = g1.half();
}
