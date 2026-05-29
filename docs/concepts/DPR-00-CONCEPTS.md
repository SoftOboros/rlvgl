# DPR-00 — Disco Platform Runtime Concepts

**Status:** Draft 2026-05-19. Not ratified. This document sets up
the spec-before-code lineage for extracting reusable STM32H747I-DISCO
platform runtime support from the current demo/analyzer copy boundary.

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

If a DPR phase changes a frozen invariant, exported type name, ownership
boundary, or ISR/register-writer rule, this doc's §15 MUST be amended
first in a separate change.

## 1. Purpose

Make STM32H747I-DISCO board bring-up, display scheduling, warm-reset
cleanup, and runtime handoff a reusable RLVGL platform contract rather
than an application-local copy of the disco demo.

The immediate goal is not to build a generic STM32 BSP generator. The
goal is narrower: the existing disco demo remains the first validation
app, and the disco analyzer becomes the second app that proves the
platform surface is not accidentally demo-shaped.

## 2. Problem Statement

The disco analyzer surfaced a platform/API boundary problem. To build
the second H747I-DISCO application, it had to mirror or adapt pieces
from the demo because the reusable `rlvgl-platform` surface did not
publish the full board runtime contract.

Concrete evidence:

- `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md`
  ratified copying the example layout because the H747I-DISCO BSP,
  linker, FreeRTOS glue, and runtime shape were not available as a
  clean application dependency.
- `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs` copied
  or adapted clock setup, peripheral safe-stop, and SDRAM bring-up
  from the RLVGL example.
- `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs` owns
  display peripheral enables, GPIO splits, HSEM setup, codec reset
  sequencing, `Stm32h747iDiscoDisplay::new`, `DiscoController`
  construction, ERIF handoff, render-dirty gating, and defensive
  DSI/LTDC re-arm logic at application level.
- `platform/src/stm32h747i_disco.rs` already has useful primitives:
  SDRAM/framebuffer allocation, QoS configuration, `present()`,
  `swap()`, and `publish_back_and_wait()`. Those are still too low
  level for a second app: the app must own the ISR/handoff policy.
- `examples/stm32h747i-disco/src/freertos_entry.rs` contains a proven
  ERIF/TIM7 holdoff task model, but it is example-local rather than a
  reusable runtime object.

The failure mode is not that any one app is wrong. The failure mode is
that the second app had to learn the same H747I-DISCO board mythology
by copying demo code. DPR exists so the third app does not.

## 3. Glossary

Reserved DPR vocabulary. Capitalized use of these terms in DPR docs
MUST refer to the definitions below.

| Term | Meaning | Owner |
|---|---|---|
| **Disco Platform Runtime** | The reusable RLVGL-owned runtime surface for STM32H747I-DISCO board bring-up, display scheduling, warm-reset cleanup, IRQ ownership, and board telemetry. | DPR. |
| **First App** | The existing RLVGL STM32H747I-DISCO demo. It remains the first hardware validation target for every DPR phase. | DPR. |
| **Second App** | The disco analyzer. It proves the runtime surface can serve a non-demo application with audio/DSP constraints and different frame pacing needs. | DPR. |
| **Board Runtime** | The initialized collection of board services returned by a platform entry point: display, frame scheduler, optional audio/codec services, input handles, telemetry, and any ownership tokens needed to install ISRs safely. | DPR. |
| **Runtime Profile** | A named configuration for Board Runtime construction. Profiles choose policy, not hardware facts: e.g. demo vs analyzer display mode, audio ownership, render pacing, and telemetry slots. | DPR. |
| **Warm-Reset Safe Stop** | A bounded sequence that stops autonomous peripherals left running across CPU reset, clears pending IRQs, and records telemetry before normal init programs those peripherals. | DPR. |
| **Frame Scheduler** | The platform object that owns DSI ERIF handling, LTDC layer handoff, render-dirty gating, optional holdoff timing, and the single-writer rule for display MMIO. | DPR. |
| **Scan Mode** | The DSI/LTDC scheduling strategy selected by a profile. Initial values are `AdaptedCommandHoldoff` and `VideoShadowReload`; see §5. | DPR. |
| **Display MMIO Owner** | The sole code path allowed to write `DSI_WCR`, `DSI_WIER`, `LTDC_LxCFBAR`, and `LTDC_SRCR` after display init. In DPR, this is the Frame Scheduler. | DPR. |
| **Platform Telemetry** | SRAM4 or serial breadcrumbs emitted by Board Runtime primitives for bench validation. Telemetry addresses are part of a profile contract and MUST avoid application-owned slots. | DPR. |

## 4. Source-of-Truth Map

| Surface | Current location | DPR treatment |
|---|---|---|
| H747I-DISCO display constructor and framebuffer ownership | `platform/src/stm32h747i_disco.rs` | Keep as the low-level display owner; layer a Board Runtime and Frame Scheduler above it. |
| Adapted-command DSI helpers | `platform/src/dsi_cmd_mode.rs` | Reuse for `AdaptedCommandHoldoff`; no app-local DSI pulse helpers once DPR ships. |
| Full raw DSI/LTDC init for hosted paths | `platform/src/display_init.rs` | Keep as a lower-level hosted/Zephyr escape hatch; profiles decide whether it participates. |
| Frame sync traits | `platform/src/frame_sync.rs` | Extend or wrap into a concrete Frame Scheduler so apps do not reimplement ISR glue. |
| FreeRTOS present/render/touch task model | `examples/stm32h747i-disco/src/freertos_entry.rs` | First extraction candidate; move policy into platform without making FreeRTOS mandatory. |
| Analyzer safe-stop and video-mode ISR lessons | Parent workspace `streamz/submodules/disco-analyzer/analyzer-cm7/src/{bsp.rs,main.rs}` | Evidence for second-app requirements. Extract equivalent RLVGL-owned primitives; do not make analyzer code authoritative. |
| Demo widget tree and capabilities | `examples/apps/disco-demo/src/lib.rs` | Remains app layer. DPR MUST NOT move demo UI into platform. |

## 5. Frozen Decisions

The following decision sets are frozen for DPR-00. Changes require a
§15 amendment.

### 5.1 ScanMode

Registration policy: **Standards Action**.

```text
ScanMode =
  AdaptedCommandHoldoff { holdoff_us: u32 }
  VideoShadowReload
```

- `AdaptedCommandHoldoff` means ERIF stops LTDC scanout, a holdoff
  timer or equivalent waits for a fixed phase offset, and `present`
  retriggers LTDC. This is the demo/FreeRTOS holdoff family.
- `VideoShadowReload` means LTDC scans continuously and the scheduler
  retargets the layer framebuffer via shadow reload at ERIF/frame
  boundary. This is the analyzer-discovered video-mode safe family.

Adding a third scan mode requires naming a first app and second app
impact analysis in §15.

### 5.2 RuntimeProfile

Registration policy: **Specification Required**.

Initial profiles:

```text
RuntimeProfile =
  Demo
  Analyzer
  Custom { scan_mode: ScanMode, telemetry: TelemetryProfile }
```

`Demo` MUST preserve existing demo behavior unless a later phase
ratifies an intentional behavior change. `Analyzer` MUST expose the
policy hooks needed by the second app without importing analyzer code.

### 5.3 TelemetryProfile

Registration policy: **Expert Review**.

Telemetry profiles reserve SRAM4 slots and optional GPIO probes. A
profile MAY be target-app-specific, but it MUST publish its reserved
address range so apps can avoid collisions.

## 6. Runtime Invariants

- **INV-DPR-1: Demo-first validation.** Every DPR execution phase MUST
  migrate and validate the RLVGL disco demo before claiming the
  platform surface is ready for analyzer adoption.
- **INV-DPR-2: Analyzer as second-app proof.** A DPR API is not
  considered reusable merely because the demo builds. Each stable
  surface MUST name how the analyzer will consume it, even if the
  analyzer-side patch lands later.
- **INV-DPR-3: Apps do not own display MMIO.** After Board Runtime
  initialization, application code MUST NOT directly write `DSI_WCR`,
  `DSI_WIER`, `LTDC_LxCFBAR`, or `LTDC_SRCR`. Those writes belong to
  the Frame Scheduler.
- **INV-DPR-4: ISR ownership is explicit.** A Board Runtime profile
  MUST declare which interrupts it installs or expects the app to
  forward. Silent shared ownership of DSI, DMA2D, TIM7, HSEM, or
  SysTick is forbidden.
- **INV-DPR-5: Warm-reset cleanup is platform-owned.** If a profile
  enables SAI/DMA/codec services, it MUST either run a platform
  Warm-Reset Safe Stop or explicitly declare why the service is cold-
  boot-only.
- **INV-DPR-6: Telemetry does not collide.** Platform telemetry MUST
  reserve a published address range before writing SRAM4 breadcrumbs.
  App-owned telemetry ranges are not available to platform code.
- **INV-DPR-7: No demo UI leakage.** `DiscoController` and demo
  widgets remain application-layer constructs. Board Runtime MAY
  provide a display, input, and scheduler; it MUST NOT depend on the
  demo widget tree.
- **INV-DPR-8: Register-mashing discipline remains in force.** New
  platform code MUST use existing typed register, address, framebuffer,
  ISR, and DCB primitives where they exist. A raw MMIO escape hatch
  requires an explicit discipline marker and a DPR rationale.
- **INV-DPR-9: Profiles choose policy, not silicon facts.** Clock
  source availability, panel wiring, SDRAM geometry, and peripheral
  base addresses are board facts. Profiles MAY choose scan mode,
  service enablement, and telemetry, but MUST NOT redefine board facts.
- **INV-DPR-10: No application-local mythology.** Any H747I-DISCO
  sequencing rule that is required by both the demo and analyzer MUST
  move into DPR docs or a lower-level platform doc before being copied
  into a second application.

## 7. Initial API Sketch

This section is informative. Exact Rust signatures ratify in DPR-01.

```rust
pub enum RuntimeProfile {
    Demo,
    Analyzer,
    Custom(RuntimeConfig),
}

pub enum ScanMode {
    AdaptedCommandHoldoff { holdoff_us: u32 },
    VideoShadowReload,
}

pub struct Stm32h747iDiscoRuntime { /* opaque */ }

impl Stm32h747iDiscoRuntime {
    pub fn init(dp: Peripherals, profile: RuntimeProfile) -> Result<Self, InitError>;
    pub fn display(&mut self) -> &mut Stm32h747iDiscoDisplay<CpuBlitter, /* ... */>;
    pub fn frame_scheduler(&mut self) -> &mut FrameScheduler;
    pub fn telemetry(&self) -> &TelemetryProfile;
}

pub struct FrameScheduler { /* opaque */ }

impl FrameScheduler {
    pub fn mark_dirty(&self);
    pub fn publish_back_and_wait(&self, display: &mut impl BackBufferProvider);
    pub unsafe fn dsi_isr_body(&self);
}
```

The app should supply rendering code; the scheduler should own when a
rendered back buffer becomes visible and which MMIO writes are legal.

## 8. Phase Plan

### DPR-01 — Runtime Boundary and Demo Migration

Define the concrete `Stm32h747iDiscoRuntime` / `RuntimeProfile` /
`FrameScheduler` API and migrate the RLVGL disco demo onto it with no
intentional behavior change.

Acceptance:

- The demo no longer owns display MMIO handoff outside platform APIs.
- The demo build still supports the existing bare-metal and FreeRTOS
  profiles that are currently maintained.
- Public APIs are documented and pass `#![deny(missing_docs)]`.

### DPR-02 — Warm-Reset Safe Stop and Telemetry Profiles

Move analyzer-proven safe-stop concepts into RLVGL platform as a
profile-controlled service. Include SAI/DMA stop, IRQ pending clear,
codec reset sequencing hooks, and published telemetry ranges.

Acceptance:

- Demo can opt in without behavior regression.
- Analyzer can delete its local equivalent once it adopts the RLVGL
  submodule update.
- Telemetry range collision checks are documented.

### DPR-03 — Dual-App Validation

Adapt the analyzer to consume the DPR runtime surface and record the
remaining app-local differences as explicit policy inputs rather than
copied platform code.

Acceptance:

- Analyzer no longer carries copied display handoff or warm-reset
  platform code except as temporary compatibility shims.
- Any remaining copied code is listed in a DPR §15 deferral with a
  named owner and deletion trigger.

### DPR-04 — BSP Generator Reopen Gate

Decide whether the now-proven runtime surface should feed the BSP
generator, stay as a handwritten H747I-DISCO platform module, or split
into a generated silicon layer plus handwritten board policy.

Acceptance:

- Decision cites concrete demo/analyzer diff evidence.
- No generator work starts until DPR-04 ratifies the boundary.

## 9. Non-Goals

- DPR-00 does not require making `examples/stm32h747i-disco` a stable
  public library crate.
- DPR-00 does not attempt a general STM32H7 BSP generator.
- DPR-00 does not move demo UI, `DiscoController`, or app-specific
  dashboard/scope composition into `rlvgl-platform`.
- DPR-00 does not require analyzer code to become public or canonical.
  Analyzer is the second-app validation target, not the source of
  RLVGL truth.

## 10. Reconciliation Decisions

| Existing concept | DPR decision |
|---|---|
| `Stm32h747iDiscoDisplay::new` | Remains the low-level display constructor. DPR wraps it in a Board Runtime that owns prerequisites and policy. |
| `publish_back_and_wait` | Retained as a low-level primitive. DPR Frame Scheduler owns the atomic slot and ISR-side consume path. |
| `dsi_cmd_mode::handle_erif_isr` | Retained for adapted-command flow. DPR may wrap it rather than duplicating it. |
| FreeRTOS `present_task` / TIM7 holdoff | Extract policy and timing into platform; keep OS-specific wait primitives behind a profile/backend boundary. |
| Analyzer `peripheral_safe_stop` | Treat as evidence for DPR-02. Equivalent behavior should move into RLVGL under platform ownership. |
| Analyzer render-dirty gating | Treat as evidence that frame pacing is platform policy. Exact heartbeat strategy is profile-specific and ratifies in DPR-01 or DPR-03. |

## 11. Open Questions

- **PCDN-DPR-001:** Should `Demo` default to `AdaptedCommandHoldoff`
  for both bare-metal and FreeRTOS, or should bare-metal retain its
  current split until DPR-01 proves parity?
- **PCDN-DPR-002:** Should `Analyzer` consume `VideoShadowReload` as
  a named profile from RLVGL, or should it use `Custom` until the
  analyzer migration is complete?
- **PCDN-DPR-003:** Which SRAM4 telemetry range should RLVGL reserve
  for platform-owned H747I-DISCO runtime breadcrumbs without colliding
  with existing demo, playit, and analyzer diagnostics?

## 12. Acceptance Checklist

DPR-00 is ratified when:

- [ ] §3 vocabulary is accepted.
- [ ] §5 `ScanMode` and `RuntimeProfile` sets are accepted.
- [ ] §6 invariants are accepted.
- [ ] §8 phase plan is accepted or amended.
- [ ] §11 open questions are resolved or explicitly deferred to
      DPR-01.

## 13. Files Cited

- `docs/concepts/README.md`
- `docs/concepts/DCB-00-CONCEPTS.md`
- `docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`
- `docs/disco-freertos-guide/01-freertos-scaffolding.md`
- `platform/src/stm32h747i_disco.rs`
- `platform/src/dsi_cmd_mode.rs`
- `platform/src/display_init.rs`
- `platform/src/frame_sync.rs`
- `examples/stm32h747i-disco/src/freertos_entry.rs`
- `examples/apps/disco-demo/src/lib.rs`
- Parent workspace:
  `streamz/submodules/disco-analyzer/docs/concepts/DAA-01-B-RLVGL-INTEGRATION.md`
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/bsp.rs`
- Parent workspace:
  `streamz/submodules/disco-analyzer/analyzer-cm7/src/main.rs`

## 14. Unblocks

Once ratified, DPR-00 unblocks DPR-01: defining the runtime boundary
and migrating the demo first.

## 15. Change Log

- **2026-05-19** — Initial draft. Captures the analyzer-surfaced
  copy boundary, establishes demo-as-first-app and analyzer-as-second-
  app validation roles, freezes initial scan/profile vocabulary, and
  proposes the DPR-01..DPR-04 phase plan.
