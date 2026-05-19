# DPR-01 — Runtime Boundary, Frame Scheduler, Demo Migration

**Status:** Draft 2026-05-19. Not ratified. This document ratifies the
concrete Rust signatures and ownership boundaries deferred from DPR-00
§7 and §11 (PCDN-DPR-001..005), and defines the acceptance gates for
migrating the disco demo onto the new platform surface.

DPR-01 unblocks DPR-02 (warm-reset safe stop) and DPR-03 (analyzer
adoption). It does not itself ratify analyzer adoption — that gate is
in DPR-03 per INV-DPR-15.

## 0. Authority Policy

This doc is the normative source for the **runtime boundary, frame
scheduler, and demo migration** sub-phase of the DPR initiative. It
ratifies the Rust API decisions DPR-00 deliberately deferred.

Vocabulary and invariants from DPR-00 §3 / §6 are binding here without
restatement. Where DPR-01 adds new vocabulary (sub-types of
`RuntimeProfile`, `Pacing`, `FrameScheduler`), the additions are
recorded in §3 below and propagate back into DPR-00 §3 via §15
cross-reference rather than duplication.

The authority split:

| Concern | Owner | DPR-01 relationship |
|---|---|---|
| Frozen scan-mode axes and runtime-profile axes | `DPR-00-CONCEPTS.md` §5 | Binding. DPR-01 picks Rust shapes inside the axes; it MUST NOT change the axes themselves. |
| Frozen invariants INV-DPR-1..15 | `DPR-00-CONCEPTS.md` §6 | Binding. DPR-01 acceptance gates verify each invariant the new code touches. |
| Telemetry range table | `DPR-00-CONCEPTS.md` §5.3 | Binding. DPR-01 reserves but does not yet write the `0x3800_0400..0x3800_0500` Frame Scheduler slot. |
| MMIO writer inventory | `DPR-01-A.md` (sub-letter) | Authoritative for *which* writer sites move under FrameScheduler. DPR-01 §10 cites the inventory; the sub-letter owns the per-site grouping. |
| Cross-repo analyzer adoption | DAA-01-B-2 (planned) | Out of scope for DPR-01. DPR-01 MAY publish the surface the analyzer will consume; ratification of adoption is a DPR-03 gate. |

## 1. Purpose

Define and ship the concrete Rust types that DPR-00 sketched
informally:

- `BoardRuntime` constructor signature with three peripheral-sets
  (display / memory / clock) per INV-DPR-13.
- `FrameScheduler<S: ScanMode>` generic struct that owns
  `DSI_WCR` / `DSI_WIER` / `DSI_WIFCR` / `LTDC_L1CFBAR` / `LTDC_SRCR`
  writes per INV-DPR-3.
- `Pacing` trait with `BareMetalLoopPacing`, `FreeRtosPacing`, and
  (placeholder) `ZephyrPacing` impls per DPR-00 §5.2.
- Migration of the disco demo (`Demo::BareMetal` and `Demo::FreeRtos`
  presets) onto the new surface with no intentional behavior change,
  validated against existing bench captures.

## 2. Problem Statement

DPR-00 ratified vocabulary and invariants but left five questions open
in §11 (PCDN-DPR-001..005). Without resolving them, the platform crate
cannot expose a stable `BoardRuntime` surface. Concrete evidence
gathered after DPR-00 ratification:

- **MMIO writers are distributed across four call sites today**
  (`stm32h747i_disco.rs::swap` line 1626–1627, `present` lines
  1646–1659, `wait_frame_done` line 1684; `freertos_entry.rs` lines
  ~200 and ~400+; `main.rs` lines 3262/3264 init-only). Concrete
  inventory in `DPR-01-A.md` §2. Without a single owner, INV-DPR-3
  cannot be enforced.
- **The bare-metal and FreeRTOS paths diverge on holdoff timing**
  (bare-metal: synchronous SysTick-paced compose+present, no TIM7;
  FreeRTOS: `present_task` priority 3 blocks on `erif_sem`, then TIM7
  one-pulse holdoff at 32 ms, then swap+retrigger). The pacing axis
  needs explicit trait dispatch, not feature-flag forks.
- **Typed register coverage is already 85% complete** (per the
  hwcore::regs survey at `dsi.rs:131..173`, `ltdc.rs:120..134`,
  `tim.rs:50..52`). `LCCR_OFFSET_IS_0X64` (`dsi.rs:156`) is
  compile-time-asserted. The Frame Scheduler can be built without any
  raw `*mut u32` writes; the question is which scheduler shape best
  exploits the type system.
- **`Stm32h747iDiscoDisplay` is gated by `#[cfg(...)]` in
  `platform/src/lib.rs:149`** and is not re-exported. External
  consumers (the analyzer) cannot build the demo profile without a
  Cargo.toml feature flag and a path import that bypasses normal
  visibility.

## 3. Glossary (Additions to DPR-00 §3)

Capitalized use of these terms in DPR docs MUST refer to the
definitions below. DPR-00 §3 entries remain authoritative and are not
restated here.

| Term | Meaning | Owner |
|---|---|---|
| **DisplayPeripheralSet** | Bundle consumed by `BoardRuntime::init` covering the display peripherals only: `LTDC`, `DSIHOST`, `DMA2D`, plus the panel reset and backlight pin handles. MUST NOT include peripherals the application also uses. | DPR-01. |
| **MemoryPeripheralSet** | Bundle consumed by `BoardRuntime::init` covering FMC SDRAM bring-up: `FMC`, and the seven GPIO bank peripherals whose pins are dedicated to SDRAM (`GPIOD`, `GPIOE`, `GPIOF`, `GPIOG`, `GPIOH`, `GPIOI`). | DPR-01. |
| **ClockPeripheralSet** | Bundle consumed by `BoardRuntime::init` covering the clock tree: `PWR`, `RCC`, `SYSCFG`. After `init` returns, the runtime publishes a clock-tree handle the app uses for child peripherals (e.g. `I2C4` for codec, `SAI1` for audio). | DPR-01. |
| **ScanMode (trait)** | Sealed trait inhabited by `AdaptedCommand` and `VideoMode` marker types. Carries const-generic axis values (`PULSED_LTDCEN`, `USES_TE_GPIO`). FrameScheduler is generic over this trait, so the per-frame writer sequence is monomorphized. | DPR-01. |
| **Pacing (trait)** | Sealed trait for OS-axis dispatch. Three impls: `BareMetalLoopPacing`, `FreeRtosPacing`, `ZephyrPacing`. Methods: `wait_erif`, `compute_holdoff_us`, `wait_holdoff`, `signal_buf_ready`, `wait_render_gate`. | DPR-01. |
| **ErifInfo** | Snapshot carried from the DSI ERIF ISR to whichever pacing impl needs it: `{ cyccnt: u32, erif_count: u32 }`. Cheap to copy, lives in `IsrChannel<ErifInfo, 1>`. | DPR-01. |
| **Holdoff** | A scan-mode-independent policy controlling when present writes are committed relative to the most recent ERIF. Values: `None` (no holdoff; bare-metal) or `FixedDelay { us: u32 }` (FreeRTOS, default 32 000 µs). | DPR-01. |
| **HsemSet** | Bitmask of supported HSEM lines for `Cores::Cm7Cm4`. Currently a single reserved name `LineInRx` (line 6, CM4→CM7 audio mailbox wake). Adding lines is Expert Review per DPR-00 §5.2. | DPR-01. |

## 4. Source-of-Truth Map (Additions)

DPR-00 §4 remains authoritative for existing surface. DPR-01 adds:

| Surface | New location | DPR-01 treatment |
|---|---|---|
| Frame Scheduler types and impls | `platform/src/frame_scheduler.rs` (new) | Public module. Re-exported as `rlvgl_platform::frame_scheduler::*`. |
| Pacing trait + impls | `platform/src/pacing/{bare_metal.rs,freertos.rs,zephyr.rs,mod.rs}` (new) | Public module. Feature-gated by `freertos` / `zephyr` cargo features. |
| BoardRuntime constructor | Public re-export through `rlvgl_platform::BoardRuntime` | The disco `BoardRuntime` is a thin wrapper composing existing primitives; the public surface lives in `platform/src/lib.rs`. |
| Re-export of Stm32h747iDiscoDisplay | `platform/src/lib.rs:256` | Already publicly re-exported as `rlvgl_platform::Stm32h747iDiscoDisplay` under the `stm32h747i_disco` feature gate. Earlier draft mis-cited the path as `rlvgl_platform::disco::Stm32h747iDiscoDisplay`; corrected here. External consumers (analyzer) adopt by setting `rlvgl-platform = { ..., features = ["stm32h747i_disco"] }` and building for `thumbv7em-none-eabihf`. |
| MMIO writer migration plan | `docs/concepts/DPR-01-A.md` (sub-letter) | Per-site inventory + migration sequence. DPR-01 §10 references; DPR-01-A owns. |

## 5. Frozen Decisions

### 5.1 Resolution of PCDN-DPR-001 — Demo Preset Shape

**Decision:** Two named presets, `Demo::BareMetal` and `Demo::FreeRtos`.

Conceptual Rust shape (final field names ratify in code review):

```rust
pub enum RuntimeProfile {
    Demo(DemoFlavor),
    Analyzer,
    Custom(RuntimeConfig),
}

pub enum DemoFlavor {
    BareMetal,
    FreeRtos,
}

pub struct RuntimeConfig {
    pub scan_mode: ScanModeTag,        // AdaptedCommand | VideoMode tag
    pub services: ServiceSet,          // bitset over {audio, codec_reset, sd, qspi, mems_mic, scope_probes}
    pub pacing: PacingTag,             // BareMetalLoop | FreeRtos | Zephyr
    pub cores: CoresMode,              // Cm7Only | Cm7Cm4 { hsem_lines: HsemSet }
    pub holdoff: Holdoff,              // None | FixedDelay { us: u32 }
    pub telemetry: TelemetryProfile,
}
```

The runtime expands `Demo::BareMetal` and `Demo::FreeRtos` to fully-
populated `RuntimeConfig` values internally; `Custom` exposes the raw
4-tuple from DPR-00 §5.2 plus the `holdoff` field.

`Demo::BareMetal` expands to:

```text
scan_mode = AdaptedCommand
services  = ServiceSet::from_cargo_features()
pacing    = BareMetalLoop
cores     = Cm7Only
holdoff   = None
telemetry = TelemetryProfile::default()
```

`Demo::FreeRtos` expands to:

```text
scan_mode = AdaptedCommand
services  = ServiceSet::from_cargo_features()
pacing    = FreeRtos
cores     = Cm7Only
holdoff   = FixedDelay { us: 32_000 }
telemetry = TelemetryProfile::default()
```

Rationale: the `pacing` axis is genuinely orthogonal to scan mode and
services, but the *holdoff* value is pacing-coupled (bare-metal does
not own TIM7; FreeRTOS uses it). One named preset per holdoff/pacing
pair keeps the demo surface obvious to readers. `Custom` admits the
full 4-tuple plus holdoff for non-demo apps.

### 5.2 Resolution of PCDN-DPR-002 — Analyzer Preset Registration

**Decision:** Ship `Analyzer` as a registered preset in
`rlvgl-platform`, not as `Custom { ... }` constructed analyzer-side.

`RuntimeProfile::Analyzer` expands to:

```text
scan_mode = VideoMode
services  = { audio, codec_reset, mems_mic, scope_probes }
pacing    = BareMetalLoop
cores     = Cm7Cm4 { hsem_lines: { LineInRx } }
holdoff   = None
telemetry = TelemetryProfile::analyzer()
```

Rationale: INV-DPR-15 ties DPR-03 acceptance to a §15 amendment in
`DAA-01-B-2`. Keeping `Analyzer` as a named preset means the analyzer
subrepo adopts by literally writing `RuntimeProfile::Analyzer` —
single API call, easy to grep, easy to diff. A `Custom` adoption would
require the analyzer to vendor a `RuntimeConfig` literal that drifts
silently from the platform's intent.

If a future second-app needs analyzer-like shape without being the
canonical analyzer, it uses `Custom`. Adding a third registered preset
is a Standards-Action §15 change.

### 5.3 Resolution of PCDN-DPR-003 — Peripheral-Set Shape

**Decision:** Three named struct types passed by value, one per
sub-system, satisfying INV-DPR-13 with no god-struct.

```rust
pub struct DisplayPeripheralSet<RST, BL>
where
    RST: OutputPin,
    BL: SetDutyCycle,
{
    pub ltdc: stm32::LTDC,
    pub dsi:  stm32::DSIHOST,
    pub dma2d: stm32::DMA2D,
    pub panel_reset: RST,
    pub backlight:   BL,
}

pub struct MemoryPeripheralSet {
    pub fmc:   stm32::FMC,
    pub gpio_d: stm32::GPIOD,
    pub gpio_e: stm32::GPIOE,
    pub gpio_f: stm32::GPIOF,
    pub gpio_g: stm32::GPIOG,
    pub gpio_h: stm32::GPIOH,
    pub gpio_i: stm32::GPIOI,
}

pub struct ClockPeripheralSet {
    pub pwr:    stm32::PWR,
    pub rcc:    stm32::RCC,
    pub syscfg: stm32::SYSCFG,
}

impl BoardRuntime {
    pub fn init<RST, BL>(
        display: DisplayPeripheralSet<RST, BL>,
        memory:  MemoryPeripheralSet,
        clock:   ClockPeripheralSet,
        profile: RuntimeProfile,
    ) -> Result<Self, InitError>
    where
        RST: OutputPin,
        BL:  SetDutyCycle;
}
```

GPIO banks not in `MemoryPeripheralSet` (`GPIOA`, `GPIOB`, `GPIOC`,
`GPIOJ`, `GPIOK`) remain app-owned. The app keeps:

- `GPIOJ`: scope probes, backlight PJ6, TE PJ2
- `GPIOK`: joystick PK2..PK6
- `GPIOD`, `GPIOE`, etc. for SDRAM are consumed by `MemoryPeripheralSet`
  — but the app retains `GPIOA`/`B`/`C` for codec I2C4, SAI audio,
  SDMMC, etc.

The `panel_reset` (PG3) and `backlight` (PJ6) handles are passed
*after* the app splits the bank, so the runtime takes only the pins
it needs.

Rationale: a single `Peripherals` god-struct violates INV-DPR-13 by
hiding what the runtime actually consumes vs. what it leaves to the
app. The three-set shape makes ownership a compile-time fact and lets
the runtime test compile against a mock peripheral set without
requiring the full PAC.

### 5.4 Resolution of PCDN-DPR-004 — FrameScheduler Scan-Mode Dispatch

**Decision:** Compile-time generic `FrameScheduler<S: ScanMode>`.
ScanMode is a sealed trait with two marker types.

```rust
mod sealed { pub trait Sealed {} }

pub trait ScanMode: sealed::Sealed {
    /// True if DSI_WCR.LTDCEN is toggled per frame.
    const PULSED_LTDCEN: bool;
    /// True if a panel TE GPIO arms scan.
    const USES_TE_GPIO: bool;
}

pub enum AdaptedCommand {}
impl sealed::Sealed for AdaptedCommand {}
impl ScanMode for AdaptedCommand {
    const PULSED_LTDCEN: bool = true;
    const USES_TE_GPIO:  bool = true;
}

pub enum VideoMode {}
impl sealed::Sealed for VideoMode {}
impl ScanMode for VideoMode {
    const PULSED_LTDCEN: bool = false;
    const USES_TE_GPIO:  bool = false;
}

pub struct FrameScheduler<S: ScanMode, P: Pacing> {
    dsi_wrapper: hwcore::regs::dsi::DsiWrapper,
    ltdc:        hwcore::regs::ltdc::Ltdc,
    pacing:      P,
    erif_channel: &'static IsrChannel<ErifInfo, 1>,
    _mode:       PhantomData<S>,
}
```

Per-frame writer sequence (compile-time specialized via `const if`-
equivalent on the trait constants):

```rust
impl<S: ScanMode, P: Pacing> FrameScheduler<S, P> {
    pub fn present(&mut self, fb: PhysAddr) {
        // Always: clear stale CERIF before retarget.
        // Note: WIFCR bit 1 = CERIF (Clear End-of-Refresh Interrupt Flag).
        self.dsi_wrapper.regs().wifcr.write(0x02);
        // Always: retarget layer 1 + trigger shadow reload.
        self.ltdc.layer1().cfbar.write(fb.raw());
        self.ltdc.regs().srcr.write(1);
        // Only when LTDCEN is pulsed per frame (AdaptedCommand).
        if S::PULSED_LTDCEN {
            self.dsi_wrapper.regs().wcr.write(0x0C);
            self.dsi_wrapper.regs().wifcr.write(0x02);
        }
    }
}
```

Rationale: monomorphization eliminates the runtime branch on the hot
path. `VideoMode` builds skip the `WCR` write entirely; `AdaptedCommand`
builds get the inlined pulse. The cost is two monomorphized copies of
the scheduler, which is acceptable for an embedded crate where each
binary picks exactly one scan mode at compile time.

Trait-object alternatives (`dyn ScanMode`, runtime-dispatched enum
tag) were considered and rejected: they preclude inlining the per-
frame writer and complicate the ISR-side body, which the analyzer
needs as a leaf function.

### 5.5 Resolution of PCDN-DPR-005 — HSEM Line Registration

**Decision:** Open registration via `HsemSet` bitmask with named
constants. Adding lines is Expert Review per DPR-00 §5.2.

```rust
pub struct HsemSet(u32);

impl HsemSet {
    /// HSEM line 6 — CM4→CM7 audio-mailbox wake.
    /// Used by the analyzer's `init_lineiin_receive` equivalent.
    pub const LINE_IN_RX: Self = Self(1 << 6);

    pub const EMPTY:      Self = Self(0);

    pub const fn contains(self, other: Self) -> bool { /* ... */ }
    pub const fn union(self, other: Self) -> Self { /* ... */ }
}

pub enum CoresMode {
    Cm7Only,
    Cm7Cm4 { hsem_lines: HsemSet },
}
```

`Analyzer` expands `cores = Cm7Cm4 { hsem_lines: HsemSet::LINE_IN_RX }`.
`Demo::*` expands `cores = Cm7Only`. Adding `LINE_*` constants is Expert
Review (chapter owner approves; no DPR-00 §5.2 amendment required).
Adding a new `CoresMode` variant remains Standards Action.

Rationale: a fixed enum locks out future HSEM consumers (the disco
analyzer is unlikely to be the only second-core consumer; SDMMC
mailbox, USB power-management mailbox, etc. are plausible future uses).
A bitmask with named consts keeps the API stable as lines accumulate.

### 5.6 Pacing Trait Surface

```rust
pub trait Pacing: sealed::Sealed {
    /// Block until the next ERIF (panel scan complete).
    /// Returns the cycle snapshot captured at the ERIF edge.
    fn wait_erif(&mut self) -> ErifInfo;

    /// Compute holdoff duration in microseconds relative to the
    /// captured ERIF, or `None` if no holdoff is configured.
    fn compute_holdoff_us(&self, erif: &ErifInfo, holdoff: Holdoff) -> Option<u32>;

    /// Block (or busy-spin) until the holdoff timer fires.
    /// `BareMetalLoopPacing` busy-spins on DWT; `FreeRtosPacing`
    /// arms TIM7 and blocks on `present_gate_sem`.
    fn wait_holdoff(&mut self, us: u32);

    /// Signal that the back buffer is safe to render into.
    fn signal_buf_ready(&self);

    /// Block until the render-gate is open (back buffer is ready).
    fn wait_render_gate(&mut self);
}

pub struct BareMetalLoopPacing { /* DWT-based; consumes ErifInfo from IsrChannel */ }
pub struct FreeRtosPacing      { /* sem handles: erif_sem, present_gate_sem, buf_ready_sem, render_start_sem */ }
pub struct ZephyrPacing        { /* k_sem handles — placeholder; populated under DPR-01c if a Zephyr profile is added */ }
```

`Pacing` is sealed; new pacing backends require a DPR-00 §5.2
Standards-Action amendment.

## 6. Invariants Inherited

DPR-00 §6 INV-DPR-1..15 remain binding. DPR-01 adds no new top-level
invariants; it ratifies the Rust surface that satisfies them.

DPR-01 acceptance verifies:

- INV-DPR-3: all four current writer sites consolidate under
  `FrameScheduler`. Direct `*mut u32` casts to `0x5000_0404`,
  `0x5000_0408`, `0x5000_0410`, `0x5000_1024`, `0x5000_10AC` outside
  `frame_scheduler.rs` are added to the discipline scanner BASELINE
  as removals.
- INV-DPR-8: every register write in `frame_scheduler.rs` goes
  through `hwcore::regs::*` typed accessors. The 85%-complete typed
  coverage per `dsi.rs:131..173` and `ltdc.rs:120..134` is sufficient
  — no new typed accessors are required.
- INV-DPR-13: `BoardRuntime::init` signature takes
  `(DisplayPeripheralSet, MemoryPeripheralSet, ClockPeripheralSet,
  RuntimeProfile)` — no `Peripherals` god-struct.
- INV-DPR-14: heap base becomes a `RuntimeConfig::heap_base` /
  `heap_size` field (added under DPR-01 once the demo migration
  surfaces the right defaults).

## 7. Concrete API Sketch — Ratified

```rust
// platform/src/lib.rs (additions)
pub use crate::frame_scheduler::{
    FrameScheduler, ScanMode, AdaptedCommand, VideoMode,
    Pacing, BareMetalLoopPacing, ErifInfo, Holdoff,
};

#[cfg(feature = "freertos")]
pub use crate::pacing::freertos::FreeRtosPacing;

// rlvgl_platform::Stm32h747iDiscoDisplay is already publicly re-exported
// via platform/src/lib.rs:256 under the `stm32h747i_disco` feature.
// No additional re-export needed for DPR-01 adoption.

pub mod board_runtime {
    pub struct BoardRuntime { /* opaque */ }

    impl BoardRuntime {
        pub fn init<RST, BL>(
            display: DisplayPeripheralSet<RST, BL>,
            memory:  MemoryPeripheralSet,
            clock:   ClockPeripheralSet,
            profile: RuntimeProfile,
        ) -> Result<Self, InitError>
        where RST: OutputPin, BL: SetDutyCycle;

        pub fn display(&mut self) -> &mut Stm32h747iDiscoDisplay<...>;
        pub fn frame_scheduler<S, P>(&mut self) -> &mut FrameScheduler<S, P>
        where S: ScanMode, P: Pacing;
        pub fn telemetry(&self) -> &TelemetryHandles;
        pub fn hsem(&self) -> Option<&HsemChannels>;
    }
}
```

`Stm32h747iDiscoDisplay` retains its existing public methods
(`new`, `present`, `swap`, `wait_frame_done`, `publish_back_and_wait`,
backlight, panel reset) but those methods are *re-routed* to call
into the `FrameScheduler` field they now own. The signature on
`Stm32h747iDiscoDisplay` does not change — DPR-01 is a refactor under
INV-DPR-3, not a public-surface change to the display struct.

## 8. Phase Plan

### DPR-01 (this doc) — Concept doc

Acceptance:

- [ ] §3 vocabulary additions accepted.
- [ ] §5.1..§5.6 resolutions accepted (or amended).
- [ ] §6 invariant restatement accepted.
- [ ] §7 API sketch accepted as the target for DPR-01a/b/c code.

### DPR-01a — FrameScheduler scaffold + Demo::BareMetal migration

Land `platform/src/frame_scheduler.rs` with the types from §5.4..§5.6
plus `BareMetalLoopPacing`. Migrate the bare-metal demo path
(`Demo::BareMetal`) onto the new surface. Consolidate
`stm32h747i_disco.rs::{swap,present,wait_frame_done}` writes into
`FrameScheduler<AdaptedCommand, BareMetalLoopPacing>`.

Acceptance:

- All MMIO writes from `stm32h747i_disco.rs::swap/present/wait_frame_done`
  go through typed `hwcore::regs` accessors via FrameScheduler.
- Discipline scanner BASELINE shrinks by the consolidated sites.
- Demo bare-metal build flashes and presents frames on hardware
  identical to pre-DPR-01a behavior (golden-frame validation).
- `Stm32h747iDiscoDisplay` is re-exported publicly.

### DPR-01b — FreeRtosPacing + Demo::FreeRtos migration

Add `platform/src/pacing/freertos.rs` with `FreeRtosPacing`. Migrate
`freertos_entry.rs` task bodies to consume the runtime API. TIM7 init
moves into `FreeRtosPacing::new`; ERIF semaphore handling moves into
`FreeRtosPacing::wait_erif`.

Acceptance:

- `freertos_entry.rs` no longer carries raw `DSI_WCR` / `LTDC_SRCR`
  casts; all routed through FrameScheduler.
- Demo FreeRTOS build flashes and presents at the same 17.9 fps
  baseline (per memory `project_freertos_port_status`).
- TIM7 holdoff still phases present writes ~32 ms after ERIF.

### DPR-01c — ZephyrPacing skeleton (deferred)

Placeholder for a future Zephyr profile. Not on the DPR-01 critical
path; spec'd here so the Pacing trait surface is forward-compatible.
Closes when (a) the Zephyr port is reactivated and (b) a `Zephyr`
preset is registered under DPR-00 §5.2 Standards Action.

### DPR-02 (out of scope here)

Warm-reset safe stop + boot-sentinel migration. Gated on DPR-01a/b
landing.

### DPR-03 (out of scope here)

Cross-repo analyzer adoption. Gated on DPR-01 publishing the surface
and DAA-01-B-2 §15 ratifying consumption.

## 9. Non-Goals

- DPR-01 does not move existing init-only register writes
  (`stm32h747i_disco.rs:787,934..993,1110`; `main.rs:3262,3264`) into
  FrameScheduler. Those remain in `Stm32h747iDiscoDisplay::new` per
  DPR-01-A §4. The Frame Scheduler only owns the **per-frame**
  writer sites.
- DPR-01 does not add new typed register accessors. The existing
  coverage in `dsi.rs` / `ltdc.rs` is sufficient.
- DPR-01 does not migrate the analyzer. That's DPR-03.
- DPR-01 does not modify `dsi_cmd_mode.rs` beyond routing through
  the new `FrameScheduler` for the hot-path writer sites. The
  init-time helpers (`configure_adapted_cmd_mode`, `start_dsi`,
  `configure_te_gpio`) remain as `Stm32h747iDiscoDisplay::new`
  internals.

## 10. Reconciliation Decisions

| Existing concept | DPR-01 decision |
|---|---|
| `Stm32h747iDiscoDisplay::present()` raw casts (lines 1646..1659) | Body migrates to `self.scheduler.present(fb)`. Public signature unchanged. DPR-01a acceptance gate. |
| `Stm32h747iDiscoDisplay::swap()` raw casts (lines 1626..1627) | Body migrates to `self.scheduler.swap(fb)`. Public signature unchanged. DPR-01a. |
| `Stm32h747iDiscoDisplay::wait_frame_done()` (line 1684) | Body migrates to `self.scheduler.consume_erif()`. DPR-01a. |
| `freertos_entry.rs` TIM7 init + holdoff sequence | Moves to `FreeRtosPacing::new` (init) and `FreeRtosPacing::wait_holdoff` (per-frame). Task bodies become thin wrappers around `scheduler.present()` + `pacing.wait_*`. DPR-01b. |
| `dsi_cmd_mode::handle_erif_isr` | Called from `FrameScheduler::dsi_isr_body` for `AdaptedCommand` mode. `VideoMode` has its own ISR body that skips the WCR pulse. DPR-01a. |
| `FreeRtosFrameSync` (per `freertos_sync.rs:110..180`) | Subsumed by `FreeRtosPacing` for sem handle ownership and `FrameScheduler` for DWT/ERIF-snapshot ownership. The trait stays for any non-pacing consumers; the impl is the new `FreeRtosPacing`. DPR-01b. |
| Init-time MMIO writes (`stm32h747i_disco.rs:787,934..993,1110`; `main.rs:3262,3264`) | Stay in `Stm32h747iDiscoDisplay::new`. INV-DPR-3 covers writes *after* init only. The DWT probe at lines 934..993 is flagged for removal in DPR-01a (not needed for production). |
| `Stm32h747iDiscoDisplay` cfg-gate at `platform/src/lib.rs:149` | Widened so the type is `pub use rlvgl_platform::disco::Stm32h747iDiscoDisplay`. Gating moves to internal feature flags on optional methods (e.g. DMA2D). DPR-01a. |
| `MEMORY.md` entry "DSI display WORKING" (adapted cmd mode) | Updated under DPR-01a to point at `FrameScheduler<AdaptedCommand, BareMetalLoopPacing>` as the canonical owner. |

## 11. Open Questions Carried Forward

DPR-01 ratification does not require resolving these; they ratify
during DPR-01a/b PRs:

- **PCDN-DPR-006:** Should `BareMetalLoopPacing::wait_erif` consume
  the `ErifInfo` from `IsrChannel<ErifInfo, 1>`, or from a raw
  atomic flag for lowest-latency? The bare-metal path today uses a
  raw `AtomicBool`; the channel adds one atomic-load on the hot path
  but is more general. Resolve under DPR-01a.
- **PCDN-DPR-007:** Should `FrameScheduler::dsi_isr_body` be a
  static method (panel-singleton style) or take `&mut self`? Static
  is closer to the existing ISR shape but requires `static mut`.
  `&mut self` requires an `IsrChannel<&mut FrameScheduler>` or a
  similar handoff. Resolve under DPR-01a.
- **PCDN-DPR-008:** Should `Holdoff::FixedDelay { us }` be exposed
  on `RuntimeConfig` for `Custom` profiles, or only on named demo
  presets? Resolve under DPR-01b.

## 12. Acceptance Checklist

DPR-01 (this concept doc) is ratified when:

- [ ] §3 vocabulary additions are accepted.
- [ ] §5.1 (Demo split) is accepted.
- [ ] §5.2 (Analyzer as registered preset) is accepted.
- [ ] §5.3 (three peripheral-sets) is accepted.
- [ ] §5.4 (compile-time generic `FrameScheduler<S>`) is accepted.
- [ ] §5.5 (HSEM open registration) is accepted.
- [ ] §5.6 (Pacing trait surface) is accepted.
- [ ] §8 phase plan (DPR-01a, DPR-01b, DPR-01c) is accepted.
- [ ] §11 open questions are explicitly deferred to DPR-01a or
      DPR-01b.

DPR-01a/b code PRs have their own acceptance gates per §8.

## 13. Files Cited

- `docs/concepts/DPR-00-CONCEPTS.md`
- `docs/concepts/DPR-01-A.md` (sub-letter — MMIO migration plan)
- `docs/concepts/DCB-00-CONCEPTS.md` (precedent for sealed-trait
  typestate dispatch)
- `platform/src/stm32h747i_disco.rs`
  (lines 200, 787, 934, 1110, 1612, 1626, 1638, 1646, 1650, 1656,
  1659, 1673, 1684)
- `platform/src/dsi_cmd_mode.rs`
- `platform/src/frame_sync.rs`
- `platform/src/hwcore/regs/dsi.rs` (lines 131..173, 156, 203..209,
  214, 217)
- `platform/src/hwcore/regs/ltdc.rs` (lines 58..68, 120..134, 145)
- `platform/src/hwcore/regs/tim.rs` (lines 50..52, 59..99)
- `platform/src/hwcore/isr.rs`
- `platform/src/hwcore/surface.rs`
- `platform/src/hwcore/addr.rs`
- `platform/src/lib.rs` (line 149, the cfg-gate widening target)
- `examples/stm32h747i-disco/src/main.rs` (lines 3262, 3264, 5232..5357)
- `examples/stm32h747i-disco/src/freertos_entry.rs`
  (lines 177..291, 718..826, 828..1500, 1503..1563, 1576..1750,
  1775, 1838..1877)
- `examples/stm32h747i-disco/src/freertos_sync.rs` (lines 26, 32..40,
  110..180)

## 14. Unblocks

DPR-01 unblocks DPR-01a (FrameScheduler scaffold + bare-metal
migration), DPR-01b (FreeRtosPacing + FreeRTOS migration), and
DPR-01c (Zephyr placeholder). DPR-02 and DPR-03 remain gated on
DPR-01a/b acceptance.

## 15. Change Log

- **2026-05-19** — Initial draft. Resolves PCDN-DPR-001..005 from
  DPR-00 §11; freezes the §3 vocabulary additions, §5 frozen
  decisions, §7 API sketch, and §8 phase plan. Defers PCDN-DPR-006..008
  to DPR-01a/b PRs. Companion sub-letter `DPR-01-A.md` ratifies the
  MMIO writer inventory and migration sequence.
