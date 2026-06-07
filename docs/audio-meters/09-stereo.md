<!--
09-stereo.md - AM-08c: stereo composition.
Generic two-channel container working with any meter widget.
-->

**[← Prev AM-08b](08-ticks-labels.md) · [Index](README.md)**

# AM-08c — Stereo Composition

Almost every real-world VU / PPM / loudness meter is stereo. This
chapter ships a generic two-channel container that works with any
first-party (or third-party) meter widget. No new visual rendering —
just composition over the existing widgets.

## Authority

- AM-05/06 (LedBargraph), AM-07 (NeedleVu), AM-08a (NumericPeak).
- No new schema. Stereo is a composition concern, not a skin concern.

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/stereo.rs` | New module. Defines `MeterWidget` trait (`update(dbfs, dt)` + `reset()`). Implements it for all three first-party meters. Exports `StereoPair<W: MeterWidget>` generic container and `split_horizontal(outer, gap)` geometry helper. |
| `widgets/src/meters/mod.rs` | Re-exports `MeterWidget`, `StereoPair`, `split_horizontal`. |
| `audio-meters-widgets/ts/src/stereo.ts` | TS mirror. `MeterCoreLike` duck-typed interface (any object with `update + draw + optional reset`). `StereoPair<C>` generic over the core type. `splitHorizontal` helper. |

## Surface

### Rust

```rust
use rlvgl_widgets::meters::{
    presets::BROADCAST_CLASSIC_BARGRAPH,
    LedBargraph, StereoPair, split_horizontal,
};
use rlvgl_core::widget::Rect;

let outer = Rect { x: 0, y: 0, width: 80, height: 320 };
let (lb, rb) = split_horizontal(outer, 4);
let left = LedBargraph::new(lb, &BROADCAST_CLASSIC_BARGRAPH).with_ticks();
let right = LedBargraph::new(rb, &BROADCAST_CLASSIC_BARGRAPH).with_ticks();

let mut pair = StereoPair::new(outer, 4, left, right);
loop {
    let (l_dbfs, r_dbfs) = pull_stereo_dbfs();
    pair.update_stereo(l_dbfs, r_dbfs, frame_dt);
    pair.draw(&mut renderer);
}
```

The same pattern works with `NeedleVu` and `NumericPeak` —
`StereoPair<NeedleVu>` etc. — because all three implement
`MeterWidget`.

### TS

```ts
import {
  LedBargraphCore,
  StereoPair,
  splitHorizontal,
} from "@rlvgl/audio-meters-widgets";

const left = new LedBargraphCore({ scale, skin, showTicks: true });
const right = new LedBargraphCore({ scale, skin, showTicks: true });
const pair = new StereoPair(
  { x: 0, y: 0, w: 80, h: 320 },
  4,
  left,
  right,
);

audioWorklet.port.onmessage = (e) => {
  pair.updateStereo(e.data.left_dbfs, e.data.right_dbfs, e.data.dt);
};
// In your rAF loop: pair.draw(canvasSink);
```

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `MeterWidget` trait (Rust) | Newly-introduced. Implemented for `LedBargraph`, `NeedleVu`, `NumericPeak`. Third-party meter widgets MAY implement it; the trait surface is intentionally tiny so future widget families adopt it without ceremony. |
| `MeterCoreLike` interface (TS) | Duck-typed: any object with `update(dbfs, dt)` and `draw(sink, x, y, w, h)` qualifies. The first-party cores already match. |
| `rlvgl_core::Widget` trait | `StereoPair<W>` implements it; the rlvgl widget tree treats a stereo pair as a single composite. Internal children are exposed as `pair.left` / `pair.right` for app-level mutation (e.g. swapping ballistic on one channel). |
| Skin layering | Both children share one skin in typical usage. Different skins per channel are technically supported (the constructor takes any two children) but unconventional — would mostly serve "L = peak, R = RMS" comparison displays. |
| `splitHorizontal` | Public helper; callers that prefer to manage their own children inside an `rlvgl_core::Container` may skip `StereoPair` and reuse just the geometry split. |

## Acceptance checklist

- [x] `MeterWidget` trait exposes the minimal `update + reset` surface.
- [x] All three first-party meters implement it.
- [x] `StereoPair<W>` is generic and `Widget`-implementing.
- [x] TS `StereoPair<C>` mirrors the surface duck-typed.
- [x] `splitHorizontal` matches between runtimes.
- [x] Cortex-M7 cross-compile clean.
- [x] Both runtimes pass headless tests.

## Non-goals

- M/S decoding, surround layouts (5.1, 7.1, Atoms-style multi-channel).
  Same composition pattern extends; awaiting need.
- Per-channel skin difference UI / tooling. Construct two children
  with different skins manually if needed.
- Vertical-split stereo (one channel above the other). The
  `splitHorizontal` helper is one-dimensional; a `splitVertical`
  helper is a one-line addition when needed.

## Files cited

- `widgets/src/meters/stereo.rs`
- `widgets/src/meters/mod.rs`
- `audio-meters-widgets/ts/src/stereo.ts`
- `audio-meters-widgets/ts/src/index.ts`

## Unblocks

- Live integration (AM-09) — driving stereo from a PCM source.
- Multi-channel composites (M/S, surround) — same pattern.

## Change log

- **2026-04-26** — Initial ratification (AM-08c). `MeterWidget` trait
  + `StereoPair` shipped on both runtimes; `splitHorizontal` helper
  for app-level layout reuse.
