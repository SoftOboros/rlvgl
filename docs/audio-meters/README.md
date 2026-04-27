<!--
README.md - Audio Meters initiative index. Informative.
Per-chapter docs are the normative artifacts.
-->

# Audio Meters

A layered, **cross-runtime** asset and code hierarchy for VU-style audio
metering. One source of truth for visual primitives (SVG / PNG), parameter
descriptors (JSON), and ballistic math (Rust core, hand-ported TS); two
consumers (rlvgl `no_std` widget tree, browser / Tauri TS).

## Conformance

A **conforming audio-meters deployment** MUST satisfy the AM-00 §12
acceptance checklist (vocabulary ratified) and the acceptance checklists of
whichever phase chapters its build includes.

A conforming deployment MAY additionally satisfy any combination of:

- AM-01 / AM-02 (`audio-meters-core` Rust + TS L0).
- AM-03 (canonical scale-descriptor JSON Schema + initial scale set).
- AM-04 (shared asset package + rlvgl-creator rasterisation).
- AM-05 / AM-06 (`LedBargraph` widget on each runtime).
- AM-07 (`NeedleVu` widget on each runtime).
- AM-08 (`NumericPeak`, `LufsIntegrated`, skin presets).
- AM-09 (live integration: `MicCapture` → ballistic → on-screen meter).

Each phase is independently conformant once its acceptance checklist passes.

## Chapters

| Phase | Status | Doc |
|---|---|---|
| AM-00 — Concepts | Ratified 2026-04-26 | [00-concepts.md](00-concepts.md) |
| AM-01 — Core math (Rust) | Ratified 2026-04-26 | _(combined with AM-02; see commit `2aa15ac`)_ |
| AM-02 — TS port of L0 | Ratified 2026-04-26 | _(combined with AM-01; see commit `2aa15ac`)_ |
| AM-03 — Scale descriptors | Ratified 2026-04-26 | [03-scales.md](03-scales.md) |
| AM-04a — Skin descriptors | Ratified 2026-04-26 | [04-skins.md](04-skins.md) |
| AM-04b — Visual primitives + creator rasterisation | Deferred | _(deferred until widget-side demand)_ |
| AM-05 — `LedBargraph` (rlvgl) | Ratified 2026-04-26 | [05-led-bargraph.md](05-led-bargraph.md) |
| AM-06 — `LedBargraph` (TS) | Ratified 2026-04-26 | [05-led-bargraph.md](05-led-bargraph.md) (combined) |
| AM-07 — `NeedleVu` (both) | Ratified 2026-04-26 | [06-needle-vu.md](06-needle-vu.md) |
| AM-08a — `NumericPeak` (both) | Ratified 2026-04-26 | [07-numeric-peak.md](07-numeric-peak.md) |
| AM-08b — Ticks + labels | Ratified 2026-04-26 | [08-ticks-labels.md](08-ticks-labels.md) |
| AM-08c — Stereo composition | Ratified 2026-04-26 | [09-stereo.md](09-stereo.md) |
| AM-08d — LUFS gauge | Ratified 2026-04-26 | [11-lufs-gauge.md](11-lufs-gauge.md) |
| AM-09 — Integration | Ratified 2026-04-26 | [10-integration.md](10-integration.md) |

## Reference

- [`CLAUDE.md` § Spec-Before-Code Planning Discipline](../../CLAUDE.md)
- IEC 60268-10 (PPM), IEC 60268-17 (VU), AES17, ITU-R BS.1770-4, EBU R 128

This index is **informative**. Normative content lives in the per-chapter
docs.
