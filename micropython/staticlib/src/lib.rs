// lib.rs - Final no_std static-library artifact for MicroPython linking.

#![no_std]

use core::panic::PanicInfo;
use rlvgl_binding as _;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
