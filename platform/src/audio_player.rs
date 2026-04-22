//! Audio player state machine for WAV file playback via SAI1 DMA.
//!
//! The player manages DMA double-buffering and tells the caller when a buffer
//! needs to be refilled with PCM data from the SD card.  This keeps the player
//! independent of the filesystem implementation.

use crate::dma_sai::DmaSai1Tx;
use crate::sai::Sai1Audio;
use crate::wav::WavHeader;

/// Result of an [`AudioPlayer::poll`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollResult {
    /// No playback in progress.
    Idle,
    /// Playback is running; no action needed this iteration.
    Playing,
    /// A buffer needs to be refilled by the caller.
    ///
    /// The caller should:
    /// 1. Read up to `max_bytes` of PCM data from the file at `file_offset`
    /// 2. Write the data into the returned buffer pointer
    /// 3. If fewer bytes are available (end of file), zero-fill the remainder
    /// 4. Call [`AudioPlayer::refill_done`] with the number of PCM bytes written
    NeedRefill {
        /// Pointer to the buffer that needs filling.
        buf: *mut u8,
        /// Byte offset into the WAV PCM data to read from.
        file_offset: u32,
        /// Maximum number of bytes to read (buffer capacity).
        max_bytes: usize,
    },
    /// Playback has finished (end of file reached and final buffer drained).
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayState {
    Idle,
    /// Both buffers are pre-filled; waiting for `start()`.
    Ready,
    /// DMA is running.
    Playing,
    /// Final partial buffer is draining; stop after DMA leaves it.
    Draining,
}

/// WAV audio player with DMA double-buffering.
///
/// # Usage
///
/// ```ignore
/// let mut player = AudioPlayer::new(buf0_ptr, buf1_ptr, 4096);
///
/// // Pre-fill: caller reads PCM data into buf0 and buf1, then:
/// player.prepare(&wav_header);
///
/// // In the main loop:
/// match player.poll() {
///     PollResult::NeedRefill { buf, file_offset, max_bytes } => {
///         // read from SD card into `buf`
///         player.refill_done(bytes_written);
///     }
///     PollResult::Finished => player.stop(&sai),
///     _ => {}
/// }
/// ```
pub struct AudioPlayer {
    dma: DmaSai1Tx,
    buf0: *mut u8,
    buf1: *mut u8,
    buf_bytes: usize,
    state: PlayState,
    /// Byte offset of the PCM data chunk within the WAV file.
    data_file_offset: u32,
    /// Total PCM data length in bytes.
    data_total: u32,
    /// How many PCM bytes have been queued for playback so far.
    bytes_queued: u32,
    /// The last DMA current_target we observed (0 or 1).
    last_target: u8,
}

impl AudioPlayer {
    /// Create a new audio player.
    ///
    /// - `buf0`, `buf1`: pointers to two equal-sized audio buffers (should be
    ///   in SDRAM, 32-byte aligned)
    /// - `buf_bytes`: size of each buffer in bytes (must be even)
    pub fn new(buf0: *mut u8, buf1: *mut u8, buf_bytes: usize) -> Self {
        Self {
            dma: DmaSai1Tx::new(),
            buf0,
            buf1,
            buf_bytes,
            state: PlayState::Idle,
            data_file_offset: 0,
            data_total: 0,
            bytes_queued: 0,
            last_target: 0,
        }
    }

    /// Prepare for playback of a WAV file.
    ///
    /// The caller must have already filled both `buf0` and `buf1` with the
    /// first `2 * buf_bytes` of PCM data before calling this.
    ///
    /// Returns `false` if the WAV format is unsupported.
    pub fn prepare(&mut self, header: &WavHeader) -> bool {
        // Phase 1: only support 48 kHz, 16-bit, stereo
        if header.sample_rate != 48_000 || header.bits_per_sample != 16 || header.num_channels != 2
        {
            return false;
        }

        self.data_file_offset = header.data_offset;
        self.data_total = header.data_length;
        // Both buffers are pre-filled
        self.bytes_queued = core::cmp::min((self.buf_bytes as u32) * 2, header.data_length);
        self.last_target = 0;
        self.state = PlayState::Ready;
        true
    }

    /// Start DMA playback.
    ///
    /// Call after `prepare()` and pre-filling both buffers.
    pub fn start(&mut self, sai: &Sai1Audio) {
        if self.state != PlayState::Ready {
            return;
        }

        // Enable DMA1 clock
        self.dma.enable_clock();

        // Configure DMA: SAI1_A DR ← double-buffer
        let periph_addr = sai.tx_data_register_addr();
        self.dma
            .configure(periph_addr, self.buf0, self.buf1, self.buf_bytes);

        // Enable SAI1 DMA requests
        sai.enable_dma_tx();

        // Start DMA
        self.dma.start();

        self.state = PlayState::Playing;
    }

    /// Poll the player.  Call once per main loop iteration.
    ///
    /// Returns a [`PollResult`] indicating what the caller should do.
    pub fn poll(&mut self) -> PollResult {
        match self.state {
            PlayState::Idle => PollResult::Idle,
            PlayState::Ready => PollResult::Idle, // not started yet
            PlayState::Playing => self.poll_playing(),
            PlayState::Draining => self.poll_draining(),
        }
    }

    /// Acknowledge that a buffer refill has been completed.
    ///
    /// `pcm_bytes` is the number of valid PCM bytes written (may be less than
    /// `max_bytes` at end of file — the caller should zero-fill the rest).
    pub fn refill_done(&mut self, pcm_bytes: usize) {
        self.bytes_queued += pcm_bytes as u32;

        // Clean D-Cache for the buffer we just wrote into so DMA sees fresh data.
        // The inactive buffer is whichever one DMA is NOT currently reading.
        let target = self.dma.current_target();
        let buf = if target == 0 { self.buf1 } else { self.buf0 };
        clean_dcache(buf, self.buf_bytes);

        // If we've queued all the data, enter draining state
        if self.bytes_queued >= self.data_total {
            self.state = PlayState::Draining;
        }
    }

    /// Stop playback immediately.
    pub fn stop(&mut self, sai: &Sai1Audio) {
        self.dma.stop();
        sai.disable_dma_tx();
        self.state = PlayState::Idle;
    }

    /// Returns `true` if playback is active.
    pub fn is_playing(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Draining)
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    fn poll_playing(&mut self) -> PollResult {
        // Check if DMA has switched buffers (transfer complete)
        if !self.dma.transfer_complete() {
            return PollResult::Playing;
        }
        self.dma.clear_transfer_complete();

        // DMA switched targets — the buffer it just finished is now free
        let current = self.dma.current_target();
        // The free buffer is the one DMA just left (opposite of current)
        let (free_buf, _free_idx) = if current == 1 {
            (self.buf0, 0u8)
        } else {
            (self.buf1, 1u8)
        };
        self.last_target = current;

        // Calculate how much data remains to be read from the file
        let remaining = self.data_total.saturating_sub(self.bytes_queued);
        if remaining == 0 {
            // All data has been queued; enter draining
            self.state = PlayState::Draining;
            return PollResult::Playing;
        }

        let max_bytes = core::cmp::min(remaining as usize, self.buf_bytes);
        let file_offset = self.data_file_offset + self.bytes_queued;

        PollResult::NeedRefill {
            buf: free_buf,
            file_offset,
            max_bytes,
        }
    }

    fn poll_draining(&mut self) -> PollResult {
        // Wait for one more transfer complete — the final buffer has been played
        if self.dma.transfer_complete() {
            self.dma.clear_transfer_complete();
            self.state = PlayState::Idle;
            return PollResult::Finished;
        }
        PollResult::Playing
    }
}

/// Clean D-Cache lines covering the given memory range.
///
/// Writes successive cache-line addresses to SCB DCCMVAC at `0xE000_EF68`.
/// The raw register write is intentionally kept here — the `cortex_m`
/// crate's `SCB::clean_dcache_by_address` requires plumbing a `&mut SCB`
/// instance through the audio player constructor, which is more
/// invasive than the discipline opt-out for a single cache-control
/// register. The opt-out marker on the const declaration documents
/// the deferral.
fn clean_dcache(ptr: *const u8, len: usize) {
    // SCB DCCMVAC (Clean D-Cache by MVA to PoC): 0xE000_EF68
    const DCCMVAC: *mut u32 = 0xE000_EF68 as *mut u32; // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    const CACHE_LINE: usize = 32;
    let start = (ptr as usize) & !(CACHE_LINE - 1);
    let end = ((ptr as usize) + len + CACHE_LINE - 1) & !(CACHE_LINE - 1);
    let mut addr = start;
    while addr < end {
        unsafe {
            DCCMVAC.write_volatile(addr as u32);
        }
        addr += CACHE_LINE;
    }
    // Data synchronization barrier
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}
