//! DPR-02 telemetry slot layout — byte offsets inside the DPR-00 §5.3
//! reserved `0x3800_0500..0x3800_0600` Board Runtime range.
//!
//! Per [DPR-02 §5.3][dpr02]: every Board Runtime telemetry surface
//! claims a fixed byte range inside SRAM4's reserved Board Runtime
//! window. Slots MUST NOT overlap; the layout is the source of truth.
//!
//! ## Frozen layout
//!
//! | Range | Slot | Owner |
//! |---|---|---|
//! | `0x3800_0500..0x3800_0510` | `boot_sentinels` | `boot_sentinel::write(stage)` |
//! | `0x3800_0510..0x3800_0520` | `safe_stop_report` | `SafeStop::run()` |
//! | `0x3800_0520..0x3800_0530` | `service_set_active` | `BoardRuntime::init` |
//! | `0x3800_0530..0x3800_0600` | reserved | (unallocated) |
//!
//! ## Registration policy
//!
//! Adding a new slot inside the reserved range is **Expert Review**
//! and MUST §15-amend the DPR-02 §5.3 table. Moving an existing slot
//! is **Standards Action**.
//!
//! ## Scaffold status
//!
//! This module exports the byte offsets as `const u32` items inside
//! [`slot`], plus the [`TelemetrySlot`] enum that names the three
//! allocated slots for typed lookup. The MMIO-side typed handles that
//! `SafeStop::run` and `boot_sentinel::write` will use to persist into
//! SRAM4 land alongside the consumer wiring in DPR-02a / DPR-02b —
//! per the DPR-00 §6 INV-DPR-3 and the "Register-Mashing Discipline"
//! section of `CLAUDE.md`, those handles live under
//! [`crate::hwcore::regs`] and not in this module.
//!
//! [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md

/// Byte-offset constants for the §5.3 Board Runtime telemetry layout.
///
/// All offsets are absolute SRAM4 addresses (SRAM4 base is `0x3800_0000`
/// on the STM32H747). The reserved Board Runtime window is
/// `0x3800_0500..0x3800_0600` (256 bytes total) per DPR-00 §5.3.
pub mod slot {
    /// Absolute address of the `boot_sentinels` slot. 4 × u32; slot 0
    /// holds the most-recent sentinel value per DPR-02 §5.3. Slots
    /// 1..3 are reserved for a possible "ring buffer of last-N stages"
    /// (DPR-02b decides whether to use them; default = slot 0 only).
    pub const BOOT_SENTINELS_BASE: u32 = 0x3800_0500;
    /// Size of the `boot_sentinels` slot in bytes (4 × u32).
    pub const BOOT_SENTINELS_LEN: u32 = 16;
    /// One-past-the-end of the `boot_sentinels` slot.
    pub const BOOT_SENTINELS_END: u32 = BOOT_SENTINELS_BASE + BOOT_SENTINELS_LEN;

    /// Absolute address of the `safe_stop_report` slot. 4 × u32 per
    /// DPR-02 §5.3: `[entry sentinel, exit | timeout mask, elapsed_us,
    /// reserved]`. The in-RAM Rust payload (the
    /// [`super::super::safe_stop::SafeStopReport`] struct) is the
    /// `timeouts` + `elapsed_us` pair; the entry/exit sentinels are
    /// supplied at persist time.
    pub const SAFE_STOP_REPORT_BASE: u32 = 0x3800_0510;
    /// Size of the `safe_stop_report` slot in bytes (4 × u32).
    pub const SAFE_STOP_REPORT_LEN: u32 = 16;
    /// One-past-the-end of the `safe_stop_report` slot.
    pub const SAFE_STOP_REPORT_END: u32 = SAFE_STOP_REPORT_BASE + SAFE_STOP_REPORT_LEN;

    /// Absolute address of the `service_set_active` slot. 4 × u32 per
    /// DPR-02 §5.3: `[bitmask of activated services, ..reserved for
    /// activation timestamps under DPR-02b]`. Bit positions follow
    /// DPR-01 §5.1 `ServiceSet`.
    pub const SERVICE_SET_ACTIVE_BASE: u32 = 0x3800_0520;
    /// Size of the `service_set_active` slot in bytes (4 × u32).
    pub const SERVICE_SET_ACTIVE_LEN: u32 = 16;
    /// One-past-the-end of the `service_set_active` slot.
    pub const SERVICE_SET_ACTIVE_END: u32 = SERVICE_SET_ACTIVE_BASE + SERVICE_SET_ACTIVE_LEN;

    /// Start of the unallocated "reserved" range inside the Board
    /// Runtime window. Claiming a slot here is Expert Review per
    /// DPR-02 §5.3 and MUST §15-amend the DPR-02 layout table.
    pub const RESERVED_BASE: u32 = 0x3800_0530;
    /// One-past-the-end of the unallocated reserved range. Coincides
    /// with the end of the DPR-00 §5.3 Board Runtime window
    /// (`0x3800_0600`).
    pub const RESERVED_END: u32 = 0x3800_0600;

    /// Start of the Board Runtime window per DPR-00 §5.3. Equal to
    /// [`BOOT_SENTINELS_BASE`].
    pub const BOARD_RUNTIME_WINDOW_BASE: u32 = 0x3800_0500;
    /// One-past-the-end of the Board Runtime window per DPR-00 §5.3.
    /// Coincides with [`RESERVED_END`].
    pub const BOARD_RUNTIME_WINDOW_END: u32 = 0x3800_0600;

    // ── Compile-time invariants ────────────────────────────────────

    // Slots are contiguous and non-overlapping.
    const _CONTIG_SENTINELS_TO_REPORT: () = {
        assert!(BOOT_SENTINELS_END == SAFE_STOP_REPORT_BASE);
    };
    const _CONTIG_REPORT_TO_SERVICES: () = {
        assert!(SAFE_STOP_REPORT_END == SERVICE_SET_ACTIVE_BASE);
    };
    const _CONTIG_SERVICES_TO_RESERVED: () = {
        assert!(SERVICE_SET_ACTIVE_END == RESERVED_BASE);
    };
    // Window covers exactly the four sub-slots.
    const _WINDOW_START: () = {
        assert!(BOARD_RUNTIME_WINDOW_BASE == BOOT_SENTINELS_BASE);
    };
    const _WINDOW_END: () = {
        assert!(BOARD_RUNTIME_WINDOW_END == RESERVED_END);
    };
    // Window is the DPR-00 §5.3 reserved 256-byte Board Runtime range.
    const _WINDOW_SIZE: () = {
        assert!(BOARD_RUNTIME_WINDOW_END - BOARD_RUNTIME_WINDOW_BASE == 0x100);
    };
    // Window MUST end before 0x3800_1000 — sanity check that we are not
    // running off SRAM4's 64 KiB span on the H747.
    const _WINDOW_FITS_SRAM4_LOW: () = {
        assert!(BOARD_RUNTIME_WINDOW_END <= 0x3800_1000);
    };
}

/// Typed name for one of the three allocated DPR-02 §5.3 telemetry
/// slots.
///
/// Provides typed [`base`][TelemetrySlot::base] /
/// [`len`][TelemetrySlot::len] / [`end`][TelemetrySlot::end] accessors
/// for the byte ranges declared in [`slot`]. The MMIO-typed handles
/// that perform the SRAM4 writes land in [`crate::hwcore::regs`] under
/// DPR-02a / DPR-02b per the platform's "Register-Mashing Discipline".
///
/// `non_exhaustive` so future Expert Review additions to DPR-02 §5.3
/// can extend the enum without breaking downstream `match` arms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TelemetrySlot {
    /// 4 × u32 ring of boot sentinels per DPR-02 §5.3.
    BootSentinels,
    /// 4 × u32 [`super::safe_stop::SafeStopReport`] persistence slot
    /// per DPR-02 §5.3.
    SafeStopReport,
    /// 4 × u32 active-ServiceSet bitmask + reserved activation
    /// timestamps per DPR-02 §5.3.
    ServiceSetActive,
}

impl TelemetrySlot {
    /// Slot base address (absolute SRAM4) per DPR-02 §5.3.
    #[inline]
    pub const fn base(self) -> u32 {
        match self {
            TelemetrySlot::BootSentinels => slot::BOOT_SENTINELS_BASE,
            TelemetrySlot::SafeStopReport => slot::SAFE_STOP_REPORT_BASE,
            TelemetrySlot::ServiceSetActive => slot::SERVICE_SET_ACTIVE_BASE,
        }
    }

    /// Slot length in bytes per DPR-02 §5.3.
    #[inline]
    pub const fn len(self) -> u32 {
        match self {
            TelemetrySlot::BootSentinels => slot::BOOT_SENTINELS_LEN,
            TelemetrySlot::SafeStopReport => slot::SAFE_STOP_REPORT_LEN,
            TelemetrySlot::ServiceSetActive => slot::SERVICE_SET_ACTIVE_LEN,
        }
    }

    /// One-past-the-end absolute SRAM4 address.
    #[inline]
    pub const fn end(self) -> u32 {
        self.base() + self.len()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_addresses_match_dpr02_section_5_3() {
        // DPR-02 §5.3 frozen layout — any change here is a
        // Standards-Action amendment for moving existing slots.
        assert_eq!(slot::BOOT_SENTINELS_BASE, 0x3800_0500);
        assert_eq!(slot::SAFE_STOP_REPORT_BASE, 0x3800_0510);
        assert_eq!(slot::SERVICE_SET_ACTIVE_BASE, 0x3800_0520);
        assert_eq!(slot::RESERVED_BASE, 0x3800_0530);
        assert_eq!(slot::RESERVED_END, 0x3800_0600);
    }

    #[test]
    fn slot_sizes_are_four_u32() {
        assert_eq!(slot::BOOT_SENTINELS_LEN, 16);
        assert_eq!(slot::SAFE_STOP_REPORT_LEN, 16);
        assert_eq!(slot::SERVICE_SET_ACTIVE_LEN, 16);
    }

    #[test]
    fn slots_are_contiguous_and_non_overlapping() {
        // Compile-time assertions in `slot` already cover contiguity;
        // duplicate them here so a test-runner failure points the reader
        // at the right invariant.
        assert_eq!(slot::BOOT_SENTINELS_END, slot::SAFE_STOP_REPORT_BASE);
        assert_eq!(slot::SAFE_STOP_REPORT_END, slot::SERVICE_SET_ACTIVE_BASE);
        assert_eq!(slot::SERVICE_SET_ACTIVE_END, slot::RESERVED_BASE);
    }

    #[test]
    fn slots_fit_inside_board_runtime_window() {
        assert!(slot::BOOT_SENTINELS_BASE >= slot::BOARD_RUNTIME_WINDOW_BASE);
        assert!(slot::RESERVED_END <= slot::BOARD_RUNTIME_WINDOW_END);
        assert_eq!(
            slot::BOARD_RUNTIME_WINDOW_END - slot::BOARD_RUNTIME_WINDOW_BASE,
            0x100,
            "DPR-00 §5.3 reserves exactly 256 bytes for Board Runtime telemetry"
        );
    }

    #[test]
    fn window_ends_before_sram4_one_kib() {
        // SRAM4 is 64 KiB on the H747; the Board Runtime window sits
        // well inside the low 4 KiB of SRAM4 with plenty of margin.
        // The `< 0x3800_1000` upper bound is the conservative DPR-00
        // §5.3 sanity check.
        assert!(slot::BOARD_RUNTIME_WINDOW_END < 0x3800_1000);
    }

    #[test]
    fn typed_slot_addresses_agree_with_module_constants() {
        assert_eq!(
            TelemetrySlot::BootSentinels.base(),
            slot::BOOT_SENTINELS_BASE
        );
        assert_eq!(TelemetrySlot::BootSentinels.len(), slot::BOOT_SENTINELS_LEN);
        assert_eq!(TelemetrySlot::BootSentinels.end(), slot::BOOT_SENTINELS_END);

        assert_eq!(
            TelemetrySlot::SafeStopReport.base(),
            slot::SAFE_STOP_REPORT_BASE
        );
        assert_eq!(
            TelemetrySlot::SafeStopReport.len(),
            slot::SAFE_STOP_REPORT_LEN
        );
        assert_eq!(
            TelemetrySlot::SafeStopReport.end(),
            slot::SAFE_STOP_REPORT_END
        );

        assert_eq!(
            TelemetrySlot::ServiceSetActive.base(),
            slot::SERVICE_SET_ACTIVE_BASE
        );
        assert_eq!(
            TelemetrySlot::ServiceSetActive.len(),
            slot::SERVICE_SET_ACTIVE_LEN
        );
        assert_eq!(
            TelemetrySlot::ServiceSetActive.end(),
            slot::SERVICE_SET_ACTIVE_END
        );
    }
}
