//! DPR-01 Frame Scheduler — owns per-frame DSI / LTDC writes.
//!
//! Per [DPR-00 §6 INV-DPR-3][dpr00] and [DPR-01 §5.4][dpr01]: after
//! Board Runtime initialization, the four per-frame display registers
//! (`DSI_WCR`, `DSI_WIER`, `DSI_WIFCR`, `LTDC_L1CFBAR`, `LTDC_SRCR`)
//! MUST be written only by the [`FrameScheduler`]. This module
//! introduces the types; the call-site migration lands incrementally
//! under DPR-01a per [`DPR-01-A.md`][dpr01a] §4.
//!
//! This is **scaffold code** as of DPR-01 ratification: the types
//! compile and self-test, but no consumer routes through them yet.
//! `Stm32h747iDiscoDisplay::{swap, present, wait_frame_done}` migrate
//! under DPR-01a; `freertos_entry` writers migrate under DPR-01b.
//!
//! ## Sealed traits
//!
//! Both [`ScanMode`] and [`Pacing`] are sealed. Extending either is a
//! Standards-Action change per DPR-00 §5.1 / §5.2 — new impls require
//! a §15 amendment with first-app and second-app impact analysis.
//!
//! [dpr00]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-00-CONCEPTS.md
//! [dpr01]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-CONCEPTS.md
//! [dpr01a]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-A.md

use core::marker::PhantomData;

use crate::hwcore::addr::PhysAddr;
use crate::hwcore::regs::dsi::DsiWrapper;
use crate::hwcore::regs::ltdc::Ltdc;

pub(crate) mod sealed {
    /// Sealing trait — prevents downstream impls of [`super::ScanMode`]
    /// and [`super::Pacing`].
    ///
    /// Crate-visible so sibling modules within `rlvgl-platform` (e.g.
    /// [`crate::pacing::freertos`]) can register new [`super::Pacing`]
    /// impls per DPR-01 §5.6 without exposing the seal to downstream
    /// crates. External impls remain Standards-Action per DPR-00 §5.2.
    pub trait Sealed {}
}

// ── ScanMode ────────────────────────────────────────────────────────────

/// Scan-mode axis decomposition per DPR-00 §5.1.
///
/// Two named presets — [`AdaptedCommand`] and [`VideoMode`] — inhabit
/// this trait. Each carries const-generic axis values that drive the
/// per-frame writer specialization in [`FrameScheduler::present`].
pub trait ScanMode: sealed::Sealed {
    /// `true` if `DSI_WCR.LTDCEN` is toggled per frame. `AdaptedCommand`
    /// pulses LTDCEN; `VideoMode` leaves it on.
    const PULSED_LTDCEN: bool;

    /// `true` if a panel TE GPIO arms scan. `AdaptedCommand` uses the
    /// external TE input (PJ2 on this board); `VideoMode` does not.
    const USES_TE_GPIO: bool;

    /// Human-readable name for telemetry and panic messages.
    const NAME: &'static str;
}

/// Adapted-command scan mode — LTDCEN pulsed per frame, TE GPIO armed,
/// optional holdoff timer phases present writes after the ERIF edge.
///
/// This is the demo's current mode for both bare-metal and FreeRTOS
/// builds. The FreeRTOS variant additionally configures a TIM7 one-pulse
/// holdoff (32 ms by default) via [`Holdoff::FixedDelay`].
pub enum AdaptedCommand {}

impl sealed::Sealed for AdaptedCommand {}
impl ScanMode for AdaptedCommand {
    const PULSED_LTDCEN: bool = true;
    const USES_TE_GPIO: bool = true;
    const NAME: &'static str = "AdaptedCommand";
}

/// Video-mode scan — LTDC scans continuously, TE GPIO unused, shadow
/// reload via `SRCR.IMR` retargets the framebuffer at the next frame
/// boundary without disturbing the running scan.
///
/// This is the disco-analyzer's current mode (CM7+CM4 dual-core
/// builds). DPR-01 publishes the type; DPR-03 ratifies analyzer
/// adoption via DAA-01-B-2 §15.
pub enum VideoMode {}

impl sealed::Sealed for VideoMode {}
impl ScanMode for VideoMode {
    const PULSED_LTDCEN: bool = false;
    const USES_TE_GPIO: bool = false;
    const NAME: &'static str = "VideoMode";
}

// ── Holdoff ─────────────────────────────────────────────────────────────

/// Holdoff policy — how long to wait after the ERIF edge before
/// committing the present writes.
///
/// `BareMetalLoopPacing` uses `None`; `FreeRtosPacing` uses
/// `FixedDelay { us: 32_000 }` by default per the demo's existing
/// `freertos_entry.rs` TIM7 configuration.
#[derive(Clone, Copy, Debug)]
pub enum Holdoff {
    /// No holdoff. Present writes commit as soon as the wake source
    /// signals.
    None,
    /// Hold present writes for a fixed duration after ERIF. Implemented
    /// via TIM7 one-pulse mode in the FreeRTOS pacing impl.
    FixedDelay {
        /// Holdoff duration in microseconds. Clamped to TIM7's 16-bit
        /// counter range (0..=65_535) by the pacing impl.
        us: u32,
    },
}

// ── ErifInfo ────────────────────────────────────────────────────────────

/// Snapshot captured by the DSI ERIF ISR, consumed by pacing impls.
///
/// `cyccnt` carries the DWT cycle counter at the ERIF edge for holdoff
/// math; `erif_count` is a monotonic counter for telemetry and missed-
/// frame detection.
#[derive(Clone, Copy, Debug)]
pub struct ErifInfo {
    /// DWT cycle counter snapshot at ERIF.
    pub cyccnt: u32,
    /// Monotonic ERIF event index (wraps every `2^32` events).
    pub erif_count: u32,
}

// ── Pacing ──────────────────────────────────────────────────────────────

/// OS-axis dispatch per DPR-00 §5.2 — selects between bare-metal-loop,
/// FreeRTOS, and Zephyr backends. Each impl owns its own synchronization
/// primitives (atomic flag / semaphore / k_sem); the trait is the seam
/// the FrameScheduler uses to remain OS-agnostic on the hot path.
pub trait Pacing: sealed::Sealed {
    /// Block until the next ERIF (panel scan complete). Returns the
    /// cycle snapshot captured at the ERIF edge.
    fn wait_erif(&mut self) -> ErifInfo;

    /// Compute holdoff duration in microseconds relative to the captured
    /// ERIF, or `None` if no holdoff is configured.
    ///
    /// Returns `Some(0)` if the deadline has already passed — callers
    /// should commit present writes immediately in that case.
    fn compute_holdoff_us(&self, erif: &ErifInfo, holdoff: Holdoff) -> Option<u32>;

    /// Block (or busy-spin) until the holdoff timer fires. The
    /// implementation chooses the timer source (DWT for bare-metal;
    /// TIM7 one-pulse for FreeRTOS).
    fn wait_holdoff(&mut self, us: u32);

    /// Signal that the back buffer is now safe to render into.
    fn signal_buf_ready(&self);

    /// Block until the render-gate is open (back buffer is ready).
    fn wait_render_gate(&mut self);
}

/// Bare-metal pacing — DWT-based holdoff, atomic-flag ERIF signaling.
///
/// Carries no semaphore handles; the bare-metal demo's main loop and
/// DSI ISR coordinate through an [`crate::hwcore::isr::IsrFlag`] passed
/// in at construction time.
///
/// DPR-01a wires the bare-metal demo onto this impl. Until then, the
/// type compiles but has no production consumer.
pub struct BareMetalLoopPacing {
    erif_count: u32,
}

impl sealed::Sealed for BareMetalLoopPacing {}

impl BareMetalLoopPacing {
    /// Construct a fresh pacing handle. The caller is responsible for
    /// installing the DSI ERIF ISR that publishes [`ErifInfo`] (DPR-01a).
    pub const fn new() -> Self {
        Self { erif_count: 0 }
    }
}

impl Default for BareMetalLoopPacing {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacing for BareMetalLoopPacing {
    fn wait_erif(&mut self) -> ErifInfo {
        // DPR-01a wires this to the bare-metal `ERIF_FLAG` / `ERIF_CYCCNT`
        // atomics introduced in `examples/stm32h747i-disco/src/main.rs`.
        // Scaffold returns the current count without a real wait so the
        // type compiles in isolation; the production migration replaces
        // this body.
        self.erif_count = self.erif_count.wrapping_add(1);
        ErifInfo {
            cyccnt: 0,
            erif_count: self.erif_count,
        }
    }

    fn compute_holdoff_us(&self, _erif: &ErifInfo, holdoff: Holdoff) -> Option<u32> {
        match holdoff {
            Holdoff::None => None,
            Holdoff::FixedDelay { us } => Some(us),
        }
    }

    fn wait_holdoff(&mut self, _us: u32) {
        // DPR-01a busy-spins on DWT here.
    }

    fn signal_buf_ready(&self) {
        // DPR-01a flips the `BUF_READY` atomic.
    }

    fn wait_render_gate(&mut self) {
        // DPR-01a polls the bare-metal render-gate atomic.
    }
}

// ── FrameScheduler ──────────────────────────────────────────────────────

/// Per-frame display register owner, generic over scan mode and pacing.
///
/// Holds the typed [`DsiWrapper`] and [`Ltdc`] handles. Every per-frame
/// write to `DSI_WCR`, `DSI_WIER`, `DSI_WIFCR`, `LTDC_L1CFBAR`, and
/// `LTDC_SRCR` is expected to flow through methods on this type
/// (DPR-01a / DPR-01b migration).
///
/// The scheduler does not own init-time writes — `Stm32h747iDiscoDisplay::new`
/// continues to program LTDC/DSI registers at boot. Per DPR-00 §6
/// INV-DPR-3, scheduler ownership begins *after* Board Runtime
/// initialization.
pub struct FrameScheduler<S: ScanMode, P: Pacing> {
    dsi_wrapper: DsiWrapper,
    ltdc: Ltdc,
    pacing: P,
    _mode: PhantomData<S>,
}

impl<S: ScanMode, P: Pacing> FrameScheduler<S, P> {
    /// Construct a frame scheduler.
    ///
    /// # Safety
    ///
    /// The caller MUST assert:
    /// 1. The DSI wrapper and LTDC peripheral blocks are unaliased — at
    ///    most one [`FrameScheduler`] may exist in the program at any
    ///    time, and no other code path may construct its own
    ///    [`DsiWrapper`] or [`Ltdc`] handle after this point.
    /// 2. The DSI and LTDC clocks are enabled
    ///    (`RCC.APB3ENR.{DSIEN, LTDCEN}` set) and
    ///    `Stm32h747iDiscoDisplay::new` has completed init.
    /// 3. The DSI ERIF ISR (if `pacing` requires one) has been installed
    ///    so [`Pacing::wait_erif`] can observe events.
    pub const unsafe fn new(dsi_wrapper: DsiWrapper, ltdc: Ltdc, pacing: P) -> Self {
        Self {
            dsi_wrapper,
            ltdc,
            pacing,
            _mode: PhantomData,
        }
    }

    /// Shared access to the pacing impl for ISR-side handoff or
    /// telemetry. Hot-path methods on the scheduler take `&mut self`.
    #[inline]
    pub fn pacing(&self) -> &P {
        &self.pacing
    }

    /// Mutable access to the pacing impl. DPR-01a wires bare-metal
    /// ERIF reads through this path.
    #[inline]
    pub fn pacing_mut(&mut self) -> &mut P {
        &mut self.pacing
    }

    /// Op A — SWAP (interrupt-safe, no WCR pulse).
    ///
    /// Retargets `LTDC_L1CFBAR` and triggers shadow reload via
    /// `LTDC_SRCR.IMR`. The caller MUST hold the critical-section
    /// invariant (e.g. wrap the call in `cortex_m::interrupt::free`)
    /// if a context-switch between the two writes would corrupt the
    /// frame.
    ///
    /// Safe only when `DSI_WCFGR.AR == 1` (auto-refresh enabled); the
    /// LTDC retargets at the next TE edge without an explicit `WCR`
    /// pulse. See DPR-01-A §3 Op A for the migration rationale.
    #[inline]
    pub fn swap(&mut self, fb: PhysAddr) {
        // Write order: L1CFBAR first, then SRCR.IMR. The hardware
        // commits the new framebuffer at the next shadow-reload edge.
        self.ltdc.layer1().cfbar.write(fb.raw());
        self.ltdc.regs().srcr.write(1);
    }

    /// Op B — PRESENT (full pipeline; LTDCEN pulse is scan-mode-gated).
    ///
    /// Sequence:
    ///
    /// 1. Clear stale `CERIF` (`WIFCR ← 0x02`).
    /// 2. Retarget layer 1 (`L1CFBAR ← fb`).
    /// 3. Trigger shadow reload (`SRCR ← 1`).
    /// 4. If `S::PULSED_LTDCEN` (i.e. `AdaptedCommand`): pulse
    ///    `WCR ← 0x0C` (DSIEN | LTDCEN) and clear the spurious ERIF
    ///    raised by the LTDCEN re-enable (`WIFCR ← 0x02`).
    ///
    /// Per DPR-00 INV-DPR-3, this method is the sole owner of those
    /// five writes outside of init code.
    #[inline]
    pub fn present(&mut self, fb: PhysAddr) {
        // 1. Pre-retarget CERIF clear. Bit 1 of WIFCR is CERIF.
        self.dsi_wrapper.regs().wifcr.write(0x02);
        // 2-3. Retarget + shadow reload trigger.
        self.ltdc.layer1().cfbar.write(fb.raw());
        self.ltdc.regs().srcr.write(1);
        // 4. AdaptedCommand only: pulse LTDCEN, then clear the spurious
        //    ERIF that the re-enable raises. VideoMode skips entirely;
        //    monomorphization elides the branch.
        if S::PULSED_LTDCEN {
            // 0x0C = (DSIEN bit 3 | LTDCEN bit 2).
            self.dsi_wrapper.regs().wcr.write(0x0C);
            self.dsi_wrapper.regs().wifcr.write(0x02);
        }
    }

    /// Op C — CONSUME_ERIF (ISR-side flag clear).
    ///
    /// Clears `WIFCR.CERIF` after the ISR has observed `WISR.ERIF` and
    /// captured its DWT snapshot. Pacing impls call this from the DSI
    /// ISR body before publishing [`ErifInfo`] to whichever channel
    /// they own.
    ///
    /// # Safety
    ///
    /// Must be called from the DSI ISR or with DSI interrupts masked;
    /// concurrent calls race the `WIFCR` write with the wrapper's
    /// internal flag latches.
    #[inline]
    pub unsafe fn consume_erif(&self) {
        self.dsi_wrapper.regs().wifcr.write(0x02);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_mode_axes_are_distinct() {
        assert!(AdaptedCommand::PULSED_LTDCEN);
        assert!(AdaptedCommand::USES_TE_GPIO);
        assert!(!VideoMode::PULSED_LTDCEN);
        assert!(!VideoMode::USES_TE_GPIO);
    }

    #[test]
    fn scan_mode_names() {
        assert_eq!(AdaptedCommand::NAME, "AdaptedCommand");
        assert_eq!(VideoMode::NAME, "VideoMode");
    }

    #[test]
    fn holdoff_variants_round_trip() {
        let n = Holdoff::None;
        let d = Holdoff::FixedDelay { us: 32_000 };
        // Pattern match exhaustiveness check.
        match n {
            Holdoff::None => {}
            Holdoff::FixedDelay { .. } => unreachable!(),
        }
        match d {
            Holdoff::None => unreachable!(),
            Holdoff::FixedDelay { us } => assert_eq!(us, 32_000),
        }
    }

    #[test]
    fn bare_metal_pacing_holdoff_dispatch() {
        let pacing = BareMetalLoopPacing::new();
        let erif = ErifInfo {
            cyccnt: 0,
            erif_count: 0,
        };
        assert_eq!(pacing.compute_holdoff_us(&erif, Holdoff::None), None);
        assert_eq!(
            pacing.compute_holdoff_us(&erif, Holdoff::FixedDelay { us: 1_234 }),
            Some(1_234)
        );
    }

    #[test]
    fn erif_info_is_cheap_to_copy() {
        // Two u32 fields → 8 bytes, Copy, no Drop.
        assert_eq!(core::mem::size_of::<ErifInfo>(), 8);
    }
}
