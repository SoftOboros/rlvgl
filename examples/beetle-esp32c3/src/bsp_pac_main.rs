//! Raw-PAC path for the DFR0868 Beetle ESP32-C3.
//!
//! This binary proves the full generator pipeline end-to-end:
//!
//! 1. `chipdb/rlvgl-chips-esp/db/chips/esp32c3.yaml` +
//!    `chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml` describe the
//!    board.
//! 2. `rlvgl-creator bsp from-yaml --vendor esp --board beetle_esp32c3`
//!    mechanically emits the six BSP files under `src/bsp_generated/`.
//! 3. `crate::bsp_generated::init()` brings up SYSTEM clock gates, IO MUX +
//!    GPIO matrix routing, and per-peripheral init (including `init_i2c0()`
//!    at 400 kHz from XTAL_HZ — see the peripherals template branch added
//!    in M2).
//! 4. This `main` then blinks the LED on GPIO8 using raw PAC access to
//!    prove the board actually boots and runs the generated bring-up.
//!
//! Scope note: this is **not** a full SSD1306 driver. The BufferedGraphics
//! flush in `ssd1306` sends ~1 KB in a single I2C write, which exceeds the
//! ESP32-C3 I2C TX FIFO (32 bytes) and requires command-list chunking via
//! the `END` opcode. That transport is deferred to a follow-up milestone.
//! The purpose of this first-pass binary is to prove the generator →
//! compile → boot pipeline, not to replicate the `esp_hal` path's features.

#![no_std]
#![no_main]

use esp_riscv_rt::entry;
use panic_halt as _;

#[path = "bsp_generated/mod.rs"]
mod bsp_generated;

use bsp_generated::board::LED;
use esp32c3 as pac;

/// Very rough microsecond-ish busy wait. CPU runs at ~160 MHz by default
/// on boot; each `nop` is roughly one cycle, so ~160 cycles ≈ 1 µs. This
/// is only used for an LED blink duty cycle and does not need to be
/// accurate.
fn busy_wait(iters: u32) {
    for _ in 0..iters {
        unsafe { core::arch::asm!("nop") };
    }
}

#[entry]
fn main() -> ! {
    // Drive the full generator-produced bring-up: SYSTEM clock gates +
    // IO MUX + GPIO matrix + init_i2c0() + init_usb_sj().
    bsp_generated::init();

    // Grab the GPIO peripheral for direct output toggling. The generator
    // already configured GPIO8 as software-driven output via `io_mux::init`
    // + `GPIO.enable_w1ts(1 << 8)`, so we just set and clear the output
    // via `out_w1ts`/`out_w1tc`.
    let p = unsafe { pac::Peripherals::steal() };
    let mask: u32 = 1u32 << LED;

    loop {
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
        busy_wait(80_000_000); // ~0.5 s at 160 MHz
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
        busy_wait(80_000_000);
    }
}
