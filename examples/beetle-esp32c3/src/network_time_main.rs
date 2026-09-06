//! DFR0868 Beetle ESP32-C3 entry point for the shared network-time runner.
//!
//! This file owns only chip initialization and the board's typed GPIO mapping.
//! The application and ESP runtime are shared with the DFR1117 ESP32-C6.

#![no_std]
#![no_main]
#![deny(missing_docs)]

extern crate alloc;

#[path = "../../common/esp_network_time.rs"]
mod esp_network_time;

use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config as I2cConfig, I2c},
    main,
    time::Rate,
};

use esp_network_time::WifiPeripherals;

#[used]
#[unsafe(export_name = "esp_app_desc")]
#[unsafe(link_section = ".rodata_desc.appdesc")]
static ESP_APP_DESC: esp_bootloader_esp_idf::EspAppDesc =
    esp_bootloader_esp_idf::EspAppDesc::new_internal(
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_NAME"),
        esp_bootloader_esp_idf::BUILD_TIME,
        esp_bootloader_esp_idf::BUILD_DATE,
        "0.0.0",
        0,
        u16::MAX,
        esp_bootloader_esp_idf::MMU_PAGE_SIZE,
    );

/// Initialize the DFR0868 bus and enter the common 1 Hz network-time runtime.
#[main]
fn network_time_main() -> ! {
    esp_alloc::heap_allocator!(size: esp_network_time::HEAP_SIZE);
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(100)),
    )
    .expect("i2c init")
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO9);

    esp_network_time::run(
        i2c,
        WifiPeripherals {
            timer_group: peripherals.TIMG0,
            rng: peripherals.RNG,
            radio_clock: peripherals.RADIO_CLK,
            wifi: peripherals.WIFI,
        },
    )
}
