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

    /// Release the underlying I²C peripheral.
    pub fn release(self) -> I2C {
        self.i2c
    }
}
