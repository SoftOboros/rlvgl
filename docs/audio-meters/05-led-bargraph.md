<!--
05-led-bargraph.md - AM-05/06: mono LED bargraph widget on rlvgl + TS.
First widget consuming the AM-00…AM-04a layering end-to-end.
-->

**[← Prev AM-04a](04-skins.md) · [Index](README.md)**

# AM-05 / AM-06 — LED Bargraph (rlvgl + TS)

The first composite widget. Mono, runtime-skinned, ballistic-driven.
Both runtimes implement the same widget end-to-end: feed dBFS at
display rate, get back a coloured LED bar with optional peak-hold pip.

## Authority

- Concepts §5 (Ballistic enum) — widget owns one [`BallisticState`].
- Concepts §7 (MeterColor enum) — palette mapping comes from the bound
  skin.
- Concepts §9 (widget update contract) — `update(dbfs, dt)` once per
  displayed frame; `dbfs` is pre-detection (caller's job to RMS / peak
  / weight upstream).
- AM-03 (Scale descriptors) — widget reads `range_db`, `zones`,
  `calibration_default` from the bound scale.
- AM-04a (Skin descriptors) — widget consumes `palette`,
  `secondary_colors`, and `layout` (orientation, `led_count`,
  `peak_hold_ms`).

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/skin.rs` | Public runtime `Scale` / `Skin` types using `&'static str` and `&'static [Zone]` so the entire skin can live in flash on `no_std` targets. |
| `widgets/src/meters/bargraph.rs` | `LedBargraph` widget. Implements `Widget` trait; owns `BallisticState` + peak-hold tracker. |
| `widgets/src/meters/presets.rs` | Hand-authored `pub static` `Scale` / `Skin` constants matching the JSON descriptors. **Replaced by `rlvgl-creator` codegen output in AM-04b** — see "Integration progression" below. |
| `widgets/tests/meter_presets_match_json.rs` | Editor-discipline net: every preset must agree with its JSON twin field-by-field. |
| `audio-meters-widgets/ts/src/skin.ts` | TS interfaces mirroring the Rust types. snake_case fields so JSON loads directly. |
| `audio-meters-widgets/ts/src/led-bargraph-core.ts` | DOM-free rendering core with a `DrawSink` interface — Node-testable. |
| `audio-meters-widgets/ts/src/led-bargraph-element.ts` | `<rlvgl-led-bargraph>` custom element. Loads skin + scale via `fetch`, runs a `requestAnimationFrame` loop, paints onto an internal `<canvas>`. |

## Widget update contract

Application code:

```rust
let mut bar = LedBargraph::new(bounds, &BROADCAST_CLASSIC_BARGRAPH);
loop {
    let dbfs = pull_latest_dbfs_from_audio_pipeline();
    bar.update(dbfs, frame_dt);
    bar.draw(&mut renderer);
}
```

```ts
const meter = document.querySelector("rlvgl-led-bargraph") as RlvglLedBargraphElement;
audioWorklet.port.onmessage = (e) => meter.feed(e.data.dbfs);
```

Both runtimes:

- Hold a `BallisticState` (or `BallisticState` instance on TS) keyed
  by the skin's `default_ballistic`.
- Cache the latest reading and a peak-hold scalar with a dwell timer
  driven by `skin.layout.peak_hold_ms`.
- After the dwell expires, the peak pip decays at a fixed `12 dB / s`
  toward the live reading. (This is the same constant in both runtimes.)
- On `set_ballistic` / `setBallistic`, reset ballistic state, cached
  reading, and peak tracker — the new ballistic starts at the floor
  and converges at its own time constant.

## Draw model

Both runtimes paint:

1. One background fill covering the widget bounds.
2. `led_count` segment fills, segment 0 at the "low" end of the meter
   (bottom for vertical, left for horizontal).
3. Each segment's centre is mapped to display dB, looked up in the
   scale's zones, and drawn in the matching palette colour when lit
   or in `secondary_colors.led_off` when unlit.
4. Optional peak-hold pip: drawn over the segment containing the
   peak, when `peak_hold_ms > 0`, peak is above the floor, and the
   meter is not fully lit. Peak colour comes from
   `secondary_colors.peak_hold` (or white if absent).

The Rust widget uses `Renderer::fill_rect`; the TS core uses a
`DrawSink.fillRect(x, y, w, h, color)` interface so tests can record
ops without a real canvas.

## Integration progression: path (b) → path (a)

This widget initially uses **path (b)**: app code constructs runtime
`Skin` / `Scale` structs (or in TS, parses the JSON at runtime via
`fetch`). Path (b) is appropriate when the skin set is small, the
target permits some startup cost, and the developer prefers to stay
out of build.rs.

The transition target is **path (a)**: `rlvgl-creator meters from-yaml`
(or equivalent) at build time emits Rust source containing
`pub static SCALE_VU_BROADCAST: Scale = Scale { ... };` literals from
the canonical JSON. The widget code does not change; the consumer's
app crate or downstream BSP imports generated constants instead of
hand-authored ones. Path (a) is appropriate when:

- The target is heavily flash-constrained and pulling in
  `serde_json` at runtime is undesirable.
- The skin set has grown beyond ~10 entries and hand-maintaining the
  Rust constants is error-prone (the `meter_presets_match_json` test
  begins flagging real drift).
- A downstream BSP wants to bake skins into firmware as `const` data.

The path-(b) → path-(a) transition is **purely a build-side change**
— the widget API, the descriptor schema, and the JSON files are
unchanged. The codegen output substitutes for `widgets::meters::presets`.

When path (a) lands (AM-04b), this chapter's reconciliation table
gains a row noting that `presets.rs` becomes the codegen output
location and that the editor-discipline test
(`meter_presets_match_json.rs`) is no longer needed (codegen makes
divergence impossible). Path (b) remains supported for app code that
prefers hand-authored constants.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `rlvgl-audio-meters-core::BallisticState` | Widget holds one. Widget calls `update(dbfs, dt)` and reads back the current reading. No other state shared. |
| `rlvgl-core::Widget` trait | `LedBargraph` implements `bounds()`, `draw()`, and `handle_event()` (always returns `false` — meters are read-only). |
| `assets/audio-meters/scales/*.json` and `skins/*.json` | Source of truth. Rust path-(b) presets are checked field-by-field against these in `widgets/tests/meter_presets_match_json.rs`. TS imports the JSON directly via `fetch`. |
| Ballistic decay vs. peak-hold decay | Ballistic decay is per-variant (concepts §5). Peak-hold dwell is per-skin (`peak_hold_ms`). Decay-after-dwell is hard-coded `12 dB / s` in both runtimes — this is a widget-level convention, not part of any L0 / L1 / L2 schema. If a skin needs a different rate, AM-08 will add `peak_decay_db_per_s` to the schema. |
| `rlvgl-creator` (AM-04b) | Widget is the first consumer that path (a) will need to support. The `Skin` / `Scale` struct shapes here are deliberately the codegen target — codegen needs no struct-shape changes, just a new emitter. |

## Acceptance checklist

A conforming AM-05 / AM-06 deployment MUST:

- [x] Implement `LedBargraph` widget in `widgets/src/meters/`.
- [x] Implement `LedBargraphCore` + `RlvglLedBargraphElement` in
      `audio-meters-widgets/ts/`.
- [x] Both runtimes pass headless rendering tests:
      - Rust: `cargo test -p rlvgl-widgets --lib meters`
      - TS: `npm test --prefix audio-meters-widgets/ts`
- [x] Cortex-M7 cross-compile of the widgets crate succeeds.
- [x] Editor-discipline test (`meter_presets_match_json`) pins the
      Rust path-(b) presets to their JSON twins.
- [x] Document path (b) → path (a) progression (this chapter).

## Non-goals (deferred)

- **Stereo, multi-channel.** Compose two `LedBargraph` widgets
  side-by-side at app level. A `StereoBargraph` composite may land
  alongside AM-08.
- **Tick marks and numeric labels on the bargraph.** AM-08 adds
  scale-text rendering. The widget's draw model leaves room for a
  text overlay without API change.
- **Anti-aliasing / glow halo on LED segments.** Cosmetic; depends on
  AM-04b graphical primitives.
- **Touch interaction.** Meters are read-only by spec. If a future
  variant needs tap-to-reset, that's a new meter type.

## Files cited

- `widgets/src/meters/skin.rs`
- `widgets/src/meters/bargraph.rs`
- `widgets/src/meters/presets.rs`
- `widgets/src/meters/mod.rs`
- `widgets/tests/meter_presets_match_json.rs`
- `audio-meters-widgets/ts/src/skin.ts`
- `audio-meters-widgets/ts/src/led-bargraph-core.ts`
- `audio-meters-widgets/ts/src/led-bargraph-element.ts`
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §5, §7, §9
- [`docs/audio-meters/03-scales.md`](03-scales.md)
- [`docs/audio-meters/04-skins.md`](04-skins.md)

## Unblocks

- **AM-04b** — `rlvgl-creator meters from-yaml` codegen for path (a).
  This widget is the first consumer.
- **AM-07** — `NeedleVu` widget. Same skin / scale layering; different
  draw model.
- **AM-08** — Tick marks, numeric readout, LUFS gauge, skin presets.

## Change log

- **2026-04-26** — Initial ratification (AM-05 / AM-06). Mono
  LedBargraph in both runtimes; path (b) integration shape with
  documented progression to path (a). 5 + 8 widget tests pass; 13 +
  40 cumulative tests across the initiative.
