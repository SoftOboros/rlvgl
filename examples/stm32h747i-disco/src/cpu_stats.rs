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
const D3_WIFI_RESERVED: u32 = 0x3800_1C44;

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

    /// Create a tracker that publishes to the CM4 telemetry slot.
    pub const fn new_cm4() -> Self {
        Self {
            frame_start: 0,
            idle_start: 0,
            idle_accum: 0,
            idle_last: 0,
            cpu_pct: 0,
            enabled: false,
            is_cm4: true,
        }
    }

    /// Unlock and enable the DWT cycle counter.
    ///
    /// # Safety
    /// Writes to DWT control registers in the PPB region.
    pub unsafe fn enable_dwt(&mut self) {
        // DEMCR.TRCENA (bit 24) must be set before DWT registers work.
        const DEMCR: u32 = 0xE000_EDFC;
        let demcr = (DEMCR as *const u32).read_volatile();
        (DEMCR as *mut u32).write_volatile(demcr | (1 << 24));
        // Unlock the DWT LAR (some implementations lock DWT on reset).
        (DWT_LAR as *mut u32).write_volatile(DWT_LAR_KEY);
        // Set CYCCNTENA (bit 0) in DWT_CTRL.
        let ctrl = (DWT_CTRL as *const u32).read_volatile();
        (DWT_CTRL as *mut u32).write_volatile(ctrl | 1);
        // Reset the cycle counter.
        (DWT_CYCCNT as *mut u32).write_volatile(0);
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
        let now = unsafe { (DWT_CYCCNT as *const u32).read_volatile() };

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
                (D3_CM4_CPU_PCT as *mut u32).write_volatile(self.cpu_pct);
            }
        } else {
            unsafe {
                (D3_CM7_CPU_PCT as *mut u32).write_volatile(self.cpu_pct);
                (D3_CM7_BUSY as *mut u32).write_volatile(busy);
                (D3_CM7_TOTAL as *mut u32).write_volatile(total);
                (D3_FRAME_IDLE_CYCLES as *mut u32).write_volatile(self.idle_last);
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
        self.idle_start = unsafe { (DWT_CYCCNT as *const u32).read_volatile() };
    }

    /// Accumulate idle duration after WFI returns (idle end).
    #[inline]
    pub fn idle_exit(&mut self) {
        if !self.enabled {
            return;
        }
        let now = unsafe { (DWT_CYCCNT as *const u32).read_volatile() };
        self.idle_accum = self
            .idle_accum
            .wrapping_add(now.wrapping_sub(self.idle_start));
    }

    /// Last computed CPU utilisation percentage (0–100).
    pub fn cpu_pct(&self) -> u32 {
        self.cpu_pct
    }

    /// Idle cycles measured in the most recently completed frame.
    pub fn idle_cycles(&self) -> u32 {
        self.idle_last
    }

    /// Read the CM4's CPU% from D3 SRAM (written by the CM4 core).
    /// Returns 0 if the CM4 hasn't written a valid value (garbage guard).
    pub fn cm4_cpu_pct(&self) -> u32 {
        let raw = unsafe { (D3_CM4_CPU_PCT as *const u32).read_volatile() };
        if raw > 100 { 0 } else { raw }
    }

    /// Read the raw CYCCNT value (for driver-level bracket timing).
    #[inline]
    pub fn cyccnt(&self) -> u32 {
        unsafe { (DWT_CYCCNT as *const u32).read_volatile() }
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
            (D3_DMA2D_CYCLES as *mut u32).write_volatile(cycles);
        }
    }

    /// Record touch poll cycles.  Stub — reserved for future use.
    #[inline]
    #[allow(dead_code)]
    pub fn record_touch_cycles(&self, cycles: u32) {
        unsafe {
            (D3_TOUCH_CYCLES as *mut u32).write_volatile(cycles);
        }
    }

    /// Record serial poll cycles.  Stub — reserved for future use.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_cycles(&self, cycles: u32) {
        unsafe {
            (D3_SERIAL_CYCLES as *mut u32).write_volatile(cycles);
        }
    }

    /// Publish loop iterations executed in the current frame.
    #[inline]
    #[allow(dead_code)]
    pub fn record_loop_count(&self, loops: u32) {
        unsafe {
            (D3_LOOP_COUNT as *mut u32).write_volatile(loops);
        }
    }

    /// Publish DMA2D max-cycle telemetry.
    #[inline]
    #[allow(dead_code)]
    pub fn record_dma2d_max_cycles(&self, cycles: u32) {
        unsafe {
            (D3_DMA2D_MAX_CYCLES as *mut u32).write_volatile(cycles);
        }
    }

    /// Publish DMA2D completion/error counters packed as `complete << 16 | error`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_dma2d_counts(&self, complete: u16, error: u16) {
        unsafe {
            (D3_DMA2D_COUNTS as *mut u32).write_volatile(((complete as u32) << 16) | error as u32);
        }
    }

    /// Publish serial queue depths packed as `rx << 16 | tx`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_depths(&self, rx_depth: u16, tx_depth: u16) {
        unsafe {
            (D3_SERIAL_DEPTHS as *mut u32)
                .write_volatile(((rx_depth as u32) << 16) | tx_depth as u32);
        }
    }

    /// Publish serial drop counters packed as `rx << 16 | tx`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_serial_drops(&self, rx_drop: u16, tx_drop: u16) {
        unsafe {
            (D3_SERIAL_DROPS as *mut u32).write_volatile(((rx_drop as u32) << 16) | tx_drop as u32);
        }
    }

    /// Publish pipeline stage and frame identifiers packed into one word.
    #[inline]
    #[allow(dead_code)]
    pub fn record_pipeline_stage(&self, stage: u8, current_frame: u16, queued_frame: u8) {
        unsafe {
            (D3_PIPELINE_STAGE as *mut u32).write_volatile(
                ((stage as u32) << 24) | ((queued_frame as u32) << 16) | current_frame as u32,
            );
        }
    }

    /// Publish legacy spin counters packed as `serial << 16 | dma`.
    #[inline]
    #[allow(dead_code)]
    pub fn record_spin_counts(&self, serial_spins: u16, dma_spins: u16) {
        unsafe {
            (D3_SPIN_COUNTS as *mut u32)
                .write_volatile(((serial_spins as u32) << 16) | dma_spins as u32);
        }
    }
}
