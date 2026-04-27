# @rlvgl/audio-meters-widgets

Vanilla-`<canvas>` custom-element widgets for the rlvgl audio-meters
initiative. Browser-only; depends on `@rlvgl/audio-meters-core` for
ballistic state. Mirror of `widgets/src/meters/` on the rlvgl Rust
side.

## Layering

- **L0** (`@rlvgl/audio-meters-core`): ballistics + dB calibration. No
  DOM dependencies.
- **L1 / L2** (`assets/audio-meters/{scales,skins}/`): JSON descriptors
  loaded at runtime via `fetch`.
- **L3** (this package): canvas rendering core (Node-testable) plus
  `<rlvgl-led-bargraph>` custom-element wrapper.

## Surface

```ts
import {
  LedBargraphCore,
  RlvglLedBargraphElement,
  defineRlvglLedBargraph,
} from "@rlvgl/audio-meters-widgets";

// Headless rendering (e.g. for tests):
const core = new LedBargraphCore({ scale, skin });
core.update(/* dbfs */ -20.0, /* dt */ 1 / 60);
core.draw(sink, /* x */ 0, /* y */ 0, /* w */ 64, /* h */ 320);

// Browser:
defineRlvglLedBargraph();
// <rlvgl-led-bargraph
//     src-skin="/assets/audio-meters/skins/broadcast_classic_bargraph.json"
//     src-scale="/assets/audio-meters/scales/vu_broadcast.json"
//     width="64" height="320">
// </rlvgl-led-bargraph>
const meter = document.querySelector("rlvgl-led-bargraph");
meter.feed(/* dbfs */ -20.0); // call from your AudioWorklet bridge
```

## Tests

```sh
npm test
```

Headless: `LedBargraphCore` is decoupled from canvas via `DrawSink` so
tests run under Node `--experimental-strip-types`.
