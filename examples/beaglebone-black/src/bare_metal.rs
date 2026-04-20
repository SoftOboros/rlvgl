//! Bare-metal entry point for BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Assumes U-Boot SPL has initialized DDR3L, PLLs, and basic clocks.
//! We take over from `_start` with a working memory system and configure
//! LCDC, I2C2 (touch), and GPIO (backlight) from scratch.
//!
//! Target: `armv7a-none-eabihf`
//! Build: `cargo build --target armv7a-none-eabihf -p rlvgl-example-bbb --bin rlvgl-bbb-bare --features bare_metal`

#![no_std]
#![no_main]

mod bsp;

use bsp::lcdc;

// ---------------------------------------------------------------------------
// Panic handler (required for no_std)
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------
// Entry point — called by U-Boot or JTAG after DDR is initialized
// ---------------------------------------------------------------------------

/// Bare-metal entry point.
///
/// U-Boot SPL initializes DDR3L and basic clocks, then chainloads this
/// binary. We set up the LCDC, fill the framebuffer with a test pattern,
/// and enter the main loop.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Phase 1: Enable peripheral clocks
    unsafe {
        bsp::prcm::enable_lcdc();
        bsp::prcm::enable_i2c2();
        bsp::prcm::enable_gpio1();
    }

    // Phase 2: Configure pin mux
    unsafe {
        bsp::pinmux::configure_lcd_pins();
        bsp::pinmux::configure_i2c2_pins();
    }

    // Phase 3: Allocate framebuffer in DDR
    // Place it at a fixed address well above our code
    let fb_addr: u32 = 0x8020_0000;
    let fb_size: u32 = lcdc::HACTIVE * lcdc::VACTIVE * 4; // ARGB8888

    // Fill framebuffer with a solid color (blue) as smoke test
    unsafe {
        let fb = core::slice::from_raw_parts_mut(fb_addr as *mut u32, (fb_size / 4) as usize);
        for pixel in fb.iter_mut() {
            *pixel = 0xFF_00_40_80; // ARGB: opaque dark blue
        }
    }

    // Phase 4: Initialize LCDC raster controller
    unsafe {
        lcdc::init_raster(fb_addr, fb_size);
    }

    // Phase 5: Main loop
    // For now, just wait for EOF interrupts and do nothing.
    // The full DiscoController integration requires a heap allocator
    // (embedded-alloc) which will be added in the next phase.
    loop {
        unsafe {
            if lcdc::is_eof_pending() {
                lcdc::clear_eof_irq();
                // Frame complete — future: render next frame here
            }
        }
        core::hint::spin_loop();
    }
}
