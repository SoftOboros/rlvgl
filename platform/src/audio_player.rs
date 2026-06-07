//! Audio player state machine for WAV file playback via SAI1 DMA.
//!
//! The player manages DMA double-buffering and tells the caller when a buffer
//! needs to be refilled with PCM data from the SD card.  This keeps the player
//! independent of the filesystem implementation.
//!
//! ## DCB integration (DCB-02b, 2026-05-02)
//!
//! The player is a [`DcaDoubleBuf`] owner — DMA1 stream X drives SAI1_A
//! TX in double-buffer mode (M0AR + M1AR alternating), so the buffer
//! pair fits the [`DeviceActiveDoubleBuf<Read>`] typestate. The
//! cache-clean of the just-refilled inactive bank is emitted by
//! [`BankGuard<Read>`] construction (DCB-00 §5 transition row); the
//! pre-DCB private `clean_dcache(ptr, len)` helper that wrote DCCMVAC
//! directly is gone, along with its `raw_addr_cast` / `raw_mmio_cast`
//! discipline opt-out markers.
//!
//! Per DCB-00 §9 INV-D13, the player owns a single
//! [`cortex_m::peripheral::SCB`] taken at construction. Callers MUST
//! NOT take an independent `&mut SCB` for the duration of an
//! `AudioPlayer<_>`'s lifetime.
//!
//! ## Lifecycle
//!
//! The player is **single-shot per construction**: `new()` consumes a
//! [`DcaDoubleBuf`] reference and the buffer pair stays in DCB
//! ownership for the player's entire lifetime. `stop()` halts the
//! DMA but does NOT transition the typestate back to `DbufCpu`; for
//! a fresh playback the caller constructs a new player.
//!
//! [`DcaDoubleBuf`]: crate::hwcore::dca::DcaDoubleBuf
//! [`DeviceActiveDoubleBuf<Read>`]: crate::hwcore::dca::DbufRead
//! [`BankGuard<Read>`]: crate::hwcore::dca::BankGuard

use crate::dma_sai::DmaSai1Tx;
use crate::hwcore::dca::{Bank, DbufCpu, DbufRead, DcaCacheCtx, DcaDoubleBuf};
use crate::sai::Sai1Audio;
use crate::wav::WavHeader;

/// Result of an [`AudioPlayer::poll_refill`] call.
///
/// Refill cases are no longer surfaced as a `PollResult` variant —
/// per the DCB-02b-A 2026-05-03 amendment, the refill is invoked
/// inline through the closure passed to `poll_refill`. The `Idle`
/// / `Playing` / `Finished` variants here describe the *engine's*
/// state at the call boundary, independent of whether a refill
/// happened during the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollResult {
    /// No playback in progress.
    Idle,
    /// Playback is running; no further action needed this iteration
    /// (the closure may or may not have been invoked — either way,
    /// the engine is still streaming).
    Playing,
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

/// Holds the buffer pair's typestate. Exactly one variant is `Some`-equivalent
/// at any instant: the DCB typestate models "either CPU owns both banks" or
/// "DMA is alternately writing both banks", not both at once.
enum DcaState<const BUF_BYTES: usize> {
    /// Player is `Idle` / `Ready`. CPU holds the buffer pair via
    /// `DbufCpu`; banks may be filled directly via the M0/M1 mut slices
    /// (currently the caller does this via the raw pointer returned by
    /// `PollResult::NeedRefill`; see `refill_done` for the cache-clean
    /// transition path).
    Cpu(DbufCpu<'static, 'static, u8, BUF_BYTES>),
    /// Player is `Playing` / `Draining`. DMA holds the buffer pair via
    /// `DbufRead`; CPU may access the inactive bank via `BankGuard<Read>`
    /// (per DCB-00 §5).
    Active(DbufRead<'static, 'static, u8, BUF_BYTES>),
    /// Sentinel during state transitions. Never observed by callers —
    /// reads of this variant indicate a bug.
    Transitioning,
}

/// WAV audio player with DMA double-buffering.
///
/// `BUF_BYTES` is each bank's byte size (one of M0/M1); per DCB-00 §6
/// INV-D14 each bank is independently cache-line aligned and padded
/// (the underlying [`DcaDoubleBuf`] enforces this at construction).
pub struct AudioPlayer<const BUF_BYTES: usize> {
    dma: DmaSai1Tx,
    /// SCB owner for cache-op consolidation per DCB-00 §9 INV-D13.
    scb: cortex_m::peripheral::SCB,
    /// Buffer pair typestate.
    dca: DcaState<BUF_BYTES>,
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

impl<const BUF_BYTES: usize> AudioPlayer<BUF_BYTES> {
    /// Create a new audio player.
    ///
    /// `BUF_BYTES` is the size of each bank in bytes; per DCB-00 §6
    /// INV-D2 it MUST be a multiple of `CACHE_LINE` (32) and a
    /// multiple of 2 (16-bit-PCM granularity).
    ///
    /// `dca` is a `&'static mut` reference to the player's
    /// [`DcaDoubleBuf`]; the player takes ownership of the DCB
    /// typestate for its entire lifetime. Construct the DCA via
    /// [`DcaDoubleBuf::from_addrs`] over the player's two fixed-
    /// address banks (typically dedicated SDRAM regions).
    ///
    /// # Safety
    ///
    /// This constructor takes ownership of the Cortex-M SCB peripheral
    /// via `cortex_m::Peripherals::steal()` per DCB-00 §9 INV-D13
    /// (SCB ownership consolidation: a DCB-owning engine driver
    /// privately holds the SCB borrow). The caller MUST ensure that:
    ///
    /// - no other code path holds a `&mut SCB` for the lifetime of
    ///   the returned player,
    /// - the player is constructed at most once per binary (calling
    ///   `new()` twice would alias the SCB across two players),
    /// - `dca` names a [`DcaDoubleBuf`] that is exclusively owned by
    ///   this player from this call onward (the `&'static mut` makes
    ///   this a borrow-check property, but the unsafe `from_addrs`
    ///   constructor that produced it requires the same one-and-only
    ///   contract for the M0/M1 underlying memory).
    ///
    /// In single-threaded firmware (analyzer-cm7, disco-bare-metal,
    /// the FreeRTOS / Zephyr ports as currently structured), the
    /// global-singleton steal pattern is the documented usage.
    pub unsafe fn new(dca: &'static mut DcaDoubleBuf<'static, u8, BUF_BYTES>) -> Self { // rlvgl-discipline: allow(static_mut)
        // SAFETY: caller contract — the returned player is the sole
        // SCB consumer; SCB is one of the Cortex-M debug peripherals
        // (`steal()` returns a fresh handle each call but only one
        // CONSUMER may use it at a time per DCB-00 §9 INV-D13).
        let scb = unsafe { cortex_m::Peripherals::steal().SCB };
        // Convert the &'static mut DcaDoubleBuf to DbufCpu<'static,
        // 'static> by reborrowing through a raw pointer to extend the
        // method-call elided lifetime to 'static. SAFETY: the
        // underlying DCA is genuinely 'static (caller contract on
        // `dca`); the reborrow is to 'static and is the unique
        // access path to the DCA from this point on (the player
        // moves the DbufCpu into its Cpu state and never produces
        // a parallel reference).
        let dca_cpu: DbufCpu<'static, 'static, u8, BUF_BYTES> = unsafe {
            let p = dca as *mut DcaDoubleBuf<'static, u8, BUF_BYTES>;
            (*p).cpu()
        };
        Self {
            dma: DmaSai1Tx::new(),
            scb,
            dca: DcaState::Cpu(dca_cpu),
            state: PlayState::Idle,
            data_file_offset: 0,
            data_total: 0,
            bytes_queued: 0,
            last_target: 0,
        }
    }

    /// Prepare for playback of a WAV file.
    ///
    /// The caller must have already filled both M0 and M1 banks with
    /// the first `2 * BUF_BYTES` of PCM data before calling this.
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
        self.bytes_queued = core::cmp::min((BUF_BYTES as u32) * 2, header.data_length);
        self.last_target = 0;
        self.state = PlayState::Ready;
        true
    }

    /// Start DMA playback.
    ///
    /// Call after `prepare()` and pre-filling both banks (M0 + M1).
    /// Per DCB-00 §5 the typestate transition into `DbufRead` cleans
    /// both banks at entry — necessary because the caller's pre-fill
    /// used direct CPU writes (or memory-mapped copies) and any
    /// dirty cache lines must reach RAM before the DMA reads them.
    pub fn start(&mut self, sai: &Sai1Audio) {
        if self.state != PlayState::Ready {
            return;
        }

        let prev = core::mem::replace(&mut self.dca, DcaState::Transitioning);
        let cpu = match prev {
            DcaState::Cpu(c) => c,
            DcaState::Active(_) | DcaState::Transitioning => {
                // Already started or mid-transition; leave state
                // unchanged.
                self.dca = prev;
                return;
            }
        };

        // Enable DMA1 clock.
        self.dma.enable_clock();

        // DCB-02b: lift the buffer pair into `DbufRead<...>`. Per
        // DCB-00 §5 transition table this cleans both banks at entry
        // so the caller's pre-fill writes (which may still be in the
        // M7 D-cache) reach RAM before the DMA reads them.
        let mut ctx = DcaCacheCtx::new(&mut self.scb);
        let dbuf_read = cpu.start_double_buffer_read(&mut ctx);

        // Capture per-bank DMA addresses for stream configuration.
        let m0 = dbuf_read.m0_dma_addr().raw() as *const u8;
        let m1 = dbuf_read.m1_dma_addr().raw() as *const u8;
        drop(ctx);

        // Configure DMA: SAI1_A DR ← double-buffer.
        let periph_addr = sai.tx_data_register_addr();
        self.dma.configure(periph_addr, m0, m1, BUF_BYTES);

        // Enable SAI1 DMA requests.
        sai.enable_dma_tx();

        // Start DMA.
        self.dma.start();

        self.dca = DcaState::Active(dbuf_read);
        self.state = PlayState::Playing;
    }

    /// Poll the player and run a refill closure if one is needed.
    ///
    /// The closure `refill` is invoked when the DMA engine has just
    /// completed a half-period and the freshly-vacated (inactive)
    /// bank needs new PCM data. It receives:
    ///
    /// - `&mut [u8]` — the inactive bank's slice, **clipped to the
    ///   number of valid PCM bytes the file still has at this offset**
    ///   (i.e. `min(remaining, BUF_BYTES)`). At end-of-file the slice
    ///   may be shorter than `BUF_BYTES`; the closure should zero-fill
    ///   any indices it doesn't write valid PCM into.
    /// - `u32` — the byte offset into the WAV PCM data the caller
    ///   should read from.
    ///
    /// And returns the number of valid PCM bytes actually written
    /// (≤ `slice.len()`). The player uses this to advance
    /// `bytes_queued` and decide when to transition into `Draining`.
    ///
    /// DCB-02b-A2 (2026-05-03): the closure scope IS the `BankGuard<
    /// Read>` scope. Per DCB-01b-A's release-side cache-op placement
    /// (DCB-01d), the release-time clean fires *after* the closure
    /// returns, publishing the just-written PCM bytes to RAM before
    /// the engine flips back to this bank. The INV-D15 CT-bit
    /// re-check on `release` catches "engine flipped during the
    /// closure" as a stream-faster-than-CPU fault.
    ///
    /// `PollResult` describes the engine's state at the call
    /// boundary. The refill happens in-band: a `Playing` return may
    /// or may not have invoked the closure depending on whether the
    /// DMA had completed a half. `Idle` / `Finished` are returned
    /// without invoking the closure.
    pub fn poll_refill<F>(&mut self, refill: F) -> PollResult
    where
        F: FnOnce(&mut [u8], u32) -> usize,
    {
        match self.state {
            PlayState::Idle | PlayState::Ready => return PollResult::Idle,
            PlayState::Playing => {}
            PlayState::Draining => return self.poll_draining(),
        }

        // Check if DMA has switched buffers (transfer complete).
        if !self.dma.transfer_complete() {
            return PollResult::Playing;
        }
        self.dma.clear_transfer_complete();

        // Calculate how much data remains to be read from the file.
        let remaining = self.data_total.saturating_sub(self.bytes_queued);
        if remaining == 0 {
            // All data has been queued; enter draining.
            self.state = PlayState::Draining;
            return PollResult::Playing;
        }

        let max_bytes = core::cmp::min(remaining as usize, BUF_BYTES);
        let file_offset = self.data_file_offset + self.bytes_queued;

        // Determine the engine's current target. `DmaSai1Tx::current_target`
        // reads `DMA1_S0CR` bit 19 (the CT bit) via the typed wrapper —
        // the live-recheck source-of-truth for INV-D15.
        let target_bit = self.dma.current_target();
        let current_target = if target_bit == 0 { Bank::M0 } else { Bank::M1 };
        self.last_target = target_bit;

        let dbuf_read = match &mut self.dca {
            DcaState::Active(d) => d,
            _ => return PollResult::Idle,
        };

        let mut ctx = DcaCacheCtx::new(&mut self.scb);
        let mut guard = dbuf_read.bank_guard(&mut ctx, current_target);

        // Run the user's refill closure with the inactive bank's
        // slice clipped to `max_bytes`. The closure scope IS the
        // bank-guard scope; the destination is a safe `&mut [u8]`
        // end-to-end.
        let bank_slice: &mut [u8; BUF_BYTES] = guard.as_mut_slice();
        let pcm_bytes = refill(&mut bank_slice[..max_bytes], file_offset);

        // Release-time clean (DCB-01b-A: Read direction's clean fires
        // at release, not at construction) + INV-D15 CT-bit re-check.
        let new_target_bit = self.dma.current_target();
        let new_target = if new_target_bit == 0 {
            Bank::M0
        } else {
            Bank::M1
        };
        let _ = guard.release(&mut ctx, new_target);

        self.bytes_queued += pcm_bytes as u32;
        if self.bytes_queued >= self.data_total {
            self.state = PlayState::Draining;
        }

        PollResult::Playing
    }

    /// Stop playback immediately.
    ///
    /// Halts the DMA stream and disables SAI TX requests. The DCB
    /// typestate is intentionally left in `Active` — the player is
    /// single-shot per construction (see module-level docs); for a
    /// fresh playback the caller drops this player and constructs a
    /// new one with its own `&'static mut DcaDoubleBuf` reference.
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
