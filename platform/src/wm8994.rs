//! Minimal driver for the WM8994 audio codec on the STM32H747I-DISCO board.
//!
//! Communicates over I2C using 16-bit register addresses and 16-bit data values.
//! The codec is controlled via AIF1 (Audio Interface 1) connected to SAI1 on the
//! Discovery board.  Headphone and speaker output paths are supported.

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

// Clocking
const REG_AIF1_CLOCKING_1: u16 = 0x0200;
const REG_AIF1_CLOCKING_2: u16 = 0x0201;
const REG_CLOCKING_1: u16 = 0x0208;
const REG_CLOCKING_2: u16 = 0x0210;
const REG_AIF1_RATE: u16 = 0x0211;

// FLL1 (Frequency Locked Loop)
const REG_FLL1_CTRL_1: u16 = 0x0220;
const REG_FLL1_CTRL_2: u16 = 0x0221;
const REG_FLL1_CTRL_3: u16 = 0x0222;
const REG_FLL1_CTRL_4: u16 = 0x0223;
const REG_FLL1_CTRL_5: u16 = 0x0224;

// AIF1 (Audio Interface 1) control
const REG_AIF1_CONTROL_1: u16 = 0x0300;
const REG_AIF1_CONTROL_2: u16 = 0x0301;
const REG_AIF1_MASTER_SLAVE: u16 = 0x0302;

// AIF1 DAC/ADC path
const REG_AIF1_DAC1_FILTER_1: u16 = 0x0420;
const REG_AIF1_DAC1_LEFT_VOL: u16 = 0x0402;
const REG_AIF1_DAC1_RIGHT_VOL: u16 = 0x0403;

// DAC
const REG_DAC1_LEFT_VOL: u16 = 0x0610;
const REG_DAC1_RIGHT_VOL: u16 = 0x0611;

// Output mixer
const REG_OUTPUT_MIXER_1: u16 = 0x002D;
const REG_OUTPUT_MIXER_2: u16 = 0x002E;

// Headphone
const REG_LEFT_OUTPUT_VOL: u16 = 0x001C;
const REG_RIGHT_OUTPUT_VOL: u16 = 0x001D;

// Speaker
const REG_SPEAKER_VOL_LEFT: u16 = 0x0026;
const REG_SPEAKER_VOL_RIGHT: u16 = 0x0027;
const REG_SPKMIXL_ATT: u16 = 0x0022;
const REG_SPKMIXR_ATT: u16 = 0x0023;
const REG_SPKOUT_MIXERS: u16 = 0x0024;
const REG_CLASS_W: u16 = 0x0025;

// Charge pump
const REG_CHARGE_PUMP_1: u16 = 0x004C;

// Anti-pop
const REG_ANTIPOP_2: u16 = 0x0039;

// Software reset
const REG_SW_RESET: u16 = 0x0000;

// Expected chip ID value
const WM8994_ID: u16 = 0x8994;

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

        // 7. System clocking: AIF1CLK -> SYSCLK
        self.write_reg(REG_CLOCKING_1, 0x000A)?; // SYSCLK_SRC = AIF1CLK, AIF1CLK / 1

        // 8. AIF1 sample rate
        let rate_bits = match sample_rate {
            8000 => 0x00,
            11025 => 0x01,
            12000 => 0x01,
            16000 => 0x02,
            22050 => 0x03,
            24000 => 0x03,
            32000 => 0x04,
            44100 => 0x05,
            48000 => 0x06,
            88200 => 0x07,
            96000 => 0x07,
            _ => 0x06, // default 48 kHz
        };
        self.write_reg(REG_AIF1_RATE, (rate_bits << 4) | 0x0001)?; // AIF1CLK_RATE=256Fs

        // 9. AIF1 control: I2S format, 16-bit word length
        self.write_reg(REG_AIF1_CONTROL_1, 0x4010)?; // FMT=I2S, WL=16-bit

        // 10. AIF1 master/slave: slave mode (SAI1 is master)
        self.write_reg(REG_AIF1_MASTER_SLAVE, 0x0000)?;

        // 11. AIF1 clocking 2: divide by 1
        self.write_reg(REG_AIF1_CLOCKING_2, 0x0000)?;

        // 12. Clocking 2: enable AIF1DAC, AIF1ADC clocks
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
    fn write_reg(&mut self, reg: u16, val: u16) -> Result<(), I2C::Error> {
        let buf = [
            (reg >> 8) as u8,
            (reg & 0xFF) as u8,
            (val >> 8) as u8,
            (val & 0xFF) as u8,
        ];
        self.i2c.write(Self::ADDRESS, &buf)
    }

    /// Read a 16-bit value from a 16-bit register address.
    fn read_reg(&mut self, reg: u16) -> Result<u16, I2C::Error> {
        let addr = [(reg >> 8) as u8, (reg & 0xFF) as u8];
        let mut buf = [0u8; 2];
        self.i2c.write_read(Self::ADDRESS, &addr, &mut buf)?;
        Ok(((buf[0] as u16) << 8) | buf[1] as u16)
    }
}
