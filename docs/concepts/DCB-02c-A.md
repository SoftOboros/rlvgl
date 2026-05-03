# DCB-02c-A — DcaBuf push through the BlockDevice trait surface

**Status:** Drafted 2026-05-03. Sub-letter analysis surfaced
during the post-DCB-04 sweep for unblocked outstanding items.
DCB-02c (rlvgl `9d90d81`) retrofitted the two SD adapters
(`stm32h747i_disco_sd.rs`, `sd_emmc_adapter.rs`) onto
`DcaCacheCtx` so the SCB cache calls live inside `hwcore::dca`'s
whitelisted module — clearing the `raw_dcache` BASELINE for
those files. DCB-02c did **not** change the trait surface; the
caller-supplied `&mut [u8]` block buffers stayed unchanged.
DCB-00 §10's SD reconciliation row prescribed the more
aggressive shape: "SDMMC R/W buffers become `DcaBuf<u8,
BLOCK_BYTES>`. The W path lends `DeviceReadPending`; the R
path lends `DeviceWritePending`." This sub-letter takes
inventory of what that retrofit actually buys, surveys the
option set the §10 row leaves open, and recommends a path. Per
the parent CLAUDE.md "Sub-letter doc convention" the
resolution folds into DCB-00 §10 / §14 / §15 (or ratifies a
closure-with-deferral) as a Standards Action amendment.

## 1. Purpose

Decide whether to push `DcaBuf<u8, BLOCK_BYTES>` through the
`rlvgl_core::fs::BlockDevice` trait surface (and inferentially
through the `embedded_sdmmc::BlockDevice` adapter, where
possible) — or accept the DCB-02c trait-dispatch shape as the
stable end state for the SD path.

## 2. Problem statement

### 2a. Current state (post DCB-02c)

```rust
// rlvgl_core::fs::BlockDevice
trait BlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError>;
    fn write_blocks(&mut self, lba: u64, buf: &[u8]) -> Result<(), FsError>;
    // ...
}

// platform/src/stm32h747i_disco_sd.rs (DCB-02c retrofit)
impl BlockDevice for DiscoSdBlockDevice {
    fn read_blocks(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), FsError> {
        Self::invalidate(buf);  // ← DcaCacheCtx::cache.invalidate(addr, len)
        self.sdmmc.read_blocks(lba as u32, buf)?;
        Self::invalidate(buf);  // ← post-DMA invalidate
        Ok(())
    }
    // ...
}
```

The `invalidate` / `clean` helpers route through `DcaCacheCtx`
+ `DcaCache::invalidate` / `clean` — which dispatch to
`hwcore::dca`'s SCB impl. The SCB calls don't appear in the
SD adapter files, so the `raw_dcache` BASELINE for those files
is empty. Caller-supplied `&mut [u8]` is alignment-tolerant via
the address-form SCB ops (round down to cache-line boundary
internally).

### 2b. What the §10 row prescribed

```rust
// Hypothetical post-DCB-02c-A shape
trait BlockDevice {
    fn read_blocks<const N: usize>(
        &mut self,
        lba: u64,
        buf: &mut DcaBuf<u8, N>,  // typestate-tracked
    ) -> Result<(), FsError>;
    // ...
}
```

The W path lends a `DcaBuf<DeviceReadPending>`; R path lends
`DcaBuf<DeviceWritePending>`. The trait gains a const generic
`N` that varies per call. The cache-line-padded `DcaBuf`
storage means the unaligned-slice rationale at the original
`sd_emmc_adapter.rs:39` is no longer needed; INV-D2 gives
alignment guarantees at the type system level.

### 2c. Cost inventory

The §10 prescription requires:

- **`rlvgl_core::fs::BlockDevice` trait change.** Const-generic
  trait method (`fn read_blocks<const N: usize>`). Every
  implementor must support arbitrary `N` or pick a fixed one.
- **Third-party trait can't change.** The
  `embedded_sdmmc::BlockDevice` trait
  (`SdMmcBlockDev`'s impl in `sd_emmc_adapter.rs`) is owned by
  an upstream crate. We can't add `DcaBuf` parameters to its
  methods. Either DCB-02c-A *only* applies to
  `rlvgl_core::fs::BlockDevice` (leaving the embedded-sdmmc
  path on `DcaCacheCtx`-dispatch), or the
  embedded-sdmmc adapter does an internal `DcaBuf` ↔ `&[u8]`
  copy at the boundary — adding a memcpy per block.
- **Every consumer of `rlvgl_core::fs::BlockDevice` ripples.**
  The FAT layer (`rlvgl_core::fs::fatfs` /
  `rlvgl_platform::fatfs_nostd`), the playit asset loader,
  any future filesystem layer — all need to allocate
  `DcaBuf<u8, N>` instead of stack/heap `[u8; 512]` arrays.
  For dynamic-block filesystems where N varies (FAT clusters
  can be 512 / 1024 / 2048 / 4096 / etc. bytes), each consumer
  picks one and the trait infrastructure doesn't generalise
  cleanly across them.
- **No observed runtime bug.** SDRAM Write-Through MPU on the
  disco target makes cache discipline largely irrelevant for
  SDRAM-resident block buffers. The DCB-02c trait-dispatch is
  already adequate for the address-form alignment-tolerant
  cache ops.

### 2d. Where DCB-02c-A *would* matter

Hypothetical / future consumers where the typestate
discipline would be runtime-load-bearing:

- **Block buffers in non-Write-Through cacheable RAM.**
  Examples: a tile-cache LRU layer in D1 SRAM that reads SD
  blocks into D1 (which is Write-Back by default); a
  read-ahead buffer ring backed by AXI SRAM. None exist in
  the current rlvgl tree.
- **Future ports without H7-style Write-Through SDRAM
  defaults.** Same argument as DCB-03-A §2c.
- **Strict alignment requirements.** If a future SDMMC HAL
  layer requires 32-byte-aligned buffers (some Renesas /
  Microchip SDMMC blocks do), the type-system-enforced
  `DcaBuf` alignment would surface that as a compile-time
  contract rather than a runtime check.

Practical reading: zero current bench scenarios benefit from
the typestate-tracked block buffer. The §10 prescription is
forward-looking design hygiene, just like DCB-03-A's DMA2D
case.

## 3. Options

### Option A — Full §10 prescription: DcaBuf in BlockDevice trait

Add `read_blocks<const N: usize>` / `write_blocks<const N:
usize>` to `rlvgl_core::fs::BlockDevice`. Update both SD
adapters to take `DcaBuf<u8, N>` typestate handles. The
`embedded_sdmmc` adapter does an internal copy at the
boundary (no choice — third-party trait).

**Pros**: spec-fidelity; type-system alignment guarantees at
the trait surface; INV-D9 fully realized for SD.

**Cons**: invasive trait change rippling through FAT and
every block-device consumer. The embedded-sdmmc adapter gets
worse (extra memcpy per block) for zero observable benefit.
Bench-validation overhead is real even though the cache ops
are no-ops on Write-Through SDRAM.

### Option B — Parallel typed methods, leave existing methods

Add `read_blocks_dca<const N: usize>` / `write_blocks_dca`
alongside the existing `read_blocks` / `write_blocks` in
`rlvgl_core::fs::BlockDevice`. Default impls in the trait
forward the typed methods to the slice methods (or vice
versa). New consumers opt into the typed shape.

**Pros**: no ripple through existing FAT consumers; gradual
migration possible.

**Cons**: two parallel paths is a soft fork; trait method
count doubles; the "default impl forwards via
`DcaBuf::as_mut_slice`" shape adds runtime overhead (an
extra slice reborrow per call) for zero benefit on
Write-Through targets.

### Option C — Defer DCB-02c-A; close as "no current consumer"

Same shape as DCB-03-A's resolution. Document closure-with-
deferral; explicit reopen triggers; existing DCB-02c trait-
dispatch shape is the stable end state for the SD path until
a real consumer materializes.

**Pros**: minimum churn; honest about value delivered (the
DCB-02c retrofit already did the cache-call containment that
the BASELINE-shrink track required); INV-D9's
forward-looking reading covers the existing pre-DCB SD
adapters as grandfathered.

**Cons**: feels like the §10 row's prescription gets
deferred *twice* (DMA2D in DCB-03, SD in DCB-02c-A) — both
on Write-Through-SDRAM-says-it-doesn't-matter grounds.

### Option D — Soft retrofit: add `DcaSlice<'a>` runtime primitive

Introduce a new `DcaSlice<'a, T, DIR>` primitive in
`hwcore::dca`: takes `&'a mut [T]` of any alignment, performs
runtime alignment / size validation against `CACHE_LINE`,
threads typestate through. Trait surface stays slice-based
but the *internal* dispatch in the SD adapter wraps the
caller's slice in a `DcaSlice` for the duration of the call.

**Pros**: doesn't break trait callers; provides typestate
discipline for runtime-sized buffers; potentially useful
beyond SD (any per-call DMA scratch buffer).

**Cons**: new API surface to ratify (DCB-NN amendment for
the `DcaSlice` primitive); the runtime alignment check
duplicates what `DcaCache::invalidate(addr, len)` already
handles via the address-form SCB op; still no observable
runtime benefit on the disco target.

## 4. Recommendation

**Option C** (defer DCB-02c-A; close as "no current consumer").

Justification:

- **Mirrors DCB-03-A's resolution.** Both DMA2D and SDMMC
  destinations live in MPU-Write-Through SDRAM on the only
  platform that currently uses them. The §10 prescription is
  forward-looking design hygiene rather than a fix for an
  observed bug. DCB-03-A closed on this reasoning; DCB-02c-A
  closes on the same reasoning.
- **DCB-02c trait-dispatch is the stable end state.** The
  cache-call containment (SCB calls live in `hwcore::dca`'s
  whitelisted module; `raw_dcache` BASELINE empty) is what
  the BASELINE-shrink track needed. The further trait-surface
  push gives nothing observable.
- **Smaller blast radius wins.** `embedded_sdmmc::BlockDevice`
  is third-party; adding internal copies just to push a
  `DcaBuf` through the boundary is overhead-without-benefit.
  Rippling FAT consumers similarly trades real refactor
  budget for imagined future requirements.
- **The trait-dispatch route is open for ad-hoc consumers.**
  If a future M2M scratch path or non-Write-Through port
  needs cache discipline at the SD boundary, it can construct
  a `DcaCacheCtx` and call `cache.invalidate(addr, len)`
  directly — no DcaBuf required.

Option A is rejected on cost-vs-benefit (no runtime value;
substantial cross-cutting refactor; the embedded-sdmmc adapter
gets *worse* in shape). Option B is rejected for soft-fork
reasons consistent with DCB-02-A's Option C rejection. Option
D is rejected on API-surface-without-runtime-benefit grounds —
adding `DcaSlice<'a, T, DIR>` is real work and the DcaCache
trait already alignment-tolerantly handles the runtime case.

## 5. Proposed amendments to DCB-00

If Option C is ratified:

### §10 row (SD adapters) — clarification

The SD reconciliation row (currently positioned between the
DMA2D row and the LTDC scanout row) gets a closure-with-
deferral rewrite analogous to the DCB-03 close-out, naming
DCB-02c-A 2026-05-03 as the resolution trigger and
documenting the same **Reopen triggers** structure DCB-03 got
in DCB-00d.

### §14 — DCB-02c entry update

Add a "DCB-02c-A 2026-05-03 closes the trait-surface push as
deferred; DCB-02c trait-dispatch is the stable end state for
the SD path; **Reopen with DCB-02c-B** when (a) a non-Write-
Through SDMMC destination is needed, (b) a port without
H7-style Write-Through SDRAM defaults adopts SDMMC, or (c) a
consumer needs strict 32-byte-aligned buffer alignment as a
compile-time guarantee" entry.

### §12 acceptance gates — no change

(c) — BASELINE empty + STRICT — already cleared by DCB-04.

### §15 ratification entry (proposed wording)

> **2026-MM-DD — DCB-02c-A resolution: Option C (closure-with-
> deferral).** DCB-02c-A (the §10-prescribed DcaBuf push
> through the BlockDevice trait surface) is closed without
> retrofit. The trait-dispatch shape adopted by DCB-02c is the
> stable end state for the SD path. Future non-Write-Through
> SDMMC destinations or strict-alignment consumers route
> through `DcaCacheCtx` per the existing trait-dispatch
> pattern; if a typestate-shaped retrofit ever becomes
> warranted, DCB-02c-B reopens the analysis with a named first
> user. INV-D9 is read forward-looking — pre-DCB SD adapters
> grandfathered alongside the `BlockDevice` trait surface that
> pre-dates DCB. No changes to typestate set, layout
> invariants, engine submission contract, or scanner rule
> required.

## 6. Implementation plan summary (informative)

If Option C is ratified, the entire DCB-02c-A deliverable is
one §15 amendment commit to DCB-00. No code changes; no
behaviour PRs follow.

## 7. Change log

- **2026-05-03 — Drafted.** Surfaced during the post-DCB-04
  cleanup pass. Recommendation: Option C (close DCB-02c-A as
  deferred; the DCB-02c trait-dispatch shape is the stable
  end state; reopen DCB-02c-B when a real consumer
  materializes). Awaiting owner ratification via a DCB-00
  §15 amendment.
