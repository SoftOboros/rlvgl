<!--
README.md - Audio subsystem doc family. Codec bring-up + SAI / DMA
audio path invariants for rlvgl-platform consumers.
-->

# rlvgl Audio Subsystem — Initiative Family

**Status:** Active. AUDIO-01 ratified 2026-05-19; chapters 02+ TBD.

## What this initiative covers

The audio path is the second-deepest hardware subsystem in rlvgl-platform
after display (LTDC/DSI). It has its own master clock domain (FLL1 inside
the codec), its own DMA controllers, its own framing rules (I²S vs TDM vs
DSP), its own bit-encoding conventions (offset-binary AIF1ADC, two's-
complement AIF1DAC), and its own latency budgets (audio-bank period
versus FFT vs render). Volume II
[Chapter 8 — Secondary Peripherals](../disco-platform-guide/08-secondary-peripherals.md)
introduces the WM8994 codec at a high level. This initiative is the
deeper walkthrough across every audio-path invariant, organised as a
multi-chapter concepts-doc family per the
[Spec-Before-Code Planning Discipline](../../CLAUDE.md#spec-before-code-planning-discipline)
in the top-level CLAUDE.md.

The family exists because audio bring-up has hit invariants that
disappear into "I tried it and it worked" PR descriptions and resurface
3-6 months later on a downstream consumer's bench. The first such
invariant (FLL1-lock-before-AIF1ADC1-serializer-arm) sits at the head of
this family by initial ratification 2026-05-19.

## Chapter list

| Ch | Path | Status | Covers |
|---|---|---|---|
| 01 | [`01-codec-bringup.md`](01-codec-bringup.md) | Ratified 2026-05-19 | WM8994 init_record path. FLL1 lock invariant + serializer arm ordering. |
| Errata | [`ERRATA.md`](ERRATA.md) | Active | Permanent log of WM8994 / audio-path issues that outlive temporary task notes. |

(More chapters land as new audio-subsystem work surfaces invariants
that deserve a frozen home — e.g. SAI bring-up, DMA double-buffer
ordering, multi-codec abstraction. Each future chapter follows the
§0–§15 layout from CLAUDE.md.)

## Conformance target

A conforming rlvgl-platform audio backend MUST satisfy the AUDIO-01
acceptance gates. Future chapters may add additional gates; each
chapter declares its own conformance level.

## Source-of-truth boundaries

Per CLAUDE.md spec-before-code discipline §"Definitions — reference vs.
restatement": this family cites
[`platform/src/wm8994.rs`](../../platform/src/wm8994.rs),
[`platform/src/sai.rs`](../../platform/src/sai.rs),
[`platform/src/sai4_pdm.rs`](../../platform/src/sai4_pdm.rs),
[`platform/src/dma_sai.rs`](../../platform/src/dma_sai.rs), and
[`platform/src/audio_player.rs`](../../platform/src/audio_player.rs)
as authoritative source for any term that has a Rust definition. Chapter
glossaries say "**as defined in [file:line]; used without modification**"
for canonical-elsewhere terms; "**adapted:** [delta]" when this family
extends or narrows; "**owned by AUDIO-NN; does not yet exist in repo**"
when the spec is canonical and code will mirror.

External authority for codec registers: `WM8994_Rev4.6.pdf` (memalpha-
indexed; queryable via `mcp__softoboros__memalpha_ask`). Page citations
in chapters use the form `(p.NNN)`.

External authority for SAI / DMA: STMicroelectronics RM0399 (STM32H745/
STM32H755/STM32H747/STM32H757 Reference Manual).

## Commit-subject prefix

Per CLAUDE.md spec-before-code discipline, audio-family commits use the
`AUDIO-NN[a-z]:` prefix (`AUDIO-01a`, `AUDIO-02a`, etc.), matching the
existing `DISCO-NN[a-z]:`, `BBB-NN[a-z]:`, `CREATOR-NN[a-z]:`,
`CHIPS-<VENDOR>-NN[a-z]:` conventions. Conventional-commit style
(`feat:`, `fix:`, `docs:`) remains the default for non-initiative work
outside this family's scope.

## Downstream consumers

The first downstream consumer driving this family is the disco-analyzer
subrepo
(`softoboros.com:streamz/submodules/disco-analyzer/`),
which uses `Wm8994::init_record` for line-in capture. Disco-analyzer's
own audio-path recon doc
(`docs/AUDIO-DATA-PATH-RECON.md` in that subrepo) is the canonical map
of the full signal chain from CODEC analog input through to LTDC
display output; this rlvgl-side family covers the platform-crate
invariants the downstream map depends on.
