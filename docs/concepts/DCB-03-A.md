# DCB-03-A — DMA2D destination cache discipline

**Status:** **Resolved 2026-05-02 — Option C ratified
(closure-with-deferral).** Folded into DCB-00 §10 / §14 / §15
via the 2026-05-02 closure-with-deferral entry. DCB-03 is
closed; **DCB-03-B reopen** is triggered by any of: (a) a new
DMA2D destination in non-Write-Through cacheable RAM; (b) a port
without H7-style Write-Through SDRAM defaults adopting DMA2D;
(c) a change to the disco's SDRAM Write-Through config. This
file is preserved as historical analysis only; no behaviour PRs
reference it directly.

## 1. Purpose

DCB-00 §10 sketched the DMA2D destination retrofit as:

> A DMA2D destination becomes a `DcaBuf` *containing* the
> framebuffer pixels, with `BackBuffer` as the format/geometry
> view. The DMA2D `start_*_typed` API takes
> `DcaBuf<u8, FB_BYTES, CpuOwned>` and a `BorrowedForDma<'dma,
> BackBuffer<'fb>>`, and returns `InFlight<'dma, BackBuffer<'fb>>`
> whose Drop releases the cache typestate back to `CpuOwned`.

This doc:

- inventories DMA2D's current consumer set in the rlvgl tree,
- documents what cache discipline DMA2D destinations *actually
  need* on the disco target vs other plausible deployments,
- enumerates the option set the §10 row's prescription leaves
  open,
- recommends one,
- proposes the §15 amendment surface (if any).

## 2. Problem statement

### 2a. DMA2D's role on the disco target

`platform/src/dma2d.rs::Dma2dBlitter` drives the H7 DMA2D engine
in three submission modes: register-to-memory fill (R2M),
memory-to-memory blit (M2M), and the A8 alpha-blend variants.
The current typed API
(`Dma2dBlitter::start_fill_typed` / `start_blit_typed`) already
enforces aliasing discipline via `BorrowedForDma<'_,
BackBuffer<'_>>` → `InFlight<'_, BackBuffer<'_>>` (Register-
Mashing Discipline rule #3, ratified before DCB and untouched
by it). Cache state is the orthogonal concern DCB-03 would
encode.

### 2b. DMA2D destinations are SDRAM Write-Through (today)

Every DMA2D destination buffer in the current tree is a
framebuffer in SDRAM:

- The disco bare-metal + freertos paths use the front / back
  framebuffers at `0xD180_0000` and `0xD200_0000` (or
  `swap`-managed equivalents).
- The bare-metal `Scanout` swap pair lives in the same SDRAM
  region.
- Star-crawl + effect overlay paths render into the same SDRAM
  framebuffers.

Per `platform/src/stm32h747i_disco.rs:1322`
(`configure_mpu_sdram_writethrough`), the SDRAM range
`0xD000_0000..0xD200_0000` is MPU-configured **Write-Through
Non-Shareable**: TEX=0, C=1, B=0, S=0. Under Write-Through, CPU
writes go to both cache and SDRAM in real time; DMA2D writes
hit SDRAM directly via AXI without touching the M7 D-cache.
Coherency is automatic: a DMA2D write completes (`InFlight`
acked) and the next CPU read sees the DMA2D-written data with
no cache op needed.

This means the `lend_for_write` typestate transition that the
§10 row prescribes (a `DeviceWritePending` → `Cpu` lifecycle
with entry-side `invalidate_dcache_by_address`) **emits a cache
op that is functionally a no-op on the disco target**. The
type-system contract still has value (it documents which
buffers the engine writes to, prevents a second concurrent
submission, etc.), but the *runtime cache discipline* part of
DCB is a no-op for the existing consumer set.

### 2c. Where DMA2D cache discipline *would* matter

Hypothetical / future consumers where DCB-03's typestate
discipline would be runtime-load-bearing:

- **M2M scratch buffers in cacheable RAM that isn't
  Write-Through.** Examples: a tile-cache layer in D1 SRAM
  (which inherits the default ATT / Normal Cacheable attributes),
  a glyph-atlas in AXI SRAM, a thumbnail cache. None exist in
  the current rlvgl tree.
- **Future ports without Write-Through SDRAM defaults.** A
  port to a different STM32 family or a different MCU vendor
  that doesn't pre-MPU SDRAM as Write-Through would expose the
  cache-coherency hazard for DMA2D destinations. None of the
  named `rlvgl-creator` targets (esp32-c3, esp32-p4,
  beetle-esp32c3, beaglebone-black) use the H7 DMA2D path —
  they have their own display engines.
- **Future H7 modes that disable Write-Through.** If a future
  bring-up needs Write-Back SDRAM (lower bus-utilization
  pressure for non-display workloads), DMA2D destinations
  there would need explicit invalidate.

Practical reading: zero current bench scenarios benefit from a
runtime cache op at the DMA2D boundary. The §10 row's
prescription is forward-looking design hygiene rather than a
fix for an observed bug — distinct from DCB-02 (which fixed
"loud bees"), DCB-02b/c (which closed real cache-coherency call
sites), and DCB-04 (which cleared the last `raw_dcache` BASELINE
entry).

### 2d. Non-trivial cost: the FrameBuffer rearchitect

The §10 row's preferred shape — "`DcaBuf` *contains* the
framebuffer pixels, with `BackBuffer` as the format/geometry
view" — is materially invasive on `platform/src/hwcore/surface.rs`:

- `FrameBuffer` (the storage type) currently holds a `PhysAddr`
  + geometry. Becoming "a `DcaBuf` containing pixels" means
  either composing `FrameBuffer { dca: DcaBuf<u8, FB_BYTES>,
  geometry }` (with `FB_BYTES` const-generic propagated through
  every consumer) or refactoring to a runtime-sized DCA
  primitive (which doesn't exist yet — DcaBuf is const-generic).
- `Scanout::try_new(front, back)` currently takes two
  `FrameBuffer`s. With DcaBuf-composition the two banks must
  match on `FB_BYTES` const-generic — feasible but every
  consumer that constructs framebuffers needs to pick a
  uniform size at compile time.
- `BackBuffer::cpu_slice()` (the `unsafe fn`) becomes the
  DcaBuf's CPU-typestate accessor; its lifetime semantics
  change to flow through DcaBuf's typestate transitions rather
  than through ad-hoc `&mut FrameBuffer`.

This isn't impossible, but it's ~300 lines of structural change
across `surface.rs` + `dma2d.rs` + every consumer
(`dma2d_draw.rs`, `freertos_entry.rs`, `effect.rs`,
`star_crawl.rs`, the disco-sim host integration test, the
`MockBlitter` host-side stub). For zero observable runtime
behaviour change on the only platform that currently uses
DMA2D.

## 3. Options

### Option A — Full §10 prescription: FrameBuffer composes DcaBuf

Implement the §10 row exactly as written. `FrameBuffer<const
FB_BYTES: usize>` becomes generic; the typed DMA2D submission
methods take `DcaBuf<u8, FB_BYTES, CpuOwned>` parameters
threaded through every consumer.

**Pros**: spec-fidelity; every DMA2D destination is type-
system-tracked through DCB; INV-D9 ("new DMA buffers MUST use
DcaBuf") fully realized for DMA2D.

**Cons**: ~300-line cross-cutting refactor; zero runtime
benefit on the only current consumer (Write-Through SDRAM
makes the ops no-ops); every consumer-facing API gains a
`FB_BYTES` const-generic that ripples through the code; the
sim/mock host paths need parallel updates. Bench-flash
validation overhead is non-trivial because DMA2D is in the
hot rendering path; any regression risk is real even though
the cache ops are functionally no-ops.

### Option B — Parallel typestate-aware submission methods

Add `start_fill_typed_dca` / `start_blit_typed_dca` alongside
the existing `start_fill_typed` / `start_blit_typed`. The new
methods take an additional `&mut DcaCacheCtx<'_, C>` and a
`DcaBuf<u8, FB_BYTES, CpuOwned>` typestate-handle (the buffer's
content is the same SDRAM range the existing `BackBuffer` holds
geometry for, but the DcaBuf wraps a separate `&'static mut`
view of the same memory through `from_addrs`-style construction).

The existing methods stay for current consumers; new code
opts in. No FrameBuffer rearchitect.

**Pros**: minimum-invasion; existing DMA2D consumers unaffected;
new-feature opt-in path; matches the §10 intent at the API
boundary even if not at the storage type.

**Cons**: two parallel paths is a soft fork; aliasing rules
get hairy if a consumer holds *both* a `BackBuffer` and a
`DcaBuf` pointing at the same SDRAM region (the typed
aliasing would need careful documentation); the FrameBuffer
type stays unchanged so DcaBuf construction has to happen
externally and be threaded through with care.

### Option C — Defer DCB-03; close as "no current consumer"

Close DCB-03 as "no observed runtime hazard on current
consumer set; reopen when a non-Write-Through DMA2D
destination materializes." Document the closure in a §15
amendment. The DcaCacheCtx + DcaCache trait already exists
(DCB-01); if a future M2M scratch path needs cache discipline
it can route ops through that without DcaBuf typestate.

**Pros**: zero refactor cost; keeps the initiative scope
honest about value delivered; DCB-00 §10 row gets a "deferred /
not currently load-bearing" note rather than an unreachable
prescription. The DcaCache trait dispatch (Option D's spirit
without the ceremony) is already available for future
ad-hoc consumers via the path DCB-02c / DCB-04 used.

**Cons**: feels like backing off from the §10 vision. INV-D9
has a documented exemption for DMA2D destinations until
further notice.

### Option D — Soft retrofit: DcaCacheCtx dispatch only, no buffer typestate

Mirror the DCB-02c / DCB-04 pattern: add a private
`fn dma2d_dst_invalidate(addr, len)` helper to `dma2d.rs`
that calls `DcaCacheCtx::cache.invalidate(addr, len)` (and
optionally `barrier()`), invoked by `start_fill_typed` /
`start_blit_typed` *before* arming the engine. No DcaBuf,
no §10-prescribed typestate change.

**Pros**: smallest possible code change; preserves the
existing API surface; gives DCB-equivalent containment for
the cache op (it lives in dca.rs's typed wrapper).

**Cons**: emits a runtime cache op (the `invalidate`) that
is a no-op on Write-Through SDRAM today; if a non-Write-
Through buffer ever becomes a destination, the op becomes
load-bearing — but without the typestate, callers have no
type-system way to know which buffers need it. The
runtime-overhead-without-runtime-benefit shape is awkward.

## 4. Recommendation

**Option C** (defer DCB-03; close as "no current consumer").

Justification:

- **Honest about value delivered.** DCB-02 fixed real bench
  bugs (SAI1 "loud bees", SD adapter cache races). DCB-04
  cleared the last `raw_dcache` BASELINE entry. DCB-03 has
  no analogous load-bearing target on the current MCU + MPU
  configuration; it's a forward-looking refactor for
  hypothetical consumers. Forcing it through now spends
  refactor budget and bench-validation risk against
  imagined future requirements.
- **The DcaCache trait route is already open.** If a future
  M2M scratch path or non-Write-Through port needs cache
  discipline at the DMA2D boundary, the consumer can route
  through `DcaCacheCtx::cache.invalidate()` exactly the way
  DCB-02c retrofitted SD adapters and DCB-04 retrofitted
  the LTDC scanout path. The infrastructure is in place; the
  consumer-specific retrofit ratifies when a real consumer
  materializes.
- **Preserves INV-D9 in spirit.** INV-D9's text is "new DMA
  buffers MUST use DcaBuf". DMA2D destinations on the disco
  ARE NOT new — they're the existing `FrameBuffer` /
  `BackBuffer` chain that pre-dates DCB. They're
  grandfathered by the same "existing primitive,
  pre-DCB" reading that grandfathers the SDRAM region itself.
  A future consumer that allocates a new DMA2D destination
  in non-Write-Through cacheable RAM will need DcaBuf per
  INV-D9; that retrofit ratifies then.
- **Minimum-invasion preserves DMA2D as a stable surface.**
  The disco firmware's hot rendering path is bench-tuned;
  re-validating after a 300-line FrameBuffer rearchitect
  for zero runtime benefit is the kind of cost that's hard
  to justify when the cleanup is *purely* spec-driven.

Option A is rejected on cost-vs-benefit (no runtime value;
substantial cross-cutting refactor; bench-validation
overhead). Option B is rejected because two parallel typed
paths is a soft fork that confuses future readers. Option D
is rejected because emitting a runtime cache op without the
typestate's protective contract is the worst of both worlds —
overhead without the soundness guarantee.

## 5. Proposed amendments to DCB-00

If Option C is ratified:

### §10 row (DMA2D destination) — clarification

Replace the "Concrete refactor lands in DCB-03" close-out
sentence with:

> DCB-03 is closed (DCB-03-A 2026-05-02): no current consumer
> requires a runtime cache op at the DMA2D destination
> boundary (SDRAM is MPU-configured Write-Through Non-
> Shareable on the disco target;
> `platform/src/stm32h747i_disco.rs:1322`). The §10 prescription
> remains the correct shape *if and when* a non-Write-Through
> DMA2D destination materializes; until then DMA2D destinations
> are grandfathered alongside the `FrameBuffer` / `BackBuffer`
> chain that pre-dates DCB. Future consumers in cacheable RAM
> that isn't Write-Through (M2M scratch buffers in D1 SRAM /
> AXI SRAM, future ports without H7-style MPU defaults) MUST
> route through `DcaCacheCtx` per the DCB-02c / DCB-04 pattern,
> or land a DCB-03-B amendment that revisits the typestate
> shape with a named first user.

### §14 unblocks — DCB-03 entry update

Mark DCB-03 as **Closed (deferred)** with a back-pointer to
DCB-03-A. The DCB-03-B reopening note is preserved for future
amendments.

### §12 acceptance gates — no change

(c) — BASELINE empty + STRICT — already cleared by DCB-04.
DCB-03 was never on a §12 acceptance critical path.

### §15 ratification entry (proposed wording)

> **2026-MM-DD — DCB-03-A resolution: Option C (closure-with-
> deferral).** DCB-03 is closed without retrofit. The DMA2D
> destination cache discipline that §10 prescribed is not
> currently load-bearing because every DMA2D destination on
> the disco target lives in MPU-Write-Through SDRAM. Future
> non-Write-Through DMA2D destinations route through
> `DcaCacheCtx` per the DCB-02c / DCB-04 trait-dispatch
> pattern; if a typestate-shaped retrofit ever becomes
> warranted, DCB-03-B reopens the analysis with a named first
> user. INV-D9 is read as "new DMA buffers in *new* DMA paths
> MUST use DcaBuf" — DMA2D's existing `FrameBuffer` /
> `BackBuffer` chain is grandfathered. No changes to the
> typestate set, layout invariants, engine submission contract,
> or scanner rule are required by this resolution.

## 6. Implementation plan summary (informative)

If Option C is ratified, this commit and one §15 amendment to
DCB-00 are the entire DCB-03 deliverable. No code changes; no
behaviour PRs follow. The DCB initiative's BASELINE-shrink
track ends at DCB-04.

## 7. Change log

- **2026-05-02 — Drafted.** Surfaced when DCB-04 cleared the
  last `raw_dcache` BASELINE entry. Recommendation: Option C
  (close DCB-03 as deferred; no current consumer needs a
  runtime cache op at the DMA2D destination boundary on the
  Write-Through SDRAM disco target). Awaiting owner
  ratification via a DCB-00 §15 amendment.
- **2026-05-02 — Resolved.** Option C ratified by owner go-ahead
  with explicit reopen path. Resolution folded into DCB-00 §10
  (DMA2D row reworded to "closed-with-deferral" with a
  normative **Reopen triggers** clause), §14 (DCB-03 marked
  Closed (deferred); DCB-03-B reopen path preserved with the
  same trigger set), and §15 (closure-with-deferral
  ratification entry; not a Standards Action change to the
  typestate set or invariants — only the §10 reconciliation
  prescription is amended). The initiative's BASELINE-shrink
  track formally ends at DCB-04. This sub-letter is now
  historical record only.
