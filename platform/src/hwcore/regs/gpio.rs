//! Typed register layout for the STM32H747 GPIO peripherals.
//!
//! Layout matches RM0399 §13.5 ("GPIO register map") — applies
//! uniformly to GPIOA..GPIOK. Per-bank constants and convenience
//! constructors cover the banks the 747I-DISCO uses (GPIOC for the
//! user button, GPIOJ for backlight + scope probes, GPIOK for the
//! FT5336 INT line and joystick).

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::{Ro, Rw};

/// MMIO layout of one GPIO bank.
#[repr(C)]
pub struct GpioRegs {
    /// `0x00` Mode Register (2 bits per pin: input/output/alternate/analog).
    pub moder: Rw<u32>,
    /// `0x04` Output Type Register (push-pull / open-drain per pin).
    pub otyper: Rw<u32>,
    /// `0x08` Output Speed Register.
    pub ospeedr: Rw<u32>,
    /// `0x0C` Pull-up / Pull-down Register.
    pub pupdr: Rw<u32>,
    /// `0x10` Input Data Register (RO).
    pub idr: Ro<u32>,
    /// `0x14` Output Data Register.
    pub odr: Rw<u32>,
    /// `0x18` Bit Set/Reset Register (atomic single-cycle pin updates).
    pub bsrr: Rw<u32>,
    /// `0x1C` Lock Register.
    pub lckr: Rw<u32>,
    /// `0x20` Alternate Function Low Register (pins 0..7).
    pub afrl: Rw<u32>,
    /// `0x24` Alternate Function High Register (pins 8..15).
    pub afrh: Rw<u32>,
    /// `0x28` Bit Reset Register.
    pub brr: Rw<u32>,
}

const _: () = assert!(offset_of!(GpioRegs, moder) == 0x00);
const _: () = assert!(offset_of!(GpioRegs, otyper) == 0x04);
const _: () = assert!(offset_of!(GpioRegs, ospeedr) == 0x08);
const _: () = assert!(offset_of!(GpioRegs, pupdr) == 0x0C);
const _: () = assert!(offset_of!(GpioRegs, idr) == 0x10);
const _: () = assert!(offset_of!(GpioRegs, odr) == 0x14);
const _: () = assert!(offset_of!(GpioRegs, bsrr) == 0x18);
const _: () = assert!(offset_of!(GpioRegs, lckr) == 0x1C);
const _: () = assert!(offset_of!(GpioRegs, afrl) == 0x20);
const _: () = assert!(offset_of!(GpioRegs, afrh) == 0x24);
const _: () = assert!(offset_of!(GpioRegs, brr) == 0x28);

/// GPIO base addresses on STM32H747. Spaced 0x400 apart.
pub const GPIOA_BASE: usize = 0x5802_0000;
/// See [`GPIOA_BASE`].
pub const GPIOB_BASE: usize = 0x5802_0400;
/// See [`GPIOA_BASE`].
pub const GPIOC_BASE: usize = 0x5802_0800;
/// See [`GPIOA_BASE`].
pub const GPIOD_BASE: usize = 0x5802_0C00;
/// See [`GPIOA_BASE`].
pub const GPIOE_BASE: usize = 0x5802_1000;
/// See [`GPIOA_BASE`].
pub const GPIOF_BASE: usize = 0x5802_1400;
/// See [`GPIOA_BASE`].
pub const GPIOG_BASE: usize = 0x5802_1800;
/// See [`GPIOA_BASE`].
pub const GPIOH_BASE: usize = 0x5802_1C00;
/// See [`GPIOA_BASE`].
pub const GPIOI_BASE: usize = 0x5802_2000;
/// See [`GPIOA_BASE`].
pub const GPIOJ_BASE: usize = 0x5802_2400;
/// See [`GPIOA_BASE`].
pub const GPIOK_BASE: usize = 0x5802_2800;

/// Typed handle on a GPIO bank.
pub struct Gpio {
    base: MmioAddr<GpioRegs>,
}

impl Gpio {
    /// Construct a handle at the given base address.
    ///
    /// # Safety
    ///
    /// `base` must be one of the silicon-defined GPIO bank bases
    /// ([`GPIOA_BASE`]..[`GPIOK_BASE`]). The bank's clock must be
    /// enabled in `RCC.AHB4ENR` before any field is accessed and the
    /// bank must be unaliased: at most one [`Gpio`] handle for a
    /// given bank exists in the program.
    pub const unsafe fn new(base: usize) -> Self {
        // SAFETY: caller contract.
        Self {
            base: unsafe { MmioAddr::new(base) },
        }
    }

    /// Convenience constructor for GPIOC (user button on PC13).
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn gpioc() -> Self {
        // SAFETY: address is the silicon-defined GPIOC base.
        unsafe { Self::new(GPIOC_BASE) }
    }

    /// Convenience constructor for GPIOJ (backlight + scope probes).
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn gpioj() -> Self {
        // SAFETY: address is the silicon-defined GPIOJ base.
        unsafe { Self::new(GPIOJ_BASE) }
    }

    /// Convenience constructor for GPIOK (FT5336 INT + joystick).
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn gpiok() -> Self {
        // SAFETY: address is the silicon-defined GPIOK base.
        unsafe { Self::new(GPIOK_BASE) }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &GpioRegs {
        // SAFETY: see [`crate::hwcore::regs::dsi::Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpio_bank_spacing_is_0x400() {
        assert_eq!(GPIOB_BASE - GPIOA_BASE, 0x400);
        assert_eq!(GPIOK_BASE - GPIOA_BASE, 10 * 0x400);
    }

    #[test]
    fn gpiok_idr_matches_legacy_constant() {
        assert_eq!(GPIOK_BASE + offset_of!(GpioRegs, idr), 0x5802_2810);
    }

    #[test]
    fn gpioj_bsrr_matches_legacy_constant() {
        assert_eq!(GPIOJ_BASE + offset_of!(GpioRegs, bsrr), 0x5802_2418);
    }
}
