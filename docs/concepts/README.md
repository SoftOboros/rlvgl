# rlvgl-platform — Concepts (cross-cutting design lineage)

This directory holds cross-cutting **design concepts** for the
`rlvgl-platform` crate. It is the home of platform-discipline initiatives
that span multiple subsystems (DMA2D, LTDC, SAI, SDMMC, USB, …) and
that need a ratified vocabulary + frozen invariants before code lands.

It is *not* a port guide. Per-port bring-up narrative continues to live
under `docs/disco-platform-guide/`, `docs/beaglebone-black/`,
`docs/disco-zephyr-guide/`, etc. Those guides describe how a single
target boots and behaves; the docs in this directory describe contracts
that any target's code MUST satisfy.

## Why this directory exists

The CLAUDE.md "Spec-Before-Code Planning Discipline" section already
governs initiative families like DISCO-NN, BBB-NN, CREATOR-NN,
CHIPS-VENDOR-NN. Each of those families is *port-shaped*: a sequence
of chapters describing one tree's bring-up.

Some platform contracts are not port-shaped. The Register-Mashing
Discipline (CLAUDE.md §"Register-Mashing Discipline") is the canonical
example: typed framebuffer ownership, `InFlight<'dma, T>`, the three
address domains, `IsrChannel<T,N>`. It applies to every target. It
ratifies a contract on the platform crate itself, not a port narrative.

`docs/concepts/` is the home for that second class of initiative. Each
family inside this directory follows the §0–§15 phase-document shape
established in CLAUDE.md and ratified in DAA-00 (the
disco-analyzer subrepo's first concepts doc).

## Active initiatives

- **DCB** — *DMA Cacheable Buffers*. RAII typestate for DMA buffers
  in cacheable RAM. Extends the existing `InFlight<'dma, T>` ownership
  rule (Register-Mashing Discipline rule #3) with automatic D-cache
  clean / invalidate at the typestate transitions, so application code
  cannot forget cache maintenance and cannot misorder it. First user is
  the SAI1 line-in/line-out path on the disco-analyzer subrepo. Future
  users: DMA2D destination buffers, SDMMC R/W buffers, USB endpoint
  buffers, LTDC scanout (or MPU non-cacheable carve-out, per DCB-00
  §10).

  - [DCB-00-CONCEPTS.md](DCB-00-CONCEPTS.md) — foundational
    vocabulary, frozen typestate, invariants, source-of-truth map.
    **Ratified 2026-05-02 (§15); DCB-01 unblocked.**
  - [DCB-RETROSPECTIVE.md](DCB-RETROSPECTIVE.md) — initiative-
    completion retrospective (2026-05-03). Captures
    divergences against the original DCB-00 spec, refactor
    inflection points, portable mitigation patterns,
    deferred-work reclassification (Safe / Coupled /
    Abandoned), and forward constraints for future
    register-mashing-discipline-scoped initiatives.
    Provenance hooks link each entry to commit hashes, doc
    sections, and datasheet references for outcome → issue
    → fix → evidence traversal. Convention documented in
    `CLAUDE.md` "Spec-Before-Code Planning Discipline →
    Initiative retrospective".
  - [DCB-02-A.md](DCB-02-A.md) — sub-letter analysis surfaced
    during DCB-02 TX retrofit. Proposed Option A
    (`DeviceActiveDoubleBuf<DIR>` typestate) for DMA double-buffer-
    mode consumers (SAI1 RX, SAI4 PDM RX, future SDMMC / USB
    streaming).
    **Resolved 2026-05-02 — Option A ratified into DCB-00
    §3/§5/§6/§10/§15. DCB-01b + DCB-02-R unblocked.**
  - [DCB-04-A.md](DCB-04-A.md) — sub-letter analysis surfaced
    during DCB-02c BASELINE shrink. The remaining `raw_dcache`
    entry is the LTDC scanout pre-clean in `freertos_entry.rs`;
    DCB-00 §10 deferred the typestate-vs-MPU decision to
    DCB-04. Proposed Option A (`DeviceLtdcScan<T, N>` typestate)
    over Option B (MPU non-cacheable carve-out) on bench-
    behaviour-preservation and portability grounds.
    **Resolved 2026-05-02 — Option A ratified into DCB-00
    §3/§5/§6/§10/§14/§15. DCB-01c + DCB-04 unblocked.**
  - [DCB-01b-A.md](DCB-01b-A.md) — sub-letter analysis on
    the cache-op placement for the `Read` direction of
    `HalfGuard` / `BankGuard`. The DCB-01b shipped pattern
    cleans at *guard entry*; the pre-DCB SAI1 TX pattern
    cleans *after* CPU writes. Steady-state effect: audio is
    correct but ~10.67 ms (2 half-periods) late vs the
    pre-DCB shape — a latency regression rather than a "bees"
    repeat. Proposed Option A (move clean to `release` for
    `Read`; keep entry for `Write`); requires a small
    DCB-01b API change (`release` gains `&mut DcaCacheCtx`)
    and mechanical updates to the two disco-analyzer
    consumer sites.
    **Resolved 2026-05-03 — Option A ratified into DCB-00
    §5/§6/§15. DCB-01d unblocked.**
  - [DCB-02b-A.md](DCB-02b-A.md) — sub-letter analysis on
    the residual raw-pointer write path in
    `audio_player::PollResult::NeedRefill`. DCB-02b made the
    cache op type-system-tracked but kept the legacy raw
    `*mut u8` for PCM byte writes. Proposed Option A
    (callback-based `poll_refill<F>` replacing `poll()` +
    `refill_done(pcm)`; closure scope = bank-guard scope;
    no self-referential token complexity); single in-tree
    consumer (disco bare-metal binary) updates mechanically.
    **Resolved 2026-05-03 — Option A ratified into DCB-00
    §10/§15. DCB-02b-A2 unblocked.**
  - [DCB-04-B.md](DCB-04-B.md) — sub-letter analysis on the
    full `LtdcScan<u8, FB_BYTES>` typestate refactor for
    `freertos_entry.rs`'s FRONT_FB atomic-swap pattern (and
    the bare-metal `Scanout::swap` parallel). DCB-04 cleared
    the `raw_dcache` BASELINE entry via trait-dispatch
    (`DcaCacheCtx::cache.clean + barrier`); the §10 row
    prescribed pushing the FB ownership through the
    `LtdcScan` typestate. The atomic-swap pattern doesn't
    fit `&'static mut DcaBuf`'s exclusive-borrow rule
    without rearchitecting render/present task ownership.
    Proposed Option C (close-with-deferral, mirroring
    DCB-03-A and DCB-02c-A): the `LtdcScan` typestate is
    preserved in-tree (DCB-01c) for future ports to adopt;
    reopen DCB-04-B-2 with a named first user.
    **Resolved 2026-05-03 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers. DCB
    initiative reaches natural software-side completion.**
  - [DCB-02c-A.md](DCB-02c-A.md) — sub-letter analysis on the
    §10-prescribed `DcaBuf` push through the
    `rlvgl_core::fs::BlockDevice` trait surface. DCB-02c
    routed the SCB cache calls through `DcaCacheCtx`
    (clearing the `raw_dcache` BASELINE) but kept the
    caller-supplied `&mut [u8]` trait shape. The §10 row
    prescribed pushing `DcaBuf<u8, BLOCK_BYTES>` into the
    trait. Proposed Option C (close-with-deferral analogous
    to DCB-03-A): no current consumer needs the typestate at
    the trait surface; the embedded-sdmmc adapter is
    third-party so can't take DcaBuf anyway; future
    non-Write-Through consumers route through `DcaCacheCtx`
    or reopen DCB-02c-B with a named first user.
    **Resolved 2026-05-03 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers; the
    initiative's optional-cleanup track ends here.**
  - [DCB-03-A.md](DCB-03-A.md) — sub-letter analysis on the
    DMA2D destination retrofit. Documented that DMA2D
    destinations on the disco target all live in MPU-Write-
    Through SDRAM, so the §10-prescribed cache discipline
    emits no-op runtime ops; the §10 row's value is forward-
    looking design hygiene rather than a fix for an observed
    bug. Proposed Option C (close DCB-03 as deferred; future
    non-Write-Through consumers route through `DcaCacheCtx`
    per the DCB-02c / DCB-04 pattern, or reopen DCB-03-B with
    a named first user).
    **Resolved 2026-05-02 — Option C ratified into DCB-00
    §10/§14/§15 with normative Reopen triggers; the
    initiative's BASELINE-shrink track ends at DCB-04.**

(Future concepts initiatives — for example: cross-core IPC primitives,
non-cacheable MPU region management, SDMMC ownership lifecycle — land
as additional families here when they cross the ~3-phase / ~3-subsystem
threshold.)

## Conformance

A conforming `rlvgl-platform` consumer MUST satisfy the acceptance
gates of every active initiative whose surface it touches. For DCB
specifically: any new DMA buffer added to a cacheable RAM region (D1
SRAM, D2 SRAM, AXI SRAM) MUST go through the DCB typestate API; manual
`clean_dcache_by_*` / `invalidate_dcache_by_*` calls in new code are a
discipline violation unless explicitly carved out per DCB-00 §11.

Existing call sites (`audio_player.rs`, `stm32h747i_disco_sd.rs`,
`sd_emmc_adapter.rs`) are grandfathered until DCB-02 / DCB-03 retrofits
land — see DCB-00 §10.

## Vocabulary discipline

Per CLAUDE.md normative-keyword convention: **MUST**, **MUST NOT**,
**SHALL**, **SHOULD**, **SHOULD NOT**, **MAY**, **RECOMMENDED** in
docs under this directory follow RFC 2119 / RFC 8174. Plain narrative
without capitalised keywords is informative.

## Sub-letter doc convention

Per the established pattern (DAA-01-A, DAA-01-B, …): a `<INIT>-NN-X`
doc is a tradeoff analysis surfaced during phase NN that needs its own
ratified resolution before phase NN proceeds. Sub-letter docs are
scoped to one decision, transient (resolution folds into the parent
phase doc's §15), and do not introduce new frozen invariants of their
own.

## Execution discipline

Once a concepts doc here is ratified (dated §15 entry), execution PRs
cite the phase as `<INIT>NN[a-z]:` in the commit subject (e.g.
`DCB01a:`, `DCB02:`). Touching a frozen typestate value or invariant
requires a §15 amendment **first**, in a separate PR. No behaviour PR
rides on an unamended invariant.
