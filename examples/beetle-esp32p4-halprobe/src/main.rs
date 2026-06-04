//! BEETLE-03s: esp-hal reference probe for ERRATA-008.
//!
//! LED blink count on GPIO 3 (active-high — HIGH = LED ON):
//!   4 fast pulses then 1 short blink loop = I2C write succeeded
//!   4 fast pulses then 2 short blinks loop = I2C write failed
//!   nothing                               = panic before init

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_println::println;

use esp_hal::{
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    i2c::{self, master::I2c},
    main,
    peripherals::Peripherals,
    time::Rate,
};

const BRIDGE_ADDR: u8 = 0x45;

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn entry() -> ! {
    println!("halprobe: before esp_hal::init");
    let peripherals: Peripherals = esp_hal::init(esp_hal::Config::default());
    println!("halprobe: after esp_hal::init");

    let mut led = Output::new(peripherals.GPIO3, Level::Low, OutputConfig::default());
    println!("halprobe: LED GPIO3 configured");
    let delay = Delay::new();
    println!("halprobe: Delay created");

    for _ in 0..4 {
        led.set_high();
        delay.delay_millis(80u32);
        led.set_low();
        delay.delay_millis(80u32);
    }
    delay.delay_millis(400u32);

    println!("halprobe: constructing I2C0 at 100 kHz");
    let i2c_result = I2c::new(
        peripherals.I2C0,
        i2c::master::Config::default().with_frequency(Rate::from_khz(100)),
    );

    let blinks: u32 = match i2c_result {
        Ok(i2c) => {
            println!("halprobe: I2C0 constructed, attaching pins SDA=7 SCL=8");
            let mut i2c = i2c.with_sda(peripherals.GPIO7).with_scl(peripherals.GPIO8);
            println!("halprobe: writing [0x07, 0x01] to addr 0x{:02x}", BRIDGE_ADDR);
            match i2c.write(BRIDGE_ADDR, &[0x07, 0x01]) {
                Ok(()) => {
                    println!("halprobe: WRITE OK — slave ACKed");
                    1
                }
                Err(e) => {
                    println!("halprobe: WRITE ERR — {:?}", e);
                    2
                }
            }
        }
        Err(e) => {
            println!("halprobe: I2C construction failed — {:?}", e);
            3
        }
    };
    println!("halprobe: entering blink loop with {} blinks/cycle", blinks);

    loop {
        for _ in 0..blinks {
            led.set_high();
            delay.delay_millis(200u32);
            led.set_low();
            delay.delay_millis(200u32);
        }
        delay.delay_millis(1500u32);
    }
}
