//! Typed register layout for the STM32H747 LTDC.
//!
//! Layout matches RM0399 §33.7 ("LTDC register map") exactly. Each
//! field's offset is asserted at compile time via `const _: () =
//! assert!(offset_of!(...) == 0xNN)`.
//!
//! The LTDC peripheral has a global control block at `0x5000_1000` and
//! two layer blocks at `+0x084` and `+0x104`. The layers share a layout,
//! so they're modelled as a fixed-size array of [`LtdcLayerRegs`] inside
//! [`LtdcRegs`] — the per-layer offset is `0x080` and falls out of the
//! struct array stride.

use core::mem::offset_of;

use crate::hwcore::addr::MmioAddr;
use crate::hwcore::regs::access::{Ro, Rw};

// ── LtdcLayerRegs (per-layer block, stride 0x80) ───────────────────────

/// Per-layer register block. Two instances live inside [`LtdcRegs`] at
/// offsets `0x084` (layer 1) and `0x104` (layer 2). Stride is `0x080`
/// bytes; trailing reserved space pads each instance to that boundary.
#[repr(C)]
pub struct LtdcLayerRegs {
    /// `0x00` Layer Control Register.
    pub cr: Rw<u32>,
    /// `0x04` Window Horizontal Position Configuration Register.
    pub whpcr: Rw<u32>,
    /// `0x08` Window Vertical Position Configuration Register.
    pub wvpcr: Rw<u32>,
    /// `0x0C` Color Keying Configuration Register.
    pub ckcr: Rw<u32>,
    /// `0x10` Pixel Format Configuration Register.
    pub pfcr: Rw<u32>,
    /// `0x14` Constant Alpha Configuration Register.
    pub cacr: Rw<u32>,
    /// `0x18` Default Color Configuration Register.
    pub dccr: Rw<u32>,
    /// `0x1C` Blending Factors Configuration Register.
    pub bfcr: Rw<u32>,
    /// `0x20..0x27` reserved (2 × `u32`).
    _reserved_020: [u32; 2],
    /// `0x28` Color Frame Buffer Address Register.
    ///
    /// Written with the back-buffer's [`crate::hwcore::addr::DmaAddr`]
    /// at swap time; consumed by LTDC scan-out.
    pub cfbar: Rw<u32>,
    /// `0x2C` Color Frame Buffer Length Register (`(stride << 16) |
    /// (width * bpp + 3)`).
    pub cfblr: Rw<u32>,
    /// `0x30` Color Frame Buffer Line Number Register (`height`).
    pub cfblnr: Rw<u32>,
    /// `0x34..0x7F` reserved (19 × `u32`) — pads the layer to the
    /// 0x80-byte stride.
    _reserved_034: [u32; 19],
}

const _: () = assert!(offset_of!(LtdcLayerRegs, cr) == 0x00);
const _: () = assert!(offset_of!(LtdcLayerRegs, whpcr) == 0x04);
const _: () = assert!(offset_of!(LtdcLayerRegs, wvpcr) == 0x08);
const _: () = assert!(offset_of!(LtdcLayerRegs, ckcr) == 0x0C);
const _: () = assert!(offset_of!(LtdcLayerRegs, pfcr) == 0x10);
const _: () = assert!(offset_of!(LtdcLayerRegs, cacr) == 0x14);
const _: () = assert!(offset_of!(LtdcLayerRegs, dccr) == 0x18);
const _: () = assert!(offset_of!(LtdcLayerRegs, bfcr) == 0x1C);
const _: () = assert!(offset_of!(LtdcLayerRegs, cfbar) == 0x28);
const _: () = assert!(offset_of!(LtdcLayerRegs, cfblr) == 0x2C);
const _: () = assert!(offset_of!(LtdcLayerRegs, cfblnr) == 0x30);
const _: () = assert!(core::mem::size_of::<LtdcLayerRegs>() == 0x80);

// ── LtdcRegs (global block) ────────────────────────────────────────────

/// MMIO layout of the LTDC peripheral at `0x5000_1000`.
#[repr(C)]
pub struct LtdcRegs {
    /// `0x000` ID Register (RO).
    pub idr: Ro<u32>,
    /// `0x004` Layer Count Register (RO).
    pub lcr: Ro<u32>,
    /// `0x008` Synchronization Size Configuration Register.
    pub sscr: Rw<u32>,
    /// `0x00C` Back Porch Configuration Register.
    pub bpcr: Rw<u32>,
    /// `0x010` Active Width Configuration Register.
    pub awcr: Rw<u32>,
    /// `0x014` Total Width Configuration Register.
    pub twcr: Rw<u32>,
    /// `0x018` Global Control Register.
    pub gcr: Rw<u32>,
    /// `0x01C..0x023` reserved (2 × `u32`).
    _reserved_01c: [u32; 2],
    /// `0x024` Shadow Reload Configuration Register.
    pub srcr: Rw<u32>,
    /// `0x028..0x02B` reserved.
    _reserved_028: u32,
    /// `0x02C` Background Color Configuration Register.
    pub bccr: Rw<u32>,
    /// `0x030..0x033` reserved.
    _reserved_030: u32,
    /// `0x034` Interrupt Enable Register.
    pub ier: Rw<u32>,
    /// `0x038` Interrupt Status Register (RO).
    pub isr: Ro<u32>,
    /// `0x03C` Interrupt Clear Register.
    pub icr: Rw<u32>,
    /// `0x040` Line Interrupt Position Configuration Register.
    pub lipcr: Rw<u32>,
    /// `0x044` Current Position Status Register (RO).
    pub cpsr: Ro<u32>,
    /// `0x048` Current Display Status Register (RO).
    pub cdsr: Ro<u32>,
    /// `0x04C..0x083` reserved (14 × `u32`) — pads to the layer block
    /// boundary at `0x084`.
    _reserved_04c: [u32; 14],
    /// `0x084` (layer 1) and `0x104` (layer 2). Indexed `layers[0]` /
    /// `layers[1]`. Per-layer stride is `0x80` bytes.
    pub layers: [LtdcLayerRegs; 2],
}

const _: () = assert!(offset_of!(LtdcRegs, idr) == 0x000);
const _: () = assert!(offset_of!(LtdcRegs, lcr) == 0x004);
const _: () = assert!(offset_of!(LtdcRegs, sscr) == 0x008);
const _: () = assert!(offset_of!(LtdcRegs, bpcr) == 0x00C);
const _: () = assert!(offset_of!(LtdcRegs, awcr) == 0x010);
const _: () = assert!(offset_of!(LtdcRegs, twcr) == 0x014);
const _: () = assert!(offset_of!(LtdcRegs, gcr) == 0x018);
const _: () = assert!(offset_of!(LtdcRegs, srcr) == 0x024);
const _: () = assert!(offset_of!(LtdcRegs, bccr) == 0x02C);
const _: () = assert!(offset_of!(LtdcRegs, ier) == 0x034);
const _: () = assert!(offset_of!(LtdcRegs, isr) == 0x038);
const _: () = assert!(offset_of!(LtdcRegs, icr) == 0x03C);
const _: () = assert!(offset_of!(LtdcRegs, lipcr) == 0x040);
const _: () = assert!(offset_of!(LtdcRegs, cpsr) == 0x044);
const _: () = assert!(offset_of!(LtdcRegs, cdsr) == 0x048);
const _: () = assert!(offset_of!(LtdcRegs, layers) == 0x084);

/// Layer 2 falls at `0x084 + 0x080 == 0x104`. Named for visibility in
/// CI when the array stride drifts.
pub const LTDC_LAYER2_OFFSET_IS_0X104: () =
    assert!(offset_of!(LtdcRegs, layers) + core::mem::size_of::<LtdcLayerRegs>() == 0x104);

// ── Handle ──────────────────────────────────────────────────────────────

/// Base address of the LTDC peripheral on STM32H747.
pub const LTDC_BASE: usize = 0x5000_1000;

/// Typed handle on the LTDC peripheral.
pub struct Ltdc {
    base: MmioAddr<LtdcRegs>,
}

impl Ltdc {
    /// Construct the singleton LTDC handle.
    ///
    /// # Safety
    ///
    /// The LTDC peripheral block at `0x5000_1000` must be unaliased; at
    /// most one `Ltdc` may exist in the program at any time. The LTDC
    /// clock (`RCC.APB3ENR.LTDCEN`) must be enabled before any field is
    /// accessed.
    pub const unsafe fn new() -> Self {
        // SAFETY: caller contract; address is the silicon-defined LTDC
        // base.
        Self {
            base: unsafe { MmioAddr::new(LTDC_BASE) },
        }
    }

    /// Shared access to the typed register block.
    #[inline]
    pub fn regs(&self) -> &LtdcRegs {
        // SAFETY: see [`crate::hwcore::regs::dsi::Dsi::regs`].
        unsafe { &*self.base.as_ptr() }
    }

    /// Convenience: shared access to layer 1's register block.
    #[inline]
    pub fn layer1(&self) -> &LtdcLayerRegs {
        &self.regs().layers[0]
    }

    /// Convenience: shared access to layer 2's register block.
    #[inline]
    pub fn layer2(&self) -> &LtdcLayerRegs {
        &self.regs().layers[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn layer_block_is_exactly_0x80_bytes() {
        // The const_assert above already enforces this; the runtime
        // test surfaces it in `cargo test` output for visibility.
        assert_eq!(size_of::<LtdcLayerRegs>(), 0x80);
    }

    #[test]
    fn layer1_offset_is_0x84() {
        assert_eq!(offset_of!(LtdcRegs, layers), 0x084);
    }

    #[test]
    fn layer2_offset_is_0x104() {
        let _: () = LTDC_LAYER2_OFFSET_IS_0X104;
        let layer1 = offset_of!(LtdcRegs, layers);
        assert_eq!(layer1 + size_of::<LtdcLayerRegs>(), 0x104);
    }

    #[test]
    fn cfbar_at_layer_offset_0x28() {
        // The framebuffer address register sits at +0x28 within each
        // layer; combined with layer 1 at 0x084 that's 0x0AC absolute,
        // matching the original `LTDC_L1CFBAR` constant.
        assert_eq!(offset_of!(LtdcLayerRegs, cfbar), 0x28);
        assert_eq!(offset_of!(LtdcRegs, layers) + 0x28, 0x0AC);
    }
}
