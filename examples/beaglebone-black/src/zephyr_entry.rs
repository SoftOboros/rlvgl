//! Zephyr staticlib entry point for BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Exports `rlvgl_init()` which is called from `zephyr/src/main.c` after
//! Zephyr kernel init. Receives kernel semaphores and display info from C,
//! then runs the DiscoController render loop.
//!
//! Touch and key events arrive via `rlvgl_touch_event()` and `rlvgl_key_event()`
//! callbacks, called from C's input subsystem handler.

#![allow(dead_code)]

use crate::zephyr_sync;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Display info (passed from C)
// ---------------------------------------------------------------------------

/// Matches `struct rlvgl_display_info` in zephyr/src/main.c.
#[repr(C)]
pub struct RlvglDisplayInfo {
    pub fb_front: *mut u8,
    pub fb_back: *mut u8,
    pub fb_len: u32,
    pub width: u16,
    pub height: u16,
    pub pixel_size: u16,
}

// ---------------------------------------------------------------------------
// Touch event (passed from C input callback)
// ---------------------------------------------------------------------------

/// Matches `struct rlvgl_touch_event` in zephyr/src/main.c.
#[repr(C)]
struct TouchEventC {
    x: i16,
    y: i16,
    pressed: u8,
}

// Atomic touch state — written by input callback, read by render loop
static TOUCH_XY: AtomicU32 = AtomicU32::new(0);
static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);
static TOUCH_DIRTY: AtomicBool = AtomicBool::new(false);

// Key ring buffer — written by input callback, read by render loop
static KEY_BUF: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static KEY_WRITE: AtomicU32 = AtomicU32::new(0);
static KEY_READ: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------
// Extern "C" callbacks — called from C input subsystem
// ---------------------------------------------------------------------------

/// Called from C when a touch event is reported by FT5x06.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_touch_event(evt: *const TouchEventC) {
    unsafe {
        let e = &*evt;
        let packed = (e.x as u16 as u32) | ((e.y as u16 as u32) << 16);
        TOUCH_XY.store(packed, Ordering::Relaxed);
        TOUCH_PRESSED.store(e.pressed != 0, Ordering::Relaxed);
        TOUCH_DIRTY.store(true, Ordering::Release);
    }
}

/// Called from C when a key event is reported (joystick, buttons).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_key_event(code: u16, pressed: u8) {
    let packed = (code as u32) | ((pressed as u32) << 16) | (1 << 31);
    let idx = KEY_WRITE.fetch_add(1, Ordering::Relaxed) as usize % KEY_BUF.len();
    KEY_BUF[idx].store(packed, Ordering::Release);
}

/// Called from C LCDC EOF ISR wrapper.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_lcdc_eof_isr() {
    // Clear the LCDC EOF interrupt flag
    unsafe {
        crate::bsp::lcdc::clear_eof_irq();
    }
}

// ---------------------------------------------------------------------------
// Input consumption (called by render loop)
// ---------------------------------------------------------------------------

/// Take the latest touch state if dirty. Returns (x, y, pressed).
fn take_touch() -> Option<(i16, i16, bool)> {
    if !TOUCH_DIRTY.swap(false, Ordering::Acquire) {
        return None;
    }
    let packed = TOUCH_XY.load(Ordering::Relaxed);
    let x = packed as u16 as i16;
    let y = (packed >> 16) as u16 as i16;
    let pressed = TOUCH_PRESSED.load(Ordering::Relaxed);
    Some((x, y, pressed))
}

/// Take the next key event if available. Returns (code, pressed).
fn take_key() -> Option<(u16, bool)> {
    let r = KEY_READ.load(Ordering::Relaxed);
    let w = KEY_WRITE.load(Ordering::Acquire);
    if r == w {
        return None;
    }
    let idx = r as usize % KEY_BUF.len();
    let packed = KEY_BUF[idx].swap(0, Ordering::Acquire);
    KEY_READ.store(r.wrapping_add(1), Ordering::Release);
    if packed & (1 << 31) == 0 {
        return None;
    }
    let code = packed as u16;
    let pressed = (packed >> 16) & 1 != 0;
    Some((code, pressed))
}

// ---------------------------------------------------------------------------
// Main entry point — called from C main()
// ---------------------------------------------------------------------------

/// Initialize the rlvgl demo under Zephyr.
///
/// Called by `main()` in `zephyr/src/main.c` after kernel boot.
/// Receives the EOF semaphore and display info from C. Sets up the
/// DiscoController and enters the render loop.
///
/// # Safety
///
/// Must be called exactly once from the Zephyr main thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_init(
    eof_sem: *mut zephyr_sync::k_sem,
    display_info: *const RlvglDisplayInfo,
) {
    let _info = unsafe { &*display_info };

    // TODO: Initialize heap allocator (embedded-alloc)
    // TODO: Create DiscoController with beaglebone_black() capabilities
    // TODO: Enter render loop:
    //   1. k_sem_take(eof_sem) — wait for LCDC frame complete
    //   2. take_touch() / take_key() — process input
    //   3. controller.dispatch_event() / controller.tick()
    //   4. Render widget tree into back framebuffer via CpuBlitter
    //   5. Swap front/back buffers
    //   6. Loop

    // Placeholder: simple render loop that fills with cycling color
    let fb_back = _info.fb_back;
    let w = _info.width as u32;
    let h = _info.height as u32;
    let mut tick: u32 = 0;

    loop {
        // Wait for frame complete
        unsafe {
            zephyr_sync::sem_wait_forever(eof_sem);
        }

        // Fill back buffer with gradient
        let phase = tick & 0xFF;
        unsafe {
            let fb = core::slice::from_raw_parts_mut(fb_back as *mut u32, (w * h) as usize);
            for (i, pixel) in fb.iter_mut().enumerate() {
                let x = (i as u32) % w;
                let y = (i as u32) / w;
                let r = ((x + phase) & 0xFF) as u8;
                let g = ((y + phase) & 0xFF) as u8;
                let b = phase as u8;
                *pixel = 0xFF00_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
            }
        }

        tick = tick.wrapping_add(1);

        // Process touch input (placeholder — dispatch to controller when integrated)
        while let Some((_x, _y, _pressed)) = take_touch() {
            // TODO: dispatch to DiscoController
        }
        while let Some((_code, _pressed)) = take_key() {
            // TODO: dispatch to DiscoController
        }
    }
}
