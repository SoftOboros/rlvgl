//! Pi-7"-Atmel-bridge wake protocol (I2C @ 0x45).
//!
//! The DFR0550-V2's on-panel STM32F072 emulates the original Atmel
//! ATTINY88 bridge that the Raspberry Pi 7" Touchscreen v1 uses. The
//! Linux kernel driver
//! `drivers/gpu/drm/panel/panel-raspberrypi-touchscreen.c` is the
//! authoritative reference for the register layout.
//!
//! Verified-working sequence (IDF reference):
//!
//! ```c
//! bridge_write(bridge, REG_POWERON, 1);
//! vTaskDelay(20 ms);
//! while (!(bridge_read(bridge, REG_PORTB) & 0x01)) { delay(10 ms); }
//! bridge_write(bridge, REG_PORTA, 0x04);  // kernel "match closed-source firmware"
//! bridge_write(bridge, REG_PWM, 255);
//! ```

#![allow(dead_code)]

use super::i2c0::{self, I2cError};

pub const BRIDGE_ADDR: u8 = 0x45;
pub const REG_PORTA: u8 = 0x81;
pub const REG_PORTB: u8 = 0x82;
pub const REG_POWERON: u8 = 0x85;
pub const REG_PWM: u8 = 0x86;

/// Kernel "match closed-source firmware orientation" PORTA value. Required
/// for the DFR0550-V2 bridge to actually drive the TFT after power-on.
pub const PORTA_KERNEL_DEFAULT: u8 = 0x04;

/// Wake the panel bridge. Returns when `PORTB & 0x01` reads true and the
/// backlight has been enabled at full PWM.
///
/// # Safety
/// I2C0 must already be initialized via the BSP, and pins routed via
/// [`super::i2c0::route_pins`].
pub unsafe fn wake() -> Result<(), BridgeError> {
    // 1. Power on the panel.
    i2c0::write_reg(BRIDGE_ADDR, REG_POWERON, 1)?;

    // 2. Coarse delay (~20 ms at 360 MHz CPU; this is a hot spin so we
    //    don't have a ticker yet — adequate for the 20 ms order of magnitude).
    delay_loops(7_200_000);

    // 3. Poll PORTB until bridge reports ready (bit 0 = panel powered).
    let mut tries = 100;
    loop {
        match i2c0::read_reg(BRIDGE_ADDR, REG_PORTB) {
            Ok(pb) if pb & 0x01 != 0 => break,
            Ok(_) => {}
            Err(I2cError::Nack) => {
                // Bridge not on the bus yet — keep retrying.
            }
            Err(e) => return Err(BridgeError::I2c(e)),
        }
        tries -= 1;
        if tries == 0 {
            return Err(BridgeError::NotReady);
        }
        delay_loops(3_600_000); // ~10 ms
    }

    // 4. Set orientation flag, full backlight.
    i2c0::write_reg(BRIDGE_ADDR, REG_PORTA, PORTA_KERNEL_DEFAULT)?;
    i2c0::write_reg(BRIDGE_ADDR, REG_PWM, 255)?;
    Ok(())
}

fn delay_loops(n: u32) {
    for _ in 0..n {
        unsafe { core::arch::asm!("nop") };
    }
}

#[derive(Debug)]
pub enum BridgeError {
    NotReady,
    I2c(I2cError),
}

impl From<I2cError> for BridgeError {
    fn from(e: I2cError) -> Self {
        BridgeError::I2c(e)
    }
}
