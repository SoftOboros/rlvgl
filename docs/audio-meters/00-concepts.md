<!--
00-concepts.md - Audio Meters initiative §0 concepts gate.
Load-bearing: this doc freezes the vocabulary, enums, JSON schema,
and source-of-truth map for all subsequent AM-NN phases.
-->

**[Index](README.md) · [Next → AM-01 core math](01-core-math.md)**

# AM-00 — Audio Meters Concepts

This is the §0 concepts gate for the **audio-meters** initiative. Per the
spec-before-code discipline in `CLAUDE.md`, no behaviour PR ships against an
unratified concepts doc; vocabulary changes happen here first, in a separate PR
with a dated change-log entry.

The audio-meters family ships a layered, **cross-runtime** asset and code
hierarchy for VU-style metering: a `no_std` Rust core (rlvgl side), a
hand-written TypeScript port (browser / Tauri side), and a single shared source
tree of visual assets and parameter descriptors that both runtimes consume.

## §0 — Authority policy

The standards listed below are normative for the named concept. Where they
conflict with general "VU meter folklore", the standards win.

| Concept | Authoritative document |
|---|---|
| VU meter ballistic | **IEC 60268-17** (Sound system equipment, Part 17: Standard volume indicators) |
| PPM Type I (DIN), PPM Type IIa (BBC), PPM Type IIb (EBU) | **IEC 60268-10** (Peak programme level meters) |
| Digital level meters, dBFS reference | **AES17-1998 (r2009)** §3.12 (full-scale digital level), §6.2 (peak meters) |
| Loudness — LUFS / LU, momentary / short-term / integrated, gating | **ITU-R BS.1770-4** ; **EBU R 128** (production practice) |
| True-peak detection (4× oversampling) | **ITU-R BS.1770-4** Annex 2 |
| Calibration references — dBu (0.775 Vrms), dBV (1 Vrms), dBSPL (20 µPa) | **IEC 60268-2** ; **IEC 61672** (sound level meters) |
| K-weighting filter coefficients | **ITU-R BS.1770-4** §3 |
| RFC 2119 / 8174 keyword interpretation | **RFC 2119**, **RFC 8174** |

When a concepts-doc statement uses **MUST**, **MUST NOT**, **SHALL**,
**SHOULD**, **MAY**, or **RECOMMENDED** in capitals, it is invoking RFC 2119.
Lowercase use is ordinary English.

## §1 — Purpose

Provide a single, layered specification for the visible and computable parts of
audio metering — the kind of meter you put on a recording-console UI, a
broadcast loudness panel, or an embedded stompbox display. Specifically:

- A **frozen vocabulary** for ballistics, scales, and colour zones, so the
  rlvgl-side widget and the TS-side widget cannot drift on what `vu_broadcast`
  or `ppm_iia` means.
- A **JSON descriptor schema** for scales and skins that both runtimes parse
  identically.
- A **widget API contract** that pins where ballistics live (in the widget),
  where calibration lives (display-time additive offset), and what the caller
  is responsible for (everything upstream of a per-frame dBFS sample).
- A **source-of-truth map** so that any term used in code (Rust struct, TS
  type, JSON field) traces to exactly one canonical definition.

## §2 — Problem statement

The naïve approach — "just paint some green/yellow/red rectangles in proportion
to the audio level" — silently breaks in three places that have bitten the
audio-tooling community for decades:

1. **Ballistic ambiguity.** A "VU meter" with no specified ballistic is a
   bargraph. Without IEC 60268-17 integration time the meter under-reads
   transients and over-reads sustained tones; users learn distrust. PPM
   variants disagree by an order of magnitude in attack time alone.
2. **Calibration drift.** Studios calibrate 0 VU = +4 dBu = -20 dBFS (US
   broadcast), or 0 VU = -18 dBFS (EBU), or other house references. A meter
   that bakes in one assumption is wrong in the other room. Calibration MUST
   be a display-time additive offset, not part of the ballistic.
3. **Cross-runtime drift.** rlvgl-side (`no_std` Rust) and the TS-side
   (browser, AudioWorklet, Tauri-hosted designer) implementations of the same
   meter MUST produce visually identical results given the same dBFS input
   sequence. Without a single source of truth for both ballistic state machines
   and visual descriptors, the two ports diverge silently within weeks.

The first symptom is "the meter reads differently between the embedded board
and the designer preview". This concepts doc exists to make that class of
divergence a compile-time / schema-validation failure rather than a runtime
surprise.

## §3 — Canonical glossary

Each term is defined once. Code definitions are cited with the relationship
marker from `CLAUDE.md` § Spec-Before-Code Planning Discipline (*used without
modification* / *adapted: <delta>* / *owned by AM-NN; does not exist in repo
yet*).

- **Sample** — A single audio amplitude value at a point in time. Type-erased
  at the meter API boundary; callers convert to dBFS upstream. *Owned by
  AM-00; not represented in code (callers' types).*
- **dBFS** — Decibels relative to digital full scale. `0 dBFS` is the maximum
  representable sample magnitude. Per AES17, `0 dBFS` is the level of a
  full-scale sine wave (peak = 1.0 in normalised float). All meter inputs are
  expressed in dBFS unless explicitly noted. *Owned by AM-00; will be a
  newtype `Dbfs(f32)` in `audio-meters-core`.*
- **dBu** — `20 · log10(Vrms / 0.7746)`. Reference for analog gear. The
  meter does not convert; it adds a calibration offset at display time so the
  scale labels read in dBu while internal state stays in dBFS.
- **dBV**, **dBSPL** — Same pattern as dBu, different reference. Pure display
  offsets.
- **Ballistic** — A state machine that converts a stream of `(dBFS, dt)`
  samples into a slowly-changing displayed value (the *meter reading*). The
  ballistic owns the integration / attack / decay behaviour. Frozen variants
  in §5. *Owned by AM-00; will be enum `Ballistic` in `audio-meters-core`.*
- **Meter reading** — The current displayed dBFS value emitted by a ballistic
  after applying its attack/decay. Same units as input (dBFS). Distinct from
  the *displayed dBu/dBV/dBSPL* which is the meter reading plus calibration
  offset.
- **Peak hold** — A separate, slower-decaying state that tracks the
  short-term maximum of the meter reading. Optional; widgets MAY display it
  as a frozen pip on top of the bargraph or as a separate LED.
- **Scale** — A named description of a meter's visible value range, tick
  marks, label text, and colour-zone boundaries. Pure data, expressed as
  JSON. Frozen scale identifiers in §6. The same Scale can be reused across
  Ballistic variants.
- **Skin** — A named binding of (Scale, Ballistic, asset-set, layout). Pure
  data, JSON. Determines what a meter widget *looks* like and *feels* like
  without changing its API.
- **Asset** — A visual primitive (SVG vector, PNG raster, or both) referenced
  by a Skin. Stored in the shared asset package; rasterised at build time on
  the rlvgl side, imported directly on the TS side.
- **Calibration offset** — A scalar `f32` in dB added to the meter reading at
  display time to translate dBFS labels into dBu/dBV/dBSPL/etc. Never
  affects ballistic state.
- **Frame update** — A single call to `meter.update(dbfs, dt)` at display
  refresh rate (typically 60 Hz). The widget assumes one update per displayed
  frame; it does not interpolate or pump internal sub-steps.
- **Caller** — Whatever code feeds the meter dBFS samples. On rlvgl this is
  the application loop driven by `MicCapture` or playback; on TS this is an
  `AudioWorklet` posting messages to the UI thread. Callers are responsible
  for everything upstream of dBFS: PCM acquisition, RMS / peak detection,
  weighting (A / C / K), true-peak oversampling, and rate normalisation.

## §4 — Source-of-truth map

One owner per concept. Implementations in other layers reference, never
restate.

| Concept | Owner | Mirrored in |
|---|---|---|
| `Ballistic` enum | `audio-meters-core` (Rust) | `@rlvgl/audio-meters` TS port (hand-ported, kept in sync via `parity_*` test fixtures) |
| `Scale` identifier set | `assets/audio-meters/scales/*.json` (one file per scale) | Rust loads via `serde_json` at build time into `const`; TS imports JSON directly |
| `Skin` identifier set | `assets/audio-meters/skins/*.json` | Same dual-consume as Scale |
| Visual primitives (bezel, needle, LED segment, faceplate) | `assets/audio-meters/svg/*.svg` and `assets/audio-meters/png/*.png` | rlvgl-creator rasterises SVG → RLE for embedded; TS imports SVG/PNG directly |
| Ballistic time constants | This doc, §5 | Rust constants in `audio-meters-core::ballistic`; TS constants in `@rlvgl/audio-meters/ballistic.ts`. Both MUST cite this doc's section in a comment. |
| `MeterColor` enum (zone colour identifiers) | This doc, §7 | Rust enum + TS string-literal union |
| Scale-descriptor JSON schema | This doc, §8 (informal) + `assets/audio-meters/schema/scale.schema.json` (canonical) | `schemars` validation Rust-side; `ajv` (or equivalent) TS-side |
| Widget update contract | This doc, §9 | rlvgl `widgets/src/meters/*.rs`; TS `@rlvgl/audio-meters-widgets/*` |

## §5 — Frozen enum: `Ballistic`

Registration policy: **Standards Action**. Adding or removing a variant
requires a §15 change-log entry and explicit owner go-ahead, in a PR separate
from any behaviour change.

| Variant | Time constants | Authority |
|---|---|---|
| `Vu` | First-order envelope follower on linear amplitude. Rise to **99 %** of a steady-state step in **300 ms ± 10 %**. Symmetric attack / decay. Equivalent first-order τ ≈ 65 ms in linear-amplitude domain. | IEC 60268-17 |
| `PpmTypeI` (DIN 45406) | Linear-amplitude attack: reaches **1 dB below** steady tone in **5 ms** (τ ≈ 2.26 ms). Decay: **20 dB / 1.5 s**, linear in dB. | IEC 60268-10 |
| `PpmTypeIIa` (BBC) | Linear-amplitude attack: reaches 1 dB below in **10 ms** (τ ≈ 4.52 ms). Decay: **24 dB / 2.8 s**, linear in dB. | IEC 60268-10 |
| `PpmTypeIIb` (EBU) | Linear-amplitude attack: 10 ms (τ ≈ 4.52 ms). Decay: **20 dB / 1.7 s**, linear in dB. | IEC 60268-10 |
| `DigitalPeak` | Attack: instantaneous (one sample). Decay: **20 dB / 1.5 s** (matches DIN PPM decay so two meters agree on transients). | AES17 §6.2 |
| `Rms` | Sliding-window RMS, **400 ms** window. No attack/decay asymmetry. | Convention; documented in this doc. |
| `LufsM` | ITU-R BS.1770-4 momentary loudness, **400 ms** sliding window, K-weighted. **Note:** K-weighting is the caller's job; the meter sees post-weighted dBFS. | ITU-R BS.1770-4 |
| `LufsS` | Short-term loudness, **3 s** sliding window. Caller K-weights. | ITU-R BS.1770-4 |
| `LufsI` | Integrated loudness, gated, full programme. Caller K-weights. | ITU-R BS.1770-4 |
| `Instant` | Zero ballistic; reading == input. For test fixtures and debug overlays. | n/a |

Each variant in `audio-meters-core` MUST cite the corresponding §5 row in a
doc comment. The TS port's `parity_<variant>.json` fixture pins the Rust
output for a canonical input sequence; the TS port's test compares its own
output to the same fixture.

## §6 — Frozen enum: `Scale`

Registration policy: **Specification Required** (per-chapter walkthrough
update; no concepts-doc amendment needed for new entries unless they change
the schema in §8).

Initial scale set:

| Identifier | Range (dB) | Pivot | Notes |
|---|---|---|---|
| `vu_broadcast` | −20 … +3 dBVU | 0 VU = −20 dBFS | US broadcast / SMPTE convention. |
| `vu_ebu` | −18 … +3 dBVU | 0 VU = −18 dBFS | EBU convention. |
| `ppm_din` | −50 … +5 dB | 0 dB = −9 dBFS | DIN 45406 / IEC Type I labels. |
| `ppm_iia_bbc` | 1 … 7 (BBC marks) | "4" = −18 dBFS test level | BBC convention; non-dB labels. |
| `digital_peak` | −60 … 0 dBFS | 0 dBFS = full scale | AES17 digital. |
| `lufs_ebu_r128` | −36 … 0 LUFS | −23 LUFS target | EBU R 128. |

Adding a scale: drop a JSON file under `assets/audio-meters/scales/`,
add a row to a per-chapter walkthrough (likely AM-03), no §15 churn.

## §7 — Frozen enum: `MeterColor`

Registration policy: **Standards Action**. Colour zone *identifiers* are
fixed; their concrete RGB / palette indices are skin data, not part of the
enum.

| Identifier | Conventional usage |
|---|---|
| `Safe` | "Green" — well below alignment level. |
| `Nominal` | Around alignment level. Often green or amber depending on skin. |
| `Caution` | Approaching headroom limit. Amber / yellow conventionally. |
| `Hot` | At or above safe peak threshold. Red conventionally. |
| `Over` | Clipped / illegal level. Bright red / flashing conventionally. |

Concrete colours, gradients, and per-LED palette indices live in the **skin**
JSON, not here. A scale's colour-zone boundaries (in dB) reference these
identifiers; the skin maps identifiers to drawable colours.

## §8 — Scale-descriptor JSON schema (informal)

The canonical schema lives at `assets/audio-meters/schema/scale.schema.json`
once AM-03 lands. The shape below is the §0 contract; AM-03 ratifies the
canonical JSON Schema document.

```json
{
  "id": "vu_broadcast",
  "label_units": "dBVU",
  "range_db": { "min": -20.0, "max": 3.0 },
  "pivot": { "label": "0", "input_dbfs": -20.0 },
  "calibration_default": { "to": "dBu", "offset_db": 4.0 },
  "ticks": {
    "majors": [-20, -10, -7, -5, -3, -1, 0, 1, 2, 3],
    "minors_per_major_division": 4,
    "labels": {
      "-20": "−20", "-10": "−10", "-7": "−7", "-5": "−5",
      "-3": "−3", "-1": "−1", "0": "0", "1": "+1", "2": "+2", "3": "+3"
    }
  },
  "zones": [
    { "from_db": -20.0, "to_db": -3.0, "color": "Safe" },
    { "from_db": -3.0,  "to_db":  0.0, "color": "Nominal" },
    { "from_db":  0.0,  "to_db":  1.0, "color": "Caution" },
    { "from_db":  1.0,  "to_db":  3.0, "color": "Hot" }
  ],
  "compatible_ballistics": ["Vu", "Rms", "DigitalPeak", "Instant"]
}
```

Required fields: `id`, `label_units`, `range_db`, `pivot`, `ticks`, `zones`,
`compatible_ballistics`. Optional: `calibration_default` (skins MAY override).

`compatible_ballistics` is advisory — a skin MAY pair any ballistic with any
scale, but the validator warns when crossing the advisory boundary (e.g.
`PpmTypeI` on `vu_broadcast` is unconventional).

## §9 — Widget update contract

Both runtimes MUST honour:

- **Per-frame update.** `meter.update(dbfs: f32, dt: f32)` is called once per
  displayed frame. The widget MAY assume `dt` is in seconds and bounded
  (typical: `0.008 < dt < 0.05`).
- **dBFS in, dBFS out internally.** All ballistic state is dBFS-domain.
  Calibration to dBu / dBV / dBSPL is a display-time additive offset applied
  when rendering tick labels and numeric readouts; it does not enter the
  ballistic state machine.
- **Stateless caller.** The caller MUST NOT keep ballistic state. If the
  caller wants two visualisations of the same signal (e.g. VU + PPM side by
  side), it instantiates two meters and feeds both the same dBFS each frame.
- **Idempotent paint.** `meter.draw()` MUST be derivable from internal state
  alone; the widget MUST NOT depend on having received a particular sequence
  of `update()` calls within the same frame.
- **Reset.** `meter.reset()` MUST clear ballistic state, peak hold, and any
  hold-decay timers. Used for transport stop, channel-strip recall, etc.
- **Sample-pump variant (optional).** A widget MAY expose
  `meter.update_block(samples: &[Dbfs], block_dt: f32)` for callers that
  prefer to push a block per audio buffer instead of a single value per
  frame; the default implementation iterates `update(dbfs, block_dt /
  samples.len())`.

## §10 — Reconciliation with adjacent rlvgl primitives

| rlvgl primitive | Relationship to audio-meters |
|---|---|
| `widgets::ProgressBar` | **Not reused.** ProgressBar fills statically with no ballistic. A bargraph meter is closer to "a Style + a Scale + a Ballistic + per-LED segment art" and warrants its own widget. |
| `widgets::Slider` | **Not reused.** Sliders are interactive; meters are read-only. The hit-test path differs. |
| `widgets::motion` | **Pattern reused.** External buffer + trait composition is the right pattern for ballistic state owned by app code that the widget borrows; we adopt the same lifetime discipline. |
| `rlvgl-creator` asset pipeline | **Extended.** AM-04 adds a `meters from-yaml` (or equivalent) subcommand that rasterises the SVG/PNG asset set into per-target RLE blobs. The chipdb-style YAML schema does not change. |
| `rlvgl-decomp` | **Reused unmodified.** RLE blobs produced by AM-04 are decoded with the existing decoder; no format extension. |
| `MicCapture` (`platform/src/mic_capture.rs`) | **Caller, not part of meters.** AM-09 wires `MicCapture::poll() → RMS/peak → dBFS → meter.update()`; the meter does not know about SAI4 or i16 samples. |

## §11 — Non-goals

- **Spectrum analysers, FFT-based displays, sonograms.** Out of scope for the
  audio-meters initiative; warrants its own concepts doc if pursued.
- **Audio routing, mixing, gain staging.** The meter measures; it does not
  process or route.
- **Recording / file I/O.** Sample acquisition is the caller's job.
- **Sample-rate conversion, anti-alias filtering.** Caller's job.
- **wasm-bindgen sharing of L0.** Decided against in favour of hand-ported TS
  (KISS — see §15 change-log entry 2026-04-26-002).
- **Backwards compatibility with LVGL's `lv_meter`.** The LVGL meter widget
  is a different model (generic gauge). Not a translation target.

## §12 — Acceptance checklist

A conforming AM-00 ratification means **all** of:

- [x] §0 authority list cites IEC 60268-10/17, AES17, ITU-R BS.1770-4, and
      RFC 2119/8174.
- [x] §3 glossary entry exists for every term used in §5–§9.
- [x] §4 source-of-truth map names exactly one owner per concept.
- [x] §5 enumerates ballistic variants with time constants and authority.
- [x] §6 enumerates initial scales.
- [x] §7 enumerates colour-zone identifiers (skin maps to RGB).
- [x] §8 sketches the scale-descriptor JSON schema; AM-03 ratifies the
      canonical JSON Schema document.
- [x] §9 pins the widget update contract (per-frame, dBFS-in, calibration as
      display-time offset).
- [x] §10 reconciles with `ProgressBar`, `Slider`, `motion`,
      `rlvgl-creator`, `rlvgl-decomp`, `MicCapture`.
- [x] §11 lists non-goals.
- [x] §15 change log carries today's ratification entry.

A conforming **subsequent phase** (AM-01 onward) MUST cite the §-numbers it
implements in its own §10 reconciliation table.

## §13 — Files cited

- `CLAUDE.md` § "Spec-Before-Code Planning Discipline"
- `docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md` (reference shape)
- `docs/beaglebone-black/README.md` (reference shape)
- `widgets/src/progress.rs`, `widgets/src/slider.rs` (adjacent primitives)
- `widgets/src/motion/mod.rs` (external-buffer pattern)
- `platform/src/mic_capture.rs`, `platform/src/pdm_filter.rs` (caller side)
- `docs/creator/ASSET-PIPELINE.md`, `docs/assets/FILESYSTEM-ASSET-LOADING.md`
  (asset pipeline)
- `rlvgl-decomp/README.md` (RLE decoder)

## §14 — Unblocks

Ratification of this doc unblocks:

- **AM-01** — `audio-meters-core` Rust crate (ballistics + dB-offset helpers).
- **AM-02** — TS port of L0, with cross-validation fixtures.
- **AM-03** — Scale-descriptor JSON Schema document.
- **AM-04** — Asset package layout and creator-side rasterisation.

AM-05 onward depend transitively on the above.

## §15 — Change log

- **2026-04-26-001** — Initial ratification. Establishes vocabulary, enums,
  schema sketch, source-of-truth map, and reconciliation with adjacent
  rlvgl primitives.
- **2026-04-26-002** — Decision: TS port of L0 is hand-written, not
  wasm-bindgen. Rationale: bundle size, debugability, KISS. Cross-runtime
  parity enforced via shared dBFS fixture sequences and `parity_<variant>`
  tests. Recorded in user-memory `project_audio_meters_architecture.md`.
- **2026-04-26-003** — Decision: ballistics live in the widget, not in a
  pipeline upstream of it. Caller delivers per-frame dBFS at display rate;
  widget owns integration / attack / decay state. Calibration is a
  display-time additive offset, not part of the ballistic. Rationale: keeps
  the upstream audio path pluggable (mic, file, AudioWorklet, synthesised
  test signal).
- **2026-04-26-004** — Refinement: §5 ballistic time constants restated in
  linear-amplitude domain. Original wording said "300 ms to within 1 dB"
  for VU and "5 / 10 ms to within 1 dB" for PPM, which is ambiguous between
  dB-domain and linear-amplitude exponential models — and yields wildly
  different τ values. The IEC test signals are physical step responses on
  analog meter movements, which are linear-amplitude envelope followers.
  VU is now "99 % rise in 300 ms" (the actual IEC 60268-17 criterion);
  PPM is "1 dB below steady tone" interpreted in linear amplitude (the
  IEC 60268-10 criterion). Decay rates unchanged — those were already
  unambiguous (linear in dB).
- **2026-04-26-005** — Schema fix in `scale.schema.json`: `pivot` gains a
  required numeric `value` field (alongside the existing `label` string
  and `input_dbfs` number). Reason: implementing AM-07 NeedleVu surfaced
  that widgets were conflating two different conversions. The widget
  needs **dBFS → scale-units** for positioning (zone lookup, needle
  angle, bargraph fraction), and the optional `calibration_default`
  field is **scale-units → alt-units** (dBu / dBV / dBSPL) for label
  rendering. Without an explicit numeric pivot value, deriving the
  positioning offset required parsing the pivot label string, which is
  fragile (unicode minus, BBC mark labels, etc.). New rule: widgets
  compute `scale_units = dbfs + (pivot.value - pivot.input_dbfs)` for
  all positioning; `calibration_default.offset_db` is reserved for
  alt-units label rendering and does **not** enter positioning math.
  All six canonical scale JSON files updated; both runtime validators
  require the new field.
