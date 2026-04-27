<!--
14-bs1770-relative-gating.md - AM-08h: standalone L0 helper for full
ITU-R BS.1770-4 §5.1 two-pass gating with a const-generic sliding window.
-->

**[← Prev AM-09d](README.md) · [Index](README.md)**

# AM-08h — BS.1770 Relative Gating

L0 helper that ships full ITU-R BS.1770-4 §5.1 two-pass gating
(absolute + relative) with bounded memory via a const-generic
sliding window. Standalone — not a `Ballistic` enum variant — so
the existing `BallisticState::LufsI` (absolute-only) stays unchanged
and the widget API doesn't need to grow generic parameters.

## Authority

- ITU-R BS.1770-4 §5.1 — gating thresholds and procedure.
- Concepts §5 (LufsI row), §15-006 (absolute gating), §15-007
  (this helper).

## What ships

| Path | Role |
|---|---|
| `audio-meters-core/src/integrated.rs` | `RelativelyGatedLufsI<const N: usize>` Rust type. `[f32; N]` ring buffer with two-pass gated mean recomputed each `update`. Const-`new`, no allocation. |
| `audio-meters-core/ts/src/integrated.ts` | TS mirror; `windowSize` is a constructor argument, ring is a `Float32Array`. Same gate constants, same two-pass structure. |

## API

### Rust

```rust
use rlvgl_audio_meters_core::RelativelyGatedLufsI;

// 256 entries × 4 B = 1 KiB ring; ~4.3 s of history at 60 Hz.
let mut g: RelativelyGatedLufsI<256> = RelativelyGatedLufsI::new();

loop {
    let dbfs = pull_k_weighted_dbfs();   // caller K-weights upstream
    let reading = g.update(dbfs, frame_dt);
    // `reading` is the LUFS-domain mean over the doubly-gated window.
}
```

### TypeScript

```ts
import { RelativelyGatedLufsI } from "@rlvgl/audio-meters-core";

const g = new RelativelyGatedLufsI(256);

const reading = g.update(dbfs, frameDt);
```

## Algorithm

For each `update(dbfs, dt)`:

1. **Push** the new `dbfs` into the ring (oldest is overwritten when
   the ring is full). NaN / -∞ / values below `-120 dBFS` are clamped
   to the floor.
2. **Pass 1 — absolute gate.** Compute the linear-power mean over
   ring entries with `dbfs ≥ -70 LUFS`. Call this `Γ_a`.
3. **Pass 2 — relative gate.** Threshold `Γ_r = max(Γ_a − 10 LU,
   -70 LUFS)`. Compute the linear-power mean over ring entries with
   `dbfs ≥ Γ_r`. Return that as the new reading.
4. **Pathological case.** If pass 1 finds no entries above the
   absolute gate (all silence), the reading floors at `-120 LUFS`.
   If pass 2 finds no entries above the relative gate (which can
   only happen if every sample is exactly at the absolute gate), the
   reading falls back to `Γ_a`.

The relative-gate floor at the absolute gate is required by
BS.1770-4 §5.1: the relative gate must not relax the absolute one.

## Choosing `N`

| `N` (Rust const-generic) | Window at 60 Hz | Memory | Use case |
|---|---|---|---|
| 64 | ~1 s | 256 B | tiny embedded readout, very recent loudness |
| 256 | ~4 s | 1 KiB | typical live meter |
| 1024 | ~17 s | 4 KiB | studio "recent material" panel |
| 8192 | ~2.3 min | 32 KiB | desktop / programme-loudness UI |
| 36000 | 10 min | 144 KiB | full-album integrated reading (host-only) |

`N` is fixed at compile time on the Rust side. On the TS side it's
constructor-supplied and stored in a `Float32Array`. Both bound
memory; choose what fits the platform.

## Deviation from a fully BS.1770-conformant reference

BS.1770-4 specifies the integrated-loudness procedure as a sequence
of **400 ms blocks with 75 % overlap** (one new block every 100 ms).
For each block, compute its momentary loudness, then gate. This
helper instead treats every per-frame `update` as one gating sample.
The block layer is omitted.

For embedded / live "loudness display" use cases driven by a
`LufsM` momentary-loudness signal at display refresh rate (≈ 60 Hz),
the omission is invisible. For studios needing strict programme-
loudness numbers — the kind that go on a delivery spec sheet —
prefer a desktop conformance implementation (e.g. Loudness Meter
plugins built on top of `libebur128`).

The deviation is documented per spec-before-code §3 (Definitions —
reference vs. restatement): this helper *adapts* the BS.1770 LufsI
procedure with the named delta "block layer omitted; per-update
gating".

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `BallisticState::LufsI` (concepts §15-006) | Streaming, absolute-gated, O(1). Default for the widget tier. Keeps the simple use case simple. |
| `RelativelyGatedLufsI<N>` (this helper) | Standalone, full two-pass, O(N) per update + O(N) memory. Opt-in for callers who need the relative gate. |
| `LufsGauge` widget | Unchanged. Owns three `BallisticState`s. Users wanting full BS.1770 compose `RelativelyGatedLufsI<N>` into a custom widget — typically pairing it next to or replacing the LufsI line of `LufsGauge`. The widget surface doesn't need a generic parameter. |
| Concepts §5 frozen `Ballistic` enum | **Not modified.** Adding a `Ballistic::LufsIRelative` variant would be Standards Action and would require const-generic enums (which Rust doesn't have). Keeping the helper as a separate type avoids both issues. |
| Cross-runtime parity fixtures | This helper is unit-tested only — no fixture parity check yet. Both runtimes follow the same algorithm; the unit tests on each side cover the same scenarios (steady, half-loud-half-silent, quiet-passages-rejected, ring-wrap, reset, NaN). |

## Acceptance checklist

- [x] Rust type `RelativelyGatedLufsI<const N: usize>` with const
      `new`, `update`, `reading_db`, `reset`, `len`, `is_empty`,
      `capacity`.
- [x] TS class `RelativelyGatedLufsI` with `windowSize` constructor
      argument and equivalent surface.
- [x] Eight unit tests on each side covering: floor, capacity,
      steady convergence, absolute-gate excludes silence, relative-
      gate excludes quiet passages (asserts > absolute-only mean),
      ring wraps after `N`, `reset`, NaN-handling.
- [x] Cortex-M7 cross-compile clean.
- [x] Concepts §15-007 change-log entry.

## Non-goals (deferred)

- BS.1770 400 ms / 75 % block layer. Helper treats per-frame
  updates as gating samples.
- LRA (loudness range, EBU Tech 3342). Statistical, not gating.
  Different computation; would warrant its own helper.
- Cross-runtime parity fixtures. The two-pass algorithm has more
  arithmetic than the leaky integrators; bit-precision parity is
  harder to enforce. Unit-level cross-runtime parity (same scenario,
  same expected behaviour) is enough for this helper.
- LufsGauge integration. The widget keeps the streaming
  `BallisticState::LufsI` for the I line. A future `LufsGaugeStrict`
  or similar can take the helper as a generic parameter.

## Files cited

- `audio-meters-core/src/integrated.rs`
- `audio-meters-core/ts/src/integrated.ts`
- `audio-meters-core/ts/test/integrated.test.ts`
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §15-007
- [`docs/audio-meters/12-lufs-gating.md`](12-lufs-gating.md)

## Change log

- **2026-04-26** — Initial ratification (AM-08h). Both runtimes
  ship the helper; eight unit tests on each side. The audio-meters
  initiative now offers full BS.1770-4 §5.1 gating semantics for
  callers who need them, while keeping the default widget surface
  simple.
