<!--
07-numeric-peak.md - AM-08a: NumericPeak text-readout widget on rlvgl + TS.
Third widget family proving the skin layering works for any meter type.
-->

**[← Prev AM-07](06-needle-vu.md) · [Index](README.md)**

# AM-08a — NumericPeak (rlvgl + TS)

A two-line text readout showing the current ballistic reading and the
peak-hold value, both expressed in the bound scale's units. Third
widget family in the audio-meters initiative; proves that the
AM-00…AM-04a layering works for any `meter_type` — bargraph, needle,
numeric — without architecture-level changes.

## Authority

- Concepts §3 (Scale glossary entry — `dbfs_to_scale_units`).
- Concepts §5 (Ballistic enum), §9 (widget update contract).
- AM-03, AM-04a (scales + skins).

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/numeric.rs` | `NumericPeak` widget. Owns a `BallisticState` and the same peak-hold tracker as `LedBargraph` (skin `peak_hold_ms` dwell + 12 dB/s decay). Draw model: 1 background fill + 2 text lines. Reading text is coloured by zone; peak text honours `secondary_colors.peak_hold` if set, else falls back to zone colour. |
| `widgets/src/meters/presets.rs` | New: `DIGITAL_STUDIO_NUMERIC`. |
| `assets/audio-meters/skins/digital_studio_numeric.json` | First numeric skin. Pairs with `digital_peak` scale. |
| `audio-meters-widgets/ts/src/numeric-peak-core.ts` | DOM-free TS rendering core. `NumericSink` exposes both `fillRect` and `drawText` for testability. |
| `audio-meters-widgets/ts/src/numeric-peak-element.ts` | `<rlvgl-numeric-peak>` custom element. Uses canvas `fillText`; default font is `ui-monospace`. |

## Text formatting (cross-runtime)

Both runtimes format numbers identically so a side-by-side rendering
of the same dBFS value produces the same string:

- Reading line: `"  -12.3 dBFS"` — Rust `{:>7.1}` / TS hand-padded
  `formatPaddedNumber(value, 7, 1)`.
- Peak line: `"PK   -2.1 dBFS"` — Rust `"PK {:>6.1} {}"` / TS
  `\`PK ${formatPaddedNumber(value, 6, 1)} ${units}\``.

Below floor (`scale.range_min_db`), text falls back to the skin's
`scale_text` colour to avoid flashing a Safe colour on silence.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `Renderer::draw_text` | Rust widget calls this; the platform-supplied renderer chooses the font. The widget does not query text width — text is left-anchored at `bounds.x + 6`. AM-08b can graduate to centered text when a `text_width` query lands. |
| Canvas 2D | TS widget uses `ctx.fillText` with a font controlled via the `font="..."` element attribute (default: `ui-monospace`). |
| `LedBargraph` peak hold | Same `peak_hold_ms` semantics, same 12 dB/s decay constant. Cross-widget consistency. |
| `Scale` / `Skin` | Same as bargraph and needle — no new fields. The skin's `meter_type` determines which widget family it belongs to. |

## Acceptance checklist

- [x] `NumericPeak` widget in Rust under `widgets/src/meters/numeric.rs`.
- [x] `NumericPeakCore` + `RlvglNumericPeakElement` in TS.
- [x] First numeric skin (`digital_studio_numeric`) shipped under
      `assets/audio-meters/skins/`.
- [x] Cross-runtime tests pass on both sides.
- [x] `meter_presets_match_json` covers the new preset and also adds
      the previously-missed `BROADCAST_CLASSIC_NEEDLE` row.
- [x] Cortex-M7 cross-compile clean.

## Non-goals

- Tick marks / scale labels on bargraph and needle (deferred — they
  share the same draw_text plumbing as this widget; will land
  alongside AM-08b skin variants).
- Centered or right-aligned text (need `Renderer::text_width` query).
- Custom font selection on the rlvgl side (renderer-controlled).
- LUFS gauge / multi-band display (deferred to AM-08c).

## Files cited

- `widgets/src/meters/numeric.rs`
- `widgets/src/meters/presets.rs`
- `assets/audio-meters/skins/digital_studio_numeric.json`
- `audio-meters-widgets/ts/src/numeric-peak-core.ts`
- `audio-meters-widgets/ts/src/numeric-peak-element.ts`
- `widgets/tests/meter_presets_match_json.rs`

## Unblocks

- **AM-08b** — Tick marks + numeric labels on bargraph / needle (same
  text plumbing).
- **AM-08c** — LUFS gauge.
- Application-level integration: numeric readout pairs naturally next
  to a bargraph or needle for full information density.

## Change log

- **2026-04-26** — Initial ratification (AM-08a). NumericPeak widget
  shipped on both runtimes; first numeric skin authored.
