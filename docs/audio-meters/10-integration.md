<!--
10-integration.md - AM-09: kitchen-sink integration test + recipes for
wiring meters to real audio sources.
-->

**[← Prev AM-08c](09-stereo.md) · [Index](README.md)**

# AM-09 — Integration

The AM-00…AM-08c layering is now end-to-end provable: this chapter
adds a kitchen-sink integration test exercising every widget family
together under a synthetic signal, and documents how to wire the
meters to real audio sources on each runtime.

## What ships

| Path | Role |
|---|---|
| `widgets/tests/audio_meters_integration.rs` | Builds one of each widget family + a stereo `LedBargraph` pair, drives them through a 480-frame synthetic dBFS sequence (silence → ramp → plateau → impulse → silence), renders every 30th frame, asserts mid-plateau tracking + stereo asymmetry + final-state plausibility. |
| `audio-meters-widgets/ts/test/integration.test.ts` | TS mirror. Same sequence, same assertions. |

Both runtimes also assert that swapping the ballistic on a running
widget resets it to the floor and that a fresh trajectory begins.

## Wiring recipes

The test fixtures use synthetic dBFS. Real applications feed the
widget from one of three upstream paths:

### 1. rlvgl on H747I-DISCO — `MicCapture`

```rust
use rlvgl_platform::mic_capture::MicCapture;
use rlvgl_widgets::meters::{LedBargraph, presets::DIGITAL_STUDIO_BARGRAPH};
use rlvgl_core::widget::Rect;

let mut mic = MicCapture::init(/* SAI4 + BDMA handles */);
let mut bar = LedBargraph::new(
    Rect { x: 0, y: 0, width: 64, height: 320 },
    &DIGITAL_STUDIO_BARGRAPH,
);

let mut pcm = [0i16; 256];
let frame_dt_s = 1.0 / 60.0;
loop {
    let n = mic.poll(&mut pcm);
    let dbfs = pcm_block_to_dbfs(&pcm[..n]);
    bar.update(dbfs, frame_dt_s);
    bar.draw(&mut renderer);
}

/// Convert a block of i16 PCM samples to a single dBFS value.
/// Caller's choice of detection: peak, RMS, K-weighted RMS — see
/// concepts §9 ("widget update contract"). The widget owns the
/// ballistic; everything *upstream* of dBFS is the caller's job.
fn pcm_block_to_dbfs(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return -120.0;
    }
    let peak = samples
        .iter()
        .map(|s| s.unsigned_abs() as u32)
        .max()
        .unwrap_or(0);
    if peak == 0 {
        return -120.0;
    }
    let normalized = peak as f32 / 32768.0;
    20.0 * libm::log10f(normalized)
}
```

The `MicCapture` ↔ widget call sites must agree on a dBFS detection
scheme. **Peak** detection is simplest and pairs naturally with
`Ballistic::DigitalPeak`; **RMS** detection pairs with `Ballistic::Vu`
or `Ballistic::Rms`. Switching between them is purely a caller
choice; the widget cares about neither.

### 2. Browser — Web Audio + AudioWorklet

```ts
import {
  RlvglLedBargraphElement,
  defineRlvglLedBargraph,
} from "@rlvgl/audio-meters-widgets";

defineRlvglLedBargraph();
const meter = document.querySelector("rlvgl-led-bargraph") as RlvglLedBargraphElement;

// AudioWorklet posts dBFS at audio-rate; we throttle to ~60 Hz on
// the UI thread to match the rAF cadence the element runs at.
const ctx = new AudioContext();
await ctx.audioWorklet.addModule("/dbfs-worker.js");
const node = new AudioWorkletNode(ctx, "dbfs-detector");
node.port.onmessage = (e) => meter.feed(e.data.dbfs);
```

Companion AudioWorklet (`dbfs-worker.js`):

```js
class DbfsDetector extends AudioWorkletProcessor {
  constructor() {
    super();
    this.lastPost = 0;
  }
  process(inputs) {
    const ch = inputs[0]?.[0];
    if (!ch || ch.length === 0) return true;
    let peak = 0;
    for (let i = 0; i < ch.length; i++) {
      const a = Math.abs(ch[i]);
      if (a > peak) peak = a;
    }
    const dbfs = peak > 0 ? 20 * Math.log10(peak) : -120;
    // Throttle to ~60 Hz to match rAF cadence.
    if (currentTime - this.lastPost > 1 / 60) {
      this.port.postMessage({ dbfs });
      this.lastPost = currentTime;
    }
    return true;
  }
}
registerProcessor("dbfs-detector", DbfsDetector);
```

### 3. File playback / synthesised signal

For demos, tests, or "show me what 0 VU looks like" tooling, feed the
meter a synthetic sequence directly — both runtimes accept any `f32`
dBFS value. The integration tests use this path; see them for a
canonical pattern:

- Rust: `widgets/tests/audio_meters_integration.rs`
- TS: `audio-meters-widgets/ts/test/integration.test.ts`

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `MicCapture` (`platform/src/mic_capture.rs`) | Caller, not widget concern. The recipe above shows the documented integration path: `MicCapture::poll()` → `pcm_block_to_dbfs()` → `bar.update()`. |
| AudioWorklet | Caller, not widget concern. The AudioWorklet computes dBFS on the audio thread and posts to the UI thread; the custom element consumes via `feed()`. |
| Concepts §9 (widget update contract) | This chapter is the operational realisation of §9 — concrete code paths feeding `dbfs` per frame to the widget. |
| `rlvgl-core::Renderer` | The integration test uses a counting `Renderer` — real apps use the platform-supplied renderer. The widget code is the same. |

## Acceptance checklist

- [x] Rust integration test passes: builds 4 widgets, drives 480
      frames, asserts mid-plateau tracking + stereo asymmetry.
- [x] TS integration test passes: same 4-widget assembly, same
      sequence, same assertions.
- [x] Three integration recipes documented (MicCapture, AudioWorklet,
      synthesised).
- [x] Cortex-M7 cross-compile clean (recipe uses `libm::log10f`,
      already a workspace dep).

## Non-goals

- A full demo binary on H747I-DISCO. The hardware integration recipe
  above is the documented path; building a demo example is out of
  scope for the layering initiative and would belong with the
  audio-feature work.
- A WebAudio playback example on a hosted page. The AudioWorklet
  recipe above is the documented path; a live demo page is presentation
  work, not part of the layering.
- Recipe variants for AVR / RP2040 / ESP32 platforms. The pattern
  generalises — replace `MicCapture` with the platform's mic
  abstraction. AM-04b's `rlvgl-creator` codegen will help downstream
  BSPs adopt this without copy-paste.

## Files cited

- `widgets/tests/audio_meters_integration.rs`
- `audio-meters-widgets/ts/test/integration.test.ts`
- `platform/src/mic_capture.rs` (caller side; not modified)
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §9
- [`docs/audio-meters/05-led-bargraph.md`](05-led-bargraph.md)
- [`docs/audio-meters/06-needle-vu.md`](06-needle-vu.md)
- [`docs/audio-meters/07-numeric-peak.md`](07-numeric-peak.md)
- [`docs/audio-meters/09-stereo.md`](09-stereo.md)

## Unblocks

The audio-meters initiative reaches a useful end-to-end state with
this chapter: every layer (L0 ballistics → L1 scales → L2 skins → L3
widgets → L4 stereo composition) has cross-runtime validation, and
the three documented integration paths cover the realistic deployment
shapes. Aesthetics work (AM-04b graphical primitives, AM-08d LUFS
gauge, more skin presets) is the natural follow-up phase, layering
on top of the descriptors and the widget surface without disturbing
either.

## Change log

- **2026-04-26** — Initial ratification (AM-09). Kitchen-sink
  integration test on both runtimes; three integration recipes
  documented. Initiative reaches end-to-end useful state.
