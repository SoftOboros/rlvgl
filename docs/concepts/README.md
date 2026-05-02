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
