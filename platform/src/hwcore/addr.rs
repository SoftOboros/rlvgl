//! Address-domain newtypes.
//!
//! Three distinct address species flow through the 747I platform layer and
//! must not collapse into a single numeric type:
//!
//! - [`MmioAddr<T>`] — a memory-mapped register-block base. Carries
//!   provenance via `NonNull<T>`; constructed through an `unsafe` entry
//!   point so callers assert the region is a live, unaliased MMIO block
//!   of `size_of::<T>()` bytes.
//! - [`PhysAddr`] — a CPU-visible RAM address (SDRAM, SRAM). Used for
//!   framebuffer placement and bank-collision math. Conversion to a
//!   usable pointer is `unsafe`.
//! - [`DmaAddr`] — an address as seen by a DMA master. On the STM32H747
//!   this equals the physical address numerically, but the separate type
//!   forces an alignment check at the boundary (`DmaAddr::from_phys`).
//!
//! The SDRAM bank-collision helper on `PhysAddr` replaces the inline magic
//! math previously at `examples/stm32h747i-disco/src/main.rs:2890-2891`
//! (`(back_addr - 0xD000_0000) / SDRAM_BANK_STRIDE`).

use core::marker::PhantomData;
use core::ptr::NonNull;

/// Base address of a memory-mapped register block.
///
/// The `T` parameter is typically a `#[repr(C)]` struct laying out the
/// peripheral registers in order.
///
/// Construction is `unsafe` because the caller asserts that the address
/// names a live MMIO region of the appropriate size and that no other
/// `MmioAddr<T>` referring to the same region exists in the program.
#[derive(Debug)]
#[repr(transparent)]
pub struct MmioAddr<T> {
    ptr: NonNull<T>,
    _phantom: PhantomData<T>,
}

impl<T> MmioAddr<T> {
    /// Construct an [`MmioAddr<T>`] from a raw address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `addr`:
    ///
    /// - names a memory-mapped region of at least `size_of::<T>()` bytes
    ///   that is mapped for the lifetime of the returned value,
    /// - is not aliased by any other `MmioAddr<T>` or `&mut T` in the
    ///   program,
    /// - is appropriately aligned for `T`.
    ///
    /// `addr` of zero panics (via `NonNull::new_unchecked`-adjacent check)
    /// to catch obvious mistakes; this does not otherwise validate the
    /// pointer.
    pub const unsafe fn new(addr: usize) -> Self {
        debug_assert!(addr != 0, "MmioAddr::new called with zero address");
        // SAFETY: caller contract; the debug_assert above catches null.
        let ptr = unsafe { NonNull::new_unchecked(addr as *mut T) };
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Raw pointer to the register block.
    ///
    /// The returned pointer carries MMIO provenance; dereferencing it
    /// requires `unsafe` and is only valid per the construction contract
    /// of this `MmioAddr<T>`.
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Numeric address of the register block.
    ///
    /// Escape hatch for writing the base address into another register
    /// (e.g. `LTDC_L1CFBAR`). Prefer typed APIs over this when available.
    #[inline]
    pub fn raw(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
}

// `MmioAddr<T>` is `Send`/`Sync` by contract: the construction SAFETY note
// requires the caller to prove uniqueness, so the type itself imposes no
// interior-mutability hazard beyond what individual register fields do.
unsafe impl<T> Send for MmioAddr<T> {}
unsafe impl<T> Sync for MmioAddr<T> {}

// ─── PhysAddr ────────────────────────────────────────────────────────────

/// CPU-visible physical address of RAM (SDRAM, SRAM, etc.).
///
/// Distinct from [`DmaAddr`] and [`MmioAddr<T>`] so that a value intended
/// as a framebuffer origin cannot be silently assigned to a DMA source or
/// a register-block base.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[repr(transparent)]
pub struct PhysAddr(u32);

/// STM32H747I-DISCO SDRAM (Bank 2) base address.
///
/// Re-asserts the `feedback_sdram_bank2.md` invariant: the discovery board
/// wires SDNE1/SDCKE1 (Bank 2), so SDRAM lives at `0xD000_0000`, not the
/// Bank 1 address `0xC000_0000`.
pub const SDRAM_BANK2_BASE: u32 = 0xD000_0000;

/// Stride between independent SDRAM banks used for front/back framebuffer
/// placement to avoid refresh-cycle collisions. Value matches the
/// `SDRAM_BANK_STRIDE` previously inlined in the example binary.
pub const SDRAM_BANK_STRIDE: u32 = 0x0080_0000;

/// Number of independent banks in the SDRAM array (IS42S32800J-6BLI has 4).
pub const SDRAM_BANK_COUNT: u8 = 4;

impl PhysAddr {
    /// Construct a [`PhysAddr`] from a raw 32-bit address.
    #[inline]
    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }

    /// Raw 32-bit address.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Offset this address by `delta` bytes, returning a new [`PhysAddr`].
    ///
    /// Saturating on overflow. No aliasing, MMU, or mapping check is
    /// performed — callers stay responsible for validity.
    #[inline]
    pub const fn offset(self, delta: u32) -> Self {
        Self(self.0.saturating_add(delta))
    }

    /// SDRAM bank index (0..[`SDRAM_BANK_COUNT`]) for Bank-2-based SDRAM,
    /// or `None` if this address is not inside the SDRAM Bank 2 window.
    ///
    /// Replaces the inline `(addr - 0xD000_0000) / SDRAM_BANK_STRIDE`
    /// expression that framebuffer collision detection previously used.
    #[inline]
    pub const fn sdram_bank(self) -> Option<u8> {
        let base = SDRAM_BANK2_BASE;
        let stride = SDRAM_BANK_STRIDE;
        let count = SDRAM_BANK_COUNT as u32;
        if self.0 < base {
            return None;
        }
        let offset = self.0 - base;
        let bank = offset / stride;
        if bank >= count {
            return None;
        }
        Some(bank as u8)
    }

    /// Reconstruct a mutable byte slice spanning `len` bytes starting at
    /// this address.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///
    /// - the region `[self, self+len)` is mapped and writable for `'a`,
    /// - no other `&_` or `&mut _` to any overlapping bytes exists for
    ///   `'a`,
    /// - the region is not concurrently written by a DMA master (see
    ///   [`DmaAddr`] and the `InFlight<T>` handoff pattern).
    #[inline]
    pub unsafe fn as_mut_slice<'a>(self, len: usize) -> &'a mut [u8] {
        // SAFETY: delegated to caller per the contract above.
        unsafe { core::slice::from_raw_parts_mut(self.0 as *mut u8, len) }
    }
}

// ─── DmaAddr ─────────────────────────────────────────────────────────────

/// Alignment / validity error returned by [`DmaAddr::from_phys`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AddrError {
    /// The physical address is not aligned to the required boundary.
    Misaligned {
        /// The address that failed the check.
        addr: u32,
        /// The alignment the caller requested.
        required: usize,
    },
    /// The requested alignment is not a power of two.
    InvalidAlignment(usize),
}

/// Address as seen by a DMA master (DMA2D, BDMA, MDMA).
///
/// Numerically identical to the CPU physical address on the STM32H747 (no
/// IOMMU, no bus translation), but kept as a distinct type so that the
/// transition from "CPU owns this" to "DMA owns this" is an explicit,
/// alignment-checked conversion rather than an implicit cast.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct DmaAddr(u32);

impl DmaAddr {
    /// Convert a [`PhysAddr`] into a [`DmaAddr`] after verifying `align`.
    ///
    /// `align` is typically the DMA pixel or burst size — 2 for RGB565, 4
    /// for ARGB8888 / DMA2D OMAR.
    #[inline]
    pub fn from_phys(p: PhysAddr, align: usize) -> Result<Self, AddrError> {
        if align == 0 || !align.is_power_of_two() {
            return Err(AddrError::InvalidAlignment(align));
        }
        let addr = p.raw();
        if (addr as usize) & (align - 1) != 0 {
            return Err(AddrError::Misaligned {
                addr,
                required: align,
            });
        }
        Ok(Self(addr))
    }

    /// Raw 32-bit address for writing into a DMA register field.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdram_bank_zero_is_bank_zero() {
        assert_eq!(PhysAddr::new(SDRAM_BANK2_BASE).sdram_bank(), Some(0));
    }

    #[test]
    fn sdram_bank_one_is_bank_one() {
        let a = SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE;
        assert_eq!(PhysAddr::new(a).sdram_bank(), Some(1));
    }

    #[test]
    fn sdram_bank_offset_within_bank_still_counts() {
        let a = SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE + 0x1234;
        assert_eq!(PhysAddr::new(a).sdram_bank(), Some(1));
    }

    #[test]
    fn address_below_sdram_has_no_bank() {
        assert_eq!(PhysAddr::new(0x2000_0000).sdram_bank(), None);
    }

    #[test]
    fn address_above_sdram_array_has_no_bank() {
        let a = SDRAM_BANK2_BASE + SDRAM_BANK_STRIDE * (SDRAM_BANK_COUNT as u32);
        assert_eq!(PhysAddr::new(a).sdram_bank(), None);
    }

    #[test]
    fn dma_addr_rejects_misaligned() {
        let p = PhysAddr::new(SDRAM_BANK2_BASE + 2);
        assert!(matches!(
            DmaAddr::from_phys(p, 4),
            Err(AddrError::Misaligned { required: 4, .. })
        ));
    }

    #[test]
    fn dma_addr_accepts_aligned() {
        let p = PhysAddr::new(SDRAM_BANK2_BASE);
        let d = DmaAddr::from_phys(p, 4).unwrap();
        assert_eq!(d.raw(), SDRAM_BANK2_BASE);
    }

    #[test]
    fn dma_addr_rejects_non_power_of_two_alignment() {
        let p = PhysAddr::new(SDRAM_BANK2_BASE);
        assert!(matches!(
            DmaAddr::from_phys(p, 3),
            Err(AddrError::InvalidAlignment(3))
        ));
    }

    #[test]
    fn phys_addr_offset_is_saturating() {
        let p = PhysAddr::new(u32::MAX - 3);
        assert_eq!(p.offset(10).raw(), u32::MAX);
    }

    #[test]
    fn mmio_addr_round_trips_address() {
        // SAFETY: test-only; the address is not dereferenced.
        let m: MmioAddr<u32> = unsafe { MmioAddr::new(0x4002_1C00) };
        assert_eq!(m.raw(), 0x4002_1C00);
        assert_eq!(m.as_ptr() as usize, 0x4002_1C00);
    }
}
