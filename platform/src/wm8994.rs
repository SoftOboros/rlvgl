//! Driver for the WM8994 audio codec on the STM32H747I-DISCO board.
//!
//! Communicates over I2C using 16-bit register addresses and 16-bit data
//! values. The codec is controlled via AIF1 (Audio Interface 1) connected to
//! SAI1 on the Discovery board. Both playback and record paths are supported:
//!
//! - [`Wm8994::init_playback`] — DAC → MIXOUT → HPOUT1 / SPKOUT (headphone or
//!   speaker), I²S slave, 16-bit, FLL1 locked to MCLK1 at 256·Fs.
//! - [`Wm8994::init_record`] — LINEIN1/2 → IN1/2-PGA → MIXIN → ADC → AIF1ADC
//!   (codec → MCU), I²S slave, 16-bit, FLL1 locked to BCLK1 at 256·Fs. The
//!   record path locks the FLL to BCLK because most carrier boards (including
//!   the H747I-DISCO via CN15 to MB1166) do not route MCLK1 to the codec —
//!   only BCLK/LRCLK/data make it across the LCD-module connector.
//!
//! [`Wm8994::init_hp_output_stage`] / [`Wm8994::finish_hp_output_stage`]
//! implement the headphone Cold Start-Up Sequence from WM8994_Rev4.6.pdf
//! Table 123. The caller MUST insert a ≥256.5 ms DC-servo settle wait between
//! them; the helpers don't busy-loop internally because the wait is on the
//! caller's clock domain.
//!
//! [`Wm8994::readback_critical_regs`] and [`Wm8994::readback_output_regs`]
//! pack the codec's record-path / output-path state into a `(reg << 16) | val`
//! layout suitable for SRAM4 telemetry slots, so debug probes can decode the
//! codec configuration without re-reading via I²C.
//!
//! ## Register-map provenance
//!
//! All register addresses + bit assignments below are quoted against
//! WM8994_Rev4.6.pdf via memalpha. The DISCO-XX (2026-04-30) commit fixed
//! three earlier address errors that were masked by `init_playback`'s narrow
//! 48 kHz HP path; the fix surfaced when disco-analyzer's record path
//! exercised more of the AIF1 register surface. See `init_record` body
//! comments for the load-bearing bench-iteration history that informed each
//! register write.

use embedded_hal::i2c::{I2c, SevenBitAddress};

// ---------------------------------------------------------------------------
// WM8994 register addresses (16-bit)
// ---------------------------------------------------------------------------

// Chip identification
const REG_CHIP_ID: u16 = 0x0000;

// Power management
const REG_PWR_MGMT_1: u16 = 0x0001;
const REG_PWR_MGMT_2: u16 = 0x0002;
const REG_PWR_MGMT_3: u16 = 0x0003;
const REG_PWR_MGMT_4: u16 = 0x0004;
const REG_PWR_MGMT_5: u16 = 0x0005;
const REG_PWR_MGMT_6: u16 = 0x0006;

// Input PGA volumes (record path)
const REG_LEFT_LINE_IN1_2_VOL: u16 = 0x0018;
const REG_RIGHT_LINE_IN1_2_VOL: u16 = 0x001A;

// Headphone output volume
const REG_LEFT_OUTPUT_VOL: u16 = 0x001C;
const REG_RIGHT_OUTPUT_VOL: u16 = 0x001D;
const REG_LINEOUT_VOL: u16 = 0x001E;

// Speaker
const REG_SPKMIXL_ATT: u16 = 0x0022;
const REG_SPKMIXR_ATT: u16 = 0x0023;
const REG_SPKOUT_MIXERS: u16 = 0x0024;
/// `R0x25` Class W (1) — speaker Class W envelope tracker. Used by
/// `init_playback` (HEADPHONE branch writes 0x0005 here historically). Note
/// that DAA's record-path bench (2026-04-30) writes 0x0005 to `R0x51` instead
/// for the HP envelope tracker — both addresses are documented in revisions
/// of the WM8994 datasheet as "Class W"-named registers; the load-bearing
/// observation was that 0x51 is what the HP output stage cold-start sequence
/// actually needs. `init_record` writes 0x51 (see [`REG_CLASSW_HP`]).
const REG_CLASS_W: u16 = 0x0025;
const REG_SPEAKER_VOL_LEFT: u16 = 0x0026;
const REG_SPEAKER_VOL_RIGHT: u16 = 0x0027;

// Input mixer routing (record path)
const REG_INPUT_MIXER_2: u16 = 0x0028;
const REG_INPUT_MIXER_3: u16 = 0x0029;
const REG_INPUT_MIXER_4: u16 = 0x002A;

// Output mixer
const REG_OUTPUT_MIXER_1: u16 = 0x002D;
const REG_OUTPUT_MIXER_2: u16 = 0x002E;

// Line-output mixers (record-path output staging)
const REG_LINEOUT_MIXER_1: u16 = 0x0033;
const REG_LINEOUT_MIXER_2: u16 = 0x0034;

// Anti-pop
const REG_ANTIPOP_2: u16 = 0x0039;

// Charge pump
const REG_CHARGE_PUMP_1: u16 = 0x004C;

/// `R0x51` Class W (1) — headphone envelope tracker (HP-specific). Distinct
/// from [`REG_CLASS_W`] (R0x25, Class W speaker side). DAA bench 2026-04-30
/// confirmed 0x0005 must be written here for HPOUT1L/R to drive a load —
/// without it the HP output stage stays gated even with charge pump,
/// mixers, volumes, and mutes all configured correctly.
const REG_CLASSW_HP: u16 = 0x0051;

// HP cold-start sequence (per WM8994_Rev4.6.pdf Table 123)
const REG_DC_SERVO_1: u16 = 0x0054;
#[allow(dead_code)]
const REG_DC_SERVO_2: u16 = 0x0055;
const REG_ANALOGUE_HP1: u16 = 0x0060;

// Clocking
const REG_AIF1_CLOCKING_1: u16 = 0x0200;
const REG_AIF1_CLOCKING_2: u16 = 0x0201;
const REG_CLOCKING_1: u16 = 0x0208;
// DISCO-XX (2026-04-30) register-map fix per WM8994_Rev4.6.pdf
// (memalpha-verified). Earlier values were:
//   REG_CLOCKING_2: 0x0210  → wrong; that's AIF1 Rate
//   REG_AIF1_RATE: 0x0211  → wrong; that's AIF2 Rate
// Per p.285: R0x209 = "Clocking (2)" (TOCLK_DIV / DBCLK_DIV / OPCLK_DIV).
// Per p.194/285-286: R0x210 = "AIF1 Rate" (bits 7:4 AIF1_SR, bits 3:0
// AIF1CLK_RATE).
// Per p.286: R0x211 = "AIF2 Rate" (not AIF1).
// `init_playback`'s previous writes happened to land on benign defaults
// for its narrow 48 kHz playback path; downstream consumers exercising
// more of the AIF1 register surface (disco-analyzer's init_record) hit
// the discrepancy.
const REG_CLOCKING_2: u16 = 0x0209;
const REG_AIF1_RATE: u16 = 0x0210;

// FLL1 (Frequency Locked Loop)
const REG_FLL1_CTRL_1: u16 = 0x0220;
const REG_FLL1_CTRL_2: u16 = 0x0221;
const REG_FLL1_CTRL_3: u16 = 0x0222;
const REG_FLL1_CTRL_4: u16 = 0x0223;
const REG_FLL1_CTRL_5: u16 = 0x0224;

// R0x732 Interrupt Raw Status 2 — bit 5 = FLL1_LOCK_STS, the LEVEL
// status indicator (0 = not locked, 1 = locked) per WM8994_Rev4.6.pdf
// p.346, memalpha-verified 2026-05-19 (bench-driven correction during
// AUDIO-01a validation; see AUDIO-01 §15 amendment 2026-05-19-b).
//
// The initial AUDIO-01 ratification cited R0x731 bit 5 (FLL1_LOCK_EINT,
// the edge-latched event bit) as the lock-detection primitive. Bench
// validation revealed the edge latch never fires under our init_record
// sequence: the codec's software reset (R0x0 = 0) puts FLL1 in the
// disabled state, then init_record writes FLL config + FLL1_ENA = 1.
// The FLL1_ENA = 1 write triggers a "FLL1 starting up" transition that
// should latch in R0x731 bit 5 — but our clear-write at the start of
// each attempt was racing with the transition latch in a way the
// hardware decided to lose (post-bench observation: R0x731 reads 0
// post-init while R0x732 bit 5 reads 1, confirming FLL IS locked but
// the latch didn't capture the event). Level status (R0x732) is robust
// — no transition timing window, no clear-write race — and is what the
// driver now polls.
const REG_FLL1_LOCK_STS: u16 = 0x0732;
const FLL1_LOCK_STS_BIT: u16 = 1 << 5;

// AIF1 (Audio Interface 1) control
const REG_AIF1_CONTROL_1: u16 = 0x0300;
const REG_AIF1_CONTROL_2: u16 = 0x0301;
const REG_AIF1_MASTER_SLAVE: u16 = 0x0302;
// AIF1 ADC LRCLK control (bit 11 = AIF1ADC_LRCLK_DIR — slave-mode enable
// for the ADC LRCLK reception path; bits 10:0 = AIF1ADC_RATE).
const REG_AIF1_ADC_LRCLK: u16 = 0x0304;
// AIF1 DAC LRCLK control (bit 11 = AIF1DAC_LRCLK_DIR — slave-mode enable
// for the DAC LRCLK reception path; bits 10:0 = AIF1DAC_RATE).
const REG_AIF1_DAC_LRCLK: u16 = 0x0305;

// AIF1 ADC volume + filters (record path)
const REG_AIF1_ADC1_L_VOL: u16 = 0x0400;
const REG_AIF1_ADC1_R_VOL: u16 = 0x0401;
const REG_AIF1_ADC1_FILTERS: u16 = 0x0410;

// AIF1 DAC volume + filters (playback path)
const REG_AIF1_DAC1_FILTER_1: u16 = 0x0420;
const REG_AIF1_DAC1_LEFT_VOL: u16 = 0x0402;
const REG_AIF1_DAC1_RIGHT_VOL: u16 = 0x0403;

// DAC1 mixer volumes (sidetone digital gain) and routing.
//
// Per WM8994_Rev4.6.pdf p.67 + p.328-329 (memalpha-verified 2026-05-01):
// - R0x600 DAC1 Mixer Volumes: bits 8:5 ADCR_DAC1_VOL, bits 3:0
//   ADCL_DAC1_VOL — both 4-bit fields, 0000 = -36 dB up to 1100 = 0 dB
//   in 3 dB steps. Default 0000 ≈ silent.
// - R0x601 DAC1 Left Mixer Routing: bit 5 ADCR_TO_DAC1L, bit 4
//   ADCL_TO_DAC1L (sidetone enables), bit 0 AIF1DAC1L_TO_DAC1L.
// - R0x602 DAC1 Right Mixer Routing: bit 5 ADCR_TO_DAC1R, bit 4
//   ADCL_TO_DAC1R (sidetone enables), bit 0 AIF1DAC1R_TO_DAC1R.
//
// HISTORICAL BUG (DISCO-XX, fixed 2026-05-01): `REG_DAC1R_MIXER` was
// previously 0x0603 — that's actually DAC2 Mixer Volumes, an entirely
// different register controlling DAC2 sidetone gain. Right-channel
// routing writes were silently going to DAC2 vol bits and never
// touching DAC1R routing, which fortunately defaults to bit 0 = 1
// (AIF1DAC1R_TO_DAC1R enabled), so playback wasn't visibly broken —
// but the right-channel sidetone path was never wired through. The
// DAA-side wrapper had the same constant error; the rlvgl promotion
// inherited it. memalpha-quoted register-summary snapshot (R1538/0602h
// = "DAC1 Right Mixer Routing", R1539/0603h = "DAC2 Mixer Volumes")
// is the authoritative source.
const REG_DAC1_MIXER_VOLUMES: u16 = 0x0600;
const REG_DAC1L_MIXER: u16 = 0x0601;
const REG_DAC1R_MIXER: u16 = 0x0602;

// AIF1 ADC1 mixer routing (record path)
const REG_AIF1_ADC1L_MIXER: u16 = 0x0606;
const REG_AIF1_ADC1R_MIXER: u16 = 0x0607;
const REG_OVERSAMPLING: u16 = 0x0620;

// DAC volume
const REG_DAC1_LEFT_VOL: u16 = 0x0610;
const REG_DAC1_RIGHT_VOL: u16 = 0x0611;

// Software reset
const REG_SW_RESET: u16 = 0x0000;

// Expected chip ID value
const WM8994_ID: u16 = 0x8994;

/// Calibrated delay for the WM8994 codec settling waits (50 ms VMID ramp,
/// 15 ms charge-pump settle, etc.).
///
/// Uses `cortex_m::asm::delay(cycles)` — a single-cycle-accurate spin
/// implemented in assembly that LLVM's optimizer cannot collapse, even in
/// release mode. An earlier `core::hint::black_box`-based loop turned out
/// to be unreliable at `--release`: in optimized code the loop body
/// degenerates to a tight branch with no memory accesses and completes
/// far faster than the intended wall-clock time, cutting the VMID ramp
/// short and leaving the ADC outputting zeros indefinitely (per DAA bench
/// 2026-04-30 — the same failure mode this routine guards against).
///
/// Calibration: `sys_ck = 400 MHz` → 400 cycles per microsecond.
#[inline(always)]
fn delay_busy(microseconds: u32) {
    let cycles = (microseconds as u64).saturating_mul(400);
    let cycles = cycles.min(u32::MAX as u64) as u32;
    cortex_m::asm::delay(cycles);
}

/// Audio output destination.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputDevice {
    /// Route audio to headphone jack (CN11).
    Headphone,
    /// Route audio to speaker outputs (JP2/JP5).
    Speaker,
    /// Route audio to both headphone and speaker.
    Both,
}

/// Audio input source for the codec record path.
///
/// Selects which physical line-input the analog front-end routes through
/// the input mixer + ADC + AIF1ADC chain. PDM / digital MEMS microphones
/// bypass the codec entirely (they ride a separate SAI), so they do not
/// appear here — callers using PDM input simply do not invoke
/// [`Wm8994::init_record`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputDevice {
    /// LINEIN1L / LINEIN1R → IN1L / IN1R PGA → MIXINL / MIXINR → ADC1.
    /// The default production line-in on STM32H747I-DISCO (CN10 blue jack).
    LineIn1,
    /// LINEIN2L / LINEIN2R → IN2L / IN2R PGA → MIXINL / MIXINR → ADC1.
    /// Reserved second line-in input.
    LineIn2,
}

/// Result of FLL1 lock acquisition from [`Wm8994::init_record`].
///
/// Per `docs/audio/01-codec-bringup.md` AUDIO-01 §6 (frozen enum,
/// registration policy: Standards Action — adding a variant requires a
/// §15 amendment to AUDIO-01).
///
/// FLL1 lock-time on the WM8994 with F_REF = BCLK1 ≈ 1.5 MHz is
/// 30–50 ms typical (WM8994_Rev4.6.pdf p.202). The post-init AIF1
/// register writes in [`Wm8994::init_record`] depend on FLL1 having
/// reached lock; if those writes land first, the AIF1ADC1 serializer
/// arms against an unstable SYSCLK and latches a wrong bit-clock phase
/// (the symptom is L/R digital-capture asymmetry that survives every
/// analog-path test — see AUDIO-01 §2 for the full causal arc).
///
/// `Wm8994::init_record` clears the `FLL1_LOCK_EINT` latch at R0x731
/// bit 5, writes `FLL1_ENA = 1`, then polls the latch with a 100 ms
/// per-attempt timeout and up to 3 retries (per AUDIO-01 §9
/// INV-AUDIO-01-2). This enum reports which path the lock acquisition
/// took so callers can surface cold-vs-warm-reset telemetry without
/// re-deriving it from external bench observation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FllLockOutcome {
    /// FLL1 reported lock within the first poll loop's timeout. Healthy
    /// boot. Expected on every warm reset (BCLK1 already running from
    /// the prior session) and most cold boots after the FLL config is
    /// correctly programmed.
    LockedFirstTry,
    /// FLL1 acquired lock only after `attempts` re-arm cycles. Callers
    /// SHOULD log this via `defmt`, USART telemetry, or whatever
    /// out-of-band path the platform uses — a consistent non-zero
    /// `attempts` distribution on warm-reset boots indicates the static
    /// FLL config tuning is wrong; a non-zero distribution on cold-boot
    /// only is expected and acceptable.
    LockedAfterRetry {
        /// Number of full retry cycles that occurred before lock.
        /// `attempts: 1` means lock was reported on the SECOND attempt
        /// (i.e. after one re-arm). Maximum value is the retry budget
        /// from AUDIO-01 §9 INV-AUDIO-01-2 (currently 3).
        attempts: u8,
    },
    /// FLL1 never acquired lock within the configured retry budget. The
    /// codec is in an undefined state. Callers MUST NOT proceed with
    /// `init_record`'s post-FLL register sequence (AIF1ADC1 serializer
    /// arm, AIF1 framing writes, AIF1 LRCLK_DIR slave-mode enables,
    /// etc.) — `init_record` returns this variant early and the caller
    /// SHOULD treat it as an error (log + return `Err` to its own
    /// caller, or panic, depending on platform policy).
    Failed,
}

/// WM8994 audio codec driver.
pub struct Wm8994<I2C> {
    i2c: I2C,
}

impl<I2C> Wm8994<I2C>
where
    I2C: I2c<SevenBitAddress>,
{
    /// 7-bit I2C address of the WM8994 on the STM32H747I-DISCO.
    const ADDRESS: SevenBitAddress = 0x1A;

    /// Create a new driver from an I2C peripheral.
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Release the I2C peripheral.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// Perform a software reset of the codec.
    pub fn reset(&mut self) -> Result<(), I2C::Error> {
        self.write_reg(REG_SW_RESET, 0x0000)
    }

    /// Read the chip identification register.  Returns `0x8994` for WM8994.
    pub fn read_id(&mut self) -> Result<u16, I2C::Error> {
        self.read_reg(REG_CHIP_ID)
    }

    /// Verify the chip ID matches `0x8994`.
    pub fn verify_id(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.read_id()? == WM8994_ID)
    }

    /// Initialize the codec for I2S playback at the given sample rate.
    ///
    /// `mclk_hz` is the master clock frequency delivered on SAI1_MCLK_A (PG7).
    /// The WM8994 FLL is configured to generate exact internal clocks from this
    /// input.
    ///
    /// This configures AIF1 in I2S slave mode, 16-bit word length, and routes
    /// DAC1 to the selected output device.
    pub fn init_playback(
        &mut self,
        sample_rate: u32,
        mclk_hz: u32,
        output: OutputDevice,
    ) -> Result<(), I2C::Error> {
        // 1. Software reset
        self.reset()?;

        // Small delay for reset to settle (busy-loop ~1ms at 400 MHz)
        for _ in 0..400_000u32 {
            core::hint::black_box(0u32);
        }

        // 2. Anti-pop: enable VMID soft start
        self.write_reg(REG_ANTIPOP_2, 0x0048)?;

        // 3. Power management 1: bias enable, VMID = 2x50k
        self.write_reg(REG_PWR_MGMT_1, 0x0003)?;

        // Delay for VMID ramp (~50ms)
        for _ in 0..20_000_000u32 {
            core::hint::black_box(0u32);
        }

        // 4. Enable charge pump for headphone output
        self.write_reg(REG_CHARGE_PUMP_1, 0x8001)?;

        // 5. Configure FLL1 for exact audio clocking
        self.configure_fll1(sample_rate, mclk_hz)?;

        // 6. AIF1 clocking: source = FLL1, enable AIF1CLK
        self.write_reg(REG_AIF1_CLOCKING_1, 0x0011)?; // FLL1 source, AIF1CLK_ENA

        // 7. R0x208 Clocking (1) per WM8994_Rev4.6.pdf p.284:
        //   bit 0 SYSCLK_SRC (0 = MCLK1, 1 = AIF1CLK)
        //   bit 1 SYSDSPCLK_ENA
        //   bit 2 AIF2DSPCLK_ENA
        //   bit 3 AIF1DSPCLK_ENA
        //   bit 4 TOCLK_ENA
        // 0x000A = bits 1+3 → SYSDSPCLK + AIF1DSPCLK enabled. SYSCLK_SRC=0
        // (MCLK1) — relies on MCLK1 being externally driven (SAI1_MCLK_A
        // on PG7 → codec MCLK1 input). FLL1 separately drives AIF1CLK
        // for the AIF1 path (see R0x200 = 0x0011 above).
        self.write_reg(REG_CLOCKING_1, 0x000A)?;

        // 8. R0x210 AIF1 Rate per WM8994_Rev4.6.pdf p.194 / 285-286
        //   (memalpha-verified, DISCO-XX 2026-04-30):
        //   bits 7:4 AIF1_SR — sample-rate code:
        //     0=8k, 1=11.025k, 2=12k, 3=16k, 4=22.05k, 5=24k, 6=32k,
        //     7=44.1k, 8=48k, 9=88.2k, A=96k.
        //   bits 3:0 AIF1CLK_RATE — AIF1CLK / Fs ratio:
        //     1=128, 2=192, 3=256, 4=384, 5=512, 6=768, 7=1024, 8=1408,
        //     9=1536.
        //   FLL1 is configured for `sample_rate * 256` (configure_fll1),
        //   so AIF1CLK_RATE = 0x3 (256·Fs) matches the FLL output.
        let rate_bits: u16 = match sample_rate {
            8000 => 0x00,
            11025 => 0x01,
            12000 => 0x02,
            16000 => 0x03,
            22050 => 0x04,
            24000 => 0x05,
            32000 => 0x06,
            44100 => 0x07,
            48000 => 0x08,
            88200 => 0x09,
            96000 => 0x0A,
            _ => 0x08, // default 48 kHz
        };
        self.write_reg(REG_AIF1_RATE, (rate_bits << 4) | 0x0003)?;

        // 9. AIF1 control: I2S format, 16-bit word length
        self.write_reg(REG_AIF1_CONTROL_1, 0x4010)?; // FMT=I2S, WL=16-bit

        // 10. AIF1 master/slave: slave mode (SAI1 is master)
        self.write_reg(REG_AIF1_MASTER_SLAVE, 0x0000)?;

        // 11. AIF1 clocking 2: divide by 1
        self.write_reg(REG_AIF1_CLOCKING_2, 0x0000)?;

        // 12. R0x209 Clocking (2) per WM8994_Rev4.6.pdf p.285:
        //   bits 10:8 TOCLK_DIV, bits 6:4 DBCLK_DIV, bits 2:0 OPCLK_DIV.
        //   No AIF1DAC/ADC clock enables here — those come from
        //   AIF1DSPCLK_ENA in R0x208 above. Writing 0x0003 sets
        //   OPCLK_DIV[2:0] = 011 = divide-by-4 for the OPCLK output
        //   (only used if explicitly routed to a GPIO; benign default).
        //   Pre-DISCO-XX this address was incorrectly 0x210 which is
        //   AIF1 Rate; that overwrote the correct AIF1 rate config but
        //   the bug was masked because R0x211 (also misnamed) was
        //   getting the rate value first and the codec accepted the
        //   stomp on R0x210.
        self.write_reg(REG_CLOCKING_2, 0x0003)?;

        // 13. Power management: enable DACs and output paths
        self.configure_output(output)?;

        // 14. AIF1 DAC1 filter: enable, normal operation
        self.write_reg(REG_AIF1_DAC1_FILTER_1, 0x0000)?;

        // 15. DAC1 volume: 0 dB
        self.write_reg(REG_DAC1_LEFT_VOL, 0x01C0)?; // VU + 0dB
        self.write_reg(REG_DAC1_RIGHT_VOL, 0x01C0)?;

        // 16. AIF1 DAC1 volume: 0 dB
        self.write_reg(REG_AIF1_DAC1_LEFT_VOL, 0x01C0)?;
        self.write_reg(REG_AIF1_DAC1_RIGHT_VOL, 0x01C0)?;

        Ok(())
    }

    /// Set headphone output volume (0 = mute, 63 = max, ~+6 dB).
    pub fn set_headphone_volume(&mut self, vol: u8) -> Result<(), I2C::Error> {
        let v = (vol.min(63) as u16) | 0x0140; // VU bit + zero-cross
        self.write_reg(REG_LEFT_OUTPUT_VOL, v)?;
        self.write_reg(REG_RIGHT_OUTPUT_VOL, v)?;
        Ok(())
    }

    /// Set speaker output volume (0 = mute, 63 = max).
    pub fn set_speaker_volume(&mut self, vol: u8) -> Result<(), I2C::Error> {
        let v = (vol.min(63) as u16) | 0x0140; // VU bit + zero-cross
        self.write_reg(REG_SPEAKER_VOL_LEFT, v)?;
        self.write_reg(REG_SPEAKER_VOL_RIGHT, v)?;
        Ok(())
    }

    /// Mute or unmute DAC output.
    pub fn mute(&mut self, mute: bool) -> Result<(), I2C::Error> {
        if mute {
            self.write_reg(REG_AIF1_DAC1_FILTER_1, 0x0200)?; // MUTE
        } else {
            self.write_reg(REG_AIF1_DAC1_FILTER_1, 0x0000)?; // unmute
        }
        Ok(())
    }

    /// Initialize the codec for I²S record at the given sample rate.
    ///
    /// `sample_rate` is the AIF1 sample rate in Hz. Supported values:
    /// 8 kHz, 11.025 kHz, 12 kHz, 16 kHz, 22.05 kHz, 24 kHz, 32 kHz,
    /// 44.1 kHz, 48 kHz, 88.2 kHz, 96 kHz. Unrecognised values fall back
    /// to 48 kHz.
    ///
    /// `_mclk_hz` is currently ignored — the FLL locks to BCLK1 at
    /// `32 × sample_rate`, not MCLK1. Most carrier boards (including the
    /// H747I-DISCO via CN15 to MB1166) do not route MCLK1 across the
    /// LCD-module connector to the codec; only BCLK/LRCLK/data make it
    /// across. Locking the FLL to BCLK is the only viable record-path
    /// clocking on those boards. The parameter is preserved in the
    /// signature so adding MCLK-source record support later does not
    /// break call sites.
    ///
    /// `input` selects the analog input route:
    /// - [`InputDevice::LineIn1`]: IN1L/IN1R PGA + MIXINL/MIXINR + ADC1.
    /// - [`InputDevice::LineIn2`]: IN2L/IN2R PGA + MIXINL/MIXINR + ADC1.
    ///
    /// On return, the codec is streaming AIF1ADC1L/R out on the AIF1
    /// data line (sub-block B's SD on the H747I-DISCO wiring) at the
    /// configured Fs. The caller is responsible for starting SAI1 RX.
    ///
    /// The implementation also brings up the AIF1DAC + DAC1L/R + MIXOUT
    /// + HPOUT1 path so the headphone jack drives audio derived from
    /// the codec digital sidetone (ADCL/R → DAC1L/R) plus any AIF1DAC
    /// data the SAI1 TX side feeds in. This makes the live record path
    /// audible at CN11 without requiring a separate `init_playback`
    /// call. The caller MUST follow up with [`Self::init_hp_output_stage`],
    /// wait ≥256.5 ms for the DC servo, then call
    /// [`Self::finish_hp_output_stage`] to bring the HP drivers fully
    /// online (per WM8994_Rev4.6.pdf Table 123 cold-start sequence).
    ///
    /// Per WM8994_Rev4.6.pdf register-bit assignments referenced inline.
    /// Bench-validated against STM32H747I-DISCO + WM8994 (rev A
    /// MB1166) by disco-analyzer (DAA-01-E §6, 2026-04-30); promoted
    /// to rlvgl-platform (DISCO-XX) so other consumers on the same
    /// codec inherit the bench-iterated register sequence.
    pub fn init_record(
        &mut self,
        sample_rate: u32,
        _mclk_hz: u32,
        input: InputDevice,
    ) -> Result<FllLockOutcome, I2C::Error> {
        // Step 1 — software reset.
        self.reset()?;
        // Reset settling: by the time the next I²C transaction lands,
        // the codec's internal blocks have de-asserted.

        // Step 2 — Anti-pop bias control (R0x39).
        // STM32CubeH7 BSP wm8994_drv.c uses 0x6C: VMID_RAMP[6:5]=0b11
        // (slow ramp, most stable) + VMID_BUF_ENA + STARTUP_BIAS_ENA.
        self.write_reg(REG_ANTIPOP_2, 0x006C)?;

        // Step 3 — Power-up VMID + master bias + MICBIAS (R1).
        // Datasheet §6.6.4. PM1 bits:
        //   bit 0 BIAS_ENA, bits 2:1 VMID_SEL=01 (2x40k normal-op),
        //   bit 4 MICB1_ENA, bit 5 MICB2_ENA. Some carrier boards
        //   route the line-in jack through MICBIAS for DC coupling
        //   (especially combined line-in/mic-in jacks) — enable both
        //   bias rails to cover that case.
        self.write_reg(REG_PWR_MGMT_1, 0x0033)?;
        // VMID ramp time ~50 ms. CRITICAL: without this delay the
        // input section powers up while VMID is still ramping; the
        // ADC's voltage references never stabilize and the chip
        // outputs zeros indefinitely. (Bench-confirmed 2026-04-30.)
        delay_busy(50_000);

        // Step 4 — Charge pump enable (R0x4C). Cube BSP enables this
        // even for record-only paths — CP_ENA powers VMID's bias rail
        // used by the input PGAs; disabling it makes the input stage
        // brown out under load. Standard Cube value 0x9F25.
        self.write_reg(REG_CHARGE_PUMP_1, 0x9F25)?;
        delay_busy(15_000); // ~15 ms charge-pump settle (Cube)

        // Step 5 — Power up input mixer + ADCs (R2). PM2 bits:
        //   bit 14 TSHUT_ENA (default 1)
        //   bit 13 TSHUT_OPDIS (default 1)
        //   bit 9  MIXINL_ENA  (left input mixer)
        //   bit 8  MIXINR_ENA  (right input mixer)
        //   bit 7  IN2L_ENA, bit 5 IN2R_ENA
        //   bit 6  IN1L_ENA, bit 4 IN1R_ENA
        let pm2: u16 = match input {
            // TSHUT defaults + MIXINL/R + IN1L + IN1R = 0x6350
            InputDevice::LineIn1 => {
                (1 << 14) | (1 << 13) | (1 << 9) | (1 << 8) | (1 << 6) | (1 << 4)
            }
            // TSHUT defaults + MIXINL/R + IN2L + IN2R = 0x63A0
            InputDevice::LineIn2 => {
                (1 << 14) | (1 << 13) | (1 << 9) | (1 << 8) | (1 << 7) | (1 << 5)
            }
        };
        self.write_reg(REG_PWR_MGMT_2, pm2)?;

        // Step 6 — Power up the analog ADC blocks ONLY (R4 bits 1:0).
        //   bit 1 ADCL_ENA, bit 0 ADCR_ENA
        // AUDIO-01-d H0 (serializer arm-phase reorder): the AIF1ADC1L/R_ENA
        // serializer-enable bits (b9/b8) are deliberately NOT set here.
        // Arming the serializer before AIF1CLK is locked + running latches a
        // metastable BCLK phase offset N≠0 vs LRCLK1 (ERRATA-004 in
        // disco-analyzer). The b9/b8 enables are deferred to Step 14b below,
        // after R0x200 AIF1CLK_ENA + the 50 ms settle, so the serializer arms
        // against an already-stable, already-running LRCLK1 frame. This also
        // brings the code into compliance with AUDIO-01 §12 gate (c) (no
        // writes to R0x4 AIF1ADC1L/R_ENA until FLL1 lock). See
        // AGENT-TASK-WM8994-AIF1-SERIALIZER-ARM-PHASE-CALIBRATION.md §0.
        self.write_reg(REG_PWR_MGMT_4, (1 << 1) | 1)?;

        // Step 7 — Enable the analog DAC blocks ONLY (R5 bits 1:0) for
        // record-with-monitor.
        //   bit 1 DAC1L_ENA, bit 0 DAC1R_ENA
        // The DAC path stays live so the headphone jack can monitor the
        // live record stream (digital sidetone from ADC1L/R).
        // AUDIO-01-d H0: the AIF1DAC1L/R_ENA serializer-enable bits (b9/b8)
        // are likewise deferred to Step 14b (same arm-phase rationale as the
        // ADC side; ERRATA-009 is the DAC-side manifestation). 0x0003 keeps
        // only DAC1L/R_ENA.
        self.write_reg(REG_PWR_MGMT_5, 0x0003)?;

        // Step 8 — PM6 default (R6). Bits gate AIF2/AIF3, not AIF1 ADC.
        self.write_reg(REG_PWR_MGMT_6, 0x0000)?;

        // Step 9 — Input PGA volumes (R0x18, R0x1A).
        //   bit 8 IN1_VU (volume update strobe)
        //   bit 7 IN1L_MUTE (0 = unmuted)
        //   bits 4:0 IN1L_VOL — 0x0B = 0 dB, 0x1F = +30 dB.
        // 0x010B = VU + unmuted + 0 dB — the codec default volume,
        // appropriate for line-level inputs (~1 V RMS from a USB-audio
        // line-out). DAA's bench iteration briefly used +30 dB (0x011F)
        // to make tiny exploration signals detectable during the
        // AIF1ADCDAT silence chase, but +30 dB clips line-level sources
        // (Mac USB audio → distortion → drops); keep the production
        // default at 0 dB.
        let line_vol: u16 = 0x010B;
        self.write_reg(REG_LEFT_LINE_IN1_2_VOL, line_vol)?;
        self.write_reg(REG_RIGHT_LINE_IN1_2_VOL, line_vol)?;

        // Step 10 — Input mixer routing (R0x28, R0x29, R0x2A).
        // R0x28 Input Mixer 2 — IN1{L,R}{P,N}_TO_IN1{L,R} pad selects.
        // The H747I-DISCO line-in jack TRS wiring drives both P/N pins
        // (bench 2026-04-29: Cube's 0x0011 N-only and 0x0022 P-only
        // both produced exact-zero ADC output even with otherwise
        // correct config). 0x0033 sums P+N pads — bench-validated to
        // produce non-zero ADC output on this board.
        let input_mix_2: u16 = match input {
            InputDevice::LineIn1 | InputDevice::LineIn2 => 0x0033,
        };
        self.write_reg(REG_INPUT_MIXER_2, input_mix_2)?;

        // R0x29 Input Mixer 3 / R0x2A Input Mixer 4: per-input PGA →
        // MIXIN routing.
        //   IN1L_TO_MIXINL bit 5 (1 = un-mute IN1L → MIXINL)
        //   IN1L_MIXINL_VOL bit 4 (1 = +30 dB on this gain stage)
        //   IN2L_TO_MIXINL bit 8 (1 = un-mute IN2L → MIXINL)
        //   IN2L_MIXINL_VOL bit 7 (1 = +30 dB on this gain stage)
        // Drop bits 2:0 (MIXOUTL→MIXINL loopback) — that's a play→record
        // loop that injects DC bias from the un-clocked DAC into the
        // ADC front-end (bench 2026-04-30 caught this).
        // Use the un-mute bit only (no +30 dB stage). The PGA gain at
        // R0x18/0x1A is the load-bearing input-level control; layering
        // a second +30 dB here in series with the PGA's own +30 dB
        // saturates line-level sources (Mac USB audio) → distortion +
        // codec overload protection drops the path (bench 2026-05-01).
        let (mixer3, mixer4) = match input {
            InputDevice::LineIn1 => (0x0020u16, 0x0020u16), // IN1L/R → MIXINL/R, 0 dB
            InputDevice::LineIn2 => (0x0100u16, 0x0100u16), // IN2L/R → MIXINL/R, 0 dB
        };
        self.write_reg(REG_INPUT_MIXER_3, mixer3)?;
        self.write_reg(REG_INPUT_MIXER_4, mixer4)?;

        // Step 11 — AIF1 ADC mixer (R0x606, R0x607).
        // Route ADC1{L,R} → AIF1ADC1{L,R}: ADC1L_TO_AIF1ADC1L bit set.
        self.write_reg(REG_AIF1_ADC1L_MIXER, 0x0002)?;
        self.write_reg(REG_AIF1_ADC1R_MIXER, 0x0002)?;

        // Step 12 — FLL1 ← BCLK1, F_OUT = 12.5 MHz target (256·Fs).
        // Datasheet §6.4.1, p.202-207, Tables 110-115:
        //   F_VCO = (F_REF / REFCLK_DIV) × (N + K/65536) × FRATIO
        //   F_OUT = F_VCO / OUTDIV
        //   90 MHz ≤ F_VCO ≤ 100 MHz (spec)
        //
        // Inputs: F_REF = BCLK1 ≈ 1.5625 MHz (32×Fs at Fs≈48.83 kHz).
        // Targets: F_OUT = 12.5 MHz so AIF1CLK = 256·Fs.
        // Per Table 111: F_REF in 1-13.5 MHz → FRATIO=0 (÷1).
        // Per Table 112: REFCLK_DIV=00 (÷1), REFCLK_SRC=11 (BCLK1).
        // OUTDIV=8 → F_VCO=100 MHz ✓. OUTDIV-1=7 (R0x221 bits 13:8).
        // N×FRATIO = 100/1.5625 = 64. So N=64, K=0. R0x223 bits 14:5
        // = 64<<5 = 0x0800. Earlier OUTDIV=4 → F_VCO=50 MHz was
        // out-of-range; FLL never locked, AIF1CLK never ran, every
        // downstream AIF1 register write was silently rejected
        // (bench-confirmed via codec readback 2026-04-30).
        let _ = sample_rate;

        // ── AUDIO-01-d open-loop fix (ERRATA-004/009): source AIF1CLK
        //    from the coherent MCLK1, NOT FLL1-locked-to-BCLK1. ───────
        //
        // Root cause of the serializer bit-clock-phase race: with
        // AIF1CLK_SRC=FLL1, the FLL VCO locks to BCLK1 at a phase that
        // is RANDOM at each cold power-up and latched for the session.
        // The decimator output (AIF1CLK domain) is then handed to the
        // BCLK1-domain ADC serializer at that random phase, slipping the
        // sample by ±1+ BCLK → the N≠0 wrap / byte-stuck captures. The
        // serializer itself frame-syncs to LRCLK (datasheet p.174), so
        // it is NOT the source of the varying N — the FLL VCO phase is.
        // No firmware delay or rerun can make an analog VCO phase
        // deterministic, so the only OPEN-LOOP fix is to remove the FLL.
        //
        // On boards that route a clean MCLK = 256·Fs to the codec MCLK1
        // pin (STM32H747I-DISCO: SAI1_MCLK_A on PG7 → CN15 → MCLK1,
        // phase-coherent with BCLK1 because both come from the MCU's one
        // SAI1 master / PLL3 clock tree), selecting MCLK1 as AIF1CLK
        // removes the VCO entirely → the AIF1CLK↔BCLK1 phase is fixed by
        // construction → deterministic N. Datasheet R0x200 p.193: "if
        // MCLK1 is selected as the AIF1CLK source, the FLL(s) are not
        // required to be enabled." AIF1CLK_RATE stays 256× (R0x210).
        //
        // Const-gated: a board WITHOUT a coherent codec MCLK must keep
        // the FLL-from-BCLK path (set false). MCLK1 is the default.
        const AIF1CLK_FROM_MCLK1: bool = true;

        let lock_outcome = if AIF1CLK_FROM_MCLK1 {
            // No FLL. AIF1CLK is sourced from MCLK1 at the R0x200 write
            // below; the caller must already be driving MCLK1 (256·Fs).
            FllLockOutcome::LockedFirstTry
        } else {
            // Step 12 — FLL1 ← BCLK1, F_OUT = 12.5 MHz (256·Fs). See the
            // F_VCO/OUTDIV math above. Enable FLL1 and poll R0x731 bit 5
            // (FLL1_LOCK_EINT) for lock before any post-FLL write lands
            // (AUDIO-01 §9 INV-AUDIO-01-1). On Failed, return early — the
            // codec is in an undefined state and the rest of init_record
            // MUST NOT proceed (AUDIO-01 §12 (d)+(f)).
            self.write_reg(REG_FLL1_CTRL_5, 0x0003)?; // REFCLK_SRC=BCLK1, REFCLK_DIV=÷1
            self.write_reg(REG_FLL1_CTRL_4, 0x0800)?; // N=64
            self.write_reg(REG_FLL1_CTRL_3, 0x0000)?; // K=0
            self.write_reg(REG_FLL1_CTRL_2, 7u16 << 8)?; // OUTDIV-1=7, FRATIO=0
            let o = self.fll1_enable_and_wait_lock_bclk1()?;
            if matches!(o, FllLockOutcome::Failed) {
                return Ok(o);
            }
            o
        };

        // Step 13 — AIF1 format BEFORE clocks (Cube order). R0x300:
        //   bits 4:3 AIF1_FMT = 0b10 (I²S)
        //   bits 6:5 AIF1_WL  = 0b00 (16-bit)
        //   bit 13 AIF1ADC_TDM = 0
        //   bit 14 AIF1ADCR_SRC = 1 (right ADC → right channel) — must
        //                          preserve the reset default; clearing
        //                          it sends LEFT ADC to BOTH channels.
        // Final: 0x4010.
        self.write_reg(REG_AIF1_CONTROL_1, 0x4010)?;
        self.write_reg(REG_AIF1_MASTER_SLAVE, 0x0000)?; // codec is slave

        // R0x304 AIF1 ADC LRCLK and R0x305 AIF1 DAC LRCLK.
        // Bit 11 = AIF1{ADC,DAC}_LRCLK_DIR — must be set in SLAVE mode
        // to enable the codec to *receive* the external LRCLK1 from the
        // SAI1 master. Without these bits the codec ignores LRCLK1
        // entirely, never knows where slot boundaries fall, and never
        // shifts ADC data out on AIF1ADCDAT (= MCU PE3) — pin reads
        // 0 forever, RX captures all-zero halfwords. AIF1CLK / SYSCLK
        // / FLL1 still lock from BCLK1, so codec-internal sidetone
        // (analog → ADC1 → DSP → DAC1 → analog) appears to work,
        // masking the problem until the AIF1 digital path is exercised.
        // Multi-session bench debug 2026-04-30 → 2026-05-01.
        //
        // Bits 10:0 = AIF1{ADC,DAC}_RATE: the BCLK1 / LRCLK1 ratio.
        // Per `docs/audio/01-codec-bringup.md` AUDIO-01 §15 amendment
        // 2026-05-19-c (bench-driven): this MUST match the actual ratio
        // produced by the SAI master, not a "codec reset default" or
        // CubeH7-BSP-inherited value. With the SAI1 master configured
        // for a 32-BCLK frame with two 16-bit slots (the common stereo
        // I²S 16-bit configuration), BCLK1 / LRCLK1 = 32, so the field
        // value is 0x020. With bit 11 set, AIF1_LRCLK_SLAVE_ENA = 0x0820.
        //
        // **Historical bug** (DAA bench 2026-05-19, fixed by AUDIO-01-c):
        // earlier versions hardcoded 0x0840 (RATE=64) with a comment
        // claiming "matches 32-BCLK-per-stereo-frame layout padded
        // internally to 32-bit slots." Bench measurement on STM32H747I-
        // DISCO refuted the claim: with RATE=64 mismatched against
        // actual BCLK1/LRCLK1 = 32, the codec's AIF1ADC1L serializer
        // state machine fails asymmetrically — it emits valid data
        // only during the last 5 BCLKs of what it believes is the L
        // MSB window, holding AIF1ADCDAT at logic 0 (per TDM=0
        // behavior) for the remaining 11 BCLKs of slot 0. The R
        // serializer is silicon-level more tolerant of the mismatch
        // and emits cleanly. Net symptom: L channel reads as a 2-value
        // bimodal `0x0000 / 0x001f` at SAI1 RX (bottom 5 bits varying,
        // top 11 zero); R reads as clean 16-bit audio. Setting RATE=32
        // (matching the actual SAI framing) recovers L to ±99% FS with
        // L/R correlation +0.92 on a stereo source.
        //
        // **Caveat:** this value assumes the caller's SAI master
        // generates a 32-BCLK frame. If a future use case configures
        // a different framing (e.g. TDM 4-slot at 64-BCLK frame), the
        // RATE field MUST be updated to match. A parameterizable
        // version of `init_record` is future work — see AUDIO-NN
        // chapter pipeline.
        const AIF1_LRCLK_SLAVE_ENA: u16 = 0x0820; // bit 11 (LRCLK_DIR) + RATE=32
        self.write_reg(REG_AIF1_ADC_LRCLK, AIF1_LRCLK_SLAVE_ENA)?;
        self.write_reg(REG_AIF1_DAC_LRCLK, AIF1_LRCLK_SLAVE_ENA)?;

        // R0x210 AIF1 Rate. Per WM8994_Rev4.6.pdf p.194 Tables 105+106:
        //   bits 7:4 AIF1_SR — sample-rate code (0x0=8k..0xA=96k)
        //   bits 3:0 AIF1CLK_RATE — AIF1CLK/Fs ratio (0x3=256)
        // FLL1's F_OUT becomes AIF1CLK; with F_OUT=12.5 MHz and
        // Fs≈48.83 kHz the ratio is 256 → AIF1CLK_RATE field = 0x3.
        let aif1_rate: u16 = match sample_rate {
            8_000 => 0x03,
            11_025 => 0x13,
            12_000 => 0x23,
            16_000 => 0x33,
            22_050 => 0x43,
            24_000 => 0x53,
            32_000 => 0x63,
            44_100 => 0x73,
            48_000 => 0x83,
            88_200 => 0x93,
            96_000 => 0xA3,
            _ => 0x83,
        };
        self.write_reg(REG_AIF1_RATE, aif1_rate)?;

        // Step 14 — AIF1 clocking AFTER format/rate. R0x208 Clocking 1:
        //   bit 4 TOCLK_ENA      = 0
        //   bit 3 AIF1DSPCLK_ENA = 1
        //   bit 2 AIF2DSPCLK_ENA = 0
        //   bit 1 SYSDSPCLK_ENA  = 1  ← critical; without it the ADC
        //                              datapath is unclocked and outputs
        //                              zeros forever (bench-2026-04-30).
        //   bit 0 SYSCLK_SRC     = 0 (SYSCLK from AIF1CLK)
        self.write_reg(REG_CLOCKING_1, 0x000A)?;

        // R0x200 AIF1CLK: ENA=1 (bit 0) + SRC (bits 4:3). Written LAST in
        // the clock chain so format/rate are already in place when
        // AIF1CLK starts running — R0x300/R0x410 require AIF1CLK live for
        // writes to take effect. Datasheet p.193 SRC encoding:
        //   MCLK1 path → SRC=00 → 0x0001 (AUDIO-01-d open-loop fix)
        //   FLL1  path → SRC=10 → 0x0011 (legacy FLL-from-BCLK)
        let aif1clk = if AIF1CLK_FROM_MCLK1 { 0x0001 } else { 0x0011 };
        self.write_reg(REG_AIF1_CLOCKING_1, aif1clk)?;
        delay_busy(5_000); // AIF1CLK detector settle

        // ADC startup settle. After AIF1CLK_ENA latches the ADC's
        // sample-rate converter and digital filters need a few SR
        // cycles to flush stale state. ~50 ms is the canonical Cube
        // BSP value; shorter waits cause first-frame glitches.
        delay_busy(50_000);

        // ADC digital-chain pre-arm setup (bench 2026-06-12).
        //
        // The lever here is ORDERING, not register values. Datasheet
        // (WM8994 Rev 4.6 pp.61/238/357) requires every clock + signal-path
        // register to be live before the ADC / AIF1ADC enable edges. Program
        // the ADC digital volume + filter state here, before the
        // AIF1ADC1L/R_ENA arm edge below, so the digital chain is in its
        // intended state when the serializer starts (ERRATA-004/009
        // serializer bit-clock-phase race).
        self.write_reg(REG_AIF1_ADC1_L_VOL, 0x01C0)?;
        self.write_reg(REG_AIF1_ADC1_R_VOL, 0x01C0)?;
        // HPF enabled on both channels, hi-fi cutoff mode.
        self.write_reg(REG_AIF1_ADC1_FILTERS, 0x1800)?;
        // R0x620 (Oversampling): per datasheet p.200, bit 1 = ADC_OSR128
        // (default 1 = high-perf 128x), bit 0 = DAC_OSR128 (default 0).
        // NOTE: an earlier bench note had these bits SWAPPED and claimed the
        // observed 0x0002 left the ADC in 64x low-power mode — that was
        // wrong. 0x0002 already has the ADC at 128x; 0x0003 only additionally
        // raises the DAC to 128x. This write is therefore a DAC-side touch,
        // NOT the ADC-phase fix — the ADC OSR value was never the lever. Kept
        // for symmetry within the pre-arm ordering block.
        self.write_reg(REG_OVERSAMPLING, 0x0003)?;
        delay_busy(5_000);

        // Step 14b — Arm the AIF1 serializers (AUDIO-01-d H0).
        // Only now — FLL1 locked (Step 12), codec in slave LRCLK mode
        // (R0x304/305), AIF1CLK enabled + settled (R0x200 above) — set the
        // AIF1ADC1L/R_ENA (R4 b9/b8) and AIF1DAC1L/R_ENA (R5 b9/b8)
        // serializer-enable bits. The 0→1 edge on these bits is the
        // serializer-arm event; performing it against an already-running,
        // phase-stable LRCLK1 frame is the H0 fix for the N≠0 arm-phase race
        // (ERRATA-004 ADC / ERRATA-009 DAC). Read-modify-write preserves the
        // analog ADCL/R + DAC1L/R enables set in Steps 6/7.
        //
        // BENCH-PENDING: this reorder is the lead AUDIO-01-d candidate but is
        // NOT yet bench-validated. Validate via the disco-analyzer mailbox
        // re-arm test (≥10 cold boots, both directions) before relying on it.
        // See AGENT-TASK-WM8994-AIF1-SERIALIZER-ARM-PHASE-CALIBRATION.md §0.
        let pm4 = self.read_reg(REG_PWR_MGMT_4)?;
        self.write_reg(REG_PWR_MGMT_4, pm4 | (1 << 9) | (1 << 8))?;
        let pm5 = self.read_reg(REG_PWR_MGMT_5)?;
        self.write_reg(REG_PWR_MGMT_5, pm5 | (1 << 9) | (1 << 8))?;

        // Step 14b settle — let the freshly-armed serializers lock to the
        // LRCLK1 frame and the digital-core FIFO flush before the path is
        // exercised. The arm 0→1 edge needs a few LRCLK frames (~20 µs each
        // at 48 kHz) to settle; 50 ms is the conservative Cube-magnitude
        // value and is bench-tunable. AUDIO-01-d H0 refinement 2026-05-29:
        // added a *post*-arm settle (the prior 50 ms covers only the
        // pre-arm AIF1CLK/ADC startup). WM8994_Rev4.6 exposes no ADC/DSP/
        // filter-ready status bit to poll instead (memalpha 2026-05-29; only
        // the Digital-Core-FIFO-Error rate-mismatch flag exists, p.158), so a
        // timed settle is the only available gate. See
        // AGENT-TASK-WM8994-AIF1-SERIALIZER-ARM-PHASE-CALIBRATION.md §0.
        delay_busy(50_000);

        // Step 15 — ADC volume + filters (R0x400, R0x401, R0x410).
        //   bit 8 AIF1ADC1_VU (volume update strobe)
        //   bits 7:0 AIF1ADC1L_VOL — 0xC0 = 0 dB.
        self.write_reg(REG_AIF1_ADC1_L_VOL, 0x01C0)?;
        self.write_reg(REG_AIF1_ADC1_R_VOL, 0x01C0)?;
        // Keep the pre-arm ADC filter state. A prior post-init DAA-side
        // override wrote this value after init_record returned; writing it
        // here keeps the driver state and serializer arm edge aligned.
        self.write_reg(REG_AIF1_ADC1_FILTERS, 0x1800)?;

        // ── Output staging for live record monitor at CN11 ─────────
        // Configure the codec's analog output path so the HP/line-out
        // jack drives audio derived from AIF1DAC + ADC sidetone. This
        // doubles as a liveness check — if no audio appears at CN11,
        // the codec analog output path is broken; if audio appears the
        // record-side digital silence (if any) is in the AIF1ADC →
        // ADCDAT1 leg specifically.

        // R0x03 PM3: HEADPHONE mode — MIXOUTL/R enables (bits 4:5).
        self.write_reg(REG_PWR_MGMT_3, 0x0030)?;
        // R0x01 PM1: add HPOUT1L_ENA (bit 9) + HPOUT1R_ENA (bit 8) to
        // existing BIAS+VMID+MICBIAS = 0x0033 → 0x0333.
        self.write_reg(REG_PWR_MGMT_1, 0x0333)?;
        // R0x600 DAC1 Mixer Volumes — sidetone digital gain.
        //   bits 8:5 ADCR_DAC1_VOL[3:0]: STR sidetone vol (1100 = 0 dB)
        //   bits 3:0 ADCL_DAC1_VOL[3:0]: STL sidetone vol (1100 = 0 dB)
        // Reset default = 0000_0000 ≈ -36 dB ≈ inaudible. Without this
        // write the sidetone routing bits below have no audible effect
        // (memalpha-verified 2026-05-01 — this was the load-bearing
        // missing register write that hid the working ADC chain behind
        // -36 dB attenuation; symptom was "sidetone path silent" even
        // when the analog input reached the codec correctly via the
        // R0x2D/0x2E IN1→MIXOUT bypass).
        // 0x018C = (0xC << 5) | 0xC = both channels at 0 dB.
        self.write_reg(REG_DAC1_MIXER_VOLUMES, 0x018C)?;
        // R0x601 DAC1 Left Mixer Routing (memalpha-verified
        // WM8994_Rev4.6.pdf p.328):
        //   bit 5 ADCR_TO_DAC1L  (right-channel sidetone → L; cross-feed)
        //   bit 4 ADCL_TO_DAC1L  (sidetone STL → DAC1L; bench-validated
        //                         2026-05-01 to carry line-in music
        //                         audibly once R0x600 vol is set)
        //   bit 0 AIF1DAC1L_TO_DAC1L (AIF1 left → DAC1L)
        // `init_record` default is **sidetone only** — bit 4 set, bit 0
        // CLEARED (= 0x0010). Reasoning: this routine sets up the codec
        // for record. Whatever the application sends on SAI1 TX (e.g.
        // a bench fixture, a captured loopback, or silence) shouldn't
        // leak into the headphone monitor unless the caller explicitly
        // opts in. Callers wanting full record-with-AIF1-playback can
        // OR bit 0 in via a follow-up `write_reg(0x0601, 0x0011)` after
        // init_record returns.
        self.write_reg(REG_DAC1L_MIXER, 0x0010)?;
        // R0x602 DAC1 Right Mixer Routing (memalpha-verified
        // WM8994_Rev4.6.pdf p.328 — was mis-addressed as 0x0603 = DAC2
        // Mixer Volumes prior to 2026-05-01 fix; this write never
        // reached DAC1R routing in earlier code revisions. The codec's
        // reset default for R0x602 has bit 0 = 1 so AIF1 right playback
        // happened to work via default routing, masking the bug for
        // playback-only callers; record-side sidetone on the right
        // channel was permanently silent until this fix). Bits:
        //   bit 5 ADCR_TO_DAC1R  (sidetone STR → DAC1R)
        //   bit 4 ADCL_TO_DAC1R  (left-channel sidetone → R; cross-feed)
        //   bit 0 AIF1DAC1R_TO_DAC1R (AIF1 right → DAC1R)
        // Sidetone-only by default (bit 5 set, bit 0 cleared = 0x0020),
        // same reasoning as R0x601 above. Note this overrides the
        // codec's reset default of bit 0 = 1, so playback-only callers
        // who previously relied on init_record's right-channel default
        // routing will need to opt in explicitly post-call.
        self.write_reg(REG_DAC1R_MIXER, 0x0020)?;
        // R0x2D / R0x2E Output Mixer 1/2: bit 0 DAC1L/R_TO_MIXOUTL/R
        // unmute. The bit-6 IN1L/R analog passthrough was a 2026-05-01
        // bench probe used to prove the input chain worked while the
        // sidetone path was silent; the real silence cause was the
        // missing R0x600 write above, so the analog passthrough is
        // dropped from production now.
        self.write_reg(REG_OUTPUT_MIXER_1, 0x0001)?;
        self.write_reg(REG_OUTPUT_MIXER_2, 0x0001)?;
        // R0x1E Line-out volume: clear all 4 mute bits, 0 dB.
        self.write_reg(REG_LINEOUT_VOL, 0x0000)?;
        // R0x1C / R0x1D HPOUT1 L/R: VU + unmuted, 0 dB.
        //   bit 8 HPOUT1_VU, bit 6 HPOUT1L_MUTE_N=1, bits 5:0 VOL.
        // Vol encoding: 0 = -57 dB, 0x39 (57) = 0 dB, 0x3F (63) = +6 dB,
        // 1 dB per step. 0 dB matches direct-line-jack speaker levels
        // (bench-validated 2026-05-01: Mac → CN10 → sidetone → CN11
        // matches Mac → speaker direct).
        let hpout_vol: u16 = 0x100 | 0x40 | 0x39;
        self.write_reg(REG_LEFT_OUTPUT_VOL, hpout_vol)?;
        self.write_reg(REG_RIGHT_OUTPUT_VOL, hpout_vol)?;
        // R0x420 AIF1 DAC1 filters: clear soft-mute (default = muted).
        self.write_reg(REG_AIF1_DAC1_FILTER_1, 0x0000)?;
        // R0x610 / R0x611 DAC1 L/R volume: bit 9 DAC1L_MUTE=0
        // (un-mute), bit 8 DAC1_VU=1, bits 7:0 = 0xC0 = 0 dB.
        // Even with R0x420 cleared, the hardware DAC has its own
        // soft-mute; missing this kept CN11 silent in bench (2026-04-29).
        self.write_reg(REG_DAC1_LEFT_VOL, 0x01C0)?;
        self.write_reg(REG_DAC1_RIGHT_VOL, 0x01C0)?;
        // R0x402 / R0x403 AIF1 DAC1 L/R volume: 0 dB with VU strobe.
        self.write_reg(REG_AIF1_DAC1_LEFT_VOL, 0x01C0)?;
        self.write_reg(REG_AIF1_DAC1_RIGHT_VOL, 0x01C0)?;
        // R0x51 Class W envelope tracker (HP-specific). Required for
        // HPOUT1 to drive a load. See [`REG_CLASSW_HP`] doc-comment.
        self.write_reg(REG_CLASSW_HP, 0x0005)?;

        Ok(lock_outcome)
    }

    /// Begin the WM8994 HP output stage enable sequence per
    /// WM8994_Rev4.6.pdf Table 123, "Headphone Cold Start-Up Default
    /// Sequence" (indices 12–13).
    ///
    /// Writes R0x60 = 0x0022 (HPOUT1L_DLY + HPOUT1R_DLY) then
    /// R0x54 = 0x0033 (DC servo channel 0/1 enables + startup triggers).
    /// The caller MUST then wait ≥256.5 ms for the DC servo to settle
    /// before calling [`Self::finish_hp_output_stage`]. The wait isn't
    /// busy-looped here because it's on the caller's clock domain.
    ///
    /// Without this sequence + the wait, HPOUT1L/R remain in their reset
    /// state with the output stage gated off — silent at the jack even
    /// with charge pump, mixers, volumes, and mutes all configured.
    pub fn init_hp_output_stage(&mut self) -> Result<(), I2C::Error> {
        self.write_reg(REG_ANALOGUE_HP1, 0x0022)?;
        self.write_reg(REG_DC_SERVO_1, 0x0033)?;
        Ok(())
    }

    /// Complete the WM8994 HP output stage enable sequence per Table 123
    /// (index 15).
    ///
    /// Writes R0x60 = 0x00EE — HPOUT1L/R_DLY (preserved) + HPOUT1L/R_OUTP
    /// + HPOUT1L/R_RMV_SHORT — bringing the HP output drivers fully online.
    ///
    /// Caller MUST have run [`Self::init_hp_output_stage`] and waited
    /// ≥256.5 ms for DC-servo settle before calling this; writing the
    /// OUTP / RMV_SHORT bits before the servo settles produces an audible
    /// pop and can leave a DC offset on HPOUT1.
    pub fn finish_hp_output_stage(&mut self) -> Result<(), I2C::Error> {
        self.write_reg(REG_ANALOGUE_HP1, 0x00EE)?;
        Ok(())
    }

    /// Read back a fixed set of critical record-path registers and pack
    /// them into the caller's `out` slice. Each entry is laid out as
    /// `((reg_addr as u32) << 16) | (reg_value as u32)` so a debugger
    /// or telemetry decoder can recover the slot identity without a
    /// separate index map.
    ///
    /// Captures: PM1/2/4/5/6, line-in volumes, input mixers 2/3/4,
    /// AIF1/SYS clocking, AIF1 rate, AIF1 control 1, ADC L/R volumes,
    /// ADC filters, AIF1ADC1 L/R mixers, anti-pop, FLL1 control 1/2/4/5.
    /// `out` MUST be at least [`Self::READBACK_SLOT_COUNT`] entries; extras
    /// are left untouched.
    pub fn readback_critical_regs(&mut self, out: &mut [u32]) -> Result<(), I2C::Error> {
        const REGS: [u16; 24] = [
            REG_PWR_MGMT_1,           // 0:  R0x01
            REG_PWR_MGMT_2,           // 1:  R0x02
            REG_PWR_MGMT_4,           // 2:  R0x04
            REG_PWR_MGMT_5,           // 3:  R0x05
            REG_PWR_MGMT_6,           // 4:  R0x06
            REG_LEFT_LINE_IN1_2_VOL,  // 5:  R0x18
            REG_RIGHT_LINE_IN1_2_VOL, // 6:  R0x1A
            REG_INPUT_MIXER_2,        // 7:  R0x28
            REG_INPUT_MIXER_3,        // 8:  R0x29
            REG_INPUT_MIXER_4,        // 9:  R0x2A
            REG_AIF1_CLOCKING_1,      // 10: R0x200
            REG_CLOCKING_1,           // 11: R0x208
            REG_AIF1_RATE,            // 12: R0x210
            REG_AIF1_CONTROL_1,       // 13: R0x300
            REG_AIF1_ADC1_L_VOL,      // 14: R0x400
            REG_AIF1_ADC1_R_VOL,      // 15: R0x401
            REG_AIF1_ADC1_FILTERS,    // 16: R0x410
            REG_AIF1_ADC1L_MIXER,     // 17: R0x606
            REG_AIF1_ADC1R_MIXER,     // 18: R0x607
            REG_ANTIPOP_2,            // 19: R0x39
            REG_FLL1_CTRL_1,          // 20: R0x220 (FLL1_ENA + lock readback)
            REG_FLL1_CTRL_2,          // 21: R0x221 (OUTDIV / FRATIO)
            REG_FLL1_CTRL_4,          // 22: R0x223 (N)
            REG_FLL1_CTRL_5,          // 23: R0x224 (REFCLK_DIV / REFCLK_SRC)
        ];
        for (i, &reg) in REGS.iter().enumerate() {
            if i >= out.len() {
                break;
            }
            let val = self.read_reg(reg)?;
            out[i] = ((reg as u32) << 16) | (val as u32);
        }
        Ok(())
    }

    /// Number of u32 slots [`Self::readback_critical_regs`] populates.
    pub const READBACK_SLOT_COUNT: usize = 24;

    /// Read back the output / analog-path registers and pack them into
    /// `out` with the same layout used by [`Self::readback_critical_regs`].
    ///
    /// The diagnostic complement to `readback_critical_regs`: where that
    /// captures record-path state, this one captures the output side
    /// (PM3, HP/Line-out volumes, DAC volumes/mutes, charge pump,
    /// Class W, DC servo, analog HP enables). Used to diagnose
    /// "digital alive but analog silent".
    ///
    /// `out` MUST be at least [`Self::READBACK_OUTPUT_SLOT_COUNT`]
    /// entries; extras are left untouched.
    pub fn readback_output_regs(&mut self, out: &mut [u32]) -> Result<(), I2C::Error> {
        const REGS: [u16; 22] = [
            REG_CHIP_ID,             // 0:  R0x000 (sanity)
            REG_PWR_MGMT_1,          // 1:  R0x001
            REG_PWR_MGMT_3,          // 2:  R0x003
            REG_PWR_MGMT_5,          // 3:  R0x005
            REG_LEFT_OUTPUT_VOL,     // 4:  R0x01C
            REG_RIGHT_OUTPUT_VOL,    // 5:  R0x01D
            REG_LINEOUT_VOL,         // 6:  R0x01E
            REG_OUTPUT_MIXER_1,      // 7:  R0x02D
            REG_OUTPUT_MIXER_2,      // 8:  R0x02E
            REG_LINEOUT_MIXER_1,     // 9:  R0x033
            REG_LINEOUT_MIXER_2,     // 10: R0x034
            REG_ANTIPOP_2,           // 11: R0x039
            REG_CHARGE_PUMP_1,       // 12: R0x04C
            REG_CLASSW_HP,           // 13: R0x051
            REG_DC_SERVO_1,          // 14: R0x054
            REG_DC_SERVO_2,          // 15: R0x055
            REG_ANALOGUE_HP1,        // 16: R0x060
            REG_AIF1_DAC1_LEFT_VOL,  // 17: R0x402
            REG_AIF1_DAC1_RIGHT_VOL, // 18: R0x403
            REG_AIF1_DAC1_FILTER_1,  // 19: R0x420
            REG_DAC1L_MIXER,         // 20: R0x601
            REG_DAC1R_MIXER,         // 21: R0x602 (DAC1R routing — was
                                     //              mis-addressed as
                                     //              R0x603 prior to the
                                     //              2026-05-01 fix)
        ];
        for (i, &reg) in REGS.iter().enumerate() {
            if i >= out.len() {
                break;
            }
            let val = self.read_reg(reg)?;
            out[i] = ((reg as u32) << 16) | (val as u32);
        }
        Ok(())
    }

    /// Number of u32 slots [`Self::readback_output_regs`] populates.
    pub const READBACK_OUTPUT_SLOT_COUNT: usize = 22;

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Configure FLL1 to generate exact audio clocks from the given MCLK input.
    ///
    /// The FLL output frequency is `Fout = Fs * 256` (e.g. 12.288 MHz for 48 kHz).
    /// FLL reference is MCLK1 (from SAI1_MCLK_A).
    fn configure_fll1(&mut self, sample_rate: u32, mclk_hz: u32) -> Result<(), I2C::Error> {
        // Target FLL output: Fs * 256
        let fll_output = sample_rate * 256;

        // FLL1_CTRL_1: disable FLL first
        self.write_reg(REG_FLL1_CTRL_1, 0x0000)?;

        // Compute FLL parameters:
        //   Fout = (Fref * N.K) / FLL_FRATIO
        //   where N = integer part, K = fractional part (16-bit)
        //   FLL_FRATIO divides reference before comparison
        //
        // Choose FLL_FRATIO based on reference frequency:
        //   Fref < 1 MHz: FRATIO = 8 (0b011)
        //   1 MHz <= Fref < 13.5 MHz: FRATIO = 1 (0b000)
        //   >= 13.5 MHz: use OUTDIV to bring it down
        let (fll_fratio, fratio_bits) = if mclk_hz < 1_000_000 {
            (8u32, 0b011u16)
        } else {
            (1u32, 0b000u16)
        };

        // N.K = Fout * FLL_FRATIO / Fref
        // We compute N and K separately to avoid overflow:
        //   N = integer part
        //   K = fractional part scaled to 16 bits = (remainder * 65536) / Fref
        let nk_x = (fll_output as u64) * (fll_fratio as u64);
        let n = (nk_x / mclk_hz as u64) as u16;
        let remainder = (nk_x % mclk_hz as u64) as u64;
        let k = ((remainder * 65536) / mclk_hz as u64) as u16;

        // FLL1_CTRL_2: N value, prescaler off
        self.write_reg(REG_FLL1_CTRL_2, n & 0x03FF)?;

        // FLL1_CTRL_3: K value (fractional)
        self.write_reg(REG_FLL1_CTRL_3, k)?;

        // FLL1_CTRL_4: FLL_FRATIO
        self.write_reg(REG_FLL1_CTRL_4, fratio_bits << 0)?;

        // FLL1_CTRL_5: FLL_REFCLK_SRC = MCLK1 (0b00), FLL_REFCLK_DIV = /1
        self.write_reg(REG_FLL1_CTRL_5, 0x0000)?;

        // FLL1_CTRL_1: enable FLL, FRACN_ENA if K != 0
        let ctrl1 = 0x0001 | if k != 0 { 0x0004 } else { 0x0000 };
        self.write_reg(REG_FLL1_CTRL_1, ctrl1)?;

        // Wait for FLL lock (~5ms)
        for _ in 0..2_000_000u32 {
            core::hint::black_box(0u32);
        }

        Ok(())
    }

    /// Enable FLL1 (in the BCLK1-referenced configuration used by
    /// [`Self::init_record`]) and poll R0x732 bit 5 for lock
    /// acquisition with timeout + retry.
    ///
    /// Implements `docs/audio/01-codec-bringup.md` AUDIO-01 §9
    /// INV-AUDIO-01-1 (poll FLL1_LOCK_STS level status) and
    /// INV-AUDIO-01-2 (100 ms per-attempt timeout, 3 max retries,
    /// 1 ms poll interval). On the cold-boot path where FLL1 takes
    /// 30–50 ms to acquire lock against a power-up-transient BCLK1
    /// reference (WM8994 p.202), the original static 20 ms delay
    /// after `FLL1_ENA = 1` is insufficient — the post-FLL register
    /// writes in `init_record` then arm the AIF1ADC1 serializer
    /// against an unstable SYSCLK and latch a wrong bit-clock phase
    /// (AUDIO-01 §2 documents the symptom shapes — 0x0000/0x8000
    /// bimodal on one slot, or 11+5 bit-split, depending on the
    /// metastable arm).
    ///
    /// Polls **R0x732 bit 5 (FLL1_LOCK_STS, level/raw status)** rather
    /// than R0x731 bit 5 (FLL1_LOCK_EINT, the edge-latched event).
    /// The initial AUDIO-01 ratification specified the edge latch,
    /// but bench validation 2026-05-19 found the edge latch never
    /// fires under our init_record sequence (post-software-reset
    /// FLL is disabled; FLL_ENA=1 transition should latch but the
    /// clear-write at the start of each attempt was racing with the
    /// hardware in a way that lost the event). Level status has no
    /// transition-timing window. AUDIO-01 §15 amendment
    /// `2026-05-19-b` documents the correction.
    ///
    /// Precondition: caller has already programmed R0x224, R0x223,
    /// R0x222, R0x221 (FLL config; F_REF = BCLK1, N = 64, K = 0,
    /// OUTDIV = 8) per the values fixed in
    /// [`Self::init_record`]'s body just above this call.
    ///
    /// Returns [`FllLockOutcome`] per AUDIO-01 §6. On `Failed` the
    /// caller MUST return early from `init_record` without arming
    /// the AIF1ADC1 serializer.
    fn fll1_enable_and_wait_lock_bclk1(&mut self) -> Result<FllLockOutcome, I2C::Error> {
        /// Per-attempt lock-poll timeout, milliseconds. AUDIO-01 §9
        /// INV-AUDIO-01-2.
        const PER_ATTEMPT_TIMEOUT_MS: u32 = 100;
        /// Inter-poll interval, milliseconds. AUDIO-01 §9 INV-AUDIO-01-2.
        const POLL_INTERVAL_MS: u32 = 1;
        /// FLL1 re-arm wait between FLL1_ENA = 0 and the next FLL
        /// config + FLL1_ENA = 1, milliseconds.
        const RE_ARM_WAIT_MS: u32 = 5;
        /// Maximum retry count after the initial attempt. AUDIO-01 §9
        /// INV-AUDIO-01-2.
        const MAX_RETRIES: u8 = 3;

        let poll_iters = PER_ATTEMPT_TIMEOUT_MS / POLL_INTERVAL_MS;

        for attempt in 0..=MAX_RETRIES {
            // Step 1: on retry, FLL1 was running but failed to lock.
            // Disable it, wait briefly to let the FLL block settle,
            // then re-write the FLL config registers (values mirror
            // what init_record programmed just before calling this
            // helper) and re-enable.
            if attempt > 0 {
                self.write_reg(REG_FLL1_CTRL_1, 0x0000)?; // FLL1_ENA = 0
                delay_busy(RE_ARM_WAIT_MS * 1_000);
                self.write_reg(REG_FLL1_CTRL_5, 0x0003)?; // REFCLK_SRC=BCLK1, REFCLK_DIV=÷1
                self.write_reg(REG_FLL1_CTRL_4, 0x0800)?; // N=64
                self.write_reg(REG_FLL1_CTRL_3, 0x0000)?; // K=0
                self.write_reg(REG_FLL1_CTRL_2, 7u16 << 8)?; // OUTDIV-1=7, FRATIO=0
            }
            // FLL1_ENA = 1 (re-arm on retry, initial arm on attempt 0).
            self.write_reg(REG_FLL1_CTRL_1, 0x0001)?;

            // Step 2: poll R0x732 bit 5 (FLL1_LOCK_STS, level) every
            // POLL_INTERVAL_MS until set, up to PER_ATTEMPT_TIMEOUT_MS
            // total. Level polling needs no clear-write — a read of
            // bit 5 = 1 simply means "FLL1 is locked right now."
            for _ in 0..poll_iters {
                delay_busy(POLL_INTERVAL_MS * 1_000);
                let status = self.read_reg(REG_FLL1_LOCK_STS)?;
                if status & FLL1_LOCK_STS_BIT != 0 {
                    return Ok(if attempt == 0 {
                        FllLockOutcome::LockedFirstTry
                    } else {
                        FllLockOutcome::LockedAfterRetry { attempts: attempt }
                    });
                }
            }
            // Per-attempt timeout reached without lock. Loop continues
            // to next retry; on attempt == MAX_RETRIES we fall through
            // and return Failed.
        }

        Ok(FllLockOutcome::Failed)
    }

    /// Configure output routing and power for the selected output device.
    fn configure_output(&mut self, output: OutputDevice) -> Result<(), I2C::Error> {
        match output {
            OutputDevice::Headphone | OutputDevice::Both => {
                // Power management: enable HPOUT1L, HPOUT1R
                let pwr1 = self.read_reg(REG_PWR_MGMT_1)?;
                self.write_reg(REG_PWR_MGMT_1, pwr1 | 0x0300)?; // HPOUT1L_ENA, HPOUT1R_ENA

                // Power management 3: enable left/right output mixer
                self.write_reg(REG_PWR_MGMT_3, 0x0030)?; // MIXOUTL_ENA, MIXOUTR_ENA

                // Output mixer 1/2: DAC1L -> left mixer, DAC1R -> right mixer
                self.write_reg(REG_OUTPUT_MIXER_1, 0x0001)?; // DAC1L_TO_MIXOUTL
                self.write_reg(REG_OUTPUT_MIXER_2, 0x0001)?; // DAC1R_TO_MIXOUTR

                // Headphone volume: ~-10 dB (comfortable default)
                self.set_headphone_volume(50)?;

                // Class W for power-efficient headphone drive
                self.write_reg(REG_CLASS_W, 0x0005)?;
            }
            _ => {}
        }

        match output {
            OutputDevice::Speaker | OutputDevice::Both => {
                // Power management 1: enable SPKOUTL, SPKOUTR
                let pwr1 = self.read_reg(REG_PWR_MGMT_1)?;
                self.write_reg(REG_PWR_MGMT_1, pwr1 | 0x3000)?; // SPKOUTL_ENA, SPKOUTR_ENA

                // Power management 3: enable speaker mixers
                let pwr3 = self.read_reg(REG_PWR_MGMT_3)?;
                self.write_reg(REG_PWR_MGMT_3, pwr3 | 0x0300)?; // SPKLVOL_ENA, SPKRVOL_ENA

                // Speaker mixer attenuation
                self.write_reg(REG_SPKMIXL_ATT, 0x0001)?; // DAC1L_TO_SPKMIXL
                self.write_reg(REG_SPKMIXR_ATT, 0x0001)?; // DAC1R_TO_SPKMIXR

                // Speaker output mixers: SPKMIXL -> SPKOUTL, SPKMIXR -> SPKOUTR
                self.write_reg(REG_SPKOUT_MIXERS, 0x0018)?;

                // Speaker volume
                self.set_speaker_volume(50)?;
            }
            _ => {}
        }

        // Power management 4: enable AIF1ADC1L, AIF1ADC1R (for DAC routing)
        self.write_reg(REG_PWR_MGMT_4, 0x0300)?;

        // Power management 5: enable DAC1L, DAC1R, AIF1DAC1L, AIF1DAC1R
        self.write_reg(REG_PWR_MGMT_5, 0x0303)?;

        // Power management 6: AIF1DAC1L -> DAC1L, AIF1DAC1R -> DAC1R
        self.write_reg(REG_PWR_MGMT_6, 0x000C)?;

        Ok(())
    }

    /// Write a 16-bit value to a 16-bit register address.
    ///
    /// Public for diagnostic / bench-bring-up use. The init_playback /
    /// init_record sequences cover the canonical register choreography;
    /// callers reaching for write_reg directly are typically iterating
    /// register experiments before promoting them into a structured init
    /// path.
    pub fn write_reg(&mut self, reg: u16, val: u16) -> Result<(), I2C::Error> {
        let buf = [
            (reg >> 8) as u8,
            (reg & 0xFF) as u8,
            (val >> 8) as u8,
            (val & 0xFF) as u8,
        ];
        self.i2c.write(Self::ADDRESS, &buf)
    }

    /// Read a 16-bit value from a 16-bit register address.
    ///
    /// Public for diagnostic / readback use. Prefer
    /// [`Self::readback_critical_regs`] / [`Self::readback_output_regs`]
    /// for the canonical record-path / output-path register snapshots —
    /// they pack the data into a `(reg << 16) | val` SRAM4-friendly
    /// layout.
    pub fn read_reg(&mut self, reg: u16) -> Result<u16, I2C::Error> {
        let addr = [(reg >> 8) as u8, (reg & 0xFF) as u8];
        let mut buf = [0u8; 2];
        self.i2c.write_read(Self::ADDRESS, &addr, &mut buf)?;
        Ok(((buf[0] as u16) << 8) | buf[1] as u16)
    }
}
