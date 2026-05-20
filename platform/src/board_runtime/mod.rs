//! DPR-02 Board Runtime — warm-reset cleanup, boot sentinels, telemetry slots.
//!
//! Per [DPR-02 §1][dpr02] and [DPR-00 §6 INV-DPR-5][dpr00]: warm-reset
//! peripheral cleanup is a Board Runtime service, not an app-copy job.
//! This module hosts the platform-side surface that
//! `BoardRuntime::init` will compose at boot:
//!
//! - `safe_stop` — bounded, `ServiceSet`-driven sequence that stops
//!   autonomous peripherals left running across CPU reset, clears
//!   pending IRQ state, and records per-peripheral timeout telemetry.
//!   Ordering per DPR-02 §5.1. Exposes `SafeStop`, `SafeStopReport`,
//!   `ServiceSet`.
//! - `boot_sentinel` — named `0xA11C_xxxx` constants written at
//!   canonical boot milestones per DPR-02 §5.2. Presence in SRAM4 is
//!   positive proof of progress (INV-DPR-2-3). Exposes `BootSentinel`.
//! - `telemetry` — frozen byte-offset layout inside DPR-00 §5.3's
//!   reserved `0x3800_0500..0x3800_0600` Board Runtime range, per
//!   DPR-02 §5.3. Exposes the `slot` const-offset module and the
//!   `TelemetrySlot` typed-name enum.
//!
//! ## Scaffold status
//!
//! This is **scaffold code** as of DPR-02a. The `SafeStop::run` body
//! is a NOP returning a zero `SafeStopReport`; the signature is
//! frozen here so the
//! call site in `BoardRuntime::init` ratifies separately from the
//! peripheral-disable sequence. The real disable sequence (DMA stream
//! clear, SAI block disable, NVIC ICER/ICPR, etc.) lands in DPR-02a
//! phase-1 step 2+ once the typed `hwcore::regs::*` accessors for SAI
//! block / BDMA channel / NVIC ICER are in place.
//!
//! Consumer wiring (`BoardRuntime::init`, the `examples/stm32h747i-disco/`
//! call sites) is also out of scope here — matches the DPR-01a /
//! DPR-01b precedent of landing the surface first and the call-site
//! migration in a follow-up PR.
//!
//! [dpr00]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-00-CONCEPTS.md
//! [dpr02]: https://github.com/softoboros/rlvgl/blob/main/docs/concepts/DPR-02-CONCEPTS.md

pub mod boot_sentinel;
pub mod safe_stop;
pub mod telemetry;

pub use boot_sentinel::BootSentinel;
pub use safe_stop::{SafeStop, SafeStopReport, ServiceSet};
pub use telemetry::TelemetrySlot;
pub use telemetry::slot as telemetry_slot;
