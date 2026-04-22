//! Compile-fail: writes to the currently-scanning front buffer.
//!
//! `FrontBuffer<'a>` deliberately exposes no `cpu_slice_mut` — LTDC is
//! actively reading the buffer, and CPU writes would tear. Resolving
//! the missing method via the type's `&` accessors must fail (E0599).

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
    let sc = Scanout::try_new(front, back).unwrap();
    let front = sc.front();
    let _slice: &mut [u8] = front.cpu_slice_mut();
}
