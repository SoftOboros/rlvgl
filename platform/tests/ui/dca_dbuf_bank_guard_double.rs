//! Compile-fail: a double-buffer DMA cannot have two `BankGuard`s
//! alive at once.
//!
//! DCB-00 §5 transition row for `DeviceActiveDoubleBuf<DIR>`:
//! `BankGuard` reborrows the parent typestate handle (`DbufRead` /
//! `DbufWrite`) mutably. A second `bank_guard` call before the first
//! is dropped is rejected by the borrow checker, which guarantees
//! that "the inactive bank" is single-owner at any instant.

use rlvgl_platform::hwcore::dca::{Bank, DcaBuf, DcaCacheCtx, DcaDoubleBuf, NullCache};

fn main() {
    let mut m0: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut m1: DcaBuf<u8, 32> = DcaBuf::new([0; 32]);
    let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
    let mut cache = NullCache::default();
    let mut ctx = DcaCacheCtx::new(&mut cache);

    let cpu = dca.cpu();
    let mut active = cpu.start_double_buffer_read(&mut ctx);

    let g1 = active.bank_guard(&mut ctx, Bank::M0);
    // The line below MUST fail: `active` is reborrowed mutably by `g1`,
    // so the second `bank_guard` call is rejected as E0499. NLL would
    // otherwise drop `g1` early; the use of `g1` past the second call
    // forces the borrow to extend.
    let _g2 = active.bank_guard(&mut ctx, Bank::M1);
    let _ = g1.bank();
}
