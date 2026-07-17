// SPDX-License-Identifier: MIT
//! `no_std` control-plane driver for the TI PCM3168A audio codec.
//!
//! The driver owns a blocking `embedded-hal` 1.0 seven-bit I2C handle. Its
//! typestate keeps all bus operations unavailable until the caller explicitly
//! attests that the supply rails are stable, external `RST` is released, and
//! SCKI, BCK, and LRCK have the required synchronous frequency relationship.
//! GPIO and clock generation remain board-layer responsibilities.
//!
//! This crate intentionally admits only common slave audio formats, reset
//! state inspection, and system resynchronization. It does not transport audio
//! samples or control attenuation, mute, master clocks, power, or GPIO pins.

#![no_std]
#![deny(missing_docs)]

use core::marker::PhantomData;

use embedded_hal::i2c::{I2c, SevenBitAddress};

const REG_RESET_CONTROL: u8 = 0x40;
const REG_DAC_CONTROL_1: u8 = 0x41;
const REG_ADC_CONTROL_1: u8 = 0x51;

/// Seven-bit address selected by the ADR1 and ADR0 straps.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Address {
    /// ADR1 low and ADR0 low. This is the fitted Kria address.
    #[default]
    BothLow = 0x44,
    /// ADR1 low and ADR0 high.
    Adr0High = 0x45,
    /// ADR1 high and ADR0 low.
    Adr1High = 0x46,
    /// ADR1 high and ADR0 high.
    BothHigh = 0x47,
}

/// Marker for a driver whose external hardware prerequisites are unverified.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Unverified;

/// Marker for a driver whose external hardware prerequisites were attested.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ready;

/// One missing prerequisite for safe control-plane access.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadinessError {
    /// Digital and analog supply rails are not known to be stable.
    RailsUnstable,
    /// External `RST` is not known to be released high.
    ResetAsserted,
    /// SCKI, BCK, and LRCK are not known to have a synchronous ratio.
    ClocksUnsynchronized,
}

/// Proof token that the board layer attested all hardware prerequisites.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HardwareReady;

impl HardwareReady {
    /// Creates a token only when all three explicit prerequisites are true.
    pub const fn new(
        rails_stable: bool,
        reset_released: bool,
        clocks_synchronous: bool,
    ) -> Result<Self, ReadinessError> {
        if !rails_stable {
            return Err(ReadinessError::RailsUnstable);
        }
        if !reset_released {
            return Err(ReadinessError::ResetAsserted);
        }
        if !clocks_synchronous {
            return Err(ReadinessError::ClocksUnsynchronized);
        }
        Ok(Self)
    }
}

/// ADC/DAC sampling-rate mode encoded in reset-control bits 1:0.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum SamplingMode {
    /// Select the rate automatically from the system/sample-clock ratio.
    Auto = 0,
    /// Force single-rate operation.
    Single = 1,
    /// Force dual-rate operation.
    Dual = 2,
    /// Force quad-rate operation.
    Quad = 3,
}

/// Audio data format supported in slave mode by both ADC and DAC interfaces.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum AudioFormat {
    /// 24-bit I2S.
    I2s24 = 0,
    /// 24-bit left-justified.
    LeftJustified24 = 1,
    /// 24-bit right-justified.
    RightJustified24 = 2,
    /// 16-bit right-justified.
    RightJustified16 = 3,
    /// 24-bit I2S-mode DSP.
    DspI2s24 = 4,
    /// 24-bit left-justified-mode DSP.
    DspLeftJustified24 = 5,
    /// 24-bit I2S-mode TDM.
    TdmI2s24 = 6,
    /// 24-bit left-justified-mode TDM.
    TdmLeftJustified24 = 7,
}

impl AudioFormat {
    /// Returns the common DAC/ADC format-field encoding.
    pub const fn register_value(self) -> u8 {
        self as u8
    }
}

/// State reported by reset-control register `0x40`.
///
/// The PCM3168A has no admitted silicon-identity register, so this probe
/// reports only a successful control-register read and its decoded state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Probe {
    reset_control: u8,
}

impl Probe {
    /// Returns whether the mode-control registers are in normal operation.
    pub const fn mode_control_normal(self) -> bool {
        self.reset_control & 0x80 != 0
    }

    /// Returns whether the ADC/DAC system is in normal operation.
    pub const fn system_normal(self) -> bool {
        self.reset_control & 0x40 != 0
    }

    /// Returns the selected ADC/DAC sampling mode.
    pub const fn sampling_mode(self) -> SamplingMode {
        match self.reset_control & 0x03 {
            0 => SamplingMode::Auto,
            1 => SamplingMode::Single,
            2 => SamplingMode::Dual,
            _ => SamplingMode::Quad,
        }
    }
}

/// A driver operation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    /// The underlying I2C backend failed.
    Bus(E),
    /// Reset-control register `0x40` had nonzero reserved bits.
    InvalidResetControl {
        /// Rejected complete register value.
        value: u8,
    },
}

/// A PCM3168A driver owning one `embedded-hal` I2C handle.
#[derive(Debug)]
pub struct Pcm3168a<I2C, State = Unverified> {
    i2c: I2C,
    address: Address,
    state: PhantomData<State>,
}

impl<I2C> Pcm3168a<I2C, Unverified> {
    /// Creates a side-effect-free unverified driver at Kria address `0x44`.
    pub const fn new(i2c: I2C) -> Self {
        Self::with_address(i2c, Address::BothLow)
    }

    /// Creates a side-effect-free unverified driver at another legal address.
    pub const fn with_address(i2c: I2C, address: Address) -> Self {
        Self {
            i2c,
            address,
            state: PhantomData,
        }
    }

    /// Consumes an explicit board-readiness token and enables I2C operations.
    pub fn into_ready(self, _token: HardwareReady) -> Pcm3168a<I2C, Ready> {
        Pcm3168a {
            i2c: self.i2c,
            address: self.address,
            state: PhantomData,
        }
    }
}

impl<I2C, State> Pcm3168a<I2C, State> {
    /// Returns the selected seven-bit address.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Releases and returns the owned I2C handle.
    pub fn release(self) -> I2C {
        self.i2c
    }
}

impl<I2C> Pcm3168a<I2C, Ready>
where
    I2C: I2c<SevenBitAddress>,
{
    /// Reads and validates reset-control register `0x40`.
    ///
    /// This is a control-plane health read, not a silicon-identity check.
    pub fn probe(&mut self) -> Result<Probe, Error<I2C::Error>> {
        self.read_reset_control()
    }

    /// Configures the DAC interface as a slave in a common audio format.
    pub fn configure_dac_interface(
        &mut self,
        format: AudioFormat,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_register(REG_DAC_CONTROL_1, format.register_value())
    }

    /// Configures the ADC interface as a slave in a common audio format.
    pub fn configure_adc_interface(
        &mut self,
        format: AudioFormat,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_register(REG_ADC_CONTROL_1, format.register_value())
    }

    /// Triggers ADC/DAC system resynchronization while preserving rate mode.
    ///
    /// This clears only SRST, keeps MRST in normal operation, and preserves the
    /// validated sampling-mode bits. The device auto-restores SRST. TI warns
    /// that resynchronization can generate pop noise; the caller owns muting
    /// and the required post-resynchronization settling time.
    pub fn resynchronize(&mut self) -> Result<(), Error<I2C::Error>> {
        let probe = self.read_reset_control()?;
        self.write_register(REG_RESET_CONTROL, 0x80 | probe.sampling_mode() as u8)
    }

    fn read_reset_control(&mut self) -> Result<Probe, Error<I2C::Error>> {
        let mut value = [0];
        self.i2c
            .write_read(
                self.address as SevenBitAddress,
                &[REG_RESET_CONTROL],
                &mut value,
            )
            .map_err(Error::Bus)?;
        if value[0] & 0x3c != 0 {
            return Err(Error::InvalidResetControl { value: value[0] });
        }
        Ok(Probe {
            reset_control: value[0],
        })
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.address as SevenBitAddress, &[register, value])
            .map_err(Error::Bus)
    }
}
