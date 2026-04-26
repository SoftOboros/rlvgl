# @rlvgl/audio-meters-core

TypeScript companion port of `rlvgl-audio-meters-core`. Hand-written —
not wasm-bindgen — to keep the bundle tiny and the dev loop simple. See
[`docs/audio-meters/00-concepts.md`] §15 entry **2026-04-26-002** for the
decision rationale.

The Rust crate is the canonical implementation; this port follows. The
two are kept in sync via shared parity fixtures in
`audio-meters-core/fixtures/`. The Rust test suite generates expected
outputs; this suite asserts that the TS port matches them to within
`1e-4` dB.

## Surface

```ts
import {
  BallisticState,
  Ballistic,
  ALL_BALLISTICS,
  NEG_INFINITY_FLOOR_DB,
  applyCalibration,
} from "@rlvgl/audio-meters-core";

const meter = new BallisticState("Vu");
const reading = meter.update(/* dbfs */ -20.0, /* dt */ 1 / 60);
const displayDbu = applyCalibration(reading, /* offset_db */ 24.0);
```

`Ballistic` is a string-literal union over the variants frozen in
concepts §5.

## Tests

```sh
npm test
```

Runs `node --test --experimental-strip-types` over the fixture set.
Requires Node ≥ 22.6 for `--experimental-strip-types`.

## Out of scope

Same as the Rust crate — see its README. PCM acquisition, weighting,
RMS/peak detection, true-peak oversampling, sample-rate conversion are
the caller's responsibility.

[`docs/audio-meters/00-concepts.md`]: ../../docs/audio-meters/00-concepts.md
