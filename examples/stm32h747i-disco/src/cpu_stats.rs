//! Lightweight CPU utilisation measurement using the DWT cycle counter.
//!
//! On Cortex-M7/M4 the Data Watchpoint and Trace unit provides a free-running
//! 32-bit cycle counter (CYCCNT) clocked at core speed.  This module snapshots
//! CYCCNT at frame boundaries and around WFI idle to compute a busy/idle ratio
//! expressed as CPU%.
//!
//! All state is stack-resident — zero heap allocation.

// ── DWT register addresses (Cortex-M Private Peripheral Bus) ───────────

const DWT_CTRL: u32 = 0xE000_1000;
const DWT_CYCCNT: u32 = 0xE000_1004;
const DWT_LAR: u32 = 0xE000_1FB0;
const DWT_LAR_KEY: u32 = 0xC5AC_CE55;

// ── D3 SRAM telemetry slots ────────────────────────────────────────────
//
// Located above the audio DMA staging buffers in SRAM4.

const D3_CM7_CPU_PCT: u32 = 0x3800_1C00;
const D3_CM7_BUSY: u32 = 0x3800_1C04;
const D3_CM7_TOTAL: u32 = 0x3800_1C08;
const D3_CM4_CPU_PCT: u32 = 0x3800_1C0C;
#[allow(dead_code)]
const D3_CM4_BUSY: u32 = 0x3800_1C10;
#[allow(dead_code)]
const D3_CM4_TOTAL: u32 = 0x3800_1C14;

// Driver metric stubs — reserved for future per-subsystem timing.
#[allow(dead_code)]
const D3_DMA2D_CYCLES: u32 = 0x3800_1C18;
#[allow(dead_code)]
const D3_TOUCH_CYCLES: u32 = 0x3800_1C1C;
#[allow(dead_code)]
const D3_SERIAL_CYCLES: u32 = 0x3800_1C20;
#[allow(dead_code)]
const D3_FRAME_IDLE_CYCLES: u32 = 0x3800_1C24;
#[allow(dead_code)]
const D3_LOOP_COUNT: u32 = 0x3800_1C28;
#[allow(dead_code)]
const D3_DMA2D_MAX_CYCLES: u32 = 0x3800_1C2C;
#[allow(dead_code)]
const D3_DMA2D_COUNTS: u32 = 0x3800_1C30;
#[allow(dead_code)]
const D3_SERIAL_DEPTHS: u32 = 0x3800_1C34;
#[allow(dead_code)]
const D3_SERIAL_DROPS: u32 = 0x3800_1C38;
#[allow(dead_code)]
const D3_PIPELINE_STAGE: u32 = 0x3800_1C3C;
#[allow(dead_code)]
const D3_SPIN_COUNTS: u32 = 0x3800_1C40;
#[allow(dead_code)]
const D3_DISPLAY_FLAGS: u32 = 0x3800_1C44;
#[allow(dead_code)]
const D3_DISPLAY_FRONT: u32 = 0x3800_1C48;
#[allow(dead_code)]
const D3_DISPLAY_BACK: u32 = 0x3800_1C4C;
#[allow(dead_code)]
const D3_DISPLAY_ACTIVE: u32 = 0x3800_1C50;
#[allow(dead_code)]
const D3_DISPLAY_STATUS: u32 = 0x3800_1C54;
#[allow(dead_code)]
const D3_DISPLAY_CPSR: u32 = 0x3800_1C58;
#[allow(dead_code)]
const D3_OVERLAY_COUNTS: u32 = 0x3800_1C5C;
#[allow(dead_code)]
const D3_OVERLAY_BYTES: u32 = 0x3800_1C60;
#[allow(dead_code)]
const D3_EVENT_STATE: u32 = 0x3800_1C64;
#[allow(dead_code)]
const D3_EVENT_DRAW_SEQ: u32 = 0x3800_1C68;
#[allow(dead_code)]
const D3_CRAWL_DIAG0: u32 = 0x3800_1C6C;
#[allow(dead_code)]
const D3_CRAWL_DIAG1: u32 = 0x3800_1C70;
#[allow(dead_code)]
const D3_CRAWL_DIAG2: u32 = 0x3800_1C74;
#[allow(dead_code)]
const D3_CRAWL_DIAG3: u32 = 0x3800_1C78;
// Clock widget telemetry (populated by `clock_demo` feature).
const D3_CLOCK_DIRTY_PX: u32 = 0x3800_1C7C;
const D3_CLOCK_PLAN_CYCLES: u32 = 0x3800_1C80;
const D3_CLOCK_DRAW_CYCLES: u32 = 0x3800_1C84;
/// Packed: byte 0 = outcome code (0=Skipped, 1=Painted, 2=FullRepaint),
/// byte 1 = layers_painted, bytes 2–3 = reserved.
const D3_CLOCK_OUTCOME: u32 = 0x3800_1C88;
/// High-water mark for `D3_CLOCK_DRAW_CYCLES` since boot. Sample this for
/// the worst-case-frame baseline; combine with `D3_CLOCK_DIRTY_PX` for
/// cycles-per-pixel comparison between AA paths.
const D3_CLOCK_DRAW_MAX: u32 = 0x3800_1C8C;

/// CPU utilisation tracker backed by the DWT cycle counter.
pub struct CpuStats {
    /// CYCCNT snapshot at the start of the current frame.
    frame_start: u32,
    /// CYCCNT snapshot taken just before entering WFI.
    idle_start: u32,
    /// Accumulated idle (sleeping) cycles within the current frame.
    idle_accum: u32,
    /// Idle cycles published for the most recent completed frame.
    idle_last: u32,
    /// Last computed CPU utilisation percentage (0–100).
    cpu_pct: u32,
    /// Whether DWT CYCCNT has been successfully enabled.
    enabled: bool,
    /// Whether this instance publishes to the CM4 telemetry slot.
    is_cm4: bool,
}

/// Centralised D3 SRAM telemetry writer.
///
/// Writes to fixed-offset SRAM4 slots that probe-rs scripts read for
/// live diagnostics. Single opt-out marker covers all telemetry writes
/// in this module; per-call-site markers would bloat the file.
///
/// # Safety
///
/// `addr` must be a writable D3 SRAM telemetry slot — typically one of
/// the `D3_*` constants declared at module top.
#[inline(always)]
unsafe fn d3_write(addr: u32, val: u32) {
    unsafe {
        (addr as *mut u32).write_volatile(val); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
    }
}

/// Centralised D3 SRAM telemetry reader. See [`d3_write`].
///
/// # Safety
///
/// Same as [`d3_write`].
#[inline(always)]
unsafe fn d3_read(addr: u32) -> u32 {
    unsafe { (addr as *const u32).read_volatile() } // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
}

impl CpuStats {
    /// Create a new tracker.  Call [`enable_dwt`] before use.
    pub const fn new() -> Self {
        Self {
            frame_start: 0,
            idle_start: 0,
            idle_accum: 0,
            idle_last: 0,
            cpu_pct: 0,
            enabled: false,
            is_cm4: false,
        }
    }

    /// Unlock and enable the DWT cycle counter.
    ///
    /// # Safety
    /// Writes to DWT control registers in the PPB region.
    pub unsafe fn enable_dwt(&mut self) {
        // DEMCR / DWT_LAR / DWT_CTRL are Cortex-M Private Peripheral Bus
        // debug registers. The cortex_m crate exposes some of them via
        // `DCB::enable_trace()` and `DWT::enable_cycle_counter()` but
        // requires plumbing `&mut DCB` / `&mut DWT` instances through
        // every caller. Opt-out markers document the deferral pending
        // a typed Cortex-M debug-control wrapper.
        unsafe {
            // DEMCR.TRCENA (bit 24) must be set before DWT registers work.
            const DEMCR: u32 = 0xE000_EDFC;
            let demcr = d3_read(DEMCR); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            d3_write(DEMCR, demcr | (1 << 24)); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            // Unlock the DWT LAR (some implementations lock DWT on reset).
            d3_write(DWT_LAR, DWT_LAR_KEY); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            // Set CYCCNTENA (bit 0) in DWT_CTRL.
            let ctrl = d3_read(DWT_CTRL); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            d3_write(DWT_CTRL, ctrl | 1); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
            // Reset the cycle counter.
            d3_write(DWT_CYCCNT, 0); // rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast)
        }
        cortex_m::asm::dsb();
        self.enabled = true;
    }

    /// Mark the beginning of a new frame.
    ///
    /// Computes the previous frame's CPU% from accumulated idle vs total
    /// cycles, writes telemetry to D3 SRAM, and resets accumulators.
    ///
    /// Call this right after `SYST.has_wrapped()` returns `true`.
    #[inline]
    pub fn frame_start(&mut self) {
        if !self.enabled {
            return;
        }
        let now = unsafe {
            d3_read(DWT_CYCCNT) /* rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast) */
        };

        // Previous frame duration (wrapping arithmetic handles CYCCNT wrap).
        let total = now.wrapping_sub(self.frame_start);
        self.idle_last = self.idle_accum;
        let busy = total.wrapping_sub(self.idle_accum);
        self.cpu_pct = if total > 0 {
            ((busy as u64 * 100) / total as u64) as u32
        } else {
            0
        };

        // Publish to D3 SRAM for probe / debugger / CM7 visibility.
        if self.is_cm4 {
            unsafe {
                d3_write(D3_CM4_CPU_PCT, self.cpu_pct);
            }
        } else {
            unsafe {
                d3_write(D3_CM7_CPU_PCT, self.cpu_pct);
                d3_write(D3_CM7_BUSY, busy);
                d3_write(D3_CM7_TOTAL, total);
                d3_write(D3_FRAME_IDLE_CYCLES, self.idle_last);
            }
        }

        // Reset for the new frame.
        self.frame_start = now;
        self.idle_accum = 0;
    }

    /// Snapshot CYCCNT before entering WFI (idle start).
    #[inline]
    pub fn idle_enter(&mut self) {
        if !self.enabled {
            return;
        }
        self.idle_start = unsafe {
            d3_read(DWT_CYCCNT) /* rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast) */
        };
    }

    /// Accumulate idle duration after WFI returns (idle end).
    #[inline]
    pub fn idle_exit(&mut self) {
        if !self.enabled {
            return;
        }
        let now = unsafe {
            d3_read(DWT_CYCCNT) /* rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast) */
        };
        self.idle_accum = self
            .idle_accum
            .wrapping_add(now.wrapping_sub(self.idle_start));
    }

    /// Last computed CPU utilisation percentage (0–100).
    pub fn cpu_pct(&self) -> u32 {
        self.cpu_pct
    }

    /// Read the CM4's CPU% from D3 SRAM (written by the CM4 core).
    /// Returns 0 if the CM4 hasn't written a valid value (garbage guard).
    pub fn cm4_cpu_pct(&self) -> u32 {
        let raw = unsafe { d3_read(D3_CM4_CPU_PCT) };
        if raw > 100 { 0 } else { raw }
    }

    /// Read the raw CYCCNT value (for driver-level bracket timing).
    #[inline]
    pub fn cyccnt(&self) -> u32 {
        unsafe {
            d3_read(DWT_CYCCNT) /* rlvgl-discipline: allow(raw_addr_cast) allow(raw_mmio_cast) */
        }
    }

    // ── Driver metric stubs ────────────────────────────────────────────
    //
    // One-liner write_volatile calls — populate these when instrumenting
    // individual subsystems (DMA2D blit time, touch poll latency, etc.).

    /// Record DMA2D operation cycles.  Stub — reserved for future use.
    #[inline]
    #[allow(dead_code)]
    pub fn record_dma2d_cycles(&self, cycles: u32) {
        unsafe {
            d3_write(D3_DMA2D_CYCLES, cycles);
        }
    }

    /// Record touch poll cycles.  Stub — reserved for future use.
    #[inline]
    #[allow(dead_code)]
    pub fn record_touch_cycles(&self, cycles: u32) {
        unsafe {
            d3_write(D3_TOUCH_CYCLES, cycles);
        }
    }

    /// Record serial poll cycles.  Stub — reserved for future use.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_cycles(&self, cycles: u32) {
        unsafe {
            d3_write(D3_SERIAL_CYCLES, cycles);
        }
    }

    /// Publish loop iterations executed in the current frame.
    #[inline]
    #[allow(dead_code)]
    pub fn record_loop_count(&self, loops: u32) {
        unsafe {
            d3_write(D3_LOOP_COUNT, loops);
        }
    }

    /// Publish DMA2D max-cycle telemetry.
    #[inline]
    #[allow(dead_code)]
    pub fn record_dma2d_max_cycles(&self, cycles: u32) {
        unsafe {
            d3_write(D3_DMA2D_MAX_CYCLES, cycles);
        }
    }

    /// Publish DMA2D completion/error counters packed as `complete << 16 | error`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_dma2d_counts(&self, complete: u16, error: u16) {
        unsafe {
            d3_write(D3_DMA2D_COUNTS, ((complete as u32) << 16) | error as u32);
        }
    }

    /// Publish serial queue depths packed as `rx << 16 | tx`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_depths(&self, rx_depth: u16, tx_depth: u16) {
        unsafe {
            d3_write(
                D3_SERIAL_DEPTHS,
                ((rx_depth as u32) << 16) | tx_depth as u32,
            );
        }
    }

    /// Publish serial drop counters packed as `rx << 16 | tx`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_drops(&self, rx_drop: u16, tx_drop: u16) {
        unsafe {
            d3_write(D3_SERIAL_DROPS, ((rx_drop as u32) << 16) | tx_drop as u32);
        }
    }

    /// Publish pipeline stage and frame identifiers packed into one word.
    #[inline]
    #[allow(dead_code)]
    pub fn record_pipeline_stage(&self, stage: u8, current_frame: u16, queued_frame: u8) {
        unsafe {
            d3_write(
                D3_PIPELINE_STAGE,
                ((stage as u32) << 24) | ((queued_frame as u32) << 16) | current_frame as u32,
            );
        }
    }

    /// Publish legacy spin counters packed as `serial << 16 | dma`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_spin_counts(&self, serial_spins: u16, dma_spins: u16) {
        unsafe {
            d3_write(
                D3_SPIN_COUNTS,
                ((serial_spins as u32) << 16) | dma_spins as u32,
            );
        }
    }

    /// Publish display/present diagnostics.
    #[inline]
    #[allow(dead_code)]
    pub fn record_display_diag(
        &self,
        flags: u32,
        front: u32,
        back: u32,
        active: u32,
        wisr: u16,
        status: u16,
        cpsr: u32,
    ) {
        unsafe {
            d3_write(D3_DISPLAY_FLAGS, flags);
            d3_write(D3_DISPLAY_FRONT, front);
            d3_write(D3_DISPLAY_BACK, back);
            d3_write(D3_DISPLAY_ACTIVE, active);
            d3_write(D3_DISPLAY_STATUS, ((wisr as u32) << 16) | status as u32);
            d3_write(D3_DISPLAY_CPSR, cpsr);
        }
    }

    /// Publish compositor/save-under diagnostics.
    #[inline]
    #[allow(dead_code)]
    pub fn record_overlay_diag(&self, counts: u32, bytes: u32) {
        unsafe {
            d3_write(D3_OVERLAY_COUNTS, counts);
            d3_write(D3_OVERLAY_BYTES, bytes);
        }
    }

    /// Publish event-window diagnostics.
    #[inline]
    #[allow(dead_code)]
    pub fn record_event_diag(&self, state: u32, draw_seq: u32) {
        unsafe {
            d3_write(D3_EVENT_STATE, state);
            d3_write(D3_EVENT_DRAW_SEQ, draw_seq);
        }
    }

    /// Publish crawl renderer diagnostics.
    #[inline]
    #[allow(dead_code)]
    pub fn record_crawl_diag(&self, word0: u32, word1: u32, word2: u32, word3: u32) {
        unsafe {
            d3_write(D3_CRAWL_DIAG0, word0);
            d3_write(D3_CRAWL_DIAG1, word1);
            d3_write(D3_CRAWL_DIAG2, word2);
            d3_write(D3_CRAWL_DIAG3, word3);
        }
    }
}

/// Read the DWT cycle counter directly. Returns garbage if DWT hasn't
/// been enabled via [`CpuStats::enable_dwt`]; deltas should always be
/// computed via `wrapping_sub` to handle the 32-bit wrap.
#[inline]
#[allow(dead_code)]
pub fn read_cyccnt() -> u32 {
    unsafe { (DWT_CYCCNT as *const u32).read_volatile() }
}

/// Publish a clock-widget telemetry frame to the D3 SRAM slots claimed by
/// `clock_demo`. Tracks a high-water draw-cycles mark internally so the
/// debugger can sample the worst-case frame without constant polling.
///
/// `outcome_code`: 0 = Skipped, 1 = Painted, 2 = FullRepaint.
#[inline]
#[allow(dead_code)]
pub fn publish_clock_telem(
    outcome_code: u8,
    layers_painted: u8,
    dirty_px: u32,
    plan_cycles: u32,
    draw_cycles: u32,
) {
    static mut DRAW_MAX: u32 = 0;
    // Safety: single-writer (called from CM7 main thread only).
    let prev_max = unsafe { DRAW_MAX };
    let new_max = if draw_cycles > prev_max {
        unsafe { DRAW_MAX = draw_cycles };
        draw_cycles
    } else {
        prev_max
    };
    let outcome_word = (outcome_code as u32) | ((layers_painted as u32) << 8);
    unsafe {
        (D3_CLOCK_DIRTY_PX as *mut u32).write_volatile(dirty_px);
        (D3_CLOCK_PLAN_CYCLES as *mut u32).write_volatile(plan_cycles);
        (D3_CLOCK_DRAW_CYCLES as *mut u32).write_volatile(draw_cycles);
        (D3_CLOCK_OUTCOME as *mut u32).write_volatile(outcome_word);
        (D3_CLOCK_DRAW_MAX as *mut u32).write_volatile(new_max);
    }
}
