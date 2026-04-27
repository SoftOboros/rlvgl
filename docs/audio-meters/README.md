<!--
README.md - Audio Meters initiative landing page. Informative.
Per-chapter docs (00-concepts.md and AM-NN chapters) are the
normative artifacts; this file points at them and gets new arrivals
oriented.
-->

# Audio Meters

A **layered, cross-runtime** asset and code hierarchy for VU-style
audio metering. Single source of truth for visual primitives (JSON
descriptors today; SVG / PNG via the aesthetics pass), parameter sets
(scales, skins), and ballistic math (`no_std` Rust + hand-ported TS).
Two first-class consumers: the rlvgl widget tree on Cortex-M /
embedded, and a vanilla-`<canvas>` custom-element library in the
browser.

## Quick start

### TypeScript / browser

```ts
import {
  defineRlvglLedBargraph,
  RlvglLedBargraphElement,
} from "@rlvgl/audio-meters-widgets";

defineRlvglLedBargraph();
// <rlvgl-led-bargraph
//     src-skin="/assets/audio-meters/skins/broadcast_classic_bargraph.json"
//     src-scale="/assets/audio-meters/scales/vu_broadcast.json"
//     width="64" height="320">
// </rlvgl-led-bargraph>

const meter = document.querySelector("rlvgl-led-bargraph") as RlvglLedBargraphElement;
audioWorklet.port.onmessage = (e) => meter.feed(e.data.dbfs);
```

Headless rendering core (Node-testable, framework-agnostic):

```ts
import { LedBargraphCore } from "@rlvgl/audio-meters-widgets";
const core = new LedBargraphCore({ scale, skin, showTicks: true });
core.update(/* dbfs */ -20.0, /* dt */ 1 / 60);
core.draw(canvasSink, 0, 0, 64, 320);
```

Run `npm run demo` in `audio-meters-widgets/ts/` to see all four
widgets driven by a synthetic 12 s dBFS sequence.

### rlvgl / no_std Rust

```rust
use rlvgl_widgets::meters::{
    LedBargraph, NeedleVu, NumericPeak, LufsGauge,
    presets::{
        BROADCAST_CLASSIC_BARGRAPH, BROADCAST_CLASSIC_NEEDLE,
        DIGITAL_STUDIO_NUMERIC,    LUFS_EBU_R128_GAUGE,
    },
};
use rlvgl_core::widget::Rect;

let mut bar = LedBargraph::new(
    Rect { x: 0, y: 0, width: 64, height: 320 },
    &BROADCAST_CLASSIC_BARGRAPH,
).with_ticks();

loop {
    let dbfs = pull_latest_dbfs();
    bar.update(dbfs, frame_dt);
    bar.draw(&mut renderer);
}
```

Run `cargo run --example audio_meters_console -p rlvgl-widgets` for
the parallel console demo.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ L5  application code                                              │
│       AudioWorklet  /  MicCapture  /  file player  →  dBFS        │
└──────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│ L4  composition  (audio-meters-widgets / widgets::meters::stereo) │
│       StereoPair<W: MeterWidget>                                   │
└──────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│ L3  widgets  (widgets::meters / @rlvgl/audio-meters-widgets)      │
│       LedBargraph    NeedleVu    NumericPeak    LufsGauge          │
└──────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│ L2  skins  (assets/audio-meters/skins/*.json)                      │
│       palette · layout · meter_type binding · optional assets      │
└──────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│ L1  scales  (assets/audio-meters/scales/*.json)                    │
│       range_db · pivot · zones · ticks · compatible_ballistics     │
└──────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│ L0  ballistics  (audio-meters-core)                                │
│       BallisticState — Vu, PpmTypeI/IIa/IIb, DigitalPeak, Rms,     │
│                         LufsM, LufsS, LufsI (BS.1770 abs-gated),   │
│                         Instant                                     │
└──────────────────────────────────────────────────────────────────┘
```

Each layer ships independently and is independently conformant. The
schemas at L1 / L2 are JSON-schema-validated in **both** runtimes;
divergence is a CI failure.

## Catalog

### Scales (L1)

| Scale id | Range | Pivot | Use case |
|---|---|---|---|
| `vu_broadcast` | -20 … +3 dBVU | 0 (=−20 dBFS) | US / SMPTE broadcast VU |
| `vu_ebu` | -18 … +3 dBVU | 0 (=−18 dBFS) | EBU broadcast VU |
| `ppm_din` | -50 … +5 dB | 0 (=−9 dBFS) | Nordic / DIN 45406 PPM |
| `ppm_iia_bbc` | 1 … 7 (BBC marks) | 4 (=−18 dBFS) | BBC PPM |
| `digital_peak` | -60 … 0 dBFS | 0 (=0 dBFS) | AES17 digital peak |
| `lufs_ebu_r128` | -36 … 0 LUFS | -23 LUFS | EBU R 128 broadcast loudness |
| `lufs_streaming_m14` | -36 … 0 LUFS | -14 LUFS | Spotify / YouTube / Apple Music streaming |

### Skins (L2)

| Skin id | Scale | Default ballistic | Widget family |
|---|---|---|---|
| `broadcast_classic_bargraph` | `vu_broadcast` | `Vu` | bargraph |
| `ebu_classic_bargraph` | `vu_ebu` | `Vu` | bargraph |
| `digital_studio_bargraph` | `digital_peak` | `DigitalPeak` | bargraph |
| `nordic_ppm_bargraph` | `ppm_din` | `PpmTypeI` | bargraph |
| `broadcast_classic_needle` | `vu_broadcast` | `Vu` | needle |
| `bbc_ppm_needle` | `ppm_iia_bbc` | `PpmTypeIIa` | needle |
| `digital_studio_numeric` | `digital_peak` | `DigitalPeak` | numeric |
| `lufs_ebu_r128_gauge` | `lufs_ebu_r128` | `LufsI` | lufs_gauge |
| `streaming_lufs_gauge` | `lufs_streaming_m14` | `LufsI` | lufs_gauge |

### Widgets (L3)

| Widget | Family | Renders | Owns |
|---|---|---|---|
| `LedBargraph` | bargraph | Background + N coloured cells + optional peak pip + optional tick strip | One `BallisticState`, peak-hold tracker |
| `NeedleVu` | needle | Background + radial needle line + pivot dot + optional arc ticks | One `BallisticState` |
| `NumericPeak` | numeric | Background + 2 text lines (reading + peak hold) | One `BallisticState`, peak-hold tracker |
| `LufsGauge` | lufs_gauge | Background + 3 text lines (I, S, M with LU deviation) | Three `BallisticState`s — first compound widget |

### Composition (L4)

`StereoPair<W: MeterWidget>` — generic two-channel container. Works
with any of the four widget families. See
[`09-stereo.md`](09-stereo.md).

## Integration recipes

See [`10-integration.md`](10-integration.md) for full code:

1. **rlvgl on H747I-DISCO** — `MicCapture::poll()` → peak detection
   → `bar.update()`. PCM acquisition + RMS / peak / weighted upstream
   stays in caller; widget only sees per-frame dBFS.
2. **Browser Web Audio + AudioWorklet** — detect dBFS in worklet
   thread, throttle to ~60 Hz, post to UI thread, feed via
   `element.feed()`.
3. **File / synthetic** — feed any `f32` directly. Both demos use
   this path.

## Conformance

A conforming deployment MUST satisfy the AM-00 §12 acceptance
checklist (vocabulary ratified) and the acceptance checklists of
whichever phase chapters its build includes. Each phase is
independently conformant once its checklist passes.

## Chapters

| Phase | Status | Doc |
|---|---|---|
| AM-00 — Concepts | Ratified 2026-04-26 | [00-concepts.md](00-concepts.md) |
| AM-01 — Core math (Rust) | Ratified 2026-04-26 | _(combined with AM-02; commit `2aa15ac`)_ |
| AM-02 — TS port of L0 | Ratified 2026-04-26 | _(combined with AM-01; commit `2aa15ac`)_ |
| AM-03 — Scale descriptors | Ratified 2026-04-26 | [03-scales.md](03-scales.md) |
| AM-04a — Skin descriptors | Ratified 2026-04-26 | [04-skins.md](04-skins.md) |
| AM-04b-stub — Asset hooks | Ratified 2026-04-26 | [13-asset-hooks.md](13-asset-hooks.md) |
| AM-04b — Visual primitives + creator rasterisation | Deferred | _(aesthetics pass)_ |
| AM-05 — `LedBargraph` (rlvgl) | Ratified 2026-04-26 | [05-led-bargraph.md](05-led-bargraph.md) |
| AM-06 — `LedBargraph` (TS) | Ratified 2026-04-26 | [05-led-bargraph.md](05-led-bargraph.md) (combined) |
| AM-07 — `NeedleVu` (both) | Ratified 2026-04-26 | [06-needle-vu.md](06-needle-vu.md) |
| AM-08a — `NumericPeak` (both) | Ratified 2026-04-26 | [07-numeric-peak.md](07-numeric-peak.md) |
| AM-08b — Ticks + labels | Ratified 2026-04-26 | [08-ticks-labels.md](08-ticks-labels.md) |
| AM-08c — Stereo composition | Ratified 2026-04-26 | [09-stereo.md](09-stereo.md) |
| AM-08d — LUFS gauge | Ratified 2026-04-26 | [11-lufs-gauge.md](11-lufs-gauge.md) |
| AM-08e — LufsI absolute gating | Ratified 2026-04-26 | [12-lufs-gating.md](12-lufs-gating.md) |
| AM-08h — BS.1770 relative gating | Ratified 2026-04-26 | [14-bs1770-relative-gating.md](14-bs1770-relative-gating.md) |
| AM-08i — LufsGaugeStrict widget | Ratified 2026-04-26 | _(commit `4ab8930` follow-up)_ |
| AM-08f — Streaming LUFS scale | Ratified 2026-04-26 | _(commit `9d873d1`)_ |
| AM-08g — PPM skin coverage | Ratified 2026-04-26 | _(commit `7ad4724`)_ |
| AM-09 — Integration | Ratified 2026-04-26 | [10-integration.md](10-integration.md) |

## Test surface

The initiative ships with end-to-end test coverage on both runtimes:

- **Rust**: `cargo test -p rlvgl-audio-meters-core -p rlvgl-widgets`
  exercises L0 unit tests, parity fixtures, scale + skin validators,
  and integration test (driving 4 widgets through a 480-frame
  sequence). 28 test sections.
- **TS L0**: `npm test --prefix audio-meters-core/ts` runs 30 cross-
  runtime parity tests + scale + skin validators. 46 tests.
- **TS widgets**: `npm test --prefix audio-meters-widgets/ts` runs
  per-widget headless render tests + stereo + LUFS gauge +
  integration test. 29 tests.

Cross-runtime parity is enforced via shared fixtures under
`audio-meters-core/fixtures/`: Rust generates expected sequences;
TS asserts match within `1e-4` dB. The full Cortex-M7 cross-compile
runs on every CI build.

## Reference

- [`CLAUDE.md` § Spec-Before-Code Planning Discipline](../../CLAUDE.md)
- **Standards**: IEC 60268-10 (PPM), IEC 60268-17 (VU), AES17 (digital
  level), ITU-R BS.1770-4 (loudness), EBU R 128 (production target).
- RFC 2119 / 8174 keyword interpretation in normative chapter
  sections.

This index is **informative**. Normative content lives in the
per-chapter docs.
