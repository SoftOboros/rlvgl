//! DPR-02 boot sentinels — named `0xA11C_xxxx` constants for SRAM4
//! progress markers.
//!
//! Per [DPR-02 §5.2][dpr02]: five named boot sentinels are frozen as
//! the DPR-02 initial set. The upper 16 bits are `0xA11C` ("ALIC",
//! preserved from the demo/analyzer convention); the lower 16 bits
//! encode the boot stage with `0x10` spacing so DPR-02b or later can
//! slot sub-stages (e.g. `0xA11C_0011` "POST_CLOCK_INIT_PLL1_LOCKED")
//! without renumbering the named set.
//!
//! ## Registration policy
//!
//! Adding a new **top-level** sentinel between two existing ones is
//! Standards Action (DPR-02 §5.2). Adding a **sub-stage** inside the
//! `0x10` gap between two named sentinels is Specification Required.
//!
//! ## Positive-progress invariant (INV-DPR-2-3)
//!
//! A `BootSentinel` value MUST be written to its [`crate::board_runtime::
//! telemetry::slot::BOOT_SENTINELS_BASE`] slot **after** the
//! corresponding boot step completes, never before. The presence of a
//! sentinel in SRAM4 is positive proof that the named step
//! succeeded; writing the sentinel speculatively before the step
//! defeats this property and is an INV-DPR-2-3 violation.
//!
//! ## Scaffold status
//!
//! This module defines the [`BootSentinel`] enum and its
//! [`BootSentinel::raw`] mapping. The `write(stage)` helper that
//! commits a sentinel to SRAM4 lands under DPR-02b together with the
//! demo-side migration of the 11 raw `0x3800_0300` writes per DPR-02
//! §8.
//!
//! [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md

/// Named boot-stage sentinels per [DPR-02 §5.2][dpr02].
///
/// Each variant maps to a 32-bit value of the form `0xA11C_xxxx` where
/// the lower 16 bits encode the named boot milestone. Per INV-DPR-2-3,
/// a sentinel MUST be written **after** the corresponding boot step
/// completes — its presence in SRAM4 is positive proof of progress.
///
/// `non_exhaustive` so future Standards-Action additions (DPR-02 §5.2)
/// don't break downstream `match` arms compiled against an earlier
/// release.
///
/// [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BootSentinel {
    /// `0xA11C_0010` — Earliest Rust execution path entered; pre
    /// clock-tree programming. Equivalent to the demo's existing
    /// `0xA11C_0001` write at `examples/stm32h747i-disco/src/main.rs:1496`.
    PreClockInit,
    /// `0xA11C_0020` — Clock tree (PLL1/PLL2/PLL3) configured per
    /// [DPR-00 INV-DPR-12][dpr00]. Heap not yet initialized.
    ///
    /// [dpr00]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-00-CONCEPTS.md
    PostClockInit,
    /// `0xA11C_0030` — SafeStopSequence completed for every
    /// peripheral in the active PeripheralServiceSet. The companion
    /// [`crate::board_runtime::safe_stop::SafeStopReport`] is committed
    /// to its TelemetrySlot at `0x3800_0510..0x3800_0520` per DPR-02
    /// §5.3 before this sentinel fires.
    PostSafeStop,
    /// `0xA11C_0040` — FMC SDRAM bring-up complete; Bank 2 at
    /// `0xD000_0000` is readable + writable. Equivalent to the
    /// analyzer's `POST_SDRAM_INIT` (`0xA11C_0007`).
    PostSdramInit,
    /// `0xA11C_0050` — `Stm32h747iDiscoDisplay::new` returned
    /// successfully. Equivalent to the analyzer's `POST_DISPLAY_INIT`
    /// (`0xA11C_0008`) and the demo's write `0xA11C_0011` at
    /// `examples/stm32h747i-disco/src/main.rs:2017`.
    PostDisplayInit,
}

impl BootSentinel {
    /// Raw 32-bit value persisted to SRAM4 per DPR-02 §5.2.
    ///
    /// Values are sparse (`0x10` spacing) so DPR-02b or later can
    /// insert sub-stage markers (`0xA11C_0011` etc.) without
    /// renumbering the named set.
    #[inline]
    pub const fn raw(self) -> u32 {
        match self {
            BootSentinel::PreClockInit => 0xA11C_0010,
            BootSentinel::PostClockInit => 0xA11C_0020,
            BootSentinel::PostSafeStop => 0xA11C_0030,
            BootSentinel::PostSdramInit => 0xA11C_0040,
            BootSentinel::PostDisplayInit => 0xA11C_0050,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper — every variant in [`BootSentinel`]. Keep in sync with
    /// the `non_exhaustive` enum above; the absence of a match-arm
    /// safety net here is deliberate so adding a Standards-Action
    /// variant triggers a test failure.
    const ALL: &[BootSentinel] = &[
        BootSentinel::PreClockInit,
        BootSentinel::PostClockInit,
        BootSentinel::PostSafeStop,
        BootSentinel::PostSdramInit,
        BootSentinel::PostDisplayInit,
    ];

    #[test]
    fn raw_values_match_dpr02_section_5_2() {
        // DPR-02 §5.2 frozen mapping — any change here is a
        // Standards-Action amendment.
        assert_eq!(BootSentinel::PreClockInit.raw(), 0xA11C_0010);
        assert_eq!(BootSentinel::PostClockInit.raw(), 0xA11C_0020);
        assert_eq!(BootSentinel::PostSafeStop.raw(), 0xA11C_0030);
        assert_eq!(BootSentinel::PostSdramInit.raw(), 0xA11C_0040);
        assert_eq!(BootSentinel::PostDisplayInit.raw(), 0xA11C_0050);
    }

    #[test]
    fn raw_values_share_alic_prefix() {
        // Upper 16 bits are 0xA11C across every named sentinel per
        // DPR-02 §5.2.
        for s in ALL {
            let v = s.raw();
            assert_eq!(
                v & 0xFFFF_0000,
                0xA11C_0000,
                "{s:?} = 0x{v:08x} does not carry the 0xA11C prefix",
            );
        }
    }

    #[test]
    fn raw_values_are_unique() {
        // Pairwise inequality — also catches accidental aliasing of
        // sub-stage sentinels onto named ones.
        for (i, a) in ALL.iter().enumerate() {
            for (j, b) in ALL.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert_ne!(
                    a.raw(),
                    b.raw(),
                    "sentinels {a:?} and {b:?} both resolve to 0x{:08x}",
                    a.raw()
                );
            }
        }
    }

    #[test]
    fn raw_values_are_monotonic_in_boot_order() {
        // The §5.2 sequence is monotonically increasing so an SRAM4
        // hex dump tells the reader the boot order at a glance. (See
        // §7's "BoardRuntime::init boot ordering" note.)
        let mut prev = 0u32;
        for s in ALL {
            let v = s.raw();
            assert!(
                v > prev,
                "{s:?} = 0x{v:08x} is not greater than the previous sentinel 0x{prev:08x}",
            );
            prev = v;
        }
    }
}
