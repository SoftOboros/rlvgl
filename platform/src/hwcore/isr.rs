//! ISR ↔ main-loop shared-state primitives.
//!
//! Encapsulates the volatile + memory-barrier plumbing that previously
//! lived inline in `examples/stm32h747i-disco/src/main.rs`
//! (`TOUCH_RING` + `touch_ring_push/pop`, `ERIF_FLAG`/`ERIF_CYCCNT`,
//! USART1 RX ring) so the discipline scanner can keep `static mut` and
//! `compiler_fence(` out of application code.
//!
//! The four types — [`IsrChannel<T, N>`], [`IsrFlag`], [`IsrCounter`],
//! [`ScratchCell<T, N>`] — are `Sync` and constructible in `static`
//! context. They do **not** enforce their respective single-borrower /
//! single-producer-single-consumer (SPSC) contracts at the type level;
//! callers must respect:
//!
//! - **`IsrChannel`**: one writer (typically one ISR) and one reader
//!   (typically the main loop). Multiple writers or multiple readers
//!   race the head/tail indices.
//! - **`IsrFlag`** / **`IsrCounter`**: any number of writers/readers
//!   are safe (atomic), but the high-level semantics typically pair an
//!   ISR setter with a main-loop consumer.
//! - **`ScratchCell`**: exactly one borrow live at a time, on a single
//!   execution context (typically the main render loop, never an ISR).
//!   It is the typed replacement for function-local `static mut SCRATCH`
//!   buffers used to dodge alloc traffic in hot paths.
//!
//! The push/pop methods on [`IsrChannel`] and the `borrow_mut` method on
//! [`ScratchCell`] are `unsafe` to make these contracts visible at the
//! call site. `IsrFlag` and `IsrCounter` are safe because their atomic
//! operations are well-defined under contention.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;
// Use `portable-atomic` so RMW ops (`fetch_add`, `swap`) work on targets
// that lack hardware atomics, e.g. `riscv32imc-unknown-none-elf`. On
// targets with `target_has_atomic = "32"` this is a thin re-export of
// `core::sync::atomic`; on others it falls back via `critical-section`.
use portable_atomic::{AtomicBool, AtomicU32};

// ── IsrChannel ──────────────────────────────────────────────────────────

/// Single-producer, single-consumer bounded queue.
///
/// Layout: a power-of-two-or-not slot array plus head/tail indices.
/// Head advances on push (writer side), tail advances on pop (reader
/// side). The queue stores at most `N - 1` elements (one slot is
/// reserved to disambiguate empty from full without an extra bit).
///
/// Memory ordering: writer uses `Release` for the head store, reader
/// uses `Acquire` for the head load — this synchronizes the slot write
/// with the slot read. Likewise for tail. On Cortex-M7 these compile to
/// `dmb ish` instructions which the M7's STREX/LDREX-aware coherence
/// honours; on the host they're full atomic fences.
pub struct IsrChannel<T: Copy, const N: usize> {
    head: AtomicU32,
    tail: AtomicU32,
    slots: UnsafeCell<[MaybeUninit<T>; N]>,
}

// SAFETY: contract per the module docs — exactly one writer and one
// reader. Within those constraints the head/tail atomic ordering plus
// the implicit ISR-enter / ISR-exit barriers on Cortex-M provide
// correct synchronization. `T: Send` is required because values move
// between the writer context and the reader context.
unsafe impl<T: Copy + Send, const N: usize> Sync for IsrChannel<T, N> {}

impl<T: Copy, const N: usize> IsrChannel<T, N> {
    /// Construct an empty channel. Usable in `static` context.
    pub const fn new() -> Self {
        Self {
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            slots: UnsafeCell::new([MaybeUninit::uninit(); N]),
        }
    }

    /// Capacity (one less than `N`, since one slot is reserved for the
    /// empty/full disambiguator).
    #[inline]
    pub const fn capacity(&self) -> usize {
        N - 1
    }

    /// Push a value at the head of the channel.
    ///
    /// # Safety
    ///
    /// At most one context (typically one ISR) may call this method on
    /// a given `IsrChannel`. Multiple writers race the head index.
    ///
    /// Returns `Err(value)` if the channel is full.
    pub unsafe fn try_push(&self, value: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed) as usize;
        let tail = self.tail.load(Ordering::Acquire) as usize;
        let next_head = (head + 1) % N;
        if next_head == tail {
            return Err(value);
        }
        // SAFETY: head < N (modulo invariant). Single-writer contract
        // means no other context concurrently writes this slot. The
        // `Release` store below publishes the new head after the slot
        // write retires.
        //
        // We use raw pointer arithmetic instead of `&mut [MaybeUninit<T>;
        // N]` to avoid creating a transient mutable reference to the
        // entire slot array — that would invalidate the consumer's
        // simultaneous shared borrow under Stacked / Tree Borrows even
        // though the actual memory accesses (head vs tail slot) don't
        // overlap. Raw pointer accesses are aliasing-safe under the
        // SPSC contract.
        unsafe {
            let slot_ptr = (self.slots.get() as *mut MaybeUninit<T>).add(head);
            slot_ptr.write(MaybeUninit::new(value));
        }
        self.head.store(next_head as u32, Ordering::Release);
        Ok(())
    }

    /// Pop a value from the tail of the channel.
    ///
    /// # Safety
    ///
    /// At most one context (typically the main loop) may call this
    /// method on a given `IsrChannel`. Multiple readers race the tail
    /// index.
    pub unsafe fn try_pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed) as usize;
        let head = self.head.load(Ordering::Acquire) as usize;
        if head == tail {
            return None;
        }
        // SAFETY: tail < N (modulo invariant). The slot was initialised
        // by the matching `try_push` (synchronized via the head Acquire
        // load above), and T: Copy means moving the value out leaves
        // the slot's bit pattern in a state we'll overwrite on the next
        // wrap-around push. The `Release` store below publishes the new
        // tail.
        //
        // Raw pointer access (no `&[MaybeUninit<T>; N]` reborrow) — see
        // the corresponding SAFETY note in `try_push` for why this
        // matters under Stacked / Tree Borrows.
        let value = unsafe {
            let slot_ptr = (self.slots.get() as *const MaybeUninit<T>).add(tail);
            slot_ptr.read().assume_init()
        };
        let next_tail = (tail + 1) % N;
        self.tail.store(next_tail as u32, Ordering::Release);
        Some(value)
    }

    /// `true` if the channel is empty (next pop would return `None`).
    ///
    /// Snapshot only: a writer may push between this check and a
    /// subsequent `try_pop`.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
}

impl<T: Copy, const N: usize> Default for IsrChannel<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

// ── IsrFlag ─────────────────────────────────────────────────────────────

/// One-bit signal from an ISR to the main loop.
///
/// Typical use: ISR calls `set()` to indicate "something happened",
/// main loop calls `take()` to consume it. `take` is atomic — if it
/// returns `true`, the flag is cleared as part of the same operation.
pub struct IsrFlag(AtomicBool);

impl IsrFlag {
    /// Construct a cleared flag.
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// Set the flag (typically called from an ISR).
    #[inline]
    pub fn set(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Atomically check-and-clear. Returns the previous state.
    #[inline]
    pub fn take(&self) -> bool {
        self.0.swap(false, Ordering::AcqRel)
    }

    /// Read the flag without clearing.
    #[inline]
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Explicitly clear the flag.
    #[inline]
    pub fn clear(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Default for IsrFlag {
    fn default() -> Self {
        Self::new()
    }
}

// ── IsrCounter ──────────────────────────────────────────────────────────

/// Monotonic counter shared between an ISR and the main loop.
///
/// Typical use: ISR calls `increment()` per event, main loop calls
/// `read()` for telemetry or `reset()` to consume the count.
pub struct IsrCounter(AtomicU32);

impl IsrCounter {
    /// Construct a zero-initialised counter.
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// Increment by one. Returns the previous value.
    #[inline]
    pub fn increment(&self) -> u32 {
        self.0.fetch_add(1, Ordering::Release)
    }

    /// Add `delta`. Returns the previous value.
    #[inline]
    pub fn add(&self, delta: u32) -> u32 {
        self.0.fetch_add(delta, Ordering::Release)
    }

    /// Read the current value.
    #[inline]
    pub fn read(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    /// Atomically read and clear. Returns the value that was present.
    #[inline]
    pub fn reset(&self) -> u32 {
        self.0.swap(0, Ordering::AcqRel)
    }

    /// Replace the value, returning the previous.
    #[inline]
    pub fn store(&self, value: u32) -> u32 {
        self.0.swap(value, Ordering::AcqRel)
    }
}

impl Default for IsrCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ── ScratchCell ─────────────────────────────────────────────────────────

/// Const-constructible single-borrower scratch buffer.
///
/// Replaces the function-local `static mut SCRATCH: [T; N]` pattern used
/// to keep hot-path rotation / blit / compose helpers from allocating a
/// fresh `Vec` per call. Backed by an `UnsafeCell` rather than `static
/// mut`, so the resulting `static SCRATCH: ScratchCell<...>` does not
/// trip the discipline scanner.
///
/// The cell carries no runtime borrow tracking — `borrow_mut` is
/// `unsafe`, and the caller MUST guarantee that no other borrow into
/// the same instance is live (no re-entrance, no concurrent ISR access).
/// Typical use is a function-local `static` accessed only from the main
/// render loop.
pub struct ScratchCell<T: Copy, const N: usize> {
    storage: UnsafeCell<[T; N]>,
}

// SAFETY: contract per the module docs — exactly one borrow live at a
// time on a single execution context. Within that constraint there are
// no concurrent accesses to the underlying storage. `T: Send` is
// required because the returned `&mut [T]` may move values that started
// life in a different thread of execution at construction time.
unsafe impl<T: Copy + Send, const N: usize> Sync for ScratchCell<T, N> {}

impl<T: Copy, const N: usize> ScratchCell<T, N> {
    /// Construct a scratch cell with every slot initialised to `init`.
    /// Usable in `static` context.
    pub const fn new(init: T) -> Self {
        Self {
            storage: UnsafeCell::new([init; N]),
        }
    }

    /// Total slot count.
    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Borrow the scratch as a mutable slice of `len` elements.
    ///
    /// # Safety
    ///
    /// The caller MUST guarantee that no other `&mut [T]` returned by a
    /// prior call to `borrow_mut` on the same cell is still live, and
    /// that no concurrent context (other thread, ISR) accesses the
    /// cell's storage during the returned borrow.
    ///
    /// # Panics
    ///
    /// Panics if `len > N`.
    //
    // `clippy::mut_from_ref` triggers because we hand out `&mut [T]`
    // through `&self`; that's the whole point of the type (interior
    // mutability for static scratch). The single-borrower contract is
    // documented above and enforced by the caller via `unsafe`.
    #[allow(clippy::mut_from_ref)]
    #[inline]
    pub unsafe fn borrow_mut(&self, len: usize) -> &mut [T] {
        assert!(
            len <= N,
            "ScratchCell::borrow_mut: requested len {len} exceeds capacity {N}",
        );
        // SAFETY: caller upholds the single-borrower contract per the
        // method's safety docs. `len <= N` per the assert above.
        unsafe {
            let ptr = self.storage.get() as *mut T;
            core::slice::from_raw_parts_mut(ptr, len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_starts_empty() {
        let ch: IsrChannel<u32, 4> = IsrChannel::new();
        assert!(ch.is_empty());
        // SAFETY: single-threaded test.
        assert_eq!(unsafe { ch.try_pop() }, None);
    }

    #[test]
    fn channel_fifo_order() {
        let ch: IsrChannel<u32, 4> = IsrChannel::new();
        // SAFETY: single-threaded test.
        unsafe {
            ch.try_push(10).unwrap();
            ch.try_push(20).unwrap();
            ch.try_push(30).unwrap();
            assert_eq!(ch.try_pop(), Some(10));
            assert_eq!(ch.try_pop(), Some(20));
            assert_eq!(ch.try_pop(), Some(30));
            assert_eq!(ch.try_pop(), None);
        }
    }

    #[test]
    fn channel_full_returns_err() {
        // N=4 → capacity 3.
        let ch: IsrChannel<u32, 4> = IsrChannel::new();
        // SAFETY: single-threaded test.
        unsafe {
            ch.try_push(1).unwrap();
            ch.try_push(2).unwrap();
            ch.try_push(3).unwrap();
            assert_eq!(ch.try_push(4), Err(4));
            assert_eq!(ch.capacity(), 3);
        }
    }

    #[test]
    fn channel_wraps_around_on_repeated_push_pop() {
        let ch: IsrChannel<u32, 4> = IsrChannel::new();
        // SAFETY: single-threaded test.
        unsafe {
            for i in 0..16 {
                ch.try_push(i).unwrap();
                assert_eq!(ch.try_pop(), Some(i));
            }
        }
        assert!(ch.is_empty());
    }

    #[test]
    fn flag_set_take_clear_round_trip() {
        let f = IsrFlag::new();
        assert!(!f.is_set());
        f.set();
        assert!(f.is_set());
        assert!(f.take());
        assert!(!f.is_set());
        // Second take returns false because the flag was cleared.
        assert!(!f.take());
    }

    #[test]
    fn counter_accumulates_then_resets() {
        let c = IsrCounter::new();
        c.increment();
        c.increment();
        c.add(3);
        assert_eq!(c.read(), 5);
        assert_eq!(c.reset(), 5);
        assert_eq!(c.read(), 0);
    }

    #[test]
    fn counter_store_replaces_value() {
        let c = IsrCounter::new();
        c.add(7);
        assert_eq!(c.store(100), 7);
        assert_eq!(c.read(), 100);
    }

    #[test]
    fn scratch_cell_borrow_initialised_to_seed() {
        let s: ScratchCell<u8, 8> = ScratchCell::new(0xAA);
        assert_eq!(s.capacity(), 8);
        // SAFETY: single-threaded test, no concurrent borrow.
        let slice = unsafe { s.borrow_mut(8) };
        assert_eq!(slice, &[0xAA; 8]);
    }

    #[test]
    fn scratch_cell_borrow_persists_writes_across_calls() {
        let s: ScratchCell<u32, 4> = ScratchCell::new(0);
        // SAFETY: borrows are taken sequentially, not concurrently.
        unsafe {
            let first = s.borrow_mut(4);
            first[0] = 0x1111;
            first[3] = 0x4444;
        }
        // SAFETY: prior borrow has been dropped.
        let second = unsafe { s.borrow_mut(4) };
        assert_eq!(second, &[0x1111, 0, 0, 0x4444]);
    }

    #[test]
    fn scratch_cell_partial_borrow_returns_prefix() {
        let s: ScratchCell<u8, 16> = ScratchCell::new(0);
        // SAFETY: single-threaded test.
        let slice = unsafe { s.borrow_mut(5) };
        assert_eq!(slice.len(), 5);
    }

    #[test]
    #[should_panic(expected = "exceeds capacity")]
    fn scratch_cell_panics_when_len_exceeds_capacity() {
        let s: ScratchCell<u8, 4> = ScratchCell::new(0);
        // SAFETY: about to panic before any UB.
        let _ = unsafe { s.borrow_mut(5) };
    }
}
