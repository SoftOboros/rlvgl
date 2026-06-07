//! Typed register layout for the STM32H747 I2C peripheral.
//!
//! Layout matches RM0399 §52.7 ("I2C register map") — applies uniformly
//! to I2C1..I2C4. The 747I-DISCO uses I2C4 (`0x5800_1C00`) for the
//! FT5336 touch controller; this module provides per-instance handles
//! for any of the four banks.

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::{Ro, Rw};

/// MMIO layout of one I2C peripheral.
#[repr(C)]
pub struct I2cRegs {
    /// `0x00` Control Register 1.
    pub cr1: Rw<u32>,
    /// `0x04` Control Register 2 (start, stop, NBYTES, slave address).
    pub cr2: Rw<u32>,
    /// `0x08` Own Address Register 1.
    pub oar1: Rw<u32>,
    /// `0x0C` Own Address Register 2.
    pub oar2: Rw<u32>,
    /// `0x10` Timing Register (SCL / hold-time / setup-time programming).
    pub timingr: Rw<u32>,
    /// `0x14` Timeout Register.
    pub timeoutr: Rw<u32>,
    /// `0x18` Interrupt and Status Register (RO — clear via `icr`).
    pub isr: Ro<u32>,
    /// `0x1C` Interrupt Clear Register.
    pub icr: Rw<u32>,
    /// `0x20` PEC Register (RO).
    pub pecr: Ro<u32>,
    /// `0x24` Receive Data Register (RO).
    pub rxdr: Ro<u32>,
    /// `0x28` Transmit Data Register.
    pub txdr: Rw<u32>,
}

const _: () = assert!(offset_of!(I2cRegs, cr1) == 0x00);
const _: () = assert!(offset_of!(I2cRegs, cr2) == 0x04);
const _: () = assert!(offset_of!(I2cRegs, oar1) == 0x08);
const _: () = assert!(offset_of!(I2cRegs, oar2) == 0x0C);
const _: () = assert!(offset_of!(I2cRegs, timingr) == 0x10);
const _: () = assert!(offset_of!(I2cRegs, timeoutr) == 0x14);
const _: () = assert!(offset_of!(I2cRegs, isr) == 0x18);
const _: () = assert!(offset_of!(I2cRegs, icr) == 0x1C);
const _: () = assert!(offset_of!(I2cRegs, pecr) == 0x20);
const _: () = assert!(offset_of!(I2cRegs, rxdr) == 0x24);
const _: () = assert!(offset_of!(I2cRegs, txdr) == 0x28);

/// Base address of I2C1.
pub const I2C1_BASE: usize = 0x4000_5400;
/// Base address of I2C2.
pub const I2C2_BASE: usize = 0x4000_5800;
/// Base address of I2C3.
pub const I2C3_BASE: usize = 0x4000_5C00;
/// Base address of I2C4 (D3 domain — used for FT5336 touch on the
/// STM32H747I-DISCO).
pub const I2C4_BASE: usize = 0x5800_1C00;

/// Typed handle on an I2C peripheral.
pub struct I2c {
    base: MmioAddr<I2cRegs>,
}

impl I2c {
    /// Construct a handle at the given base address.
    ///
    /// # Safety
    ///
    /// `base` must be the silicon-defined base of an I2C peripheral
    /// (one of [`I2C1_BASE`]..[`I2C4_BASE`]). The peripheral clock
    /// must be enabled before any field is accessed and the peripheral
    /// must be unaliased: at most one [`I2c`] for a given bank exists
    /// in the program at any time.
    pub const unsafe fn new(base: usize) -> Self {
        // SAFETY: caller contract.
        Self {
            base: unsafe { MmioAddr::new(base) },
        }
    }

    /// Convenience constructor for I2C4 (FT5336 touch on 747I-DISCO).
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn i2c4() -> Self {
        // SAFETY: address is the silicon-defined I2C4 base.
        unsafe { Self::new(I2C4_BASE) }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &I2cRegs {
        // SAFETY: see [`crate::hwcore::regs::dsi::Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i2c4_base_matches_legacy_constants() {
        assert_eq!(I2C4_BASE, 0x5800_1C00);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, cr1), 0x5800_1C00);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, cr2), 0x5800_1C04);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, isr), 0x5800_1C18);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, icr), 0x5800_1C1C);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, rxdr), 0x5800_1C24);
        assert_eq!(I2C4_BASE + offset_of!(I2cRegs, txdr), 0x5800_1C28);
    }
}
