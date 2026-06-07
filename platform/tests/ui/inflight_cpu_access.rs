//! Compile-fail: CPU access during a DMA in-flight transfer.
//!
//! A `BorrowedForDma` carries a `&mut BackBuffer` reborrow. While that
//! token is alive, calling `cpu_slice()` on the originating back buffer
//! must be rejected by the borrow checker — otherwise the CPU could
//! mutate pixels that the DMA engine is still reading.

use rlvgl_platform::PixelFmt;
use rlvgl_platform::hwcore::addr::{PhysAddr, SDRAM_BANK2_BASE};
use rlvgl_platform::hwcore::surface::{BackBuffer, FrameBuffer};

fn main() {
    // SAFETY: fixture; the address is never dereferenced because the
    // example fails to compile before any code runs.
    let mut fb = unsafe {
        FrameBuffer::from_phys(
            PhysAddr::new(SDRAM_BANK2_BASE),
            480,
            272,
            480 * 4,
            PixelFmt::Argb8888,
        )
    };
    let mut back = BackBuffer::wrap(&mut fb);
    let dst = back.dma_dst();
    // The line below must fail: `back` is reborrowed mutably by `dst`,
    // so a second `&mut self` for `cpu_slice` is illegal while `dst`
    // is still live (used after this call).
    let _slice = unsafe { back.cpu_slice() };
    // Force NLL to keep `dst` alive past `cpu_slice()`.
    let _ = dst.dma_addr();
}
