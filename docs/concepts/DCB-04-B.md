# DCB-04-B — Full LtdcScan typestate refactor for FRONT_FB swap atomics

**Status:** Drafted 2026-05-03. Sub-letter analysis surfaced
during the post-DCB-04 sweep for unblocked outstanding items.
DCB-04 (rlvgl `f7b728c`) routed the two LTDC scanout pre-clean
sites through `DcaCacheCtx::cache.clean + barrier` — a "soft
retrofit" that closed the `raw_dcache` BASELINE entry without
threading the buffer ownership through the `LtdcScan<u8,
FB_BYTES>` typestate that DCB-01c added (rlvgl `a825522`).
DCB-04's commit notes flagged that the full typestate
retrofit was deferred because the FRONT_FB_ADDR atomic-swap
pattern in `freertos_entry.rs` would need rearchitecting to
fit a `&'static mut DcaBuf` ownership model. This sub-letter
inventories what the deferral actually trades, surveys the
option set, and recommends a path. Per the parent CLAUDE.md
"Sub-letter doc convention" the resolution folds into DCB-00
§10 / §14 / §15 (or ratifies a closure-with-deferral) as a
Standards Action amendment.

## 1. Purpose

Decide whether to push the LTDC scanout buffers through the
`LtdcScan<u8, FB_BYTES>` typestate as DCB-00 §10's amended
row prescribed — or accept the DCB-04 trait-dispatch shape as
the stable end state for the LTDC path on the disco firmware.

## 2. Problem statement

### 2a. Current state (post DCB-04)

`examples/stm32h747i-disco/src/freertos_entry.rs:1009-1011,
1447-1451` (the previous BASELINE entries) became:

```rust
ltdc_scanout_present(0xD180_0000usize, ARGB_BYTES);  // splash path
// ...
ltdc_scanout_present(front as usize, bytes as usize); // desktop path
```

where `ltdc_scanout_present` is a private helper at the top
of the file that wraps `DcaCacheCtx::cache.clean(addr, len) +
ctx.cache.barrier()`. The cache discipline is contained
inside `hwcore::dca`'s whitelisted SCB impl; the
`raw_dcache` BASELINE is empty.

**What's still pre-DCB-typestate**:

- `FRONT_FB_ADDR`, `BACK_FB_ADDR`, `FB_BYTES`, `FB_W`, `FB_H`
  are `static AtomicU32` slots in `freertos_entry.rs:378-382`.
  Two FreeRTOS tasks share these (render writes, present
  reads) — atomics enforce inter-task ordering.
- `front` is loaded from `FRONT_FB_ADDR.load(Acquire)` per
  iteration, cast to `*mut u8`, wrapped in a temporary
  `core::slice::from_raw_parts_mut(front, bytes)` for the
  widget-tree painter.
- The single-FRONT path (`No buf_ready — present retriggers
  the same FRONT. Single-buffer = zero flicker.`,
  freertos_entry.rs:1453-1454) keeps one address; the
  swap-mode path (line 742: `FRONT_FB_ADDR.store(back,
  Release)`) atomically toggles between two addresses.

### 2b. What the DCB-00 §10 amendment prescribed

The DCB-00c amendment (Option A from DCB-04-A) added:

> Wraps the buffer in `DeviceLtdcScan<T, N>` typestate.
> `paint_full()` returns a `&mut [T; N]` for in-place CPU
> writes; `present()` emits the existing clean + DSB pattern.

The full retrofit would mean:

- Construct `DcaBuf<u8, FB_BYTES>` via `from_addr(0xD180_0000)`
  (and a second one for the back buffer if the swap-mode
  path stays). Or wrap in `DcaDoubleBuf` for the swap pair.
- Transition once at boot: `start_ltdc_scan(&mut ctx)` →
  `LtdcScan<'static, u8, FB_BYTES>`.
- Per-render: hold the `LtdcScan` handle in shared state;
  call `paint_full()` for slice access; call `present(&mut
  ctx)` after the writes.

### 2c. The atomic-swap obstacle

`LtdcScan<'a, u8, FB_BYTES>` holds an `&'a mut DcaBuf` (a
unique mutable borrow). Two FreeRTOS tasks can't share `&mut
DcaBuf` — Rust's exclusive-borrow rule forbids it. The
existing `AtomicU32` atomics give "thread-safe shared mutable
address pointing at *the same* memory"; the typestate gives
"single-owner mutable handle to a *typed* memory region".
These don't compose without one of:

- **Mutex / spinlock**: wrap `Option<LtdcScan<'static, u8, N>>`
  in a critical-section primitive. Render task takes the
  lock, calls paint_full + present, releases. The lock cost
  per frame at 30 fps is ~33 ms apart — non-issue from a
  contention standpoint, but introduces a runtime
  abstraction the current code doesn't have.
- **Single-task ownership**: refactor so one task owns the
  `LtdcScan` handle and the other communicates *intent* via
  a channel / atomic flag, rather than sharing the address.
  Substantial rearchitect of the render/present split that's
  been bench-tuned (ERIF deadline scheduling per the
  project's prior bench notes).
- **`Send<&mut DcaBuf<...>>` shenanigans**: theoretically a
  `&mut` borrow could be moved between tasks via channel,
  but FreeRTOS task scheduling makes this fragile.

### 2d. What DCB-04-B would actually buy

- **Type-system-tracked FB ownership.** `paint_full()` returns
  a typed slice instead of `core::slice::from_raw_parts_mut(
  front, bytes)`. The `unsafe { ... }` blocks at the
  scanout sites shrink.
- **No new runtime guarantee.** The cache discipline is
  already contained via DCB-04's trait-dispatch. The DSB +
  clean pattern is unchanged.
- **No observed bug.** Bench-validated single-FRONT pattern
  is working. ERIF deadline scheduling notes in the project
  memory don't reference cache-coherency hazards in the LTDC
  path.
- **Forward-looking design hygiene for ports.** Future
  Zephyr / BBB / esp32-p4 DPI ports could adopt the same
  LtdcScan pattern — but those ports have their own
  scanout-engine specifics and aren't directly served by
  the disco's FreeRTOS atomic-swap retrofit.

### 2e. Cost inventory

- **~200-line refactor** in `freertos_entry.rs` (init_fbs +
  render_task + present_task + the two scanout sites).
- **Bench-validation risk in the disco rendering hot path.**
  The current freertos_entry.rs is bench-tuned for
  ERIF-deadline timing; reorganising ownership across tasks
  has timing implications even for type-system-only
  changes (Rust's borrow-checker forces certain
  serialisations the atomic pattern doesn't).
- **Unclear interaction with the bare-metal disco render
  path.** The bare-metal binary uses
  `platform/src/hwcore/surface.rs::Scanout::swap()`; that
  code already exists and has a different ownership model
  than the freertos atomic pattern. Two retrofits in
  parallel, or one that subsumes both?

## 3. Options

### Option A — Full §10 prescription: LtdcScan in freertos_entry.rs + Scanout

Replace `FRONT_FB_ADDR` / `BACK_FB_ADDR` atomics with shared
ownership of `LtdcScan<'static, u8, FB_BYTES>` (single-FRONT)
or `(LtdcScan<...>, LtdcScan<...>)` swap pair. Refactor
render_task / present_task to acquire the typestate via a
critical-section primitive. Update both scanout sites to
call `paint_full()` + `present()` directly. Same refactor
applied to `Scanout::swap()` in surface.rs.

**Pros**: spec-fidelity; type-system tracks FB ownership;
INV-D9 fully realized for LTDC; sets a portable pattern for
future ports.

**Cons**: substantial refactor in the disco rendering hot
path; bench-validation overhead is real; FreeRTOS task-
ownership model needs rework alongside the typestate; the
atomic-swap pattern that the bare-metal disco uses
(Scanout::swap) and the FreeRTOS render-task pattern have
different shapes — handling both adds complexity.

### Option B — Partial: typestate single-FRONT only; leave swap pattern

Refactor only the `single-buffer FRONT` path in
`freertos_entry.rs:1453-1454`. The swap-mode path (line 742
`FRONT_FB_ADDR.store(back, Release)`) stays on the atomic
pattern. Two LTDC ownership models coexist.

**Pros**: smaller refactor; the no-flicker single-FRONT
pattern (which is what bench-9l has been validating) becomes
typestate-tracked.

**Cons**: two models in the same file is a soft fork;
maintainers have to remember which path uses which
ownership shape; the swap-mode path is the one that exists
to handle dirty-rect tracking — not retrofitting it leaves
the more complex case unconverted.

### Option C — Defer DCB-04-B; close as deferred (mirrors DCB-03-A / DCB-02c-A)

Document closure-with-deferral; explicit reopen triggers;
existing DCB-04 trait-dispatch shape is the stable end state
for the LTDC path. The `LtdcScan<u8, FB_BYTES>` typestate
remains in-tree (DCB-01c shipped it; unit + trybuild tests
exercise it; future ports adopt it on first need).

**Pros**: minimum churn; honest about value delivered (the
DCB-04 retrofit cleared the BASELINE entry that mattered);
INV-D9's forward-looking reading covers the existing
pre-DCB FB chain as grandfathered; consistent with the
DCB-03-A and DCB-02c-A closure pattern (all three §10 rows
that were beyond the BASELINE-shrink track close on the
same Write-Through-SDRAM-says-it-doesn't-matter-runtime
reasoning).

**Cons**: leaves the `LtdcScan` typestate without a real
in-tree consumer beyond unit tests. Future ports will be
the first real users.

### Option D — Hybrid: typestate single-FRONT + Scanout retrofit; defer freertos swap

Same as Option B but ALSO retrofit the bare-metal disco's
`Scanout::swap()` to use two `LtdcScan` handles in the
swap-pair shape. The FreeRTOS swap-mode path stays on
atomics.

**Pros**: surface.rs::Scanout becomes the canonical
"LtdcScan + swap" reference for future ports; the freertos
single-FRONT path also gets the typestate.

**Cons**: still two models coexisting in the disco firmware
(freertos swap-mode is unchanged); the surface.rs Scanout
retrofit is its own refactor that may not pull its
own weight without a consumer.

## 4. Recommendation

**Option C** (defer DCB-04-B; close as deferred).

Justification:

- **Mirrors DCB-03-A's and DCB-02c-A's resolutions.** All
  three §10 rows that are *beyond* the BASELINE-shrink track
  (DMA2D, SDMMC, LTDC) close on the same reasoning: on the
  only platform that currently uses them, the runtime cache
  op is either a no-op (Write-Through SDRAM) or contained
  by DCB-04's trait-dispatch shape. The §10 prescription is
  forward-looking design hygiene rather than a fix for an
  observed bug.
- **No observed bug.** The §12 (c) acceptance gate is
  cleared. The §12 (b) bench-flash gate is hardware-
  dependent and tracks the SAI1 retrofit, not LTDC.
- **Hot-path bench-validation risk.** The disco rendering
  loop is bench-tuned (ERIF deadline scheduling per the
  project memory); a substantial refactor there for pure
  spec-driven cleanup is exactly the cost-benefit upside-
  down case DCB-03-A / DCB-02c-A rejected.
- **The typestate is preserved.** `LtdcScan<u8, FB_BYTES>`
  ships in DCB-01c with full unit + trybuild coverage. When
  a future port (Zephyr DSI command-mode, BBB LCDC, esp32-p4
  DPI panel) reaches the LTDC-equivalent retrofit step, it
  adopts the typestate from a clean greenfield rather than
  inheriting the disco's atomic-swap workarounds.
- **Reopen path is clear.** Future bench results showing a
  cache-coherency hazard in the LTDC path, or a port that
  needs the typestate-tracked ownership for a new
  scanout-engine feature, can reopen via DCB-04-B-2 with a
  named first user.

Option A is rejected on cost-vs-benefit grounds — large
refactor in a bench-tuned hot path for zero runtime benefit
on the only current consumer. Option B is rejected because
two ownership models in the same file is the kind of
soft-fork DCB-02-A's Option C rejection established as the
anti-pattern. Option D is rejected for the same Scanout-
without-real-consumer concern that DCB-03-A flagged.

## 5. Proposed amendments to DCB-00

If Option C is ratified:

### §10 row (LTDC scanout) — clarification

The LTDC scanout reconciliation row (currently positioned
between the SDMMC row and the cross-core row) gets a
closure-with-deferral rewrite analogous to DCB-03's and
DCB-02c-A's, naming DCB-04-B 2026-05-03 as the resolution
trigger. The existing DCB-04 (trait-dispatch) close-out
sentence stays; the row gains a normative **Reopen
triggers** clause.

### §14 — DCB-04 entry update

Add a "DCB-04-B 2026-05-03 closes the full LtdcScan typestate
push as deferred; DCB-04 trait-dispatch is the stable end
state for the LTDC path; **Reopen with DCB-04-B-2** when (a)
a port adopts LTDC and needs typestate-tracked FB ownership
that the trait-dispatch shape doesn't provide; (b) bench
results surface a cache-coherency hazard in the LTDC path
attributable to the absence of typestate; (c) a new feature
requires the `LtdcScan` typestate's specific guarantees
(e.g. compile-time FB-size contracts for a fixed-resolution
display)" entry.

### §12 acceptance gates — no change

(c) — already cleared by DCB-04.

### §15 ratification entry (proposed wording)

> **2026-MM-DD — DCB-04-B resolution: Option C (closure-
> with-deferral).** DCB-04-B (the §10-amended full
> `LtdcScan<u8, FB_BYTES>` typestate push for the
> `freertos_entry.rs` FRONT_FB swap atomics + the bare-metal
> `Scanout::swap` retrofit) is closed without code changes.
> The DCB-04 trait-dispatch shape is the stable end state for
> the LTDC path. The `LtdcScan<u8, FB_BYTES>` typestate
> remains in-tree (DCB-01c) for future ports to adopt.
> Mirrors DCB-03-A / DCB-02c-A: all three §10 rows beyond
> the BASELINE-shrink track close on the same Write-Through-
> SDRAM-says-it-doesn't-matter-runtime + bench-validation-
> risk-vs-zero-runtime-benefit reasoning. INV-D9 is read
> forward-looking — pre-DCB FB chain is grandfathered.
> No `BASELINE` change required (raw_dcache empty after
> DCB-04).

## 6. Implementation plan summary (informative)

If Option C is ratified, the entire DCB-04-B deliverable is
one §15 amendment commit to DCB-00. No code changes; no
behaviour PRs follow. With this closure, the DCB initiative
reaches its natural completion on the software side: every
named §10 row is either implemented or closed-with-deferral,
the BASELINE is empty, the typestate APIs are shipped + unit-
tested, and the §12 (b) hardware-dependent gate is the only
outstanding item.

## 7. Change log

- **2026-05-03 — Drafted.** Surfaced during the post-
  DCB-02c-A cleanup pass — the last named §10 row that
  hadn't been resolved one way or another. Recommendation:
  Option C (close DCB-04-B as deferred; the DCB-04 trait-
  dispatch shape is the stable end state; the `LtdcScan`
  typestate stays in-tree for future ports). Awaiting
  owner ratification via a DCB-00 §15 amendment.
