//! `FreeRtosPacing` — FreeRTOS-backed [`Pacing`] impl scaffold.
//!
//! Per [DPR-01 §5.6][dpr01] and [DPR-01-A §4 phase-2 step 1][dpr01a]:
//! this module ships the `FreeRtosPacing<S, T>` type that the DPR-01b
//! migration will route `examples/stm32h747i-disco/src/freertos_entry.rs`
//! through. Today it is **scaffold-only** — the production consumer
//! migration is DPR-01b phase-2 steps 2-5 and is owned by the next
//! agent.
//!
//! ## Generic-trait design
//!
//! `rlvgl-platform` is `no_std` and MUST NOT depend on FreeRTOS's C
//! bindings or its `static` semaphore storage. Instead, the example
//! crate (which already links `libfreertos.a` and owns the
//! `StaticSemaphore` buffers per `freertos_sync.rs`) supplies concrete
//! impls of [`SemaphoreOps`] and [`TimerOps`]. `FreeRtosPacing<S, T>`
//! is generic over those two traits, so the platform crate stays C-
//! library-free while the example wires real `rlvgl_sem_take` /
//! `tim7_arm` calls in.
//!
//! ## Sealed-trait participation
//!
//! `FreeRtosPacing` joins [`BareMetalLoopPacing`][bm] in the seal-set
//! per DPR-00 §5.2 Standards Action (DPR-01 ratifies the registration).
//! The third planned impl, `ZephyrPacing`, is deferred to DPR-01c.
//!
//! [dpr01]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-CONCEPTS.md
//! [dpr01a]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-A.md
//! [bm]: crate::frame_scheduler::BareMetalLoopPacing

use core::sync::atomic::Ordering;
use portable_atomic::AtomicU32;

use crate::frame_scheduler::{ErifInfo, Holdoff, Pacing, sealed};

// ── Capability traits ───────────────────────────────────────────────────

/// Binary-semaphore primitive supplied by the OS-integration crate.
///
/// The example crate's `freertos_sync` module wraps `rlvgl_sem_take` /
/// `rlvgl_sem_give` (see `examples/stm32h747i-disco/src/freertos_sync.rs:48..57`)
/// to satisfy this trait. The platform crate consumes only the
/// abstract operations so it never depends on FreeRTOS itself.
pub trait SemaphoreOps {
    /// Wake any task blocked in [`Self::take_blocking`] or
    /// [`Self::take_with_timeout_ms`]. ISR-side implementations may
    /// route through `rlvgl_sem_give_from_isr` — that's the consumer's
    /// choice; this trait does not distinguish task vs. ISR context.
    fn give(&self);

    /// Block until the semaphore is given (no timeout). FreeRTOS
    /// `xSemaphoreTake(sem, portMAX_DELAY)`.
    fn take_blocking(&self);

    /// Block until the semaphore is given or `ms` milliseconds elapse,
    /// whichever comes first. Returns `true` if the semaphore was taken
    /// before the timeout, `false` if the wait timed out.
    fn take_with_timeout_ms(&self, ms: u32) -> bool;
}

/// One-pulse hardware timer primitive supplied by the OS-integration
/// crate.
///
/// On the DISCO this is wired to TIM7 in one-pulse mode at 1 MHz tick
/// (see `examples/stm32h747i-disco/src/freertos_entry.rs::tim7_arm`,
/// lines ~257-272). The TIM7 ISR is expected to `give` whichever
/// [`SemaphoreOps`] handle was passed to `FreeRtosPacing::new` as the
/// `present_gate_sem` argument — that linkage is wired by the example
/// crate, not by this trait.
pub trait TimerOps {
    /// Arm the one-pulse timer for `us` microseconds. When the timer
    /// fires, the ISR shall give the semaphore the consumer-side
    /// integration tied to it. Values larger than the timer's natural
    /// range MAY be clamped by the impl.
    fn arm_one_pulse_us(&self, us: u32);
}

// ── FreeRtosPacing ──────────────────────────────────────────────────────

/// FreeRTOS-backed pacing — TIM7-armed holdoff, semaphore-based ERIF /
/// render-gate signalling. See [DPR-01 §5.6][dpr01].
///
/// Field roles map to the DPR-01 §5.6 trait surface:
///
/// | Field             | Role                                                                |
/// |-------------------|---------------------------------------------------------------------|
/// | `erif_sem`        | given by DSI ERIF ISR; taken by [`Pacing::wait_erif`].              |
/// | `present_gate_sem`| given by TIM7 OPM ISR; taken inside [`Pacing::wait_holdoff`].       |
/// | `buf_ready_sem`   | given by [`Pacing::signal_buf_ready`]; taken by the present task.   |
/// | `render_start_sem`| given alongside `erif_sem` by the DSI ISR; taken by render task via [`Pacing::wait_render_gate`]. |
/// | `holdoff_timer`   | armed by [`Pacing::wait_holdoff`] for `us` microseconds.            |
/// | `erif_count`      | monotonic counter — DSI ISR calls [`Self::isr_record_erif`].        |
/// | `last_erif_cyccnt`| DWT snapshot at most-recent ERIF — DSI ISR publishes.               |
///
/// All sem/timer ownership stays inside the integration crate; this
/// struct holds them by value so the seal-set check (`sealed::Sealed`)
/// can verify the type is constructible without `unsafe` from outside.
///
/// [dpr01]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-CONCEPTS.md
pub struct FreeRtosPacing<S: SemaphoreOps, T: TimerOps> {
    erif_sem: S,
    present_gate_sem: S,
    buf_ready_sem: S,
    render_start_sem: S,
    holdoff_timer: T,
    /// Monotonic ERIF count — bumped by [`Self::isr_record_erif`].
    erif_count: AtomicU32,
    /// DWT cycle counter snapshot at most-recent ERIF.
    last_erif_cyccnt: AtomicU32,
}

impl<S: SemaphoreOps, T: TimerOps> FreeRtosPacing<S, T> {
    /// Construct a pacing handle from four semaphore handles and one
    /// one-pulse timer handle. The caller's ISR shims must wire:
    ///
    /// - DSI ERIF ISR → `give` on the same `S` value passed as
    ///   `erif_sem` here.
    /// - DSI ERIF ISR → `give` on the same `S` value passed as
    ///   `render_start_sem`.
    /// - TIM7 OPM ISR → `give` on the same `S` value passed as
    ///   `present_gate_sem`.
    ///
    /// The `buf_ready_sem` is signalled from the render task via
    /// [`Pacing::signal_buf_ready`] and taken (non-blockingly) by the
    /// present task — no ISR involvement.
    pub const fn new(
        erif_sem: S,
        present_gate_sem: S,
        buf_ready_sem: S,
        render_start_sem: S,
        holdoff_timer: T,
    ) -> Self {
        Self {
            erif_sem,
            present_gate_sem,
            buf_ready_sem,
            render_start_sem,
            holdoff_timer,
            erif_count: AtomicU32::new(0),
            last_erif_cyccnt: AtomicU32::new(0),
        }
    }

    /// ISR-side ERIF record hook.
    ///
    /// The DSI ERIF ISR calls this with the freshly-sampled DWT
    /// `CYCCNT`; it bumps [`erif_count`][Self] and stores the snapshot
    /// so [`Pacing::wait_erif`] can return a coherent [`ErifInfo`]
    /// after the matching `give`.
    ///
    /// # Safety
    ///
    /// Must be called from the DSI ERIF ISR context. Concurrent callers
    /// from multiple ISR sources would race the count/cyccnt store
    /// ordering — the caller is responsible for guaranteeing single-
    /// producer semantics (the DPR-01b migration installs exactly one
    /// DSI ERIF ISR shim).
    pub unsafe fn isr_record_erif(&self, cyccnt: u32) {
        // Order matters: write the cyccnt first (Release), then bump
        // the count (Release). A reader that observes a fresh count
        // is guaranteed to see the matching cyccnt because the count
        // store happens-after the cyccnt store.
        self.last_erif_cyccnt.store(cyccnt, Ordering::Release);
        self.erif_count.fetch_add(1, Ordering::Release);
    }

    /// Mutable access to the contained `erif_sem` — for the DSI ISR
    /// shim that needs to call `give_from_isr`. The semaphore impl is
    /// expected to be `Sync`-friendly internally (FreeRTOS's
    /// `xSemaphoreGiveFromISR` is); this method is the seam.
    #[inline]
    pub fn erif_sem(&self) -> &S {
        &self.erif_sem
    }

    /// Mutable access to the contained `render_start_sem` — for the
    /// DSI ISR shim, mirroring [`Self::erif_sem`].
    #[inline]
    pub fn render_start_sem(&self) -> &S {
        &self.render_start_sem
    }

    /// Mutable access to the contained `present_gate_sem` — for the
    /// TIM7 OPM ISR shim that gives this sem on deadline-fire.
    #[inline]
    pub fn present_gate_sem(&self) -> &S {
        &self.present_gate_sem
    }
}

impl<S: SemaphoreOps, T: TimerOps> sealed::Sealed for FreeRtosPacing<S, T> {}

impl<S: SemaphoreOps, T: TimerOps> Pacing for FreeRtosPacing<S, T> {
    fn wait_erif(&mut self) -> ErifInfo {
        self.erif_sem.take_blocking();
        // Read order is the reverse of the ISR-side write order in
        // `isr_record_erif`: load the count first (Acquire), then the
        // cyccnt (Acquire). Pairs with the Release stores above.
        let erif_count = self.erif_count.load(Ordering::Acquire);
        let cyccnt = self.last_erif_cyccnt.load(Ordering::Acquire);
        ErifInfo { cyccnt, erif_count }
    }

    fn compute_holdoff_us(&self, _erif: &ErifInfo, holdoff: Holdoff) -> Option<u32> {
        // Identical to BareMetalLoopPacing — the holdoff policy is
        // OS-axis-independent. The OS axis only affects *how* the wait
        // is implemented (DWT spin vs. TIM7 + semaphore).
        match holdoff {
            Holdoff::None => None,
            Holdoff::FixedDelay { us } => Some(us),
        }
    }

    fn wait_holdoff(&mut self, us: u32) {
        // Arm TIM7 for `us` microseconds; TIM7's UIF ISR gives
        // `present_gate_sem` when the deadline fires. The 50 ms
        // ceiling matches `freertos_entry.rs::present_task` (~line
        // 772) — it's a watchdog that prevents a stuck TIM7 from
        // wedging the present pipeline.
        self.holdoff_timer.arm_one_pulse_us(us);
        let _taken = self.present_gate_sem.take_with_timeout_ms(50);
        // We deliberately ignore the timeout return value here — the
        // present task continues to retrigger the panel even if the
        // gate semaphore didn't fire; DPR-01b's existing behaviour
        // (freertos_entry.rs ~line 772) likewise discards the take
        // return value.
    }

    fn signal_buf_ready(&self) {
        self.buf_ready_sem.give();
    }

    fn wait_render_gate(&mut self) {
        self.render_start_sem.take_blocking();
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    // Mock SemaphoreOps — every method bumps a counter so tests can
    // verify call dispatch without standing up a real FreeRTOS kernel.
    //
    // The mock is intentionally `!Sync` (uses `Cell`) — `FreeRtosPacing`
    // does not require `Sync` on its `S` parameter at the trait level,
    // so the host test suite is free to use single-threaded mocks.
    #[derive(Default)]
    struct MockSem {
        gives: Cell<u32>,
        takes_blocking: Cell<u32>,
        takes_timeout: Cell<u32>,
        /// What `take_with_timeout_ms` should return; default `true`.
        take_timeout_result: Cell<bool>,
    }

    impl MockSem {
        fn new() -> Self {
            Self {
                gives: Cell::new(0),
                takes_blocking: Cell::new(0),
                takes_timeout: Cell::new(0),
                take_timeout_result: Cell::new(true),
            }
        }
    }

    impl SemaphoreOps for MockSem {
        fn give(&self) {
            self.gives.set(self.gives.get() + 1);
        }
        fn take_blocking(&self) {
            self.takes_blocking.set(self.takes_blocking.get() + 1);
        }
        fn take_with_timeout_ms(&self, _ms: u32) -> bool {
            self.takes_timeout.set(self.takes_timeout.get() + 1);
            self.take_timeout_result.get()
        }
    }

    #[derive(Default)]
    struct MockTimer {
        arms: Cell<u32>,
        last_arm_us: Cell<u32>,
    }

    impl MockTimer {
        fn new() -> Self {
            Self {
                arms: Cell::new(0),
                last_arm_us: Cell::new(0),
            }
        }
    }

    impl TimerOps for MockTimer {
        fn arm_one_pulse_us(&self, us: u32) {
            self.arms.set(self.arms.get() + 1);
            self.last_arm_us.set(us);
        }
    }

    fn build_pacing() -> FreeRtosPacing<MockSem, MockTimer> {
        FreeRtosPacing::new(
            MockSem::new(),
            MockSem::new(),
            MockSem::new(),
            MockSem::new(),
            MockTimer::new(),
        )
    }

    #[test]
    fn freertos_pacing_holdoff_dispatch() {
        // Mirrors `frame_scheduler::tests::bare_metal_pacing_holdoff_dispatch`
        // — the holdoff policy is shared.
        let pacing = build_pacing();
        let erif = ErifInfo {
            cyccnt: 0,
            erif_count: 0,
        };
        assert_eq!(pacing.compute_holdoff_us(&erif, Holdoff::None), None);
        assert_eq!(
            pacing.compute_holdoff_us(&erif, Holdoff::FixedDelay { us: 32_000 }),
            Some(32_000)
        );
    }

    #[test]
    fn wait_erif_takes_erif_sem_blocking() {
        let mut pacing = build_pacing();
        let info = pacing.wait_erif();
        assert_eq!(pacing.erif_sem.takes_blocking.get(), 1);
        // No ISR record yet — count and cyccnt are both zero.
        assert_eq!(info.cyccnt, 0);
        assert_eq!(info.erif_count, 0);
    }

    #[test]
    fn wait_holdoff_arms_timer_then_takes_present_gate_with_timeout() {
        let mut pacing = build_pacing();
        pacing.wait_holdoff(1_234);
        assert_eq!(pacing.holdoff_timer.arms.get(), 1);
        assert_eq!(pacing.holdoff_timer.last_arm_us.get(), 1_234);
        assert_eq!(pacing.present_gate_sem.takes_timeout.get(), 1);
        // wait_holdoff must NOT touch any other sem.
        assert_eq!(pacing.erif_sem.takes_blocking.get(), 0);
        assert_eq!(pacing.buf_ready_sem.gives.get(), 0);
        assert_eq!(pacing.render_start_sem.takes_blocking.get(), 0);
    }

    #[test]
    fn signal_buf_ready_gives_buf_ready_sem() {
        let pacing = build_pacing();
        pacing.signal_buf_ready();
        assert_eq!(pacing.buf_ready_sem.gives.get(), 1);
        // No other side-effects.
        assert_eq!(pacing.erif_sem.gives.get(), 0);
        assert_eq!(pacing.present_gate_sem.gives.get(), 0);
        assert_eq!(pacing.render_start_sem.gives.get(), 0);
    }

    #[test]
    fn wait_render_gate_takes_render_start_blocking() {
        let mut pacing = build_pacing();
        pacing.wait_render_gate();
        assert_eq!(pacing.render_start_sem.takes_blocking.get(), 1);
        assert_eq!(pacing.erif_sem.takes_blocking.get(), 0);
    }

    #[test]
    fn isr_record_erif_round_trip() {
        let mut pacing = build_pacing();
        unsafe {
            pacing.isr_record_erif(0xCAFE_F00D);
        }
        let info = pacing.wait_erif();
        assert_eq!(info.cyccnt, 0xCAFE_F00D);
        assert_eq!(info.erif_count, 1);

        // A second record/wait pair bumps the count.
        unsafe {
            pacing.isr_record_erif(0xDEAD_BEEF);
        }
        let info2 = pacing.wait_erif();
        assert_eq!(info2.cyccnt, 0xDEAD_BEEF);
        assert_eq!(info2.erif_count, 2);
    }

    #[test]
    fn accessor_helpers_return_owned_handles() {
        // Compile-time check: the accessor signatures are stable so
        // the DSI/TIM7 ISR shims in DPR-01b can reach the semaphores
        // without breaking the seal.
        let pacing = build_pacing();
        let _: &MockSem = pacing.erif_sem();
        let _: &MockSem = pacing.render_start_sem();
        let _: &MockSem = pacing.present_gate_sem();
    }
}
