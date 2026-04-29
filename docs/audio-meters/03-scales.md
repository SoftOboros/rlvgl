<!--
03-scales.md - AM-03: scale descriptor schema and canonical scale set.
Normative sections cited from acceptance checklist below.
-->

**[← Prev AM-00](00-concepts.md) · [Index](README.md)**

# AM-03 — Scale Descriptors

This chapter ratifies the canonical [JSON
Schema](../../assets/audio-meters/schema/scale.schema.json) for audio-meter
scale descriptors, and the initial scale set under
[`assets/audio-meters/scales/`](../../assets/audio-meters/scales/). It
implements the §8 sketch from AM-00.

## Authority

- Concepts §0 (RFC 2119 / 8174 keywords).
- Concepts §6 (frozen `Scale` identifier set; registration policy:
  Specification Required).
- Concepts §7 (frozen `MeterColor` zone identifiers; concrete RGB owned
  by skins, not scales).
- Concepts §8 (informal schema; this chapter ratifies the canonical
  JSON Schema document).

## What ships

| File | Role |
|---|---|
| `assets/audio-meters/schema/scale.schema.json` | Canonical JSON Schema 2020-12 document. Both runtimes parse against this; `additionalProperties: false` at every object boundary so unknown keys fail loudly. |
| `assets/audio-meters/scales/vu_broadcast.json` | US broadcast / SMPTE VU. `0 VU = -20 dBFS = +4 dBu` → `calibration_default` adds 24 dB. |
| `assets/audio-meters/scales/vu_ebu.json` | EBU VU. `0 VU = -18 dBFS = 0 dBu` → `calibration_default` adds 18 dB. |
| `assets/audio-meters/scales/ppm_din.json` | DIN 45406 PPM. Range −50…+5 dB, pivot `0 = -9 dBFS`. |
| `assets/audio-meters/scales/ppm_iia_bbc.json` | BBC PPM. Range 1…7 (BBC marks), pivot `4 = -18 dBFS`. |
| `assets/audio-meters/scales/digital_peak.json` | AES17 dBFS. Range −60…0, pivot `0 = 0 dBFS`. |
| `assets/audio-meters/scales/lufs_ebu_r128.json` | EBU R 128. Range −36…0 LUFS, pivot `−23 = -23 dBFS`. |

The §6 enumeration and the on-disk set are the same six identifiers.

## Validator contract

Both runtimes implement the same internal-consistency checks. Anything
listed here MUST pass in **both** runtimes — divergence is a bug in
whichever runtime fails first.

1. `id` field equals the filename stem.
2. `range_db.min < range_db.max`.
3. `pivot.input_dbfs` is finite.
4. `ticks.majors` is strictly ascending; first major equals
   `range_db.min`, last major equals `range_db.max` (within 1e-3).
5. `zones` partition `range_db` exactly: zone[0].`from_db` equals
   `range_db.min`; zone[i].`from_db` equals zone[i-1].`to_db` (within
   1e-3); zone[last].`to_db` equals `range_db.max`.
6. Each `zone.color` ∈ §7 enum (`Safe`, `Nominal`, `Caution`, `Hot`,
   `Over`).
7. `compatible_ballistics` is non-empty, contains no duplicates, and
   each entry is in the §5 enum.
8. Each key in `ticks.labels` parses as a number that matches a value
   in `ticks.majors` (within 1e-3).
9. No unknown top-level keys (`$schema` is permitted; everything else
   must be in the schema).

Tests:

- Rust: `cargo test -p rlvgl-audio-meters-core --test scales`
- TS: `npm test --prefix audio-meters-core/ts` (the
  `scales.test.ts` suite)

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| Concepts §8 (informal schema sketch) | The on-disk schema is the canonical extension. Any field added in `scale.schema.json` MUST also appear in the §8 sketch (or the §8 sketch MUST be updated in the same PR). |
| `audio-meters-core` (`rlvgl-audio-meters-core`, Rust) | No public `Scale` type yet — runtime-typed validation lives in the integration test. Public Scale types land with widgets in AM-05, where they are used to drive ticking, zones, and label rendering. |
| `@rlvgl/audio-meters-core` (TS) | Same as Rust: validation lives in the test suite for AM-03. Widget package adds runtime types in AM-06. |
| `rlvgl-creator` (AM-04) | Will consume `scale.schema.json` for codegen — generated Rust skin modules emit `const SCALE: ScaleStatic = …` from the JSON. AM-04 owns that path. |

## Acceptance checklist

A conforming AM-03 deployment MUST:

- [x] Author `assets/audio-meters/schema/scale.schema.json` (JSON Schema
      2020-12).
- [x] Ship the §6 enumeration as on-disk JSON files in
      `assets/audio-meters/scales/`.
- [x] Validate every checked-in scale against the validator contract
      above, in **both** runtimes.
- [x] Reject unknown top-level fields (`additionalProperties: false`
      / `deny_unknown_fields`).
- [x] Document this chapter in [`docs/audio-meters/README.md`](README.md).

## Non-goals

- Public `Scale` type in either runtime. Deferred to AM-05 / AM-06.
- Skin descriptors (binding scale + ballistic + assets). Owned by
  AM-04.
- JSON Schema runtime validator dependency. Both runtimes use
  hand-rolled internal-consistency checks; the schema document is
  authoritative documentation. AM-04 may add a runtime validator if a
  consumer needs schema-driven error messages.

## Files cited

- `assets/audio-meters/schema/scale.schema.json`
- `assets/audio-meters/scales/*.json`
- `audio-meters-core/tests/scales.rs`
- `audio-meters-core/ts/test/scales.test.ts`
- [`docs/audio-meters/00-concepts.md`](00-concepts.md)

## Unblocks

- **AM-04** — Asset package layout (skins, SVG/PNG primitives,
  rlvgl-creator rasterisation). AM-04 references this scale set
  unmodified.
- **AM-05 / AM-06** — `LedBargraph` widgets (rlvgl + TS) consume
  `range_db`, `pivot`, `zones`, `compatible_ballistics`.

## Change log

- **2026-04-26** — Initial ratification. Schema + 6 canonical scales
  shipped; cross-runtime validators in both Rust and TS pass.
