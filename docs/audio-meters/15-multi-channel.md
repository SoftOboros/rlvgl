<!--
15-multi-channel.md - AM-10: parametric multi-channel composite.
Foundation for stereo, 5.1, graphic EQ, and any other fixed-N layout.
-->

**[← Prev AM-08i](14-bs1770-relative-gating.md) · [Index](README.md)**

# AM-10 — Parametric Multi-Channel Composite

Generalises [`StereoPair<W>`](09-stereo.md) to N channels of any
[`MeterWidget`]. The `MultiChannel<W, N>` container is the
foundation for:

- **Stereo** (`N = 2`)
- **5.1 surround** (`N = 6`: L, R, C, LFE, Ls, Rs)
- **7.1 surround** (`N = 8`: + Lr, Rr)
- **Graphic EQ** (`N = 8 / 16 / 32` bands)
- **Multi-band loudness** (`N = 3` for low / mid / high broadcast bands)
- Any other fixed-channel-count layout

The container is type-generic over the meter widget *and* const-
generic over the channel count on the Rust side, so storage is
`[W; N]` — no allocation. The TypeScript mirror takes the channel
count as a constructor argument and uses an array.

## Authority

- AM-08c [`StereoPair<W>`](09-stereo.md) — predecessor; covers
  `N = 2` with a slightly more ergonomic API.
- `MeterWidget` trait (concepts §-aligned through AM-08c).
- No new schema; pure composition.

## What ships

| Path | Role |
|---|---|
| `widgets/src/meters/multi_channel.rs` | `MultiChannel<W, N>` struct. Stores `[W; N]`; partitions outer bounds horizontally / vertically; forwards per-channel `update_n` / sparse `update_at`; implements `Widget` (draws all children). Helpers: `split_horizontal_n<N>`, `split_vertical_n<N>`. |
| `audio-meters-widgets/ts/src/multi-channel.ts` | TS mirror. `MultiChannel<C extends MeterCoreLike>`. Channel count is runtime; helpers `splitHorizontalN`, `splitVerticalN`. |

## Surface

### Rust

```rust
use rlvgl_widgets::meters::{
    LedBargraph, MultiChannel, presets::DIGITAL_STUDIO_BARGRAPH,
};
use rlvgl_core::widget::Rect;

let outer = Rect { x: 0, y: 0, width: 480, height: 320 };

// 5.1 surround: 6 channels, all using the digital-peak skin.
let mut surround: MultiChannel<LedBargraph, 6> =
    MultiChannel::from_horizontal_factory(outer, 4, |_idx, b| {
        LedBargraph::new(b, &DIGITAL_STUDIO_BARGRAPH)
    });

let dt = 1.0 / 60.0;
loop {
    let dbfs: [f32; 6] = pull_5_1_dbfs();   // [L, R, C, LFE, Ls, Rs]
    surround.update_n(&dbfs, dt);
    surround.draw(&mut renderer);
}
```

Sparse update path for graphic EQ:

```rust
let mut eq: MultiChannel<LedBargraph, 16> =
    MultiChannel::from_horizontal_factory(outer, 2, |_, b| {
        LedBargraph::new(b, &DIGITAL_STUDIO_BARGRAPH)
    });

// Band filters can run at different rates; push each as it produces.
eq.update_at(band_idx, band_dbfs, dt);
```

### TypeScript

```ts
import { MultiChannel, LedBargraphCore } from "@rlvgl/audio-meters-widgets";

const outer = { x: 0, y: 0, w: 480, h: 320 };
const surround = MultiChannel.fromHorizontalFactory(
  outer, 4, 6,
  (_idx, _b) => new LedBargraphCore({ scale, skin }),
);

audioWorklet.port.onmessage = (e) => {
  surround.updateN(e.data.dbfs6, e.data.dt);   // [L, R, C, LFE, Ls, Rs]
};
// In your rAF loop: surround.draw(canvasSink);
```

## Layout helpers

`split_horizontal_n<N>` / `split_vertical_n<N>` partition an outer
`Rect` into `N` equally-sized child rects with a configurable gap.
The last child absorbs any rounding remainder so the outer width
(or height) is exactly partitioned.

Public so callers that prefer to manage their own children inside
an `rlvgl_core::Container` can reuse the geometry split without
instantiating `MultiChannel`.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `StereoPair<W>` (AM-08c) | Predecessor for `N = 2`. Slightly more ergonomic for stereo (`update_stereo(l, r, dt)` vs. `update_n(&[l, r], dt)`); both shipped, neither deprecates the other. |
| `MeterWidget` trait | Required as the type bound on `W`. All four first-party meters implement it. |
| `rlvgl_core::Widget` | `MultiChannel<W, N>` itself implements `Widget`, so the rlvgl widget tree treats a multi-channel composite as a single child. |
| Skin layering | All children share one skin in typical usage (5.1 / EQ / etc.). The factory closure can vary skin per channel for unusual layouts (e.g. coloured "centre channel" highlighted differently). |
| `LufsGauge` / `LufsGaugeStrict` | Implements `MeterWidget`? Currently does not — the LUFS gauges have their own update signature. Adding the trait is straightforward when stereo / multi-channel loudness becomes a use case; out of scope here. |

## Acceptance checklist

- [x] `MultiChannel<W: MeterWidget, const N: usize>` Rust type
      with `from_horizontal_factory`, `from_vertical_factory`,
      `new`, `update_n`, `update_at`, `reset`, `channels`,
      `channel`, `channel_mut`.
- [x] `split_horizontal_n<N>` and `split_vertical_n<N>` helpers.
- [x] TS `MultiChannel<C>` with runtime channel count + parallel
      helpers (`splitHorizontalN`, `splitVerticalN`).
- [x] Six tests on each side: 5.1 surround forwarding, 8-band EQ
      with sparse updates, partition exactness (h + v), reset
      floors all channels, channel-mut for per-channel ballistic
      swap, draw counts match expected child ops.
- [x] Cortex-M7 cross-compile clean — `[W; N]` storage works in
      `no_std`.

## Non-goals (deferred)

- Mixed-widget composites (e.g. needle for L/R, numeric for centre).
  Doable today by composing two separate `MultiChannel`s; a
  heterogeneous container would need erasure (`Box<dyn Widget>`)
  and `alloc`.
- Channel labels (e.g. "L R C LFE Ls Rs" annotation strip). Compose
  separate text drawing alongside the `MultiChannel`; the container
  intentionally stays focused on layout + forwarding.
- Auto-orientation (vertical bargraphs in horizontal-split layouts
  vs. horizontal bargraphs in vertical-split). The `Skin.layout.orientation`
  is per-skin, not per-composite; the container places children at
  whatever sub-bounds the partition produces.
- Channel groups (e.g. front L+R+C grouped tighter than rear Ls+Rs).
  Compose two `MultiChannel`s side-by-side at the application
  level.
- LUFS-gauge `MeterWidget` impl. Loudness is conventionally
  programme-level not per-channel; will land if/when stereo or
  surround LUFS becomes a real use case.

## Files cited

- `widgets/src/meters/multi_channel.rs`
- `widgets/src/meters/stereo.rs` (predecessor)
- `audio-meters-widgets/ts/src/multi-channel.ts`
- `audio-meters-widgets/ts/test/multi-channel.test.ts`
- [`docs/audio-meters/09-stereo.md`](09-stereo.md)

## Change log

- **2026-04-26** — Initial ratification (AM-10). `MultiChannel<W, N>`
  shipped on both runtimes. 6 unit tests on each side covering
  5.1 surround, 8-band graphic EQ, partition helpers, reset, and
  per-channel mutation. The audio-meters initiative now supports
  arbitrary fixed-channel-count meter layouts as a first-class
  composition primitive.
