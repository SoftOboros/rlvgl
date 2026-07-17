// SPDX-License-Identifier: MIT
//! `no_std` driver for the Vishay VEML3235SL ambient light sensor.
//!
//! This crate implements the register map in datasheet revision 1.4. The
//! driver uses the blocking `embedded-hal` 1.0 seven-bit I2C contract and owns
//! its backend. Construction is side-effect free. After enabling or changing
//! the integration time, the caller is responsible for waiting long enough
//! for a fresh conversion before calling [`Veml3235sl::read_illuminance`].
//!
//! The sensor has no configurable I2C address. Its fixed address is [`ADDRESS`].

#![no_std]
#![deny(missing_docs)]

use embedded_hal::i2c::{I2c, SevenBitAddress};

const REG_COMMAND: u8 = 0x00;
const REG_WHITE: u8 = 0x04;
const REG_ALS: u8 = 0x05;
const REG_ID: u8 = 0x09;
const PART_NUMBER: u8 = 0x35;

/// Fixed seven-bit I2C address of every VEML3235SL.
pub const ADDRESS: SevenBitAddress = 0x10;

/// Ambient-light integration time.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum IntegrationTime {
    /// 50 ms.
    Ms50 = 0,
    /// 100 ms.
    Ms100 = 1,
    /// 200 ms.
    Ms200 = 2,
    /// 400 ms.
    Ms400 = 3,
    /// 800 ms.
    Ms800 = 4,
}

impl IntegrationTime {
    const fn base_micro_lux_per_count(self) -> u32 {
        match self {
            Self::Ms50 => 272_640,
            Self::Ms100 => 136_320,
            Self::Ms200 => 68_160,
            Self::Ms400 => 34_080,
            Self::Ms800 => 17_040,
        }
    }
}

/// Analog ALS-channel gain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Gain {
    /// Unity gain.
    X1,
    /// Two-times gain.
    X2,
    /// Four-times gain.
    X4,
}

impl Gain {
    const fn register_bits(self) -> u8 {
        match self {
            Self::X1 => 0,
            Self::X2 => 1,
            Self::X4 => 3,
        }
    }

    const fn factor(self) -> u32 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X4 => 4,
        }
    }
}

/// Additional digital ALS-channel gain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DigitalGain {
    /// Unity digital gain.
    X1,
    /// Two-times digital gain.
    X2,
}

impl DigitalGain {
    const fn register_bits(self) -> u8 {
        match self {
            Self::X1 => 0,
            Self::X2 => 1,
        }
    }

    const fn factor(self) -> u32 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
        }
    }
}

/// A complete revision-1.4 command-register configuration.
///
/// The type admits only documented integration-time and gain encodings. Both
/// shutdown bits are controlled together, and reserved high-byte bit zero is
/// always written as one as required by the datasheet.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Config {
    integration_time: IntegrationTime,
    gain: Gain,
    digital_gain: DigitalGain,
    enabled: bool,
}

impl Config {
    /// Creates an enabled configuration.
    pub const fn new(
        integration_time: IntegrationTime,
        gain: Gain,
        digital_gain: DigitalGain,
    ) -> Self {
        Self {
            integration_time,
            gain,
            digital_gain,
            enabled: true,
        }
    }

    /// Selects whether the ALS and white channels are enabled.
    ///
    /// Disabling a configuration sets both documented shutdown bits.
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns whether both measurement channels are enabled.
    pub const fn enabled(self) -> bool {
        self.enabled
    }

    /// Returns the exact datasheet resolution in micro-lux per ALS count.
    pub const fn micro_lux_per_count(self) -> u32 {
        self.integration_time.base_micro_lux_per_count()
            / self.gain.factor()
            / self.digital_gain.factor()
    }

    /// Converts one raw ALS count using this configuration's exact resolution.
    pub const fn illuminance_from_raw(self, raw_count: u16) -> Illuminance {
        Illuminance {
            raw_count,
            micro_lux: raw_count as u64 * self.micro_lux_per_count() as u64,
        }
    }

    const fn command_bytes(self) -> [u8; 3] {
        let shutdown_low = if self.enabled { 0 } else { 0x01 };
        let shutdown_high = if self.enabled { 0 } else { 0x80 };
        let low = (self.integration_time as u8) << 4 | shutdown_low;
        let high = 0x01
            | (self.digital_gain.register_bits() << 5)
            | (self.gain.register_bits() << 3)
            | shutdown_high;
        [REG_COMMAND, low, high]
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(IntegrationTime::Ms100, Gain::X1, DigitalGain::X1)
    }
}

/// An exact ambient-light result.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Illuminance {
    raw_count: u16,
    micro_lux: u64,
}

impl Illuminance {
    /// Returns the raw 16-bit ALS channel count.
    pub const fn raw_count(self) -> u16 {
        self.raw_count
    }

    /// Returns the converted illuminance in micro-lux.
    pub const fn micro_lux(self) -> u64 {
        self.micro_lux
    }
}

/// A driver operation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    /// The underlying I2C backend failed.
    Bus(E),
    /// The ID register did not contain the VEML3235SL part number.
    InvalidDevice {
        /// Required low ID byte.
        expected: u8,
        /// Low ID byte returned by the addressed device.
        actual: u8,
    },
    /// An illuminance read was requested before configuration succeeded.
    NotConfigured,
    /// An illuminance read was requested while both channels were shut down.
    Shutdown,
}

/// A VEML3235SL driver owning one `embedded-hal` I2C handle.
#[derive(Debug)]
pub struct Veml3235sl<I2C> {
    i2c: I2C,
    config: Option<Config>,
}

impl<I2C> Veml3235sl<I2C> {
    /// Creates a side-effect-free driver at the sensor's fixed address.
    pub const fn new(i2c: I2C) -> Self {
        Self { i2c, config: None }
    }

    /// Returns the sensor's fixed address.
    pub const fn address(&self) -> SevenBitAddress {
        ADDRESS
    }

    /// Returns whether a complete command configuration was written.
    pub const fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// Returns the last completely written configuration.
    pub const fn config(&self) -> Option<Config> {
        self.config
    }

    /// Releases and returns the owned I2C handle.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C> Veml3235sl<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Verifies that the low ID byte contains part number `0x35`.
    ///
    /// The high byte is reserved and therefore intentionally ignored.
    pub fn probe(&mut self) -> Result<(), Error<I2C::Error>> {
        let identity = self.read_word(REG_ID)?;
        let actual = identity.to_le_bytes()[0];
        if actual == PART_NUMBER {
            Ok(())
        } else {
            Err(Error::InvalidDevice {
                expected: PART_NUMBER,
                actual,
            })
        }
    }

    /// Writes a complete command-register configuration.
    ///
    /// A failed write invalidates the cached state, preventing later lux
    /// conversion from assuming that the hardware accepted uncertain settings.
    pub fn configure(&mut self, config: Config) -> Result<(), Error<I2C::Error>> {
        self.config = None;
        self.i2c
            .write(ADDRESS, &config.command_bytes())
            .map_err(Error::Bus)?;
        self.config = Some(config);
        Ok(())
    }

    /// Reads the raw 16-bit white-channel count.
    ///
    /// Raw reads are allowed before configuration and while shut down; in those
    /// states the device may return a reset or stale value.
    pub fn read_white_raw(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_word(REG_WHITE)
    }

    /// Reads the raw 16-bit ambient-light-channel count.
    ///
    /// Raw reads are allowed before configuration and while shut down; in those
    /// states the device may return a reset or stale value.
    pub fn read_als_raw(&mut self) -> Result<u16, Error<I2C::Error>> {
        self.read_word(REG_ALS)
    }

    /// Reads ALS data and converts it with the last active configuration.
    ///
    /// This method performs no I2C transaction unless configuration previously
    /// succeeded and both measurement channels are enabled.
    pub fn read_illuminance(&mut self) -> Result<Illuminance, Error<I2C::Error>> {
        let Some(config) = self.config else {
            return Err(Error::NotConfigured);
        };
        if !config.enabled() {
            return Err(Error::Shutdown);
        }

        let raw_count = self.read_als_raw()?;
        Ok(config.illuminance_from_raw(raw_count))
    }

    fn read_word(&mut self, register: u8) -> Result<u16, Error<I2C::Error>> {
        let mut bytes = [0; 2];
        self.i2c
            .write_read(ADDRESS, &[register], &mut bytes)
            .map_err(Error::Bus)?;
        Ok(u16::from_le_bytes(bytes))
    }
}
