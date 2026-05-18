//! Diagnostic: toggle GPIO0..GPIO47 simultaneously at ~1 Hz.
//!
//! Used for first-boot verification on a board where the user-LED pin is
//! unknown. If *any* LED on the board blinks, the chip is running our
//! code; the blinking pin is the user LED. If nothing blinks, the chip
//! either isn't booting our app or is trapping early.

#![no_std]
#![no_main]

use esp32p4 as pac;
use esp_riscv_rt::entry;
use panic_halt as _;

#[path = "../app_desc.rs"]
mod app_desc;

const PIN_MASK: u32 = 0x00FF_FF00; // skip 0..7 (strapping/USB) and >=24 (high pins handled below)

#[entry]
fn main() -> ! {
    let p = unsafe { pac::Peripherals::steal() };

    // Configure each candidate pin as a simple software-driven GPIO output.
    for pin in 0..=47 {
        if pin == 16 || pin == 17 {
            // USB-Serial-JTAG D+/D- — leave alone.
            continue;
        }
        // mcu_sel = 1 → simple GPIO function on every P4 pad.
        p.IO_MUX.gpio(pin).modify(|_, w| unsafe {
            w.mcu_sel().bits(1).fun_ie().clear_bit()
        });
        // Drive output from the gpio_out register (sig idx 256 = simple).
        p.GPIO
            .func_out_sel_cfg(pin)
            .modify(|_, w| unsafe { w.out_sel().bits(256) });
    }

    // Output enable for low pins (0-31) and high pins (32-47).
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(0xFFFF_FFFF) });
    p.GPIO.enable1_w1ts().write(|w| unsafe { w.bits(0x0000_FFFF) });

    loop {
        // All ON.
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(0xFFFF_FFFF) });
        p.GPIO.out1_w1ts().write(|w| unsafe { w.bits(0x0000_FFFF) });
        delay();
        // All OFF.
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(0xFFFF_FFFF) });
        p.GPIO.out1_w1tc().write(|w| unsafe { w.bits(0x0000_FFFF) });
        delay();
    }
}

fn delay() {
    for _ in 0..40_000_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
    let _ = PIN_MASK;
}
