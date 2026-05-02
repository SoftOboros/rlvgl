//! DMA Cacheable Buffers — typestate ownership for cache-coherent DMA.
//!
//! Lands the contract ratified in
//! [`docs/concepts/DCB-00-CONCEPTS.md`](../../../docs/concepts/DCB-00-CONCEPTS.md):
//! D-cache maintenance for DMA buffers is a property of the type system
//! rather than a property of the call site. Each [`DcaBuf`] is in exactly
//! one of {[`Cpu`], [`DeviceRead`], [`DeviceWrite`], [`CircRead`],
//! [`CircWrite`]} at a time, and the typestate transitions emit the
//! correct cache op (clean before device-read, invalidate before
//! CPU-read-after-device-write) automatically.
//!
//! ## Single-master scope
//!
//! Per DCB-00 §7 (non-goal): a `DcaBuf` has at most one DMA master at a
//! time. Multi-master coherency and cross-core (CM7↔CM4) sharing live
//! outside this contract — DAA-03 §7 / INV-D14 governs cross-core
//! regions, which live in D3 SRAM4 and are non-cacheable from CM7.
//!
//! ## Cache-controller plumbing
//!
//! [`DcaCache`] abstracts the SCB primitives so host tests can run the
//! typestate round-trip without a real Cortex-M present. On target,
//! [`cortex_m::peripheral::SCB`] is the concrete implementer; the
//! `DcaCache` impl is the **only** site in `rlvgl-platform` that may
//! call `clean_dcache_by_*` / `invalidate_dcache_by_*` on the SCB
//! (DCB-00 §9 INV-D8: scanner rule `raw_dcache` whitelists this module).
//!
//! ## INV-D13 — SCB ownership consolidation
//!
//! New code MUST NOT take a `&mut SCB` borrow outside of constructing a
//! [`DcaCacheCtx`] or a DCB-owning engine driver. The follow-on scanner
//! rule `raw_scb_for_cache` referenced in DCB-00 §9 INV-D13 is deferred
//! to a separate phase; in DCB-01 the convention is enforced by review.
//!
//! ## Status
//!
//! DCB-01 lands the typestate API + the `raw_dcache` scanner rule with
//! a starting `BASELINE` covering the three pre-existing manual cache
//! sites (DCB-00 §4 / §9 INV-D10). DCB-02..DCB-02c retrofit those
//! sites onto [`DcaBuf`] and shrink `BASELINE` to empty.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::size_of;

use crate::hwcore::addr::{DmaAddr, PhysAddr};

// ── Constants ──────────────────────────────────────────────────────────

/// Cortex-M7 D-cache line size, in bytes.
///
/// Architectural per ARMv7-M ARM. DCB enforces alignment and padding to
/// this granule via [`DcaBuf`]'s `#[repr(C, align(32))]` and a
/// const-time size assertion.
pub const CACHE_LINE: usize = 32;

// ── Direction markers ──────────────────────────────────────────────────

/// Direction marker for circular DMA: device reads RAM (CPU is the
/// producer; e.g. SAI1 TX, DMA2D source).
pub enum Read {}

/// Direction marker for circular DMA: device writes RAM (CPU is the
/// consumer; e.g. SAI1 RX, ADC stream).
pub enum Write {}

// ── DcaCache trait + DcaCacheCtx ───────────────────────────────────────

/// Cache-controller abstraction.
///
/// On target, the `cortex-m` feature provides an implementation for
/// [`cortex_m::peripheral::SCB`] (the only site in `rlvgl-platform`
/// allowed to call the raw SCB cache APIs — DCB-00 §9 INV-D8).
///
/// On host, [`NullCache`] is a no-op implementer suitable for typestate
/// round-trip tests where the DMA never actually runs.
pub trait DcaCache {
    /// Clean (write-back) cache lines covering `[addr, addr + len)`.
    fn clean(&mut self, addr: usize, len: usize);

    /// Invalidate cache lines covering `[addr, addr + len)`.
    fn invalidate(&mut self, addr: usize, len: usize);

    /// Clean+invalidate cache lines covering `[addr, addr + len)`.
    ///
    /// DCB-00 §6 INV-D5 documents this as idempotent over an aligned,
    /// padded extent. DCB itself does not insert clean+invalidate at
    /// any §5 transition (the directional ops are sufficient under
    /// INV-D3 line-sharing prohibition); this method is exposed for
    /// engine drivers that need bidirectional handoff for unrelated
    /// reasons.
    fn clean_invalidate(&mut self, addr: usize, len: usize);
}

/// No-op [`DcaCache`] for host tests.
///
/// Tracks last-call metadata so tests can assert the right op was
/// emitted at the right transition without a real cache.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NullCache {
    /// Sequence-numbered last operation, useful in unit tests.
    pub last: Option<NullCacheOp>,
}

/// Recorded operation kind for [`NullCache`] assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullCacheOp {
    /// `clean(addr, len)` was called.
    Clean(usize, usize),
    /// `invalidate(addr, len)` was called.
    Invalidate(usize, usize),
    /// `clean_invalidate(addr, len)` was called.
    CleanInvalidate(usize, usize),
}

impl DcaCache for NullCache {
    fn clean(&mut self, addr: usize, len: usize) {
        self.last = Some(NullCacheOp::Clean(addr, len));
    }
    fn invalidate(&mut self, addr: usize, len: usize) {
        self.last = Some(NullCacheOp::Invalidate(addr, len));
    }
    fn clean_invalidate(&mut self, addr: usize, len: usize) {
        self.last = Some(NullCacheOp::CleanInvalidate(addr, len));
    }
}

#[cfg(feature = "cortex-m")]
impl DcaCache for cortex_m::peripheral::SCB {
    fn clean(&mut self, addr: usize, len: usize) {
        // SAFETY: caller (DCB typestate transition) has established that
        // `[addr, addr+len)` is the extent of a `DcaBuf` whose alignment
        // and padding satisfy INV-D1 / INV-D2 — i.e. addr is cache-line
        // aligned and len is a multiple of CACHE_LINE. The DcaBuf is
        // owned by a single typestate handle, so no concurrent CPU
        // access can race the clean.
        unsafe {
            cortex_m::peripheral::SCB::clean_dcache_by_address(self, addr, len);
        }
    }
    fn invalidate(&mut self, addr: usize, len: usize) {
        // SAFETY: as above; INV-D3 forbids cache-line sharing so this
        // invalidate cannot discard live data belonging to an adjacent
        // owner.
        unsafe {
            cortex_m::peripheral::SCB::invalidate_dcache_by_address(self, addr, len);
        }
    }
    fn clean_invalidate(&mut self, addr: usize, len: usize) {
        // SAFETY: as above.
        unsafe {
            cortex_m::peripheral::SCB::clean_invalidate_dcache_by_address(self, addr, len);
        }
    }
}

/// Owning wrapper around a [`DcaCache`].
///
/// Per DCB-00 §9 INV-D13, the canonical place to plumb a `&mut SCB`
/// into the platform crate. Engine drivers (e.g. `AudioPlayer`,
/// `SdmmcEngine`) build a `DcaCacheCtx` once at construction; their
/// per-transfer methods take `&mut DcaCacheCtx<...>` rather than
/// `&mut SCB` directly. Application code MUST NOT construct a bare
/// `&mut SCB` outside of (a) a `DcaCacheCtx` constructor or (b) the
/// pre-DCB grandfathered call sites.
pub struct DcaCacheCtx<'a, C: DcaCache> {
    cache: &'a mut C,
}

impl<'a, C: DcaCache> DcaCacheCtx<'a, C> {
    /// Wrap a cache controller for use by DCB transitions.
    #[inline]
    pub fn new(cache: &'a mut C) -> Self {
        Self { cache }
    }

    /// Borrow the underlying cache controller. Reserved for engine
    /// drivers that need to issue DCB-internal cache ops on extents
    /// that are not themselves [`DcaBuf`]s (e.g. an `MPU` carve-out
    /// scratch zone). Call sites count against the discipline scanner
    /// budget unless DCB owns them.
    #[inline]
    pub fn cache_mut(&mut self) -> &mut C {
        self.cache
    }
}

// ── DcaBuf storage ─────────────────────────────────────────────────────

/// Owning, cache-line-aligned, cache-line-padded DMA buffer.
///
/// DCB-00 §3 / §6:
///
/// - **INV-D1** — aligned to [`CACHE_LINE`] via `#[repr(C, align(32))]`.
/// - **INV-D2** — `T*N` MUST be a multiple of [`CACHE_LINE`]. Enforced
///   at construction by [`DcaBuf::new`]; misuse produces a `const`
///   panic at compile time. Pad `N` upward (or wrap `T` in a
///   cache-line-sized newtype) if your element geometry forces a
///   non-multiple.
/// - **INV-D3** — no two `DcaBuf`s share a cache line. Implied by
///   INV-D1 + INV-D2 because every `DcaBuf` starts on a line and
///   covers an integer number of lines.
/// - **INV-D4** — single owner. Enforced by the typestate handles
///   ([`Cpu`] / [`DeviceRead`] / [`DeviceWrite`] / [`CircRead`] /
///   [`CircWrite`]); see [`DcaBuf::cpu`].
///
/// `T` MUST be `Copy` so the storage can be initialised in `const`
/// context without dropping uninitialised values, and so DMA can read
/// or overwrite bytes without disturbing destructors.
#[repr(C, align(32))]
pub struct DcaBuf<T: Copy, const N: usize> {
    storage: UnsafeCell<[T; N]>,
}

// SAFETY: `DcaBuf` is `Sync` because every access path goes through a
// typestate handle that holds an `&mut DcaBuf`. The handle's exclusive
// borrow excludes concurrent CPU access; the DMA-owned typestates
// document that the CPU MUST NOT touch the buffer through the handle.
// Static placement is supported (the typical SAI/audio pattern).
unsafe impl<T: Copy + Send, const N: usize> Sync for DcaBuf<T, N> {}

impl<T: Copy, const N: usize> DcaBuf<T, N> {
    /// Construct a buffer in CPU-owned state from an initial value
    /// array.
    ///
    /// `const fn` so the result is usable as a `static` initialiser.
    /// The compiler-evaluated assertion below enforces INV-D2: the
    /// total byte length MUST be a multiple of [`CACHE_LINE`]. A
    /// failed assertion is a compile-time error, not a runtime panic.
    #[inline]
    pub const fn new(init: [T; N]) -> Self {
        // INV-D2 / INV-D1: post-condition checks. `align_of::<Self>()`
        // is forced to 32 by the `#[repr(C, align(32))]` on the
        // struct; INV-D2 requires the size of the storage payload to
        // be a whole multiple of CACHE_LINE so the buffer occupies an
        // integer number of cache lines.
        assert!(
            (size_of::<T>() * N).is_multiple_of(CACHE_LINE),
            "DcaBuf<T, N>: size_of::<T>() * N must be a multiple of CACHE_LINE (32). Pad N upward or wrap T in a cache-line-sized newtype.",
        );
        assert!(
            size_of::<T>() * N > 0,
            "DcaBuf<T, N>: empty buffers are not permitted (no DMA target).",
        );
        Self {
            storage: UnsafeCell::new(init),
        }
    }

    /// Total byte length of the storage payload.
    ///
    /// Equal to `size_of::<T>() * N` and, by INV-D2, a multiple of
    /// [`CACHE_LINE`].
    #[inline]
    pub const fn byte_len(&self) -> usize {
        size_of::<T>() * N
    }

    /// CPU virtual address of the storage payload, as a `usize`.
    ///
    /// Used by [`DcaBuf::dma_addr`] and by the typestate transitions
    /// to compute the cache-op extent.
    #[inline]
    fn addr_usize(&self) -> usize {
        self.storage.get() as *mut u8 as usize
    }

    /// DMA bus address of the storage payload.
    ///
    /// On Cortex-M7 / STM32H747 the DMA address space coincides
    /// numerically with the CPU physical address space; this method
    /// is the ratified conversion site (DCB-00 §4 source-of-truth row
    /// for `hwcore::addr`). Asserts cache-line alignment, which
    /// follows from INV-D1.
    pub fn dma_addr(&self) -> DmaAddr {
        let addr = self.addr_usize();
        // On the embedded target `addr` fits in u32; on host
        // (64-bit usize) the cast truncates, but the low 32 bits of
        // an aligned address preserve cache-line alignment, so the
        // DmaAddr conversion succeeds. Host tests that read this
        // value must not feed it to real DMA hardware.
        let phys = PhysAddr::new(addr as u32);
        DmaAddr::from_phys(phys, CACHE_LINE).expect(
            "DcaBuf is #[repr(C, align(32))]; storage address is always cache-line aligned",
        )
    }

    /// Take ownership of the buffer as a CPU-owned typestate handle.
    ///
    /// Borrows `self` exclusively; while the returned [`Cpu`] (or any
    /// typestate-transitioned descendant) is alive, no other code may
    /// reach the storage. This is the entry point for all typestate
    /// transitions.
    #[inline]
    pub fn cpu(&mut self) -> Cpu<'_, T, N> {
        Cpu { buf: self }
    }
}

// ── Cpu typestate ──────────────────────────────────────────────────────

/// CPU-owned typestate handle.
///
/// Per DCB-00 §3 glossary: "the CPU may freely read and write the
/// buffer; no DMA master is reading or writing it." Cache state is
/// unconstrained — the CPU's view is authoritative.
pub struct Cpu<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> Cpu<'a, T, N> {
    /// Read-only view of the storage as a fixed-size array.
    #[inline]
    pub fn as_slice(&self) -> &[T; N] {
        // SAFETY: typestate Cpu holds an `&mut DcaBuf` — no other
        // borrow of the storage exists for the lifetime of `self`.
        unsafe { &*self.buf.storage.get() }
    }

    /// Mutable view of the storage as a fixed-size array.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T; N] {
        // SAFETY: as above; `&mut self` is the unique borrow.
        unsafe { &mut *self.buf.storage.get() }
    }

    /// DMA bus address of the storage.
    ///
    /// Available in CPU-owned state for callers that pre-program the
    /// DMA engine before lending. Once lent, the same address is
    /// returned by [`Cpu::lend_for_read`] / [`Cpu::lend_for_write`].
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// Transition to [`DeviceRead`] (DMA reads RAM, CPU is producer).
    ///
    /// Cache op (DCB-00 §5): `clean_dcache_by_address` over the full
    /// padded extent so any CPU-written cache lines are written back
    /// to RAM before the DMA reads them.
    pub fn lend_for_read<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> (DeviceRead<'a, T, N>, DmaAddr) {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        let dma_addr = self.buf.dma_addr();
        (DeviceRead { buf: self.buf }, dma_addr)
    }

    /// Transition to [`DeviceWrite`] (DMA writes RAM, CPU is consumer).
    ///
    /// Cache op (DCB-00 §5): `invalidate_dcache_by_address` over the
    /// full padded extent so any stale CPU cache lines are evicted
    /// before the DMA writes; INV-D3 (no cache-line sharing) prevents
    /// adjacent-line refill from reintroducing stale data during
    /// transfer, so no exit-side op is needed in
    /// [`DeviceWrite::complete`].
    pub fn lend_for_write<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> (DeviceWrite<'a, T, N>, DmaAddr) {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.invalidate(addr, len);
        let dma_addr = self.buf.dma_addr();
        (DeviceWrite { buf: self.buf }, dma_addr)
    }

    /// Transition to [`CircRead`] (continuous DMA read; CPU may access
    /// the inactive half via [`HalfGuard`]).
    ///
    /// Entry cache op: `clean_dcache_by_address` over the full padded
    /// extent (CPU may have pre-filled the buffer before arming).
    pub fn start_circular_read<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> CircRead<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        CircRead { buf: self.buf }
    }

    /// Transition to [`CircWrite`] (continuous DMA write; CPU may
    /// access the inactive half via [`HalfGuard`]).
    ///
    /// Entry cache op: `invalidate_dcache_by_address` over the full
    /// padded extent so the CPU's first read after the first
    /// half-period observes DMA-written data.
    pub fn start_circular_write<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> CircWrite<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.invalidate(addr, len);
        CircWrite { buf: self.buf }
    }
}

// ── DeviceRead / DeviceWrite (one-shot) ────────────────────────────────

/// One-shot DMA-read typestate handle.
///
/// While alive, the CPU MUST NOT access the buffer. Drop or
/// [`DeviceRead::complete`] returns the buffer to [`Cpu`] state with
/// no exit cache op (the device only read RAM; the CPU's cached copy
/// is unchanged from before the transfer).
pub struct DeviceRead<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> DeviceRead<'a, T, N> {
    /// DMA bus address of the buffer.
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// Transition back to [`Cpu`] state.
    ///
    /// Called by an engine completion handler once the DMA "transfer
    /// complete" interrupt or status bit confirms the device has
    /// stopped reading. Per DCB-00 §5 transition table, no cache op
    /// is emitted — the device only read RAM.
    #[inline]
    pub fn complete(self) -> Cpu<'a, T, N> {
        Cpu { buf: self.buf }
    }
}

/// One-shot DMA-write typestate handle.
///
/// While alive, the CPU MUST NOT access the buffer. Drop or
/// [`DeviceWrite::complete`] returns the buffer to [`Cpu`] state with
/// no exit cache op (the entry-side invalidate already prepared the
/// cache for the post-transfer CPU read).
pub struct DeviceWrite<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> DeviceWrite<'a, T, N> {
    /// DMA bus address of the buffer.
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// Transition back to [`Cpu`] state.
    ///
    /// Per DCB-00 §5 transition table: no cache op at exit. The
    /// entry-side invalidate evicted the buffer's cache lines, INV-D3
    /// forbids adjacent-line refill from reintroducing stale data
    /// during transfer, so the CPU's first read after this call hits
    /// RAM and observes the DMA-written data.
    #[inline]
    pub fn complete(self) -> Cpu<'a, T, N> {
        Cpu { buf: self.buf }
    }
}

// ── CircRead / CircWrite + HalfGuard ───────────────────────────────────

/// Identifies which half of a circular buffer is currently *inactive*
/// (i.e. safe for CPU access via a [`HalfGuard`]).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Half {
    /// First half: indices `[0, N/2)`.
    First,
    /// Second half: indices `[N/2, N)`.
    Second,
}

/// Continuous DMA-read typestate handle.
///
/// The DMA engine reads the buffer in circular mode (typically
/// double-buffer for audio TX, video stream-out, etc.). The CPU is
/// the producer and may fill the *inactive* half through a
/// [`HalfGuard<Read, _, _>`] obtained from [`CircRead::half_guard`].
pub struct CircRead<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> CircRead<'a, T, N> {
    /// DMA bus address of the buffer (stable for the typestate's
    /// lifetime; equal to the address returned during the
    /// `start_circular_*` transition).
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// Acquire a guard over the inactive half.
    ///
    /// `half` names which half is currently safe to access — the
    /// caller knows from the engine's stream-position register
    /// (`NDTR` for STM32 DMA, `LIVR` for SAI, etc.) which half the
    /// DMA is currently servicing. Per DCB-00 §6 INV-D7, releasing
    /// the guard with [`HalfGuard::release`] re-checks the
    /// stream-position; the live-recheck infrastructure is engine-
    /// specific and is plumbed through DCB-02 / DCB-03.
    ///
    /// Cache op for [`Read`] direction: [`DcaCache::clean`] over the
    /// inactive half so the DMA engine's next pass over that half
    /// sees CPU-written data.
    pub fn half_guard<'b, C: DcaCache>(
        &'b mut self,
        ctx: &mut DcaCacheCtx<'_, C>,
        half: Half,
    ) -> HalfGuard<'b, Read, T, N> {
        let (addr, len) = half_extent::<T, N>(self.buf.addr_usize(), half);
        ctx.cache.clean(addr, len);
        HalfGuard {
            buf: self.buf,
            half,
            _dir: PhantomData,
        }
    }

    /// Stop the circular transfer and transition back to [`Cpu`].
    ///
    /// Caller MUST stop the engine before calling. Cache op:
    /// [`DcaCache::clean`] over the full padded extent, ensuring any
    /// CPU-written data still in cache is published to RAM before
    /// the buffer leaves DCB ownership.
    pub fn stop_circular<C: DcaCache>(self, ctx: &mut DcaCacheCtx<'_, C>) -> Cpu<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        Cpu { buf: self.buf }
    }
}

/// Continuous DMA-write typestate handle.
///
/// The DMA engine writes the buffer in circular mode. The CPU is the
/// consumer and may drain the *inactive* half through a
/// [`HalfGuard<Write, _, _>`] obtained from [`CircWrite::half_guard`].
pub struct CircWrite<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> CircWrite<'a, T, N> {
    /// DMA bus address of the buffer.
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// Acquire a guard over the inactive half.
    ///
    /// Cache op for [`Write`] direction: [`DcaCache::invalidate`]
    /// over the inactive half so the CPU's next read of that half
    /// observes DMA-written data.
    pub fn half_guard<'b, C: DcaCache>(
        &'b mut self,
        ctx: &mut DcaCacheCtx<'_, C>,
        half: Half,
    ) -> HalfGuard<'b, Write, T, N> {
        let (addr, len) = half_extent::<T, N>(self.buf.addr_usize(), half);
        ctx.cache.invalidate(addr, len);
        HalfGuard {
            buf: self.buf,
            half,
            _dir: PhantomData,
        }
    }

    /// Stop the circular transfer and transition back to [`Cpu`].
    ///
    /// Caller MUST stop the engine before calling. Cache op:
    /// [`DcaCache::invalidate`] over the full padded extent so the
    /// CPU's next read after release observes the final DMA-written
    /// state, not stale cache lines.
    pub fn stop_circular<C: DcaCache>(self, ctx: &mut DcaCacheCtx<'_, C>) -> Cpu<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.invalidate(addr, len);
        Cpu { buf: self.buf }
    }
}

/// RAII guard for half-buffer access in a circular DMA transfer.
///
/// Holds an `&'b mut` reborrow of the parent [`CircRead`] /
/// [`CircWrite`], so a second `half_guard` call is rejected by the
/// borrow checker until this guard is dropped.
///
/// The `DIR` parameter is one of [`Read`] / [`Write`] and selects
/// which cache op is emitted at construction (DCB-00 §5 transition
/// row for `DeviceActiveCirc<DIR>` HalfGuard).
pub struct HalfGuard<'a, DIR, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
    half: Half,
    _dir: PhantomData<DIR>,
}

impl<'a, DIR, T: Copy, const N: usize> HalfGuard<'a, DIR, T, N> {
    /// Read-only view of the guarded half.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let (lo, hi) = half_indices::<N>(self.half);
        // SAFETY: guard borrows the parent CircRead/CircWrite mutably,
        // which itself holds an `&mut DcaBuf` — no other access path
        // to the storage exists for `self`'s lifetime. Slicing
        // [lo, hi) is in bounds: lo,hi ∈ [0, N] by half_indices.
        unsafe {
            let storage = &*self.buf.storage.get();
            core::slice::from_raw_parts(storage.as_ptr().add(lo), hi - lo)
        }
    }

    /// Mutable view of the guarded half.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let (lo, hi) = half_indices::<N>(self.half);
        // SAFETY: as above; `&mut self` is the unique borrow.
        unsafe {
            let storage = &mut *self.buf.storage.get();
            core::slice::from_raw_parts_mut(storage.as_mut_ptr().add(lo), hi - lo)
        }
    }

    /// Which half this guard exposes.
    #[inline]
    pub fn half(&self) -> Half {
        self.half
    }

    /// Release the guard with a stream-position checkpoint.
    ///
    /// `current_half` is the half the DMA engine is **currently**
    /// servicing; the caller reads it from `NDTR` / `LIVR` / etc.
    /// immediately before this call. If the DMA has crossed into
    /// the half this guard exposes (i.e. `current_half == self.half`),
    /// the post-condition check fires per DCB-00 §6 INV-D7:
    /// `panic!` in `debug_assertions`, error return in release.
    /// The release-mode error path is plumbed through `Result` so
    /// callers can decide whether to propagate or set a fault flag.
    pub fn release(self, current_half: Half) -> Result<(), HalfGuardOverrun> {
        if current_half == self.half {
            #[cfg(debug_assertions)]
            {
                panic!(
                    "DCB HalfGuard overrun: DMA crossed into the inactive half ({:?}) during the guard's lifetime; INV-D7 violated. Stream is faster than the CPU consumer/producer.",
                    self.half
                );
            }
            #[cfg(not(debug_assertions))]
            {
                return Err(HalfGuardOverrun { half: self.half });
            }
        }
        Ok(())
    }
}

/// Error returned by [`HalfGuard::release`] in release builds when the
/// DMA stream crossed into the guarded half during the guard's
/// lifetime — INV-D7 violation.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HalfGuardOverrun {
    /// The half the guard exposed at construction.
    pub half: Half,
}

// ── Internal helpers ──────────────────────────────────────────────────

/// Compute (addr, len) of one half of a `DcaBuf<T, N>` starting at
/// `base_addr`. INV-D2 ensures the byte length is a multiple of
/// CACHE_LINE; for the half-extent to also be a multiple of CACHE_LINE
/// (so the cache op on one half doesn't touch the other), `N * sizeof(T)`
/// MUST be a multiple of `2 * CACHE_LINE`. Asserted at runtime here;
/// the constructor will be tightened to a `const` assertion when
/// `generic_const_exprs` stabilises.
fn half_extent<T: Copy, const N: usize>(base_addr: usize, half: Half) -> (usize, usize) {
    let total = size_of::<T>() * N;
    debug_assert!(
        total.is_multiple_of(2 * CACHE_LINE),
        "DcaBuf used with half_guard must have N*sizeof(T) divisible by 2*CACHE_LINE so each half is itself cache-line aligned",
    );
    let half_bytes = total / 2;
    let off = match half {
        Half::First => 0,
        Half::Second => half_bytes,
    };
    (base_addr + off, half_bytes)
}

/// Half boundaries as element indices.
const fn half_indices<const N: usize>(half: Half) -> (usize, usize) {
    let mid = N / 2;
    match half {
        Half::First => (0, mid),
        Half::Second => (mid, N),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 32 bytes (16 i16 elements) — minimum INV-D2-compliant size.
    type Buf = DcaBuf<i16, 16>;

    #[test]
    fn dcabuf_alignment_is_32() {
        let buf = Buf::new([0; 16]);
        let addr = (&buf as *const Buf) as usize;
        assert_eq!(addr % CACHE_LINE, 0, "INV-D1: 32-byte alignment");
    }

    #[test]
    fn dcabuf_size_is_multiple_of_cache_line() {
        assert_eq!(core::mem::size_of::<Buf>() % CACHE_LINE, 0);
    }

    #[test]
    fn cpu_round_trip_through_device_read() {
        let mut buf = Buf::new([0; 16]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let (pending, _addr) = cpu.lend_for_read(&mut ctx);
        let _ = pending.complete();
    }

    #[test]
    fn lend_for_read_emits_clean() {
        let mut buf = Buf::new([0; 16]);
        let mut cache = NullCache::default();
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = buf.cpu();
            let (pending, _addr) = cpu.lend_for_read(&mut ctx);
            let _ = pending.complete();
        }
        assert!(matches!(cache.last, Some(NullCacheOp::Clean(_, 32))));
    }

    #[test]
    fn lend_for_write_emits_invalidate() {
        let mut buf = Buf::new([0; 16]);
        let mut cache = NullCache::default();
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = buf.cpu();
            let (pending, _addr) = cpu.lend_for_write(&mut ctx);
            let _ = pending.complete();
        }
        assert!(matches!(cache.last, Some(NullCacheOp::Invalidate(_, 32))));
    }

    #[test]
    fn cpu_can_read_and_write_storage() {
        let mut buf = Buf::new([0; 16]);
        let mut cpu = buf.cpu();
        cpu.as_mut_slice()[3] = 42;
        assert_eq!(cpu.as_slice()[3], 42);
    }

    #[test]
    fn cpu_dma_addr_is_cache_line_aligned() {
        let mut buf = Buf::new([0; 16]);
        let cpu = buf.cpu();
        let addr = cpu.dma_addr();
        assert_eq!(addr.raw() as usize % CACHE_LINE, 0);
    }

    #[test]
    fn circ_read_half_guard_emits_clean_per_half() {
        // 64 bytes → two 32-byte halves, both cache-line aligned.
        type CircBuf = DcaBuf<u8, 64>;
        let mut buf = CircBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let mut circ = cpu.start_circular_read(&mut ctx);
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Clean(_, 64))
        ));

        let mut guard = circ.half_guard(&mut ctx, Half::Second);
        guard.as_mut_slice()[0] = 0xAB;
        // Engine reports we're still in First half — release OK.
        guard.release(Half::First).unwrap();
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Clean(_, 32))
        ));

        let _cpu = circ.stop_circular(&mut ctx);
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Clean(_, 64))
        ));
    }

    #[test]
    fn circ_write_half_guard_emits_invalidate_per_half() {
        type CircBuf = DcaBuf<u8, 64>;
        let mut buf = CircBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let mut circ = cpu.start_circular_write(&mut ctx);
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Invalidate(_, 64))
        ));

        {
            let _guard = circ.half_guard(&mut ctx, Half::First);
        }
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Invalidate(_, 32))
        ));

        let _cpu = circ.stop_circular(&mut ctx);
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Invalidate(_, 64))
        ));
    }

    #[test]
    fn half_indices_split_evenly() {
        assert_eq!(half_indices::<16>(Half::First), (0, 8));
        assert_eq!(half_indices::<16>(Half::Second), (8, 16));
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn half_guard_release_returns_err_on_overrun_in_release() {
        type CircBuf = DcaBuf<u8, 64>;
        let mut buf = CircBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let mut circ = cpu.start_circular_read(&mut ctx);
        let guard = circ.half_guard(&mut ctx, Half::Second);
        // DMA crossed into Second — overrun.
        assert_eq!(
            guard.release(Half::Second),
            Err(HalfGuardOverrun {
                half: Half::Second
            })
        );
    }
}
