<!--
06-needle-vu.md - AM-07: NeedleVu analog-style meter on rlvgl + TS.
Same skin/scale layering as the LedBargraph; different draw model.
-->

**[← Prev AM-05/06](05-led-bargraph.md) · [Index](README.md)**

# AM-07 — NeedleVu (rlvgl + TS)

The second composite widget. Mono analog needle, ballistic-driven,
runtime-skinned. Validates that the AM-00…AM-04a layering generalises
beyond bargraphs: same `Skin` schema, same scale projection, same
ballistic API — different draw model.

## Authority

- Concepts §3 (Scale glossary entry; `dbfs_to_scale_units`).
- Concepts §5 (Ballistic enum) — widget owns one [`BallisticState`].
- Concepts §9 (widget update contract) — `update(dbfs, dt)` per frame.
- AM-03 (Scale descriptors), AM-04a (Skin descriptors).
- Concepts §15 entry **2026-04-26-005** — `pivot.value` schema fix
  surfaced by this chapter.

## Schema fix (concepts §15-005)

Building NeedleVu surfaced that the widget was conflating two
different dB-domain conversions:

1. **dBFS → scale-units** — required for positioning (zone lookup,
   needle angle, bargraph fraction). The conversion is
   `scale_units = dbfs + (pivot.value - pivot.input_dbfs)`.
2. **scale-units → alt-units** — optional, for relabeling (dBVU →
   dBu, etc.). The conversion is `alt = scale_units + offset_db`,
   carried by `calibration_default`.

The original `pivot` shape only had `label` (string) and `input_dbfs`
(number). To derive offset (1) the widget had to parse the label
string, which is fragile (unicode minus, BBC marks, future LUFS
labels). Schema now requires a numeric `pivot.value`.

The fix is **non-breaking** for widgets that haven't shipped yet
(both LedBargraph and NeedleVu adopted the new model in the same
commit) and **breaking** for any external consumer that hand-wrote
scale JSON before this commit. Migration: add `"value": <number>` to
`pivot`. The validator's new check that `value ∈ [range_db.min,
range_db.max]` catches typos.

LedBargraph was updated in lockstep — it now also uses
`dbfs_to_scale_units` for zone lookup and lit-fraction. The previous
implementation accidentally used `calibration_default.offset_db` as
the positioning offset, which only happened to give plausible-looking
output for `digital_peak` (where the offset is 0).

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/needle.rs` | `NeedleVu` widget. Implements `Widget` trait; owns a `BallisticState`. Draw model: face fill + needle line + pivot dot. |
| `widgets/src/meters/skin.rs` | New: `Scale::dbfs_to_scale_units`. New: `Scale.pivot_value` field. |
| `widgets/src/meters/presets.rs` | New: `BROADCAST_CLASSIC_NEEDLE` constant matching the JSON skin. All scale presets gained `pivot_value: 0.0` (or the appropriate scale-units number for ppm_iia_bbc / lufs_ebu_r128 once AM-08 lands more presets). |
| `assets/audio-meters/schema/scale.schema.json` | Added required `pivot.value` (number); range check (within `range_db`). |
| `assets/audio-meters/scales/*.json` | All six canonical scales updated with explicit `pivot.value`. |
| `audio-meters-widgets/ts/src/needle-vu-core.ts` | DOM-free TS rendering core mirroring the Rust widget. |
| `audio-meters-widgets/ts/src/needle-vu-element.ts` | `<rlvgl-needle-vu>` custom element. |
| `audio-meters-widgets/ts/src/skin.ts` | New: `dbfsToScaleUnits` helper. New: `Scale.pivot.value` field. |

## Draw model

Both runtimes paint:

1. One background fill (face colour from `secondary_colors.background`).
2. A needle line from the pivot at the bottom-centre of the widget,
   length ≈ 0.95 × widget height, at angle
   `−half_arc + frac · 2·half_arc`, where
   `frac = (scale_units − range_min) / span`. `half_arc` is hard-coded
   at 50° so the needle sweeps ±50° about vertical (classic analog VU
   sweep).
3. A small filled square at the pivot for visual closure.

Tick marks, numeric labels, and the curved arc background are
deferred to **AM-08**. The current draw model is intentionally minimal
— it proves the layering, gives a runnable widget, and leaves room
for AM-08 to layer text and curves on top without API change.

The Rust path uses `libm::sinf` / `cosf` and walks the line in unit
steps painting `2 × 2` filled rectangles per step (since the
[`Renderer`] trait offers `fill_rect` only). The TS path uses
`Math.sin` / `Math.cos` and the same `DrawSink` step-and-paint pattern
so the two implementations produce visually-identical pixel-step
output for unit tests.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `Scale` runtime type | Gained `pivot_value: f32` (Rust) / `pivot.value: number` (TS). All consumers updated. |
| `LedBargraph` (AM-05/06) | Updated in lockstep to use `dbfs_to_scale_units` for positioning. The semantics improve for VU / PPM / LUFS scales whose `calibration_default.offset_db` is non-zero — those bargraphs were previously off-by-the-cal-offset. `digital_peak` rendering is unchanged (offset is 0). |
| `meter_presets_match_json` | Updated to assert `runtime.pivot_value == json.pivot.value`. |
| Concepts §3 glossary entry "Scale" | Refined to clarify the two distinct dB-domain conversions. |
| AM-04b (creator codegen) | Will emit `pivot_value` from JSON unchanged — the schema field is the codegen source. |

## Acceptance checklist

A conforming AM-07 deployment MUST:

- [x] Implement `NeedleVu` in `widgets/src/meters/needle.rs`.
- [x] Implement `NeedleVuCore` + `RlvglNeedleVuElement` under
      `audio-meters-widgets/ts/`.
- [x] Adopt the `dbfs_to_scale_units` model in **both** widgets
      (LedBargraph + NeedleVu).
- [x] Update the schema, all six canonical scale JSON files, and both
      runtime validators to require `pivot.value`.
- [x] Both runtimes pass headless rendering tests.
- [x] Cortex-M7 cross-compile clean.
- [x] Concepts §15 amended with the schema-fix entry
      (`2026-04-26-005`).

## Non-goals

- Tick marks, numeric labels, arc background. Deferred to AM-08.
- Anti-aliased / SVG-rasterised needle art. Deferred to AM-04b.
- Peak-hold pip on the needle. The needle's analog-style movement is
  already a peak-decay tracker via the bound ballistic; a separate
  pip would add visual clutter without conveying new information.

## Files cited

- `assets/audio-meters/schema/scale.schema.json`
- `assets/audio-meters/scales/*.json` (six files updated)
- `widgets/src/meters/skin.rs`
- `widgets/src/meters/needle.rs`
- `widgets/src/meters/bargraph.rs`
- `widgets/src/meters/presets.rs`
- `audio-meters-widgets/ts/src/needle-vu-core.ts`
- `audio-meters-widgets/ts/src/needle-vu-element.ts`
- `audio-meters-widgets/ts/src/led-bargraph-core.ts`
- `audio-meters-widgets/ts/src/skin.ts`
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §15-005

## Unblocks

- **AM-08** — Tick marks, numeric labels, additional skin variants,
  arc backgrounds, LUFS gauge.

## Change log

- **2026-04-26** — Initial ratification (AM-07). NeedleVu shipped on
  both runtimes; concepts §15-005 schema fix landed in lockstep.
