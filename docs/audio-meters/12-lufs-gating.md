<!--
12-lufs-gating.md - AM-08e: BS.1770 absolute gating in Ballistic::LufsI.
L0 improvement; widgets pick up the new semantics transparently.
-->

**[← Prev AM-08d](11-lufs-gauge.md) · [Index](README.md)**

# AM-08e — LufsI Absolute Gating

`Ballistic::LufsI` upgrades from an ungated streaming mean to an
absolute-gated streaming mean per ITU-R BS.1770-4. Pure L0 change;
the LufsGauge widget (AM-08d) picks up the new behaviour
transparently because it consumes `BallisticState` by value.

## Authority

- **ITU-R BS.1770-4** §5.1 — gating thresholds.
- Concepts §5 (`LufsI` row), §15-006.

## What changed

| Before | After |
|---|---|
| Every per-frame dBFS sample contributed to the running mean. Silence dragged the integrated value down toward the noise floor. | Samples below `-70 LUFS` are skipped: they don't advance the count and don't contribute to the mean. The reading holds across silent stretches. |

Implementation:

- Rust `audio-meters-core/src/ballistic.rs` — `LUFS_ABSOLUTE_GATE_DB`
  constant; `Ballistic::LufsI` arm checks `dbfs >= gate` before
  updating state.
- TS `audio-meters-core/ts/src/ballistic.ts` — same constant, same
  conditional. Mirrored to keep cross-runtime parity.

Parity fixtures regenerated under
`audio-meters-core/fixtures/expected/`. The Rust suite produces the
canonical output; the TS suite asserts match within `1e-4` dB.

## Why per-sample, not per-block

A fully BS.1770-conformant gate uses **400 ms blocks with 75 % overlap**
— compute momentary loudness on each block, then exclude blocks
below the gate threshold. That requires a block ring buffer and
state proportional to the longest programme being measured.

L0 today is a streaming `BallisticState` with O(1) state per meter.
Per-sample gating preserves the O(1) shape while catching the same
class of "silence shouldn't bias the mean" cases. The deviation
shows up only on edge cases:

- A loud transient followed by a long quiet tail just barely above
  the gate (block gating would exclude blocks straddling the
  transition; per-sample gating includes them).
- Programmes where the per-sample envelope dips briefly below
  -70 LUFS during otherwise active content.

For typical metering — which is what the widget surface targets —
the difference is below the threshold of audible action. A future
phase (`alloc`-friendly, or const-generic ring) can replace this
with full block gating without changing the widget API.

## Why absolute-only, no relative gate

ITU-R BS.1770-4 §5.1 also specifies a **relative gate** at
`programme-mean − 10 LU`: after the absolute gate, the gated mean is
recomputed by excluding blocks below `gated_mean - 10 LU`. This is a
two-pass operation that fundamentally requires storing every block
that survived the absolute gate.

Skipping the relative gate is documented in concepts §15-006. In
practice the two gates affect the integrated reading by < 1 LU for
typical content; full conformance is a follow-up phase.

## Reconciliation with adjacent layers

| Adjacent layer | Reconciliation |
|---|---|
| `LufsGauge` widget (AM-08d) | No code change. The widget creates `BallisticState::new(Ballistic::LufsI)` and the new semantics flow through. The "integrated colour at target" test still passes (input is -23 dBFS = -23 LUFS, well above the -70 gate). |
| Other widgets | Untouched. `LufsM`, `LufsS`, `Vu`, `Ppm*`, `Rms`, `DigitalPeak`, `Instant` are unchanged. |
| Cross-runtime parity fixtures | Regenerated. Rust generates the canonical sequence; TS asserts match. Regeneration is reproducible via `RLVGL_AUDIO_METERS_REGENERATE=1 cargo test -p rlvgl-audio-meters-core --test parity`. |
| `audio-meters-core::tests::lufs_i_running_mean_converges` | Still passes — the test drives `-23 dBFS` for 1000 frames, well above the gate. |

## Acceptance checklist

- [x] `LUFS_ABSOLUTE_GATE_DB` constant in both runtimes; same value.
- [x] `LufsI` skips samples below the gate; previous reading is held.
- [x] Parity fixtures regenerated; both runtimes pass.
- [x] LufsGauge tests still pass without modification.
- [x] Concepts §5 LufsI row + §15-006 change-log entry updated.

## Non-goals (deferred)

- Block-based gating per BS.1770-4 §5.1.
- Relative gate at `gated_mean - 10 LU`.
- LRA (Loudness Range, EBU Tech 3342) — different statistic.
- Audio-rate K-weighting filter inside L0. Caller's job per
  concepts §9.

## Files cited

- `audio-meters-core/src/ballistic.rs`
- `audio-meters-core/ts/src/ballistic.ts`
- `audio-meters-core/fixtures/expected/*__LufsI.json` (regenerated)
- [`docs/audio-meters/00-concepts.md`](00-concepts.md) §5, §15-006
- [`docs/audio-meters/11-lufs-gauge.md`](11-lufs-gauge.md)

## Change log

- **2026-04-26** — Initial ratification (AM-08e). LufsI applies
  BS.1770 absolute gate at `-70 LUFS` in both runtimes.
