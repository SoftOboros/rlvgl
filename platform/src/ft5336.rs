//! Minimal driver for the FT5336 capacitive touch controller.
//!
//! Communicates over I²C to retrieve touch coordinates from the controller.

use embedded_hal::i2c::{I2c, SevenBitAddress};

/// FT5336 touch controller driver.
pub struct Ft5336<I2C> {
    i2c: I2C,
}

impl<I2C> Ft5336<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// 7-bit I²C address of the FT5336.
    const ADDRESS: SevenBitAddress = 0x38;

    /// Create a new driver from an I²C peripheral.
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Read the first touch point from the controller.
    ///
    /// Returns `Ok(Some((x, y)))` if a touch is detected, `Ok(None)` if no touch
    /// is present, or an I²C error.
    pub fn read_touch(&mut self) -> Result<Option<(u16, u16)>, I2C::Error> {
        let mut buf = [0u8; 5];
        // Register 0x02 contains the number of touch points (low nibble). The
        // following bytes hold X and Y for the first touch.
        self.i2c.write_read(Self::ADDRESS, &[0x02], &mut buf)?;
        let touches = buf[0] & 0x0F;
        if touches == 0 {
            return Ok(None);
        }
        let x = (((buf[1] & 0x0F) as u16) << 8) | buf[2] as u16;
        let y = (((buf[3] & 0x0F) as u16) << 8) | buf[4] as u16;
        Ok(Some((x, y)))
    }

    /// Read all active touch points from the controller.
    ///
    /// Returns `(count, points)` where `count` is `0..=5` and each entry in
    /// `points[..count]` holds `(id, event_flag, x, y)`.
    ///
    /// `event_flag`: 0 = press down, 1 = lift up, 2 = contact (held/moved).
    pub fn read_touches(&mut self) -> Result<(u8, [(u8, u8, u16, u16); 5]), I2C::Error> {
        // Bulk read registers 0x02..0x20 (31 bytes):
        //   byte 0     = TD_STATUS (touch count in low nibble)
        //   bytes 1–6  = touch point 0
        //   bytes 7–12 = touch point 1
        //   bytes 13–18 = touch point 2
        //   bytes 19–24 = touch point 3
        //   bytes 25–30 = touch point 4
        //
        // Each 6-byte touch block:
        //   [0] bits 7:6 = event flag, bits 3:0 = X high nibble
        //   [1] X low byte
        //   [2] bits 7:4 = touch ID, bits 3:0 = Y high nibble
        //   [3] Y low byte
        //   [4] weight (unused)
        //   [5] area (unused)
        let mut buf = [0u8; 31];
        self.i2c.write_read(Self::ADDRESS, &[0x02], &mut buf)?;
        let count = (buf[0] & 0x0F).min(5);
        let mut points = [(0u8, 0u8, 0u16, 0u16); 5];
        for i in 0..count as usize {
            let base = 1 + i * 6;
            let event_flag = buf[base] >> 6;
            let x = (((buf[base] & 0x0F) as u16) << 8) | buf[base + 1] as u16;
            let id = buf[base + 2] >> 4;
            let y = (((buf[base + 2] & 0x0F) as u16) << 8) | buf[base + 3] as u16;
            points[i] = (id, event_flag, x, y);
        }
        Ok((count, points))
    }

    /// Release the underlying I²C peripheral.
    pub fn release(self) -> I2C {
        self.i2c
    }
}
