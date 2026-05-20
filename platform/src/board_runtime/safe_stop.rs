//! DPR-02 SafeStop — warm-reset peripheral cleanup scaffold.
//!
//! Per [DPR-02 §5.1][dpr02]: a [`SafeStop`] run executes the frozen
//! 5-step ordering — NVIC mask → DMA channel disable → peripheral
//! disable → NVIC pending clear → telemetry record — for every
//! peripheral implied by the active [`ServiceSet`]. The full mapping
//! from service to peripheral set, including each service's NVIC IRQ
//! lines and timeout bit position, is in DPR-02 §5.4.
//!
//! Total wall-clock budget across all peripherals in a single run is
//! ~5 ms (DPR-02 INV-DPR-2-2). A peripheral that exceeds its per-step
//! budget (~1 ms each at step 2 / step 3) sets its bit in
//! [`SafeStopReport::timeouts`] and the sequence proceeds — safe-stop
//! MUST NOT block boot on a stuck peripheral.
//!
//! ## Scaffold status
//!
//! [`SafeStop::run`] is a NOP returning `SafeStopReport { timeouts: 0,
//! elapsed_us: 0 }`. The signature is frozen by this module; the real
//! disable sequence lands under DPR-02a phase-1 step 2+. Consumer
//! wiring into `BoardRuntime::init` is also deferred (call-site
//! migration matches the DPR-01a / DPR-01b precedent).
//!
//! ## Alignment with DPR-01 `ServiceSet`
//!
//! The [`ServiceSet`] bitset defined here mirrors the DPR-01 §5.1
//! `ServiceSet` semantically — `{ audio, codec_reset, sd, qspi,
//! mems_mic, scope_probes }`. DPR-01's own `ServiceSet` type lands in
//! a future DPR-01a/b code PR; at that point this scaffold's
//! `ServiceSet` either re-exports the DPR-01 type or is replaced by it,
//! per the eventual `BoardRuntime::init` composition.
//!
//! [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md

// ── ServiceSet ──────────────────────────────────────────────────────────

/// Service bitset consumed by [`SafeStop::run`].
///
/// Mirrors [DPR-01 §5.1][dpr01] `ServiceSet` semantically; the
/// authoritative bit positions ratify when the DPR-01 type lands.
/// Bit positions chosen here are stable for DPR-02a scaffold purposes.
///
/// Adding a new service is **Specification Required** per DPR-01 §5.1
/// (and DPR-02 §5.4 — the SafeStopSequence mapping table MUST be
/// amended in the same PR per [INV-DPR-2-4][inv]).
///
/// [dpr01]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-01-CONCEPTS.md
/// [inv]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceSet(u32);

impl ServiceSet {
    /// Empty service set — no peripherals registered.
    pub const EMPTY: Self = Self(0);
    /// `audio` — SAI1 Block A/B + DMA1 streams 0/1 per DPR-02 §5.4.
    pub const AUDIO: Self = Self(1 << 0);
    /// `codec_reset` — I2C4 transactions to the WM8994 codec per DPR-02 §5.4.
    pub const CODEC_RESET: Self = Self(1 << 1);
    /// `sd` — SDMMC1 + built-in IDMA per DPR-02 §5.4.
    pub const SD: Self = Self(1 << 2);
    /// `qspi` — QUADSPI + MDMA channel per DPR-02 §5.4.
    pub const QSPI: Self = Self(1 << 3);
    /// `mems_mic` — SAI4 Block A + BDMA per DPR-02 §5.4.
    pub const MEMS_MIC: Self = Self(1 << 4);
    /// `scope_probes` — GPIO output pins. Excluded from PeripheralServiceSet
    /// per DPR-02 §3 (GPIO does not require disable sequencing).
    pub const SCOPE_PROBES: Self = Self(1 << 5);

    /// `true` if every bit in `other` is also set in `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Bitwise union of two service sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Raw bitmask. Mirrors the value persisted to the
    /// `service_set_active` telemetry slot (DPR-02 §5.3) for the eventual
    /// `BoardRuntime::init` composition.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

// ── SafeStopReport ──────────────────────────────────────────────────────

/// Structured output of a [`SafeStop::run`] execution.
///
/// Persisted to the `safe_stop_report` telemetry slot at
/// `0x3800_0510..0x3800_0520` per DPR-02 §5.3. The struct stores the
/// in-RAM detail (`timeouts` + `elapsed_us`); the on-SRAM4 layout adds
/// entry / exit sentinels (`0xB007_5A5E` / `0xB007_D000 | timeouts`)
/// and a reserved word per DPR-02 §5.3.
///
/// Layout sized for two `u32` payload words — `mem::size_of` is 8.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SafeStopReport {
    /// Per-peripheral timeout bitmask. Bit positions follow the
    /// DPR-02 §5.4 mapping table (audio = bits 0..3, mems_mic =
    /// bits 4..5, sd = bit 6, qspi = bit 7, codec_reset = bit 8;
    /// 9..15 reserved for future services, 16..31 reserved for
    /// per-peripheral sub-step timeouts).
    pub timeouts: u32,
    /// Total elapsed wall-clock from the DWT cycle counter, in
    /// microseconds, across the entire safe-stop sequence. Hard upper
    /// bound is the DPR-02 §6 INV-DPR-2-2 budget of ~5 000 µs.
    pub elapsed_us: u32,
}

// ── SafeStop ────────────────────────────────────────────────────────────

/// SafeStop — owns the warm-reset cleanup sequence.
///
/// Per [DPR-02 §5.5][dpr02] and the §7 API sketch: `SafeStop::run` is
/// the single entry point, called once during `BoardRuntime::init`
/// before the new boot's init code touches any peripheral in the
/// active [`ServiceSet`]. The current boot's clock tree MUST NOT have
/// been reprogrammed yet (INV-DPR-2-1).
///
/// The type is intentionally a zero-sized marker today. DPR-02a
/// phase-1 step 2+ extends it to own typed `hwcore::regs::*` handles
/// for the peripherals it disables (DMA1, SAI1, BDMA, SAI4, SDMMC1,
/// QUADSPI, I2C4, NVIC) plus a DWT cycle-counter reference for the
/// `elapsed_us` accumulator. The owned-handles refactor preserves the
/// `unsafe fn run` signature.
///
/// [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md
pub struct SafeStop;

impl SafeStop {
    /// Run the safe-stop sequence for every peripheral implied by
    /// `services`, in the [DPR-02 §5.1][dpr02] frozen order. Returns
    /// the per-peripheral [`SafeStopReport::timeouts`] bitmask and the
    /// elapsed wall-clock from the DWT cycle counter.
    ///
    /// # Safety
    ///
    /// The caller MUST assert all of the following:
    ///
    /// 1. **Called exactly once during early boot** — before any
    ///    peripheral in the active [`ServiceSet`] is enabled by the
    ///    new boot's init code. The DPR-02 §5.4 mapping enumerates
    ///    which peripherals correspond to each service.
    /// 2. **Pre-clock-tree-reprogramming** — the current boot's
    ///    `RCC` PLL / divider / kernel-clock-mux configuration MUST
    ///    NOT have been modified yet (DPR-02 §6 INV-DPR-2-1).
    ///    Reprogramming kernel clocks while a peripheral is still
    ///    running an inherited transfer can latch the peripheral into
    ///    a partially-disabled state.
    /// 3. **No concurrent access to the named peripherals** — no
    ///    other code path is currently writing to any peripheral in
    ///    the DPR-02 §5.4 mapping for `services`. The caller's
    ///    typed-register-handle ownership discipline is what makes
    ///    this assertion safe (DPR-02a phase-1 wires this through
    ///    `hwcore::regs::*` accessors).
    ///
    /// ## Scaffold body
    ///
    /// This is **scaffold**: the body is a NOP and returns
    /// `SafeStopReport { timeouts: 0, elapsed_us: 0 }`. The signature
    /// is frozen here so consumer wiring in `BoardRuntime::init`
    /// ratifies separately from the peripheral-disable sequence.
    /// DPR-02a phase-1 step 2+ replaces the body with the real
    /// 5-step ordering per DPR-02 §5.1.
    ///
    /// [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md
    #[inline]
    pub unsafe fn run(services: ServiceSet) -> SafeStopReport {
        // Suppress unused-arg warning in the scaffold. The DPR-02a
        // phase-1 step 2+ body iterates each service bit and executes
        // the §5.1 ordering for its peripherals.
        let _ = services;
        SafeStopReport {
            timeouts: 0,
            elapsed_us: 0,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_set_empty_is_zero() {
        assert_eq!(ServiceSet::EMPTY.raw(), 0);
    }

    #[test]
    fn service_set_named_bits_distinct() {
        // Each named service occupies a unique single bit per DPR-02 §5.4
        // mapping intent. Bit positions are scaffold-local but stable for
        // DPR-02a; the eventual DPR-01 ServiceSet type ratifies the
        // canonical positions.
        let all = [
            ServiceSet::AUDIO,
            ServiceSet::CODEC_RESET,
            ServiceSet::SD,
            ServiceSet::QSPI,
            ServiceSet::MEMS_MIC,
            ServiceSet::SCOPE_PROBES,
        ];
        for (i, a) in all.iter().enumerate() {
            assert!(
                a.raw().is_power_of_two(),
                "service {i} is not a single bit: 0x{:08x}",
                a.raw()
            );
            for (j, b) in all.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert_eq!(
                    a.raw() & b.raw(),
                    0,
                    "services {i} and {j} share bits: 0x{:08x} & 0x{:08x}",
                    a.raw(),
                    b.raw()
                );
            }
        }
    }

    #[test]
    fn service_set_union_and_contains() {
        let audio_sd = ServiceSet::AUDIO.union(ServiceSet::SD);
        assert!(audio_sd.contains(ServiceSet::AUDIO));
        assert!(audio_sd.contains(ServiceSet::SD));
        assert!(!audio_sd.contains(ServiceSet::QSPI));
        assert!(audio_sd.contains(ServiceSet::EMPTY));
        // Self-union is identity.
        assert_eq!(
            audio_sd.union(audio_sd).raw(),
            audio_sd.raw(),
            "union is idempotent"
        );
    }

    #[test]
    fn service_set_round_trip_raw() {
        // Compose a few services, take .raw(), and confirm individual
        // bit checks recover the original membership.
        let s = ServiceSet::AUDIO
            .union(ServiceSet::CODEC_RESET)
            .union(ServiceSet::MEMS_MIC);
        let raw = s.raw();
        assert_ne!(raw & ServiceSet::AUDIO.raw(), 0);
        assert_ne!(raw & ServiceSet::CODEC_RESET.raw(), 0);
        assert_ne!(raw & ServiceSet::MEMS_MIC.raw(), 0);
        assert_eq!(raw & ServiceSet::SD.raw(), 0);
        assert_eq!(raw & ServiceSet::QSPI.raw(), 0);
        assert_eq!(raw & ServiceSet::SCOPE_PROBES.raw(), 0);
    }

    #[test]
    fn safe_stop_report_size_is_two_u32() {
        // DPR-02 §5.3 reserves a 16-byte on-SRAM4 slot; the in-RAM
        // payload is the two `u32` fields. Future fields require a
        // §5.3 amendment.
        assert_eq!(core::mem::size_of::<SafeStopReport>(), 8);
    }

    #[test]
    fn safe_stop_run_scaffold_returns_zero_report() {
        // Scaffold body invariant: zero timeouts + zero elapsed_us
        // regardless of the service set. The DPR-02a phase-1 step 2+
        // body replaces this; this test will be replaced (not deleted)
        // when the real body lands.
        let report = unsafe { SafeStop::run(ServiceSet::EMPTY) };
        assert_eq!(report.timeouts, 0);
        assert_eq!(report.elapsed_us, 0);

        let report = unsafe {
            SafeStop::run(
                ServiceSet::AUDIO
                    .union(ServiceSet::CODEC_RESET)
                    .union(ServiceSet::SD),
            )
        };
        assert_eq!(report.timeouts, 0);
        assert_eq!(report.elapsed_us, 0);
    }
}
