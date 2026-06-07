//! Volatile-access field wrappers for `#[repr(C)]` register blocks.
//!
//! `Rw<T>` and `Ro<T>` are `#[repr(transparent)]` newtypes around an
//! `UnsafeCell<T>` so:
//!
//! - `size_of::<Rw<u32>>() == size_of::<u32>() == 4` (no layout overhead),
//! - reads and writes go through `core::ptr::{read,write}_volatile`,
//! - access takes `&self` (shared) because the MMIO region is implicitly
//!   `Sync` from the program's perspective — synchronization between
//!   ISRs and the main loop is a hardware concern, not a Rust borrow
//!   issue (and `MmioAddr<RegBlockRegs>` was constructed under an
//!   `unsafe` contract that the region is unaliased at the type level).
//!
//! The `Wo<T>` write-only variant is provided for symmetry but is rarely
//! useful — most "WO" peripheral registers also tolerate reads (returning
//! garbage). Use `Rw<T>` plus a comment naming the read as undefined when
//! that fits better.

use core::cell::UnsafeCell;

/// Read-write field. Volatile read + volatile write.
#[repr(transparent)]
pub struct Rw<T: Copy>(UnsafeCell<T>);

impl<T: Copy> Rw<T> {
    /// Volatile load.
    #[inline(always)]
    pub fn read(&self) -> T {
        // SAFETY: `MmioAddr<RegBlockRegs>` was constructed under the
        // contract that the underlying region is a live MMIO mapping
        // unaliased by other Rust references. Volatile access is the
        // correct semantics for hardware-mapped memory.
        unsafe { core::ptr::read_volatile(self.0.get()) }
    }

    /// Volatile store.
    #[inline(always)]
    pub fn write(&self, value: T) {
        // SAFETY: see `read`.
        unsafe { core::ptr::write_volatile(self.0.get(), value) }
    }

    /// Read, transform via `f`, then write back. The intermediate
    /// computation runs on a CPU register, not in MMIO space.
    #[inline(always)]
    pub fn modify<F: FnOnce(T) -> T>(&self, f: F) {
        let v = self.read();
        self.write(f(v));
    }
}

/// Read-only field. Volatile read only.
#[repr(transparent)]
pub struct Ro<T: Copy>(UnsafeCell<T>);

impl<T: Copy> Ro<T> {
    /// Volatile load.
    #[inline(always)]
    pub fn read(&self) -> T {
        // SAFETY: see [`Rw::read`].
        unsafe { core::ptr::read_volatile(self.0.get()) }
    }
}

/// Write-only field. Volatile write only.
#[repr(transparent)]
pub struct Wo<T: Copy>(UnsafeCell<T>);

impl<T: Copy> Wo<T> {
    /// Volatile store.
    #[inline(always)]
    pub fn write(&self, value: T) {
        // SAFETY: see [`Rw::read`].
        unsafe { core::ptr::write_volatile(self.0.get(), value) }
    }
}

// Field wrappers are not `Sync` by default because of `UnsafeCell`. The
// register blocks they live in are wrapped by `MmioAddr<T>` whose
// construction contract asserts unaliased MMIO ownership; that handle
// is the synchronization boundary. Within a register block, `&self`
// volatile access from multiple contexts (ISR + main) is the standard
// pattern and is safe under the volatile-access ordering rules.
unsafe impl<T: Copy + Sync> Sync for Rw<T> {}
unsafe impl<T: Copy + Sync> Sync for Ro<T> {}
unsafe impl<T: Copy + Sync> Sync for Wo<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn wrappers_are_zero_overhead() {
        assert_eq!(size_of::<Rw<u32>>(), size_of::<u32>());
        assert_eq!(size_of::<Ro<u32>>(), size_of::<u32>());
        assert_eq!(size_of::<Wo<u32>>(), size_of::<u32>());
    }

    #[test]
    fn rw_round_trip_through_unsafe_cell() {
        // SAFETY: not actually MMIO — host test exercising the wrapper
        // arithmetic. Backing storage is a stack `UnsafeCell<u32>`.
        let cell = Rw(UnsafeCell::new(0u32));
        cell.write(0xDEAD_BEEF);
        assert_eq!(cell.read(), 0xDEAD_BEEF);
        cell.modify(|v| v ^ 0xFFFF_FFFF);
        assert_eq!(cell.read(), 0x2152_4110);
    }
}
