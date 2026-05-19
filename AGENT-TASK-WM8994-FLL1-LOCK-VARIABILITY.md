# Agent Task: WM8994 FLL1 Lock-Variability Fix (`platform/src/wm8994.rs`)

**Branch:** `v0.2.0`
**Filed:** 2026-05-19 by disco-analyzer bench session investigating chronic L/R capture asymmetry
**Updated:** 2026-05-19 (bench-driven correction — see AUDIO-01 §15 amendment 2026-05-19-b: lock detection polls **R0x732 bit 5 FLL1_LOCK_STS** level status, not R0x731 bit 5 FLL1_LOCK_EINT edge latch).
**Authoritative source:** `WM8994_Rev4.6.pdf` (queried via memalpha) + [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md) AUDIO-01

## Why this task exists

A downstream consumer (the disco-analyzer subrepo, `softoboros.com:streamz/submodules/disco-analyzer/`) has been chasing a chronic codec-digital-path bug across half a dozen bench sessions. The symptom inverts L↔R across power cycles and presents as either (a) `0x0000 / 0x8000` bimodal toggle on one slot or (b) an 11+5 bit-split between the two slots that looks like a ~57 dB L/R level asymmetry on a stereo 1 kHz sine source. The 2026-05-19 bench session (anchored at disco-analyzer SHA `836cb07`) joined these two symptoms under a single causal arc:

```
FLL1 not yet locked when post-FLL AIF1 register writes land
  → AIF1ADC1 serializer arms against an unstable SYSCLK
  → bit-clock phase relative to LRCLK1 latches in a wrong position
  → which slot "loses" depends on the offset N:
        N=15 → 0x0000/0x8000 bimodal
        N=11 → 11+5 bit-split
        N=0  → clean stereo (what we want every boot)
```

The 20 ms static `delay_busy` after `FLL1_ENA = 1` (`platform/src/wm8994.rs:611`) plus the additional 100 ms in disco-analyzer's CM7 post-init (`analyzer-cm7/src/main.rs:~2273`) cover the *average* FLL1 lock time but not the *worst-case* 30–50 ms documented in WM8994 p.202. Cold boot from USB plug-in mis-locks frequently; warm reset (NRST button, probe-rs `--core 1 reset`, `SCB::sys_reset()`) appears reliable because BCLK1 is already running on the wire from the prior session.

The root-cause arc is ratified in [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md) AUDIO-01 §2; the fix is normatively specified in §9 INV-AUDIO-01-1.

This task is the **implementation tracker** for landing AUDIO-01 in `platform/src/wm8994.rs`. The chapter is the normative artifact; this file is the workflow notes.

## What changes

### 1. Add poll-and-retry helper

New private method on `Wm8994` between `configure_fll1` and `write_reg`:

```rust
/// Enable FLL1 and wait for lock, retrying up to N times on timeout.
///
/// Implements [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md)
/// AUDIO-01 §9 INV-AUDIO-01-1 (clear-then-poll FLL1_LOCK_EINT) and
/// INV-AUDIO-01-2 (100 ms per-attempt timeout, 3 max retries, 1 ms
/// poll interval).
///
/// Sequence per attempt:
///   1. Write 1 to R0x731 bit 5 (clear the FLL1_LOCK_EINT latch).
///   2. Write FLL1_ENA = 1 to R0x220 (re-write FLL config first on retry).
///   3. Poll R0x731 bit 5 every 1 ms until set or timeout.
///   4. On timeout: write FLL1_ENA = 0, wait 5 ms, retry.
///
/// Returns:
///   - `Ok(LockedFirstTry)` if lock reported within attempt 0's timeout.
///   - `Ok(LockedAfterRetry { attempts: N })` if lock acquired on attempt N (N ≥ 1).
///   - `Ok(Failed)` if MAX_RETRIES exhausted. Caller MUST treat as error.
///   - `Err(I2C::Error)` if any I²C transaction fails.
fn fll1_enable_and_wait_lock(&mut self) -> Result<FllLockOutcome, I2C::Error> {
    const PER_ATTEMPT_TIMEOUT_MS: u32 = 100;
    const POLL_INTERVAL_MS: u32 = 1;
    const RE_ARM_WAIT_MS: u32 = 5;
    const MAX_RETRIES: u8 = 3;

    for attempt in 0..=MAX_RETRIES {
        // Step 1: clear FLL1_LOCK_EINT latch (write 1 to R0x731 bit 5).
        // Per WM8994 p.297: this is a write-1-to-clear latching event bit
        // triggered on both lock-acquired and lock-lost edges. Clearing
        // ensures the next observed bit-5-set corresponds to the current
        // FLL1_ENA cycle.
        self.write_reg(REG_FLL1_LOCK_EINT, 1 << 5)?;

        // Step 2: on retry, re-write FLL config + FLL1_ENA. On attempt 0
        // the caller has already programmed the config registers.
        if attempt > 0 {
            self.write_reg(REG_FLL1_CTRL_1, 0x0000)?; // FLL1_ENA = 0
            delay_busy(RE_ARM_WAIT_MS * 1_000);
            self.write_reg(REG_FLL1_CTRL_5, 0x0003)?;
            self.write_reg(REG_FLL1_CTRL_4, 0x0800)?;
            self.write_reg(REG_FLL1_CTRL_3, 0x0000)?;
            self.write_reg(REG_FLL1_CTRL_2, 7u16 << 8)?;
        }
        self.write_reg(REG_FLL1_CTRL_1, 0x0001)?; // FLL1_ENA = 1

        // Step 3: poll bit 5 with timeout.
        for _ in 0..(PER_ATTEMPT_TIMEOUT_MS / POLL_INTERVAL_MS) {
            delay_busy(POLL_INTERVAL_MS * 1_000);
            let status = self.read_reg(REG_FLL1_LOCK_EINT)?;
            if status & (1 << 5) != 0 {
                return Ok(if attempt == 0 {
                    FllLockOutcome::LockedFirstTry
                } else {
                    FllLockOutcome::LockedAfterRetry { attempts: attempt }
                });
            }
        }
        // Step 4: timeout; loop will re-arm at the top.
    }
    Ok(FllLockOutcome::Failed)
}
```

### 2. Add the new register constant

```rust
// R0x731 Interrupt Status 2 — bit 5 = FLL1_LOCK_EINT (latching,
// write-1-to-clear, triggered on both lock-acquired AND lock-lost
// edges per WM8994 p.297). AUDIO-01 §9 INV-AUDIO-01-1 uses this as
// the lock-detection primitive.
const REG_FLL1_LOCK_EINT: u16 = 0x0731;
```

Place adjacent to the FLL1 control register block (after `REG_FLL1_CTRL_5`).

### 3. Add the `FllLockOutcome` enum

Public on `Wm8994` (or on the module). Three variants per AUDIO-01 §6:

```rust
/// Result of FLL1 lock-and-retry from [`Wm8994::init_record`].
/// Per AUDIO-01 §6 (frozen enum, registration policy: Standards Action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FllLockOutcome {
    /// FLL1 reported lock within the first poll interval. Healthy boot.
    LockedFirstTry,
    /// FLL1 acquired lock only after one or more re-arm cycles. Callers
    /// SHOULD log for telemetry.
    LockedAfterRetry { attempts: u8 },
    /// FLL1 never acquired lock within the configured retry budget.
    /// Callers MUST NOT proceed with the remainder of `init_record`.
    Failed,
}
```

### 4. Modify `init_record` to use the helper

Replace the existing FLL1 config block at `platform/src/wm8994.rs:606–611`:

```rust
// Before (current):
self.write_reg(REG_FLL1_CTRL_5, 0x0003)?; // REFCLK_SRC=BCLK1, REFCLK_DIV=÷1
self.write_reg(REG_FLL1_CTRL_4, 0x0800)?; // N=64
self.write_reg(REG_FLL1_CTRL_3, 0x0000)?; // K=0
self.write_reg(REG_FLL1_CTRL_2, 7u16 << 8)?; // OUTDIV-1=7, FRATIO=0
self.write_reg(REG_FLL1_CTRL_1, 0x0001)?; // FLL1_ENA=1 (set LAST per p.202)
delay_busy(20_000); // FLL settle ~20 ms

// After:
self.write_reg(REG_FLL1_CTRL_5, 0x0003)?; // REFCLK_SRC=BCLK1, REFCLK_DIV=÷1
self.write_reg(REG_FLL1_CTRL_4, 0x0800)?; // N=64
self.write_reg(REG_FLL1_CTRL_3, 0x0000)?; // K=0
self.write_reg(REG_FLL1_CTRL_2, 7u16 << 8)?; // OUTDIV-1=7, FRATIO=0
// AUDIO-01 §9 INV-AUDIO-01-1: poll-and-retry lock instead of static
// 20 ms delay. Per AUDIO-01 §9 INV-AUDIO-01-3 the return value
// propagates the outcome to the caller for telemetry.
let lock_outcome = self.fll1_enable_and_wait_lock()?;
if matches!(lock_outcome, FllLockOutcome::Failed) {
    // Codec is in undefined state; do NOT proceed with serializer arm
    // (R0x4 AIF1ADC1L/R_ENA, R0x300 framing, R0x304/R0x305 LRCLK_DIR).
    // Return early — the I²C path itself didn't fail, but the codec
    // didn't acquire FLL1 lock. Caller decides whether to retry or
    // panic.
    return Ok(lock_outcome);
}
```

### 5. Modify `init_record` return type

`Result<(), I2C::Error>` → `Result<FllLockOutcome, I2C::Error>` per
AUDIO-01 §9 INV-AUDIO-01-3.

### 6. Migrate in-repo callers

Search for `init_record(` call sites:

```bash
grep -rn "init_record(" --include="*.rs"
```

Expected hits (verify before editing):
- `platform/src/audio_player.rs`
- `examples/stm32h747i-disco/src/audio_scope.rs` (if present)
- any other in-repo example.

For each: change call to either:
- `let _ = wm8994.init_record(...)?;` (preserve old behaviour; discard outcome).
- `match wm8994.init_record(...)? { FllLockOutcome::Failed => ..., _ => ... }` (act on outcome).

Default is the discard form unless the call site already does its own
post-init verification.

### 7. Update the `init_record` doc-comment

Mention the new return semantics + cite AUDIO-01 §9 in the rustdoc.

## Verification — pre-publish

Standard rlvgl pre-publish gates per CLAUDE.md "Pre-Publish Validation":

```bash
# Phase 0: format
cargo fmt --all -- --check

# Phase 1: clippy
RUSTFLAGS="" cargo clippy --workspace -- -D warnings

# Phase 2: tests (host)
RUSTFLAGS="" cargo test --workspace

# Phase 2.5: HAL discipline
RLVGL_LINT_STRICT=1 RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline
RUSTFLAGS="" cargo test -p rlvgl-platform --test discipline_compile

# Phase 4.6 not needed (this change is platform-side, not creator).

# Embedded cross-compile sanity:
RUSTFLAGS="-C target-cpu=cortex-m7" \
  cargo build --target thumbv7em-none-eabihf \
  -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco \
  --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio
```

## Verification — bench

After landing this rlvgl change and bumping the disco-analyzer
submodule pointer in the parent `softoboros` repo:

1. Flash disco-analyzer CM7 (which transitively uses
   `Wm8994::init_record` through `rlvgl-platform`).
2. Power-cycle (USB unplug → plug) — the worst-case cold-boot path.
3. Read disco-analyzer's SAI1 RX BUF0 (`probe-rs read --chip STM32H747XIHx
   b16 0x30000000 512 > /tmp/sai.txt`) and compute L vs R distribution
   via the existing `/tmp/sai_stats.py` helper.
4. Expected: L and R both show full-range stereo audio of the source signal.
   No bit-split, no bimodal-at-rail.
5. Repeat 5 cold-boot cycles. Expected: 5/5 clean stereo. Prior state
   was ~3/5 clean (FLL lock variability).

Capture the `FllLockOutcome` value disco-analyzer's wrapper logs across
the 5 cycles. Distribution `LockedFirstTry: 5, LockedAfterRetry: 0,
Failed: 0` is the success ratio target; `LockedAfterRetry` events on
cold boot are acceptable evidence the retry path is doing its job.

## Workflow

1. **Read** `docs/audio/01-codec-bringup.md` and confirm the invariants
   in §9 are still the ones you want to implement before touching code.
   The chapter is the canonical authority.
2. **Edit** `platform/src/wm8994.rs` per §1–§5 above. Cite memalpha
   page numbers in any new comments where you make non-obvious choices.
3. **Migrate** in-repo callers per §6.
4. **Rebuild + test** per "Verification — pre-publish" above. All
   phases must pass.
5. **Commit** with subject `AUDIO-01a: wm8994 init_record FLL1 lock poll-and-retry per AUDIO-01 §9`.
   Body cites this AGENT-TASK doc + the AUDIO-01 chapter.
6. `git push origin v0.2.0`.
7. **Bench-validate** per "Verification — bench" once the downstream
   consumer (disco-analyzer) has bumped its `rlvgl-platform` dependency.
   Expected disco-analyzer subrepo change: bump `Cargo.lock`'s
   `rlvgl-platform` revision to the new SHA + remove the now-redundant
   100 ms post-init delay in `analyzer-cm7/src/main.rs:~2273`.
8. In the parent `softoboros.com` repo: bump the rlvgl submodule
   pointer to the new SHA (`git -C ops/packer/submodules/rlvgl
   rev-parse HEAD`, then `git add ops/packer/submodules/rlvgl` +
   commit in parent).

## Cross-references

- **AUDIO-01 chapter (normative):** [`docs/audio/01-codec-bringup.md`](docs/audio/01-codec-bringup.md).
- **WM8994 driver:** [`platform/src/wm8994.rs`](platform/src/wm8994.rs).
- **Prior register-map fix:** [`AGENT-TASK-WM8994-REGISTER-MAP-FIX.md`](AGENT-TASK-WM8994-REGISTER-MAP-FIX.md) (resolved 2026-04-30).
- **Downstream-side recon doc:** `softoboros.com:streamz/submodules/disco-analyzer/docs/AUDIO-DATA-PATH-RECON.md` "Smoking gun" section names this exact arc.
- **Downstream-side memory note:** `softoboros` memory `project_daa_serializer_phase_unified` (2026-05-19) captures the symptom-vs-N mapping in the form the next bench session will encounter it.

## Out of scope for this task

- SAI Block A / Block B reconfiguration discipline ("never re-touch
  SAI1_A's clock generation after the codec has armed AIF1ADC1") —
  that's a separate invariant for a future AUDIO-NN chapter.
- The chronic AIF1ADCDAT-silent issue from `AGENT-TASK-WM8994-REGISTER-MAP-FIX.md` —
  resolved 2026-04-30 by setting R0x304/R0x305 bit 11 (LRCLK_DIR
  slave-mode enable). This task assumes that fix is in place
  (`platform/src/wm8994.rs:642–644`).
- The chronic Cm7Loop "bees" issue (downstream-only; tracked in
  disco-analyzer memory `project_daa_cm7_loopback_bees`).
- Hardware-level investigations on the H747I-DISCO board (cable, jack,
  inner-layer traces). Bench-10i sidetone test proved the analog path
  is clean; this task addresses only the digital-side ordering bug.

## Quick memalpha re-verify queries (optional)

If you want to double-check before committing, run these in this order
— answers should match the implementation:

1. `WM8994 R0x731 bit 5 — name and semantics. Is it FLL1_LOCK_EINT? Is it latching (write-1-to-clear)? Triggered on which edges?`
2. `WM8994 R0x220 bit 0 — is it FLL1_ENA? What value enables FLL1?`
3. `WM8994 FLL1 worst-case lock time when F_REF = BCLK1 = 1.5–1.6 MHz. Cite page.`

If any answer disagrees with the implementation, defer to memalpha
and update both this doc and the code accordingly.
