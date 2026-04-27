<!--
11-lufs-gauge.md - AM-08d: LUFS loudness gauge.
Compound widget owning three ballistic states; canonical EBU R 128 display.
-->

**[← Prev AM-09](10-integration.md) · [Index](README.md)**

# AM-08d — LUFS Gauge

The canonical EBU R 128 / ITU-R BS.1770-4 production display:
**M**omentary, **S**hort-term, and **I**ntegrated loudness shown
simultaneously, with the integrated reading colour-coded against the
target loudness derived from `scale.pivot_value`.

## Authority

- Concepts §5 — `LufsM`, `LufsS`, `LufsI` ballistic variants.
- Concepts §15-005 — `pivot.value` is the target in scale-units.
- AM-03 / AM-04a — `lufs_ebu_r128` scale, `lufs_gauge` `meter_type`.

## Compound widget — exception to "one widget, one ballistic"

Bargraph, needle, and numeric meters each own a single
`BallisticState`. The LUFS gauge is the first **compound** widget in
the initiative: it owns three (`LufsM`, `LufsS`, `LufsI`) and drives
all of them from a single per-frame `update(dbfs, dt)`. This is a
deliberate exception — three correlated readings of the same signal
are conventionally displayed together (you don't show "just the M"
in isolation), so factoring them as separate widgets would force the
caller to wire three update sites and three render sites for what is
visually one component.

The exception leaves the architecture intact: the widget still owns
its ballistics (concepts §15-003), the caller still feeds K-weighted
dBFS upstream (concepts §5 `LufsM/S/I` rows), and the schema is
unchanged. Only the widget surface differs.

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/lufs_gauge.rs` | `LufsGauge` widget. Three internal `BallisticState`s. Layout: 3 stacked text lines (I, S, M); integrated line shows additional `(±N.N LU)` deviation from target. |
| `widgets/src/meters/presets.rs` | New `SCALE_LUFS_EBU_R128` const + new `LUFS_EBU_R128_GAUGE` skin. The `pivot_label` carries unicode minus (`"−23"`) matching the JSON. |
| `assets/audio-meters/skins/lufs_ebu_r128_gauge.json` | First `lufs_gauge` skin. Pairs with the existing `lufs_ebu_r128` scale. |
| `audio-meters-widgets/ts/src/lufs-gauge-core.ts` | DOM-free TS rendering core. `LufsSink` requires both `fillRect` and `drawText`. |
| `audio-meters-widgets/ts/src/lufs-gauge-element.ts` | `<rlvgl-lufs-gauge>` custom element. |

## Colour banding

The integrated reading colours by its LU deviation from target
(`scale.pivot_value`):

| LU range | Colour |
|---|---|
| ≤ -1.5 | `Safe` (under-target) |
| -1.5 to -0.5 | `Caution` |
| -0.5 to +0.5 | `Nominal` (at target) |
| +0.5 to +1.5 | `Caution` |
| ≥ +1.5 | `Hot` (over-target) |

Below `range_min_db` (silence), the integrated reading falls back to
`secondary_colors.scale_text` to avoid flashing a Safe colour on
silence. ITU-R BS.1770-4 doesn't mandate these bands; they are
conventional EBU R 128 production thresholds (±0.5 LU "in the
pocket", ±1.5 LU "acceptable", beyond is action-required).

`LufsM` and `LufsS` lines render in the default text colour
(secondary `scale_text`); they're for context, not for action. AM-08e
or aesthetics pass MAY add momentary-only colour banding if a use
case demands it.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| Concepts §5 (`LufsM/S/I`) | Widget instantiates one `BallisticState` per variant. The L0 `LufsI` is currently ungated (concepts §5 LufsI row notes BS.1770 gating deferred to AM-08); when full gating lands, `LufsGauge` will pick it up automatically since it consumes `BallisticState` by value. |
| K-weighting | Caller's responsibility (concepts §9 widget update contract). The widget API doesn't change between K-weighted and unweighted input — the widget treats whatever it receives as authoritative dBFS. |
| `MeterWidget` trait (AM-08c stereo) | Not implemented. Stereo loudness gauges are unconventional (loudness is typically programme-level, not per-channel); when needed, a `StereoLufs` composite would be the natural shape. |
| Existing skins (bargraph / needle / numeric) | Untouched. The `lufs_gauge` `meter_type` was already in the §schema enum; this chapter is the first widget to consume it. |

## Acceptance checklist

- [x] `LufsGauge` widget in Rust with three internal ballistics.
- [x] `LufsGaugeCore` + `RlvglLufsGaugeElement` in TS.
- [x] First `lufs_gauge` skin shipped under `assets/audio-meters/skins/`.
- [x] `SCALE_LUFS_EBU_R128` Rust preset added to fill the previously
      JSON-only gap.
- [x] `meter_presets_match_json` covers the new scale + skin.
- [x] Both runtimes pass headless tests including colour-banding
      assertion at and above target.
- [x] Cortex-M7 cross-compile clean.

## Non-goals

- True ITU-R BS.1770-4 absolute (-70 LUFS) and relative (-10 LU)
  gating in `LufsI`. Currently the L0 `LufsI` is an ungated running
  mean (concepts §5 LufsI row). Full gating is a separate L0 phase
  (call it AM-08e); the widget will benefit immediately when L0
  upgrades.
- True-peak (1.7704-Annex-2) overlay alongside loudness. True peak
  is a different ballistic; pair an `LedBargraph` (DigitalPeak) next
  to the gauge in app code if needed.
- LRA (Loudness Range) display per EBU Tech 3342. Statistical, not
  ballistic — different computation, different widget.
- Per-channel-LR LUFS (M/S decoded). Niche; awaiting need.

## Files cited

- `widgets/src/meters/lufs_gauge.rs`
- `widgets/src/meters/presets.rs`
- `assets/audio-meters/skins/lufs_ebu_r128_gauge.json`
- `audio-meters-widgets/ts/src/lufs-gauge-core.ts`
- `audio-meters-widgets/ts/src/lufs-gauge-element.ts`
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §5, §15-005

## Unblocks

- **Aesthetics pass** — A LUFS gauge is the most visual-design-heavy
  meter (typical production gauges have arc indicators, multi-band
  histograms, animated target lines). The widget surface here is
  intentionally text-minimal so designers can replace the rendering
  without touching the ballistic plumbing.
- **AM-08e** — true BS.1770 gating for `LufsI`.

## Change log

- **2026-04-26** — Initial ratification (AM-08d). LufsGauge shipped
  on both runtimes; first compound widget in the initiative.
