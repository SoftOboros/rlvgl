# DPR-00 — Disco Platform Runtime Concepts

**Status:** Draft 2026-05-19 (revision b). Not ratified. This document
sets up the spec-before-code lineage for extracting reusable
STM32H747I-DISCO platform runtime support from the current demo/analyzer
copy boundary.

Revision b (2026-05-19) reshapes §5 / §6 / §8 / §10 / §11 against
concrete evidence from `streamz/submodules/disco-analyzer/analyzer-cm7/`
and the existing `platform/` surface. See §15 for the diff against
revision a (initial Codex import).

## 0. Authority Policy

This doc is the normative source for the **Disco Platform Runtime**
initiative inside `rlvgl-platform`. It governs the reusable board-level
runtime APIs that will let multiple applications run on STM32H747I-DISCO
without copying demo-specific bring-up code.

The authority split:

| Concern | Owner | DPR relationship |
|---|---|---|
| STM32H747 register semantics, clock tree, reset behavior, DSI/LTDC/FMC/SAI/DMA/HSEM registers | ST RM0399, ARM Cortex-M7/M4 architecture docs | Cited; DPR does not redocument the MCU manuals. |
| Existing working board bring-up and visual demo behavior | `examples/stm32h747i-disco/` and `examples/apps/disco-demo/` | Evidence and first migration target. Demo behavior remains the initial hardware validation surface. |
| Analyzer-discovered second-app requirements | `streamz/submodules/disco-analyzer/` in the parent workspace | Evidence only. DPR MAY cite analyzer findings and API needs, but RLVGL owns the platform-side contract. |
| Existing display/DSI/framebuffer primitives | `platform/src/stm32h747i_disco.rs`, `platform/src/dsi_cmd_mode.rs`, `platform/src/display_init.rs`, `platform/src/frame_sync.rs`, `platform/src/hwcore/*` | DPR composes and extends these rather than replacing them wholesale. |
| DMA/cache/address ownership discipline | `docs/concepts/DCB-00-CONCEPTS.md` and the Register-Mashing Discipline in `CLAUDE.md` | DPR MUST preserve these invariants; no new raw register or cache bypass is admitted without a DPR §15 amendment. |
| Cross-repo analyzer adoption | `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md` and its planned successor `DAA-01-B-2` | DPR-03 acceptance is *gated* by a §15 amendment ratifying RLVGL adoption in that companion doc; rlvgl-side PRs MAY land first but the gate is not closed until DAA-01-B-2 §15 lands. |

If a DPR phase changes a frozen invariant, exported type name, ownership
boundary, or ISR/register-writer rule, this doc's §15 MUST be amended
first in a separate change.

## 1. Purpose

Make STM32H747I-DISCO board bring-up, display scheduling, warm-reset
cleanup, dual-core handoff (HSEM), and runtime ownership a reusable
RLVGL platform contract rather than an application-local copy of the
disco demo.

The immediate goal is not to build a generic STM32 BSP generator. The
goal is narrower: the existing disco demo remains the first validation
app, and the disco analyzer becomes the second app that proves the
platform surface is not accidentally demo-shaped.

## 2. Problem Statement

The disco analyzer surfaced a platform/API boundary problem. To build
the second H747I-DISCO application, it had to mirror or adapt pieces
from the demo because the reusable `rlvgl-platform` surface did not
publish the full board runtime contract.

Concrete evidence (paths relative to repo root unless noted):

- `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md`
  §6 (2026-04-27) ratified Option C: mirror linker / BSP / FreeRTOS-glue /
  IPC offsets from `rlvgl@d99f793:examples/stm32h747i-disco/` because the
  BSP-generator path was out of scope and example alignment was deemed
  sufficient pending a separate effort.
- `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs` (871 lines)
  carries an `// Adapted from rlvgl@d99f793:...` header and re-implements
  `init_clocks`, `peripheral_safe_stop`, `init_fmc_sdram`, plus the
  `HalGpioBacklight` / `HalResetPin` / `I2c4Adapter` / boot-sentinel
  helpers.
- `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs`
  (entry-point destructuring at `fn main` line ~1431, ~200 lines of
  GPIO splits, HSEM[6] init for CM4→CM7 wake, peripheral safe-stop call,
  SAI1 kernel-clock selection, FMC SDRAM init, boot sentinels at
  `0xA11C_xxxx`, then `Stm32h747iDiscoDisplay::new`, then codec/audio
  init, then widget tree).
- `platform/src/stm32h747i_disco.rs` already exposes
  `Stm32h747iDiscoDisplay::new`, `present()`, `swap()`, and
  `publish_back_and_wait()` — but the struct is gated by
  `#[cfg(...stm32h747i_disco...)]` in `platform/src/lib.rs:149` and
  hence not re-exported as public API.
- `examples/stm32h747i-disco/src/freertos_entry.rs` contains a proven
  ERIF/TIM7 holdoff task model and direct `DSI_WCR` / `LTDC_SRCR`
  writes via raw casts (lines ~200, ~400+).
- Display MMIO writes are scattered across **three call sites today**:
  `stm32h747i_disco.rs::present()` (raw casts to 0x5000_10AC, 0x5000_1024,
  0x5000_0404), `dsi_cmd_mode.rs::present()` (typed via `hwcore::regs`),
  and `freertos_entry.rs` task body (raw casts again). The analyzer
  mirrors a fourth instance.

The failure mode is not that any one app is wrong. The failure mode is
that the second app had to learn the same H747I-DISCO board mythology
by copying demo code, *and* the demo itself has not yet consolidated
display-MMIO ownership behind a single writer. DPR exists so the third
app does not, and so that consolidation lands before the next port.

## 3. Glossary

Reserved DPR vocabulary. Capitalized use of these terms in DPR docs
MUST refer to the definitions below.

| Term | Meaning | Owner |
|---|---|---|
| **Disco Platform Runtime** | The reusable RLVGL-owned runtime surface for STM32H747I-DISCO board bring-up, display scheduling, warm-reset cleanup, HSEM/IRQ ownership, and board telemetry. | DPR. |
| **First App** | The existing RLVGL STM32H747I-DISCO demo. It remains the first hardware validation target for every DPR phase. | DPR. |
| **Second App** | The disco analyzer (`streamz/submodules/disco-analyzer/`). It proves the runtime surface can serve a non-demo, dual-core (CM7+CM4), audio-DSP application. | DPR. |
| **Board Runtime** | The initialized collection of board services returned by a platform entry point: display, frame scheduler, optional audio/codec services, input handles, HSEM channels, telemetry, and any ownership tokens needed to install ISRs safely. | DPR. |
| **Runtime Profile** | A 4-tuple `(scan_mode, services, pacing, cores)` (see §5.2) selecting Board Runtime construction policy. Profiles choose policy, not hardware facts. | DPR. |
| **Warm-Reset Safe Stop** | A bounded sequence that stops autonomous peripherals left running across CPU reset, clears pending IRQs, and records telemetry before normal init programs those peripherals. Modeled on the analyzer's `peripheral_safe_stop()`. | DPR. |
| **Frame Scheduler** | The platform object that owns DSI ERIF handling, LTDC layer handoff (`L1CFBAR`), shadow-reload triggering (`SRCR.IMR`), optional LTDCEN gating, optional holdoff timing, and the single-writer rule for display MMIO. | DPR. |
| **Scan Mode** | The DSI/LTDC scheduling policy selected by a profile. Modeled as the 4-axis decomposition in §5.1, with two named presets `AdaptedCommand` and `VideoMode`. | DPR. |
| **Display MMIO Owner** | The sole code path allowed to write `DSI_WCR`, `DSI_WIER`, `LTDC_LxCFBAR`, and `LTDC_SRCR` after display init. In DPR, this is the Frame Scheduler. | DPR. |
| **Platform Telemetry** | SRAM4 or serial breadcrumbs emitted by Board Runtime primitives for bench validation. Telemetry addresses are part of §5.3's frozen range table; profiles MUST NOT collide. | DPR. |
| **Cores Mode** | Whether a profile runs CM7-only or CM7+CM4. CM7+CM4 mode requires HSEM[6] receive init on CM7 before CM4 unhalt; see INV-DPR-11. | DPR. |

## 4. Source-of-Truth Map

| Surface | Current location | DPR treatment |
|---|---|---|
| H747I-DISCO display constructor and framebuffer ownership | `platform/src/stm32h747i_disco.rs` | Keep as the low-level display owner; layer a Board Runtime and Frame Scheduler above it. Re-export publicly. |
| Adapted-command DSI helpers | `platform/src/dsi_cmd_mode.rs` | Reuse for `AdaptedCommand` scan mode; no app-local DSI pulse helpers once DPR ships. |
| Full raw DSI/LTDC init for hosted paths | `platform/src/display_init.rs` | Keep as a lower-level hosted/Zephyr escape hatch; profiles decide whether it participates. |
| Frame sync traits | `platform/src/frame_sync.rs` | Wrap into a concrete `FrameScheduler` so apps do not reimplement ISR glue. `FrameSync` / `Dma2dSync` / `ScopeProbe` traits stay as the polymorphism layer. |
| FreeRTOS present/render/touch task model | `examples/stm32h747i-disco/src/freertos_entry.rs` | First extraction candidate; move policy into platform without making FreeRTOS mandatory. The `pacing` profile axis (§5.2) selects between bare-metal loop / FreeRTOS / Zephyr backends. |
| Clock tree + PLL3 audio configuration | `examples/stm32h747i-disco/src/main.rs` (and copy in analyzer `bsp.rs`) | Extract into Board Runtime. PLL3_P=12.288 MHz / PLL3_R=32.768 MHz become Board Runtime invariants — see INV-DPR-12. |
| FMC SDRAM bring-up | `examples/stm32h747i-disco/src/main.rs` (and copy in analyzer `bsp.rs`) | Extract into Board Runtime. SDRAM geometry (Bank 2, 16 MiB, IS42S32400F-6BL) is a board fact, not profile policy. |
| Peripheral safe stop | `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs::peripheral_safe_stop()` | Evidence for DPR-02. Equivalent moves into platform under `Warm-Reset Safe Stop`. |
| HSEM init for CM7↔CM4 wake | `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs` (HSEM[6] init) and `analyzer-cm7/src/hsem.rs` | Evidence. Platform-side `init_lineiin_receive`-equivalent becomes part of `Cores::Cm7Cm4` profile boot. See INV-DPR-11. |
| Boot sentinels | analyzer `bsp.rs` (`0xA11C_xxxx` series) | DPR-02 candidate. SRAM4 sentinel range gets frozen in §5.3. |
| Analyzer safe-stop and video-mode ISR lessons | Parent workspace `analyzer-cm7/src/main.rs` ERIF ISR | Evidence for second-app requirements. Extract equivalent RLVGL-owned primitives; do not make analyzer code authoritative. |
| Demo widget tree and capabilities | `examples/apps/disco-demo/src/lib.rs` | Remains app layer. DPR MUST NOT move demo UI into platform. |

## 5. Frozen Decisions

The following decision **structures** are frozen for DPR-00. Concrete
Rust type names and field names ratify in DPR-01; this section freezes
*axes* and *registration policies*, not signatures.

### 5.1 ScanMode

Registration policy: **Standards Action**.

Scan mode is a 4-axis decomposition; named presets are syntactic sugar
over a fixed axis set. The frozen axes are:

| Axis | Values | Description |
|---|---|---|
| **Wake source** | `Erif`, `Te`, `VSync` | What edge triggers a present opportunity. Demo+analyzer both use `Erif` today. |
| **LTDCEN gating** | `Continuous`, `PulsedPerFrame` | Whether LTDC scanout is continuous (`Continuous`, video-mode style) or whether `DSI_WCR.LTDCEN` is toggled each frame (`PulsedPerFrame`, adapted-command style). |
| **TE arm** | `NotUsed`, `ExternalGpio { pin: TePin }` | Whether the panel's TE signal arms scanout. Adapted-command demo uses `ExternalGpio { pin: PJ2 }`; analyzer video-mode uses `NotUsed`. |
| **Holdoff phase** | `None`, `FixedDelay { us: u32 }` | Whether a one-pulse timer phases present writes a fixed number of microseconds after the wake edge. FreeRTOS demo uses `FixedDelay`; bare-metal demo and analyzer use `None`. |

Two named presets, registered as Standards Action entries:

- **`AdaptedCommand`** — `(Erif, PulsedPerFrame, ExternalGpio { pin: PJ2 }, holdoff)` where `holdoff` is profile-supplied (FreeRTOS demo: `FixedDelay { us: 32_000 }`; bare-metal demo: `None`).
- **`VideoMode`** — `(Erif, Continuous, NotUsed, None)`. Matches the
  analyzer's current shape per analyzer-cm7 `main.rs:~2060` ("LTDC scans
  continuously … shadow-reload via SRCR.IMR picks up the new layer
  address on the next frame boundary without disturbing the running
  scan").

Both presets retain the **same retarget mechanism**: write `L1CFBAR`
inside `cortex_m::interrupt::free`, then write `SRCR.IMR=1`, then (only
in `PulsedPerFrame`) write `DSI_WCR=0x0C` to re-arm LTDCEN+DSIEN. The
shared mechanism is captured in INV-DPR-3.

Adding a third preset or a fifth axis is a Standards Action change in
§15 with first-app and second-app impact analysis.

### 5.2 RuntimeProfile

Registration policy: **Standards Action** for preset names; **Expert
Review** for new field values inside an existing preset.

A `RuntimeProfile` is a 4-tuple over **four orthogonal axes**:

| Axis | Type (conceptual) | Frozen values |
|---|---|---|
| **scan_mode** | `ScanMode` (§5.1) | `AdaptedCommand` \| `VideoMode` \| Standards-Action extension. |
| **services** | `ServiceSet` bitset | `{ audio, codec_reset, sd, qspi, mems_mic, scope_probes }`. Adding a service is **Specification Required**. |
| **pacing** | enum | `BareMetalLoop` \| `FreeRtos` \| `Zephyr`. Adding a pacing backend is Standards Action. |
| **cores** | enum | `Cm7Only` \| `Cm7Cm4 { hsem_lines: HsemSet }`. Adding a cores mode is Standards Action. |

Initial named presets, registered as Standards Action:

- **`Demo::bare_metal`** — `(AdaptedCommand { holdoff: None }, { audio?, sd?, qspi? per feature flags }, BareMetalLoop, Cm7Only)`.
- **`Demo::freertos`** — `(AdaptedCommand { holdoff: FixedDelay { us: 32_000 } }, { audio?, sd?, qspi? per feature flags }, FreeRtos, Cm7Only)`.
- **`Analyzer`** — `(VideoMode, { audio, codec_reset, mems_mic, scope_probes }, BareMetalLoop, Cm7Cm4 { hsem_lines: { line_6_in } })`.
- **`Custom { ... }`** — user-supplied 4-tuple; not a registered preset.

The OS axis (`pacing`) is orthogonal to scan mode and to cores mode.
DPR-01 ratifies the concrete Rust shape of these tuples; the axes
themselves are frozen here.

### 5.3 TelemetryProfile

Registration policy: **Expert Review** for additions; **Standards
Action** for moving an existing range.

The following SRAM4 byte ranges are frozen as the current allocation.
Future additions MUST publish their range here before writing.

| Range | Owner | Purpose |
|---|---|---|
| `0x3800_0000..0x3800_0100` | playit | Command-protocol counters (queue depth, drops, last-cmd tick). |
| `0x3800_0300..0x3800_0310` | analyzer `peripheral_safe_stop` | Safe-stop entry/exit sentinels (`0xB007_5A5E`, `0xB007_D000 \| timeout_mask`). |
| `0x3800_0400..0x3800_0500` | reserved for DPR Frame Scheduler | Future: ERIF count, present count, dropped-frame count, last-present cycle delta. |
| `0x3800_0500..0x3800_0600` | reserved for DPR Board Runtime | Future: boot sentinels (replaces analyzer's `0xA11C_xxxx` literals once §10 reconciliation lands). |
| `0x3800_0600..0x3800_1000` | reserved for future profiles | Unallocated; profiles MUST §15-amend to claim. |
| Outside SRAM4 — D3 SRAM (`0x3800_4000..`) | cpu_stats DWT samples | Telemetry, not SRAM4. Listed here for completeness; profiles do not collide. |

The analyzer's `0xA11C_xxxx` boot-sentinel literals are **out-of-range**
relative to SRAM4 (`0x3800_xxxx`); they are absolute address sentinels
in the analyzer's own framing. DPR-02 migrates these into the
`0x3800_0500..0x3800_0600` Board Runtime range.

## 6. Runtime Invariants

- **INV-DPR-1: Demo-first validation.** Every DPR execution phase MUST
  migrate and validate the RLVGL disco demo (`Demo::bare_metal` and
  `Demo::freertos`) before claiming the platform surface is ready for
  analyzer adoption.
- **INV-DPR-2: Analyzer as second-app proof.** A DPR API is not
  considered reusable merely because the demo builds. Each stable
  surface MUST name how the `Analyzer` profile will consume it, even if
  the analyzer-side patch lands later under DAA-01-B-2.
- **INV-DPR-3: Apps do not own display MMIO.** After Board Runtime
  initialization, application code MUST NOT directly write `DSI_WCR`,
  `DSI_WIER`, `LTDC_LxCFBAR`, or `LTDC_SRCR`. Those writes belong to
  the Frame Scheduler. This invariant subsumes the three current writer
  sites in `stm32h747i_disco.rs::present()`, `dsi_cmd_mode.rs::present()`,
  and `freertos_entry.rs` — consolidation is the DPR-01 acceptance gate.
- **INV-DPR-4: ISR ownership is explicit.** A Board Runtime profile
  MUST declare which interrupts it installs or expects the app to
  forward. Silent shared ownership of DSI, DMA2D, TIM7, HSEM, SAI1, or
  SysTick is forbidden.
- **INV-DPR-5: Warm-reset cleanup is platform-owned.** If a profile
  enables SAI/DMA/codec services, it MUST either run a platform
  Warm-Reset Safe Stop or explicitly declare why the service is cold-
  boot-only. `Demo::*` profiles with `audio` in `services` MUST opt in
  to safe-stop in DPR-02 — this is the demo's safe-stop validation.
- **INV-DPR-6: Telemetry does not collide.** Platform telemetry MUST
  reserve a range in §5.3 before writing SRAM4 breadcrumbs. App-owned
  telemetry ranges are not available to platform code.
- **INV-DPR-7: No demo UI leakage.** `DiscoController` and demo widgets
  remain application-layer constructs. Board Runtime MAY provide a
  display, input, and scheduler; it MUST NOT depend on the demo widget
  tree.
- **INV-DPR-8: Register-mashing discipline remains in force.** New
  platform code MUST use existing typed register, address, framebuffer,
  ISR, and DCB primitives where they exist (`platform/src/hwcore/regs/`,
  `addr`, `surface`, `isr`, `dca`). A raw MMIO escape hatch requires an
  explicit discipline marker and a DPR rationale.
- **INV-DPR-9: Profiles choose policy, not silicon facts.** SDRAM
  geometry (Bank 2, 16 MiB, IS42S32400F-6BL), panel wiring (NT35510,
  PG3 reset, PJ6 backlight), peripheral base addresses, and AHB/APB
  bus topology are board facts. Profiles MAY choose scan mode, service
  enablement, pacing, and cores mode, but MUST NOT redefine board facts.
- **INV-DPR-10: No application-local mythology.** Any H747I-DISCO
  sequencing rule that is required by both the demo and analyzer MUST
  move into DPR docs or a lower-level platform doc before being copied
  into a second application.
- **INV-DPR-11: HSEM-before-CM4-unhalt.** For any profile with
  `Cores::Cm7Cm4`, the Board Runtime MUST call the platform equivalent
  of analyzer-cm7's `hsem::init_lineiin_receive()` (enable HSEM AHB4
  clock, KEYR unlock, ICR clear, IER1 enable, NVIC unmask) **before**
  CM4 is unhalted (i.e. before `RCC_GCR.BOOT_C2` is set or before
  releasing the CPU2 hold). Failure to do so deadlocks the wake
  protocol. The CM7-side init in `Cores::Cm7Only` MUST NOT touch
  HSEM[6] receive state.
- **INV-DPR-12: Clock-tree audio invariants.** Profiles with `audio` or
  `mems_mic` in `services` MUST run on a clock tree that provides
  PLL3_P = 12.288 MHz (SAI1 kernel, 48 kHz exact) and PLL3_R =
  32.768 MHz (LTDC pixel). These values are inherited from the demo
  today and copied by the analyzer; DPR-01 makes them Board Runtime
  guarantees rather than open-coded constants.
- **INV-DPR-13: Profiles publish consumed-peripheral set.** A
  RuntimeProfile MUST document exactly which PAC peripherals
  (`LTDC`, `DSIHOST`, `DMA2D`, `FMC`, `I2C4`, `SAI1`, `SDMMC1`,
  `QUADSPI`, GPIO ports, …) it consumes by ownership, and which it
  leaves to the application. The Board Runtime constructor signature
  MUST reflect this consumed set; passing in a struct-of-all-peripherals
  ("god peripherals") is a discipline violation.
- **INV-DPR-14: Heap base is a profile output, not an app constant.**
  Today the demo places its heap in DTCM and the analyzer relocated to
  D1 AXI SRAM at `0x2402_0000` to avoid stack overflow on dual-core
  builds. The Board Runtime MUST declare `(heap_base, heap_size)` per
  profile; apps consume the published values rather than open-coding
  them. Boot ordering is: clock tree → heap init → SDRAM init →
  display init.
- **INV-DPR-15: Cross-repo gating is named.** DPR-03 acceptance
  requires a `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-2`
  §15 amendment ratifying RLVGL adoption. RLVGL-side PRs MAY ship the
  surface in advance, but DPR-03's §15 entry MUST cite the DAA-01-B-2
  commit hash before claiming the gate is closed.

## 7. API Sketch — Informative, Deferred to DPR-01

This section is informative. Exact Rust signatures ratify in DPR-01.
The intent here is to surface the **ownership boundaries** implied by
§5 and §6, not to commit to names. INV-DPR-13 requires that the final
signatures take only the peripherals they actually own — the sketch
below is illustrative.

```text
// Pseudo-Rust. Ratifies in DPR-01.

struct BoardRuntime {
    display: Stm32h747iDiscoDisplay<...>,
    scheduler: FrameScheduler,
    services: ActiveServices,        // {audio, codec_reset, sd, qspi, mems_mic, scope_probes}
    telemetry: TelemetryHandles,     // typed slot handles into §5.3 ranges
    hsem: Option<HsemChannels>,      // Some when cores = Cm7Cm4
}

impl BoardRuntime {
    fn init(
        display_peripherals: DisplayPeripheralSet,   // LTDC, DSIHOST, DMA2D, panel reset pin, backlight pin
        memory_peripherals: MemoryPeripheralSet,     // FMC + the GPIO banks required for SDRAM
        clock_peripherals: ClockPeripheralSet,       // PWR, RCC, SYSCFG
        profile: RuntimeProfile,
    ) -> Result<Self, InitError>;

    fn display(&mut self) -> &mut Display;
    fn frame_scheduler(&mut self) -> &mut FrameScheduler;
    fn telemetry(&self) -> &TelemetryHandles;
}

struct FrameScheduler { /* opaque; owns DSI_WCR / DSI_WIER / L1CFBAR / SRCR per INV-DPR-3 */ }

impl FrameScheduler {
    fn mark_dirty(&self);
    fn publish_back_and_wait(&self, display: &mut impl BackBufferProvider);
    /// SAFETY: only the registered ISR for the chip's DSI/ERIF line.
    unsafe fn dsi_isr_body(&self);
}
```

Note that GPIO banks not required for SDRAM (joystick PK2..PK6, codec
I2C4 PD12/PD13, audio SAI pins, etc.) are **not** in the runtime's
consumed set — they remain app- or service-owned to satisfy INV-DPR-13.
DPR-01 ratifies the exact split.

## 8. Phase Plan

### DPR-01 — Runtime Boundary, Display MMIO Consolidation, Demo Migration

Define concrete Rust signatures for `BoardRuntime`, `RuntimeProfile`,
`FrameScheduler`, and `ScanMode`. Consolidate `DSI_WCR` / `DSI_WIER` /
`L1CFBAR` / `SRCR` writes to a single `FrameScheduler` owner. Migrate
the RLVGL disco demo (`Demo::bare_metal` and `Demo::freertos`) onto
the new API with no intentional behavior change. Validate against
existing golden frames / bench captures.

Acceptance:

- All four current display-MMIO writer sites
  (`stm32h747i_disco.rs::present()`, `dsi_cmd_mode.rs::present()`,
  `freertos_entry.rs` task body, plus any incidental scatter) now
  route through `FrameScheduler`.
- `Demo::bare_metal` and `Demo::freertos` builds boot and present
  frames on hardware with no visible regression vs the pre-DPR
  baseline.
- `Stm32h747iDiscoDisplay` becomes publicly re-exported (or wrapped
  by a public `BoardRuntime` shell) so an external crate can build the
  demo profile.
- Public APIs are documented and pass `#![deny(missing_docs)]` on the
  newly-exported surface.
- INV-DPR-13 (consumed-peripheral set) is satisfied — the constructor
  takes display/memory/clock peripherals only, not "all of `Peripherals`."

### DPR-02 — Warm-Reset Safe Stop, Boot Sentinels, Telemetry Profiles

Move analyzer-proven safe-stop concepts into RLVGL platform as a
profile-controlled service. Migrate analyzer-style boot sentinels
into the §5.3 `0x3800_0500..0x3800_0600` range. Include SAI/DMA stop,
IRQ pending clear, codec reset sequencing hooks, and a typed handle
for each published telemetry slot.

Acceptance:

- `Demo::*` profiles with `audio` in `services` opt into safe-stop and
  validate on hardware (cold boot, warm reset, repeated power-cycle).
- Analyzer can plan to delete its local `peripheral_safe_stop` once it
  adopts the rlvgl submodule update (cross-repo gate, formally closed
  by DPR-03).
- Boot-sentinel literals in `0xA11C_xxxx` are reconciled to
  `0x3800_0500..0x3800_0600` per §10.
- Telemetry range collision checks are part of CI (a compile-time or
  test-time assertion that ranges don't overlap).

### DPR-03 — Dual-App Validation (Cross-Repo Gated)

Adapt the analyzer to consume the DPR runtime surface and record the
remaining app-local differences as explicit `services` / `cores` /
`scan_mode` policy inputs rather than copied platform code. Land
`DAA-01-B-2` in the analyzer subrepo ratifying adoption.

Acceptance:

- Analyzer no longer carries `// Adapted from rlvgl@...` headers on
  `bsp.rs`, except as temporary compatibility shims documented in
  DAA-01-B-2 §10.
- `Analyzer` profile (`VideoMode`, `BareMetalLoop`, `Cm7Cm4`) boots
  end-to-end with the same audio/DSP behavior the analyzer has today.
- DPR-03 §15 entry cites the DAA-01-B-2 §15 commit hash (per
  INV-DPR-15).
- Any remaining copied code is listed in a DPR §15 deferral with a
  named owner and deletion trigger.

### DPR-04 — BSP Generator Reopen Gate

Decide whether the now-proven runtime surface should feed the BSP
generator, stay as a handwritten H747I-DISCO platform module, or split
into a generated silicon layer plus handwritten board policy.

Acceptance:

- Decision cites concrete demo/analyzer diff evidence from DPR-01/02/03.
- No generator work starts until DPR-04 ratifies the boundary.

## 9. Non-Goals

- DPR-00 does not require making `examples/stm32h747i-disco` a stable
  public library crate. (The crate itself stays an example; what gets
  re-exported is the `platform/`-side `BoardRuntime` surface.)
- DPR-00 does not attempt a general STM32H7 BSP generator. DPR-04 is
  the gate that decides whether such an effort is in scope.
- DPR-00 does not move demo UI, `DiscoController`, or app-specific
  dashboard/scope composition into `rlvgl-platform`.
- DPR-00 does not require analyzer code to become public or canonical.
  Analyzer is the second-app validation target, not the source of
  RLVGL truth.
- DPR-00 does not commit to a fifth scan-mode axis, a third pacing
  backend, or a third cores mode. Adding any of those is a §15
  Standards-Action change in a later phase.

## 10. Reconciliation Decisions

| Existing concept | DPR decision |
|---|---|
| `Stm32h747iDiscoDisplay::new` | Remains the low-level display constructor. DPR wraps it in `BoardRuntime` that owns prerequisites and policy. DPR-01 re-exports publicly. |
| `publish_back_and_wait` | Retained as a low-level primitive on `Stm32h747iDiscoDisplay`. DPR `FrameScheduler` owns the atomic slot, the ISR-side consume path, and the four MMIO writes named in INV-DPR-3. |
| `dsi_cmd_mode::handle_erif_isr` | Retained for `AdaptedCommand` scan mode. DPR `FrameScheduler::dsi_isr_body` calls it rather than duplicating it. |
| `dsi_cmd_mode::present` / direct raw casts in `stm32h747i_disco.rs::present` / `freertos_entry.rs` | All three consolidate into `FrameScheduler` per INV-DPR-3. DPR-01 acceptance gate. |
| FreeRTOS `present_task` / TIM7 holdoff | Extract policy and timing into platform; keep OS-specific wait primitives behind the `pacing` profile axis. TIM7 register access already uses `hwcore::regs::tim::TimBasic` (typed) — preserved. |
| Bare-metal vs FreeRTOS demo split | Today the demo ships both feature mixes (`cm7,desktop,...` and `cm7,freertos,...`). DPR-01 ratifies these as `Demo::bare_metal` and `Demo::freertos` rather than a single `Demo` preset. |
| Analyzer `peripheral_safe_stop` | Evidence for DPR-02. Equivalent behavior moves into RLVGL under platform ownership. First validator is `Demo::*` with `audio` service enabled. |
| Analyzer render-dirty gating | Frame pacing is platform policy. Exact heartbeat strategy is profile-specific and ratifies in DPR-01 (`pacing: FreeRtos` / `pacing: BareMetalLoop`) or DPR-03 (`pacing: BareMetalLoop` + cores `Cm7Cm4`). |
| Analyzer HSEM[6] init for CM4 wake | Becomes `Cores::Cm7Cm4 { hsem_lines: { line_6_in } }` profile field. INV-DPR-11 freezes the ordering rule. Platform-side init is part of `BoardRuntime::init` when `cores = Cm7Cm4`. |
| Heap base (demo: DTCM; analyzer: `0x2402_0000` D1 AXI) | DPR-01 ratifies `(heap_base, heap_size)` as a per-profile output. Apps consume the published values per INV-DPR-14. |
| Boot sentinels (analyzer `0xA11C_xxxx`) | Migrated into §5.3 `0x3800_0500..0x3800_0600` Board Runtime range under DPR-02. The `0xA11C_xxxx` literals become an analyzer-side deletion trigger documented in DAA-01-B-2 §10. |
| Codec reset ordering | Today the analyzer runs `codec_driver.reset()` after the display constructor; the demo's order depends on feature flags. DPR-02 ratifies the canonical order as: clock tree → heap → SDRAM → display init → codec reset (gated on `codec_reset` service). The order becomes part of the `BoardRuntime::init` contract. |
| Clock tree (PLL3_P=12.288 MHz, PLL3_R=32.768 MHz) | Inherited from demo, copied by analyzer. DPR-01 makes these Board Runtime guarantees per INV-DPR-12. Profiles MAY NOT downgrade these values; adding a profile that needs different PLL3 values is a §15 Standards-Action change. |
| GPIO bank ownership | LTDC/DSI display pins owned by `BoardRuntime`. Joystick (PK2..PK6), codec I2C4 (PD12/PD13), SAI audio pins, SDMMC pins remain app- or service-owned per INV-DPR-13. The bank-split call (`GPIOK.split()` etc.) happens app-side; the runtime takes only the pins it needs as typed handles. |

## 11. Open Questions Carried into DPR-01

These do not block DPR-00 ratification; they are deferred to DPR-01
deliverables. DPR-01 §15 MUST resolve each (or re-defer with a named
later phase).

- **PCDN-DPR-001:** Concrete Rust shape of `Demo::bare_metal` vs
  `Demo::freertos`. Two enum variants of `RuntimeProfile`? One variant
  with a `pacing` field? A const-generic backend type? DPR-01 picks one
  and the others lock out.
- **PCDN-DPR-002:** Whether `Analyzer` ships as a registered preset in
  `rlvgl-platform` itself, or only as `RuntimeProfile::Custom { ... }`
  with the analyzer subrepo owning the construction. The
  Standards-Action policy in §5.2 admits either; DPR-01 picks.
- **PCDN-DPR-003:** Whether `BoardRuntime::init` takes a tuple of
  peripheral sets (illustrated in §7) or a single builder that consumes
  PAC peripherals one at a time. INV-DPR-13 forbids the "god
  peripherals" shape; this question is about which type-system shape
  enforces it cleanly.
- **PCDN-DPR-004:** Whether `FrameScheduler` is parameterized over
  `ScanMode` at the type level (compile-time dispatch) or holds a
  runtime tag. Compile-time variant likely cleaner; needs validation
  against the FreeRTOS/bare-metal split.
- **PCDN-DPR-005:** Whether HSEM[6] is the only HSEM line DPR
  promises to support, or whether the `hsem_lines: HsemSet` field in
  `Cores::Cm7Cm4` admits an open registration policy. Analyzer uses
  only line 6 today; DPR-01 picks the registration policy for new
  lines.

## 12. Acceptance Checklist

DPR-00 is ratified when:

- [ ] §3 vocabulary is accepted.
- [ ] §5 axes and registration policies are accepted (named presets
      themselves may evolve in DPR-01 under their registration policies
      without re-ratifying DPR-00).
- [ ] §5.3 telemetry range table is accepted.
- [ ] §6 invariants INV-DPR-1 through INV-DPR-15 are accepted.
- [ ] §8 phase plan is accepted or amended.
- [ ] §10 reconciliation rows are accepted (additions allowed via §15
      amendment).
- [ ] §11 PCDN-DPR-001..005 are explicitly deferred to DPR-01.

## 13. Files Cited

- `docs/concepts/README.md`
- `docs/concepts/DCB-00-CONCEPTS.md`
- `docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`
- `docs/disco-freertos-guide/01-freertos-scaffolding.md`
- `platform/src/stm32h747i_disco.rs`
- `platform/src/dsi_cmd_mode.rs`
- `platform/src/display_init.rs`
- `platform/src/frame_sync.rs`
- `platform/src/hwcore/regs/` (typed register-block modules)
- `platform/src/hwcore/addr.rs`, `surface.rs`, `isr.rs`, `dca.rs`
- `platform/src/lib.rs` (re-export gates)
- `examples/stm32h747i-disco/src/freertos_entry.rs`
- `examples/stm32h747i-disco/src/main.rs`
- `examples/apps/disco-demo/src/lib.rs`
- Parent workspace:
  `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md`
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs`
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs`
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/hsem.rs`

## 14. Unblocks

Once ratified, DPR-00 unblocks DPR-01: defining the concrete runtime
boundary, consolidating display MMIO ownership, and migrating the demo
first. DPR-02/03/04 remain gated on DPR-01 acceptance.

## 15. Change Log

- **2026-05-19 (revision b)** — Substantive rewrite of §5, §6, §8,
  §10, §11, §13 against concrete evidence from analyzer-cm7 source
  tree and `platform/` surface. Changes:
  - §5.1 `ScanMode` decomposed into 4 axes (wake source, LTDCEN
    gating, TE arm, holdoff phase). `AdaptedCommandHoldoff` and
    `VideoShadowReload` renamed to `AdaptedCommand` and `VideoMode`
    as named presets over the axis set.
  - §5.2 `RuntimeProfile` decomposed into 4 orthogonal axes
    (scan_mode, services, pacing, cores). `Demo` split into
    `Demo::bare_metal` and `Demo::freertos`. `Analyzer` adopts
    `Cm7Cm4` cores mode.
  - §5.3 telemetry range table moved from "MAY publish" to a frozen
    address table covering playit, analyzer safe-stop, and reserved
    DPR ranges.
  - §6 added INV-DPR-11 (HSEM-before-CM4-unhalt), INV-DPR-12
    (clock-tree audio invariants), INV-DPR-13 (consumed-peripheral
    set), INV-DPR-14 (heap base as profile output), INV-DPR-15
    (cross-repo gating named).
  - §7 demoted to "Informative, Deferred to DPR-01"; the pseudo-Rust
    sketch reorganized to surface peripheral-set ownership rather
    than a `Peripherals`-shaped god struct.
  - §8 acceptance gates expanded: DPR-01 explicitly gates on
    display-MMIO writer consolidation and INV-DPR-13 compliance.
    DPR-02 gates on demo `audio`-service safe-stop validation.
    DPR-03 gates on DAA-01-B-2 §15 cross-repo ratification.
  - §10 added rows: HSEM init, heap base, boot sentinels, codec
    reset ordering, clock tree, GPIO bank ownership, bare-metal vs
    FreeRTOS demo split.
  - §11 reframed as deferred-to-DPR-01 deliverables (PCDN-DPR-001..005)
    rather than DPR-00 ratification blockers.
  - §0 added a row for cross-repo analyzer authority.
- **2026-05-19 (revision a)** — Initial Codex import. Captured the
  analyzer-surfaced copy boundary, established demo-as-first-app and
  analyzer-as-second-app validation roles, proposed an initial
  ScanMode/RuntimeProfile vocabulary and a four-phase plan. Superseded
  by revision b above; revision a vocabulary names (`AdaptedCommandHoldoff`,
  `VideoShadowReload`) are retired in favor of the axis-decomposed
  forms in revision b §5.1.
