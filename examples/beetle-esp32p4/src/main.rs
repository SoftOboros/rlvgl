//! rlvgl showcase scaffold for the DFRobot DFR1172 FireBeetle 2 ESP32-P4 AI
//! Vision Board driving a Riverdi RVT70HSMFWN00 (7", 1024×600 IPS, cap-touch)
//! over the board's 2-lane MIPI-DSI interface.
//!
//! This is a **bare-metal** scaffold against the `esp32p4` PAC — no HAL, no
//! ESP-IDF. The register-level bring-up is done against the ESP32-P4 TRM
//! ingested in memalpha notebook 15. Same pattern as
//! `examples/stm32h747i-disco/src/main.rs`.

#![no_std]
#![no_main]

use esp32p4::Peripherals;
use esp_riscv_rt::entry;
use panic_halt as _;

#[entry]
fn main() -> ! {
    let _dp = Peripherals::take().unwrap();

    // TODO(TRM §System Clock): bring up the HP CPU clock tree (set CPU to
    //                          360 MHz from the PLL).
    // TODO(TRM §IO MUX + GPIO Matrix): configure a status LED pin and blink
    //                                  it so we have a life sign.
    // TODO(TRM §PSRAM Controller): enable the 32 MB on-board PSRAM so we can
    //                              allocate a 1024×600×2 framebuffer there.
    // TODO(TRM §MIPI-DSI Host): bring up the DSI host PHY + link layer, push
    //                           the EK79007AD3 panel init command sequence,
    //                           and start streaming pixels from PSRAM.
    // TODO(TRM §I2C): bring up I2C0 for the Riverdi cap-touch controller
    //                 (ILI2132A @ 0x41, 400 kHz).
    // TODO(rlvgl): implement a new rlvgl_platform::mipi_dsi renderer and wire
    //              it up here.

    loop {
        // Wait for interrupt. Using inline asm rather than pulling in the
        // `riscv` crate as a direct dep (it's already available transitively
        // via `esp-riscv-rt`, but we don't need any of its higher-level API).
        unsafe { core::arch::asm!("wfi") };
    }
}
