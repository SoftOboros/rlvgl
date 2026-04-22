//! Compile-fail: two concurrent mutable borrows of the back buffer.
//!
//! `Scanout::back_mut` returns `BackBuffer<'_>` borrowing `&mut self`.
//! Calling it twice without dropping the first result is the "renderer
//! and overlay both writing the back buffer" hazard — the borrow
//! checker rejects it with E0499.

use rlvgl_platform::PixelFmt;
use rlvgl_platform::hwcore::addr::{PhysAddr, SDRAM_BANK2_BASE, SDRAM_BANK_STRIDE};
use rlvgl_platform::hwcore::surface::{FrameBuffer, Scanout};

fn main() {
    // SAFETY: fixture; addresses are never dereferenced.
    let front = unsafe {
        FrameBuffer::from_phys(
            PhysAddr::new(SDRAM_BANK2_BASE),
            480,
            272,
            480 * 4,
            PixelFmt::Argb8888,
        )
    };
    let back = unsafe {
        FrameBuffer::from_phys(
            PhysAddr::new(SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE),
            480,
            272,
            480 * 4,
            PixelFmt::Argb8888,
        )
    };
    let mut sc = Scanout::try_new(front, back).unwrap();
    let a = sc.back_mut();
    let b = sc.back_mut();
    let _ = (a, b);
}
