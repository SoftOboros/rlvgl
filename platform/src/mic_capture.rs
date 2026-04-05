//! High-level microphone capture API for the STM32H747I-DISCO onboard MEMS mic.
//!
//! Orchestrates SAI4 PDM + BDMA + CIC decimation filter to provide a simple
//! poll-based interface for reading PCM audio samples from the MP34DT05-A
//! digital microphone.
//!
//! # Memory requirements
//!
//! The PDM DMA buffers **must** reside in SRAM4 (`0x3800_0000`).  The caller
//! is responsible for providing suitably placed buffers.  SRAM4 is 64 KB and
//! not cached by the CM7 D-Cache, so no cache maintenance is needed.
//!
//! # Usage
//!
//! ```ignore
//! let mut mic = MicCapture::new();
//! mic.init(&mut pdm_buf0, &mut pdm_buf1, 64);
//! mic.start();
//! loop {
//!     if let Some(count) = mic.poll(&mut pcm_out) {
//!         // pcm_out[..count] contains 16-bit PCM samples
//!     }
//! }
//! mic.stop();
//! ```

use crate::bdma::BdmaSai4Rx;
use crate::pdm_filter::CicFilter;
use crate::sai4_pdm::Sai4Pdm;

/// Microphone capture state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Ready,
    Recording,
}

/// High-level microphone capture driver.
pub struct MicCapture {
    sai: Sai4Pdm,
    bdma: BdmaSai4Rx,
    filter: CicFilter,
    state: State,
    /// Which half-buffer was last processed (0 or 1).
    last_processed: u8,
    /// Number of 16-bit PDM halfwords per buffer.
    pdm_buf_len: usize,
}

impl MicCapture {
    /// Create a new mic capture instance (unconfigured).
    pub fn new() -> Self {
        Self {
            sai: Sai4Pdm::new(),
            bdma: BdmaSai4Rx::new(),
            filter: CicFilter::new(64),
            state: State::Idle,
            last_processed: 1, // so first poll processes buffer 0
            pdm_buf_len: 0,
        }
    }

    /// Initialize the capture pipeline.
    ///
    /// - `pdm_buf0`, `pdm_buf1`: two equally-sized buffers in SRAM4 for PDM DMA
    /// - `decimation`: CIC filter decimation ratio (64 or 128)
    /// - `sai_clock_source`: kernel clock source for SAI4 (0=PLL1_Q, 1=PLL2_P, etc.)
    /// - `mckdiv`: SAI4 master clock divider
    ///
    /// After `init()`, call `start()` to begin recording.
    pub fn init(
        &mut self,
        pdm_buf0: &mut [u16],
        pdm_buf1: &mut [u16],
        decimation: u32,
        sai_clock_source: u8,
        mckdiv: u8,
    ) {
        assert_eq!(pdm_buf0.len(), pdm_buf1.len());
        self.pdm_buf_len = pdm_buf0.len();

        // Enable clocks
        self.sai.enable_clock(sai_clock_source);
        self.bdma.enable_clock();

        // Configure SAI4 PDM
        self.sai.configure(mckdiv);

        // Configure BDMA
        let buf_bytes = pdm_buf0.len() * 2;
        self.bdma.configure(
            self.sai.data_register_addr(),
            pdm_buf0.as_mut_ptr(),
            pdm_buf1.as_mut_ptr(),
            buf_bytes,
        );

        // Set up CIC filter
        self.filter = CicFilter::new(decimation);

        self.state = State::Ready;
        self.last_processed = 1;
    }

    /// Start recording.
    pub fn start(&mut self) {
        if self.state != State::Ready && self.state != State::Recording {
            return;
        }

        self.filter.reset();
        self.last_processed = 1;

        // Enable DMA on SAI4
        self.sai.enable_dma();

        // Start BDMA first, then SAI
        self.bdma.start();
        self.sai.enable();

        self.state = State::Recording;
    }

    /// Stop recording.
    pub fn stop(&mut self) {
        self.sai.disable();
        self.sai.disable_dma();
        self.bdma.stop();
        self.state = State::Ready;
    }

    /// Poll for new PCM data.
    ///
    /// When a PDM buffer has been filled by DMA, this method runs it through
    /// the CIC decimation filter and writes PCM samples to `pcm_out`.
    ///
    /// Returns `Some(n)` with the number of PCM samples written, or `None`
    /// if no new data is available yet.
    ///
    /// The `pdm_buf` parameter must be a reference to whichever SRAM4 buffer
    /// was just completed by DMA.  In practice, pass the buffer that
    /// corresponds to the *non-active* DMA target.
    pub fn poll_with_buffer(&mut self, pdm_buf: &[u16], pcm_out: &mut [i16]) -> Option<usize> {
        if self.state != State::Recording {
            return None;
        }

        // Check for transfer-complete (full buffer done)
        let tc = self.bdma.transfer_complete();
        let ht = self.bdma.half_transfer();

        if !tc && !ht {
            return None;
        }

        if tc {
            self.bdma.clear_transfer_complete();
        }
        if ht {
            self.bdma.clear_half_transfer();
        }

        // Clear any overrun
        self.sai.clear_overrun();

        // Run PDM data through CIC filter
        let count = self.filter.process(pdm_buf, pcm_out);
        Some(count)
    }

    /// Simple poll that checks DMA flags and returns which buffer index (0 or 1)
    /// is ready for processing, or `None` if neither is ready.
    pub fn poll_ready(&mut self) -> Option<u8> {
        if self.state != State::Recording {
            return None;
        }

        let tc = self.bdma.transfer_complete();
        let ht = self.bdma.half_transfer();

        if tc {
            self.bdma.clear_transfer_complete();
            self.sai.clear_overrun();
            // TC means DMA just finished one full buffer and switched.
            // The buffer that just completed is the opposite of current_target.
            let completed = if self.bdma.current_target() == 0 { 1 } else { 0 };
            if completed != self.last_processed {
                self.last_processed = completed;
                return Some(completed);
            }
        }

        if ht {
            self.bdma.clear_half_transfer();
            // Half-transfer: first half of current buffer is done.
            // For double-buffer mode, this is less commonly used but we
            // handle it for completeness.
        }

        None
    }

    /// Returns a reference to the CIC filter for direct access.
    pub fn filter(&mut self) -> &mut CicFilter {
        &mut self.filter
    }

    /// Returns `true` if currently recording.
    pub fn is_recording(&self) -> bool {
        self.state == State::Recording
    }
}
