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
| AM-01 — Core math (Rust) | In progress | _(pending)_ |
| AM-02 — TS port of L0 | Planned | _(pending)_ |
| AM-03 — Scale descriptors | Planned | _(pending)_ |
| AM-04 — Asset package | Planned | _(pending)_ |
| AM-05 — `LedBargraph` (rlvgl) | Planned | _(pending)_ |
| AM-06 — `LedBargraph` (TS) | Planned | _(pending)_ |
| AM-07 — `NeedleVu` (both) | Planned | _(pending)_ |
| AM-08 — Aux meters + skins | Planned | _(pending)_ |
| AM-09 — Integration | Planned | _(pending)_ |

## Reference

- [`CLAUDE.md` § Spec-Before-Code Planning Discipline](../../CLAUDE.md)
- IEC 60268-10 (PPM), IEC 60268-17 (VU), AES17, ITU-R BS.1770-4, EBU R 128

This index is **informative**. Normative content lives in the per-chapter
docs.
