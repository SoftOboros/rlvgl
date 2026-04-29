//! Typed register layout for the STM32H747 USART/UART peripherals.
//!
//! Layout matches RM0399 §47.8 ("USART register map"). USART1 is the
//! ST-LINK VCP UART used by the `rlvgl-playit` runtime control plane on
//! the 747I-DISCO.

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::{Ro, Rw};

/// MMIO layout of one USART/UART peripheral.
#[repr(C)]
pub struct UsartRegs {
    /// `0x00` Control Register 1.
    pub cr1: Rw<u32>,
    /// `0x04` Control Register 2.
    pub cr2: Rw<u32>,
    /// `0x08` Control Register 3.
    pub cr3: Rw<u32>,
    /// `0x0C` Baud Rate Register.
    pub brr: Rw<u32>,
    /// `0x10` Guard Time and Prescaler Register.
    pub gtpr: Rw<u32>,
    /// `0x14` Receiver Timeout Register.
    pub rtor: Rw<u32>,
    /// `0x18` Request Register.
    pub rqr: Rw<u32>,
    /// `0x1C` Interrupt and Status Register (RO — clear via `icr`).
    pub isr: Ro<u32>,
    /// `0x20` Interrupt Clear Register.
    pub icr: Rw<u32>,
    /// `0x24` Receive Data Register (RO).
    pub rdr: Ro<u32>,
    /// `0x28` Transmit Data Register.
    pub tdr: Rw<u32>,
    /// `0x2C` Prescaler Register.
    pub presc: Rw<u32>,
}

const _: () = assert!(offset_of!(UsartRegs, cr1) == 0x00);
const _: () = assert!(offset_of!(UsartRegs, cr2) == 0x04);
const _: () = assert!(offset_of!(UsartRegs, cr3) == 0x08);
const _: () = assert!(offset_of!(UsartRegs, brr) == 0x0C);
const _: () = assert!(offset_of!(UsartRegs, gtpr) == 0x10);
const _: () = assert!(offset_of!(UsartRegs, rtor) == 0x14);
const _: () = assert!(offset_of!(UsartRegs, rqr) == 0x18);
const _: () = assert!(offset_of!(UsartRegs, isr) == 0x1C);
const _: () = assert!(offset_of!(UsartRegs, icr) == 0x20);
const _: () = assert!(offset_of!(UsartRegs, rdr) == 0x24);
const _: () = assert!(offset_of!(UsartRegs, tdr) == 0x28);
const _: () = assert!(offset_of!(UsartRegs, presc) == 0x2C);

/// Base address of USART1 (ST-LINK VCP on the 747I-DISCO).
pub const USART1_BASE: usize = 0x4001_1000;
/// Base address of USART2.
pub const USART2_BASE: usize = 0x4000_4400;
/// Base address of USART3.
pub const USART3_BASE: usize = 0x4000_4800;
/// Base address of UART4.
pub const UART4_BASE: usize = 0x4000_4C00;
/// Base address of UART5.
pub const UART5_BASE: usize = 0x4000_5000;
/// Base address of USART6.
pub const USART6_BASE: usize = 0x4001_1400;
/// Base address of UART7.
pub const UART7_BASE: usize = 0x4000_7800;
/// Base address of UART8.
pub const UART8_BASE: usize = 0x4000_7C00;

/// Typed handle on a USART/UART peripheral.
pub struct Usart {
    base: MmioAddr<UsartRegs>,
}

impl Usart {
    /// Construct a handle at the given base address.
    ///
    /// # Safety
    ///
    /// See [`crate::hwcore::regs::i2c::I2c::new`].
    pub const unsafe fn new(base: usize) -> Self {
        // SAFETY: caller contract.
        Self {
            base: unsafe { MmioAddr::new(base) },
        }
    }

    /// Convenience constructor for USART1 (ST-LINK VCP on 747I-DISCO).
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn usart1() -> Self {
        // SAFETY: address is the silicon-defined USART1 base.
        unsafe { Self::new(USART1_BASE) }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &UsartRegs {
        // SAFETY: see [`crate::hwcore::regs::dsi::Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usart1_base_matches_legacy_constants() {
        assert_eq!(USART1_BASE, 0x4001_1000);
        assert_eq!(USART1_BASE + offset_of!(UsartRegs, cr1), 0x4001_1000);
        assert_eq!(USART1_BASE + offset_of!(UsartRegs, isr), 0x4001_101C);
        assert_eq!(USART1_BASE + offset_of!(UsartRegs, icr), 0x4001_1020);
        assert_eq!(USART1_BASE + offset_of!(UsartRegs, rdr), 0x4001_1024);
        assert_eq!(USART1_BASE + offset_of!(UsartRegs, tdr), 0x4001_1028);
    }
}
