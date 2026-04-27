<!--
04-skins.md - AM-04a: skin descriptors (palettes + layout, no graphics yet).
AM-04b (SVG/PNG primitives + creator rasterisation) ships separately when widgets demand it.
-->

**[← Prev AM-03](03-scales.md) · [Index](README.md)**

# AM-04a — Skin Descriptors

A *skin* binds a Scale + Ballistic + concrete colour palette + layout
hints into a named look-and-feel preset. Widgets pick a skin matching
their family (`bargraph`, `needle`, `numeric`, `lufs_gauge`) and render
accordingly.

This chapter ships the **palette + layout** half of skinning. Visual
primitives (SVG / PNG) and the rlvgl-creator rasterisation pipeline are
deferred to AM-04b — every chapter from AM-05 onward will run with
descriptor-driven widgets that synthesise primitives in code from the
palette here. Bring on real art when widgets are stable.

## Authority

- Concepts §3 (skin glossary entry).
- Concepts §5 (`Ballistic` enum referenced by `default_ballistic`).
- Concepts §7 (`MeterColor` enum mapped to RGB by `palette`).
- AM-03 §6 (canonical scales referenced by `scale_id`).

## What ships

| File | Role |
|---|---|
| `assets/audio-meters/schema/skin.schema.json` | JSON Schema 2020-12. `additionalProperties: false` everywhere. Hex-colour pattern enforced via `$defs/hexColor`. The `assets` field is reserved for AM-04b filenames; AM-04a skins do not populate it. |
| `assets/audio-meters/skins/broadcast_classic_bargraph.json` | US broadcast LED bargraph; classic green / amber / red over `vu_broadcast`. |
| `assets/audio-meters/skins/ebu_classic_bargraph.json` | EBU LED bargraph over `vu_ebu`; teal-tinted Safe / Nominal differentiates from US look. |
| `assets/audio-meters/skins/digital_studio_bargraph.json` | High-resolution dBFS peak meter over `digital_peak`; 48 LEDs, blue Safe band. |
| `assets/audio-meters/skins/broadcast_classic_needle.json` | Cream-faced analog VU look over `vu_broadcast`; black needle, brown frame. |

## Required fields

Every skin MUST declare:

- `id` (matches filename stem),
- `title` (human-readable picker label),
- `scale_id` (must reference a file under `assets/audio-meters/scales/`),
- `default_ballistic` (concepts §5 enum; widget MAY override at
  instantiation),
- `meter_type` (`bargraph` | `needle` | `numeric` | `lufs_gauge`),
- `palette` (every §7 zone identifier mapped to a hex colour),
- `layout` (`orientation`, `aspect_ratio`, plus `led_count` for
  bargraphs).

Optional fields:

- `calibration_override` — overrides the scale's `calibration_default`.
- `secondary_colors` — non-zone colours (background, frame, ticks,
  needle, led_off, peak_hold).
- `assets` — optional pointers to graphical primitives (filenames
  under `assets/audio-meters/{svg,png}/`). AM-04a skins leave this
  empty; AM-04b-stub (commit `<TBD>`) added the runtime
  `SkinAssets` type and `skin.assets` field on the runtime `Skin`
  struct so the aesthetics pass can populate it without changing
  the schema. See `docs/audio-meters/13-asset-hooks.md`.

## Validator contract

Both runtimes enforce:

1. `id` field equals the filename stem.
2. `scale_id` references an existing scale file.
3. `default_ballistic` is in the §5 enum. **Advisory** — if it is
   not in the referenced scale's `compatible_ballistics`, the test
   prints a notice but does not fail (concepts §6 permits
   non-conventional pairings).
4. `meter_type` is in the enum.
5. Every key in `palette` is in the §7 enum; every value is a valid
   `#RRGGBB` or `#RRGGBBAA` hex colour.
6. `secondary_colors` keys (if present) are in the schema-declared set
   and values are valid hex colours.
7. `layout.orientation` is `horizontal` or `vertical`.
8. `0 < layout.aspect_ratio ≤ 100`.
9. `meter_type == "bargraph"` ⇒ `layout.led_count` declared and in
   `[4, 256]`.
10. `layout.peak_hold_ms` (if present) is in `[0, 60000]`.
11. No unknown top-level keys (`$schema` is permitted; everything else
    must be in the schema).

Tests:

- Rust: `cargo test -p rlvgl-audio-meters-core --test skins`
- TS: `npm test --prefix audio-meters-core/ts` (the
  `skins.test.ts` suite).

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| AM-03 scales | Each skin references one scale by id. Skin invalidates if the scale is renamed. Compatible-ballistics check is advisory; widgets MAY pair off-axis. |
| Concepts §7 colour enum | Skin palette MUST cover all 5 §7 identifiers. Skin's concrete colours are `#RRGGBB[AA]` strings; the §7 enum stays abstract. |
| AM-04b (SVG / PNG primitives) | The `assets` field is reserved here so AM-04b can populate filenames without a schema change. AM-04a skins synthesise primitives in widget code from palette + layout alone. |
| `rlvgl-creator` | When AM-04b lands, creator will rasterise SVGs per target and emit `const SkinStatic { palette, assets: { led_segment_data: &[u8], ... } }` modules. Skins authored under AM-04a remain valid; the `assets` block is additive. |
| AM-05 / AM-06 (`LedBargraph`) | First widget consuming a skin. Picks any `meter_type == "bargraph"` skin, renders `led_count` segments in palette colours partitioned by zone boundaries. |

## Acceptance checklist

A conforming AM-04a deployment MUST:

- [x] Author `assets/audio-meters/schema/skin.schema.json`.
- [x] Ship at least three skins covering bargraph + needle and at
      least two distinct scales.
- [x] Validate every checked-in skin against the validator contract
      above, in **both** runtimes.
- [x] Reject unknown top-level fields.
- [x] Document AM-04b (SVG/PNG layer) as deferred but not lost.

## Non-goals (deferred to AM-04b)

- SVG / PNG primitive sources under `assets/audio-meters/{svg,png}/`.
- `rlvgl-creator meters from-yaml` (or equivalent) subcommand for
  rasterisation per target.
- TS bundler-side direct ESM import of SVG / PNG.
- Compile-time codegen of `const SkinStatic` modules for embedded
  consumers.

## Files cited

- `assets/audio-meters/schema/skin.schema.json`
- `assets/audio-meters/skins/*.json`
- `audio-meters-core/tests/skins.rs`
- `audio-meters-core/ts/test/skins.test.ts`
- [`docs/audio-meters/03-scales.md`](03-scales.md)
- [`docs/audio-meters/00-concepts.md`](00-concepts.md)

## Unblocks

- **AM-05 / AM-06** — `LedBargraph` widgets (rlvgl + TS) consume
  bargraph skins to render zone colours and `led_count` segments.

## Change log

- **2026-04-26** — Initial ratification (AM-04a). Schema + 4 canonical
  skins shipped; cross-runtime validators pass on both sides. AM-04b
  (graphical primitives) deferred.
