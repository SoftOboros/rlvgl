# DCB Retrospective — divergences, refactor points, forward constraints

**Status:** Drafted 2026-05-03. Initiative-completion
retrospective for the DCB (DMA Cacheable Buffers) initiative
on rlvgl `v0.2.0`. Not a chronicle and not a celebration — a
delta against the original DCB-00 spec, organized for future
register-mashing-discipline-scoped initiatives to consume.

Retrospective in the agile sense: surfaces what diverged from
plan, what gates worked / didn't work, what patterns to carry
forward, and what preconditions future initiatives must
satisfy. The initiative-retrospective convention is documented
in `CLAUDE.md` "Spec-Before-Code Planning Discipline →
Initiative retrospective"; one retrospective per multi-phase
initiative, co-located with the phase docs at
`<initiative-dir>/<INIT>-RETROSPECTIVE.md`.

This doc is a **historical artifact** with one normative
section (§6 forward constraints). Behaviour PRs reference
DCB-00 directly; this retrospective is the bridge between
*what we shipped* and *what to do differently next time*.

## 1. Outcome snapshot

### Final architecture

`platform/src/hwcore/dca.rs` houses the typestate set, the
`DcaCache` trait, and the `DcaCacheCtx<'a, C>` plumbing wrapper.
Three storage families: `DcaBuf<T, N>` (single circular),
`DcaDoubleBuf<'b, T, N>` (DMA double-buffer mode, M0AR + M1AR),
and `DcaBuf` again with the `LtdcScan` typestate variant (LTDC
continuous read). Direction markers `Read` / `Write` are
zero-sized phantom types; transitions are by-value with
release-time live-recheck (INV-D7 / INV-D15) and direction-
specific cache-op placement (INV-D16 for LtdcScan; the §5
transition table for guards).

`platform/tests/discipline.rs` enforces the `raw_dcache` rule
that flags any direct `SCB::*_dcache_by_*` call outside
`hwcore::dca`'s whitelisted module. The BASELINE is empty;
`RLVGL_LINT_STRICT=1` ratifies (DCB-00 §12 (c) acceptance gate).

26 commits on rlvgl `v0.2.0` (8 spec amendments, 7 sub-letter
analyses, 4 typestate API commits, 7 retrofit/fix commits).
3 commits on disco-analyzer `bench-9-snapshot` (SAI1 TX +
SAI1 RX retrofits + cache-op-placement consumer updates).

### Deferred items (explicit)

1. **DCB-03** — DMA2D destination retrofit (FrameBuffer composes
   DcaBuf prescription). Closed-with-deferral. Reopen triggers
   in DCB-00 §10 + §14.
2. **DCB-02c-A** — DcaBuf push through `rlvgl_core::fs::
   BlockDevice` trait surface. Closed-with-deferral. Reopen
   triggers in DCB-00 §10 + §14.
3. **DCB-04-B** — Full `LtdcScan` typestate refactor for the
   FreeRTOS FRONT_FB swap atomics + bare-metal `Scanout::swap`.
   Closed-with-deferral. Reopen triggers in DCB-00 §10 + §14.

All three deferrals are **coupled** (per §6 reclassification)
to the disco's MPU-Write-Through SDRAM configuration; if that
configuration changes, all three reopen.

### Known residual risks

- **§12 (b) bench-flash validation is pending.** Audio fidelity
  reproduction on H747I-DISCO with the post-DCB-01d typestate
  API. The DCB-01b-A bug (see §2.1) was caught by manual code
  review, not by any automated gate. The bench is the only
  remaining verification that (a) the SAI1 retrofit preserves
  audio fidelity vs the pre-DCB pattern AND (b) the DCB-01d
  release-side cache-op placement actually eliminates the
  ~10.67 ms latency regression.
- **Future ports inherit the soft-retrofit shape.** Zephyr,
  BBB LCDC, esp32-p4 DPI panel: each port that adopts an
  LTDC-equivalent will land on the DCB-04 trait-dispatch
  pattern unless it explicitly opts into the `LtdcScan`
  typestate. The typestate is in-tree (DCB-01c) but unused
  in production code.
- **Three closures coupled to one MPU configuration.** The
  load-bearing assumption across DCB-03 / DCB-02c-A / DCB-04-B
  closures is `configure_mpu_sdram_writethrough` at
  `platform/src/stm32h747i_disco.rs:1322`. Any change to that
  function's behaviour silently invalidates three deferral
  arguments.
- **Cross-repo retrofit lockstep.** DCB-02 / DCB-02-R /
  DCB-02-r2 / DCB-02-R-r2 ship in disco-analyzer; the
  `rlvgl-platform` `[patch.crates-io]` entry in
  disco-analyzer's `Cargo.toml` is currently repointed at the
  local rlvgl tree. Reverting that patch (when DCB-01 / 01b /
  01c / 01d publish to crates.io) requires the published
  versions to match the consumer expectations; version-skew
  here would silently break the disco-analyzer build.

## 2. Divergence log

Capturing where reality diverged from the original DCB-00
spec. Each entry follows: **Assumption** (what the spec said) →
**Symptom** (observable failure) → **Root cause**
(mechanistic) → **Detection gap** (why automated gates didn't
catch it).

### 2.1. Read-direction cache-op timing was wrong in the spec

- **Assumption.** DCB-00 §5 (original ratification, 2026-05-02
  morning) prescribed: "HalfGuard entry: clean (Read), invalidate
  (Write)". DCB-01b shipped the entry-clean placement for both
  HalfGuard<Read> and BankGuard<Read>.
- **Symptom.** Audio-correct-but-2-half-periods-late on SAI1
  TX after DCB-02 retrofit. At 5.33 ms half-period (256 frames
  @ 48 kHz Fs), ~10.67 ms steady-state latency regression vs
  the pre-DCB pattern.
- **Root cause.** For a producer-into-inactive-bank model (CPU
  writes, DMA reads), cleaning *before* the writes publishes
  stale data; the writes that follow stay in cache until the
  next iteration's clean. Iteration N's writes reach RAM in
  iteration N+2's cache op. The pre-DCB SAI1 TX code in
  `analyzer-cm7/main.rs` cleaned *after* writes, which is the
  correct ordering for this access pattern.
- **Detection gap.** Unit tests used `NullCache` which records
  *which* op fired but not *when* relative to writes; trybuild
  fixtures cover compile-time soundness only; the discipline
  scanner has no semantic model; the §12 (b) bench gate hadn't
  run. The bug shipped through DCB-01b → DCB-02 → DCB-02-R →
  DCB-02b. Found by manual re-read of the implementation
  against the pre-DCB pattern during the post-DCB-04 cleanup
  pass. Fix took 5 commits (DCB-00e / DCB-01d / DCB-02-r2 /
  DCB-02-R-r2 / DCB-02b-r2).

### 2.2. SAI1 RX path doesn't fit `DcaBuf<T, N>`

- **Assumption.** DCB-00 §10 SAI1 row prescribed "becomes a
  `DcaBuf<i16, SAI1_TX_HALFWORDS, DeviceActiveCirc<Read>>`...
  SAI1 RX similarly becomes `DeviceActiveCirc<Write>` with a
  `HalfGuard<Write>`."
- **Symptom.** Initial DCB-02-R retrofit attempt against the
  `DcaBuf<i16, ...>` prescription: SAI1 RX uses STM32 DMA
  *double-buffer mode* (M0AR + M1AR alternating, CIRC=1, DBM=1)
  with two physical buffers at non-contiguous addresses
  (0x3000_0000 + 0x3000_1000, 3 KiB gap). `DcaBuf` assumes a
  single contiguous buffer with halves; the prescription
  doesn't fit.
- **Root cause.** The original DCB-00 §10 row was written
  before the actual SAI1 driver code was carefully read. The
  `DcaBuf` prescription assumes a circular ring with halves;
  STM32 DMA double-buffer mode is structurally a different
  engine mode (M0AR + M1AR + CT bit + MBM flag).
- **Detection gap.** Code review during DCB-00 ratification
  didn't catch the engine-mode mismatch. The doc-vs-code drift
  surfaced when the retrofit physically couldn't be written
  against the prescribed shape. Required DCB-02-A sub-letter
  + DCB-00b §15 amendment + DCB-01b API extension (~150 lines
  of new typestate family + 8 unit tests + 3 trybuild fixtures)
  before DCB-02-R could ship.

### 2.3. Write-Through SDRAM was the load-bearing assumption all along

- **Assumption.** DCB-00 §0 / §2 framed the initiative around
  "Cortex-M7 D-cache covers SDRAM... DMA bypasses cache".
  Implied that the cache discipline is universally load-bearing
  on DMA destinations.
- **Symptom.** During DCB-04-A drafting (LTDC scanout
  pre-clean analysis), re-reading
  `platform/src/stm32h747i_disco.rs:1322`
  (`configure_mpu_sdram_writethrough`) revealed that SDRAM is
  MPU-configured Write-Through Non-Shareable (TEX=0, C=1,
  B=0, S=0). Under Write-Through, CPU writes hit RAM in real
  time; the explicit `clean_dcache_by_address` call at
  `freertos_entry.rs:1011, 1449` is *cache-redundant*. The
  load-bearing primitive is the DSB that drains the AXI write
  buffer.
- **Root cause.** The original DCB-00 framing inherited the
  general M7 cache-coherency narrative without auditing the
  actual MPU configuration on the disco platform. The MPU was
  set up correctly (Write-Through is the right choice for
  display scanout traffic on SDRAM); the spec just didn't
  acknowledge it.
- **Detection gap.** None of the automated gates inspect MPU
  state. The discovery reframed the entire deferral
  rationale for the three closed-with-deferral phases —
  DMA2D, SDMMC, LTDC scanout — all of which target SDRAM
  destinations and therefore have no current need for the
  prescribed cache discipline. INV-D16 (the clean-AND-barrier
  contract for `LtdcScan::present`) was added to DCB-00 §6
  to capture the AXI-write-buffer-drain semantics that the
  original spec missed.

### 2.4. `DeviceActiveCirc<DIR>` was over-specified

- **Assumption.** DCB-00 §5 (original ratification) defined
  `DIR ∈ {Read, Write, ReadWrite}` for `DeviceActiveCirc<DIR>`.
- **Symptom.** No consumer for `ReadWrite`. The
  cache-op transition table required ad-hoc handling for the
  `ReadWrite` case (clean+invalidate on inactive half) that
  was never exercised.
- **Root cause.** Designing for hypothetical chained M2M
  pipelines without a named first user.
- **Detection gap.** Caught during DCB-00a clarifications
  (same-day tightening pass before §15 ratification). The
  `ReadWrite` direction was removed via Standards Action
  subtraction with the documentation note that future
  consumers can re-add it via §15 amendment with a named
  first user.

### 2.5. DCB-02b's `PollResult::NeedRefill { buf: *mut u8 }` left a residual unsafe block

- **Assumption.** DCB-02b's "minimum retrofit" treated the
  cache discipline as the only DCB-relevant concern. The PCM
  byte writes through `*mut u8` raw pointers were left as a
  separate concern.
- **Symptom.** Post-DCB-02b, the disco bare-metal binary's
  audio refill path retained an `unsafe { copy_nonoverlapping
  (...); write_bytes(...); }` block targeting a raw pointer
  derived from `PollResult::NeedRefill::buf`.
- **Root cause.** The retrofit boundary was drawn at "make the
  cache op type-tracked" rather than "make the entire data
  flow type-tracked". The destination-side raw pointer was
  inherited from the pre-DCB API and not revisited.
- **Detection gap.** No gate measures "fraction of unsafe
  blocks the type system *could* cover". Found during the
  post-DCB-04 cleanup sweep. Fixed via DCB-02b-A → DCB-00f
  → DCB-02b-A2: `PollResult::NeedRefill` removed; new
  `poll_refill<F>(refill: F)` closure-based API where the
  destination is a safe `&mut [u8]` slice.

### 2.6. The §10 "Replaces" prescription overconfidence (× 3)

- **Assumption.** DCB-00 §10 prescribed full retrofits for
  DMA2D destinations, SDMMC R/W buffers, and LTDC scanout
  pairs, all using the same "wrap in DcaBuf typestate" shape.
- **Symptom.** Three of those prescriptions had no observable
  runtime benefit on the only platform (disco) actually
  exercising the path. DMA2D: Write-Through SDRAM + DMA2D
  bypasses cache → no cache discipline needed. SDMMC: same
  argument; plus `embedded_sdmmc::BlockDevice` is third-party
  and can't take a DcaBuf parameter. LTDC: the FreeRTOS
  render/present split shares state via atomics that don't
  fit `&'static mut DcaBuf`.
- **Root cause.** DCB-00 §10 was written aspirationally:
  "this is the design we'd have if we were starting from
  scratch". The actual code already worked, used pre-DCB
  patterns that were grandfathered, and existed on a single
  MCU + MPU configuration that obviates the runtime payoff.
  The §10 prescriptions were forward-looking design hygiene
  rather than fixes for observed bugs.
- **Detection gap.** Cost-vs-benefit wasn't analyzed at
  ratification time. Each closure surfaced as a separate
  sub-letter (DCB-03-A, DCB-02c-A, DCB-04-B) once the actual
  retrofit was attempted or scoped. Three closures-with-
  deferral with explicit reopen triggers — total spec churn:
  3 sub-letters + 3 §15 amendments — was the correction.

## 3. Refactor points

Decision inflection nodes where the initiative changed
direction. Each entry: **Trigger** (what forced the pivot) →
**Alternatives** (what was considered) → **Selection**
(constraint-driven rationale) → **Cost of switch** (what was
paid).

### 3.1. DCB-00b — add `DeviceActiveDoubleBuf<DIR>` family

- **Trigger.** DCB-02-A surfaced that SAI1 RX uses STM32 DMA
  double-buffer mode, not circular mode; the original
  `DcaBuf` prescription doesn't fit.
- **Alternatives.** (A) Add `DeviceActiveDoubleBuf<DIR>`
  parallel typestate family. (B) Linker-coerce RX buffers
  contiguous, reuse `DeviceActiveCirc`. (C) One-shot
  `DeviceWritePending` per bank, re-lent on each TC. (D)
  Permanent BASELINE entry.
- **Selection.** Option A. Hardware fidelity: DMA double-
  buffer mode is a first-class engine mode with its own
  register layout (CT bit, M0AR + M1AR, MBM flag); shoehorning
  it into the circular family either misrepresents engine
  state (Option C) or breaks live-recheck (Option B).
- **Cost.** ~150 implementation lines + 8 unit tests + 3
  trybuild fixtures + DCB-00b spec amendment + delayed
  DCB-02-R retrofit by one cycle.

### 3.2. DCB-00e + DCB-01d — Read clean moves to release

- **Trigger.** DCB-01b-A characterised the entry-clean off-by-
  one as a steady-state latency regression. The bug had
  shipped through DCB-01b → DCB-02 → DCB-02-R → DCB-02b
  before being found.
- **Alternatives.** (A) Move clean to release for Read; keep
  entry for Write. (B) Document the latency, defer the fix
  (with a "pre-arm extra silence" workaround). (C) Emit clean
  at both entry and release.
- **Selection.** Option A. Restores the pre-DCB SAI1 TX shape
  (write-then-clean); brings the continuous-Read family into
  line with `DcaBuf<DeviceWritePending>` and
  `LtdcScan::present` which already use "publish at the
  explicit checkpoint" semantics. Symmetric `release(ctx,
  current)` signature for both directions.
- **Cost.** Breaking API change on `HalfGuard::release` /
  `BankGuard::release` (gain `&mut DcaCacheCtx`). 4 consumer
  call-site updates: DCB-02-r2 + DCB-02-R-r2 (disco-analyzer)
  + DCB-02b-r2 (audio_player). Unit-test assertion-order
  flips for the affected fixtures. ~250 lines diff total.

### 3.3. DCB-00f + DCB-02b-A2 — `poll_refill<F>` closure API

- **Trigger.** DCB-02b-A surfaced the residual raw-pointer
  write path in the disco audio refill.
- **Alternatives.** (A) Callback-based `poll_refill<F>(refill:
  F)`. (B) Token returned by separate `acquire_refill()`
  method, with `commit(pcm_bytes)` finaliser. (C) Defer
  indefinitely; close as discretionary. (D) Soft retrofit:
  add `DcaSlice<'a>` runtime-sized primitive.
- **Selection.** Option A. Standard Rust idiom for scoped
  resources (`with_*`-style); single in-tree consumer; no
  self-referential token complexity (the closure scope IS
  the bank-guard scope; lifetimes flow naturally).
- **Cost.** Breaking change to `audio_player` API:
  `PollResult::NeedRefill` variant removed; `poll()` +
  `refill_done()` replaced. Single in-tree consumer
  (disco bare-metal binary) updated mechanically. Net `unsafe`
  block count in disco audio path decreased.

### 3.4. DCB-00d / DCB-00g / DCB-00h — three closures-with-deferral

- **Trigger.** Cost-vs-benefit analysis of the three §10
  prescriptions (DMA2D, SDMMC trait push, LTDC freertos
  retrofit) all showed zero observable runtime benefit on
  Write-Through SDRAM disco.
- **Alternatives** (per phase). For each: (A) Full §10
  prescription. (B) Partial / parallel API. (C) Close-with-
  deferral. (D) Soft-retrofit variants.
- **Selection.** Option C for all three. Same reasoning across
  all three phases: the spec prescription is forward-looking;
  no current consumer benefits; full retrofit cost exceeds
  zero-runtime-benefit; explicit DCB-NN-B reopen path
  preserves the prescription if a real consumer materializes.
- **Cost.** Three sub-letter analyses + three §15 amendments
  = six doc-only commits. No code churn. Future-maintainer
  cost: each reopen requires a sub-letter analysis with a
  named first user.

## 4. Mitigation patterns (portable)

Abstracted from the divergences and refactor points. These
are reusable units for future register-mashing-discipline-
scoped initiatives.

### 4.1. "Producer-into-X → cache op at release, not entry"

**When**: a typestate guard exposes a slice to the CPU for
in-place writes, and a DMA engine consumes the same slice
later.

**Apply**: emit the `clean` cache op at guard `release`, not
at guard construction. The pre-DCB SAI1 TX pattern (write-
then-clean) is the canonical reference; any guard-based API
must preserve that ordering. For the inverse direction
(consumer → guard exposes engine-written slice for CPU
read), emit `invalidate` at construction (drain stale lines
before the read).

**Encode as**: §5 transition-table cell distinguishes per-
direction boundary. Future typestate families adopting the
guard pattern reference this rule explicitly.

### 4.2. "Trait-dispatch as containment when typestate is invasive"

**When**: a §10-prescribed full typestate retrofit costs more
than the runtime safety it would deliver on the only current
consumer (e.g. Write-Through memory makes the cache op a
no-op; or the consumer trait is third-party and can't take
typestate parameters).

**Apply**: route the cache calls through a trait-dispatch
pattern (`DcaCacheCtx::cache.invalidate/clean/barrier`) rather
than threading typestate through the trait surface. The trait
impl lives in DCB's owning module (whitelisted by the scanner
rule); call sites in retrofit consumers don't match the
scanner pattern. BASELINE shrinks without forcing a
buffer-typestate refactor.

**Encode as**: pre-flight check before sub-letter analysis —
"does the §10 prescription survive on this MCU's MPU
configuration?" If not, sub-letter analysis with explicit
options including trait-dispatch as Option C/D.

### 4.3. "Fixed-address typestate wrapping → AtomicBool + MaybeUninit + raw-ptr cast"

**When**: a `DcaBuf<T, N>` (or `DcaDoubleBuf<'static, T, N>`)
must wrap memory at a known fixed address (SDRAM-resident DMA
buffer, dedicated audio TX/RX banks, scanout framebuffers).

**Apply**: declare a `static AtomicBool _INIT` + `static mut
_BUF: MaybeUninit<DcaBuf<...>> = MaybeUninit::uninit()` pair.
First call to the init path swaps the atomic with `AcqRel`
ordering; if the previous value was `false`, write the
`MaybeUninit` via `(&raw mut _BUF).write(MaybeUninit::new(
DcaBuf::from_addrs(...)))`. Subsequent calls read via raw
pointer cast through `(&raw mut _BUF).cast::<DcaBuf<...>>()`.

**Encode as**: candidate for a `dca_static!` macro or a
`StaticDca<T, N>` helper struct. Repeated three times in
this initiative (SAI1 RX, audio_player consumer in main.rs,
the disco main.rs `audio_dca` init); ergonomic cost is real
even though the pattern is sound.

### 4.4. "Sub-letter analysis BEFORE implementation when §10 row doesn't fit reality"

**When**: a §10 reconciliation row prescribes a shape that
doesn't fit observed code (engine mode mismatch, buffer
geometry mismatch, ownership-pattern mismatch).

**Apply**: write a sub-letter (DCB-NN-X.md) with explicit
options A/B/C/D and recommendation; get owner ratification
via DCB-00 §15 amendment; only then implement. Don't ship a
half-retrofit.

**Encode as**: DCB-00 §15 policy gate. The sub-letter
discipline is already documented; the retrospective
observation here is that we *had* it and shouldn't have
shipped DCB-01b without sub-letter review of the cache-op-
placement question — the spec text was the bug, not just
the implementation.

### 4.5. "Closure-with-deferral for forward-looking §10 prescriptions"

**When**: a §10-prescribed retrofit has no current consumer
and the prescription is forward-looking design hygiene rather
than a fix for an observed bug.

**Apply**: close with explicit reopen triggers in §10 + §14.
Each trigger names a concrete condition (a port that adopts
the engine; a memory configuration change; a feature requiring
the typestate's specific guarantees). DCB-NN-B reopen path
named with a placeholder for the first user's identity.

**Encode as**: standard sub-letter shape with §3 (options) §4
(recommendation) §5 (proposed amendments) §6 (implementation
plan). The closure ratification doesn't gain new invariants
or APIs; it amends only the §10 reconciliation prescription
+ §14 unblock entry.

### 4.6. "Self-referential typestate state-machine → enum + `mem::replace`"

**When**: a struct needs to hold typestate-tracked DMA buffer
ownership across multiple method calls, and the typestate
handle borrows from the struct's own storage.

**Apply**: model the typestate as an enum field
(`DcaState { Cpu(...), Active(...), Transitioning }`) and use
`core::mem::replace(&mut self.dca, DcaState::Transitioning)`
to swap states by value. The `Transitioning` sentinel handles
the brief window where the field is mid-update. Avoids
self-referential structs (which Rust doesn't support without
Pin/PhantomPinned tricks).

**Encode as**: pattern reference in `audio_player.rs` (DCB-02b
implementation). Future engine drivers needing the same
shape adopt this idiom.

## 5. Deferred work reclassification

Per the framework: **Safe** (orthogonal, no impact on core
invariants), **Coupled** (affects assumptions; must be
revisited with context), **Abandoned** (explicitly killed).

### 5.1. Deferred (coupled): DCB-03 — DMA2D destination retrofit

- **Coupled to**: disco MPU Write-Through SDRAM
  configuration; the DMA2D consumer set being SDRAM-only.
- **Revisit context**: if `configure_mpu_sdram_writethrough`
  is revised, OR a new DMA2D consumer in non-Write-Through
  cacheable RAM is introduced (D1 SRAM tile cache, AXI SRAM
  glyph atlas, M2M-only off-screen buffer pairs), OR a port
  without H7-style WT defaults adopts DMA2D.
- **Reopen ID**: DCB-03-B (sub-letter analysis with named
  first user).

### 5.2. Deferred (coupled): DCB-02c-A — DcaBuf push through BlockDevice

- **Coupled to**: disco MPU Write-Through SDRAM; the
  third-party `embedded_sdmmc::BlockDevice` trait shape.
- **Revisit context**: new SDMMC destination in non-Write-
  Through cacheable RAM, port adopting SDMMC without
  Write-Through defaults, or a consumer needing strict
  32-byte buffer alignment as compile-time contract.
- **Reopen ID**: DCB-02c-B.

### 5.3. Deferred (coupled): DCB-04-B — full LtdcScan retrofit

- **Coupled to**: FreeRTOS render/present task-ownership
  pattern; bench-tuned ERIF deadline scheduling on the disco
  rendering hot path.
- **Revisit context**: a port adopts LTDC and needs typestate-
  tracked FB ownership the trait-dispatch shape doesn't
  provide; OR bench surfaces a cache-coherency hazard in the
  LTDC path attributable to the absence of typestate; OR a
  new feature requires `LtdcScan`-specific guarantees
  (compile-time FB-size contract, hardware-managed swap via
  DBM / DSI command-mode needing `DeviceLtdcScanDouble`).
- **Reopen ID**: DCB-04-B-2.

### 5.4. Deferred (safe): `DeviceLtdcScanDouble`

- **Coupled to**: nothing in the current tree; deferred from
  the DCB-00c amendment as "not in scope until needed".
- **Revisit context**: a consumer using hardware-managed swap
  (DSI command-mode swap, DBM display engines) where the LTDC
  reads bank A while CPU writes bank B with hardware swap
  on TE / VSYNC. The current `LtdcScan` typestate models
  software-managed swap.
- **Reopen ID**: DCB-NN (next available; not pre-numbered).

### 5.5. Abandoned: `DeviceActiveCirc<ReadWrite>`

- **Killed in**: DCB-00a (same-day clarifications, 2026-05-02).
- **Why abandoned**: no consumer; the cache-op transition
  table for ReadWrite would have been clean+invalidate which
  is overhead-without-benefit absent a chained M2M pipeline.
- **Resurrection prevention**: §5 ASCII tree explicitly notes
  "ReadWrite direction deliberately omitted; future consumers
  ratify via §15 amendment with a named first user". The
  removal is documented in DCB-00a §15 entry.

## 6. Forward constraints

Preconditions for the next register-mashing-discipline-scoped
initiative (or for any DCB-NN-B reopen). Treat these as
binding rules during planning, not aspirational guidelines.

### 6.1. Verify MPU memory attributes before designing typestate

Before the first sub-letter, run `grep -nE
"configure_mpu|MPU::|RNR|RBAR|RASR"` on the platform's BSP
and document the actual memory attributes for every region
the proposed typestate touches. The Write-Through-vs-Write-
Back distinction changes whether the cache op is load-bearing
at all. DCB-00 ratified before this audit; the result was
three §10 prescriptions that turned out to be forward-looking
design hygiene rather than fixes. Future initiatives must
gate ratification on this audit.

### 6.2. Bench-validate timing on hardware before declaring API complete

Automated tests don't model timing. The DCB-01b cache-op-
placement bug was a steady-state latency regression that
no automated gate caught — `NullCache` records ops without
ordering semantics; trybuild covers compile-time soundness;
the discipline scanner has no semantic model. The §12 (b)
bench-flash gate was the only mechanism that would have
surfaced the bug. Initiatives MUST flag hardware-dependent
gates as binding before declaring software-side completion.

### 6.3. Sub-letter analysis precedes implementation when §10 doesn't fit

Whenever a §10 reconciliation row's prescription doesn't fit
observed code (engine mode mismatch, buffer geometry, ownership
pattern), write a sub-letter analysis with explicit options
A/B/C/D and recommendation. Get owner ratification via §15
amendment. Only then implement. The DCB-01b cache-op-placement
bug shipped because the spec was wrong AND no sub-letter
review caught it before implementation. Spec text is a
ratifiable artifact; it can be wrong; sub-letter discipline
is the validation layer.

### 6.4. Default to closure-with-deferral when no current consumer

If a §10-prescribed retrofit has no observable runtime
benefit on the platforms currently exercising the path, the
default is closure-with-deferral with explicit reopen
triggers. Three closures-with-deferral on this initiative
(DCB-03 / DCB-02c-A / DCB-04-B) all closed on the same
reasoning; the pattern is the answer when the §10 prescription
is forward-looking. Don't ship half-retrofits or partial
parallel APIs as "interim" measures.

### 6.5. Self-referential typestate ownership is forbidden

Typestate handles that need to live across multiple method
calls of an owning struct MUST use the `enum DcaState { ... }`
+ `core::mem::replace` pattern. Don't design APIs that
require a self-referential token (token holds a borrow into
the struct's own field). Rust's exclusive-borrow rule plus
`Pin`'s ergonomic cost make the alternative unworkable.

### 6.6. Cross-repo retrofit timing is a separate constraint

When a phase ships in a different repo than the API surface,
the path-patch repointing in the consumer's `Cargo.toml`
introduces a temporal dependency that's invisible to the
ratification chain. Document it explicitly in the §14 unblock
entry. DCB-02 / DCB-02-R timing dependency on rlvgl-platform
publish was documented in DCB-00a §15; future initiatives
should follow that template.

### 6.7. Discipline scanner false positives need per-line opt-out, not BASELINE

When the regex matches a legitimate construct that isn't a
violation (e.g., `&'static mut` containing the substring
`static mut `), the answer is a per-line `// rlvgl-discipline:
allow(<rule_id>)` marker, not a BASELINE entry. BASELINE is
for grandfathered violations awaiting retrofit; opt-out
markers are for false positives. The two have different
ratification paths (BASELINE entries imply a future retrofit;
opt-outs imply ongoing accepted exceptions).

## 7. Provenance hooks

Linking each divergence and refactor point to the
authoritative artifacts so future agents can traverse:
**outcome → issue → fix → underlying evidence**.

### 7.1. Divergence-to-fix traversal

| Divergence (§2) | Sub-letter | Spec amendment | Implementation |
|---|---|---|---|
| 2.1 Cache-op-placement bug | `DCB-01b-A.md` | DCB-00e (`8665162`) | DCB-01d (`b47b9a1`) + DCB-02-r2 / DCB-02-R-r2 (disco-analyzer `a74252d`) + DCB-02b-r2 (`d9310a0`) |
| 2.2 SAI1 RX engine-mode mismatch | `DCB-02-A.md` | DCB-00b (`87dd5fb`) | DCB-01b (`f263ad5`) + DCB-02-R (disco-analyzer `1df2b9c`) |
| 2.3 Write-Through SDRAM discovery | `DCB-04-A.md` | DCB-00c (`d3c2b05`) — INV-D16 added | DCB-01c (`a825522`) — `DcaCache::barrier` + `LtdcScan` typestate |
| 2.4 ReadWrite over-spec | (no sub-letter; same-day fix) | DCB-00a (`2cb2eb7`) | (typestate set subtraction; no consumer impact) |
| 2.5 NeedRefill residual unsafe | `DCB-02b-A.md` | DCB-00f (`e5bccb3`) | DCB-02b-A2 (`28e56a8`) |
| 2.6 §10 over-prescription (×3) | `DCB-03-A.md` / `DCB-02c-A.md` / `DCB-04-B.md` | DCB-00d (`89d124b`) / DCB-00g (`b5ee975`) / DCB-00h (`fa9e3e6`) | (no implementation; closure-with-deferral) |

### 7.2. External evidence anchors

- **Pre-DCB SAI1 TX cache pattern** (write-then-clean ordering
  reference for §2.1): `analyzer-cm7/main.rs` pre-`c117a20`,
  in disco-analyzer's `bench-9-snapshot` history.
- **MPU Write-Through SDRAM configuration** (load-bearing
  assumption for §2.3 + three closures): `platform/src/
  stm32h747i_disco.rs:1322` (function
  `configure_mpu_sdram_writethrough`).
- **STM32 DMA double-buffer mode register layout**
  (drove §2.2 + DcaDoubleBuf typestate): RM0399 §15.6.5
  (`DMA_SxCR`), specifically the CT bit (bit 19) and MBM
  flag.
- **Cortex-M7 AXI write buffer behaviour** (drove §2.3 +
  INV-D16): ARMv7-M Architecture Reference Manual; PM0253
  (STM32 Cortex-M7 programming manual). Specifically: AXI
  write buffer can hold pending Write-Through writes past
  the cache write-back; DSB drains it.

### 7.3. Memory-system traversal

Future Claude / Codex agents working on a DCB-NN-B reopen or
a structurally similar register-mashing-discipline-scoped
initiative should traverse:

- `MEMORY.md` → `project_dcb_initiative_status.md` (initiative
  outcome + closures + reopen triggers).
- `docs/concepts/DCB-00-CONCEPTS.md` §15 (canonical change
  log with all amendments).
- This retrospective
  (`docs/concepts/DCB-RETROSPECTIVE.md`) for the
  divergences-and-mitigations corpus.
- `docs/concepts/DCB-NN-X.md` sub-letters for the
  resolution-decision history (preserved as historical
  record; no behaviour PR references them directly).

The traversal pattern is: **start at MEMORY.md, drill into
DCB-00 §15 for the canonical decisions, drill into this
retrospective for the failure-mode analysis, drill into
sub-letters for the option-space exploration**. Behaviour PRs
reference DCB-00 sections only.

## 8. Change log

- **2026-05-03 — Drafted.** Initiative-completion
  retrospective for the DCB initiative on rlvgl `v0.2.0`.
  Captures divergences (§2), refactor points (§3), portable
  mitigation patterns (§4), deferred-work reclassification
  (§5), and forward constraints (§6) for use by future
  register-mashing-discipline-scoped initiatives. Provenance
  hooks (§7) link each entry to the authoritative artifact
  (commit hash, doc section, datasheet reference).
- **2026-05-03 — Renamed `DCB-POST-MORTEM.md` →
  `DCB-RETROSPECTIVE.md`.** Aligned with the
  initiative-retrospective convention added to CLAUDE.md
  "Spec-Before-Code Planning Discipline → Initiative
  retrospective". Agile retrospective framing (neutral) over
  post-mortem (project-management connotation); content
  unchanged except for terminology cleanup.
