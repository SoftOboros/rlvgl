<!--
08-ticks-labels.md - AM-08b: tick marks + numeric labels for bargraph + needle.
Refinement layer atop AM-05/06/07; opt-in via show_ticks toggle.
-->

**[← Prev AM-08a](07-numeric-peak.md) · [Index](README.md)**

# AM-08b — Ticks + Labels

Adds major-tick marks and numeric labels to [`LedBargraph`] and
[`NeedleVu`]. Opt-in via a `show_ticks` toggle (default `false`) so
existing callers and tests are unaffected; new callers get a more
"complete-looking" widget by adding a single line.

## Authority

- Concepts §3 (`Scale.tick_labels` glossary entry).
- AM-03 §6 / §8 — `ticks.majors` and `ticks.labels` already in scale
  schema.
- AM-08a (`Renderer::draw_text` plumbing).

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/skin.rs` | New `TickLabel { value, label }` struct. New `Scale.tick_labels: &'static [TickLabel]` field. New `Scale::label_for_major(value)` helper. |
| `widgets/src/meters/presets.rs` | All three Scale presets gain `tick_labels` slices populated from the canonical JSON `ticks.labels` map (vu_broadcast, vu_ebu, digital_peak). |
| `widgets/src/meters/bargraph.rs` | `pub show_ticks: bool` (default `false`). Builder `LedBargraph::new(...).with_ticks()`. When enabled on a vertical bargraph, the LED column shrinks by `BARGRAPH_TICK_STRIP_PX` (36 px) and a tick + label is drawn for each `scale.majors`. |
| `widgets/src/meters/needle.rs` | `pub show_ticks: bool` (default `false`). Builder `.with_ticks()`. Tick marks render as short radial lines just inside the arc; labels render just outside. |
| `audio-meters-widgets/ts/src/led-bargraph-core.ts` | `showTicks: boolean` member, `showTicks` config field, `BARGRAPH_TICK_STRIP_PX` export, optional `DrawSink.drawText`. |
| `audio-meters-widgets/ts/src/needle-vu-core.ts` | Same shape; `NeedleSink.drawText` optional. |

## Draw model

Both runtimes pick label text in this order:

1. `scale.label_for_major(value)` if a `TickLabel` matches (within 1e-3),
2. else `format!("{value:.0}")` (Rust) / `value.toFixed(0)` (TS).

This means a scale's JSON `ticks.labels` map drives label glyphs (with
unicode minus `−` for canonical scales), and majors not listed there
fall back to ASCII formatting.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `Renderer::draw_text` (rlvgl) / canvas `fillText` (TS) | Both invoked from the widget's tick-rendering path. AM-08a established the plumbing. |
| `meter_presets_match_json` | Now also asserts `runtime.tick_labels.len() == json.ticks.labels.len()` and matches each entry by value. |
| Existing widget tests | Unchanged — ticks default off. New tests assert one label per major when `show_ticks = true`. |
| Horizontal bargraph | Tick rendering currently skipped for horizontal orientation (the natural place is the bottom strip with rotated labels — a `Renderer::text_rotation` API doesn't exist yet). AM-08c can revisit. |
| Minor ticks | Not yet rendered. `scale.minors_per_major_division` is read at AM-03 validation time but not yet drawn. AM-08c may add. |

## Acceptance checklist

- [x] `TickLabel` struct + `Scale.tick_labels` field + `label_for_major`.
- [x] All three Rust scale presets carry `tick_labels`.
- [x] `meter_presets_match_json` asserts label parity.
- [x] `LedBargraph` and `NeedleVu` render major ticks + labels when
      `show_ticks` is set; default off.
- [x] TS `LedBargraphCore` and `NeedleVuCore` mirror.
- [x] Both runtimes pass headless rendering tests for the new path.
- [x] Cortex-M7 cross-compile clean.

## Non-goals

- Minor ticks. `Scale` already encodes them; AM-08c can render.
- Horizontal-orientation tick rendering (rotated labels needed).
- Centered or right-aligned label text (Renderer trait offers no text
  metrics).
- Per-zone label colour. Currently all labels render in
  `secondary_colors.scale_text` regardless of zone.

## Files cited

- `widgets/src/meters/skin.rs`, `bargraph.rs`, `needle.rs`, `presets.rs`,
  `mod.rs`
- `widgets/tests/meter_presets_match_json.rs`
- `audio-meters-widgets/ts/src/led-bargraph-core.ts`,
  `needle-vu-core.ts`
- [`docs/audio-meters/03-scales.md`](03-scales.md)

## Unblocks

- **AM-08c** — Minor ticks, horizontal-orientation tick layout, LUFS
  gauge.

## Change log

- **2026-04-26** — Initial ratification (AM-08b). Major ticks +
  labels render on both widgets, both runtimes.
