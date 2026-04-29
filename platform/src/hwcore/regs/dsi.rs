//! Typed register layout for the STM32H747 DSI host + wrapper.
//!
//! Layout matches RM0399 §34.16 ("DSI Host register map") exactly. Each
//! field's offset is asserted at compile time; the wrong layout becomes
//! a `cargo check` failure rather than a runtime "snow over splash"
//! panel artifact (the failure mode of the prior LCCR-at-0x2C bug
//! captured in the `feedback_dsi_lccr_offset.md` memory note).
//!
//! The host block lives at `0x5000_0000` and covers offsets `0x000`
//! through `0x0B0`. The wrapper block lives at `0x5000_0400` and covers
//! offsets `0x004` through `0x030`. They are modelled as two independent
//! `#[repr(C)]` structs so the ~760-byte gap between them does not bloat
//! a single struct's `_reserved_*` arrays.

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::{Ro, Rw};

// ── DsiRegs (host) ──────────────────────────────────────────────────────

/// MMIO layout of the DSI host register block (`0x5000_0000`).
///
/// Field offsets verified against RM0399 §34.16 and the `stm32h7-0.15.1`
/// PAC. The static assertions at the bottom of this file fail to
/// compile if any field's `offset_of!` disagrees with the documented
/// offset.
#[repr(C)]
pub struct DsiRegs {
    /// `0x000` Version Register (RO).
    pub vr: Ro<u32>,
    /// `0x004` Control Register.
    pub cr: Rw<u32>,
    /// `0x008` Clock Control Register.
    pub ccr: Rw<u32>,
    /// `0x00C` LTDC Virtual Channel ID Register.
    pub lvcidr: Rw<u32>,
    /// `0x010` LTDC Color Coding Register.
    pub lcolcr: Rw<u32>,
    /// `0x014` LTDC Polarity Configuration Register.
    pub lpcr: Rw<u32>,
    /// `0x018` Low-Power Mode Configuration Register.
    pub lpmcr: Rw<u32>,
    /// `0x01C..0x02B` reserved (4 × `u32`).
    _reserved_01c: [u32; 4],
    /// `0x02C` Protocol Configuration Register.
    pub pcr: Rw<u32>,
    /// `0x030` Generic VCID Register.
    pub gvcidr: Rw<u32>,
    /// `0x034` Mode Configuration Register.
    pub mcr: Rw<u32>,
    /// `0x038` Video Mode Configuration Register.
    pub vmcr: Rw<u32>,
    /// `0x03C` Video Packet Configuration Register.
    pub vpcr: Rw<u32>,
    /// `0x040` Video Chunks Configuration Register.
    pub vccr: Rw<u32>,
    /// `0x044` Video Null Packet Configuration Register.
    pub vnpcr: Rw<u32>,
    /// `0x048` Video HSA Configuration Register.
    pub vhsacr: Rw<u32>,
    /// `0x04C` Video HBP Configuration Register.
    pub vhbpcr: Rw<u32>,
    /// `0x050` Video Line Configuration Register.
    pub vlcr: Rw<u32>,
    /// `0x054` Video VSA Configuration Register.
    pub vvsacr: Rw<u32>,
    /// `0x058` Video VBP Configuration Register.
    pub vvbpcr: Rw<u32>,
    /// `0x05C` Video VFP Configuration Register.
    pub vvfpcr: Rw<u32>,
    /// `0x060` Video VA Configuration Register.
    pub vvacr: Rw<u32>,
    /// `0x064` LTDC Command Configuration Register.
    ///
    /// **Critical offset**: this is `0x64`, not `0x2C`. Writing to
    /// `0x2C` would clobber `pcr` (Protocol Configuration) and leave
    /// `LCCR.CMDSIZE = 0`, producing snow on the panel. The
    /// `LCCR_OFFSET_IS_0X64` const assertion below traps any layout
    /// drift at compile time.
    pub lccr: Rw<u32>,
    /// `0x068` Command Mode Configuration Register.
    pub cmcr: Rw<u32>,
    /// `0x06C` Generic Header Configuration Register.
    pub ghcr: Rw<u32>,
    /// `0x070..0x073` reserved.
    _reserved_070: u32,
    /// `0x074` Generic Packet Status Register (RO).
    pub gpsr: Ro<u32>,
    /// `0x078` Timeout Counter Configuration Register 0.
    pub tccr0: Rw<u32>,
    /// `0x07C..0x093` reserved (6 × `u32`).
    _reserved_07c: [u32; 6],
    /// `0x094` Clock Lane Configuration Register.
    pub clcr: Rw<u32>,
    /// `0x098` Clock Lane Timer Configuration Register.
    pub cltcr: Rw<u32>,
    /// `0x09C` Data Lane Timer Configuration Register.
    pub dltcr: Rw<u32>,
    /// `0x0A0` PHY Control Register.
    pub pctlr: Rw<u32>,
    /// `0x0A4` PHY Configuration Register.
    pub pconfr: Rw<u32>,
    /// `0x0A8..0x0AF` reserved (2 × `u32`).
    _reserved_0a8: [u32; 2],
    /// `0x0B0` PHY Status Register (RO).
    pub psr: Ro<u32>,
    /// `0x0B4..0x0BB` reserved (2 × `u32`).
    _reserved_0b4: [u32; 2],
    /// `0x0BC` Interrupt and Status Register 0 (RO — ACK / PHY errors).
    pub isr0: Ro<u32>,
    /// `0x0C0` Interrupt and Status Register 1 (RO — payload errors).
    pub isr1: Ro<u32>,
    /// `0x0C4..0x0CB` reserved (2 × `u32`).
    _reserved_0c4: [u32; 2],
    /// `0x0CC` Interrupt Enable Register 0.
    pub ier0: Rw<u32>,
    /// `0x0D0` Interrupt Enable Register 1.
    pub ier1: Rw<u32>,
    /// `0x0D4..0x0D7` reserved.
    _reserved_0d4: u32,
    /// `0x0D8` Force Interrupt Register 0 (write-1-to-clear ISR0 flags).
    pub fir0: Rw<u32>,
    /// `0x0DC` Force Interrupt Register 1 (write-1-to-clear ISR1 flags).
    pub fir1: Rw<u32>,
}

// Compile-time offset checks. Each line below is the contract that
// caused the original LCCR-at-0x2C panic. Any layout drift (a missed
// reserved gap, a reordered field) trips a `cargo check` error.
const _: () = assert!(offset_of!(DsiRegs, vr) == 0x000);
const _: () = assert!(offset_of!(DsiRegs, cr) == 0x004);
const _: () = assert!(offset_of!(DsiRegs, ccr) == 0x008);
const _: () = assert!(offset_of!(DsiRegs, lvcidr) == 0x00C);
const _: () = assert!(offset_of!(DsiRegs, lcolcr) == 0x010);
const _: () = assert!(offset_of!(DsiRegs, lpcr) == 0x014);
const _: () = assert!(offset_of!(DsiRegs, lpmcr) == 0x018);
const _: () = assert!(offset_of!(DsiRegs, pcr) == 0x02C);
const _: () = assert!(offset_of!(DsiRegs, gvcidr) == 0x030);
const _: () = assert!(offset_of!(DsiRegs, mcr) == 0x034);
const _: () = assert!(offset_of!(DsiRegs, vmcr) == 0x038);
const _: () = assert!(offset_of!(DsiRegs, vpcr) == 0x03C);
const _: () = assert!(offset_of!(DsiRegs, vccr) == 0x040);
const _: () = assert!(offset_of!(DsiRegs, vnpcr) == 0x044);
const _: () = assert!(offset_of!(DsiRegs, vhsacr) == 0x048);
const _: () = assert!(offset_of!(DsiRegs, vhbpcr) == 0x04C);
const _: () = assert!(offset_of!(DsiRegs, vlcr) == 0x050);
const _: () = assert!(offset_of!(DsiRegs, vvsacr) == 0x054);
const _: () = assert!(offset_of!(DsiRegs, vvbpcr) == 0x058);
const _: () = assert!(offset_of!(DsiRegs, vvfpcr) == 0x05C);
const _: () = assert!(offset_of!(DsiRegs, vvacr) == 0x060);

/// The LCCR offset assertion that the entire typed register-block
/// approach exists to enforce. Named so the failure message in CI
/// points directly at the discipline rule it protects.
pub const LCCR_OFFSET_IS_0X64: () = assert!(offset_of!(DsiRegs, lccr) == 0x064);

const _: () = assert!(offset_of!(DsiRegs, cmcr) == 0x068);
const _: () = assert!(offset_of!(DsiRegs, ghcr) == 0x06C);
const _: () = assert!(offset_of!(DsiRegs, gpsr) == 0x074);
const _: () = assert!(offset_of!(DsiRegs, tccr0) == 0x078);
const _: () = assert!(offset_of!(DsiRegs, clcr) == 0x094);
const _: () = assert!(offset_of!(DsiRegs, cltcr) == 0x098);
const _: () = assert!(offset_of!(DsiRegs, dltcr) == 0x09C);
const _: () = assert!(offset_of!(DsiRegs, pctlr) == 0x0A0);
const _: () = assert!(offset_of!(DsiRegs, pconfr) == 0x0A4);
const _: () = assert!(offset_of!(DsiRegs, psr) == 0x0B0);
const _: () = assert!(offset_of!(DsiRegs, isr0) == 0x0BC);
const _: () = assert!(offset_of!(DsiRegs, isr1) == 0x0C0);
const _: () = assert!(offset_of!(DsiRegs, ier0) == 0x0CC);
const _: () = assert!(offset_of!(DsiRegs, ier1) == 0x0D0);
const _: () = assert!(offset_of!(DsiRegs, fir0) == 0x0D8);
const _: () = assert!(offset_of!(DsiRegs, fir1) == 0x0DC);

// ── DsiWrapperRegs (DSI + 0x400) ────────────────────────────────────────

/// MMIO layout of the DSI wrapper register block (`0x5000_0400`).
///
/// The wrapper sits 0x400 bytes above the host block; modelling them as
/// two structs avoids a 944-byte `_reserved_*` array inside `DsiRegs`.
#[repr(C)]
pub struct DsiWrapperRegs {
    /// `0x000` Wrapper Configuration Register (DSIM, COLMUX, TESRC, AR).
    pub wcfgr: Rw<u32>,
    /// `0x004` Wrapper Control Register (DSIEN, LTDCEN).
    pub wcr: Rw<u32>,
    /// `0x008` Wrapper Interrupt Enable Register.
    pub wier: Rw<u32>,
    /// `0x00C` Wrapper Interrupt Status Register (RO).
    pub wisr: Ro<u32>,
    /// `0x010` Wrapper Interrupt Flag Clear Register.
    pub wifcr: Rw<u32>,
    /// `0x014..0x017` reserved.
    _reserved_014: u32,
    /// `0x018` Wrapper PHY Configuration Register 0.
    pub wpcr0: Rw<u32>,
    /// `0x01C..0x02F` reserved (5 × `u32`).
    _reserved_01c: [u32; 5],
    /// `0x030` Wrapper Regulator and PLL Control Register.
    pub wrpcr: Rw<u32>,
}

const _: () = assert!(offset_of!(DsiWrapperRegs, wcfgr) == 0x000);
const _: () = assert!(offset_of!(DsiWrapperRegs, wcr) == 0x004);
const _: () = assert!(offset_of!(DsiWrapperRegs, wier) == 0x008);
const _: () = assert!(offset_of!(DsiWrapperRegs, wisr) == 0x00C);
const _: () = assert!(offset_of!(DsiWrapperRegs, wifcr) == 0x010);
const _: () = assert!(offset_of!(DsiWrapperRegs, wpcr0) == 0x018);
const _: () = assert!(offset_of!(DsiWrapperRegs, wrpcr) == 0x030);

// ── Handles ─────────────────────────────────────────────────────────────

/// Base address of the DSI host block on STM32H747.
pub const DSI_HOST_BASE: usize = 0x5000_0000;

/// Base address of the DSI wrapper block on STM32H747 (`DSI + 0x400`).
pub const DSI_WRAPPER_BASE: usize = 0x5000_0400;

/// Typed handle on the DSI host block. Wraps an `MmioAddr<DsiRegs>`
/// constructed at the silicon-defined base.
pub struct Dsi {
    base: MmioAddr<DsiRegs>,
}

impl Dsi {
    /// Construct the singleton DSI host handle.
    ///
    /// # Safety
    ///
    /// The DSI peripheral block at `0x5000_0000` must be unaliased: at
    /// most one `Dsi` may exist in the program at any time. The DSI
    /// clock (`RCC.APB3ENR.DSIEN`) must be enabled before any field is
    /// accessed.
    pub const unsafe fn new() -> Self {
        // SAFETY: caller contract; address is the silicon-defined DSI
        // host base.
        Self {
            base: unsafe { MmioAddr::new(DSI_HOST_BASE) },
        }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &DsiRegs {
        // SAFETY: the `MmioAddr<DsiRegs>` construction contract in
        // `Self::new` asserts the region is live and unaliased. Field
        // access goes through the volatile wrappers in
        // [`crate::hwcore::regs::access`].
        unsafe { &*self.base.as_ptr() }
    }
}

/// Typed handle on the DSI wrapper block.
pub struct DsiWrapper {
    base: MmioAddr<DsiWrapperRegs>,
}

impl DsiWrapper {
    /// Construct the singleton DSI wrapper handle.
    ///
    /// # Safety
    ///
    /// See [`Dsi::new`]. The wrapper block is at `DSI + 0x400`; its
    /// access discipline matches the host block.
    pub const unsafe fn new() -> Self {
        // SAFETY: caller contract; address is the silicon-defined DSI
        // wrapper base.
        Self {
            base: unsafe { MmioAddr::new(DSI_WRAPPER_BASE) },
        }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &DsiWrapperRegs {
        // SAFETY: see [`Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn host_block_is_at_least_psr_offset_plus_one_word() {
        // PSR is at 0x0B0; the struct must be ≥ 0x0B4 bytes.
        assert!(size_of::<DsiRegs>() >= 0x0B4);
    }

    #[test]
    fn wrapper_block_is_at_least_wrpcr_offset_plus_one_word() {
        assert!(size_of::<DsiWrapperRegs>() >= 0x034);
    }

    #[test]
    fn lccr_offset_const_evaluates() {
        // If this compiles, the const_assert above succeeded.
        let _: () = LCCR_OFFSET_IS_0X64;
    }
}
