//! Software CIC (Cascaded Integrator-Comb) decimation filter for PDM-to-PCM
//! conversion.
//!
//! Implements a 3rd-order sinc filter (sinc³) that converts a PDM bitstream
//! from the MP34DT05-A MEMS microphone into 16-bit signed PCM audio samples.
//!
//! The PDM data arrives packed into 16-bit halfwords (MSB first) from the SAI4
//! PDM interface.  Each bit represents a single 1-bit sample at the PDM clock
//! rate.  The CIC filter decimates by a configurable ratio (typically 64 or 128)
//! to produce output PCM at standard audio sample rates.

/// 3rd-order CIC decimation filter state.
pub struct CicFilter {
    /// Decimation ratio (number of PDM bits per output sample).
    decimation: u32,
    /// Integrator accumulators (3 stages).
    integrators: [i32; 3],
    /// Comb delay registers (3 stages).
    combs: [i32; 3],
    /// Bit counter within current decimation period.
    bit_count: u32,
    /// Right-shift to normalize CIC output to 16-bit range.
    /// For sinc³ with decimation R, gain = R³, so shift = 3*log2(R) - 15.
    output_shift: u32,
}

impl CicFilter {
    /// Create a new CIC filter with the given decimation ratio.
    ///
    /// Typical values:
    /// - `decimation = 64` → output rate = PDM_CLK / 64 (e.g., 2.048 MHz → 32 kHz)
    /// - `decimation = 128` → output rate = PDM_CLK / 128
    pub fn new(decimation: u32) -> Self {
        // sinc³ gain = R³.  We want to map the output to i16 range.
        // Shift = 3 * log2(R) - 15 (for signed 16-bit output).
        let log2_r = 31 - decimation.leading_zeros(); // approximate log2
        let shift = (3 * log2_r).saturating_sub(15);

        Self {
            decimation,
            integrators: [0; 3],
            combs: [0; 3],
            bit_count: 0,
            output_shift: shift,
        }
    }

    /// Reset the filter state (call when starting a new capture).
    pub fn reset(&mut self) {
        self.integrators = [0; 3];
        self.combs = [0; 3];
        self.bit_count = 0;
    }

    /// Process a buffer of raw PDM data (16-bit packed, MSB first) and write
    /// decimated PCM samples to the output buffer.
    ///
    /// Returns the number of PCM samples written to `pcm_out`.
    ///
    /// The caller must ensure `pcm_out` is large enough:
    /// at most `(pdm_data.len() * 16) / decimation + 1` samples.
    pub fn process(&mut self, pdm_data: &[u16], pcm_out: &mut [i16]) -> usize {
        let mut out_idx = 0;
        let decimation = self.decimation;
        let shift = self.output_shift;

        for &halfword in pdm_data {
            // Unpack 16 bits, MSB first
            for bit_pos in (0..16).rev() {
                let bit = (halfword >> bit_pos) & 1;
                // Map PDM bit: 0 → -1, 1 → +1
                let sample: i32 = if bit != 0 { 1 } else { -1 };

                // Integrator stages (running sums)
                self.integrators[0] = self.integrators[0].wrapping_add(sample);
                self.integrators[1] = self.integrators[1].wrapping_add(self.integrators[0]);
                self.integrators[2] = self.integrators[2].wrapping_add(self.integrators[1]);

                self.bit_count += 1;

                if self.bit_count >= decimation {
                    self.bit_count = 0;

                    // Comb stages (differentiators)
                    let mut val = self.integrators[2];
                    for stage in 0..3 {
                        let prev = self.combs[stage];
                        self.combs[stage] = val;
                        val = val.wrapping_sub(prev);
                    }

                    // Normalize to i16 range
                    let pcm = (val >> shift).clamp(-32768, 32767) as i16;

                    if out_idx < pcm_out.len() {
                        pcm_out[out_idx] = pcm;
                        out_idx += 1;
                    }
                }
            }
        }

        out_idx
    }

    /// Returns the decimation ratio.
    pub fn decimation(&self) -> u32 {
        self.decimation
    }
}
