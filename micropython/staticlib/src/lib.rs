// lib.rs - Final no_std static-library artifact for MicroPython linking.

#![no_std]

use core::panic::PanicInfo;
use rlvgl_binding as _;

#[cfg(not(target_os = "none"))]
unsafe extern "C" {
    fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    #[cfg(not(target_os = "none"))]
    unsafe {
        abort()
    }

    #[cfg(target_os = "none")]
    loop {
        core::hint::spin_loop();
    }
}
