# Agent Task: WM8994 AIF1 Serializer Arm-Phase Calibration — Bi-Directional (`platform/src/wm8994.rs`)

**Branch:** target the next AUDIO-01 amendment cycle (proposed `AUDIO-01-d`); coordinate with the maintainer for the branch name.
**Filed:** 2026-05-28 by disco-analyzer bench session investigating DAC-side high-frequency hash that survives every register-level check (ERRATA-009 in `streamz/submodules/disco-analyzer/docs/concepts/ERRATA.md`). Filed jointly against the long-standing ADC-side serializer race (ERRATA-004 in the same log, open 🔴 since 2026-05-23).
**Authoritative source:** `WM8994_Rev4.6.pdf` (queryable via `mcp__softoboros__memalpha_ask`) + [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md) AUDIO-01, particularly §9 INV-AUDIO-01-1 (FLL1 lock invariant — predecessor in this family).

## Why this task exists

AUDIO-01-a/b/c resolved the FLL1-lock-before-arm race and the AIF1ADC LRCLK rate mismatch (R0x304 = 0x0820 RATE=32 matching SAI's 32-BCLK frame at 48 kHz). Two bench-confirmed symptoms remain that share a root-cause class **not** addressed by AUDIO-01-a/b/c:

```
WM8994 AIF1 serializer (BOTH AIF1ADC1 and AIF1DAC1 sides) arms at a
deterministic-but-wrong N≠0 BCLK phase offset relative to LRCLK.

  ADC side (ERRATA-004, 🔴 since 2026-05-23):
    captures bit-shifted from the codec ADC digital path
    → stuck patterns / 2× overflow wrap / half-amplitude
      depending on N (N=0 → clean stereo, what we want every boot)

  DAC side (ERRATA-009, 🔴 since 2026-05-28):
    reconstructs bit-shifted from MCU-provided AIF1DAC1 stream
    → recognizable audio with dense high-frequency hash overlay
    → bank-coherent amplitude modulation at 93.75 Hz (SAI1 TX
      bank-swap rate) when the source data has bank-boundary
      discontinuities (e.g., L-mono-fold L→{L,R} pattern from
      disco-analyzer's `Cm4Loop` mode); bank-uniform hash when
      the source is bank-smooth (e.g., the same mode's
      `stimulus_mode = 1` synth tones)
```

The ADC manifestation has been characterized exhaustively at the disco-analyzer bench across 16 deterministic init_record reruns (bench-26..33 in disco-analyzer's history; see [ERRATA-004](https://internal/ — file is at `streamz/submodules/disco-analyzer/docs/concepts/ERRATA.md`) for the full timeline). The N value chosen at arm is reproducible per cold-boot path but does NOT randomize across init_record re-runs — the same N latches every time once the SAI BCLK is running.

The DAC manifestation was identified 2026-05-28 in a single bench round after exhausting every register-level hypothesis (see §"Why hypothesis #1 was insufficient" below). The symptom-shape symmetry against ERRATA-004 — particularly that both errata share a "wire is correct, but the serializer interprets it at the wrong bit offset" mechanism, that both are bench-only-visible (every readable register reads correct), and that the DAC garbage is gated by the same `init_record` arm sequence — made the joint hypothesis ratifiable on the same session.

**This task is the implementation tracker for landing AUDIO-01-d** — a serializer-arm-phase calibration step that ensures N=0 on BOTH directions, on every cold boot, across all downstream consumers. The chapter [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md) is the normative artifact; this file is the workflow notes for the AUDIO-01-d amendment.

### Why hypothesis #1 was insufficient on the DAC side

The original 2026-05-28 hypothesis was "AIF1DAC LRCLK / RATE not corrected by AUDIO-01-c the way the ADC side R0x304 was." Falsified within the same session by the existing CM7 codec-register dump at `ADC_ENABLE_DUMP_BASE + slot 14/15`:

- R0x304 (AIF1ADC LRCLK_DIR + RATE) = `0x0820` — RATE=32, LRCLK_DIR=1 ✓
- R0x305 (AIF1DAC LRCLK_DIR + RATE) = `0x0820` — RATE=32, LRCLK_DIR=1 ✓ (byte-identical to R0x304)

AUDIO-01-c's fix is already symmetric across ADC and DAC at the LRCLK-rate register layer. Subsequent full WM8994 register dump at `AIF1_EXTRA_BASE = 0x3800_3700` (45+ registers covering R0x420, R0x610/611, R0x601/602, R0x402/403, R0x306, R0x201, R0x212, R0x000, etc.) read cleanly across every DAC-path register. SAI Block A (TX, master) status flags equally clean: `SAI_ASR = 0x00040000` (FLVL=4 nearly-full TX FIFO, no OVRUDR/MUTEDET/WCKCFG/CNRDY/AFSDET/LFSDET), `SAI_ACR2 = 0x00000003` (FTH=3/4, MUTE=0, TRIS=0). DMA1 stream 1 cycling normally. The wire and codec digital config are demonstrably correct.

User independently confirmed `LoopbackMode::CodecSidetone` produces clean DAC output — the codec analog stage + HP output + jack are fine. The bug therefore lives in the digital handoff between the SAI TX wire and the AIF1DAC1 serializer's interpretation of it, which is exactly the slot ERRATA-004 characterizes for the AIF1ADC1 serializer on the opposite direction.

## What changes

> **Read order:** §0 below is the **lead candidate** and MUST be tested on
> the bench before any of §1–§5 is built. §1–§5 (the `SerializerArmOutcome`
> closed-loop calibration machinery) are the **fallback** that applies only
> if §0 fails to make N=0 deterministic. If §0 succeeds, AUDIO-01-d collapses
> to a one-commit ordering fix + a §15 amendment, and §1–§5 are mooted.

### 0. LEAD CANDIDATE — deterministic serializer arm-ordering fix (test FIRST)

**Hypothesis (AUDIO-01-d-H0):** the N≠0 arm-phase race is not metastable
hardware luck — it is a **control-write ordering bug**. The AIF1ADC1 /
AIF1DAC1 serializer-enable bits are currently written *before* the clock
domain they arm against is locked, configured for slave LRCLK, and running.
Deferring those two enable writes until after AIF1CLK is up and stable (and
BCLK1/LRCLK1 are confirmed running from the SAI master) makes the serializer
arm against an already-settled LRCLK1 frame, pinning N=0 every boot — with
no measurement, no stimulus, and no closed loop. One-time bench
characterization proves the recipe; field boots then run it blind ("calibrate
once with a head, ship headless").

**Evidence — current `init_record` ordering (`platform/src/wm8994.rs`):**

| Step | Line | Write | What it does |
|---|---|---|---|
| 6 | ~595 | `R0x004 = (1<<9)|(1<<8)|(1<<1)|1` | **AIF1ADC1L/R_ENA (b9/b8) ← ADC serializer enable** + ADCL/R_ENA (b1/b0, analog) |
| 7 | ~602 | `R0x005 = 0x0303` | **AIF1DAC1L/R_ENA (b9/b8) ← DAC serializer enable** + DAC1L/R_ENA (b1/b0, analog) |
| 12 | ~702 | FLL1 lock wait | FLL1 reaches lock (AUDIO-01-1) |
| 13 | ~715 | `R0x300` / `R0x302` | AIF1 format / codec-is-slave |
| — | ~763 | `R0x304` / `R0x305` | AIF1 LRCLK_DIR slave-enable + RATE=32 (AUDIO-01-c) |
| 14 | ~795 | `R0x208` | SYSDSPCLK_ENA |
| 14 | ~801 | `R0x200 = 0x0011` | **AIF1CLK_ENA ← AIF1CLK actually starts running here** |
| 14 | ~808 | `delay_busy(50_000)` | ADC SR-converter / filter settle |

The serializer-enable bits (R0x004 b9/b8, R0x005 b9/b8) are latched at
Steps 6–7 while **FLL1 is not yet locked, the codec has not been told it is
in slave LRCLK mode (R0x304/305 b11 unset), and AIF1CLK is not running**
(R0x200 is written ~200 lines later). When AIF1CLK finally starts at Step 14
with those enables already =1, the serializer's bit-clock state machine spins
up against whatever phase the just-started AIF1CLK happens to hold relative
to the incoming LRCLK1 — the metastable N latch.

**Why this fits the research (F4):** the datasheet's only slave-mode
requirement is that AIF1CLK be synchronised to the external LRCLK1 (p.357),
and it documents no soft-reset / GPIO-sync / write-sequence to realign the
frame (F4). That is precisely the signature of an ordering bug rather than a
fixable-in-place hardware knob: get the *order* right and the documented sync
requirement does the rest; get it wrong and there is nothing to poll or
re-trigger (consistent with F1/F2/F3 all coming up empty).

**Apparent tension with AUDIO-01 §12 gate (c):** gate (c) already says
"Issue NO writes to R0x4 (AIF1ADC1L/R_ENA) … until `LockedFirstTry` or
`LockedAfterRetry`." The current code writes R0x004 (incl. the AIF1ADC1L/R_ENA
serializer bits) at Step 6, **before** the lock wait at Step 12 — i.e. the
implementation appears to under-satisfy its own ratified gate (c). H0's
reorder is therefore partly *bringing the code into compliance with an
existing invariant*, not only a new fix. (Flag for the maintainer: confirm
whether gate (c) was always intended to cover the b9/b8 serializer bits or
only the framing writes; the glossary's "serializer arm … implicit in
AIF1ADC1L/R_ENA" reading says it covers them.)

**Proposed reorder (split the enables; keep analog power-up timing intact):**

1. **Step 6 → keep only the analog ADC blocks:** `R0x004 = (1<<1)|1`
   (ADCL_ENA + ADCR_ENA, b1/b0). These need the early VMID / charge-pump
   settle and are NOT serializer arms.
2. **Step 7 → keep only the analog DAC blocks:** `R0x005 = 0x0003`
   (DAC1L_ENA + DAC1R_ENA, b1/b0).
3. **New Step 14b (after R0x200 AIF1CLK_ENA + the 50 ms settle, line ~808):**
   now that FLL1 is locked, the codec is in slave LRCLK mode (R0x304/305),
   and AIF1CLK is running, set the serializer enables:
   `R0x004 |= (1<<9)|(1<<8)` (AIF1ADC1L/R_ENA) and
   `R0x005 |= (1<<9)|(1<<8)` (AIF1DAC1L/R_ENA). Optionally gate this on a
   read confirming BCLK1/LRCLK1 are live (the caller/SAI master side knows
   this; the codec exposes no such status — see F1).

This is read-modify-write on R0x004/R0x005 so the analog-enable bits set
earlier are preserved.

**Why this is the lead candidate:**

- **Cheap & deterministic:** a write-reordering, no new API surface, no enum,
  no closed loop, no stimulus. Matches the shape of AUDIO-01-1 (lock-before-arm)
  and AUDIO-01-c (RATE=32) — both "characterize once, bake a deterministic
  recipe, ship headless."
- **Headless-clean forever** if it holds: field boots run the fixed recipe
  blind; no per-boot "head" required (directly answers the
  "calibrate once with a head?" question — yes, if H0 holds).
- **Falsifiable in one bench round.**

**Bench test protocol (one round, both directions):**

- Apply the reorder on a branch; flash disco-analyzer pinned to it.
- ADC side: chromatic-sine / 1 kHz line-in source; read SAI1 RX bank over
  probe-rs across **≥10 cold boots** (USB-plug cold-boot per
  `feedback_b1_left_codec_wedged`). Pass = clean stereo, N=0, every boot
  (no `0x0000/0x8000` bimodal, no 11+5 split, no half-amplitude slot).
- DAC side: `LoopbackMode::Cm4Loop` + scope on the output jack; pass = audible
  source with **no** high-frequency hash overlay and no 93.75 Hz envelope
  (ERRATA-009).
- **If PASS:** AUDIO-01-d = this reorder. Land it; amend AUDIO-01 §15 +
  reconcile gate (c); flip ERRATA-004 / ERRATA-009 → 🟢. §1–§5 below are
  dropped (note them as "considered, mooted by H0" in §15).
- **If FAIL (N still ≠0 on some boots):** the race is genuinely metastable,
  not ordering — fall through to the §1–§5 closed-loop calibration machinery,
  which then needs a head at every boot it must guarantee (see the regime
  table in the design discussion). Record the failing boots' N values to seed
  the detector's test vectors.

**Out of scope for H0:** do not touch SAI master config (RM0399 side) or
re-litigate AUDIO-01-c's RATE=32 — H0 is purely the codec-side write order.

---

The following §1–§5 describe the **fallback** closed-loop calibration. Build
them only if H0 fails on the bench.

### 1. New normative section in `docs/audio/01-codec-bringup.md`

Add INV-AUDIO-01-4 (or successor §-block per the AUDIO chapter's discretion):

> **INV-AUDIO-01-4 — AIF1 serializer arm phase MUST be N=0 on both directions before `init_record` returns success.**
>
> A WM8994 AIF1 serializer that arms at N≠0 BCLK offset relative to LRCLK produces deterministic bit-shifted data on the arming direction. On AIF1ADC1, this manifests as stuck patterns / 2× overflow wrap / half-amplitude depending on N (per ERRATA-004 in downstream consumer disco-analyzer). On AIF1DAC1, this manifests as recognizable audio with dense high-frequency hash overlay (per ERRATA-009 in the same downstream consumer). `init_record` MUST execute a calibration sequence that detects N≠0 arm and re-arms until N=0 on both directions, OR returns a new `SerializerArmOutcome::Misaligned(N_adc, N_dac)` outcome variant the caller can act on. Default behaviour SHOULD be to retry within an internal budget (analogous to AUDIO-01-1's FLL1 lock retry budget) before surfacing the outcome to the caller.

The §6 frozen enum `FllLockOutcome` precedent suggests a sibling enum `SerializerArmOutcome` with variants `{ Calibrated, Misaligned { adc_n: u8, dac_n: u8 }, CalibrationUnavailable }`. The third variant covers the path where the bench cannot supply a calibration stimulus (e.g., headless boot without a known signal source). Default behaviour on `CalibrationUnavailable` MUST be to log + proceed, NOT to fail boot — the failure mode without calibration is "audio works but with serializer-race artifacts," which is acceptable degraded-mode operation.

### 2. Calibration primitive — design open

The calibration step needs a way to (a) **detect** N=0 vs N≠0 arm post-`init_record`, and (b) **force a re-arm** without bricking the codec's I²C or breaking the FLL1 lock.

Detection candidates the bench has hinted at (no consensus yet — this is the open design question):

- ~~**Loopback-based detection.** Write a known stereo-distinct stimulus to AIF1DAC1 (e.g., L = `0x4000`, R = `0xC000` — alternating positive-max / negative-max), enable codec-internal sidetone or AIF1 digital loopback (R0x301 bit 14 `AIF1_LOOPBACK`), and read back the captured AIF1ADC1 samples. If the captured stereo distinctness matches the stimulus, both serializers arm at N=0. If L≠L or R≠R, compute N from the bit-pattern offset.~~ **REFUTED by datasheet — see research findings below. AIF1_LOOPBACK is R0x301 bit 0 (not bit 14) and routes ADCDAT1→DACDAT1 (ADC capture → DAC playback), NOT an MCU stimulus into the ADC capture path. It cannot inject a known pattern into what the AIF1ADC1 serializer emits, so it cannot observe ADC-side arm phase.**
- **DRC-bypass detection.** Disable R0x440 DRC + R0x420 DAC filters for the calibration window, drive a known stimulus, observe the recovered samples. Distinguishes pure serializer arm-phase from filter-induced corruption. (Still requires an external/bench stimulus + capture; see findings.)
- ~~**R0x731 / R0x732 status-register polling.** These are FLL1 status registers (per AUDIO-01-1 normative use); WM8994_Rev4.6 may document additional serializer-arm status bits in adjacent registers.~~ **REFUTED by datasheet — no serializer-arm-phase status register or observability bit is documented anywhere in WM8994_Rev4.6. See findings.**

## Datasheet research findings (memalpha, 2026-05-29)

Six `mcp__softoboros__memalpha_ask` queries against `WM8994_Rev4.6.pdf`
(content_id 536) resolved the §2 open design questions. The net result is
that **neither headless detection primitive sketched above exists**, and
**no datasheet-blessed "force N=0" knob exists** — which reshapes the
AUDIO-01-d architecture.

**F1 — No serializer-arm-phase observability bit (refutes status-register
detection).** The datasheet documents no status register or bit reporting
AIF1ADC1/AIF1DAC1 serializer bit-clock-phase alignment relative to LRCLK1.
The only phase/polarity controls are `AIF1_BCLK_INV` / `AIF1_LRCLK_INV`
(R0x300 bits 8 / 7, p.176) and the DSP-mode MSB-position A/B select. There
is nothing to poll. (Query: "AIF1 serializer arm phase … status register
bit" → "I don't know … does not document a status register or bit
reporting the … serializer's bit-clock phase alignment.")

**F2 — `AIF1_LOOPBACK` is ADC→DAC at bit 0, not DAC→ADC at bit 14 (refutes
loopback detection AND corrects two factual errors in this task doc's
sketch).** `AIF1_LOOPBACK` is **R769 (0x0301) bit 0** (WM8994_Rev4.6 p.180,
"AIF1 - LOOPBACK"). When set, "ADCDAT1 data output [is] directly input to
DACDAT1 data input … the normal input (DACDAT1) is not used." It is an
internal **ADC-capture → DAC-playback** loop. It therefore cannot inject an
MCU-supplied AIF1DAC1 pattern into the AIF1ADC1 capture stream, so it cannot
observe the ADC-side serializer arm phase that ERRATA-004 is about. The
sketch above had both the bit position (said 14, is 0) and the data
direction (said DAC→ADC, is ADC→DAC) wrong.

**F3 — Re-arm-by-toggle is plausible but UNDOCUMENTED.** R0x004 bit 9 =
`AIF1ADC1L_ENA`, bit 8 = `AIF1ADC1R_ENA` ("AIF1, Timeslot 0" output path);
R0x005 the matching `AIF1DAC1L/R_ENA` input path (p.174 / p.238). The
datasheet confirms the bit semantics but says nothing about toggling them
re-arming the serializer or any sequencing/FLL-lock interaction ("I don't
know if toggling these bits off then on re-arms the … serializer, nor if
there are any sequencing requirements"). Re-arm primitive **(a)** is thus a
**bench-only hypothesis**, not datasheet-supported — it cannot be ratified
as a normative invariant on datasheet authority; bench is the sole authority
(same situation as AUDIO-01-c's RATE asymmetry).

**F4 — Root cause is slave-mode clock-sync; no soft-reset / GPIO-sync /
write-sequence to force frame alignment.** In slave mode the datasheet
*requires* AIF1CLK to be synchronised to the external LRCLK1, achievable by
either (1) an MCLK derived from the same reference as LRCLK1, or (2) using
external BCLK1/LRCLK1 as the FLL reference for AIF1CLK (p.357). The driver
already satisfies (2) via FLL1 ← BCLK1 (AUDIO-01-1), so it is
datasheet-compliant on clock sync. The datasheet documents **no**
control-write sequence, soft-reset, or GPIO sync to guarantee the serializer
aligns to the LRCLK frame boundary (queries 5 + 6 both: "context does not
specify … soft-reset, a GPIO sync, or a required relationship between
AIF1CLK startup and the external LRCLK1 frame"). It frames misalignment as
inherent asynchronous-clock tolerance, mitigated by the sync requirement we
already meet. Note also: AIF1CLK_ENA (R0x200 bit 0) "should be set to 0 when
reconfiguring the clock sources" (p.193) — relevant if a re-arm path cycles
AIF1CLK.

### Consequences for the AUDIO-01-d design

1. **Detection cannot be headless / register-only.** Both candidate
   primitives are refuted (F1, F2). Detecting N requires an external known
   stimulus captured at SAI1 RX (ADC side) or observed on the analog output
   jack (DAC side) — a **bench operation**. `init_record` cannot self-detect
   N; it has no stimulus source or capture channel it owns. This validates
   the `CalibrationUnavailable` default path being not just the backwards-
   compat default but the **only** path available in a headless boot.

2. **No datasheet-blessed deterministic "force N=0" knob (F3, F4).** The
   re-arm-toggle must be **bench-proven before any spec ratification**. Per
   the chapter §0 authority policy, the datasheet wins for what it owns;
   here it is silent, so bench is the only authority and INV-AUDIO-01-4
   cannot be ratified on datasheet grounds alone.

3. **Architecture must be caller-supplied-stimulus, not headless.** The
   calibration is necessarily split: rlvgl-platform owns the **N-detection
   arithmetic** (pure, host-testable against synthetic buffers for
   N ∈ {0,1,11,15}) and the **re-arm action** (toggle R0x004/R0x005, looped
   until detected N=0); the **stimulus + capture** is owned by the
   downstream consumer (disco-analyzer), which feeds captured samples + the
   known stimulus into the platform's detector. This is a revision of §3-§5
   above, which assumed `init_record` could run calibration internally.

4. **Recommended re-arm primitive remains (a)** the `AIF1ADC1L/R_ENA` /
   `AIF1DAC1L/R_ENA` toggle (R0x004 / R0x005) — lowest cost, no SAI re-touch,
   no AIF1CLK_SRC reconfigure — but it is **bench-gated** (F3), so it lands
   as a `#[doc(hidden)]`/explicit-opt-in helper, NOT in `init_record`'s
   default path, until the disco-analyzer bench confirms it drives N→0.

These findings DO NOT unblock a headless calibration in `init_record`. They
DO unblock: (a) the `SerializerArmOutcome` enum + N-detection arithmetic +
unit tests (bench-independent, ratifiable now); (b) a bench-gated re-arm
helper exposed for disco-analyzer to drive. INV-AUDIO-01-4 should be written
to require N=0 *as observed by a caller-supplied calibration*, with the
`CalibrationUnavailable` headless path explicitly conformant in degraded
mode (no regression vs today).

Re-arm primitive candidates (each has costs — pick one with maintainer review):

- **(a) Toggle AIF1ADC1L_ENA / AIF1DAC1L_ENA in R0x004 (PM4) / R0x005 (PM5).** These are the per-direction serializer enables. Toggling SHOULD re-arm the serializer at a fresh BCLK phase without disturbing FLL1 lock or SAI master clocks. Lowest-cost re-arm primitive; should be tried first.
- **(b) Cycle SAI Block A SAIAEN (external, downstream-consumer side).** ERRATA-004's forward-fix path (a). Costs: violates the "never re-touch SAI mid-run" doctrine documented in disco-analyzer's `feedback_no_midrun_sai_stop_restart` memory; requires coordination across the rlvgl-platform / consumer-side boundary.
- **(c) Hardware load-switch on VDD_CODEC.** ERRATA-004's forward-fix path (b). Out of scope for AUDIO-01-d (this is a hardware-modification ask, not a software calibration).

The 2026-05-23/24 bench attempt at ERRATA-004 candidate-d (relocating codec phase A reset + verify_id to immediately precede `init_record`) is closed-broken (see ERRATA-004 entry for detail) and is **not** a viable re-arm primitive — codec rails persist across button-reset with the previous session's wedged state, and the relocated reset cannot get an I²C ACK after SAI1 has already started feeding BCLK. Do not re-attempt that path.

### 3. Helper method on `Wm8994`

Sketch (to be refined by the implementer after the design questions above resolve):

```rust
impl<I2C, E> Wm8994<I2C>
where
    I2C: blocking::i2c::Read<Error = E> + blocking::i2c::Write<Error = E>,
{
    /// Calibrate both AIF1 serializer directions to N=0 BCLK arm phase
    /// relative to LRCLK. Runs after FLL1 lock per AUDIO-01-1 and after
    /// AIF1 LRCLK rate config per AUDIO-01-c, but before `init_record`
    /// returns success.
    ///
    /// Returns `SerializerArmOutcome::Calibrated` on success;
    /// `Misaligned { adc_n, dac_n }` if calibration budget exhausted
    /// with at least one direction still at N≠0; `CalibrationUnavailable`
    /// if no known stimulus is configured (headless boot path).
    ///
    /// AUDIO-01-d normative behaviour.
    fn calibrate_aif1_serializer_arm_phase(
        &mut self,
        stimulus: Option<CalibrationStimulus>,
    ) -> Result<SerializerArmOutcome, Wm8994Error<E>> {
        // TODO(AUDIO-01-d): design + implement per the open
        // questions in AGENT-TASK-WM8994-AIF1-SERIALIZER-ARM-PHASE-CALIBRATION.md
        todo!()
    }
}
```

### 4. Migrate `init_record` to call the new helper

After AUDIO-01-1's FLL1 lock + AUDIO-01-c's LRCLK rate writes, before returning success, call `calibrate_aif1_serializer_arm_phase`. Default stimulus passing should be `None` (CalibrationUnavailable path) for backwards-compat; downstream consumers that have a known stimulus inject it via a new `init_record_calibrated(..., stimulus)` variant or a constructor-time option.

### 5. Extend `init_record` return type

Bubble the `SerializerArmOutcome` into the existing `FllLockOutcome` return type or as a sibling tuple field. Migration-compatible analog of the AUDIO-01-1 return-type extension pattern. Existing in-repo callers (per AUDIO-01-1's migration notes) MAY ignore the new outcome variant initially; downstream consumer disco-analyzer will start branching on it once AUDIO-01-d lands.

## Verification — pre-publish

Standard rlvgl audio-family verification (mirrors AUDIO-01-1 task):

```bash
# Phase 0: format
cargo fmt --check -p rlvgl-platform

# Phase 1: clippy
cargo clippy -p rlvgl-platform --all-targets -- -D warnings

# Phase 2: tests (host)
cargo test -p rlvgl-platform

# Phase 2.5: HAL discipline
# (run rlvgl's existing HAL-discipline lint per repo conventions)

# Phase 4.6: not applicable (platform-side change, not creator)

# Embedded cross-compile sanity:
cargo build -p rlvgl-platform --target thumbv7em-none-eabihf --release
```

The new helper's unit-test surface SHOULD include at least: stimulus-injection scaffolding that exercises the N-detection arithmetic against synthetic captured-sample buffers covering N ∈ {0, 1, 11, 15} (the bench-observed N values from ERRATA-004's characterization).

## Verification — bench

AUDIO-01-d acceptance requires bench evidence on BOTH directions, on the disco-analyzer board (STM32H747I-DISCO) which is the lead downstream consumer.

**ADC side (ERRATA-004 verification):**
- Cold boot from USB plug-in (per disco-analyzer's `feedback_b1_left_codec_wedged` — drain codec rails between attempts).
- `init_record` returns `SerializerArmOutcome::Calibrated`.
- Probe-rs read of SAI1 RX bank shows clean stereo capture of a known line-in source (chromatic sine or 1 kHz test tone): both slots vary across the expected i16 range, no bit-stuck patterns, no half-amplitude on one slot, no 2× overflow wrap.
- Reproducible across at least 10 cold boots (ERRATA-004's bench-26..33 used 16 attempts; aim for the same statistical confidence).

**DAC side (ERRATA-009 verification):**
- Same board, same cold-boot procedure.
- Disco-analyzer in `LoopbackMode::Cm4Loop` (default), with a chromatic sine source on the line-in jack.
- Oscilloscope on the codec output jack: scope shows the audible source frequency **without** the dense high-frequency hash overlay observed in ERRATA-009 image 1.
- Flip to `STIMULUS_MODE = 1` (CM4 synth L=500Hz / R=1kHz): scope shows clean 500 Hz on L and clean 1 kHz on R, **without** the high-frequency hash observed in ERRATA-009 image 2.
- No 93.75 Hz envelope modulation visible at slow time-base on the line-in mode.

Both passes are required to flip ERRATA-004 and ERRATA-009 from 🔴 → 🟢 in disco-analyzer's errata log. The disco-analyzer side will reciprocate with a `docs/concepts/ERRATA.md` §status update + a §15 amendment in disco-analyzer's parent CLAUDE.md cross-references once the AUDIO-01-d pin propagates through the parent rlvgl-platform submodule.

## Workflow

1. ~~Open a memalpha query against `WM8994_Rev4.6.pdf` for documented serializer-arm-phase observability.~~ **DONE 2026-05-29** — six queries run; see "Datasheet research findings (memalpha, 2026-05-29)" above. Result: no observability bit (F1), `AIF1_LOOPBACK` is the wrong path (F2), toggle re-arm undocumented (F3), no force-N=0 knob (F4).
2. **Build §0 H0 (lead candidate) and bench it FIRST.** Apply the serializer-enable reorder (split R0x004/R0x005 analog vs AIF1 enables; defer the b9/b8 serializer bits to after AIF1CLK is running). Run the §0 bench protocol (≥10 cold boots, both directions). This is one branch, one bench round, no new API.
3. **If H0 PASSES:** AUDIO-01-d = the reorder. Land it; amend `docs/audio/01-codec-bringup.md` §15 (write INV-AUDIO-01-4 as an *ordering* invariant, not a calibration loop) and reconcile §12 gate (c) with the code; flip ERRATA-004 / ERRATA-009 → 🟢. Mark §1–§5 below as "considered, mooted by H0." **STOP — steps 4–6 below do not apply.**
4. **If H0 FAILS:** fall through to the closed-loop fallback. Decision on the **detection primitive**: ~~loopback-based vs status-register polling~~ — both refuted by F1/F2; detection MUST be caller-supplied-stimulus (samples captured at SAI1 RX / observed on the analog jack) fed into a platform-side N-detection arithmetic. Decision on the **re-arm primitive**: keep (a) `AIF1ADC1L/R_ENA` / `AIF1DAC1L/R_ENA` toggle (R0x004/R0x005), bench-gated per F3 (explicit-opt-in/`#[doc(hidden)]`, NOT in `init_record`'s default path); fall back to (b) SAIAEN cycle only if the toggle proves ineffective.
5. (Fallback only) Implement per §1–§5 above; run the pre-publish verification staircase locally.
6. Coordinate with disco-analyzer for bench cycles. The bench owner is the operator at the bench during disco-analyzer sessions — they flash a build of disco-analyzer pinned at the rlvgl-platform branch this work lives on, and report bench evidence into ERRATA-004 / ERRATA-009.
7. After bench acceptance, land the AUDIO-01-d amendment in `docs/audio/01-codec-bringup.md` §15.
8. Disco-analyzer reciprocates by bumping the parent rlvgl-platform submodule pin and flipping ERRATA-004 / ERRATA-009 to 🟢 with cross-reference to the resolving rlvgl commit.

## Cross-references

- **AUDIO-01 chapter:** [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md). AUDIO-01-d is the next amendment in §15; INV-AUDIO-01-4 is the new normative invariant to add in §9.
- **AUDIO-01-1 predecessor task:** [`AGENT-TASK-WM8994-FLL1-LOCK-VARIABILITY.md`](AGENT-TASK-WM8994-FLL1-LOCK-VARIABILITY.md) — same shape, same downstream consumer, same outcome-type extension pattern (the new `SerializerArmOutcome` mirrors `FllLockOutcome`).
- **AUDIO-01-c predecessor:** AIF1ADC LRCLK rate fix (R0x304 = 0x0820 RATE=32). AUDIO-01-d builds on AUDIO-01-c — calibration runs after AUDIO-01-c's rate writes have landed and the FLL1 lock per AUDIO-01-1 is confirmed.
- **Downstream consumer errata:** disco-analyzer [`docs/concepts/ERRATA.md`](https://internal/ — file is at `streamz/submodules/disco-analyzer/docs/concepts/ERRATA.md`), entries **ERRATA-004** (ADC, 🔴 open since 2026-05-23) and **ERRATA-009** (DAC, 🔴 open since 2026-05-28). Both errata's EOQs (EOQ-002-ERRATA-004 + EOQ-005-ERRATA-009) point at this task as the joint forward-fix path.
- **External authority:** `WM8994_Rev4.6.pdf` for serializer / framing semantics; STMicroelectronics RM0399 for SAI Block A master-clock / FIFO behaviour.

## Out of scope for this task

- Hardware modifications (VDD_CODEC load switch — ERRATA-004 forward-fix path (b)). That is a separate hardware-engineering ticket if eventually needed.
- AUDIO-01-c re-litigation. R0x304 / R0x305 are confirmed correct at `0x0820` (RATE=32) on the bench; do not revert or re-tune those values.
- Disco-analyzer-side workarounds. AUDIO-01-d is the vendor-side fix; if it doesn't land within a reasonable horizon, disco-analyzer MAY consider a DAA-side workaround under its own errata-tracked process, but that path is independent of and not blocking on this task.
- Re-attempting the codec phase A relocation (ERRATA-004 candidate-d, closed-broken at bench-34/35 2026-05-23/24). The reset-relocation path is not viable; do not re-explore.

## Quick memalpha re-verify queries (optional)

```
mcp__softoboros__memalpha_ask "WM8994 AIF1 serializer arm phase BCLK LRCLK status register bit"
mcp__softoboros__memalpha_ask "WM8994 R0x004 PM4 AIF1ADC1L_ENA toggle re-arm serializer"
mcp__softoboros__memalpha_ask "WM8994 R0x005 PM5 AIF1DAC1L_ENA toggle re-arm serializer"
mcp__softoboros__memalpha_ask "WM8994 R0x301 AIF1_LOOPBACK bit 14 codec internal digital loopback"
mcp__softoboros__memalpha_ask "WM8994 AIF1ADC1 bit-clock-phase race deterministic N init_record"
```

These should confirm or refute the existence of a documented serializer-arm-phase status bit (which would short-circuit the loopback-based detection design) and the safety of the AIF1xDC1L_ENA toggle approach (which is the lowest-cost re-arm primitive).

---

## Bench results — H0 partial success (2026-05-29, disco-analyzer bench session)

H0 reorder applied at rlvgl commit `b202af3` (branch `audio-01-d-h0-serializer-arm-reorder`) and bench-tested via disco-analyzer pinned at the matching parent-repo state. Test stimulus: chromatic-sine line-in source, fixed at 150 mV pk-pk (operator-anchored hardware reference); fallback 75 mV for headroom; full-scale for stress.

### AIF1DAC1 (DAC) side: **FIXED ✓**

- **9 cold-boot iterations** with USB-cycle drains per `feedback_b1_left_codec_wedged`.
- SAI1 RX BUF0 pk-pk symmetric L/R across all 9 boots, ratio 1.000 ± 0.01.
- 130–155 unique values per slot per 256-pair window (not bimodal, no half-amplitude slot).
- **Synth-mode DAC scope on jack: clean ~1 kHz sine, no HF hash overlay, no 93.75 Hz envelope.** Direct comparison to pre-H0 ERRATA-009 image 2 (uniform dense HF hash texture) confirms dramatic improvement.

### AIF1ADC1 (ADC) side: **NOT FIXED ✗**

- Line-in-mode DAC output still shows snowy garbage on the operator's scope, visually similar to pre-H0 ERRATA-009 image 1.
- CM7 on-screen waveform display (independent consumer of the same RX bank via `linein_pool`, not part of the DAC playback path) also shows HF garbage per operator observation. Two independent downstream consumers seeing the same corruption confirms the bug is in the RX bank itself, not downstream of it.
- **Quantitative**: 512-sample FFT on SAI1 RX BUF0 in line-in mode shows HF/LF energy ratio of **2.19 (L) / 2.60 (R)**. For a clean 220 Hz sine source the ratio should be `<< 0.01`. So roughly **200× excess high-frequency energy in the ADC capture path**.
- **Critical**: the corruption is **broadband HF noise, NOT the ERRATA-004 N=15/N=11 2× wrap signature**. Zero wrap-jumps in 512-sample windows. The pk-pk diagnostic also did not catch this — pk-pk looks symmetric and reasonable, all amplitude metrics check out. The HF noise lives entirely above ~4 kHz and the LF/baseband content of the chromatic sine is recoverable from the spectrum.

### Implication for the §0 H0 reorder

H0 was *necessary but not sufficient*. The serializer arm-phase reorder demonstrably fixes the AIF1DAC1 side, but the AIF1ADC1 side has a second, independent root cause that H0 doesn't touch. The DAC-side fix should land regardless (it's a real correctness improvement and a compliance fix for §12 gate (c)), but ERRATA-004 (and the ADC half of ERRATA-009) remain open.

### Hypothesis H1 candidates for the ADC-side residual

Listing what the bench data shape suggests, in rough order of plausibility:

1. **ADC anti-alias / decimation filter not properly initialized post-H0-reorder.** The codec's AIF1ADC1 decimation filter requires SYSCLK (= AIF1CLK in our config) to be running before filter coefficients load. With the H0 reorder, AIF1ADC1L/R_ENA is now armed in Step 14b which is right after R0x200 (AIF1CLK_ENA) + 50 ms settle. If the 50 ms isn't long enough for the filter coefficients to fully load, or if a register write between R0x200 and the Step 14b enable disturbs the filter SRAM, the serializer might arm against an empty/partial filter and the captured stream skips the anti-alias decimation. Result: aliasing of supersonic content into the audible band → broadband HF noise in the capture, exactly matching the FFT signature.
2. **R0x208 Clocking 1 (SYSDSPCLK_ENA bit 1, SYSCLK_SRC bit 0) timing.** R0x208 currently writes `0x000A` at Step 14 (~line 795). The ADC digital filter chain depends on this. If R0x208's enables don't propagate before AIF1ADC1L/R_ENA arms, the decimation chain sees no clock for its filter taps.
3. **R0x410 AIF1 ADC1 Filters interaction.** Currently `0x1800` (HPF enabled, plus bit 12 set). Bit 12 in some WM8994 documentation is the 4FS-mode select for the ADC filter; if H0 changed the effective timing of when this bit's filter bank is loaded, the captured stream might use an uninitialized 4FS filter response. Worth a memalpha query specifically on R0x410 bit 12 semantics.
4. **DRC pumping at near-FS input.** R0x440 currently `0x0098`. DRC is documented disabled (bits 1:0 = 0 = no AIF1ADC1L/R_DRC_ENA), but QR + ANTICLIP are on (bits 4 + 3). At near-FS input from the bench source, the anticlip path might be producing visible spectral modulation. Less likely given the FFT shape is broadband not pumped.
5. **A different N≠0 mode at small N (N=1..3).** Per the ERRATA-004 catalog, the documented N values produce 2× wrap or bimodal patterns. But the catalog might not exhaustively cover small N — N=1 would just shift each sample by 1 bit, producing LSB-region jitter that aggregates into broadband HF noise without obvious 2× wraps. This is a darker-horse but worth checking via spectral analysis on synthetic test vectors.

### What the bench can deliver next

The disco-analyzer side can produce on the bench, with no further code change to rlvgl, additional characterization data to help narrow the ADC-side root cause:

- **Reduce input PGA gain** (R0x18 / R0x1A IN1L/R_VOL) to escape the FS-saturation regime that masked small-N detection in the 9-boot pk-pk test. With input at unambiguously sub-FS (e.g., -20 dBFS), small-N bit-shift would show as amplitude mismatch L vs R or as adjacency-noise.
- **Vary input frequency** with a swept tone (slow chirp). A clean ADC reproduces the chirp; an N≠0 ADC produces audible distortion harmonics at deterministic offsets that vary with input frequency. A DRC-pumping ADC produces gain-modulation sidebands. Distinguishable spectra.
- **Spectral analysis at silence / DC input.** With line-in source disconnected (zero input), the noise floor of the ADC reveals the chain's intrinsic noise. If broadband HF noise persists with no input, it's filter-bypass or DRC-noise; if it disappears, it's signal-correlated (bit shift or compression).

If the rlvgl side wants disco-analyzer to run any specific instrumented bench protocol before the next code revision lands, name it and we'll execute. Otherwise, this update is a status check — H0 lands as a partial fix for AIF1DAC1; ERRATA-004 / ERRATA-009 ADC half remains open with the candidate H1 hypotheses listed above.

### Silence-floor test (2026-05-29 follow-up) — H1 #1 confirmed lead candidate

After the H0 partial-success result, operator ran the silence-floor discrimination test (source physically disconnected from line-in jack):

**Visual observation:** CM7 waveform display shows "almost flat" trace — initially read as healthy noise floor.

**Actual FFT-quantified result:**
- L silence pk-pk = **57288**, RMS ≈ 3286, top bins at **21.7-22.1 kHz**
- R silence pk-pk = **62563**, RMS ≈ 4047, top bins at **23.3-23.9 kHz**
- HF (>4 kHz) magnitude sum: L = 15.5M, R = 18.5M
- LF (50 Hz - 1 kHz) magnitude sum: L = 133k, R = 648k

The "almost flat" visual is consistent with the FFT: the noise is concentrated at near-Nyquist frequencies (21-24 kHz) which oscillate too fast to render as visible structure on the time-domain display — every pixel column averages to a thick uniform band that looks flat.

This is the **classic decimation-filter-bypass signature**. A sigma-delta ADC's raw modulator output (before decimation) is broadband near-Nyquist noise — if the decimation filter is bypassed or not yet loaded when the AIF1ADC1 serializer arms, the captured stream carries this raw modulator output. With no analog input at all, the noise floor is dominated by the modulator's quantization noise concentrated near Nyquist, exactly matching what we see.

Asymmetric per-channel peak frequencies (L at ~22 kHz, R at ~24 kHz) further suggest the two decimation filters may be in different load states at the moment the serializer arms — possibly an even-vs-odd channel asymmetry in the filter loading sequence, or different SYSCLK-relative phases.

**This promotes H1 candidate #1 from "candidate" to "lead":** the H0 reorder fixed the DAC serializer timing but exposed (or failed to address) an ADC-side filter-load race. Recommended next investigation directions:

1. **Increase settle time in Step 14b** from 50 ms to a measured value sufficient for filter-coefficient load completion. Memalpha query suggestion: `mcp__softoboros__memalpha_ask "WM8994 AIF1ADC1 decimation filter coefficient load time after SYSCLK enable"`.
2. **Add explicit filter-ready poll** if the WM8994 exposes any status register indicating decimation filter ready (analogous to FLL1_LOCK_STS for FLL1). Memalpha query: `"WM8994 ADC decimation filter ready status register"`.
3. **Reorder R0x208 Clocking 1** (`SYSDSPCLK_ENA` bit 1) writes to land BEFORE R0x200 (AIF1CLK_ENA), or split it analogously to how H0 split R0x004/R0x005 — ensure the DSP clock is fully running before the AIF1 master clock and serializer come up.
4. **Investigate R0x410** (`AIF1 ADC1 Filters`) for any "force filter load" or "filter reset" bit. Currently reads `0x1800` (HPF_ENA + bit 12). If bit 12 controls 4FS-mode decimation, its timing relative to AIF1CLK_ENA may matter.

The silence-floor result is the most diagnostic bench data so far for the ADC-side residual. Adding it to H1's evidence base; H1 candidates #2 (small-N bit shift) and #4 (DRC pumping) can be deprioritised — neither would produce silence-floor noise concentrated specifically at near-Nyquist with the asymmetric L/R peak shape we see.

### Round-2 result — post-arm settle NOT sufficient (2026-05-29 follow-up)

Bench tested rlvgl commit `7e6589e` ("AUDIO-01-d H0: add post-arm settle to Step 14b round-2 refinement") on the same disco-analyzer setup as round-1. Source disconnected throughout (silence-floor protocol).

**Result: silence-floor signature unchanged in magnitude; spectral shape shifted but corruption pattern persists.**

| Metric | R1 H0 (b202af3, warm) | R2 +postarm (7e6589e, warm) | R2 +postarm (7e6589e, **cold**) |
|---|---|---|---|
| L pk-pk | 57288 | 57288 | 57096 |
| R pk-pk | 62563 | 62563 | 62563 |
| L RMS | 3286 | 3530 | 4048 |
| R RMS | 4047 | 4377 | 4688 |
| L HF (>4 kHz) sum | 15.5M | 16.4M | 17.4M |
| R HF (>4 kHz) sum | 18.5M | 19.7M | 21.0M |
| L top bin | ~22.0 kHz | 21.7 kHz | 21.0 kHz |
| R top bin | ~23.6 kHz | 21.0 kHz | **12.75 kHz** |

**Two diagnostic-strength observations:**

1. **R pk-pk = 62563 across all three independent runs, byte-for-byte identical** (and L pk-pk = 57288 across the two warm runs). The codec is hitting a **deterministic state** at the silence-floor extremes — these are not stochastic noise values. This is the same deterministic-latch class of behavior ERRATA-004 documents for the AIF1ADC1 arm-phase race, just expressed as deterministic-noise-floor-pk-pk rather than deterministic-bit-stuck-pattern.

2. **R top bin moved from ~23.6 kHz (R1) → 21.0 kHz (R2 warm) → 12.75 kHz (R2 cold)** — a ~half-Nyquist shift between R2 warm and R2 cold. The post-arm settle changed *something* in the codec's internal state but doesn't fix the underlying corruption; the cold-boot path additionally re-latched into a different but still-corrupted state.

**Reframing the ADC-side hypothesis:**

Post-arm settle (and pre-arm settle extension by extension) won't address this. The mechanism is not "give the serializer / filter time to settle" but rather "the AIF1ADC1 serializer is arming against an undefined-but-deterministic clock-phase that produces broadband near-Nyquist content in the captured stream." This is precisely the symmetric AIF1ADC1 manifestation of the same arm-phase race H0 hypothesised — but with a different spectral signature than the 2× wrap that ERRATA-004's catalog assumed.

**Proposed H2 candidate set (vendor-side decision needed):**

- **H2a:** Apply H0's reordering principle to AIF1ADC1 separately. The R0x004 b9/b8 (ADC enables) and R0x005 b9/b8 (DAC enables) are currently OR'd together at the same Step 14b. If the ADC path needs a different ordering pivot than DAC — e.g., must wait for FLL1-lock confirmation completion, or must follow R0x208 (Clocking 1 SYSDSPCLK_ENA propagation) — then splitting them lets each direction's enable land at the right moment. Test plan: defer just R0x004 b9/b8 to a separate later step, leave R0x005 b9/b8 at current Step 14b; bench-test to see if signature changes.
- **H2b:** Sigma-delta + decimation chain race independent of the I²S serializer. The near-Nyquist content suggests the codec's ADC is running but the decimation filter is either bypassed-by-config or operating in a mismatched-rate mode. Memalpha worth checking: WM8994 R0x410 bit 12 semantic (4FS-mode enable?), R0x300 bit 14 (AIF1ADCR_SRC) routing, R0x210 AIF1CLK_RATE expected value for our 48 kHz / 256x MCLK config. If a register is wrong, no settle change will fix it.
- **H2c:** Codec input bias / PGA noise being amplified. With input disconnected and PGA at ~30 dB gain, codec internal noise (charge-pump switching, FLL1 reference leak) gets amplified to near-FS at the ADC output. Test plan: bench-side reduces R0x18 / R0x1A (IN1L/R_VOL) to 0 dB and re-runs silence-floor. If pk-pk drops dramatically, it's PGA noise amplification not serializer race.

**Bench-side will not act on this autonomously** — handing back for vendor-side decision on which H2 candidate to chase first. H2c is the cheapest test for bench-side to validate (one I²C write, no rebuild); H2a is a small rlvgl-side change; H2b needs memalpha + a config change.

### H2c falsification — input PGA noise amplification ruled out (2026-05-29)

Bench-side ran H2c using the existing CM7 codec-write mailbox at SRAM4 `0x3800_2040..0x3800_204F`. Reset board, used the mailbox to drop both `R0x18` (IN1L_VOL) and `R0x1A` (IN1R_VOL) from 0 dB (current code = `0x000B`) to -16.5 dB (`0x0100` = vol=0 + VU=1 + bit-8 commit). Mailbox ACKed both writes with `0xC0DE_0001` / `0xC0DE_0002` (status OK). Released the drain loop, sampled silence floor.

**Result: pk-pk unchanged.**

| Metric | Pre-H2c (PGA 0 dB) | Post-H2c (PGA -16.5 dB) |
|---|---|---|
| L pk-pk | 57096 (cold-boot reference) | **57096** |
| R pk-pk | 62563 | **62563** |
| L HF (>4 kHz) sum | 17.4M | 15.0M |
| R HF (>4 kHz) sum | 21.0M | 18.8M |
| L top bin shape | 21 kHz peak | DC + 17.5 / 21.8 kHz peaks |
| R top bin shape | 12.75 kHz peak | back to ~23.6 kHz |

Pk-pk is byte-identical to pre-H2c. Spectral shape changed modestly (the codec internal state IS responding to the write — particularly R's top bins moved from 12.75 kHz back to near-Nyquist, and L picked up a DC component), confirming the I²C writes did land. But none of that touches the pk-pk envelope.

**Conclusion: silence-floor noise is generated DOWNSTREAM of the input PGA**, inside the codec's ADC analog-to-digital + decimation + AIF1ADC1 serializer chain. PGA-noise-amplification hypothesis is falsified — reducing PGA gain by 16.5 dB does not attenuate the captured noise floor.

This narrows the remaining ADC-side root-cause candidates to:

- **H2a** — separate AIF1ADC1 enable from AIF1DAC1 enable at different ordering pivots (currently both at Step 14b). Vendor-side code change.
- **H2b** — decimation-chain config issue (R0x410 bit 12 semantic, R0x300 b14/b15 AIF1ADCR/L_SRC routing, R0x210 AIF1CLK_RATE expected value for our config). Vendor-side memalpha + possible code change.

Bench-side has exhausted what it can do without an rlvgl-side revision. Both H2a and H2b are vendor-side decisions. Recommend the rlvgl thread pick the cheaper of the two to test first (H2a probably, as it follows the H0 reordering principle that's already proven its mechanism on the DAC side).

## Round-3 (2026-06-07, disco-analyzer bench) — bi-directional confirmation + caller-supplied-stimulus harness validated

This round closes the §2 design loop: the "caller-supplied stimulus + capture" architecture (Consequences #3) is now **physically realized and proven** on disco-analyzer, the **DAC side is confirmed to need the same calibration** (not just H0), and we have **real captured N-vectors** to drive the platform-side N-detector unit tests.

### R3.1 — The DAC and ADC corruption are one root cause (mono test rules out L/R coding)

disco-analyzer added a CM4 mono stimulus (`stimulus_mode == 2` → identical L==R==1 kHz on both AIF1 slots; two-tone `== 1` stays L=500 Hz / R=1 kHz). With the **H0-clean DAC build** (rlvgl branch `audio-01-d-h0-serializer-arm-reorder`) flashed and the mono tone played:

- TX bank verified clean mono (identical L/R pairs).
- Operator scoped the DAC analog output: **1 kHz fundamental present on both channels, buried in HF hash, not a discernible sine.**

Because identical L/R still hashes, an L/R interleave / slot-swap fault is **ruled out** (it would scope clean under mono). The DAC carries the ERRATA-009 "recognizable tone + HF hash" signature **despite H0** — i.e. H0's deferral reduced but did not eliminate the DAC-side arm-phase race. Combined with the ADC results below, this confirms the task's bi-directional framing: **AIF1ADC1 and AIF1DAC1 are the two directions of one arm-phase root cause**, and the calibration (arm → detect → re-arm) is required on **both**, not deferral alone.

### R3.2 — Caller-supplied-stimulus capture channel is built and proven (validates Consequences #1, #3)

The §2 findings concluded detection cannot be headless and must be caller-supplied. disco-analyzer now provides exactly that channel:

- **Stimulus source:** CM4 synthesises a known tone into the AIF1DAC1 TX bank (`stimulus_mode`: 1 = L500/R1k two-tone for slot-mapping checks, 2 = mono-1k for pure serializer-phase checks).
- **Loopback:** an external physical wire from the codec line/HP output back into line-in carries the DAC analog output into the ADC — closing DAC→analog→ADC so a single capture exercises both serializers.
- **Capture + display path:** a new CM7 control word `DISPLAY_SHOW_INPUT` (`0x3800_2078`) decouples the scope/FFT display source from the TX synth, so the **real captured ADC samples** (raw SAI1 RX `0x3000_0000`, or the `linein_pool` blocks at `0x3800_E000`) are observable while the synth plays. probe-rs reads the raw RX directly — upstream of the pool/cross-core/display.

This is the concrete instantiation of the design's "downstream consumer owns stimulus + capture; platform owns N-detection arithmetic + re-arm action." The platform's `calibrate_aif1_serializer_arm_phase(stimulus: Some(...))` path now has a real consumer that can feed `(known_stimulus, captured_samples)` and apply the re-arm toggle in a loop.

### R3.3 — Real N-vectors for the platform-side N-detector unit tests

Captured raw SAI1 RX (256 stereo frames, 48 kHz), **H0-clean DAC source**, two boots that armed at different N — concrete fixtures for the host-testable detector arithmetic (§3 `SerializerArmOutcome`):

| Boot | Arm phase | pk-pk | RMS | adj. \|Δ\|>16k | sign-flip | low-byte pattern | DFT |
|---|---|---|---|---|---|---|---|
| warm reset | **N≈10–15 (byte-stuck)** | ~8,000 | ~1,470 | 0/255 | 73–76% | low byte stuck `00`/`ff` (~8 eff. bits) | no tone; energy 15–21 kHz |
| cold boot | **N≈−1 (2× wrap)** | ~64,800 | ~18,300 | ~140/255 | 63–67% | full-range | no tone; sigfrac 0.02–0.06 |
| *(target)* | **N=0 (clean)** | ≈ 2× synth amplitude | ≈ synth RMS | 0/255 | low (≈ tone) | full 16-bit, smooth | single peak at stimulus freq, signal-bin fraction → 1 |

Detector discriminators (all computable host-side on a captured `[i16; N]` vs the known stimulus, no codec/datasheet dependency):

1. **Adjacent-jump count** `|x[i+1]−x[i]| > 0x4000` → high = 2× overflow-wrap (N≈−1).
2. **Low-byte stuck rate** (fraction of samples with low byte ∈ {`0x00`,`0xff`}) → high = byte-stuck (N≈8–15).
3. **Signal-bin energy fraction** (DFT energy at the known stimulus bin ±1 / total) → ≈1 = N=0 clean; ≈0 = corrupted.
4. **Cross-correlation / phase offset** against the known stimulus → recovers N directly when the capture is partially coherent.

`N=0` accept gate (proposed INV-AUDIO-01-4 observable form): adjacent-jump count = 0 **and** low-byte-stuck rate < 5% **and** signal-bin fraction > 0.8 on the mono-1k stimulus.

### R3.4 — Closed-loop calibration sequence (the disco-side harness contract)

Concrete loop the consumer drives, using re-arm primitive (a) (R0x004/R0x005 enable toggle), now with a working capture channel:

```
for attempt in 0..BUDGET:
    play known mono stimulus (CM4 stimulus_mode=2)               # consumer
    capture 256+ frames at SAI1 RX 0x3000_0000                    # consumer
    outcome = platform.detect_arm_phase(&captured, &stimulus)     # rlvgl-platform, host-testable
    if outcome.n == 0 and gate passes: break -> Calibrated
    platform.rearm_aif1_serializers()                             # rlvgl-platform: toggle R0x004/R0x005 enables
# budget exhausted -> Misaligned { adc_n, dac_n }   (degraded, no regression)
```

Open item the bench must still answer (F3 was undocumented): **does toggling R0x004/R0x005 enables actually re-roll N within a powered session?** ERRATA-004 bench-23..33 found `init_record` *reruns* are deterministic (do NOT re-roll N), and the only re-roll we have *observed* is a full codec power-cycle (warm→byte-stuck vs cold→wrap this session). If the enable-toggle does not re-roll N, primitive (a) is insufficient and the loop needs primitive (b) (SAIAEN cycle to force FLL re-lock + re-arm) under the SAI-re-touch doctrine carve-out. **This is the single highest-value next bench measurement** and it is now runnable: in the boot-window codec mailbox (`0x3800_2040`), toggle R0x004 bits 9/8 off→on, re-capture RX, check whether the byte-stuck/wrap signature changes. If it changes → (a) works, the headless-ish calibration is viable; if not → escalate to (b).

### R3.5 — Recommendation update

- **H2a (per-direction arm pivot) remains the lead** vendor-side ordering change, now with bi-directional motivation (DAC needs it too, R3.1).
- The **N-detector arithmetic + `SerializerArmOutcome` + unit tests are ratifiable and implementable now** (host-testable against the R3.3 vectors; no bench dependency) — this is the unblocked slice.
- The **re-arm-toggle re-roll question (R3.4)** gates whether primitive (a) suffices or (b) is required; the disco-analyzer loopback rig can answer it next bench round.
- disco-analyzer should **move its rlvgl pin onto an H0-containing ref** so the default build at least carries H0's partial DAC improvement (today's `main`/`bc69338` pin predates H0, so the default build ships a fully-broken DAC).

Cross-ref: disco-analyzer `docs/concepts/ERRATA.md` ERRATA-009 (2026-06-07 bench chain) + ERRATA-004; memory `project_daa_adc_isolated_loopback_2026-06-07`.

## R4 — disco-analyzer bench 2026-06-13 (bit-shift = N = amplitude scaling; transport ruled out)

Two findings from the disco loopback rig that sharpen the target:

1. **The "bit or two shift" IS the serializer N, and it manifests as amplitude scaling.**
   Operator observation across power cycles: one boot came up half-scale, the
   next full-scale, with "a bit or two shift" in the captured data. This is one
   mechanism, not two: the AIF1ADC1 serializer arming N bit-clocks off LRCLK
   shifts the 16-bit sample window by N bits, so the captured magnitude scales
   by 2^±N (left-shift = ×2 = full scale; right-shift = ÷2 = half scale). The
   boot-variable amplitude we kept chasing as a "gain/de-clip" issue was the
   boot-variable N all along. So a clean acceptance gate is **not just N==0 but
   unity scale** (captured amplitude == expected ±1 bit), which is cheaply
   checkable against the known SINE_48 stimulus — this strengthens the
   `detect_arm_phase` N-detector (R3.3): N is directly readable from the
   power-of-2 amplitude error, not only from a cross-correlation.

2. **The discontinuity is transport-independent → it's the serializer, confirming H2a is the lead.**
   disco A/B (2026-06-13): the CM7 FFT/scope feed was moved off the lossy
   cross-core pool (CM4→D3-SRAM4, the ERRATA-019 store-loss path) onto a
   **direct read of the SAI1 RX bank** (every bank, ~190/s). Phase-discontinuity
   was **~28%** direct vs ~18.6% pool — i.e. bypassing the entire transport did
   NOT clean it up. So the on-screen bounce is upstream of any buffering: it is
   the capture (serializer N varying/glitching), not the pool/DMA transport.
   Corollary: the planned circular-DMA buffering rework is the wrong layer for
   this bug and is parked until the capture is bit-clean.

**R3.4 is now primed to run** (the gating measurement): with the codec in a
known bit-shifted state, toggle R0x004 bits 9:8 off→on via the boot-window codec
mailbox (`0x3800_2040`) and check whether the captured amplitude/alignment
signature changes. Changed ⇒ enable-toggle re-rolls N ⇒ primitive (a) re-arm
calibration loop is viable; unchanged ⇒ ERRATA-004's "reruns are deterministic"
extends to enable-toggles ⇒ escalate to primitive (b) (SAI re-touch under the
doctrine carve-out) or the MCLK1-unlocked **arm-before-frame-clock** ordering
(now feasible: AIF1CLK←MCLK1 removes the BCLK-for-FLL dependency, so the codec
serializer can be enabled before SAI BCLK/FS start and sync to the first frame
edge — a deterministic-N candidate not available under the FLL-from-BCLK path).

Cross-ref: disco `docs/concepts/ERRATA.md` ERRATA-004 (+ ERRATA-019 for the
transport store-loss that R4#2 rules out as the bounce cause).

## R5 — H1 (arm-before-frame-clock via LRCLK_DIR defer) lands + bench result (2026-06-14)

**Implemented + committed** (`platform/src/wm8994.rs`, rlvgl v0.2.4 `bd5b3cb`). H1 is
the missing complement to H0: H0 deferred the serializer ENABLE (R0x004/005 b9:8)
to Step 14b but left LRCLK_DIR=1 (reception ON) at Step 13, so the serializer still
armed mid-frame against an already-running LRCLK1. H1 **also defers LRCLK_DIR=1**:

- Step 13 → `R0x304/305 = 0x0020` (RATE=32, **LRCLK_DIR=0** — codec ignores the
  running LRCLK1; AIF1 framing held off).
- Step 14b → serializer enable (unchanged).
- **NEW Step 14c → `R0x304/305 = 0x0820`** (LRCLK_DIR=1) — the already-enabled
  serializers sync to the FIRST received LRCLK1 frame edge.

**Bench (disco-analyzer, 10 probe-rs resets, 96-sample captures):** EVERY boot
smooth (adjacent-jump count = 0), zero byte-stuck/wrap/bimodal corruption —
a complete elimination of the pre-H1 chaos (the boot lottery of 2×/byte-stuck/
bimodal/silent). **The N≠0 *corruption* family is gone.** This answers F4/§0:
arm-before-frame-clock is the deterministic-cleanliness fix; an explicit resync
register was confirmed absent (memalpha WM8994 datasheet query 2026-06-14).

**Residual (NOT corruption):** a 2-state amplitude — captures cluster at ~26% FS
or ~3% FS (a clean 3-bit right-shift), both smooth + corruption-free. So H1 pins
*cleanliness* (no torn/stuck bits) but not yet *unity scale*; N still has a small
clean residual (right-shift, two states). Candidate follow-ons: (i) a further
ordering tweak to pin scale, (ii) the R3.3 detector's unity-scale check + a 1-shot
software normalize (the capture is clean, just scaled), (iii) accept it (clean
display at boot-variable gain).

**Methodology correction (supersedes R4#2's "bounce is transport-independent →
codec"):** with H1 making the codec capture smooth, the disco CM7 *direct-feed*
still shows ~30% **time-domain** phase-discont over 20 s — but that is the
**direct-feed's own DBM bank-handoff loss** (CM7 misses ~30% of banks; the 2-bank
DBM keep-up/dedup limit), NOT the codec (96-sample captures are smooth). So the
bounce has TWO layers: codec (H1-fixed) + transport (pool store-loss / direct-DBM
miss). The DAA-04-G circular-DMA + NDTR-drain **un-parks** — it is the correct fix
for the transport layer now that the codec confound is removed.

**Still pending:** ≥10 COLD power cycles (rails drained) to confirm H1 holds
cold (resets retain analog/VMID state). Both-direction (DAC) check.

Cross-ref: disco `docs/concepts/ERRATA.md` ERRATA-004/-009; DAA-04-G (transport).
