// SPDX-License-Identifier: MIT
//! `no_std` control-plane driver for the NXP PTN3460 eDP-to-LVDS bridge.
//!
//! The driver uses the blocking `embedded-hal` 1.0 seven-bit I2C contract and
//! owns its backend. NXP's PTN3460 literature expresses slave addresses as
//! eight-bit address bytes: `0x40` for DEV_CFG low and `0xc0` for DEV_CFG open.
//! This crate normalizes them to the seven-bit values `0x20` and `0x60`.
//!
//! The admitted configuration surface is intentionally narrow. It covers the
//! configuration-table validity marker and LVDS electrical register `0x82`.
//! EDID, flash, pin override, panel timing, and broader link configuration are
//! outside this crate's current evidence-backed surface.

#![no_std]
#![deny(missing_docs)]

use embedded_hal::i2c::{I2c, SevenBitAddress};

const REG_LVDS_ELECTRICAL: u8 = 0x82;
const REG_CONFIGURATION_MAGIC: u8 = 0xec;
const VALID_CONFIGURATION_MAGIC: u32 = 0x1234_5678;

/// PTN3460 address selected by the DEV_CFG pin.
///
/// Values are normalized seven-bit addresses suitable for `embedded-hal`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Address {
    /// DEV_CFG pulled low; NXP's eight-bit write-address byte is `0x40`.
    #[default]
    DevCfgLow = 0x20,
    /// DEV_CFG left open; NXP's eight-bit write-address byte is `0xc0`.
    DevCfgOpen = 0x60,
}

/// LVDS clock-frequency center-spreading depth.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ClockSpread {
    /// Center spreading disabled.
    Disabled = 0,
    /// 0.5 percent center spreading.
    HalfPercent = 1,
    /// 1.0 percent center spreading.
    OnePercent = 2,
    /// 1.5 percent center spreading.
    OneAndHalfPercent = 3,
    /// 2.0 percent center spreading.
    TwoPercent = 4,
    /// 2.5 percent center spreading.
    TwoAndHalfPercent = 5,
}

/// LVDS differential output swing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum OutputSwing {
    /// 150 mV.
    Millivolts150 = 0,
    /// 200 mV.
    Millivolts200 = 1,
    /// 250 mV.
    Millivolts250 = 2,
    /// 300 mV.
    Millivolts300 = 3,
    /// 350 mV.
    Millivolts350 = 4,
    /// 400 mV.
    Millivolts400 = 5,
    /// 450 mV.
    Millivolts450 = 6,
}

/// One legal value of LVDS electrical configuration register `0x82`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LvdsElectricalConfig {
    clock_spread: ClockSpread,
    output_swing: OutputSwing,
}

impl LvdsElectricalConfig {
    /// Creates an electrical configuration from legal typed fields.
    pub const fn new(clock_spread: ClockSpread, output_swing: OutputSwing) -> Self {
        Self {
            clock_spread,
            output_swing,
        }
    }

    /// Returns the selected center-spreading depth.
    pub const fn clock_spread(self) -> ClockSpread {
        self.clock_spread
    }

    /// Returns the selected differential output swing.
    pub const fn output_swing(self) -> OutputSwing {
        self.output_swing
    }

    /// Returns the exact register `0x82` encoding.
    pub const fn register_value(self) -> u8 {
        (self.clock_spread as u8) << 3 | self.output_swing as u8
    }

    /// Decodes register `0x82`, rejecting every reserved encoding.
    pub const fn from_register_value<E>(value: u8) -> Result<Self, Error<E>> {
        if value & 0xc0 != 0 {
            return Err(Error::InvalidElectricalConfig { value });
        }

        let clock_spread = match (value >> 3) & 0x07 {
            0 => ClockSpread::Disabled,
            1 => ClockSpread::HalfPercent,
            2 => ClockSpread::OnePercent,
            3 => ClockSpread::OneAndHalfPercent,
            4 => ClockSpread::TwoPercent,
            5 => ClockSpread::TwoAndHalfPercent,
            _ => return Err(Error::InvalidElectricalConfig { value }),
        };
        let output_swing = match value & 0x07 {
            0 => OutputSwing::Millivolts150,
            1 => OutputSwing::Millivolts200,
            2 => OutputSwing::Millivolts250,
            3 => OutputSwing::Millivolts300,
            4 => OutputSwing::Millivolts350,
            5 => OutputSwing::Millivolts400,
            6 => OutputSwing::Millivolts450,
            _ => return Err(Error::InvalidElectricalConfig { value }),
        };

        Ok(Self::new(clock_spread, output_swing))
    }
}

impl Default for LvdsElectricalConfig {
    fn default() -> Self {
        Self::new(ClockSpread::Disabled, OutputSwing::Millivolts300)
    }
}

/// Result of a safe configuration-table probe.
///
/// PTN3460 documentation does not admit a silicon-identity register. This
/// value proves only that the addressed target completed the documented read
/// and reports whether its flashed configuration marker is valid.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Probe {
    configuration_magic: u32,
}

impl Probe {
    /// Returns the four-byte big-endian configuration marker read from `0xec`.
    pub const fn configuration_magic(self) -> u32 {
        self.configuration_magic
    }

    /// Returns whether the configuration marker equals `0x12345678`.
    pub const fn configuration_valid(self) -> bool {
        self.configuration_magic == VALID_CONFIGURATION_MAGIC
    }
}

/// A driver operation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    /// The underlying I2C backend failed.
    Bus(E),
    /// Register `0x82` contained a reserved encoding.
    InvalidElectricalConfig {
        /// Rejected complete register value.
        value: u8,
    },
}

/// A PTN3460 driver owning one `embedded-hal` I2C handle.
#[derive(Debug)]
pub struct Ptn3460<I2C> {
    i2c: I2C,
    address: Address,
}

impl<I2C> Ptn3460<I2C> {
    /// Creates a side-effect-free driver for the Kria DEV_CFG-low strap.
    pub const fn new(i2c: I2C) -> Self {
        Self::with_address(i2c, Address::DevCfgLow)
    }

    /// Creates a side-effect-free driver for either documented slave strap.
    pub const fn with_address(i2c: I2C, address: Address) -> Self {
        Self { i2c, address }
    }

    /// Returns the selected normalized seven-bit address.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Releases and returns the owned I2C handle.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C> Ptn3460<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Reads the configuration-table validity marker.
    ///
    /// A completed read is not a silicon-identity check; inspect
    /// [`Probe::configuration_valid`] to distinguish valid flashed table state.
    pub fn probe(&mut self) -> Result<Probe, Error<I2C::Error>> {
        let mut bytes = [0; 4];
        self.i2c
            .write_read(
                self.address as SevenBitAddress,
                &[REG_CONFIGURATION_MAGIC],
                &mut bytes,
            )
            .map_err(Error::Bus)?;
        Ok(Probe {
            configuration_magic: u32::from_be_bytes(bytes),
        })
    }

    /// Reads and validates LVDS electrical register `0x82`.
    pub fn read_lvds_electrical(&mut self) -> Result<LvdsElectricalConfig, Error<I2C::Error>> {
        let mut value = [0];
        self.i2c
            .write_read(
                self.address as SevenBitAddress,
                &[REG_LVDS_ELECTRICAL],
                &mut value,
            )
            .map_err(Error::Bus)?;
        LvdsElectricalConfig::from_register_value::<I2C::Error>(value[0])
    }

    /// Writes only LVDS electrical register `0x82`.
    pub fn configure_lvds_electrical(
        &mut self,
        config: LvdsElectricalConfig,
    ) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(
                self.address as SevenBitAddress,
                &[REG_LVDS_ELECTRICAL, config.register_value()],
            )
            .map_err(Error::Bus)
    }
}
