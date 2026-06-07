//! Zephyr kernel API bindings for the BeagleBone Black demo.
//!
//! Thin FFI wrappers around k_sem operations. The actual C functions
//! are implemented in `zephyr/src/main.c` as wrappers around Zephyr
//! kernel macros.

#![allow(dead_code)]
#![allow(non_camel_case_types)]

/// Opaque Zephyr kernel semaphore.
#[repr(C)]
pub struct k_sem {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    /// Take a Zephyr semaphore. `timeout_ms < 0` means K_FOREVER.
    #[link_name = "rlvgl_k_sem_take"]
    pub fn k_sem_take(sem: *mut k_sem, timeout_ms: i32) -> i32;

    /// Give a Zephyr semaphore.
    #[link_name = "rlvgl_k_sem_give"]
    pub fn k_sem_give(sem: *mut k_sem);

    /// Sleep for the given number of milliseconds.
    #[link_name = "rlvgl_k_sleep_ms"]
    pub fn k_sleep_ms(ms: u32);
}

/// Wait forever on a semaphore (blocking).
pub unsafe fn sem_wait_forever(sem: *mut k_sem) -> bool {
    unsafe { k_sem_take(sem, -1) == 0 }
}

/// Non-blocking semaphore take.
pub unsafe fn sem_try_take(sem: *mut k_sem) -> bool {
    unsafe { k_sem_take(sem, 0) == 0 }
}
