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

    /// Emit a memory barrier sufficient to drain pending writes from
    /// the architectural write buffer (the AXI write buffer on
    /// Cortex-M7; the equivalent on other targets).
    ///
    /// Required by DCB-00 §6 INV-D16: a `clean` alone does not flush
    /// the AXI write buffer on STM32H7 + Write-Through SDRAM; pixel
    /// writes can sit in the buffer past the cache write-back. The
    /// `DeviceLtdcScan<T, N>` typestate's `present()` /
    /// `start_ltdc_scan` / `stop_scan` boundaries call this *in
    /// addition to* the `clean` to enforce ordering for
    /// continuous-read consumers (LTDC, DCMI display-out, parallel
    /// RGB).
    ///
    /// Routing the DSB through this trait keeps raw
    /// `cortex_m::asm::dsb()` (or equivalent) call sites contained
    /// to DCB's owning module — preserving the discipline scanner's
    /// containment guarantee for the architecturally-mandated
    /// barrier intrinsic.
    fn barrier(&mut self);
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
    /// `barrier()` was called.
    Barrier,
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
    fn barrier(&mut self) {
        self.last = Some(NullCacheOp::Barrier);
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
    fn barrier(&mut self) {
        // The architecturally-mandated DSB barrier; on Cortex-M7 this
        // drains the AXI write buffer (the load-bearing primitive
        // for `DeviceLtdcScan<T, N>::present()` per DCB-00 §6
        // INV-D16). Routing through this method (rather than
        // letting consumers call `cortex_m::asm::dsb()` directly)
        // keeps the discipline scanner's `compiler_fence` /
        // barrier-intrinsic containment intact — the raw
        // intrinsic stays inside `hwcore::dca`.
        cortex_m::asm::dsb();
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
        DmaAddr::from_phys(phys, CACHE_LINE)
            .expect("DcaBuf is #[repr(C, align(32))]; storage address is always cache-line aligned")
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

    /// Transition to [`LtdcScan`] (continuous-read consumer; LTDC,
    /// DCMI display-out, parallel RGB).
    ///
    /// Per DCB-00 §5 transition table + INV-D16: emits
    /// [`DcaCache::clean`] over the full padded extent **and** a
    /// [`DcaCache::barrier`] (DSB on Cortex-M; AXI-write-buffer
    /// drain on STM32H7 + Write-Through SDRAM). Both are required;
    /// the clean alone does not enforce ordering on architectures
    /// where the cache write-back queues into a separate write
    /// buffer.
    pub fn start_ltdc_scan<C: DcaCache>(self, ctx: &mut DcaCacheCtx<'_, C>) -> LtdcScan<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        ctx.cache.barrier();
        LtdcScan { buf: self.buf }
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
    /// stream-position.
    ///
    /// **No cache op is emitted at construction.** Per DCB-00 §5
    /// (DCB-01b-A 2026-05-03 amendment), the `Read` direction's
    /// cache op fires at `release` — the CPU is about to write
    /// new data via the guard's slice, so cleaning *now* would
    /// publish stale data. The clean at release publishes the
    /// just-completed CPU writes before the engine wraps to this
    /// half on its next pass.
    ///
    /// `ctx` is unused at construction for `Read` direction (kept
    /// in the signature for symmetry with `CircWrite::half_guard`,
    /// which uses it for the entry-side invalidate).
    pub fn half_guard<'b, C: DcaCache>(
        &'b mut self,
        _ctx: &mut DcaCacheCtx<'_, C>,
        half: Half,
    ) -> HalfGuard<'b, Read, T, N> {
        // INV-D2 / half_extent debug_assert is still useful for
        // sanity-checking the call-site geometry, even though no
        // cache op fires here. Compute and discard.
        let _ = half_extent::<T, N>(self.buf.addr_usize(), half);
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

// ── LtdcScan (continuous-read consumer; DeviceLtdcScan typestate) ──────

/// Continuous-read typestate handle for fixed-rate pixel-stream
/// consumers (LTDC, DCMI display-out, parallel RGB).
///
/// Distinguished from [`CircRead`]:
///
/// - the consumer never pauses (no half-rotation; same buffer is
///   read continuously),
/// - the CPU paints in place between consumer reads rather than
///   producing into an inactive half,
/// - per-frame ordering is enforced by [`LtdcScan::present`] (clean
///   plus DSB) at checkpoints chosen by the caller, not by an
///   engine-side completion flag.
///
/// The `DIR` parameter is **deliberately omitted** — the CPU is
/// always producer for this family. The inverse direction (camera
/// frame-grab via DCMI) would be a separate `LtdcGrab` family;
/// not in scope here, would ratify in a future §15 amendment with
/// a named first user.
pub struct LtdcScan<'a, T: Copy, const N: usize> {
    buf: &'a mut DcaBuf<T, N>,
}

impl<'a, T: Copy, const N: usize> LtdcScan<'a, T, N> {
    /// DMA bus address of the buffer (stable for the typestate's
    /// lifetime; equal to the address returned during the
    /// `start_ltdc_scan` transition). Pass to the consumer engine's
    /// frame-buffer base register (e.g. LTDC `CFBAR`).
    #[inline]
    pub fn dma_addr(&self) -> DmaAddr {
        self.buf.dma_addr()
    }

    /// In-place CPU access to the full buffer.
    ///
    /// Returns a `&mut [T; N]` valid until the next call that
    /// borrows `self` mutably ([`LtdcScan::present`] /
    /// [`LtdcScan::stop_scan`]). The borrow checker rejects
    /// concurrent `paint_full` / `present` calls.
    ///
    /// No cache op is emitted here; ordering between the CPU's
    /// writes and the consumer's reads is enforced at `present`
    /// (DCB-00 §5 transition table).
    #[inline]
    pub fn paint_full(&mut self) -> &mut [T; N] {
        // SAFETY: typestate LtdcScan holds an `&mut DcaBuf` — no
        // other borrow of the storage exists for `self`'s lifetime.
        // The consumer reads the same memory continuously but does
        // so at engine-determined timing; aliasing with the consumer
        // is the documented contract of `DeviceLtdcScan<T, N>` and
        // is mediated by `present()`.
        unsafe { &mut *self.buf.storage.get() }
    }

    /// Per-frame checkpoint: publish CPU writes to RAM for the
    /// consumer's next read.
    ///
    /// Per DCB-00 §6 INV-D16 emits **both** [`DcaCache::clean`]
    /// over the full padded extent AND [`DcaCache::barrier`]
    /// (DSB-equivalent). Does not transition the typestate — the
    /// consumer engine is still reading; only the ordering between
    /// CPU writes and consumer reads is checkpointed.
    pub fn present<C: DcaCache>(&mut self, ctx: &mut DcaCacheCtx<'_, C>) {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        ctx.cache.barrier();
    }

    /// Transition back to [`Cpu`].
    ///
    /// Caller MUST stop the consumer engine before calling. Per
    /// DCB-00 §5: emits a final [`DcaCache::clean`] +
    /// [`DcaCache::barrier`] over the full padded extent so the
    /// next phase's CPU read sees the buffer's terminal state.
    pub fn stop_scan<C: DcaCache>(self, ctx: &mut DcaCacheCtx<'_, C>) -> Cpu<'a, T, N> {
        let addr = self.buf.addr_usize();
        let len = self.buf.byte_len();
        ctx.cache.clean(addr, len);
        ctx.cache.barrier();
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
}

/// Direction-specific `release` for `HalfGuard<Read, _, _>`.
///
/// Per DCB-00 §5 (DCB-01b-A 2026-05-03 amendment) the `Read`
/// direction's cache op fires at `release`, not at construction:
/// the CPU is the producer; cleaning before the writes would
/// publish stale data. The clean here publishes the just-completed
/// CPU writes before the engine wraps to this half on its next
/// pass.
impl<'a, T: Copy, const N: usize> HalfGuard<'a, Read, T, N> {
    /// Release the guard. Emits [`DcaCache::clean`] over the
    /// guarded half's extent (the cache op for the `Read`
    /// direction; publishes CPU writes), then performs the
    /// INV-D7 stream-position checkpoint.
    ///
    /// `current_half` is the half the DMA engine is **currently**
    /// servicing; the caller reads it from `NDTR` / `LIVR` / etc.
    /// immediately before this call. If the DMA has crossed into
    /// the half this guard exposes (i.e. `current_half == self.half`),
    /// the post-condition check fires per DCB-00 §6 INV-D7:
    /// `panic!` in `debug_assertions`, error return in release.
    pub fn release<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
        current_half: Half,
    ) -> Result<(), HalfGuardOverrun> {
        // Cache op first: publish the just-completed CPU writes
        // before checking the live stream-position. If the engine
        // has flipped, we still want the clean to fire so the
        // stale-data window is bounded — the overrun is reported
        // separately for the caller to act on.
        let (addr, len) = half_extent::<T, N>(self.buf.addr_usize(), self.half);
        ctx.cache.clean(addr, len);
        check_half_overrun(self.half, current_half)
    }
}

/// Direction-specific `release` for `HalfGuard<Write, _, _>`.
///
/// Per DCB-00 §5 the `Write` direction's cache op fires at
/// construction (`invalidate` over the inactive half). `release`
/// performs only the INV-D7 stream-position checkpoint — the CPU
/// has only read; no dirty lines need publishing.
impl<'a, T: Copy, const N: usize> HalfGuard<'a, Write, T, N> {
    /// Release the guard. No cache op (the entry-side invalidate
    /// already drained stale lines). Performs the INV-D7
    /// stream-position checkpoint.
    ///
    /// `_ctx` is unused for `Write` direction (kept for symmetry
    /// with `HalfGuard<Read>::release`).
    pub fn release<C: DcaCache>(
        self,
        _ctx: &mut DcaCacheCtx<'_, C>,
        current_half: Half,
    ) -> Result<(), HalfGuardOverrun> {
        check_half_overrun(self.half, current_half)
    }
}

/// Internal helper: direction-independent INV-D7 live-recheck.
#[inline]
fn check_half_overrun(guard_half: Half, current_half: Half) -> Result<(), HalfGuardOverrun> {
    if current_half == guard_half {
        #[cfg(debug_assertions)]
        {
            panic!(
                "DCB HalfGuard overrun: DMA crossed into the inactive half ({guard_half:?}) during the guard's lifetime; INV-D7 violated. Stream is faster than the CPU consumer/producer."
            );
        }
        #[cfg(not(debug_assertions))]
        {
            return Err(HalfGuardOverrun { half: guard_half });
        }
    }
    Ok(())
}

/// Error returned by [`HalfGuard::release`] in release builds when the
/// DMA stream crossed into the guarded half during the guard's
/// lifetime — INV-D7 violation.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HalfGuardOverrun {
    /// The half the guard exposed at construction.
    pub half: Half,
}

// ── DcaDoubleBuf storage (DMA double-buffer mode) ──────────────────────

/// Identifies which bank of a double-buffer-mode DMA buffer is the
/// engine's *current target* (or, conversely, which bank is inactive
/// and safe for CPU access via a [`BankGuard`]).
///
/// Maps to the STM32 DMA `CR.CT` bit (bit 19 on H7): `M0` ↔ `CT=0`,
/// `M1` ↔ `CT=1`. Per DCB-00 §6 INV-D15 the bit is the source-of-truth
/// for `BankGuard::release` live-recheck.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Bank {
    /// First bank (programmed into the DMA stream's `M0AR` register).
    M0,
    /// Second bank (programmed into the DMA stream's `M1AR` register).
    M1,
}

/// Owning handle for **STM32 DMA double-buffer mode** storage: a pair
/// of cache-line-aligned, cache-line-padded banks of `[T; N]`.
///
/// The two banks MAY live at arbitrary disjoint addresses (DCB-00 §6
/// INV-D14): the typical embedded use is `from_addrs(m0_addr, m1_addr)`
/// wrapping pre-existing fixed-address regions (e.g. SAI1 RX banks at
/// `0x3000_0000` + `0x3000_1000`). Host tests use [`DcaDoubleBuf::new`]
/// over two stack-allocated [`DcaBuf`]s.
///
/// Distinct from [`DcaBuf`] because the engine register layout (M0AR +
/// M1AR + `CT` bit + `MBM` flag) is different from circular mode. A
/// buffer family chooses one or the other at construction and cannot
/// switch — see DCB-00 §5 "Parallel family: `DcaDoubleBuf<T, N>`".
pub struct DcaDoubleBuf<'b, T: Copy, const N: usize> {
    m0: &'b mut DcaBuf<T, N>,
    m1: &'b mut DcaBuf<T, N>,
}

impl<'b, T: Copy, const N: usize> DcaDoubleBuf<'b, T, N> {
    /// Wrap two pre-existing [`DcaBuf`]s as the M0 / M1 banks.
    ///
    /// The borrow checker enforces that no other access path to either
    /// bank exists for the lifetime of the returned `DcaDoubleBuf`.
    /// Host tests use this; embedded code typically uses
    /// [`DcaDoubleBuf::from_addrs`].
    #[inline]
    pub fn new(m0: &'b mut DcaBuf<T, N>, m1: &'b mut DcaBuf<T, N>) -> Self {
        Self { m0, m1 }
    }

    /// Take ownership of this storage as a CPU-owned typestate handle.
    #[inline]
    pub fn cpu(&mut self) -> DbufCpu<'_, 'b, T, N> {
        DbufCpu { buf: self }
    }
}

impl<T: Copy, const N: usize> DcaDoubleBuf<'static, T, N> {
    /// Construct from two fixed bank addresses with `'static` lifetime.
    ///
    /// Both addresses are reinterpreted as `&'static mut DcaBuf<T, N>`;
    /// the typical embedded use case is wrapping pre-existing fixed-
    /// address DMA buffers (e.g. `0x3000_0000` and `0x3000_1000` for
    /// SAI1 RX banks BUF0 and BUF1).
    ///
    /// # Safety
    ///
    /// The caller MUST ensure that:
    ///
    /// - both `m0_addr` and `m1_addr` name disjoint, mapped, writable
    ///   RAM regions of `size_of::<DcaBuf<T, N>>()` bytes each, valid
    ///   for `'static`,
    /// - the two regions do not overlap each other and share no cache
    ///   line with each other or with any other `DcaBuf` /
    ///   `DcaDoubleBuf` (INV-D3 / INV-D14),
    /// - both addresses are aligned to [`CACHE_LINE`] (32 bytes) — the
    ///   `#[repr(C, align(32))]` requirement on `DcaBuf<T, N>`,
    /// - no other `&mut` to overlapping bytes exists for `'static`,
    /// - `from_addrs` is called *at most once* per pair of addresses;
    ///   producing two `DcaDoubleBuf<'static, T, N>` instances over
    ///   the same physical regions would alias the underlying
    ///   `&'static mut`.
    pub unsafe fn from_addrs(m0_addr: usize, m1_addr: usize) -> Self {
        debug_assert!(
            m0_addr.is_multiple_of(CACHE_LINE),
            "DcaDoubleBuf::from_addrs: m0_addr must be cache-line aligned (INV-D14)",
        );
        debug_assert!(
            m1_addr.is_multiple_of(CACHE_LINE),
            "DcaDoubleBuf::from_addrs: m1_addr must be cache-line aligned (INV-D14)",
        );
        debug_assert!(
            m0_addr != m1_addr,
            "DcaDoubleBuf::from_addrs: m0_addr and m1_addr must be distinct",
        );
        // SAFETY: caller contract — addresses name disjoint, mapped,
        // exclusive `'static` regions of `DcaBuf<T, N>` size and
        // alignment.
        let m0 = unsafe { &mut *(m0_addr as *mut DcaBuf<T, N>) };
        // SAFETY: as above.
        let m1 = unsafe { &mut *(m1_addr as *mut DcaBuf<T, N>) };
        Self { m0, m1 }
    }
}

// ── DbufCpu typestate ──────────────────────────────────────────────────

/// CPU-owned typestate handle for a [`DcaDoubleBuf`].
///
/// Per DCB-00 §3 / §5: the CPU may freely read and write either bank;
/// no DMA master is reading or writing them. Cache state is
/// unconstrained — the CPU's view is authoritative.
pub struct DbufCpu<'a, 'b, T: Copy, const N: usize> {
    buf: &'a mut DcaDoubleBuf<'b, T, N>,
}

impl<'a, 'b, T: Copy, const N: usize> DbufCpu<'a, 'b, T, N> {
    /// Read-only view of the M0 bank.
    #[inline]
    pub fn as_m0_slice(&self) -> &[T; N] {
        // SAFETY: typestate DbufCpu holds `&mut DcaDoubleBuf` which
        // itself holds `&mut DcaBuf` for each bank — no other borrow
        // of the storage exists for `self`'s lifetime.
        unsafe { &*self.buf.m0.storage.get() }
    }

    /// Read-only view of the M1 bank.
    #[inline]
    pub fn as_m1_slice(&self) -> &[T; N] {
        // SAFETY: as above.
        unsafe { &*self.buf.m1.storage.get() }
    }

    /// Mutable view of the M0 bank.
    #[inline]
    pub fn as_m0_mut_slice(&mut self) -> &mut [T; N] {
        // SAFETY: as above; `&mut self` is the unique borrow.
        unsafe { &mut *self.buf.m0.storage.get() }
    }

    /// Mutable view of the M1 bank.
    #[inline]
    pub fn as_m1_mut_slice(&mut self) -> &mut [T; N] {
        // SAFETY: as above.
        unsafe { &mut *self.buf.m1.storage.get() }
    }

    /// DMA bus address of the M0 bank (program into the engine's `M0AR`).
    #[inline]
    pub fn m0_dma_addr(&self) -> DmaAddr {
        self.buf.m0.dma_addr()
    }

    /// DMA bus address of the M1 bank (program into the engine's `M1AR`).
    #[inline]
    pub fn m1_dma_addr(&self) -> DmaAddr {
        self.buf.m1.dma_addr()
    }

    /// Transition to [`DbufRead`] (DMA reads RAM, CPU is producer).
    ///
    /// Cache op (DCB-00 §5 transition table): `clean_dcache_by_address`
    /// over **both** banks (CPU may have pre-filled either bank before
    /// arming).
    pub fn start_double_buffer_read<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> DbufRead<'a, 'b, T, N> {
        let m0_addr = self.buf.m0.addr_usize();
        let m1_addr = self.buf.m1.addr_usize();
        let len = self.buf.m0.byte_len();
        ctx.cache.clean(m0_addr, len);
        ctx.cache.clean(m1_addr, len);
        DbufRead { buf: self.buf }
    }

    /// Transition to [`DbufWrite`] (DMA writes RAM, CPU is consumer).
    ///
    /// Cache op (DCB-00 §5 transition table): `invalidate_dcache_by_address`
    /// over **both** banks so the CPU's first read after the first
    /// bank flip observes DMA-written data.
    pub fn start_double_buffer_write<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> DbufWrite<'a, 'b, T, N> {
        let m0_addr = self.buf.m0.addr_usize();
        let m1_addr = self.buf.m1.addr_usize();
        let len = self.buf.m0.byte_len();
        ctx.cache.invalidate(m0_addr, len);
        ctx.cache.invalidate(m1_addr, len);
        DbufWrite { buf: self.buf }
    }
}

// ── DbufRead / DbufWrite + BankGuard ──────────────────────────────────

/// Continuous DMA-read typestate handle for double-buffer mode.
///
/// The DMA engine reads M0 and M1 alternately in continuous transfer.
/// The CPU is the producer and may fill the *inactive* bank through a
/// [`BankGuard<Read, _, _>`] obtained from [`DbufRead::bank_guard`].
pub struct DbufRead<'a, 'b, T: Copy, const N: usize> {
    buf: &'a mut DcaDoubleBuf<'b, T, N>,
}

impl<'a, 'b, T: Copy, const N: usize> DbufRead<'a, 'b, T, N> {
    /// DMA bus address of the M0 bank (stable for the typestate's
    /// lifetime).
    #[inline]
    pub fn m0_dma_addr(&self) -> DmaAddr {
        self.buf.m0.dma_addr()
    }

    /// DMA bus address of the M1 bank.
    #[inline]
    pub fn m1_dma_addr(&self) -> DmaAddr {
        self.buf.m1.dma_addr()
    }

    /// Acquire a guard over the inactive bank.
    ///
    /// `current_target` names the bank the engine is **currently**
    /// servicing (read from the stream's `CT` bit). The guard exposes
    /// the *opposite* bank.
    ///
    /// **No cache op is emitted at construction.** Per DCB-00 §5
    /// (DCB-01b-A 2026-05-03 amendment), the `Read` direction's
    /// cache op fires at `release` — the CPU is about to write
    /// new data via the guard's slice, so cleaning *now* would
    /// publish stale data. The clean at release publishes the
    /// just-completed CPU writes before the engine flips back to
    /// this bank.
    ///
    /// `_ctx` is unused at construction for `Read` direction (kept
    /// for symmetry with `DbufWrite::bank_guard`, which uses it
    /// for the entry-side invalidate).
    pub fn bank_guard<'g, C: DcaCache>(
        &'g mut self,
        _ctx: &mut DcaCacheCtx<'_, C>,
        current_target: Bank,
    ) -> BankGuard<'g, Read, T, N> {
        let exposed_bank = inactive_bank(current_target);
        let exposed = match exposed_bank {
            Bank::M0 => &mut *self.buf.m0,
            Bank::M1 => &mut *self.buf.m1,
        };
        BankGuard {
            bank_storage: exposed,
            bank: exposed_bank,
            _dir: PhantomData,
        }
    }

    /// Stop the double-buffer transfer and transition back to
    /// [`DbufCpu`].
    ///
    /// Caller MUST stop the engine before calling. Cache op:
    /// [`DcaCache::clean`] over both banks.
    pub fn stop_double_buffer<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> DbufCpu<'a, 'b, T, N> {
        let m0_addr = self.buf.m0.addr_usize();
        let m1_addr = self.buf.m1.addr_usize();
        let len = self.buf.m0.byte_len();
        ctx.cache.clean(m0_addr, len);
        ctx.cache.clean(m1_addr, len);
        DbufCpu { buf: self.buf }
    }
}

/// Continuous DMA-write typestate handle for double-buffer mode.
///
/// The DMA engine writes M0 and M1 alternately. The CPU is the
/// consumer and may drain the *inactive* bank through a
/// [`BankGuard<Write, _, _>`].
pub struct DbufWrite<'a, 'b, T: Copy, const N: usize> {
    buf: &'a mut DcaDoubleBuf<'b, T, N>,
}

impl<'a, 'b, T: Copy, const N: usize> DbufWrite<'a, 'b, T, N> {
    /// DMA bus address of the M0 bank.
    #[inline]
    pub fn m0_dma_addr(&self) -> DmaAddr {
        self.buf.m0.dma_addr()
    }

    /// DMA bus address of the M1 bank.
    #[inline]
    pub fn m1_dma_addr(&self) -> DmaAddr {
        self.buf.m1.dma_addr()
    }

    /// Acquire a guard over the inactive bank.
    ///
    /// Cache op for [`Write`] direction: [`DcaCache::invalidate`] over
    /// the inactive bank's full extent so the CPU's next read of that
    /// bank observes DMA-written data.
    pub fn bank_guard<'g, C: DcaCache>(
        &'g mut self,
        ctx: &mut DcaCacheCtx<'_, C>,
        current_target: Bank,
    ) -> BankGuard<'g, Write, T, N> {
        let exposed_bank = inactive_bank(current_target);
        let exposed = match exposed_bank {
            Bank::M0 => &mut *self.buf.m0,
            Bank::M1 => &mut *self.buf.m1,
        };
        let addr = exposed.addr_usize();
        let len = exposed.byte_len();
        ctx.cache.invalidate(addr, len);
        BankGuard {
            bank_storage: exposed,
            bank: exposed_bank,
            _dir: PhantomData,
        }
    }

    /// Stop the double-buffer transfer and transition back to
    /// [`DbufCpu`].
    pub fn stop_double_buffer<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
    ) -> DbufCpu<'a, 'b, T, N> {
        let m0_addr = self.buf.m0.addr_usize();
        let m1_addr = self.buf.m1.addr_usize();
        let len = self.buf.m0.byte_len();
        ctx.cache.invalidate(m0_addr, len);
        ctx.cache.invalidate(m1_addr, len);
        DbufCpu { buf: self.buf }
    }
}

/// RAII guard for inactive-bank access in a double-buffer DMA transfer.
///
/// Holds an `&'g mut` reborrow of one bank's [`DcaBuf`], obtained by
/// reborrowing through the parent [`DbufRead`] / [`DbufWrite`]. A
/// second concurrent `bank_guard` call is rejected by the borrow
/// checker until this guard is dropped.
///
/// The `DIR` parameter is one of [`Read`] / [`Write`] and selects
/// which cache op was emitted at construction (DCB-00 §5 transition
/// row for `DeviceActiveDoubleBuf<DIR>` BankGuard).
pub struct BankGuard<'a, DIR, T: Copy, const N: usize> {
    bank_storage: &'a mut DcaBuf<T, N>,
    bank: Bank,
    _dir: PhantomData<DIR>,
}

impl<'a, DIR, T: Copy, const N: usize> BankGuard<'a, DIR, T, N> {
    /// Read-only view of the guarded bank.
    #[inline]
    pub fn as_slice(&self) -> &[T; N] {
        // SAFETY: guard borrows the parent DbufRead/DbufWrite mutably,
        // which itself holds `&mut DcaDoubleBuf` (and through it
        // `&mut DcaBuf` for each bank) — no other access path to this
        // bank exists for `self`'s lifetime.
        unsafe { &*self.bank_storage.storage.get() }
    }

    /// Mutable view of the guarded bank.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T; N] {
        // SAFETY: as above; `&mut self` is the unique borrow.
        unsafe { &mut *self.bank_storage.storage.get() }
    }

    /// Which bank this guard exposes.
    #[inline]
    pub fn bank(&self) -> Bank {
        self.bank
    }
}

/// Direction-specific `release` for `BankGuard<Read, _, _>`.
///
/// Per DCB-00 §5 (DCB-01b-A 2026-05-03 amendment) the `Read`
/// direction's cache op fires at `release`, not at construction:
/// the CPU is the producer; cleaning before the writes would
/// publish stale data. The clean here publishes the just-completed
/// CPU writes before the engine flips back to this bank.
impl<'a, T: Copy, const N: usize> BankGuard<'a, Read, T, N> {
    /// Release the guard. Emits [`DcaCache::clean`] over the
    /// guarded bank's full extent, then performs the INV-D15
    /// CT-bit checkpoint.
    ///
    /// `current_target` is the bank the DMA engine is **currently**
    /// servicing per the latest read of the stream's `CT` bit. If
    /// the engine has flipped CT into the bank this guard exposes
    /// (i.e. `current_target == self.bank`), the post-condition
    /// check fires: `panic!` in `debug_assertions` builds, error
    /// return in release builds.
    pub fn release<C: DcaCache>(
        self,
        ctx: &mut DcaCacheCtx<'_, C>,
        current_target: Bank,
    ) -> Result<(), BankGuardOverrun> {
        let addr = self.bank_storage.addr_usize();
        let len = self.bank_storage.byte_len();
        ctx.cache.clean(addr, len);
        check_bank_overrun(self.bank, current_target)
    }
}

/// Direction-specific `release` for `BankGuard<Write, _, _>`.
///
/// Per DCB-00 §5 the `Write` direction's cache op fires at
/// construction (`invalidate` over the inactive bank). `release`
/// performs only the INV-D15 CT-bit checkpoint — the CPU has
/// only read; no dirty lines need publishing.
impl<'a, T: Copy, const N: usize> BankGuard<'a, Write, T, N> {
    /// Release the guard. No cache op (the entry-side invalidate
    /// already drained stale lines). Performs the INV-D15 CT-bit
    /// checkpoint.
    ///
    /// `_ctx` is unused for `Write` direction (kept for symmetry
    /// with `BankGuard<Read>::release`).
    pub fn release<C: DcaCache>(
        self,
        _ctx: &mut DcaCacheCtx<'_, C>,
        current_target: Bank,
    ) -> Result<(), BankGuardOverrun> {
        check_bank_overrun(self.bank, current_target)
    }
}

/// Internal helper: direction-independent INV-D15 live-recheck.
#[inline]
fn check_bank_overrun(guard_bank: Bank, current_target: Bank) -> Result<(), BankGuardOverrun> {
    if current_target == guard_bank {
        #[cfg(debug_assertions)]
        {
            panic!(
                "DCB BankGuard overrun: DMA flipped CT into the inactive bank ({guard_bank:?}) during the guard's lifetime; INV-D15 violated. Stream is faster than the CPU consumer/producer."
            );
        }
        #[cfg(not(debug_assertions))]
        {
            return Err(BankGuardOverrun { bank: guard_bank });
        }
    }
    Ok(())
}

/// Error returned by [`BankGuard::release`] in release builds when the
/// DMA engine flipped its `CT` bit into the guarded bank during the
/// guard's lifetime — INV-D15 violation.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct BankGuardOverrun {
    /// The bank the guard exposed at construction.
    pub bank: Bank,
}

/// Internal helper: the inactive bank, given the current target.
#[inline]
const fn inactive_bank(current_target: Bank) -> Bank {
    match current_target {
        Bank::M0 => Bank::M1,
        Bank::M1 => Bank::M0,
    }
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

        // Per DCB-01b-A 2026-05-03: Read direction emits NO cache op
        // at half_guard construction; the clean fires at release.
        let mut guard = circ.half_guard(&mut ctx, Half::Second);
        // The last op observed is still the start_circular_read clean
        // (no new op fired by half_guard).
        assert!(matches!(
            ctx.cache_mut().last,
            Some(NullCacheOp::Clean(_, 64))
        ));
        guard.as_mut_slice()[0] = 0xAB;
        // Engine reports we're still in First half — release OK.
        // The release-time clean for Read direction publishes the
        // CPU writes we just made.
        guard.release(&mut ctx, Half::First).unwrap();
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
        // DMA crossed into Second — overrun. Per DCB-01b-A the
        // release-time clean still fires before the overrun check.
        assert_eq!(
            guard.release(&mut ctx, Half::Second),
            Err(HalfGuardOverrun { half: Half::Second })
        );
    }

    // ── DcaDoubleBuf (DCB-01b) ─────────────────────────────────────────

    type DbBank = DcaBuf<i16, 16>;

    #[test]
    fn dbufcpu_round_trip_through_active_double_buffer_write() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = dca.cpu();
        let active = cpu.start_double_buffer_write(&mut ctx);
        let _cpu_back = active.stop_double_buffer(&mut ctx);
    }

    #[test]
    fn dbufcpu_can_read_and_write_either_bank() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cpu = dca.cpu();
        cpu.as_m0_mut_slice()[3] = 42;
        cpu.as_m1_mut_slice()[7] = 99;
        assert_eq!(cpu.as_m0_slice()[3], 42);
        assert_eq!(cpu.as_m1_slice()[7], 99);
    }

    #[test]
    fn dbufcpu_dma_addrs_are_distinct_and_aligned() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let cpu = dca.cpu();
        let a0 = cpu.m0_dma_addr().raw() as usize;
        let a1 = cpu.m1_dma_addr().raw() as usize;
        assert_eq!(a0 % CACHE_LINE, 0);
        assert_eq!(a1 % CACHE_LINE, 0);
        assert_ne!(a0, a1, "M0 / M1 must address distinct memory");
    }

    #[test]
    fn start_double_buffer_read_cleans_both_banks() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = dca.cpu();
            let active = cpu.start_double_buffer_read(&mut ctx);
            let _ = active.stop_double_buffer(&mut ctx);
        }
        // The last operation observed by the cache during start_*_read
        // is the *second* clean (M1). The first clean (M0) was overwritten.
        // Accept either; both must be `Clean`.
        assert!(matches!(cache.last, Some(NullCacheOp::Clean(_, 32))));
    }

    #[test]
    fn start_double_buffer_write_invalidates_both_banks() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = dca.cpu();
            let active = cpu.start_double_buffer_write(&mut ctx);
            let _ = active.stop_double_buffer(&mut ctx);
        }
        assert!(matches!(cache.last, Some(NullCacheOp::Invalidate(_, 32))));
    }

    #[test]
    fn dbuf_read_bank_guard_emits_clean_on_inactive_bank() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let m0_addr = (&raw const m0) as usize;
        let m1_addr = (&raw const m1) as usize;
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = dca.cpu();
        let mut active = cpu.start_double_buffer_read(&mut ctx);
        // Engine currently on M0 → inactive bank is M1.
        // Per DCB-01b-A 2026-05-03: Read direction emits NO cache op
        // at bank_guard construction; the clean fires at release.
        let mut guard = active.bank_guard(&mut ctx, Bank::M0);
        assert_eq!(guard.bank(), Bank::M1);
        guard.as_mut_slice()[0] = 0xAB;
        // Engine still on M0 — release OK. The release-time clean
        // for Read direction publishes the CPU writes we just made.
        guard.release(&mut ctx, Bank::M0).unwrap();
        // Confirm cache op was a Clean over M1's extent (32 bytes).
        let last = ctx.cache_mut().last.unwrap();
        match last {
            NullCacheOp::Clean(addr, len) => {
                assert_eq!(len, 32);
                assert_eq!(addr, m1_addr, "expected M1 (inactive) cleaned");
                assert_ne!(addr, m0_addr);
            }
            other => panic!("expected Clean, got {other:?}"),
        }
    }

    #[test]
    fn dbuf_write_bank_guard_emits_invalidate_on_inactive_bank() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let m0_addr = (&raw const m0) as usize;
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = dca.cpu();
        let mut active = cpu.start_double_buffer_write(&mut ctx);
        // Engine currently on M1 → inactive bank is M0.
        let guard = active.bank_guard(&mut ctx, Bank::M1);
        assert_eq!(guard.bank(), Bank::M0);
        let last = ctx.cache_mut().last.unwrap();
        match last {
            NullCacheOp::Invalidate(addr, len) => {
                assert_eq!(len, 32);
                assert_eq!(addr, m0_addr, "expected M0 (inactive) invalidated");
            }
            other => panic!("expected Invalidate, got {other:?}"),
        }
        guard.release(&mut ctx, Bank::M1).unwrap();
    }

    #[test]
    fn inactive_bank_swaps_correctly() {
        assert_eq!(inactive_bank(Bank::M0), Bank::M1);
        assert_eq!(inactive_bank(Bank::M1), Bank::M0);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn bank_guard_release_returns_err_on_overrun_in_release() {
        let mut m0 = DbBank::new([0; 16]);
        let mut m1 = DbBank::new([0; 16]);
        let mut dca = DcaDoubleBuf::new(&mut m0, &mut m1);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = dca.cpu();
        let mut active = cpu.start_double_buffer_read(&mut ctx);
        let guard = active.bank_guard(&mut ctx, Bank::M0);
        // Guard exposes M1; engine flipped CT into M1 → overrun.
        // Per DCB-01b-A the release-time clean still fires before
        // the overrun check.
        assert_eq!(
            guard.release(&mut ctx, Bank::M1),
            Err(BankGuardOverrun { bank: Bank::M1 })
        );
    }

    // ── LtdcScan (DCB-01c) ──────────────────────────────────────────

    type ScanBuf = DcaBuf<u8, 64>;

    #[test]
    fn ltdc_scan_round_trip_through_cpu() {
        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let scan = cpu.start_ltdc_scan(&mut ctx);
        let _cpu_back = scan.stop_scan(&mut ctx);
    }

    #[test]
    fn start_ltdc_scan_emits_clean_then_barrier() {
        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = NullCache::default();
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = buf.cpu();
            let scan = cpu.start_ltdc_scan(&mut ctx);
            // After start: last op observed is the barrier (issued
            // *after* the clean per INV-D16).
            assert_eq!(ctx.cache_mut().last, Some(NullCacheOp::Barrier));
            let _ = scan.stop_scan(&mut ctx);
        }
        // After stop: the FINAL op is also a barrier (clean + barrier
        // emitted at exit).
        assert_eq!(cache.last, Some(NullCacheOp::Barrier));
    }

    #[test]
    fn ltdc_scan_paint_full_yields_buffer_slice() {
        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let mut scan = cpu.start_ltdc_scan(&mut ctx);
        let pixels = scan.paint_full();
        pixels[0] = 0xAA;
        pixels[63] = 0x55;
        // Borrow ends here; subsequent paint_full / present ok.
        assert_eq!(scan.paint_full()[0], 0xAA);
        assert_eq!(scan.paint_full()[63], 0x55);
    }

    #[test]
    fn ltdc_scan_present_emits_clean_then_barrier_no_transition() {
        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let mut scan = cpu.start_ltdc_scan(&mut ctx);
        // Drop the start_ltdc_scan op record by observing it then
        // calling present, which must emit clean + barrier again.
        scan.paint_full()[5] = 0x42;
        scan.present(&mut ctx);
        assert_eq!(ctx.cache_mut().last, Some(NullCacheOp::Barrier));
        // Typestate is still LtdcScan — paint_full should still work.
        scan.paint_full()[7] = 0x33;
        scan.present(&mut ctx);
        let _ = scan.stop_scan(&mut ctx);
    }

    #[test]
    fn ltdc_scan_dma_addr_is_cache_line_aligned() {
        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = NullCache::default();
        let mut ctx = DcaCacheCtx::new(&mut cache);
        let cpu = buf.cpu();
        let scan = cpu.start_ltdc_scan(&mut ctx);
        let addr = scan.dma_addr();
        assert_eq!(addr.raw() as usize % CACHE_LINE, 0);
        let _ = scan.stop_scan(&mut ctx);
    }

    #[test]
    fn ltdc_scan_present_clean_extent_is_full_buffer() {
        // Track multiple ops by inspecting them in sequence — switch
        // to a custom local `LoggingCache` that records all calls.
        struct LoggingCache {
            ops: alloc::vec::Vec<NullCacheOp>,
        }
        impl DcaCache for LoggingCache {
            fn clean(&mut self, addr: usize, len: usize) {
                self.ops.push(NullCacheOp::Clean(addr, len));
            }
            fn invalidate(&mut self, addr: usize, len: usize) {
                self.ops.push(NullCacheOp::Invalidate(addr, len));
            }
            fn clean_invalidate(&mut self, addr: usize, len: usize) {
                self.ops.push(NullCacheOp::CleanInvalidate(addr, len));
            }
            fn barrier(&mut self) {
                self.ops.push(NullCacheOp::Barrier);
            }
        }

        let mut buf = ScanBuf::new([0; 64]);
        let mut cache = LoggingCache {
            ops: alloc::vec::Vec::new(),
        };
        {
            let mut ctx = DcaCacheCtx::new(&mut cache);
            let cpu = buf.cpu();
            let mut scan = cpu.start_ltdc_scan(&mut ctx);
            scan.present(&mut ctx);
            let _ = scan.stop_scan(&mut ctx);
        }
        // Expected sequence: start (clean+barrier), present (clean+
        // barrier), stop (clean+barrier) = 6 ops, alternating.
        assert_eq!(cache.ops.len(), 6);
        for (i, op) in cache.ops.iter().enumerate() {
            if i % 2 == 0 {
                assert!(
                    matches!(op, NullCacheOp::Clean(_, 64)),
                    "expected Clean(_, 64) at index {i}, got {op:?}"
                );
            } else {
                assert_eq!(*op, NullCacheOp::Barrier);
            }
        }
    }
}
