//! rlvgl demo for the DFRobot DFR0868 Beetle ESP32-C3 driving an SSD1306
//! 128x64 I2C OLED (SDA=GPIO1, SCL=GPIO2).

#![no_std]
#![no_main]
#![deny(missing_docs)]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::main;
use esp_hal::time::Rate;

use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::Ssd1306Display;
use rlvgl_platform::display::DisplayDriver;

use ssd1306::I2CDisplayInterface;
use ssd1306::Ssd1306;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64};

const HEAP_SIZE: usize = 32 * 1024;

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

/// Entry point: init heap + peripherals, bring up SSD1306, spin a demo loop.
#[main]
fn beetle_main() -> ! {
    esp_alloc::heap_allocator!(size: HEAP_SIZE);
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("i2c init")
    .with_sda(peripherals.GPIO1)
    .with_scl(peripherals.GPIO2);

    let interface = I2CDisplayInterface::new(i2c);
    let raw = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    let mut display = Ssd1306Display::new(raw).expect("ssd1306 init");

    let delay = Delay::new();
    let white = Color(255, 255, 255, 255);
    let black = Color(0, 0, 0, 255);
    let screen = Rect {
        x: 0,
        y: 0,
        width: 128,
        height: 64,
    };

    let mut frame: u32 = 0;
    loop {
        display.fill_rect(screen, black);
        display.draw_text((2, 12), "rlvgl", white);
        display.draw_text((2, 24), "Beetle ESP32-C3", white);
        display.draw_text((2, 36), "SSD1306 128x64", white);
        // Simple activity indicator (single blinking pixel-ish bar).
        let bar = Rect {
            x: 2,
            y: 52,
            width: ((frame % 64) as i32).max(1),
            height: 4,
        };
        display.fill_rect(bar, white);
        DisplayDriver::vsync(&mut display);
        frame = frame.wrapping_add(1);
        delay.delay_millis(50u32);
    }
}
