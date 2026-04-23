//! Bare-metal entry point for BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! Assumes U-Boot SPL has initialized DDR3L, PLLs, pinmux for UART0, and
//! basic clocks. We take over from `_start` with a working memory system
//! and configure LCDC, I2C2 (touch), and GPIO (backlight) from scratch.
//!
//! Target: `armv7a-none-eabihf`
//! Build:
//!   RUSTFLAGS="" cargo build --target armv7a-none-eabihf \
//!       -p rlvgl-example-bbb --bin rlvgl-bbb-bare \
//!       --no-default-features --features bare_metal --release
//!
//! Chainload from U-Boot (serial console, press any key to interrupt):
//!   => fatload mmc 0:1 0x82000000 rlvgl-bbb-bare.bin
//!   => go 0x82000000

#![no_std]
#![no_main]

mod bsp;

#[cfg(feature = "freertos")]
mod freertos_entry;
#[cfg(feature = "freertos")]
mod freertos_sync;

#[cfg(feature = "zephyr")]
mod zephyr_entry;
#[cfg(feature = "zephyr")]
mod zephyr_sync;

use bsp::{lcdc, uart0};

// ---------------------------------------------------------------------------
// Framebuffer layout — must stay in sync with `memory.x`.
//
// memory.x reserves DDR up to 0x83000000 for .text/.rodata/.data/.bss and
// places __stack_top at 0x84000000. The framebuffer lives past that.
// ---------------------------------------------------------------------------

const FB_BASE: u32 = 0x8400_0000;
const FB_BYTES: u32 = lcdc::HACTIVE * lcdc::VACTIVE * 4; // ARGB8888

// ---------------------------------------------------------------------------
// Startup assembly.
//
// U-Boot's `go <addr>` enters with:
//   - CPU in SVC mode (but state of A/I/F masks undefined — we re-mask)
//   - MMU may or may not be enabled with U-Boot's ID-mapped 1:1 table
//   - D-cache / I-cache likely ON (U-Boot turns them on for speed)
//   - SP set to wherever U-Boot's stack was
//
// We can't inherit any of that: the MMU mapping vanishes once we overwrite
// U-Boot's page tables (which live in its relocated region), and we need a
// deterministic SP before Rust runs. This preamble:
//   1. Forces SVC mode, masks A/I/F interrupts.
//   2. Clears SCTLR.M/C/I/Z (MMU, D-cache, I-cache, branch prediction OFF).
//   3. Invalidates I-cache and TLB.
//   4. Sets SP = __stack_top.
//   5. Zeros .bss.
//   6. Branches to rust_main.
//
// Running with caches off costs us CPU perf but is by far the simplest
// safe default for bring-up — DMA-to-DDR (LCDC reading framebuffer) needs
// coherent memory anyway. A later pass can enable I-cache + D-cache with
// appropriate inner/outer shareable mappings.
// ---------------------------------------------------------------------------

core::arch::global_asm!(
    r#"
    .section .text._start, "ax"
    .arm
    .globl _start
    .type _start, %function
_start:
    // --- switch to SVC, mask A/I/F ---
    mov     r0, #0xD3              // SVC (0x13) | I-mask (0x80) | F-mask (0x40)
    msr     cpsr_c, r0

    // --- SCTLR: MMU/C/I/Z off ---
    mrc     p15, 0, r0, c1, c0, 0
    bic     r0, r0, #(1 << 0)      // M: MMU
    bic     r0, r0, #(1 << 2)      // C: D-cache
    bic     r0, r0, #(1 << 12)     // I: I-cache
    bic     r0, r0, #(1 << 11)     // Z: branch prediction
    mcr     p15, 0, r0, c1, c0, 0
    isb

    // --- invalidate I-cache and TLB ---
    mov     r0, #0
    mcr     p15, 0, r0, c7, c5, 0  // ICIALLU
    mcr     p15, 0, r0, c8, c7, 0  // TLBIALL
    dsb
    isb

    // --- install our vector table at VBAR ---
    // Without this, any abort jumps to VBAR+offset with VBAR=0
    // (ROM on AM335x) — invisible hang. Our table lights a
    // distinctive LED pattern and spins so the user can tell
    // "aborted" from "stuck in a loop".
    ldr     r0, =vector_table
    mcr     p15, 0, r0, c12, c0, 0  // VBAR
    isb

    // --- stack ---
    ldr     sp, =__stack_top

    // --- zero .bss ---
    ldr     r0, =__bss_start
    ldr     r1, =__bss_end
    mov     r2, #0
1:
    cmp     r0, r1
    strlo   r2, [r0], #4
    blo     1b

    // --- into Rust ---
    bl      rust_main

    // rust_main is declared ! but belt-and-suspenders:
2:  b       2b
    .size _start, . - _start

    // --- exception vector table (8 slots, 32 bytes aligned) ---
    .align 5
    .globl vector_table
    .type vector_table, %function
vector_table:
    b       _start              // 0x00 reset
    b       undef_handler       // 0x04 undefined
    b       svc_handler         // 0x08 supervisor call
    b       pabort_handler      // 0x0C prefetch abort
    b       dabort_handler      // 0x10 data abort
    b       reserved_handler    // 0x14 (not used on ARMv7)
    b       irq_handler         // 0x18 IRQ
    b       fiq_handler         // 0x1C FIQ

    // All handlers converge on a GPIO1_SETDATAOUT write that lights
    // a unique LED pattern per vector so the user can distinguish
    // the cause visually:
    //   data abort      : USR0 + USR3 lit ( . 1 . . 1 )  = outside pair
    //   prefetch abort  : USR0 + USR2 lit (binary 0b0101)
    //   undefined instr : USR1 + USR3 lit (binary 0b1010)
    //   svc/irq/fiq/res : all 4 LEDs lit  (binary 0b1111)
    // In every case we then spin forever.
dabort_handler:
    ldr     r0, =0x4804C190        // GPIO1_CLEARDATAOUT
    ldr     r1, =0x01E00000        // mask USR0..USR3
    str     r1, [r0]
    ldr     r0, =0x4804C194        // GPIO1_SETDATAOUT
    ldr     r1, =0x01200000        // USR0 (bit21) + USR3 (bit24)
    str     r1, [r0]
    b       .

pabort_handler:
    ldr     r0, =0x4804C190
    ldr     r1, =0x01E00000
    str     r1, [r0]
    ldr     r0, =0x4804C194
    ldr     r1, =0x00A00000        // USR0 (bit21) + USR2 (bit23)
    str     r1, [r0]
    b       .

undef_handler:
    ldr     r0, =0x4804C190
    ldr     r1, =0x01E00000
    str     r1, [r0]
    ldr     r0, =0x4804C194
    ldr     r1, =0x01400000        // USR1 (bit22) + USR3 (bit24)
    str     r1, [r0]
    b       .

svc_handler:
reserved_handler:
irq_handler:
fiq_handler:
    ldr     r0, =0x4804C190
    ldr     r1, =0x01E00000
    str     r1, [r0]
    ldr     r0, =0x4804C194
    ldr     r1, =0x01E00000        // all 4 LEDs
    str     r1, [r0]
    b       .
    "#
);

// ---------------------------------------------------------------------------
// Panic handler
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    uart0::puts("\n!!! PANIC: ");
    if let Some(loc) = info.location() {
        uart0::puts(loc.file());
        uart0::puts(":");
        uart0::put_hex32(loc.line());
    } else {
        uart0::puts("<no location>");
    }
    uart0::puts("\n");
    loop {
        core::hint::spin_loop();
    }
}

// ---------------------------------------------------------------------------
// Rust entry (called by the `_start` asm preamble once the environment
// is safe: SVC mode, caches off, SP set, .bss zeroed).
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // Blink-count diagnostic: each `blink_stage(N)` blinks USR0 N times
    // at ~2 Hz so the stage number can be counted by eye regardless of
    // contrast/viewing angle. If the board stalls, the last complete
    // blink count identifies exactly which operation hung.
    //
    //   1 blink : WDT disabled, GPIO1 clock up
    //   2 blinks: LCDC + I2C2 PRCM clocks up
    //   3 blinks: LCD + I2C2 pin mux done
    //   4 blinks: framebuffer write at FB_BASE complete
    //   5 blinks: LCDC main reset pulse done
    //   6 blinks: LCDC raster init done (LCDEN set)
    //   → LED chase running: main loop reached

    // FIRST: WDT off before anything else can catch us.
    unsafe {
        bsp::wdt::disable();
        bsp::prcm::enable_gpio1();
        bsp::leds::configure();
        bsp::leds::blink_stage(1);
    }

    unsafe {
        bsp::prcm::enable_lcdc();
        bsp::prcm::enable_i2c2();
        bsp::leds::blink_stage(2);
    }

    unsafe {
        bsp::pinmux::configure_lcd_pins();
        bsp::pinmux::configure_i2c2_pins();
        bsp::leds::blink_stage(3);
    }

    unsafe {
        let fb = core::slice::from_raw_parts_mut(FB_BASE as *mut u32, (FB_BYTES / 4) as usize);
        // Vertical color-bar test pattern. For TFT24_UNPACKED the low
        // 24 bits of each 32-bit word go out to LCD_DATA[23:0] with
        // byte layout {R[23:16], G[15:8], B[7:0]}. Seeing six distinct
        // bands (RED, GREEN, BLUE, YELLOW, MAGENTA, CYAN) confirms the
        // data path — colors that come out wrong show a byte-order swap
        // needing a `*pixel.swap_bytes()` or a different `to_be/le`.
        let w = lcdc::HACTIVE as usize;
        let h = lcdc::VACTIVE as usize;
        let bar_w = w / 6;
        let bars: [u32; 6] = [
            0x00FF_0000, // red    — R=FF G=00 B=00
            0x0000_FF00, // green  — R=00 G=FF B=00
            0x0000_00FF, // blue   — R=00 G=00 B=FF
            0x00FF_FF00, // yellow — R=FF G=FF B=00
            0x00FF_00FF, // magenta— R=FF G=00 B=FF
            0x0000_FFFF, // cyan   — R=00 G=FF B=FF
        ];
        for y in 0..h {
            for x in 0..w {
                let idx = (x / bar_w).min(5);
                fb[y * w + x] = bars[idx];
            }
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        bsp::leds::blink_stage(4);
    }

    unsafe {
        use bsp::am335x::{LCDC_CLKC_RESET, reg_write};
        reg_write(LCDC_CLKC_RESET, 1 << 3);
        for _ in 0..0x1000 {
            core::hint::spin_loop();
        }
        reg_write(LCDC_CLKC_RESET, 0);
        bsp::leds::blink_stage(5);
    }

    unsafe {
        lcdc::init_raster(FB_BASE, FB_BYTES);
        bsp::leds::blink_stage(6);
    }
    let _ = uart0::puts;

    #[cfg(feature = "freertos")]
    unsafe {
        let fb1 = FB_BASE + FB_BYTES;
        freertos_entry::start(FB_BASE, fb1);
    }

    #[cfg(not(feature = "freertos"))]
    {
        // LED register-state display: cycle through 4 one-second
        // frames of LCDC/pinmux readbacks. USR0..USR3 show the low
        // nibble of each sampled value. Each frame is framed by a
        // brief all-off pulse for visual separation.
        //
        // Expected patterns if everything is configured correctly:
        //
        //   Frame A — RASTER_CTRL[3:0]    : USR0 lit       (LCDEN=1)
        //   Frame B — CONF_LCD_DATA0[3:0] : USR3 lit       (0x08 = Mode 0 + slew fast)
        //   Frame C — LCD_CTRL[3:0]       : USR0 lit       (MODESEL_RASTER=1)
        //   Frame D — CLKC_ENABLE[3:0]    : USR0 + USR2    (DMA|CORE = 0x5)
        //
        // Anything different localises which register didn't take:
        //   - Frame A blank → LCDEN didn't stick (LCDC dead)
        //   - Frame B shows 0 → pinmux writes ignored (CTRLMOD lock)
        //   - Frame A+B+C as expected but Frame D wrong → enable seq issue
        //
        // Report: "I see lit pattern X in each frame" and we narrow
        // down the next fix.

        // Knight-Rider chase — panel shows color bars; LEDs sweep
        // USR0..USR3..USR0 so it's visually obvious the board is
        // alive and EOF interrupts are firing (we clear EOF0 each
        // time a frame completes).
        let mut step: u32 = 0;
        loop {
            for _ in 0..2_000_000u32 {
                core::hint::spin_loop();
            }
            let pos = (step & 7) as usize;
            let led = if pos < 4 { pos } else { 6 - pos };
            unsafe {
                bsp::leds::set_one(led);
                if lcdc::is_eof_pending() {
                    lcdc::clear_eof_irq();
                }
            }
            step = step.wrapping_add(1);
        }
    }
}
