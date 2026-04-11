//! rlvgl "low end" counterpoint: DFRobot Beetle BLE (DFR0339) driving an
//! SSD1306 128x64 I2C OLED from an ATmega328P over hardware TWI on PC4/PC5
//! (Arduino A4/A5).
//!
//! This crate deliberately does **not** link rlvgl core — on 2 KB of SRAM
//! there's no room for `alloc::Vec` / widget trees after the 1024-byte mono
//! framebuffer. Instead it drives the exact same SSD1306 module the ESP32-C3
//! example uses, via `embedded-graphics`, so the two boards can be A/B'd on
//! identical hardware.

#![no_std]
#![no_main]

use arduino_hal::I2c;
use arduino_hal::prelude::*;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use panic_halt as _;
use ssd1306::mode::DisplayConfig;
use ssd1306::prelude::{DisplayRotation, DisplaySize128x64};
use ssd1306::{I2CDisplayInterface, Ssd1306};

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    // Hardware TWI on A4 (SDA / PC4) and A5 (SCL / PC5).
    // DFR0339 Beetle BLE exposes both pins on the gilded edge pads.
    let i2c = I2c::new(
        dp.TWI,
        pins.a4.into_pull_up_input(),
        pins.a5.into_pull_up_input(),
        100_000,
    );

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().unwrap_infallible();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline("rlvgl", Point::new(2, 2), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap_infallible();
    Text::with_baseline(
        "Beetle BLE (AVR)",
        Point::new(2, 14),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap_infallible();
    Text::with_baseline(
        "ATmega328P, 2KB SRAM",
        Point::new(2, 26),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap_infallible();
    Text::with_baseline(
        "SSD1306 128x64",
        Point::new(2, 38),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap_infallible();

    display.flush().unwrap_infallible();

    loop {
        arduino_hal::delay_ms(1000);
    }
}
