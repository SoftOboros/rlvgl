<!--
01-codec-bringup.md - AUDIO-01: WM8994 codec bring-up concepts. FLL1 lock
invariant + AIF1ADC1 serializer arm ordering.
-->

**[Index](README.md) · [Next →]** *(AUDIO-02 not yet authored)*

# AUDIO-01 — Codec Bring-Up Concepts (FLL1 Lock & Serializer Arm Ordering)

## §0 Authority policy

This chapter has multiple external authorities. Each frozen decision
below cites which authority owns the underlying invariant.

| Authority | Scope | Cite shape |
|---|---|---|
| `WM8994_Rev4.6.pdf` (memalpha-indexed) | Codec register layouts, bit assignments, lock semantics | `(WM8994 p.NNN)` |
| ST RM0399 | SAI peripheral framing, BCLK derivation, DMA double-buffer | `(RM0399 §NN.N)` |
| `platform/src/wm8994.rs:NN` | Driver-canonical names (`Wm8994`, `init_record`, `InputDevice`) | `(wm8994.rs:NN)` |
| `platform/src/sai.rs:NN` / `platform/src/dma_sai.rs:NN` | Driver-canonical names for SAI bring-up | `(sai.rs:NN)` |

When this chapter and a cited authority disagree, the cited authority
wins for the term it owns. This chapter does not redefine bit
assignments documented in `WM8994_Rev4.6.pdf` — it only freezes
**ordering and timing** invariants that the datasheet documents
informally or not at all.

## §1 Purpose

Freeze the platform-side invariants that govern WM8994 record-path
bring-up on rlvgl. Specifically: the timing relationship between FLL1
lock acquisition and the arming of the AIF1ADC1 digital serializer.
Misordering produces an undefined-but-persistent bit-clock phase that
corrupts the captured audio in ways that survive every analog-side
test (sidetone, line-out playback) and only manifest in the digital
capture path. The same misordering accounts for the slot-coin-flip
behaviour where the captured stereo asymmetry inverts L↔R across
power cycles.

## §2 Problem statement

The WM8994 codec's AIF1ADC1 serializer generates its bit-clock from
SYSCLK (`WM8994 p.284`). SYSCLK is sourced from AIF1CLK, which in
turn is the FLL1 output (`platform/src/wm8994.rs:316–322`). FLL1 takes
30–50 ms to acquire lock against BCLK1 as F_REF (`WM8994 p.202`); the
current driver waits a static 20 ms after writing `FLL1_ENA = 1`
(`platform/src/wm8994.rs:610–611`):

```rust
self.write_reg(REG_FLL1_CTRL_1, 0x0001)?; // FLL1_ENA=1 (set LAST per p.202)
delay_busy(20_000); // FLL settle ~20 ms
```

If FLL1 has not yet locked when the post-init AIF1 register writes
land (`platform/src/wm8994.rs:621–702`), the serializer's bit-clock
state machine has armed against an unstable SYSCLK. The state machine
latches the bit-clock phase relative to LRCLK1 at this point; the
phase persists until the next serializer re-arm (effectively a power
cycle, since `init_record` does not re-arm mid-runtime).

Observed downstream symptoms vary with the bit-clock phase offset N:

- **N = 15**: one slot emits `0x0000 / 0x8000` bimodal toggle (OB-
  encoded `-FS / 0`). The other slot carries the actual ADC data
  unmolested. Documented in disco-analyzer bench sessions 9n / 10l /
  10m / 13 / 16 (which slot is bad flips across boot).
- **N = 11**: both slots carry truncated views of one source sample
  — slot 0 receives `sample & 0x07FF` (lower 11 bits), slot 1
  receives `sample & 0xF800` (upper 5 bits). Appears as a ~57 dB
  level asymmetry on a 1 kHz sine source; the "quiet" channel is a
  bit-truncated copy of the loud channel, not silence. Documented in
  disco-analyzer 2026-05-19.
- **N = 0**: clean stereo. This is the intended boot state and the
  outcome the fix in §9 INV-AUDIO-01-1 makes deterministic.

The variability across boots ("press the reset button and it works")
arises because warm reset leaves BCLK1 already running on the wire
from the prior session — FLL1 then re-locks quickly. Cold boot (USB
plug-in) starts FLL1 against a power-up-transient F_REF and frequently
mis-locks. The 20 ms / 100 ms static delays cover the average case
but not the worst case.

The chronic appearance of this bug — "the digital capture path on this
codec is broken" — is what motivated this chapter; promoting the
diagnostic into a frozen invariant gives every future audio-path
consumer a single piece of doctrine to depend on instead of
re-deriving the timing from bench data.

## §3 Canonical glossary

- **FLL1** — Frequency Locked Loop 1, internal to the WM8994. Locks
  to F_REF (typically BCLK1 in slave AIF1 configurations) and
  synthesises F_OUT used as AIF1CLK. **As defined in `WM8994 p.202`;
  used without modification.**
- **FLL1 lock** — the state where FLL1's F_OUT phase and frequency
  have settled to within the codec's internal tolerance of the
  intended ratio. The codec reports the *current* lock state via the
  level/raw status bit `FLL1_LOCK_STS` at **R0x732 bit 5** (0 = not
  locked, 1 = locked), `WM8994 p.346`. The codec separately latches
  lock-state *transitions* in `FLL1_LOCK_EINT` at R0x731 bit 5
  (`WM8994 p.297`); this driver polls the level status, not the edge
  latch — see §15 amendment 2026-05-19-b for the bench-driven
  rationale. **As defined in `WM8994 p.346`; used without
  modification.**
- **AIF1ADC1 serializer** — the codec-internal block that takes the
  digital output of the ADC1 stage and shifts it onto the AIF1ADCDAT
  pin one bit per BCLK1 edge, synchronised against LRCLK1.
  **As defined in `WM8994 p.171`; used without modification.**
- **Serializer arm** — the point at which the serializer's bit-clock
  state machine begins generating BCLK-aligned shift-out events.
  Implicit in the codec's AIF1ADC1L_ENA / AIF1ADC1R_ENA bits
  (R0x4 bits 0–1). **Owned by AUDIO-01; the datasheet does not name
  this transition.**
- **Bit-clock phase offset N** — the number of BCLK ticks between
  the codec's intended bit position 0 of slot 0 and where the
  serializer actually places bit position 0 of slot 0. N is sampled
  at serializer arm time and held until the next arm. **Owned by
  AUDIO-01.**
- **Cold boot** — power-on from a power-removed state (USB unplug
  → USB plug). Distinct from warm reset (NRST button, probe-rs
  reset, or `SCB::sys_reset()`) because BCLK1 is not yet running on
  the wire when the codec first sees power. **Owned by AUDIO-01.**

## §4 Source-of-truth map

For each named concept, exactly one owner:

| Concept | Owner | Reason |
|---|---|---|
| WM8994 register addresses + bit positions | `platform/src/wm8994.rs` constants | per [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-001 outcome 2026-04-30 |
| WM8994 register *semantics* (what each bit means, default value, polarity) | `WM8994_Rev4.6.pdf` via memalpha | datasheet is canonical |
| WM8994 init_record sequence body | `platform/src/wm8994.rs::init_record` | code is canonical |
| FLL1 lock-status detection | This chapter §9 INV-AUDIO-01-1 | new invariant; no prior owner |
| FLL1 lock retry budget | This chapter §9 INV-AUDIO-01-2 | new invariant |
| `Wm8994::FllLockOutcome` enum | This chapter §6 (frozen) → mirrored in `platform/src/wm8994.rs` | spec authors; code mirrors |

## §5 (reserved for future audio-path frozen enums — pixel-format-equivalent for sample formats)

## §6 Frozen enum — `FllLockOutcome`

Returned by the FLL1-lock-aware init path. Three variants. Registration
policy: **Standards Action** (adding a value requires a §15 amendment
to this chapter, since this enum crosses the rlvgl-platform-vs-
consumer API surface).

```rust
pub enum FllLockOutcome {
    /// FLL1 reported lock within the first poll interval. Healthy boot.
    LockedFirstTry,
    /// FLL1 acquired lock only after one or more re-arm cycles. Caller
    /// SHOULD log this for telemetry: a warm-reset path that consistently
    /// retries indicates the static delay tuning is wrong; a cold-boot
    /// path that consistently retries is expected.
    LockedAfterRetry { attempts: u8 },
    /// FLL1 never acquired lock within the configured retry budget. The
    /// codec is in an undefined state; the caller SHOULD treat the
    /// `init_record` Result as a hard error and either log + return
    /// `Err` or panic depending on platform policy.
    Failed,
}
```

## §7 (reserved for SAI / DMA invariants — future chapter material)

## §8 (reserved)

## §9 Frozen invariants

### INV-AUDIO-01-1 — FLL1 lock MUST be confirmed before AIF1ADC1 serializer arm

After writing `FLL1_ENA = 1` to R0x220, the driver MUST:

1. Poll R0x732 bit 5 (FLL1_LOCK_STS, level/raw status) with a
   per-attempt timeout. A read returning bit 5 = 1 means FLL1 is
   currently locked.
2. On bit 5 = 1 within timeout: continue init. Return
   [`FllLockOutcome::LockedFirstTry`] from `init_record` (if the
   loop completed on the first attempt) or
   [`FllLockOutcome::LockedAfterRetry`] (otherwise).
3. On timeout: write `FLL1_ENA = 0`, wait briefly (~5 ms), re-write
   the FLL config registers (R0x224, R0x223, R0x222, R0x221), re-
   write `FLL1_ENA = 1`, and re-poll. Retry up to `MAX_RETRIES`
   (§9 INV-AUDIO-01-2) times.
4. If MAX_RETRIES is reached without lock: return
   [`FllLockOutcome::Failed`] from `init_record` (or `Err` if the
   caller does not consume the enum directly).

The serializer MUST NOT be armed (the `AIF1ADC1L_ENA` / `AIF1ADC1R_ENA`
writes in R0x4 MUST NOT have been issued, and the post-FLL AIF1 framing
writes at `wm8994.rs:621–702` MUST NOT have been issued) until either
[`FllLockOutcome::LockedFirstTry`] or [`FllLockOutcome::LockedAfterRetry`]
has been returned. This is the load-bearing invariant; absent this
ordering, the bit-clock-phase-offset symptoms in §2 are not preventable.

**Why level polling (R0x732 bit 5) and not edge latching (R0x731
bit 5):** the initial AUDIO-01 ratification specified the edge latch
`FLL1_LOCK_EINT` with a clear-then-poll sequence. Bench validation
2026-05-19 (see §15 amendment 2026-05-19-b) found that the edge latch
never fires under our init_record sequence — the clear-write at the
start of each attempt was racing with the actual lock transition in
a way the hardware decided to lose, leaving the latch at 0 indefinitely
even after FLL1 successfully achieved lock. Level polling (R0x732
bit 5) reports the *current* lock state with no transition-timing
window; it is robust against arbitrary clear-vs-transition races.

### INV-AUDIO-01-2 — Per-attempt timeout and retry budget

- Per-attempt poll timeout: **100 ms**. Worst-case observed FLL1
  lock time on the WM8994 is 50 ms (`WM8994 p.202`); 100 ms gives
  2× margin.
- Maximum retries: **3** (so up to 4 attempts total).
- Poll interval inside the timeout: **1 ms**. Coarser is acceptable;
  finer adds unnecessary I²C bus traffic.

These values MAY be tuned per platform if bench data demands it, but
the tuning is **Specification Required** (per CLAUDE.md spec-before-
code) — i.e. requires a chapter walkthrough update, not a free PR.
Setting timeout < 50 ms or retries < 1 is forbidden absent a §15
amendment.

### INV-AUDIO-01-3 — `init_record` return type extension

`Wm8994::init_record` returns
`Result<FllLockOutcome, I2C::Error>` as of this chapter's
ratification, replacing the prior `Result<(), I2C::Error>`. Callers
that previously discarded `()` MAY discard `FllLockOutcome` via
`let _ = wm8994.init_record(...)?;` to preserve existing call sites
without semantic change. Callers that want telemetry SHOULD match on
the enum.

This is a **breaking API change at the type-signature level** but
non-breaking at the behavioural level for existing callers — the
new error return semantics in `FllLockOutcome::Failed` were
previously masked as "init_record returned Ok but the audio path
never worked," i.e. a silent failure. Making it an explicit enum
surfaces what was previously a latent bug class.

## §10 Reconciliation vs adjacent repo primitives

This chapter does not modify:

- The bit-position assignments in `platform/src/wm8994.rs` constants
  (those are owned by [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-001 and
  mirrored in code).
- The SAI1 / DMA1 bring-up in `platform/src/sai.rs` /
  `platform/src/dma_sai.rs` (those are owned by future chapters in
  this family).
- The `delay_busy` primitive (`platform/src/wm8994.rs:NN`) — the new
  poll loop uses the same primitive for its 1 ms inter-poll interval.

This chapter does modify:

- `Wm8994::init_record` return type (per §9 INV-AUDIO-01-3).
- The body of `init_record` between the FLL1 config writes and the
  post-FLL AIF1 register writes (per §9 INV-AUDIO-01-1).

Existing in-repo callers (`platform/src/audio_player.rs`,
`examples/stm32h747i-disco/src/audio_scope.rs`) are migrated in the
same PR that lands this chapter (the AGENT-TASK doc names the file
list).

## §11 Non-goals

- This chapter does NOT cover the analog input path (Input PGA,
  MIXIN, line-in routing). Analog-path config is symmetric L vs R
  in every bench session that exhibited the digital asymmetry; the
  bug is digital, not analog. Future AUDIO-NN chapters may cover
  analog if a bug surfaces there.
- This chapter does NOT cover SAI Block B sync-with-A configuration.
  That's a separate invariant (Block B's slot framing must match
  Block A's master clock generation) that deserves its own chapter.
- This chapter does NOT cover the chronic AIF1ADCDAT-silent issue
  tracked in [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-001 — that was
  resolved by adding R0x304/R0x305 bit 11 (LRCLK_DIR slave-mode
  enable) and is now in the `init_record` body unconditionally
  (`platform/src/wm8994.rs:642–644`). This chapter assumes that
  fix is in place.
- This chapter does NOT cover R0x731 bit 5's behavior under
  spurious or noisy F_REF conditions. The retry loop's bound is a
  best-effort recovery; ill-conditioned input clocks may exhaust
  retries.

## §12 Acceptance checklist

A conforming `Wm8994::init_record` implementation MUST:

- [ ] (a) Poll R0x732 bit 5 (FLL1_LOCK_STS, level status) with the per-attempt timeout from §9 INV-AUDIO-01-2 after writing `FLL1_ENA = 1`.
- [ ] (b) Re-arm FLL1 on per-attempt timeout up to the retry budget from §9 INV-AUDIO-01-2.
- [ ] (c) Issue NO writes to R0x4 (AIF1ADC1L/R_ENA), R0x300, R0x302, R0x304, R0x305, R0x210 (or any other post-FLL AIF1 register) until either `FllLockOutcome::LockedFirstTry` or `LockedAfterRetry` is reached.
- [ ] (d) Return `Result<FllLockOutcome, I2C::Error>` per §9 INV-AUDIO-01-3.
- [ ] (e) On `FllLockOutcome::Failed`, NOT proceed with the remainder of `init_record` — return early.

Optional (RECOMMENDED but not required for conformance):

- [ ] (f) Log `LockedAfterRetry { attempts }` via the platform's `defmt`/`log`/USART telemetry path to support cold-boot-rate observability.

## §13 Files cited

- `platform/src/wm8994.rs` (lines per §2 and §10)
- `WM8994_Rev4.6.pdf` (pages per §3 and §9)
- [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-001 (register-map corrections)
- [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-002 (FLL1 lock variability and LRCLK rate fix)
- Downstream consumer: `softoboros.com:streamz/submodules/disco-analyzer/docs/AUDIO-DATA-PATH-RECON.md`
  (the full-pipeline recon doc; not a normative authority for this chapter, cited for context).

## §14 Unblocks

Once this chapter is ratified and AUDIO-01a (the implementation
commit) lands:

- Disco-analyzer's L/R digital-capture asymmetry symptoms (chronic
  across bench sessions 9n through 2026-05-19) are eliminated at the
  root.
- Future audio-path consumers receive a deterministic
  `init_record` boot regardless of cold-vs-warm reset.
- The operator workaround "press the reset button after USB plug-in"
  becomes unnecessary.
- Future AUDIO-NN chapters (SAI framing, DMA double-buffer,
  multi-codec abstraction) can assume codec FLL1 lock as a
  precondition rather than a fragility.

## §15 Change log

- **2026-05-19 — Initial ratification.** Drafted in response to the
  disco-analyzer 2026-05-19 bench session that observed the 11+5
  bit-split symptom variant and unified it with the prior
  `0x0000/0x8000` bimodal under one root-cause arc. Frozen
  invariants INV-AUDIO-01-1, INV-AUDIO-01-2, INV-AUDIO-01-3 and the
  `FllLockOutcome` enum. Durable errata entry:
  [`ERRATA.md`](ERRATA.md) AUDIO-ERRATA-002.
- **2026-05-19-c — AIF1ADC_RATE / AIF1DAC_RATE field corrected from
  64 to 32 in `init_record` (R0x304 / R0x305 low 11 bits).** The
  driver previously hardcoded `AIF1_LRCLK_SLAVE_ENA = 0x0840` (LRCLK_DIR
  bit 11 + RATE = 64) with a comment claiming this was the "codec reset
  default" appropriate for a "32-BCLK-per-stereo-frame layout padded
  internally to 32-bit slots." Bench measurement on STM32H747I-DISCO +
  disco-analyzer 2026-05-19 refuted the claim. With the SAI master
  configured for the standard 32-BCLK frame with two 16-bit slots
  (BCLK1 / LRCLK1 = 32 exactly), setting `AIF1ADC_RATE` to 64 causes
  the codec's AIF1ADC1L serializer state machine to fail asymmetrically:
  only emits valid data during the last 5 BCLKs of what it internally
  believes is the L MSB window, holds AIF1ADCDAT at logic 0 (per the
  TDM=0 datasheet-documented behavior) for the remaining 11 BCLKs of
  slot 0. The R serializer is silicon-level more tolerant of the
  mismatch and emits cleanly across all 16 BCLKs of slot 1. Net
  symptom: L channel reads as `0x0000 / 0x001f` 2-value bimodal at
  SAI1 RX (5 LSBs varying), R channel reads as clean 16-bit audio
  (~77 distinct values per 256-sample snapshot, ~half-FS amplitude on
  the test source). The asymmetric serializer behavior is undocumented
  in the WM8994 datasheet; bench is the only available authority.
  Fix: hardcoded value changed to `AIF1_LRCLK_SLAVE_ENA = 0x0820`
  (LRCLK_DIR bit 11 + RATE = 32 = actual BCLK1/LRCLK1 ratio). Bench
  evidence post-fix: L channel recovered to 266 distinct values per
  512 samples, ±99.2% FS swing, **L/R correlation +0.92** confirming
  both channels carry the same stereo source, **L/R peak-peak ratio
  1.006** (within 0.6%). Verified using the boot-time codec-write
  mailbox in disco-analyzer's analyzer-cm7/src/main.rs (issuing
  `write_reg(0x0304, 0x0820)` + `write_reg(0x0305, 0x0820)` at runtime
  against the existing rlvgl-platform driver, before the render loop
  starts). Lessons captured in `[[project_rlvgl_audio_01_codec_bringup]]`:
  comments claiming "codec reset default = correct" without bench
  validation are a chronic failure mode, and AIF1ADC_RATE is NOT
  informational — it gates serializer behavior asymmetrically. Caveat:
  the new value of 0x0820 is correct for the 32-BCLK frame SAI
  framing assumed throughout AUDIO-01. A future caller running TDM-4
  at 64-BCLK frame (or any other ratio) MUST update this field to
  match its actual BCLK1/LRCLK1; future AUDIO-NN chapters may
  parameterize `init_record`'s framing assumption to make this
  enforceable at the type level. Implementation lands in this same
  rlvgl commit as AUDIO-01-c.
- **2026-05-19-b — INV-AUDIO-01-1 corrected from edge latch to
  level status.** Initial ratification cited R0x731 bit 5
  `FLL1_LOCK_EINT` (edge-latched interrupt event, write-1-to-clear,
  fires on both lock-acquired AND lock-lost edges per `WM8994
  p.297`) as the lock-detection primitive, with a clear-then-poll
  sequence at the head of each attempt. Bench validation against
  AUDIO-01a on STM32H747I-DISCO + disco-analyzer firmware found
  that, with the clear-write executing immediately before
  `FLL1_ENA = 1`, the codec hardware never latched the
  lock-acquired transition: post-init readback of R0x731 returned
  `0x0000` (bit 5 unset) while R0x732 bit 5 (FLL1_LOCK_STS, level
  status) returned `1` confirming FLL1 *was* in fact locked. The
  driver thus reported `FllLockOutcome::Failed` and skipped the
  AIF1 framing + ADC enable writes; the rest of main.rs's post-init
  workarounds ran on a partially-configured codec and SAI1 RX read
  all-zero. Root cause hypothesis: the clear-write at attempt-0 was
  racing with the FLL1-lock transition latch in a way the WM8994
  hardware decided to lose. Level-status polling (R0x732 bit 5) has
  no transition-timing window — a read of bit 5 = 1 simply reports
  current state. Glossary in §3 now cites R0x732 bit 5 as the
  canonical FLL1-lock observation, §9 INV-AUDIO-01-1 simplifies to
  poll-until-set (no clear-write), §12 acceptance gates renumbered
  (a)–(e) (was (a)–(f)). The `FllLockOutcome` enum (§6) and retry
  budget (§9 INV-AUDIO-01-2) are unchanged. Implementation lands in
  the same commit as this amendment via AUDIO-01a (corrected).
