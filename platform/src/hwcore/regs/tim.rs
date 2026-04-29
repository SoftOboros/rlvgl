//! Typed register layout for the STM32H747 basic timers (TIM6 / TIM7).
//!
//! Layout matches RM0399 §43.4 ("TIM6/7 register map"). Basic timers
//! have a strict subset of the general-purpose / advanced timer
//! registers — they have no input capture, no output compare, no
//! complementary outputs. The 747I-DISCO uses TIM6 in the bare-metal
//! build for the touch-poll cadence and TIM7 in the FreeRTOS build for
//! the system tick.

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::Rw;

/// MMIO layout of one basic timer (TIM6 / TIM7).
#[repr(C)]
pub struct TimBasicRegs {
    /// `0x00` Control Register 1 (CEN, UDIS, URS, OPM, ARPE).
    pub cr1: Rw<u32>,
    /// `0x04` Control Register 2 (MMS).
    pub cr2: Rw<u32>,
    /// `0x08..0x0B` reserved.
    _reserved_08: u32,
    /// `0x0C` DMA / Interrupt Enable Register (UIE).
    pub dier: Rw<u32>,
    /// `0x10` Status Register (UIF flag).
    pub sr: Rw<u32>,
    /// `0x14` Event Generation Register (UG).
    pub egr: Rw<u32>,
    /// `0x18..0x23` reserved (3 × `u32`).
    _reserved_18: [u32; 3],
    /// `0x24` Counter Register.
    pub cnt: Rw<u32>,
    /// `0x28` Prescaler Register.
    pub psc: Rw<u32>,
    /// `0x2C` Auto-Reload Register.
    pub arr: Rw<u32>,
}

const _: () = assert!(offset_of!(TimBasicRegs, cr1) == 0x00);
const _: () = assert!(offset_of!(TimBasicRegs, cr2) == 0x04);
const _: () = assert!(offset_of!(TimBasicRegs, dier) == 0x0C);
const _: () = assert!(offset_of!(TimBasicRegs, sr) == 0x10);
const _: () = assert!(offset_of!(TimBasicRegs, egr) == 0x14);
const _: () = assert!(offset_of!(TimBasicRegs, cnt) == 0x24);
const _: () = assert!(offset_of!(TimBasicRegs, psc) == 0x28);
const _: () = assert!(offset_of!(TimBasicRegs, arr) == 0x2C);

/// Base address of TIM6 (APB1, used by bare-metal touch poll).
pub const TIM6_BASE: usize = 0x4000_1000;
/// Base address of TIM7 (APB1, used by FreeRTOS tick).
pub const TIM7_BASE: usize = 0x4000_1400;

/// Typed handle on a basic timer.
pub struct TimBasic {
    base: MmioAddr<TimBasicRegs>,
}

impl TimBasic {
    /// Construct a handle at the given base address.
    ///
    /// # Safety
    ///
    /// `base` must be [`TIM6_BASE`] or [`TIM7_BASE`]. The timer's
    /// clock (`RCC.APB1LENR.TIM{6,7}EN`) must be enabled before any
    /// field is accessed and the timer must be unaliased.
    pub const unsafe fn new(base: usize) -> Self {
        // SAFETY: caller contract.
        Self {
            base: unsafe { MmioAddr::new(base) },
        }
    }

    /// Convenience constructor for TIM6.
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn tim6() -> Self {
        // SAFETY: address is the silicon-defined TIM6 base.
        unsafe { Self::new(TIM6_BASE) }
    }

    /// Convenience constructor for TIM7.
    ///
    /// # Safety
    ///
    /// See [`Self::new`].
    pub const unsafe fn tim7() -> Self {
        // SAFETY: address is the silicon-defined TIM7 base.
        unsafe { Self::new(TIM7_BASE) }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &TimBasicRegs {
        // SAFETY: see [`crate::hwcore::regs::dsi::Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tim6_sr_matches_legacy_constant() {
        assert_eq!(TIM6_BASE + offset_of!(TimBasicRegs, sr), 0x4000_1010);
    }

    #[test]
    fn tim7_layout_matches_freertos_entry_constants() {
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, cr1), 0x4000_1400);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, dier), 0x4000_140C);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, sr), 0x4000_1410);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, egr), 0x4000_1414);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, cnt), 0x4000_1424);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, psc), 0x4000_1428);
        assert_eq!(TIM7_BASE + offset_of!(TimBasicRegs, arr), 0x4000_142C);
    }
}
