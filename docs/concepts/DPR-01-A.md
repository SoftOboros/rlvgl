# DPR-01-A — Display MMIO Writer Migration Plan

**Status:** Draft 2026-05-19. Sub-letter to DPR-01 per the DCB-NN-A
precedent. Ratifies the per-site MMIO writer inventory, the
consolidation grouping (init-only vs. per-frame), and the migration
sequence for DPR-01a / DPR-01b code PRs.

This sub-letter is *not* a new vocabulary or invariant — those live in
DPR-00 and DPR-01. It is the authoritative per-site reference that
DPR-01 §10 cites.

## 1. Problem

DPR-00 INV-DPR-3 requires that after Board Runtime initialization, only
the `FrameScheduler` writes `DSI_WCR`, `DSI_WIER`, `DSI_WIFCR`,
`LTDC_L1CFBAR`, and `LTDC_SRCR`. DPR-01 §5.4 ratifies the scheduler's
shape. This sub-letter is the per-site evidence base for the migration:

- Which lines does INV-DPR-3 actually move?
- Which lines are *init-only* and stay where they are?
- In what order must the consolidation happen so the demo never
  regresses?

Without an explicit per-site sequence, DPR-01a risks landing a
half-finished consolidation that leaves the discipline scanner with a
mixed BASELINE.

## 2. Per-Site Writer Inventory

All paths relative to `/Users/iraabbott/rlvgl/`. Line numbers are
current as of 2026-05-19 (`v0.2.0` branch HEAD `cdff3f8`).

### 2.1 platform/src/stm32h747i_disco.rs

| Line | Register | Value | Phase | Owner under DPR-01 |
|---|---|---|---|---|
| 787 | `LTDC_SRCR` | `1` (IMR) | init: post-GCR enable | Stays in `Stm32h747iDiscoDisplay::new`. INV-DPR-3 does not cover init. |
| 788 | `LTDC_SRCR` | `1` | init: redundant | Removed by DPR-01a (duplicate of line 787). |
| 934 | `DSI_WIFCR` | `0x03` | init: DWT probe | Removed by DPR-01a — DWT probe is dev-only. |
| 952 | `DSI_WCFGR` | `wcfgr & !(1<<6)` | init: DWT probe | Removed by DPR-01a. |
| 955 | `DSI_WIFCR` | `0x03` | init: DWT probe | Removed by DPR-01a. |
| 964 | `DSI_WCR` | `0x0C` | init: DWT probe | Removed by DPR-01a. |
| 992 | `DSI_WCFGR` | `wcfgr` (restore) | init: DWT probe | Removed by DPR-01a. |
| 993 | `DSI_WIFCR` | `0x03` | init: DWT probe | Removed by DPR-01a. |
| 1110 | `LTDC_L1CFBAR` | `fb` | init: layer config | Stays in `Stm32h747iDiscoDisplay::new`. |
| 1118 | `LTDC_SRCR` | `1` | init: reload | Stays. |
| **1626** | **`LTDC_L1CFBAR`** | `next` (back fb) | **per-frame: `swap()`** | **Moves to `FrameScheduler::swap`.** |
| **1627** | **`LTDC_SRCR`** | `1` | **per-frame: `swap()`** | **Moves to `FrameScheduler::swap`.** |
| **1646** | **`DSI_WIFCR`** | `0x02` (CERIF) | **per-frame: `present()`** | **Moves to `FrameScheduler::present` (pre-retarget clear).** |
| **1650** | **`LTDC_L1CFBAR`** | `next` | **per-frame: `present()`** | **Moves to `FrameScheduler::present` (retarget).** |
| **1651** | **`LTDC_SRCR`** | `1` | **per-frame: `present()`** | **Moves to `FrameScheduler::present` (shadow reload).** |
| **1656** | **`DSI_WCR`** | `0x0C` (DSIEN \| LTDCEN) | **per-frame: `present()` (AdaptedCommand only)** | **Moves to `FrameScheduler::present`, gated on `S::PULSED_LTDCEN`.** |
| **1659** | **`DSI_WIFCR`** | `0x02` | **per-frame: `present()` (AdaptedCommand only)** | **Moves to `FrameScheduler::present`, gated on `S::PULSED_LTDCEN`.** |
| **1684** | **`DSI_WIFCR`** | `0x02` | **per-frame: `wait_frame_done()` / ISR** | **Moves to `FrameScheduler::consume_erif`.** |

### 2.2 examples/stm32h747i-disco/src/main.rs

| Line | Register | Value | Phase | Owner under DPR-01 |
|---|---|---|---|---|
| 3262 | `DSI_WIER` | `1 << 1` (ERIE) | init: one-shot ISR enable | Stays in `main.rs`. Consider promoting to `Stm32h747iDiscoDisplay::new` under DPR-01a §10 reconciliation, but not strictly required by INV-DPR-3 (init-only). |
| 3264 | `DSI_WIFCR` | `0x3FFF` | init: clear all pending | Stays. |
| 4143 | `LTDC_L1CFBAR` | (read) | telemetry: readback | Stays. INV-DPR-3 covers writes only. |

### 2.3 examples/stm32h747i-disco/src/freertos_entry.rs

| Line | Register | Value | Phase | Owner under DPR-01 |
|---|---|---|---|---|
| **~200** | **`DSI_WCR`** | `0x08` (DSIEN only) / `0x0C` (DSIEN \| LTDCEN) | **per-frame: `present_task` body** | **Moves to `FrameScheduler::present` under DPR-01b.** |
| **~200** | **`LTDC_SRCR`** | `1` | **per-frame: `present_task` body** | **Moves to `FrameScheduler::present` under DPR-01b.** |
| 599 | `LTDC_SRCR` | `1` | per-frame: `disable_ltdc_layer2()` | Moves under DPR-01b. Layer-2 disable is a one-off but per-frame in shape. |
| **~400+** | **`DSI_WCR`** | varied | **per-frame: holdoff retrigger** | **Moves under DPR-01b.** |

The exact line numbers in `freertos_entry.rs` shift as that file is
edited; DPR-01b PR description MUST capture the inventory at PR base
SHA and check each writer site is migrated.

### 2.4 examples/apps/disco-demo/src/lib.rs

No writes to any of the five tracked registers. The widget tree
emits `DiscoCommand` enums; platform adapters translate. INV-DPR-7 is
already satisfied for this file.

## 3. Consolidation Grouping

The 17 per-frame writes across `stm32h747i_disco.rs` and
`freertos_entry.rs` reduce to **four distinct operations** the
FrameScheduler must own:

### Op A — SWAP (interrupt-safe, no WCR pulse)

```text
// inside cortex_m::interrupt::free()
LTDC_L1CFBAR ← fb_addr
LTDC_SRCR    ← 1   (IMR — shadow reload at next frame boundary)
```

Used by `stm32h747i_disco.rs::swap` (lines 1626..1627). Relies on
WCFGR.AR=1 (auto-refresh) being set so LTDC retargets without an
explicit DSI_WCR pulse. Cheapest path; only safe when AR mode is
active.

### Op B — PRESENT (full pipeline, AdaptedCommand)

```text
DSI_WIFCR    ← 0x02  (clear stale CERIF before retarget)
LTDC_L1CFBAR ← fb_addr
LTDC_SRCR    ← 1     (shadow reload trigger)
// AdaptedCommand only:
DSI_WCR      ← 0x0C  (DSIEN | LTDCEN — pulse scan)
DSI_WIFCR    ← 0x02  (clear spurious ERIF from LTDCEN re-enable)
```

Used by `stm32h747i_disco.rs::present` (lines 1646..1659) and
`freertos_entry.rs::present_task` body. In `VideoMode` (analyzer),
the trailing two writes are skipped — `S::PULSED_LTDCEN == false`.

### Op C — CONSUME_ERIF (ISR-side flag clear)

```text
// in DSI ISR, after reading WISR.ERIF
DSI_WIFCR ← 0x02  (clear CERIF)
// snapshot DWT_CYCCNT, push to IsrChannel<ErifInfo, 1>
```

Used by `stm32h747i_disco.rs::wait_frame_done` (line 1684) and the
DSI ISR body in `main.rs`/`freertos_entry.rs`. Pacing impls read the
channel rather than calling Op C directly.

### Op D — INIT (one-shot at boot, NOT in scope)

The init sequence at `stm32h747i_disco.rs:787..1118` plus
`main.rs:3262..3264` is the *one-shot* register programming that
runs before any frame is presented. INV-DPR-3 explicitly carves this
out ("after Board Runtime initialization"). DPR-01 leaves Op D in
place; DPR-01a tidies it (removes the DWT probe at lines 934..993)
but does not move it under FrameScheduler.

## 4. Migration Sequence

### Phase 1 (DPR-01a)

1. **Add `platform/src/frame_scheduler.rs`** with the §5.4..§5.6
   types compiled but unused. No call-site changes; CI green on the
   scaffold alone. Run discipline scanner — BASELINE unchanged.
2. **Migrate Op A (`swap`).** Rewrite `Stm32h747iDiscoDisplay::swap`
   body to `self.scheduler.swap(fb)`. Discipline scanner sheds two
   BASELINE entries (lines 1626, 1627). Bench-flash bare-metal demo,
   confirm visible behavior unchanged.
3. **Migrate Op B (`present`, AdaptedCommand path).** Rewrite
   `Stm32h747iDiscoDisplay::present`. BASELINE sheds five entries
   (lines 1646, 1650, 1651, 1656, 1659). Bench-flash, confirm.
4. **Migrate Op C (`consume_erif`).** Rewrite
   `Stm32h747iDiscoDisplay::wait_frame_done` and the DSI ISR body in
   `main.rs` (bare-metal). BASELINE sheds line 1684 plus the bare-
   metal `_dsi_isr` writes. Bench-flash, confirm ERIF gating still
   works.
5. **Remove DWT probe (lines 934..993).** Dev-only diagnostic.
   BASELINE sheds eight entries.
6. **Re-export `Stm32h747iDiscoDisplay`** publicly. Widens the
   cfg-gate at `platform/src/lib.rs:149`. No behavior change; opens
   the surface for analyzer adoption.

Acceptance gate: pre-publish phases 0-7 all green, demo bare-metal
flashes and presents 24-hour soak with no flicker regression vs.
pre-DPR-01a golden frames.

### Phase 2 (DPR-01b)

1. **Add `platform/src/pacing/freertos.rs`** with `FreeRtosPacing`
   compiled but unused under `feature = "freertos"`.
2. **Migrate `freertos_entry.rs::present_task` body** to
   `scheduler.present(fb)` + `pacing.wait_holdoff(...)`. TIM7 init
   moves into `FreeRtosPacing::new`. ERIF semaphore handling moves
   into `FreeRtosPacing::wait_erif`.
3. **Migrate `freertos_entry.rs::disable_ltdc_layer2`** (line 599)
   to a scheduler method or eliminate the call if no longer needed.
4. **Migrate FreeRTOS DSI ISR body** to call the scheduler's
   `consume_erif` path. The semaphore `_from_isr` give still happens
   in the ISR shim around the scheduler call.
5. **Discipline scanner**: `freertos_entry.rs` BASELINE entries for
   `DSI_WCR` / `LTDC_SRCR` raw casts are removed.

Acceptance gate: demo FreeRTOS build presents at the same 17.9 fps
baseline (per memory `project_freertos_port_status`), TIM7 holdoff
still phases present writes ~32 ms after ERIF, 24-hour soak with no
regression.

### Phase 3 (DPR-01c, deferred)

ZephyrPacing skeleton. Not on the DPR-01 critical path.

## 5. Test/Validation Strategy

Each consolidation step has three checkpoints:

1. **`cargo test` workspace** — discipline scanner output diff:
   each migrated site appears as a BASELINE removal, no new violations.
2. **Bench flash + golden-frame capture** — pre-DPR-01a frame captures
   land in `target/test-artifacts/dpr-01-golden/` (one capture per
   demo build profile). Post-migration captures must be pixel-
   identical for static screens; for animated screens, the
   `RS`/`RE`/`RD` recorder protocol (per `playit/README.md`) captures
   a 60-frame event log that must match.
3. **24-hour soak** — `make flash-disco` + leave running with the
   star-crawl on. Memory `project_sdram_ltdc_interaction` and
   `project_flicker_probe_results` describe the failure modes to
   watch for. Any new flicker or bus-contention regression blocks
   the merge.

The discipline scanner is the *cheap* gate; the bench flash + soak
are the *load-bearing* gates because INV-DPR-3 is about preserving
behavior, not just shrinking BASELINE.

## 6. Resolution

**Proposed:** Ratify this sub-letter alongside DPR-01 §12. DPR-01a
and DPR-01b PRs reference §4's phase-1 and phase-2 sequences and cite
this sub-letter for the per-site inventory at PR base SHA.

**Closes when:** all per-frame writer sites listed in §2 are
migrated, discipline scanner BASELINE for the five tracked registers
is empty in `frame_scheduler.rs`-external code, and the §5
validation checkpoints have all green entries in DPR-01a's and
DPR-01b's §15 change logs.

## 7. Change Log

- **2026-05-19** — Initial draft. Captures the writer inventory from
  the DPR-01 evidence-gathering pass (three parallel research streams
  run 2026-05-19), proposes the four-operation consolidation
  grouping (Op A swap, Op B present, Op C consume_erif, Op D init),
  and lays out the DPR-01a / DPR-01b phase sequences.
