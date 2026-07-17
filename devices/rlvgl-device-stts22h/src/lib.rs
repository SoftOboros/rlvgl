// SPDX-License-Identifier: MIT
//! `no_std` driver for the STMicroelectronics STTS22H temperature sensor.
//!
//! The driver uses the blocking `embedded-hal` 1.0 seven-bit I2C contract.
//! Construction is side-effect free. Callers must wait at least 12 ms after
//! power-on before communicating with the sensor, then call [`Stts22h::probe`]
//! and [`Stts22h::configure`] before requesting a coherent temperature value.

#![no_std]
#![deny(missing_docs)]

use embedded_hal::i2c::{I2c, SevenBitAddress};

const REG_WHO_AM_I: u8 = 0x01;
const REG_HIGH_LIMIT: u8 = 0x02;
const REG_LOW_LIMIT: u8 = 0x03;
const REG_CTRL: u8 = 0x04;
const REG_STATUS: u8 = 0x05;
const REG_TEMP_LOW: u8 = 0x06;

const WHO_AM_I_VALUE: u8 = 0xa0;

const CTRL_LOW_ODR_START: u8 = 1 << 7;
const CTRL_BDU: u8 = 1 << 6;
const CTRL_AVG_SHIFT: u8 = 4;
const CTRL_IF_ADD_INC: u8 = 1 << 3;
const CTRL_FREERUN: u8 = 1 << 2;
const CTRL_TIMEOUT_DISABLE: u8 = 1 << 1;
const CTRL_ONE_SHOT: u8 = 1;

/// A legal seven-bit STTS22H address selected by the ADDR pin connection.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Address {
    /// ADDR tied to VDD. This is the fitted Kria board address.
    #[default]
    Vdd = 0x38,
    /// ADDR pulled up to VDD through 15 kΩ ±5%.
    PullUp15K = 0x3c,
    /// ADDR pulled up to VDD through 56 kΩ ±5%.
    PullUp56K = 0x3e,
    /// ADDR tied to ground.
    Gnd = 0x3f,
}

/// Number of measurements averaged into one temperature result.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Averaging {
    /// Eight measurements; the lowest-noise setting.
    Samples8 = 0,
    /// Four measurements.
    Samples4 = 1,
    /// Two measurements.
    Samples2 = 2,
    /// One measurement; the highest-rate setting.
    Samples1 = 3,
}

/// Output rate in freerun mode.
///
/// The STTS22H couples these rates to the same CTRL bits that select averaging.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum FreeRunRate {
    /// 25 Hz with eight averaged measurements.
    Hz25 = 0,
    /// 50 Hz with four averaged measurements.
    Hz50 = 1,
    /// 100 Hz with two averaged measurements.
    Hz100 = 2,
    /// 200 Hz with one measurement.
    Hz200 = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    OneShot,
    LowOdr,
    FreeRun,
}

/// A complete safe CTRL-register configuration.
///
/// Every configuration enables block-data update and register auto-increment.
/// This makes the low-byte-first two-byte temperature read coherent and avoids
/// relying on contradictory reset-value prose in the datasheet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    mode: Mode,
    average_bits: u8,
    timeout_disabled: bool,
}

impl Config {
    /// Selects triggered one-shot conversions with the requested averaging.
    pub const fn one_shot(averaging: Averaging) -> Self {
        Self {
            mode: Mode::OneShot,
            average_bits: averaging as u8,
            timeout_disabled: false,
        }
    }

    /// Selects continuous 1 Hz low-ODR conversions.
    pub const fn low_odr(averaging: Averaging) -> Self {
        Self {
            mode: Mode::LowOdr,
            average_bits: averaging as u8,
            timeout_disabled: false,
        }
    }

    /// Selects continuous freerun conversions at the requested rate.
    pub const fn free_run(rate: FreeRunRate) -> Self {
        Self {
            mode: Mode::FreeRun,
            average_bits: rate as u8,
            timeout_disabled: false,
        }
    }

    /// Sets whether the SMBus 30 ms inactivity timeout remains enabled.
    ///
    /// It is enabled by default, matching device power-on behavior.
    pub const fn with_smbus_timeout(mut self, enabled: bool) -> Self {
        self.timeout_disabled = !enabled;
        self
    }

    /// Returns whether the SMBus inactivity timeout is enabled.
    pub const fn smbus_timeout_enabled(self) -> bool {
        !self.timeout_disabled
    }

    const fn is_one_shot(self) -> bool {
        matches!(self.mode, Mode::OneShot)
    }

    const fn control_bits(self) -> u8 {
        let mut bits = CTRL_BDU | CTRL_IF_ADD_INC | (self.average_bits << CTRL_AVG_SHIFT);
        bits |= match self.mode {
            Mode::OneShot => 0,
            Mode::LowOdr => CTRL_LOW_ODR_START,
            Mode::FreeRun => CTRL_FREERUN,
        };
        if self.timeout_disabled {
            bits |= CTRL_TIMEOUT_DISABLE;
        }
        bits
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::one_shot(Averaging::Samples8)
    }
}

/// A driver operation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    /// The underlying I2C backend failed.
    Bus(E),
    /// The WHOAMI register did not contain the STTS22H identity.
    InvalidDevice {
        /// Required WHOAMI value.
        expected: u8,
        /// Value returned by the addressed device.
        actual: u8,
    },
    /// A coherent temperature read was requested before configuration.
    NotConfigured,
    /// A one-shot trigger was requested while not configured for one-shot.
    WrongMode,
}

/// A signed STTS22H temperature sample in hundredths of a degree Celsius.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Temperature {
    centi_celsius: i16,
}

impl Temperature {
    /// Returns the raw signed hundredths of a degree Celsius.
    pub const fn centi_celsius(self) -> i16 {
        self.centi_celsius
    }
}

/// An exact threshold value rejected by the sensor's 0.64 °C encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThresholdError {
    /// Rejected value in hundredths of a degree Celsius.
    pub centi_celsius: i16,
}

/// One programmable high- or low-temperature threshold.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Threshold {
    register_value: u8,
}

impl Threshold {
    /// Creates the disabled threshold encoding.
    pub const fn disabled() -> Self {
        Self { register_value: 0 }
    }

    /// Creates an exactly representable active threshold.
    ///
    /// Accepted values range from -3968 to 12288 centi-degrees in 64
    /// centi-degree increments. The disabled value is created separately with
    /// [`Threshold::disabled`].
    pub const fn from_centi_celsius(centi_celsius: i16) -> Result<Self, ThresholdError> {
        if centi_celsius < -3968 || centi_celsius > 12288 || centi_celsius % 64 != 0 {
            return Err(ThresholdError { centi_celsius });
        }

        Self::from_valid_centi_celsius(centi_celsius)
    }

    const fn from_valid_centi_celsius(centi_celsius: i16) -> Result<Self, ThresholdError> {
        let register_value = centi_celsius / 64 + 63;
        Ok(Self {
            register_value: register_value as u8,
        })
    }

    /// Returns the exact register encoding.
    pub const fn register_value(self) -> u8 {
        self.register_value
    }

    /// Returns the active threshold in centi-degrees, or `None` when disabled.
    pub const fn centi_celsius(self) -> Option<i16> {
        if self.register_value == 0 {
            None
        } else {
            Some((self.register_value as i16 - 63) * 64)
        }
    }
}

/// Status bits returned by the read-to-clear STATUS register.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Status {
    bits: u8,
}

impl Status {
    /// Returns whether a one-shot conversion is in progress.
    pub const fn busy(self) -> bool {
        self.bits & 0x01 != 0
    }

    /// Returns whether the high threshold was reached or exceeded.
    pub const fn over_high_limit(self) -> bool {
        self.bits & 0x02 != 0
    }

    /// Returns whether the temperature fell below the low threshold.
    pub const fn under_low_limit(self) -> bool {
        self.bits & 0x04 != 0
    }
}

/// An STTS22H driver owning one `embedded-hal` I2C handle.
#[derive(Debug)]
pub struct Stts22h<I2C> {
    i2c: I2C,
    address: Address,
    config: Option<Config>,
}

impl<I2C> Stts22h<I2C> {
    /// Creates a side-effect-free driver at the Kria board address (`0x38`).
    pub const fn new(i2c: I2C) -> Self {
        Self::with_address(i2c, Address::Vdd)
    }

    /// Creates a side-effect-free driver at another legal STTS22H address.
    pub const fn with_address(i2c: I2C, address: Address) -> Self {
        Self {
            i2c,
            address,
            config: None,
        }
    }

    /// Returns the selected address.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Returns whether a CTRL configuration was completely written.
    pub const fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// Releases and returns the owned I2C handle.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C> Stts22h<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Verifies that WHOAMI contains the documented `0xa0` identity.
    pub fn probe(&mut self) -> Result<(), Error<I2C::Error>> {
        let mut identity = [0];
        self.read_register(REG_WHO_AM_I, &mut identity)?;
        if identity[0] == WHO_AM_I_VALUE {
            Ok(())
        } else {
            Err(Error::InvalidDevice {
                expected: WHO_AM_I_VALUE,
                actual: identity[0],
            })
        }
    }

    /// Writes a complete CTRL configuration using the required power-down
    /// transition before any operating-mode or ODR change.
    ///
    /// A failed write leaves the local configuration invalid so later coherent
    /// reads cannot assume uncertain hardware state.
    pub fn configure(&mut self, config: Config) -> Result<(), Error<I2C::Error>> {
        self.config = None;
        let final_bits = config.control_bits();
        let power_down_bits = final_bits & !(CTRL_LOW_ODR_START | CTRL_FREERUN | CTRL_ONE_SHOT);

        self.write_register(REG_CTRL, power_down_bits)?;
        if final_bits != power_down_bits {
            self.write_register(REG_CTRL, final_bits)?;
        }

        self.config = Some(config);
        Ok(())
    }

    /// Starts a conversion when configured for one-shot operation.
    pub fn start_one_shot(&mut self) -> Result<(), Error<I2C::Error>> {
        let Some(config) = self.config else {
            return Err(Error::WrongMode);
        };
        if !config.is_one_shot() {
            return Err(Error::WrongMode);
        }

        self.write_register(REG_CTRL, config.control_bits() | CTRL_ONE_SHOT)
    }

    /// Reads a coherent signed temperature sample.
    ///
    /// [`Stts22h::configure`] must succeed first because this operation relies
    /// on its forced block-data-update and address-auto-increment bits.
    pub fn read_temperature(&mut self) -> Result<Temperature, Error<I2C::Error>> {
        if self.config.is_none() {
            return Err(Error::NotConfigured);
        }

        let mut bytes = [0; 2];
        self.read_register(REG_TEMP_LOW, &mut bytes)?;
        Ok(Temperature {
            centi_celsius: i16::from_le_bytes(bytes),
        })
    }

    /// Reads STATUS.
    ///
    /// Reading this register clears the high/low alert flags and deasserts the
    /// ALERT output until a later conversion reasserts an active condition.
    pub fn read_status(&mut self) -> Result<Status, Error<I2C::Error>> {
        let mut bits = [0];
        self.read_register(REG_STATUS, &mut bits)?;
        Ok(Status { bits: bits[0] })
    }

    /// Programs or disables the high-temperature alert threshold.
    pub fn set_high_threshold(&mut self, threshold: Threshold) -> Result<(), Error<I2C::Error>> {
        self.write_register(REG_HIGH_LIMIT, threshold.register_value())
    }

    /// Programs or disables the low-temperature alert threshold.
    pub fn set_low_threshold(&mut self, threshold: Threshold) -> Result<(), Error<I2C::Error>> {
        self.write_register(REG_LOW_LIMIT, threshold.register_value())
    }

    fn read_register(
        &mut self,
        register: u8,
        destination: &mut [u8],
    ) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write_read(self.address as SevenBitAddress, &[register], destination)
            .map_err(Error::Bus)
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.address as SevenBitAddress, &[register, value])
            .map_err(Error::Bus)
    }
}
