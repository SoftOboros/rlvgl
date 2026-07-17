// SPDX-License-Identifier: MIT
//! Kria logical I2C topology and backend adapters.
//!
//! Device drivers consume ordinary [`embedded_hal::i2c::I2c`] handles. This
//! crate names stable board roles, records evidence-admitted fitted addresses,
//! and supplies sharing/backend adapters without coupling leaf drivers to a
//! Kria controller or Linux adapter number.

#![no_std]
#![deny(missing_docs)]

#[cfg(feature = "std")]
extern crate std;

use core::cell::RefCell;

use embedded_hal::i2c::{I2c, SevenBitAddress};
use embedded_hal_bus::i2c::RefCellDevice;

/// Stable logical roles for the three admitted board I2C buses.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LogicalBus {
    /// PS I2C0 on `PS_MIO46`/`PS_MIO47`.
    PsI2c0,
    /// PS I2C1 on `PS_MIO32`/`PS_MIO33`.
    PsI2c1,
    /// Shared FPGA front-panel bus behind the PCA9306 level translator.
    PlFrontPanelI2c,
}

/// An evidence-admitted fitted device endpoint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DeviceEndpoint {
    bus: LogicalBus,
    address: SevenBitAddress,
}

impl DeviceEndpoint {
    /// Creates an endpoint from a logical bus and seven-bit address.
    pub const fn new(bus: LogicalBus, address: SevenBitAddress) -> Self {
        Self { bus, address }
    }

    /// Returns the logical bus carrying the device.
    pub const fn bus(self) -> LogicalBus {
        self.bus
    }

    /// Returns the device's seven-bit I2C address.
    pub const fn address(self) -> SevenBitAddress {
        self.address
    }
}

/// STTS22H U35 on PS I2C1.
pub const STTS22H: DeviceEndpoint = DeviceEndpoint::new(LogicalBus::PsI2c1, 0x38);
/// EEPROM U6 on PS I2C0; its protocol remains evidence-gated.
pub const EEPROM_U6: DeviceEndpoint = DeviceEndpoint::new(LogicalBus::PsI2c0, 0x50);
/// VEML3235SL U4 on the shared PL front-panel bus.
pub const VEML3235SL: DeviceEndpoint = DeviceEndpoint::new(LogicalBus::PlFrontPanelI2c, 0x10);
/// PTN3460 U7 on the shared PL front-panel bus.
///
/// NXP literature writes the DEV_CFG-low address as the eight-bit write byte
/// `0x40`. `embedded-hal` takes the normalized seven-bit address, `0x20`.
pub const PTN3460: DeviceEndpoint = DeviceEndpoint::new(LogicalBus::PlFrontPanelI2c, 0x20);
/// PCM3168A U2 on the shared PL front-panel bus.
pub const PCM3168A: DeviceEndpoint = DeviceEndpoint::new(LogicalBus::PlFrontPanelI2c, 0x44);

/// Caller-owned mapping from logical Kria buses to physical backend identifiers.
///
/// The identifier type is intentionally generic. Linux callers commonly use
/// paths, while bare-metal integrations can use controller indices or board
/// support package identifiers.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PhysicalBusMap<P> {
    ps_i2c0: P,
    ps_i2c1: P,
    pl_front_panel: P,
}

impl<P> PhysicalBusMap<P> {
    /// Creates a complete logical-to-physical bus mapping.
    pub const fn new(ps_i2c0: P, ps_i2c1: P, pl_front_panel: P) -> Self {
        Self {
            ps_i2c0,
            ps_i2c1,
            pl_front_panel,
        }
    }

    /// Returns the physical identifier assigned to a logical bus.
    pub const fn get(&self, bus: LogicalBus) -> &P {
        match bus {
            LogicalBus::PsI2c0 => &self.ps_i2c0,
            LogicalBus::PsI2c1 => &self.ps_i2c1,
            LogicalBus::PlFrontPanelI2c => &self.pl_front_panel,
        }
    }
}

/// Owns the three physical Kria I2C backends used by the admitted topology.
///
/// Each backend is wrapped once in a [`RefCell`]. Factory methods lend leaf
/// drivers a transaction-serializing handle without moving or duplicating the
/// underlying controller.
pub struct KriaI2cBuses<Ps0, Ps1, Pl> {
    ps_i2c0: RefCell<Ps0>,
    ps_i2c1: RefCell<Ps1>,
    pl_front_panel: RefCell<Pl>,
}

impl<Ps0, Ps1, Pl> KriaI2cBuses<Ps0, Ps1, Pl> {
    /// Creates a bundle from the two PS controllers and shared PL controller.
    pub const fn new(ps_i2c0: Ps0, ps_i2c1: Ps1, pl_front_panel: Pl) -> Self {
        Self {
            ps_i2c0: RefCell::new(ps_i2c0),
            ps_i2c1: RefCell::new(ps_i2c1),
            pl_front_panel: RefCell::new(pl_front_panel),
        }
    }

    /// Returns all three owned backends in logical-bus order.
    pub fn release(self) -> (Ps0, Ps1, Pl) {
        (
            self.ps_i2c0.into_inner(),
            self.ps_i2c1.into_inner(),
            self.pl_front_panel.into_inner(),
        )
    }

    /// Lends an STTS22H driver on PS I2C1.
    pub fn stts22h(&self) -> rlvgl_device_stts22h::Stts22h<RefCellDevice<'_, Ps1>>
    where
        Ps1: I2c<SevenBitAddress>,
    {
        rlvgl_device_stts22h::Stts22h::new(share_i2c(&self.ps_i2c1))
    }

    /// Lends a VEML3235SL driver on the shared PL front-panel bus.
    pub fn veml3235sl(&self) -> rlvgl_device_veml3235sl::Veml3235sl<RefCellDevice<'_, Pl>>
    where
        Pl: I2c<SevenBitAddress>,
    {
        rlvgl_device_veml3235sl::Veml3235sl::new(share_i2c(&self.pl_front_panel))
    }

    /// Lends a PTN3460 driver on the shared PL front-panel bus.
    pub fn ptn3460(&self) -> rlvgl_device_ptn3460::Ptn3460<RefCellDevice<'_, Pl>>
    where
        Pl: I2c<SevenBitAddress>,
    {
        rlvgl_device_ptn3460::Ptn3460::new(share_i2c(&self.pl_front_panel))
    }

    /// Lends an unverified PCM3168A driver on the shared PL front-panel bus.
    ///
    /// The caller must independently attest the codec's rails, reset, and
    /// clocks before converting this driver to its ready state.
    pub fn pcm3168a(&self) -> rlvgl_device_pcm3168a::Pcm3168a<RefCellDevice<'_, Pl>>
    where
        Pl: I2c<SevenBitAddress>,
    {
        rlvgl_device_pcm3168a::Pcm3168a::new(share_i2c(&self.pl_front_panel))
    }
}

/// Allocation-free result of probing one evidence-admitted endpoint.
#[derive(Debug)]
pub struct ProbeDiagnostic<T, E> {
    endpoint: DeviceEndpoint,
    result: Result<T, E>,
}

impl<T, E> ProbeDiagnostic<T, E> {
    const fn new(endpoint: DeviceEndpoint, result: Result<T, E>) -> Self {
        Self { endpoint, result }
    }

    /// Returns the endpoint associated with this probe result.
    pub const fn endpoint(&self) -> DeviceEndpoint {
        self.endpoint
    }

    /// Returns `true` when the leaf driver's probe succeeded.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Borrows the successful probe value or preserved leaf error.
    pub fn result(&self) -> Result<&T, &E> {
        self.result.as_ref()
    }

    /// Consumes the diagnostic and returns the leaf driver's original result.
    pub fn into_result(self) -> Result<T, E> {
        self.result
    }
}

/// Probes the fitted STTS22H and associates its result with the board endpoint.
pub fn probe_stts22h<I2C>(
    driver: &mut rlvgl_device_stts22h::Stts22h<I2C>,
) -> ProbeDiagnostic<(), rlvgl_device_stts22h::Error<I2C::Error>>
where
    I2C: I2c<SevenBitAddress>,
{
    ProbeDiagnostic::new(STTS22H, driver.probe())
}

/// Probes the fitted VEML3235SL and associates its result with the board endpoint.
pub fn probe_veml3235sl<I2C>(
    driver: &mut rlvgl_device_veml3235sl::Veml3235sl<I2C>,
) -> ProbeDiagnostic<(), rlvgl_device_veml3235sl::Error<I2C::Error>>
where
    I2C: I2c<SevenBitAddress>,
{
    ProbeDiagnostic::new(VEML3235SL, driver.probe())
}

/// Probes the fitted PTN3460 and associates its result with the board endpoint.
pub fn probe_ptn3460<I2C>(
    driver: &mut rlvgl_device_ptn3460::Ptn3460<I2C>,
) -> ProbeDiagnostic<rlvgl_device_ptn3460::Probe, rlvgl_device_ptn3460::Error<I2C::Error>>
where
    I2C: I2c<SevenBitAddress>,
{
    ProbeDiagnostic::new(PTN3460, driver.probe())
}

/// Probes a ready PCM3168A and associates its result with the board endpoint.
pub fn probe_pcm3168a<I2C>(
    driver: &mut rlvgl_device_pcm3168a::Pcm3168a<I2C, rlvgl_device_pcm3168a::Ready>,
) -> ProbeDiagnostic<rlvgl_device_pcm3168a::Probe, rlvgl_device_pcm3168a::Error<I2C::Error>>
where
    I2C: I2c<SevenBitAddress>,
{
    ProbeDiagnostic::new(PCM3168A, driver.probe())
}

/// Creates a single-threaded shared-bus handle implementing `embedded-hal` I2C.
///
/// Create one handle per leaf driver. The underlying [`RefCell`] serializes a
/// complete I2C transaction, so operations from different handles cannot
/// interleave. Interrupt- or thread-shared consumers must select a stronger
/// `embedded-hal-bus` adapter appropriate to their executor.
pub fn share_i2c<I2C>(bus: &RefCell<I2C>) -> RefCellDevice<'_, I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    RefCellDevice::new(bus)
}

/// Linux kernel-I2C backend support.
///
/// This module exists only when the `linux` feature is enabled for a Linux
/// target. Callers supply a configured device-node path; adapter numbering is
/// deliberately not encoded in the board topology.
#[cfg(all(feature = "linux", target_os = "linux"))]
pub mod linux {
    use super::{KriaI2cBuses, LogicalBus, PhysicalBusMap};
    use linux_embedded_hal::I2cdev;
    use linux_embedded_hal::i2cdev::linux::LinuxI2CError;
    use std::path::Path;

    /// Failure to open one physical controller in a logical bus mapping.
    #[derive(Debug)]
    pub struct OpenError {
        bus: LogicalBus,
        error: LinuxI2CError,
    }

    impl OpenError {
        const fn new(bus: LogicalBus, error: LinuxI2CError) -> Self {
            Self { bus, error }
        }

        /// Returns the logical bus whose physical controller failed to open.
        pub const fn bus(&self) -> LogicalBus {
            self.bus
        }

        /// Returns the underlying Linux I2C error.
        pub const fn error(&self) -> &LinuxI2CError {
            &self.error
        }
    }

    /// Opens a Linux I2C controller device node as an `embedded-hal` backend.
    pub fn open(path: impl AsRef<Path>) -> Result<I2cdev, LinuxI2CError> {
        I2cdev::new(path)
    }

    /// Opens all controllers in a caller-supplied physical bus mapping.
    ///
    /// Controllers are opened in PS I2C0, PS I2C1, then PL front-panel order.
    /// On failure, [`OpenError`] identifies the logical controller involved.
    pub fn open_mapped<P>(
        mapping: &PhysicalBusMap<P>,
    ) -> Result<KriaI2cBuses<I2cdev, I2cdev, I2cdev>, OpenError>
    where
        P: AsRef<Path>,
    {
        let ps_i2c0 = open(mapping.get(LogicalBus::PsI2c0))
            .map_err(|error| OpenError::new(LogicalBus::PsI2c0, error))?;
        let ps_i2c1 = open(mapping.get(LogicalBus::PsI2c1))
            .map_err(|error| OpenError::new(LogicalBus::PsI2c1, error))?;
        let pl_front_panel = open(mapping.get(LogicalBus::PlFrontPanelI2c))
            .map_err(|error| OpenError::new(LogicalBus::PlFrontPanelI2c, error))?;

        Ok(KriaI2cBuses::new(ps_i2c0, ps_i2c1, pl_front_panel))
    }
}
