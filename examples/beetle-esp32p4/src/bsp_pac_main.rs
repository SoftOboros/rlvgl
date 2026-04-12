//! Raw PAC BSP path for DFR1172 FireBeetle 2 ESP32-P4.
//!
//! Uses the generated BSP from `bsp_generated/` to bring up clocks, IO MUX,
//! and peripheral init, then blinks the LED via raw GPIO register writes.
//!
//! Regenerate the BSP with:
//! ```sh
//! cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
//!     --vendor esp --board beetle_esp32p4 --chip esp32p4 \
//!     --out examples/beetle-esp32p4/src/bsp_generated_raw --emit-pac
//! ```

#![no_std]
#![no_main]

use esp_riscv_rt::entry;
use esp32p4::Peripherals;
use panic_halt as _;

mod bsp_generated;

#[entry]
fn main() -> ! {
    let _dp = Peripherals::take().unwrap();

    bsp_generated::init();

    let led_mask = 1u32 << bsp_generated::board::LED;

    // TODO(TRM §PSRAM Controller): enable the 32 MB on-board PSRAM so we can
    //                              allocate a 1024×600×2 framebuffer there.
    // TODO(TRM §MIPI-DSI Host): bring up the DSI host PHY + link layer, push
    //                           the EK79007AD3 panel init command sequence,
    //                           and start streaming pixels from PSRAM.
    // TODO(rlvgl): implement a new rlvgl_platform::mipi_dsi renderer and wire
    //              it up here.

    loop {
        unsafe {
            let gpio = &*esp32p4::GPIO::PTR;
            gpio.out_w1ts().write(|w| w.bits(led_mask));
        }
        for _ in 0..500_000 {
            unsafe { core::arch::asm!("nop") };
        }
        unsafe {
            let gpio = &*esp32p4::GPIO::PTR;
            gpio.out_w1tc().write(|w| w.bits(led_mask));
        }
        for _ in 0..500_000 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}
