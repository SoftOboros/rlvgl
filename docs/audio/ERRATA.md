<!--
ERRATA.md - Audio subsystem errata log for WM8994 / SAI bring-up
issues that should outlive temporary agent task notes.
-->

# rlvgl Audio Errata

This log preserves audio-subsystem institutional memory that is too
specific for the high-level changelog but too durable to leave in root
scratch files. Resolved entries stay here; do not delete them after the
fix lands.

## Index

| ID | Title | Status | First seen | Fixed in |
|---|---|---|---|---|
| [AUDIO-ERRATA-001](#audio-errata-001--wm8994-register-map-corrections-for-aif1-clocking) | WM8994 register-map corrections for AIF1 clocking | Resolved | 2026-04-30 | `9c3d2c1` and follow-up `wm8994.rs` fixes |
| [AUDIO-ERRATA-002](#audio-errata-002--wm8994-fll1-lock-variability-and-aif1adc1-serializer-phase) | WM8994 FLL1 lock variability and AIF1ADC1 serializer phase | Resolved | 2026-05-19 | `9984b34`, `e7935db` |

## AUDIO-ERRATA-001 - WM8994 register-map corrections for AIF1 clocking

**Status:** Resolved.
**First seen:** 2026-04-30, disco-analyzer bench session.
**Owning area:** `platform/src/wm8994.rs`, WM8994 AIF1 playback and
record bring-up.

### Symptom

Downstream record-path users could configure apparently valid WM8994
playback / record paths while AIF1ADCDAT remained silent or register
readback disagreed with the intended AIF1 setup. The narrow
`init_playback` path was partly masked because the wrong writes landed
on benign or tolerable defaults, but `init_record`-style code touched
more of the AIF1 register surface and exposed the bug.

### Root Cause

Several WM8994 register identities in `platform/src/wm8994.rs` were
wrong relative to `WM8994_Rev4.6.pdf`:

- `REG_AIF1_RATE` was treated as `0x0211`, but `R0x211` is AIF2 Rate.
  AIF1 Rate is `R0x210`.
- `REG_CLOCKING_2` was treated as `0x0210`, but `R0x210` is AIF1 Rate.
  Clocking (2) is `R0x209`.
- Comments around `R0x208` Clocking (1) implied `SYSCLK_SRC =
  AIF1CLK` for value `0x000A`; per the datasheet bit 0 is clear in
  that value.
- `R0x301` AIF1 Control (2) writes must preserve `AIF1DACR_SRC`
  (bit 14) unless the caller explicitly intends to swap DAC source.
- `AIF1_TRI` belongs to `R0x302` bit 15, not `R0x301`.

### Fix

The driver now uses datasheet-correct register addresses and comments:

- `REG_CLOCKING_2 = 0x0209`
- `REG_AIF1_RATE = 0x0210`
- `R0x208`, `R0x209`, `R0x210`, `R0x301`, and `R0x302` comments cite
  their actual WM8994 register roles.

The fix also made the later AUDIO-01 record-path work possible: once
AIF1 Rate and Clocking (2) were no longer aliased, record bring-up
could reason about FLL1 lock, LRCLK rate, and serializer behavior
without hidden register overwrites.

### Verification

The corrected map is mirrored in `platform/src/wm8994.rs` and is
cross-referenced by [`01-codec-bringup.md`](01-codec-bringup.md).
Downstream disco-analyzer code uses the same corrected AIF1 register
addresses for record-path experiments.

## AUDIO-ERRATA-002 - WM8994 FLL1 lock variability and AIF1ADC1 serializer phase

**Status:** Resolved.
**First seen:** 2026-05-19, disco-analyzer cold-boot / warm-reset bench
session.
**Owning area:** `platform/src/wm8994.rs::init_record`,
[`01-codec-bringup.md`](01-codec-bringup.md) AUDIO-01.

### Symptom

Digital capture from the WM8994 record path varied across boots even
when the analog sidetone path was audible:

- One stereo slot sometimes emitted a `0x0000 / 0x8000` bimodal
  pattern while the other slot carried real ADC data.
- Another failure mode split one source sample across slots: one slot
  carried the lower 11 bits and the other carried the upper 5 bits,
  producing an apparent large L/R level asymmetry.
- Warm reset was much more reliable than cold boot because BCLK1 was
  already running from the prior session.

The analog route could sound correct while SAI1 RX still captured bad
digital samples, which made the issue look like a downstream DMA or
analyzer bug.

### Root Cause

`init_record` could arm the AIF1ADC1 serializer before FLL1 was truly
locked. The static post-`FLL1_ENA` delay covered typical lock time but
not the worst case. If post-FLL AIF1 framing writes landed while SYSCLK
was still unstable, the serializer latched a wrong bit-clock phase
relative to LRCLK1 and held that phase until the next arm cycle.

The first implementation also tried to observe lock through
`R0x731 bit 5` (`FLL1_LOCK_EINT`, an edge-latched interrupt event).
Bench validation showed that this latch could remain clear while
`R0x732 bit 5` (`FLL1_LOCK_STS`, level status) reported that FLL1 was
actually locked. The edge latch was therefore the wrong primitive for
the init sequence.

A separate but related framing error set `AIF1ADC_RATE` /
`AIF1DAC_RATE` in `R0x304` and `R0x305` to 64 (`0x0840`) while the SAI
master was generating a 32-BCLK stereo frame. That mismatch produced
asymmetric serializer behavior, especially on the left channel.

### Fix

`Wm8994::init_record` now follows AUDIO-01:

- Poll `R0x732 bit 5` (`FLL1_LOCK_STS`) as the current level status.
- Use a 100 ms per-attempt timeout, 1 ms poll interval, and up to
  three retries.
- Do not issue post-FLL AIF1 framing / ADC enable writes until FLL1
  lock is confirmed.
- Report lock acquisition via `FllLockOutcome`.
- Program `AIF1_LRCLK_SLAVE_ENA = 0x0820`, matching the actual
  BCLK1/LRCLK1 ratio of 32 for the current 16-bit stereo frame.

### Verification

Bench validation after the `0x0820` correction recovered full-range
stereo capture: left channel recovered to hundreds of distinct values
per snapshot, L/R correlation was approximately `+0.92`, and L/R
peak-to-peak ratio was approximately `1.006`.

The durable normative form of this fix is
[`01-codec-bringup.md`](01-codec-bringup.md) AUDIO-01, especially
§9 and §15 amendments `2026-05-19-b` and `2026-05-19-c`.
