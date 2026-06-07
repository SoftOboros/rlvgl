# Agent Task: WM8994 Register-Map Corrections (`platform/src/wm8994.rs`)

**Branch:** `v0.2.0`
**Filed:** 2026-04-30 by disco-analyzer bench-9h session
**Authoritative source:** `WM8994_Rev4.6.pdf` (queried via memalpha; page citations below)

## Why this task exists

A downstream consumer (the disco-analyzer subrepo, `softoboros.com:streamz/submodules/disco-analyzer/`) hit a real bug while debugging Mac → CN11 line-in → SAI1 RX → MCU → SAI1 TX → AIF1DAC → CN11 round-trip on STM32H747I-DISCO. The codec's `AIF1ADCDAT` pin outputs continuous zeros even though all the obvious enables are set and internal sidetone (LINEIN→ADC→DAC→HP) is audible. While bisecting the codec config, every register address rlvgl-platform's `wm8994.rs` cited was cross-checked against the WM8994 Rev 4.6 datasheet via memalpha. **Several addresses in `wm8994.rs` are wrong.** They happen to not break rlvgl's narrow `init_playback` path because the wrong addresses land on benign defaults — but disco-analyzer's `init_record`-style code (which writes more registers + reads them back) trips over them.

This task is to correct the register addresses in `platform/src/wm8994.rs` so both rlvgl-platform itself and downstream consumers (disco-analyzer, future record-path users) get correct behaviour.

## Errors to fix

All citations are from `WM8994_Rev4.6.pdf` (private corpus; queryable via `mcp__softoboros__memalpha_ask`). Re-verify by querying memalpha if you want a second opinion before committing.

### 1. `REG_AIF1_RATE` is **0x210**, not 0x211

```rust
// WRONG (current platform/src/wm8994.rs:29):
const REG_AIF1_RATE: u16 = 0x0211;

// CORRECT:
const REG_AIF1_RATE: u16 = 0x0210;
```

Per p.194/285-286: `R0x210 (AIF1 Rate)` has bits 7:4 = `AIF1_SR` (sample rate, `1000` = 48 kHz) and bits 3:0 = `AIF1CLK_RATE` (`0011` = 256·Fs). `R0x211` is `AIF2 Rate` (p.286).

The `init_playback` write at line 188 currently lands on AIF2 Rate — silently writing AIF2 sample rate when the caller intended AIF1.

### 2. `REG_CLOCKING_2` is **0x209**, not 0x210

```rust
// WRONG (current platform/src/wm8994.rs:28):
const REG_CLOCKING_2: u16 = 0x0210;

// CORRECT:
const REG_CLOCKING_2: u16 = 0x0209;
```

Per p.285: `R0x209 (Clocking (2))` has TOCLK_DIV[2:0] (bits 10:8), DBCLK_DIV[2:0] (bits 6:4), OPCLK_DIV[2:0] (bits 2:0). `R0x210` is `AIF1 Rate` (see #1).

The `init_playback` write of `0x0003` to `REG_CLOCKING_2` at line 200 currently lands on `R0x210` (AIF1 Rate), which OVERWRITES the AIF1 sample-rate write made at line 188 (R0x211, even more wrong) — net result: `R0x210 = 0x0003` (SR=0=8 kHz, CLK_RATE=3=256·Fs) and `R0x211 = 0x61` (AIF2 SR=6=32 kHz, CLK_RATE=1=128·Fs). The chip ends up with AIF1 nominally at 8 kHz Fs at 256·Fs, locked to FLL1 at 12.5 MHz, which somehow audibly works for `init_playback` because the `12.5 MHz / 256` ≈ 48.83 kHz mismatch falls within the codec's tolerance for DAC playback. Don't rely on this — record-path users won't be so lucky.

### 3. Bit position references in comments

After fixing #1 and #2, audit comments in `init_playback` and helpers that cite bit positions in the wrong registers. Particularly:

- The comment at `init_playback` step 7 says `// SYSCLK_SRC = AIF1CLK, AIF1CLK / 1` for `R0x208 = 0x000A`. Per p.284: `R0x208 Clocking (1)` has bit 0 = `SYSCLK_SRC` (1 = AIF1CLK), bit 1 = `SYSDSPCLK_ENA`, bit 3 = `AIF1DSPCLK_ENA`, bit 4 = `TOCLK_ENA`. `0x000A` = bits 1+3 set, **bit 0 cleared** (SYSCLK_SRC=0 = MCLK1, NOT AIF1CLK). The current `init_playback` either intends bit 0 set (and the value should be `0x000B`) or the comment is wrong. Verify and fix.

### 4. Verify `R0x301` writes (DAC source-select bits)

`R0x301` (`AIF1 Control (2)`, p.178/293) has:
- bit 14 `AIF1DACR_SRC` (default = 1)
- bit 15 `AIF1DACL_SRC` (default = 0)
- bits 11:10 `AIF1DAC_BOOST`

If `init_playback` (or any future `init_record`) writes to R0x301, **don't clear bit 14** unless you intend to swap right-DAC source. Default-preserve writes should use `0x4000`, not `0x0000`.

### 5. Verify `AIF1_TRI` location

`AIF1_TRI` is **R0x302 bit 15** (p.292/173), NOT R0x301. The existing `REG_AIF1_MASTER_SLAVE = 0x0302` is correct; just make sure no comment mis-attributes the bit to a different register.

## Verification — pre-publish

After fixing the constants, run the standard rlvgl pre-publish gates from this repo's `CLAUDE.md` (Phase 0–6). The `cargo check` for `--features cm7,...,audio` against `rlvgl-example-disco` must still build clean. The `init_playback`-driven 48 kHz playback path will now actually run AIF1 at 48 kHz; if the bench audio was previously fine at the wrong rate, it will still be fine — just for the right reason now.

## Workflow

1. Edit `platform/src/wm8994.rs`. Update the four constants (REG_AIF1_RATE, REG_CLOCKING_2, plus any related bit-position comments). Cite memalpha page numbers in the new comments where you make non-obvious choices.
2. Rebuild: `RUSTFLAGS="-C target-cpu=cortex-m7" cargo build --target thumbv7em-none-eabihf -p rlvgl-example-disco --bin rlvgl-stm32h747i-disco --features cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio` — must succeed.
3. Run pre-publish (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`).
4. Commit on `v0.2.0` with subject `DISCO-XX: wm8994 register-map fix per WM8994_Rev4.6 (memalpha)` (use the next free `DISCO-NN` number per `docs/disco-platform-guide/`).
5. `git push origin v0.2.0`.
6. In the parent `softoboros.com` repo: bump the rlvgl submodule pointer to the new SHA (`git -C ops/packer/submodules/rlvgl rev-parse HEAD`, then `git add ops/packer/submodules/rlvgl` + commit in parent).

## Cross-references

- **disco-analyzer's mirror of the corrected addresses:** `streamz/submodules/disco-analyzer/analyzer-audio/src/wm8994_record.rs` already uses `REG_AIF1_RATE = 0x0210` and treats `R0x208` per the memalpha-confirmed layout — that file is correct; only `rlvgl-platform/src/wm8994.rs` needs to change.
- **Memalpha-verified register identities (full list from bench-9h, 2026-04-30):**
  - R0x208 Clocking (1) — bit 0 SYSCLK_SRC, bit 1 SYSDSPCLK_ENA, bit 2 AIF2DSPCLK_ENA, bit 3 AIF1DSPCLK_ENA, bit 4 TOCLK_ENA (p.284).
  - R0x209 Clocking (2) — TOCLK_DIV / DBCLK_DIV / OPCLK_DIV (p.285).
  - R0x210 AIF1 Rate — bits 7:4 AIF1_SR (1000=48 kHz), bits 3:0 AIF1CLK_RATE (0011=256·Fs) (p.194/285-286).
  - R0x211 AIF2 Rate (p.286).
  - R0x300 AIF1 Control (1) — bits 4:3 AIF1_FMT, bits 6:5 AIF1_WL, bit 7 AIF1_LRCLK_INV, bit 8 AIF1_BCLK_INV, bit 13 AIF1ADC_TDM, bit 14 AIF1ADCR_SRC, bit 15 AIF1ADCL_SRC (p.171/176-177).
  - R0x301 AIF1 Control (2) — bits 1:0 unused for AIF1 (no AIF1_LOOPBACK; only AIF2 has loopback at R0x311 bit 0), bits 11:10 AIF1DAC_BOOST, bit 14 AIF1DACR_SRC default 1, bit 15 AIF1DACL_SRC default 0 (p.178/293).
  - R0x302 AIF1 Master/Slave — bit 13 AIF1_CLK_FRC, bit 14 AIF1_MSTR (default slave), bit 15 AIF1_TRI (p.292/173).
  - R0x600 DAC1 Mixer Volumes — bits 8:5 ADCR_DAC1_VOL (sidetone STR gain), bits 3:0 ADCL_DAC1_VOL (sidetone STL gain). NOT a routing register (p.326-327).
  - R0x601 DAC1 Left Mixer Routing — bit 0 AIF1DAC1L_TO_DAC1L, bit 4 ADCL_TO_DAC1L sidetone, bit 5 ADCR_TO_DAC1L sidetone (p.327).
  - R0x603 DAC1 Right Mixer Routing — symmetric to R0x601 with bit 0 = AIF1DAC1R, bit 5 = ADCR sidetone.
  - R0x606 AIF1 ADC1 Left Mixer Routing — bit 0 AIF2DACL_TO_AIF1ADC1L, bit 1 ADC1L_TO_AIF1ADC1L (p.329).
  - R0x607 AIF1 ADC1 Right Mixer Routing — bit 0 AIF2DACR_TO_AIF1ADC1R, bit 1 ADC1R_TO_AIF1ADC1R (p.329).

## Out of scope for this task

- The `AIF1ADCDAT silent despite all enables correct` issue that prompted this audit is **bench-level** on the H747I-DISCO + MB1166 daughterboard, not register-side. Probably a board-level isolation or a missing pin we haven't identified. **Do not** chase it from this task; the rlvgl fix here is purely the register-map correction. The disco-analyzer subrepo has the open hardware investigation in its own task tracker.

## Quick memalpha re-verify queries (optional)

If you want to double-check before committing, run these in this order — answers should match the citations above:

1. `WM8994 R0x210 register name and bit layout. Cite page.`
2. `WM8994 R0x211 register name. Is it AIF1 Rate or AIF2 Rate?`
3. `WM8994 R0x209 register name and bit layout.`
4. `WM8994 R0x208 bit 0 — is it SYSCLK_SRC?`

If any answer disagrees with the citations above, defer to memalpha (it's reading the actual datasheet PDF) and update both this doc and the code accordingly.
