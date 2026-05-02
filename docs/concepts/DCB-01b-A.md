# DCB-01b-A — Cache-op placement for DMA-Read direction

**Status:** Drafted 2026-05-02. Sub-letter analysis surfaced when
re-reading DCB-01b's `HalfGuard` / `BankGuard` cache-op placement
during the post-DCB-04 sweep for unblocked outstanding items.
DCB-01b puts the cache `clean` at *guard entry* for the `Read`
direction; that placement is off-by-one against the pre-DCB
"write then clean" pattern. This doc characterises the issue,
recommends a fix, and proposes the API + spec amendments. Per
the parent CLAUDE.md "Sub-letter doc convention" the resolution
folds into DCB-00 §5 + DCB-01b API + the consuming retrofits
(DCB-02 / DCB-02-R) once ratified.

## 1. Purpose

Audit the cache-op timing for the continuous-Read DCB typestates
(`CircRead` + `HalfGuard<Read>`; `DbufRead` + `BankGuard<Read>`)
against the actual data-flow they're used for, and reconcile a
discrepancy between the spec and the pre-DCB working pattern.

## 2. Problem statement

### 2a. The pre-DCB SAI1 TX pattern (working bench)

Before DCB-02, `analyzer-cm7/src/main.rs` produced the SAI1 TX
ring data in this order, per iteration:

1. Determine inactive half (the one DMA is *not* currently
   reading) from `NDTR`.
2. Write fresh audio samples to the inactive half via raw
   pointer.
3. **`scb.clean_dcache_by_address(half_addr, half_bytes)`** —
   push CPU writes from the M7 D-cache out to D2 SRAM so DMA
   reads the fresh samples on the next bank flip.

The clean is **after** the writes. This is the natural shape
because the writes are what create the dirty cache lines that
need pushing.

### 2b. The DCB-01b shape (what shipped)

`HalfGuard<DIR>` / `BankGuard<DIR>` emit their cache op at
**guard entry** (per DCB-00 §5 transition table). For the `Read`
direction the entry op is `clean`. The DCB-02 / DCB-02-R
retrofits adopt this:

```rust
let mut guard = circ.half_guard(&mut ctx, inactive_half);  // ← clean here
let slice = guard.as_mut_slice();
// ... CPU writes via slice ...                            // ← cache dirty
let _ = guard.release(active_now);                         // ← no cache op
```

The clean fires **before** the CPU writes. Whatever is in cache
at that moment is what gets published to RAM — *not* the new
writes. The new writes stay in cache until the next iteration's
guard entry cleans them.

### 2c. Steady-state effect

Tracing through the iteration ring:

| Iter | Engine on | Guard exposes | Clean publishes | CPU writes | Engine next reads |
|------|-----------|---------------|-----------------|-----------|-------------------|
| 1 | A | B | (B was start-cleaned; no-op) | B = iter-1 data | B → reads pre-fill (stale) |
| 2 | B | A | (A was start-cleaned; no-op) | A = iter-2 data | A → reads pre-fill (stale) |
| 3 | A | B | iter-1 data → RAM[B] | B = iter-3 data (overwrites cache) | B → reads iter-1 data |
| 4 | B | A | iter-2 data → RAM[A] | A = iter-4 data | A → reads iter-2 data |
| ... | | | | | |

**Steady state**: audio plays back **2 iterations late** — at
SAI1's 5.33 ms half-period (256 frames @ 48 kHz Fs), that's
~10.67 ms extra latency vs the pre-DCB shape. Audio is
*correct* (no "bees" / no garbage data), just **delayed**.

### 2d. Why this isn't a "loud bees" repeat

The DCB-00 §2 motivating bug was *garbage* in the audio output —
DMA reading uninitialised SDRAM cache lines because the clean
was missing entirely. DCB-02's retrofit *does* clean (just at
the wrong moment), so RAM eventually receives the CPU writes.
The output isn't garbage — it's just shifted in time.

Three reasons this likely hasn't been bench-flagged:

1. The disco-analyzer is a spectrum analyzer; ~10 ms input
   latency is invisible in the spectrum / meter output.
2. The MCU loopback path (square-wave or live-mic → TX) plays
   *some* audio; without comparing against a reference it's
   hard to spot a 2-half delay.
3. The bench-9l investigation that DCB-02 sat on was focused
   on the AIF1ADCDAT silence problem, not on TX latency.

### 2e. Where the spec went wrong

DCB-00 §3 / §5 inherited the "guard entry" placement from
the original DCB-00 draft (rlvgl `b4dfc72`, 2026-05-02
upstream merge). DCB-00a's tightening pass extended the
justification text but didn't audit the placement against the
pre-DCB pattern. The transition-table cell `clean for Read on
inactive half/bank` reads naturally as "the clean
*precedes* the CPU writes" — which is exactly the off-by-one.

## 3. Options

### Option A — Move cache op to guard release for `Read`; keep entry for `Write`

For `HalfGuard<Read>` / `BankGuard<Read>`:

- Construction: no cache op (CPU is about to write; no state
  needs publishing yet).
- `release(ctx, current_target)`: emits `clean` over the
  guarded half/bank's extent, then performs the existing
  CT-bit / NDTR live-recheck.

For `HalfGuard<Write>` / `BankGuard<Write>` (CPU is consumer
draining engine-written data):

- Construction: `invalidate` (pre-DCB pattern: invalidate
  before reading; DMA writes during the read window only
  ever touch the *active* bank, not this one).
- `release(ctx, current_target)`: no cache op (CPU has only
  read; no dirty lines to publish).

The `release` method gains a `ctx` parameter on both
directions for symmetry; the `Write` direction's `release`
just uses `ctx` for the live-recheck (no extra cache op).

**Pros**: matches the pre-DCB pattern verbatim; no
steady-state latency penalty; the `release` method becomes a
true "publish" checkpoint analogous to `LtdcScan::present`.

**Cons**: `release` signature change is breaking — every
consumer of DCB-02 / DCB-02-R / DCB-02b passes a `ctx` to
release now. The DCB-01b API ships in `f263ad5` but only one
crate consumes it externally (disco-analyzer at `c117a20` /
`1df2b9c`); the in-tree consumer (`audio_player.rs`) doesn't
use BankGuard at all (uses the cache-op-only refill pattern
DCB-02b documented).

### Option B — Document the latency, defer the fix

Keep the entry-clean placement; add an INV-D17 noting the
off-by-one latency and instructing engine drivers to
pre-arm one extra silence half / bank to mask it. INV-D7's
"DMA crossed into the half" check still holds.

**Pros**: zero API change; existing retrofits unaffected.

**Cons**: imposes a per-consumer workaround; pollutes the
typestate's contract with "you must pre-fill more than one
half" semantics that aren't in the type system; doesn't
restore parity with the pre-DCB pattern that the §10 row
prescribed as "Replaces".

### Option C — Emit the clean at BOTH entry and release

Keep the entry clean (preserves the existing transition
table), add a release-time clean (publishes new writes
promptly). 2× cache op per guard scope.

**Pros**: backward-compatible-with-spec (entry clean still
fires); fixes the latency.

**Cons**: 2× cache op overhead (an extra ~512-cache-line
clean per audio half-period — measurable on the M7);
muddles the contract ("which clean does the work?" is now
implementation-defined).

## 4. Recommendation

**Option A** (move clean to release for `Read`; keep entry for
`Write`).

Justification:

- **Restores pre-DCB semantics.** The pre-DCB SAI1 TX shape
  cleaned after writes; that's the proven working pattern.
  Option A makes the typestate match the proven shape rather
  than approximating it.
- **Symmetric `release(ctx, current_target)` shape.** Both
  directions get a uniform release signature — `ctx` for the
  cache op (Read) or just for the live-recheck plumbing
  (Write); `current_target` for INV-D7 / INV-D15.
- **Minimum API blast radius.** The two consumer call sites
  in disco-analyzer (`c117a20`, `1df2b9c`) update mechanically
  — `release(active_now)` → `release(&mut ctx, active_now)`.
  No structural change to the retrofits.
- **`DcaBuf<DeviceWritePending>` and `LtdcScan::present` align.**
  Both already enforce "publish at the explicit checkpoint"
  semantics (DeviceWritePending: invalidate at lend, no exit op;
  LtdcScan: clean+barrier at present). Moving Read's clean to
  release brings the continuous-Read family into line.

Option B is rejected on type-system-cleanliness grounds (the
pre-fill workaround leaks implementation detail into every
consumer's contract). Option C is rejected on overhead — an
extra clean per half-period is measurable on the M7 and the
"which clean is load-bearing" ambiguity is the kind of
soft-fork we've been trying to avoid (cf. DCB-02-A Option C
rejection).

## 5. Proposed amendments to DCB-00 + DCB-01b

If Option A is ratified:

### §5 transition table updates

Two rows change for `DcaBuf` (and the parallel rows for
`DcaDoubleBuf`):

- `DeviceActiveCirc<DIR>` → `DeviceActiveCirc<DIR>` (HalfGuard
  scope): change "per-DIR op on the *inactive half only*:
  `clean` for `Read`, `invalidate` for `Write`" to
  **"per-DIR op on the *inactive half only* at the boundary
  *appropriate to the direction*: `Read` direction emits no op
  at construction (CPU is about to write) and `clean` at
  `release` (publishes CPU writes); `Write` direction emits
  `invalidate` at construction (drains stale cache lines) and
  no op at `release` (CPU has only read). Both directions'
  `release` performs the INV-D7 live-recheck."**
- The parallel `DcaDoubleBuf` row (BankGuard scope) gets the
  same edit, substituting BankGuard / INV-D15.

### §6 INV-D7 / INV-D15 wording

Both remain valid (the live-recheck on `release` is
unchanged). Add a clarifying sentence noting that `release`
now also performs the cache op for the `Read` direction.

### DCB-01b API change

`HalfGuard::release` and `BankGuard::release`:

```rust
// Before:
pub fn release(self, current: Half) -> Result<(), HalfGuardOverrun>

// After:
pub fn release<C: DcaCache>(
    self,
    ctx: &mut DcaCacheCtx<'_, C>,
    current: Half,
) -> Result<(), HalfGuardOverrun>
```

For `Read` direction the impl emits the cache op before the
overrun check; for `Write` direction the impl just does the
overrun check (preserves existing behaviour).

### §15 ratification entry

Standard sub-letter resolution shape; lists the §5 edits, the
DCB-01b API change, and the consumer-update obligation
(disco-analyzer DCB-02 + DCB-02-R; in-tree DCB-02b doesn't use
the guard pattern so no change there).

## 6. Implementation plan summary (informative)

After DCB-00 §15 ratifies the amendment (call it DCB-00e):

- **DCB-01d** — Update `HalfGuard::release` and
  `BankGuard::release` signatures + impls. Emit the cache op
  for `Read` direction at release time; preserve `Write`
  direction at entry. Update unit tests (the
  `circ_read_half_guard_emits_clean_per_half` test asserts
  clean-at-construction; flip to clean-at-release).
- **DCB-02-r2 / DCB-02-R-r2** — Update the disco-analyzer
  consumer call sites to pass `&mut ctx` to `release`. Two
  files, two call sites total. Mechanical change.

## 7. Change log

- **2026-05-02 — Drafted.** Surfaced during post-DCB-04
  cleanup pass when re-reading the `HalfGuard` / `BankGuard`
  cache-op placement against the pre-DCB SAI1 TX pattern in
  `analyzer-cm7/src/main.rs`. The off-by-one is a
  steady-state latency regression (~10.67 ms at 48 kHz / SAI1
  half-period), not a "bees" repeat. Recommendation: Option A
  (move clean to release for `Read`; keep entry for `Write`).
  Awaiting owner ratification via a DCB-00 §15 amendment.
