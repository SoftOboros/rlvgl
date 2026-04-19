//! FreeRTOS entry point: ISRs, tasks, and scheduler start.
//!
//! This module is the counterpart to `zephyr_entry.rs`, but for a
//! hybrid bare-metal + FreeRTOS build. Hardware init (clocks, SDRAM,
//! DSI, LTDC, DMA2D) is driven by the existing bare-metal code in
//! `main.rs` — FreeRTOS only provides the preemptive task / semaphore
//! layer on top.
//!
//! ## Task model
//!
//! | Task     | Priority | Stack | Blocks on                               |
//! |----------|----------|-------|-----------------------------------------|
//! | present  | 3        | 2 KB  | `erif_sem` — DSI ERIF signals it        |
//! | render   | 1        | 8 KB  | `buf_ready_sem` — render request        |
//! | touch    | 2        | 1 KB  | `vTaskDelayUntil` 120 Hz                |
//!
//! ## ISR model
//!
//! DSI (IRQ 78) and DMA2D (IRQ 90) ISRs are routed through the normal
//! cortex-m-rt vector table (`#[interrupt]`). FreeRTOS only takes over
//! SVCall / PendSV / SysTick via cortex-m-rt `#[exception]` handlers.
//! All three ISRs run at NVIC priority 6-7 — above
//! `configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY = 5` — so they stay
//! ISR-safe for `xSemaphoreGiveFromISR`.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::sync::atomic::{AtomicPtr, Ordering};

use crate::freertos_sync::{self, FreeRtosFrameSync, SemaphoreHandle_t, StaticSemaphore};

// ── FreeRTOS task + kernel FFI ────────────────────────────────────────────────

/// `StaticTask_t` from FreeRTOS — opaque TCB storage. On ARM-CM7 the
/// sizeof is ~96 bytes with default config; reserve 128 for safety.
#[repr(C, align(8))]
pub struct StaticTask {
    _storage: [u8; 128],
}

impl StaticTask {
    pub const fn new() -> Self {
        Self { _storage: [0; 128] }
    }
}

#[repr(C)]
pub struct TaskDefinition {
    _opaque: [u8; 0],
}
pub type TaskHandle_t = *mut TaskDefinition;
pub type StackType_t = u32;

type TaskFunction_t = unsafe extern "C" fn(*mut core::ffi::c_void);

unsafe extern "C" {
    fn xTaskCreateStatic(
        pxTaskCode: TaskFunction_t,
        pcName: *const u8,
        ulStackDepth: u32,
        pvParameters: *mut core::ffi::c_void,
        uxPriority: u32,
        puxStackBuffer: *mut StackType_t,
        pxTaskBuffer: *mut StaticTask,
    ) -> TaskHandle_t;

    fn vTaskStartScheduler() -> !;
    fn vTaskDelay(xTicksToDelay: u32);
}

// ── SVC / PendSV / SysTick exception routing ──────────────────────────────────
//
// FreeRTOS's `vPortSVCHandler` and `xPortPendSVHandler` are declared
// `__attribute__((naked))` — they hand-roll the Cortex-M context save
// and return via `bx r14`. Wrapping them in a Rust `extern "C"` shim
// would emit a function prologue that corrupts the handler's stack
// frame. Instead, `ffi_shims.c` provides GCC `__attribute__((alias))`
// declarations that point the vector-table symbols `SVCall` and
// `PendSV` directly at the naked handlers. cortex-m-rt declares those
// slots as weak defaults, so the strong C aliases win at link time.
//
// `xPortSysTickHandler` is a normal function, so we can wrap it via
// the usual `#[cortex_m_rt::exception]` path.

unsafe extern "C" {
    fn xPortSysTickHandler();
}

/// Set true in start() right before vTaskStartScheduler(). Until then,
/// the SysTick handler is a no-op — prevents xPortSysTickHandler from
/// running on uninitialized FreeRTOS scheduler data when the HAL's
/// clock setup enables SysTick before the scheduler starts.
static SYSTICK_READY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

#[cortex_m_rt::exception]
fn SysTick() {
    if SYSTICK_READY.load(Ordering::Relaxed) {
        unsafe { xPortSysTickHandler() }
    } else {
        // Count pre-scheduler SysTick hits at D3 SRAM 0x3800_0608
        unsafe {
            let p = 0x3800_0608u32 as *mut u32;
            p.write_volatile(p.read_volatile().wrapping_add(1));
        }
    }
}

// ── Global sync pointer ───────────────────────────────────────────────────────

static SYNC: AtomicPtr<FreeRtosFrameSync> = AtomicPtr::new(core::ptr::null_mut());

#[inline]
fn get_sync() -> Option<&'static FreeRtosFrameSync> {
    let ptr = SYNC.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Expose the DMA2D completion semaphore to other FreeRTOS-side
/// modules (e.g. `freertos_dma2d::FreeRtosDma2dBlitter`) that need to
/// block on DMA2D transfer completion from a task context.
pub fn dma2d_done_sem() -> Option<SemaphoreHandle_t> {
    get_sync().map(|s| s.dma2d_done_sem)
}

// ── TIM7 present gate ─────────────────────────────────────────────────────────
//
// TIM7 is a basic 16-bit APB1 timer with a single update interrupt.
// We run it in one-pulse mode at 1 MHz (PSC=199 against the 200 MHz
// APB1 timer clock) so `ARR` is a count of microseconds up to 65535.
//
// Phase-lock flow:
//   1. DSI ERIF ISR records erif_cyc (DWT snapshot) and gives erif_sem.
//   2. present_task wakes, reads erif_cyc, computes remaining µs until
//      the PRESENT_HOLDOFF deadline.
//   3. present_task arms TIM7 with ARR = remaining_us (OPM=1) and
//      blocks on `present_gate_sem`.
//   4. TIM7 UIF fires at the deadline; its ISR gives present_gate_sem.
//   5. present_task wakes, swaps + retriggers LTDC — every present
//      lands on the exact same DWT offset from ERIF, phase-locked to
//      the panel's TE signal. No DWT spin, no vTaskDelay jitter.

const TIM7_BASE: usize = 0x4000_1400;
const TIM7_CR1:  *mut u32 = (TIM7_BASE + 0x00) as *mut u32;
const TIM7_DIER: *mut u32 = (TIM7_BASE + 0x0C) as *mut u32;
const TIM7_SR:   *mut u32 = (TIM7_BASE + 0x10) as *mut u32;
const TIM7_EGR:  *mut u32 = (TIM7_BASE + 0x14) as *mut u32;
const TIM7_CNT:  *mut u32 = (TIM7_BASE + 0x24) as *mut u32;
const TIM7_PSC:  *mut u32 = (TIM7_BASE + 0x28) as *mut u32;
const TIM7_ARR:  *mut u32 = (TIM7_BASE + 0x2C) as *mut u32;

/// RCC APB1LENR on STM32H747 (D2 domain, CM7 view).
const RCC_APB1LENR: *mut u32 = 0x5802_44E8 as *mut u32;
/// TIM7EN is bit 5 of APB1LENR.
const RCC_APB1LENR_TIM7EN: u32 = 1 << 5;

/// Present gate semaphore — given by TIM7 ISR when the holdoff
/// deadline fires; taken (with portMAX_DELAY) by present_task.
static PRESENT_GATE_SEM: AtomicPtr<freertos_sync::QueueDefinition> =
    AtomicPtr::new(core::ptr::null_mut());
static mut PRESENT_GATE_SEM_BUF: StaticSemaphore = StaticSemaphore::new();

/// Render-start semaphore — given by the DSI ERIF ISR alongside
/// `erif_sem`. `render_task` blocks on this so render begins on
/// every ERIF edge, phase-locked to the panel scan. Without this
/// gate, render runs on its own vTaskDelay cadence (~62 Hz) against
/// a 30 Hz present rate; the non-integer ratio collapses `buf_ready`
/// into a binary sem race where alternate renders are lost,
/// producing beat-frequency jitter on the moving-block demo.
static RENDER_START_SEM: AtomicPtr<freertos_sync::QueueDefinition> =
    AtomicPtr::new(core::ptr::null_mut());
static mut RENDER_START_SEM_BUF: StaticSemaphore = StaticSemaphore::new();

/// One-time TIM7 setup. Enables clock, configures PSC/ARR for 1 MHz
/// one-pulse operation, enables UIE. Call from `start()` after the
/// sync object is wired.
unsafe fn tim7_init() {
    unsafe {
        RCC_APB1LENR
            .write_volatile(RCC_APB1LENR.read_volatile() | RCC_APB1LENR_TIM7EN);
        // Barrier: the PAC docs recommend a read-back after clock
        // enable to ensure the register write has landed before we
        // touch the peripheral.
        let _ = RCC_APB1LENR.read_volatile();

        TIM7_CR1.write_volatile(0); // disable + clear all flags
        TIM7_CNT.write_volatile(0);
        // APB1 timer clock = 200 MHz on this board (bare-metal TIM6
        // uses the same divisor — see main.rs line ~2167 for the
        // same empirical value). PSC=199 → 1 MHz, 1 µs per count.
        TIM7_PSC.write_volatile(199);
        TIM7_ARR.write_volatile(0xFFFF);
        TIM7_EGR.write_volatile(1); // UG: reload PSC shadow
        TIM7_SR.write_volatile(0); // clear any pending UIF
        TIM7_DIER.write_volatile(1); // UIE
        // CR1: OPM (one-pulse) | URS (only overflow asserts UEV — so
        // our `EGR.UG=1` reload above does NOT spuriously fire UIF).
        // CEN is off; we set it per-arm.
        TIM7_CR1.write_volatile((1 << 3) | (1 << 2));
    }
}

/// Arm TIM7 for `us` microseconds. Caller must have installed the
/// sem before calling. Safe to call repeatedly — each arm restarts
/// the timer from 0.
unsafe fn tim7_arm(us: u32) {
    unsafe {
        // Stop (in case a previous arm is still running)
        TIM7_CR1.write_volatile((1 << 3) | (1 << 2));
        TIM7_CNT.write_volatile(0);
        // Clamp to timer range. For values larger than 65 ms this
        // would overflow — in practice present holdoff is 15 ms so
        // we never hit that, but clamp defensively.
        let arr = us.max(1).min(0xFFFF);
        TIM7_ARR.write_volatile(arr);
        TIM7_SR.write_volatile(0); // clear stale UIF
        // CR1: OPM | URS | CEN
        TIM7_CR1.write_volatile((1 << 3) | (1 << 2) | (1 << 0));
    }
}

/// TIM7 interrupt body — called from the #[interrupt] wrapper in main.rs.
#[inline]
pub fn tim7_isr_body() {
    unsafe {
        let sr = TIM7_SR.read_volatile();
        if sr & 1 != 0 {
            TIM7_SR.write_volatile(0); // clear UIF
            let gate = PRESENT_GATE_SEM.load(Ordering::Acquire);
            if !gate.is_null() {
                freertos_sync::rlvgl_sem_give_from_isr(gate);
            }
        }
    }
}

// ── ISR handlers (named per STM32H7 NVIC; #[interrupt] in main.rs wraps these)

/// DSI ISR body — same register reads as bare-metal, but gives the
/// ERIF semaphore from ISR context instead of setting an atomic.
#[inline]
pub fn dsi_isr_body() {
    unsafe {
        const WISR: *const u32 = 0x5000_040C as *const u32;
        const WIFCR: *mut u32 = 0x5000_0410 as *mut u32;
        const ISR0: *const u32 = 0x5000_00BC as *const u32;
        const ISR1: *const u32 = 0x5000_00C0 as *const u32;
        const FIR0: *mut u32 = 0x5000_00D8 as *mut u32;
        const FIR1: *mut u32 = 0x5000_00DC as *mut u32;
        const DSI_WCR: *mut u32 = 0x5000_0404 as *mut u32;
        const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;
        const GPIOJ_BSRR: *mut u32 = (0x5802_2400 + 0x18) as *mut u32;

        let wisr = WISR.read_volatile();
        WIFCR.write_volatile(wisr & 0x3FFF);

        if wisr & 0x02 != 0 {
            let cyc = DWT_CYCCNT.read_volatile();
            // PJ0 LOW — LTDC scan done
            GPIOJ_BSRR.write_volatile(1u32 << 16);
            // Clear LTDCEN to prevent auto-refresh
            DSI_WCR.write_volatile(0x08);

            if let Some(sync) = get_sync() {
                sync.isr_record_erif(cyc);
                // Sticky flag for non-destructive queries (star_crawl
                // gate, etc.). Cleared by present_task just before
                // each LTDC retrigger.
                sync.scan_complete.store(true, Ordering::Release);
                freertos_sync::rlvgl_sem_give_from_isr(sync.erif_sem);
            }
            // Wake render_task at ERIF so its compose+blend DMA2D
            // runs during the back porch (ERIF → retrigger) while
            // present_task is blocked on TIM7. Zero AXI contention
            // because LTDC scan hasn't started yet.
            let rs = RENDER_START_SEM.load(Ordering::Acquire);
            if !rs.is_null() {
                freertos_sync::rlvgl_sem_give_from_isr(rs);
            }
        }

        let isr0 = ISR0.read_volatile();
        if isr0 != 0 {
            FIR0.write_volatile(isr0);
        }
        let isr1 = ISR1.read_volatile();
        if isr1 != 0 {
            FIR1.write_volatile(isr1);
        }
    }
}

/// DMA2D ISR body — clears flags and gives the completion semaphore.
#[inline]
pub fn dma2d_isr_body() {
    unsafe {
        const DMA2D_ISR: *const u32 = 0x5200_1004 as *const u32;
        const DMA2D_IFCR: *mut u32 = 0x5200_1008 as *mut u32;

        let isr = DMA2D_ISR.read_volatile();
        let clear = isr & 0x3F;
        if clear != 0 {
            DMA2D_IFCR.write_volatile(clear);
        }

        if isr & (1 << 1) != 0 {
            if let Some(sync) = get_sync() {
                freertos_sync::rlvgl_sem_give_from_isr(sync.dma2d_done_sem);
            }
        }
    }
}

// ── Static storage for kernel objects ─────────────────────────────────────────

const PRESENT_STACK_WORDS: usize = 512; // 2 KB
const RENDER_STACK_WORDS: usize = 2048; // 8 KB
const TOUCH_STACK_WORDS: usize = 256; // 1 KB
const PLAYIT_STACK_WORDS: usize = 512; // 2 KB — small cmd parsing + serial

static mut ERIF_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut DMA2D_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut BUF_READY_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut I2C4_SEM_BUF: StaticSemaphore = StaticSemaphore::new();

static mut PRESENT_TCB: StaticTask = StaticTask::new();
static mut RENDER_TCB: StaticTask = StaticTask::new();
static mut TOUCH_TCB: StaticTask = StaticTask::new();
static mut PLAYIT_TCB: StaticTask = StaticTask::new();

static mut PRESENT_STACK: [StackType_t; PRESENT_STACK_WORDS] = [0; PRESENT_STACK_WORDS];
static mut RENDER_STACK: [StackType_t; RENDER_STACK_WORDS] = [0; RENDER_STACK_WORDS];
static mut TOUCH_STACK: [StackType_t; TOUCH_STACK_WORDS] = [0; TOUCH_STACK_WORDS];
static mut PLAYIT_STACK: [StackType_t; PLAYIT_STACK_WORDS] = [0; PLAYIT_STACK_WORDS];

static mut SYNC_STORAGE: core::mem::MaybeUninit<FreeRtosFrameSync> =
    core::mem::MaybeUninit::uninit();

/// Binary semaphore: render task signals when back buffer is ready to
/// present; present task takes it after the ERIF holdoff.
static BUF_READY_SEM: AtomicPtr<freertos_sync::QueueDefinition> =
    AtomicPtr::new(core::ptr::null_mut());

// ── Task entry functions ──────────────────────────────────────────────────────
//
// These are deliberately skeletal for the first milestone — enough to
// prove the build, semaphore creation, and scheduler handoff work.
// The real present / render / touch bodies will land in follow-up
// commits once we prove the image boots into the scheduler.

// ── Framebuffer addresses (written by main.rs at handoff) ────────────────────
//
// After bare-metal init finishes, `main.rs` calls
// `freertos_entry::init_fbs(front, back, bytes)` before `start()`.
//
// Double-buffer flow:
//   - render_task writes into BACK_FB_ADDR.
//   - render_task gives BUF_READY_SEM when done.
//   - present_task takes BUF_READY_SEM non-blocking each frame. If a
//     new frame is ready, it atomically swaps FRONT_FB_ADDR and
//     BACK_FB_ADDR before re-triggering the LTDC scan.
//
// With no render in flight, present_task just re-scans the same
// FRONT_FB_ADDR every frame, so the splash stays on the panel.

static FRONT_FB_ADDR: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static BACK_FB_ADDR: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static FB_BYTES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static FB_W: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static FB_H: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Record the framebuffer geometry for the present / render tasks.
/// Called once from `main()` just before `freertos_entry::start()`.
pub fn init_fbs(front: u32, back: u32, width: u32, height: u32) {
    let bytes = width * height * 4; // ARGB8888
    FRONT_FB_ADDR.store(front, core::sync::atomic::Ordering::Release);
    BACK_FB_ADDR.store(back, core::sync::atomic::Ordering::Release);
    FB_W.store(width, core::sync::atomic::Ordering::Release);
    FB_H.store(height, core::sync::atomic::Ordering::Release);
    FB_BYTES.store(bytes, core::sync::atomic::Ordering::Release);
}

/// Backwards-compat shim for the earlier single-buffer entry.
pub fn init_fb_addr(front: u32) {
    FRONT_FB_ADDR.store(front, core::sync::atomic::Ordering::Release);
}

/// Trigger a fresh LTDC scan of the framebuffer at `fb_addr`.
///
/// Mirrors the raw register writes in
/// `rlvgl_platform::stm32h747i_disco::Stm32h747iDiscoDisplay::present()`
/// without requiring exclusive access to the display struct.
///
/// 1. Clear stale ERIF (so the DSI ISR fires on *this* scan's completion).
/// 2. Write `fb_addr` to LTDC layer 1 CFBAR.
/// 3. Trigger shadow reload via LTDC SRCR.IMR.
/// 4. Enable LTDCEN + DSIEN in DSI_WCR — the next TE edge kicks off the scan.
#[inline]
unsafe fn ltdc_retrigger(fb_addr: u32) {
    unsafe {
        const DSI_WIFCR: *mut u32 = 0x5000_0410 as *mut u32;
        const DSI_WCR: *mut u32 = 0x5000_0404 as *mut u32;
        const LTDC_L1CFBAR: *mut u32 = 0x5000_10AC as *mut u32;
        const LTDC_SRCR: *mut u32 = 0x5000_1024 as *mut u32;

        cortex_m::asm::dsb();
        DSI_WIFCR.write_volatile(0x02); // clear ERIF
        cortex_m::asm::dsb();

        LTDC_L1CFBAR.write_volatile(fb_addr);
        LTDC_SRCR.write_volatile(1); // IMR — immediate shadow reload

        DSI_WCR.write_volatile(0x0C); // DSIEN + LTDCEN
        cortex_m::asm::dsb();
        DSI_WIFCR.write_volatile(0x02); // clear spurious ERIF from re-enable
    }
}

// ── LTDC Layer 2: A8 text overlay ────────────────────────────────────────────

/// LTDC Layer 2 register base (Layer 1 + 0x80).
const LTDC_L2: u32 = 0x5000_1000 + 0x104;
const LTDC_L2CR: *mut u32 = (LTDC_L2 + 0x00) as *mut u32;
const LTDC_L2WHPCR: *mut u32 = (LTDC_L2 + 0x04) as *mut u32;
const LTDC_L2WVPCR: *mut u32 = (LTDC_L2 + 0x08) as *mut u32;
const LTDC_L2PFCR: *mut u32 = (LTDC_L2 + 0x10) as *mut u32;
const LTDC_L2CACR: *mut u32 = (LTDC_L2 + 0x14) as *mut u32;
const LTDC_L2DCCR: *mut u32 = (LTDC_L2 + 0x18) as *mut u32;
const LTDC_L2BFCR: *mut u32 = (LTDC_L2 + 0x1C) as *mut u32;
const LTDC_L2CFBAR: *mut u32 = (LTDC_L2 + 0x28) as *mut u32;
const LTDC_L2CFBLR: *mut u32 = (LTDC_L2 + 0x2C) as *mut u32;
const LTDC_L2CFBLNR: *mut u32 = (LTDC_L2 + 0x30) as *mut u32;
const LTDC_L2CLUTWR: *mut u32 = (LTDC_L2 + 0x34) as *mut u32;
const LTDC_SRCR: *const u32 = 0x5000_1024 as *const u32;

/// A8 text buffer for LTDC Layer 2. Must be in SDRAM (AXI-accessible)
/// since LTDC reads via AXI, not AHB. D2 SRAM at 0x3000_0000 is
/// AHB-only and invisible to LTDC. Uses the old layer_buf region.
const A8_L2_ADDR: u32 = 0xD180_0000;
const A8_WIDTH: u32 = 480;
const A8_HEIGHT: u32 = 600;
/// Portrait row offset where the A8 region starts (centering the
/// 600-row text region within the 800-row display).
const A8_Y_BASE: u32 = 100; // (800 - 600) / 2

/// Configure LTDC Layer 2 as an L8+CLUT overlay for the A8 text
/// buffer. Each L8 pixel value maps via CLUT to yellow with
/// proportional alpha. LTDC hardware-blends this over the
/// starfield in Layer 1 during scan — zero DMA2D.
///
/// # Safety
/// Must be called after LTDC is initialized (Layer 1 active).
unsafe fn setup_ltdc_layer2_a8(panel_w: u16, panel_h: u16) {
    unsafe {
        // Use the same timing parameters as Layer 1.
        // HSW=2, HBP=34, VSW=120, VBP=150 (from display_init)
        let hsw: u32 = 2;
        let hbp: u32 = 34;
        let vsw: u32 = 120;
        let vbp: u32 = 150;

        // Layer 2 window covers only the text region (A8_Y_BASE
        // to A8_Y_BASE + A8_HEIGHT) of the full display.
        let x0 = hsw + hbp + 1;
        let x1 = x0 + panel_w as u32 - 1;
        let y0 = vsw + vbp + 1 + A8_Y_BASE;
        let y1 = y0 + A8_HEIGHT - 1;

        LTDC_L2WHPCR.write_volatile((x1 << 16) | x0);
        LTDC_L2WVPCR.write_volatile((y1 << 16) | y0);

        // ARGB8888 pixel format (0). The A8→ARGB expansion in
        // the render loop writes (alpha << 24) | 0x00FFD700 for
        // each pixel — yellow at the FIR-computed alpha.
        LTDC_L2PFCR.write_volatile(0); // ARGB8888
        LTDC_L2CACR.write_volatile(255); // constant alpha = opaque
        LTDC_L2DCCR.write_volatile(0); // default color = transparent
        // Blending: BF1=PAxCA (0x06), BF2=1-PAxCA (0x07)
        // This blends Layer 2 (text) over Layer 1 (starfield)
        // using the per-pixel alpha from the CLUT.
        LTDC_L2BFCR.write_volatile(0x0607);

        // ARGB8888 buffer in SDRAM
        LTDC_L2CFBAR.write_volatile(A8_L2_ADDR);
        let pitch = A8_WIDTH * 4; // 4 bytes per pixel for ARGB8888
        LTDC_L2CFBLR.write_volatile((pitch << 16) | (pitch + 7));
        LTDC_L2CFBLNR.write_volatile(A8_HEIGHT);

        // Load CLUT: 256 entries. Entry[i] = alpha=i, yellow.
        // CLUTWR format: [31:24]=CLUTADD, [23:16]=R, [15:8]=G, [7:0]=B
        // But CLUT entries provide RGB; alpha comes from CACR or
        // the pixel value maps to a full ARGB via the CLUT.
        //
        // For L8: pixel value selects CLUT index. CLUT entry
        // provides RGB. The pixel value IS the alpha (via CLUT
        // alpha channel in ARGB CLUT mode — but STM32H7 LTDC
        // CLUT is RGB888 only, no alpha per entry).
        //
        // Workaround: treat L8 value as both color index AND alpha.
        // CLUT[i] = yellow (same for all i). The L8 pixel value
        // drives constant-alpha multiplication via BF1/BF2.
        //
        // Actually, for L8 with blending: the pixel value IS the
        // color index. Alpha is from CACR (constant = 255). To get
        // per-pixel alpha, we need AL88 format — but that's 2 bytes.
        //
        // Better approach: skip CLUT. Use ARGB8888 format for Layer 2
        // but point it at a small pre-blended buffer. OR use the
        // default color + alpha approach.
        //
        // Simplest: just fill the CLUT with yellow at varying alpha.
        // Each CLUT entry = RGB(0xFF, 0xD7, 0x00). The L8 pixel
        // value selects the entry (all same color). Per-pixel alpha
        // isn't supported via L8 CLUT on STM32H7 — CLUT entries
        // are RGB only, alpha comes from CACR.
        //
        // The real solution: convert A8 to ARGB8888 with CPU or
        // DMA2D (small, A8_WIDTH × A8_HEIGHT × 4 = 1.15MB) into a
        // scratch buffer, use that as Layer 2 source. But that's
        // DMA2D per frame again.
        //
        // OR: Use AL88 format (format 7). Each pixel is 16 bits:
        // upper 8 = alpha, lower 8 = luminance. Set constant color
        // to yellow. But our A8 buffer is 8-bit, not 16-bit.
        //
        // For now: load CLUT with yellow entries. Alpha blending
        // uses CACR × pixel_alpha. With L8, pixel_alpha isn't
        // available. We get solid yellow overlay where A8 > 0.
        //
        // TODO: convert to AL88 or ARGB for proper alpha gradients.
        // Enable Layer 2: LEN (bit 0). No SRCR here — present_task's
        // next retrigger does shadow reload, avoiding the race where
        // present's SRCR fires mid-setup and loads partial config.
        LTDC_L2CR.write_volatile(0x01); // LEN
    }
}

/// Disable LTDC Layer 2.
unsafe fn disable_ltdc_layer2() {
    unsafe {
        LTDC_L2CR.write_volatile(0); // LEN = 0
        (0x5000_1024 as *mut u32).write_volatile(1); // SRCR.IMR
    }
}

// ── D3 SRAM heartbeat breadcrumbs ─────────────────────────────────────────────
//
// Each task increments a distinct 32-bit slot in SRAM3 (0x3800_0000 region) so
// a debugger can confirm the scheduler is actually running each one. These are
// used as "signs of life" before the real present/render/touch bodies land.
//
// Layout:
//   0x3800_0700 — present_task tick count
//   0x3800_0704 — render_task  tick count
//   0x3800_0708 — touch_task   tick count
//   0x3800_070C — present_task ERIF wake count (sem-take successes)
//   0x3800_0710 — touch_task non-empty sample count (FT5336 touches seen)
//   0x3800_0714 — touch_task last reported count+event_flag+x+y packed
//   0x3800_0718 — last render frame pixels-touched count
//   0x3800_071C — last render frame rect count
//   0x3800_0720 — last render frame cycle count (CpuBlitter cost)
//   0x3800_0724 — cumulative pixels-touched  (for fps * pixels calc)

const HB_PRESENT_TICKS: *mut u32 = 0x3800_0700 as *mut u32;
const HB_RENDER_TICKS: *mut u32 = 0x3800_0704 as *mut u32;
const HB_TOUCH_TICKS: *mut u32 = 0x3800_0708 as *mut u32;
const HB_ERIF_WAKES: *mut u32 = 0x3800_070C as *mut u32;
const HB_TOUCH_HITS: *mut u32 = 0x3800_0710 as *mut u32;
const HB_TOUCH_LAST: *mut u32 = 0x3800_0714 as *mut u32;
const HB_RENDER_PIXELS: *mut u32 = 0x3800_0718 as *mut u32;
const HB_RENDER_RECTS: *mut u32 = 0x3800_071C as *mut u32;
const HB_RENDER_CYC: *mut u32 = 0x3800_0720 as *mut u32;
const HB_RENDER_PX_TOT: *mut u32 = 0x3800_0724 as *mut u32;
const HB_CRAWL_FRAMEID: *mut u32 = 0x3800_0728 as *mut u32;
const HB_CRAWL_TICKS:   *mut u32 = 0x3800_072C as *mut u32;
const HB_CRAWL_READY:   *mut u32 = 0x3800_0730 as *mut u32;
const HB_PLAYIT_POLLS:  *mut u32 = 0x3800_0734 as *mut u32;

// ── Cross-task touch ring (touch_task → render_task) ──────────────────────────
//
// Lock-free SPSC: touch_task is the sole producer, render_task the sole
// consumer. Atomic head/tail with Acquire/Release ordering.

use crate::touch_i2c::RawTouchSample;

const TOUCH_EVT_CAP: usize = 16;
static TOUCH_EVT_HEAD: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static TOUCH_EVT_TAIL: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static mut TOUCH_EVT_SLOTS: [RawTouchSample; TOUCH_EVT_CAP] =
    [RawTouchSample::EMPTY; TOUCH_EVT_CAP];

fn touch_evt_push(s: RawTouchSample) {
    let head = TOUCH_EVT_HEAD.load(Ordering::Relaxed);
    let tail = TOUCH_EVT_TAIL.load(Ordering::Acquire);
    let next = (head + 1) % TOUCH_EVT_CAP as u32;
    if next == tail { return; } // full — drop
    unsafe { TOUCH_EVT_SLOTS[head as usize] = s; }
    TOUCH_EVT_HEAD.store(next, Ordering::Release);
}

fn touch_evt_pop() -> Option<RawTouchSample> {
    let tail = TOUCH_EVT_TAIL.load(Ordering::Relaxed);
    let head = TOUCH_EVT_HEAD.load(Ordering::Acquire);
    if tail == head { return None; }
    let s = unsafe { TOUCH_EVT_SLOTS[tail as usize] };
    TOUCH_EVT_TAIL.store((tail + 1) % TOUCH_EVT_CAP as u32, Ordering::Release);
    Some(s)
}

/// Shared flag — playit `C` command sets this; render task reads to
/// toggle the star crawl on/off.
pub static CRAWL_REQ: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// When true, touch_task yields instead of polling I2C4 — lets the F
/// command get clean bus access.
static TOUCH_PAUSE: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Set by `render_task` while star_crawl is producing frames. Read
/// by `present_task` — when active, present holds each swapped
/// frame for exactly two present cycles (≈ 66 ms at the 30 Hz panel
/// rate) instead of one.
///
/// Rationale: a crawl frame takes ~55 ms while present cycles at
/// 33 ms. At a non-integer 1.67:1 ratio, `buf_ready` polls on
/// different present cycles produce an irregular swap pattern
/// `[yes, no, yes, no, yes, yes, no, ...]` — each swap has a
/// hold-time jitter of one present cycle, visible as crawl flicker.
/// Pacing the swap rate down to every-other cycle gives a clean 15
/// fps with constant 66 ms hold per frame, phase-locked to the
/// panel.
pub static CRAWL_ACTIVE: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// When non-zero, present_task retriggers with this address instead
/// of FRONT_FB_ADDR. Set by render_task to point LTDC directly at
/// the jumbo/starfield buffer at the current scroll offset. Zero
/// DMA2D per frame — just a CFBAR register write.
static CRAWL_FB_ADDR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

#[inline(always)]
fn cyccnt() -> u32 {
    const DWT_CYCCNT: *const u32 = 0xE000_1004 as *const u32;
    unsafe { DWT_CYCCNT.read_volatile() }
}

#[inline(always)]
unsafe fn hb_inc(addr: *mut u32) {
    unsafe {
        let v = addr.read_volatile().wrapping_add(1);
        addr.write_volatile(v);
    }
}

unsafe extern "C" fn present_task(_arg: *mut core::ffi::c_void) {
    use core::sync::atomic::Ordering;
    use rlvgl_platform::frame_sync::FrameSync;

    // Counts cycles since the last successful swap. Used together
    // with CRAWL_ACTIVE to enforce a minimum 2-cycle hold per
    // swapped frame when star_crawl is driving render — see the
    // CRAWL_ACTIVE doc for the rationale.
    let mut cycles_since_swap: u8 = 0;

    // Kick the first scan so ERIF can fire. Without this the display
    // would stay idle because ERIF is a scan-complete signal — we need
    // a scan to complete before the first semaphore give.
    let fb = FRONT_FB_ADDR.load(Ordering::Acquire);
    if fb != 0 {
        unsafe { ltdc_retrigger(fb) };
    }

    loop {
        unsafe { hb_inc(HB_PRESENT_TICKS) };

        let Some(sync) = get_sync() else {
            unsafe { vTaskDelay(100) };
            continue;
        };

        // Block until DSI ERIF fires. portMAX_DELAY = wait forever.
        if sync.wait_erif(freertos_sync::portMAX_DELAY) {
            unsafe { hb_inc(HB_ERIF_WAKES) };
        }

        // Holdoff: present the next frame precisely PRESENT_HOLDOFF
        // after ERIF, phase-locked to the panel's TE. See the TIM7
        // block at the top of this module for the rationale — every
        // frame must land at the same DWT offset or we beat against
        // TE and get intermittent flicker.
        //
        // We arm TIM7 (one-pulse, 1 MHz) for `PRESENT_HOLDOFF -
        // elapsed_since_erif` µs and block on `present_gate_sem`.
        // TIM7's UIF ISR gives the sem at the exact deadline. Zero
        // busy-spin, preemption-friendly, ERIF-phase-locked.
        const PRESENT_HOLDOFF_CYC: u32 = 12_800_000; // 32 ms @ 400 MHz — settings wing (5 icons) needs more render time
        const CYC_PER_US: u32 = 400; //  400 MHz / 1 MHz
        let elapsed = sync.cycles_since_erif();
        if elapsed < PRESENT_HOLDOFF_CYC {
            let remaining_us = (PRESENT_HOLDOFF_CYC - elapsed) / CYC_PER_US;
            if remaining_us > 0 {
                unsafe { tim7_arm(remaining_us) };
                let gate = PRESENT_GATE_SEM.load(Ordering::Acquire);
                if !gate.is_null() {
                    // Take with a 50 ms timeout so a misconfigured
                    // or stuck TIM7 doesn't wedge the present task
                    // permanently; 50 ms is > the 15 ms holdoff and
                    // the longest plausible panel frame period.
                    unsafe { freertos_sync::rlvgl_sem_take(gate, 50) };
                }
            }
        }

        // Non-blocking buf_ready for ALL modes. If a new frame is
        // ready, swap FRONT/BACK. If not, retrigger the same FRONT
        // (frame repeat). This keeps the ERIF cycle alive at 30 Hz
        // so render_start_sem fires every porch — essential when
        // compose spans multiple porches via compose_tick.
        let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
        if !buf_ready.is_null()
            && unsafe { freertos_sync::rlvgl_sem_take(buf_ready, 0) }
                == freertos_sync::pdTRUE
        {
            let front = FRONT_FB_ADDR.load(Ordering::Acquire);
            let back = BACK_FB_ADDR.load(Ordering::Acquire);
            FRONT_FB_ADDR.store(back, Ordering::Release);
            BACK_FB_ADDR.store(front, Ordering::Release);
            cycles_since_swap = 0;
        } else {
            cycles_since_swap = cycles_since_swap.saturating_add(1);
        }
        // DMA2D safety gate — compose_tick is timer-gated so DMA2D
        // should be idle by retrigger time. This is a safety net.
        {
            const DMA2D_CR: *const u32 = 0x5200_1000 as *const u32;
            const CR_START: u32 = 1 << 0;
            let mut wait_ticks: u32 = 0;
            while unsafe { DMA2D_CR.read_volatile() } & CR_START != 0 {
                wait_ticks += 1;
                if wait_ticks > 30 { break; }
                unsafe { vTaskDelay(1) };
            }
        }

        // Choose retrigger address: CRAWL_FB_ADDR when crawl is
        // driving (points LTDC directly at the jumbo/starfield
        // buffer — zero DMA2D per frame), else FRONT_FB_ADDR.
        let crawl_fb = CRAWL_FB_ADDR.load(Ordering::Acquire);
        let fb = if crawl_fb != 0 {
            crawl_fb
        } else {
            FRONT_FB_ADDR.load(Ordering::Acquire)
        };
        if fb != 0 {
            sync.scan_complete.store(false, Ordering::Release);
            unsafe { ltdc_retrigger(fb) };
            rlvgl_platform::frame_sync::ScopeProbe::ltdc_active(sync);
        }
        // render_start_sem is given by the DSI ISR on ERIF so
        // render wakes during the back porch, not after retrigger.
    }
}

unsafe extern "C" fn render_task(_arg: *mut core::ffi::c_void) {
    use alloc::boxed::Box;
    use core::sync::atomic::Ordering;
    use rlvgl_core::packed_font::PackedFont;
    use rlvgl_platform::blit::{PixelFmt, Rect, Surface};
    use rlvgl_platform::cpu_blitter::CpuBlitter;
    use rlvgl_platform::dma2d::Dma2dBlitter;

    use crate::freertos_layers::{Compositor, MotionBlockLayer, SolidBackgroundLayer};
    use crate::star_crawl::{self, RenderMode, StarCrawl, StepResult};

    const IDLE_PERIOD_MS: u32 = 16; // ~62 Hz idle compositor demo

    // Bold font and README crawl text — mirrors the bare-metal and
    // Zephyr setup so FreeRTOS renders the same content.
    static BOLD_FONT_DATA: &[u8] =
        include_bytes!("../assets/fonts/DejaVuSans-Bold-32.bin");
    static BOLD_FONT: PackedFont = PackedFont {
        height: 32,
        ascent: 30,
        glyphs: &crate::fonts::DEJAVU_SANS_BOLD_32_GLYPHS,
        data: BOLD_FONT_DATA,
    };

    let mut compositor: Option<Compositor> = None;
    let mut cpu_blitter = CpuBlitter;
    let mut dma2d: Option<Dma2dBlitter> = None;
    let mut crawl: Option<StarCrawl> = None;

    loop {
        unsafe { hb_inc(HB_RENDER_TICKS) };

        // ── ERIF/swap gate ────────────────────────────────────────
        // Block until present_task has completed its swap + retrigger
        // for the current frame. present gives `render_start_sem` at
        // the END of its loop body (after ltdc_retrigger), so when
        // we resume here the BACK atomic points at a buffer that is
        // off-screen and safe to write. This gate applies to BOTH
        // the crawl path and the idle compositor path — without it,
        // both can race the front/back swap and produce tearing.
        //
        // The 100-ms timeout is defensive — if present stalls, we
        // still loop to re-check lazy-init paths rather than
        // deadlocking.
        {
            let rs = RENDER_START_SEM.load(Ordering::Acquire);
            if !rs.is_null() {
                unsafe { freertos_sync::rlvgl_sem_take(rs, 100) };
            } else {
                unsafe { vTaskDelay(IDLE_PERIOD_MS) };
            }
        }

        let back = BACK_FB_ADDR.load(Ordering::Acquire);
        let w = FB_W.load(Ordering::Acquire);
        let h = FB_H.load(Ordering::Acquire);
        let bytes = FB_BYTES.load(Ordering::Acquire);

        if back == 0 || w == 0 || h == 0 || bytes == 0 {
            unsafe { vTaskDelay(IDLE_PERIOD_MS) };
            continue;
        }

        // Wait for the sync object (gives us dma2d_done_sem for TCIE
        // ack) before claiming the DMA2D peripheral.
        if get_sync().is_none() {
            unsafe { vTaskDelay(IDLE_PERIOD_MS) };
            continue;
        }

        // Lazy-init DMA2D blitter — owned directly so `star_crawl.tick`
        // can borrow &mut. We enable the TC interrupt so the FreeRTOS
        // DMA2D ISR gives `dma2d_done_sem`, which the sync trait
        // (`take_complete`) drains between tick calls.
        if dma2d.is_none() {
            let mut b = unsafe { Dma2dBlitter::steal() };
            b.enable_tc_interrupt();
            dma2d = Some(b);
        }

        // Lazy-init StarCrawl — uses the same bold font + README text
        // as bare-metal / Zephyr. Incremental mode is the efficient
        // path (per-frame M2M shift + narrow fill-in) and requires a
        // persistent layer buffer — 0xD180_0000 matches Zephyr.
        if crawl.is_none() {
            let mut c = StarCrawl::new(&BOLD_FONT, crate::readme_crawl::README_CRAWL, 30);
            c.set_render_mode(RenderMode::Incremental);
            crawl = Some(c);
        }

        // No initial fill — bare-metal init decoded the splash into
        // FRONT. The widget tree renders into BACK on every idle frame.

        let dma = dma2d.as_mut().unwrap();
        let cr = crawl.as_mut().unwrap();

        // ── Toggle handshake with playit_task ─────────────────────
        let req = CRAWL_REQ.swap(false, Ordering::AcqRel);
        if req {
            if cr.is_active() {
                cr.deactivate();
                CRAWL_FB_ADDR.store(0, Ordering::Release);
                unsafe { disable_ltdc_layer2() };
            } else {
                cr.activate(dma);
                // Drain stale buf_ready
                let br = BUF_READY_SEM.load(Ordering::Acquire);
                if !br.is_null() {
                    unsafe { freertos_sync::rlvgl_sem_take(br, 0) };
                }
                // Zero the Layer 2 ARGB buffer before enabling —
                // stale SDRAM data with non-zero alpha would cover
                // the starfield. 480×600×4 = 1.15 MB.
                unsafe {
                    core::ptr::write_bytes(
                        0xD180_0000 as *mut u8,
                        0,
                        480 * 600 * 4,
                    );
                }
                cortex_m::asm::dsb();
                // Enable LTDC Layer 2 for ARGB8888 text overlay.
                unsafe { setup_ltdc_layer2_a8(w as u16, h as u16) };
            }
        }
        CRAWL_ACTIVE.store(cr.is_active(), Ordering::Release);

        // ── Crawl mode: two-phase pipeline ───────────────────────
        //
        // Phase A runs FIRST on every ERIF — compose starfield +
        // text into BACK. 1-2 DMA2D blits (~2ms total), fully in
        // the back porch since render_start_sem fires on ERIF and
        // present is blocked on TIM7.
        //
        // Phase B runs in remaining time — CPU FIR text prep for
        // the NEXT frame. If Phase B can't finish before the next
        // ERIF, it yields; Phase A still runs on the next ERIF
        // (composing whatever text is ready so far). This prevents
        // Phase B from starving Phase A and pushing DMA2D into the
        // scan window.
        if cr.is_active() {
            // Touch-to-dismiss: any touch while crawl is active
            // deactivates it (matches bare-metal behavior).
            if touch_evt_pop().is_some() {
                // Drain remaining events
                while touch_evt_pop().is_some() {}
                cr.deactivate();
                CRAWL_FB_ADDR.store(0, Ordering::Release);
                unsafe { disable_ltdc_layer2() };
                CRAWL_ACTIVE.store(false, Ordering::Release);
                // Force desktop re-render
                continue;
            }

            let sync = get_sync().unwrap();

            // ── Jumbo/CFBAR model: zero DMA2D per frame ──────────
            //
            // Point LTDC directly at the starfield source buffer at
            // the current scroll offset. Present reads CRAWL_FB_ADDR
            // and retriggers with it — just a register write, no
            // DMA2D blit to the display buffer at all. LTDC reads
            // 800 contiguous rows from the starfield.
            //
            // The starfield is 1600 rows (double-height, mirrored).
            // As long as star_row + 800 <= 1600, the source is
            // contiguous and LTDC reads correctly.
            const CRAWL_BASE: usize = 0xD100_0000;
            const STAR_STRIDE: u32 = 480 * 4; // FB_W * BPP
            const STAR_ROWS: u32 = 1600;
            const FB_H_CONST: u32 = 800;

            let star_row =
                ((cr.star_scroll_q8() >> 8) as u32) % STAR_ROWS;

            // Clamp: if viewport would wrap, pin to last safe row
            let safe_row = if star_row + FB_H_CONST <= STAR_ROWS {
                star_row
            } else {
                STAR_ROWS - FB_H_CONST
            };

            let cfbar = CRAWL_BASE as u32 + safe_row * STAR_STRIDE;
            CRAWL_FB_ADDR.store(cfbar, Ordering::Release);

            unsafe {
                HB_CRAWL_FRAMEID.write_volatile(cr.frame_id());
                hb_inc(HB_CRAWL_READY);
            }

            // Phase B: advance scroll + CPU FIR text into D2 SRAM
            // A8 buffer. After prep completes, copy to SDRAM for
            // LTDC Layer 2 to read.
            let mut prep_done = false;
            for _ in 0..480u32 {
                unsafe { hb_inc(HB_CRAWL_TICKS) };
                match cr.prep_next_frame(dma, sync) {
                    StepResult::FrameReady => {
                        prep_done = true;
                        break;
                    }
                    StepResult::Pending => {}
                    StepResult::Finished => {
                        cr.deactivate();
                        CRAWL_FB_ADDR.store(0, Ordering::Release);
                        unsafe { disable_ltdc_layer2() };
                        break;
                    }
                    StepResult::Idle => break,
                }
            }
            // Expand A8 → ARGB8888 from D2 SRAM → SDRAM for Layer 2.
            // Each A8 byte → (alpha << 24) | 0x00FFD700 (yellow).
            // 480×600 pixels = 288K reads + 1.15M writes, ~3ms CPU.
            if prep_done {
                const A8_SRC: *const u8 = 0x3000_0000 as *const u8;
                const ARGB_DST: *mut u32 = 0xD180_0000 as *mut u32;
                const PIXEL_COUNT: usize = 480 * 600;
                const ARGB_BYTES: usize = PIXEL_COUNT * 4;
                const YELLOW_RGB: u32 = 0x00FF_D700;
                unsafe {
                    for i in 0..PIXEL_COUNT {
                        let alpha = *A8_SRC.add(i) as u32;
                        *ARGB_DST.add(i) = (alpha << 24) | YELLOW_RGB;
                    }
                }
                // D-cache clean: CPU writes are in write-back cache;
                // LTDC reads SDRAM via AXI (bypasses D-cache). Without
                // clean, LTDC sees stale/zero data.
                {
                    let mut cp = unsafe { cortex_m::Peripherals::steal() };
                    cp.SCB.clean_dcache_by_address(
                        0xD180_0000usize,
                        ARGB_BYTES,
                    );
                }
                cortex_m::asm::dsb();
            }

            continue;
        }

        // DMA2D staging blit deferred — using buf_ready swap for now.

        // ── Desktop mode: widget tree render ─────────────────────
        {
            use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoController};
            use rlvgl_platform::blit::{BlitterRenderer, RotatedRenderer};
            use rlvgl_platform::screen::Screen;

            static mut DESKTOP_CTRL: Option<DiscoController> = None;
            static mut DESKTOP_DIAG: bool = false;
            // Dirty-frame counter: >0 = render + swap, 0 = idle (present
            // repeats FRONT). Start at 4 for initial convergence.
            // Touch events set this to 20 (~660ms at 30Hz) to cover
            // wing slide + double-buffer convergence.
            static mut DIRTY_FRAMES: u8 = 4;
            if unsafe { (*core::ptr::addr_of!(DESKTOP_CTRL)).is_none() } {
                crate::runtime_serial::write_bytes(b"RND:ctrl_new\r\n");
                crate::runtime_serial::kick_tx();
                let screen = Screen::landscape(800, 480);
                let ctrl = DiscoController::new(
                    screen,
                    DiscoCapabilities::stm32h747i_disco(),
                );
                unsafe { *core::ptr::addr_of_mut!(DESKTOP_CTRL) = Some(ctrl) };
                crate::runtime_serial::write_bytes(b"RND:ctrl_ok\r\n");
                crate::runtime_serial::kick_tx();
            }

            let ctrl = unsafe { &mut *core::ptr::addr_of_mut!(DESKTOP_CTRL) }
                .as_mut()
                .unwrap();

            // Track whether a gesture changed widget state — only
            // then do we pristine-restore before draw.
            static mut NEEDS_PRISTINE: bool = false;

            use rlvgl_core::event::Event;

            // ── Poll button (PC13, active-high) ──
            {
                const GPIOC_IDR: *const u32 = 0x5802_0810 as *const u32;
                static mut BTN_LAST: bool = false;
                let pressed = unsafe { GPIOC_IDR.read_volatile() } & (1 << 13) != 0;
                let last = unsafe { BTN_LAST };
                if pressed != last {
                    unsafe { BTN_LAST = pressed; }
                    let evt = if pressed {
                        Event::KeyDown { key: rlvgl_core::event::Key::Enter }
                    } else {
                        Event::KeyUp { key: rlvgl_core::event::Key::Enter }
                    };
                    unsafe {
                        // Enter changes panels — needs pristine.
                        // Arrows just move focus — draw-only suffices.
                        if matches!(evt, Event::KeyDown { key: rlvgl_core::event::Key::Enter }) {
                            core::ptr::write_volatile(core::ptr::addr_of_mut!(NEEDS_PRISTINE), true);
                        }
                        core::ptr::write_volatile(core::ptr::addr_of_mut!(DIRTY_FRAMES), 1);
                    }
                    ctrl.dispatch_event(&evt);
                }
            }

            // ── Poll joystick (PK2-PK6, active-low, pull-up) ──
            {
                const GPIOK_IDR: *const u32 = 0x5802_2810 as *const u32;
                static mut JOY_LAST: [bool; 5] = [false; 5];
                let idr = unsafe { GPIOK_IDR.read_volatile() };
                let pins = [
                    idr & (1 << 2) == 0, // PK2 = SEL/Enter
                    idr & (1 << 3) == 0, // PK3 = Down
                    idr & (1 << 4) == 0, // PK4 = Left
                    idr & (1 << 5) == 0, // PK5 = Right
                    idr & (1 << 6) == 0, // PK6 = Up
                ];
                use rlvgl_core::event::Key;
                const KEYS: [Key; 5] = [
                    Key::Enter,
                    Key::ArrowDown,
                    Key::ArrowLeft,
                    Key::ArrowRight,
                    Key::ArrowUp,
                ];
                for i in 0..5 {
                    let last = unsafe { JOY_LAST[i] };
                    if pins[i] != last {
                        unsafe { JOY_LAST[i] = pins[i]; }
                        // Only dispatch KeyDown (press edge).
                        // KeyUp doesn't change visual state.
                        if pins[i] {
                            let evt = Event::KeyDown { key: KEYS[i].clone() };
                            unsafe {
                                if matches!(KEYS[i], Key::Enter) {
                                    core::ptr::write_volatile(core::ptr::addr_of_mut!(NEEDS_PRISTINE), true);
                                }
                                core::ptr::write_volatile(core::ptr::addr_of_mut!(DIRTY_FRAMES), 1);
                            }
                            ctrl.dispatch_event(&evt);
                        }
                    }
                }
            }

            // ── Drain touch events and dispatch to widget tree ──
            {
                // Print ring h/t every 150 iterations (~5s at 30Hz)
                // to catch state changes after user taps.
                static mut DESKTOP_LOOP_DIAG: u32 = 0;
                unsafe {
                    let c = core::ptr::read_volatile(core::ptr::addr_of!(DESKTOP_LOOP_DIAG));
                    core::ptr::write_volatile(core::ptr::addr_of_mut!(DESKTOP_LOOP_DIAG), c + 1);
                    if c % 150 == 0 && c <= 600 {
                        let h = TOUCH_EVT_HEAD.load(Ordering::Relaxed);
                        let t = TOUCH_EVT_TAIL.load(Ordering::Relaxed);
                        let mut out = [0u8; 40];
                        let mut p = 0;
                        p = write_slice(&mut out, p, b"DL:");
                        p = write_u32(&mut out, p, c);
                        p = write_slice(&mut out, p, b" h=");
                        p = write_u32(&mut out, p, h);
                        p = write_slice(&mut out, p, b",t=");
                        p = write_u32(&mut out, p, t);
                        p = write_slice(&mut out, p, b"\r\n");
                        crate::runtime_serial::write_bytes(&out[..p]);
                        crate::runtime_serial::kick_tx();
                    }
                }

                use rlvgl_core::event::Event;
                use rlvgl_platform::gesture::{DoubleTapRecognizer, TapRecognizer};

                static mut TAP: Option<TapRecognizer> = None;
                static mut DTAP: Option<DoubleTapRecognizer> = None;
                static mut LAST_TOUCH: Option<(u16, u16)> = None;
                static mut LAST_COUNT: u8 = 0;

                let tap = unsafe {
                    let p = core::ptr::addr_of_mut!(TAP);
                    if (*p).is_none() { *p = Some(TapRecognizer::new(30)); }
                    (*p).as_mut().unwrap()
                };
                let dtap = unsafe {
                    let p = core::ptr::addr_of_mut!(DTAP);
                    if (*p).is_none() { *p = Some(DoubleTapRecognizer::new(30)); }
                    (*p).as_mut().unwrap()
                };

                const DW: i32 = 480; // portrait width (RotatedRenderer Y axis)

                static mut TOUCH_DRAIN_DIAG: u32 = 0;
                while let Some(sample) = touch_evt_pop() {
                    unsafe {
                        let c = core::ptr::read_volatile(core::ptr::addr_of!(TOUCH_DRAIN_DIAG));
                        if c < 3 {
                            core::ptr::write_volatile(core::ptr::addr_of_mut!(TOUCH_DRAIN_DIAG), c + 1);
                            let mut out = [0u8; 24];
                            let mut p = 0;
                            p = write_slice(&mut out, p, b"TD:");
                            p = write_u32(&mut out, p, sample.count as u32);
                            p = write_slice(&mut out, p, b"\r\n");
                            crate::runtime_serial::write_bytes(&out[..p]);
                            crate::runtime_serial::kick_tx();
                        }
                    }
                    let count = sample.count;
                    let raw = &sample.points;

                    // Portrait→landscape transform + state machine
                    let evt = if count >= 2 {
                        use rlvgl_core::event::{
                            MAX_TOUCH_POINTS, TouchPoint,
                            TouchState as EvtTouchState,
                        };
                        let mut points = [TouchPoint::default(); MAX_TOUCH_POINTS];
                        for i in 0..count as usize {
                            let (id, flag, x, y) = raw[i];
                            points[i] = TouchPoint {
                                id,
                                x: y as i32,
                                y: DW - 1 - x as i32,
                                state: match flag {
                                    0 => EvtTouchState::Down,
                                    1 => EvtTouchState::Up,
                                    _ => EvtTouchState::Contact,
                                },
                            };
                        }
                        let (_, _, x0, y0) = raw[0];
                        unsafe { LAST_TOUCH = Some((x0, y0)); LAST_COUNT = count; }
                        Some(Event::Touch { count, points })
                    } else {
                        let touch = if count == 1 {
                            let (_, _, x, y) = raw[0];
                            Some((x, y))
                        } else {
                            None
                        };
                        let was_multi = unsafe { LAST_COUNT } >= 2;
                        let last = unsafe { LAST_TOUCH };
                        unsafe { LAST_COUNT = count; }
                        let to_l = |px: u16, py: u16| (py as i32, DW - 1 - px as i32);
                        match (touch, last) {
                            (Some((x, y)), Some((lx, ly))) => {
                                unsafe { LAST_TOUCH = Some((x, y)); }
                                if was_multi {
                                    let (lx, ly) = to_l(x, y);
                                    Some(Event::PointerDown { x: lx, y: ly })
                                } else if (x, y) != (lx, ly) {
                                    let (lx, ly) = to_l(x, y);
                                    Some(Event::PointerMove { x: lx, y: ly })
                                } else {
                                    None
                                }
                            }
                            (Some((x, y)), None) => {
                                unsafe { LAST_TOUCH = Some((x, y)); }
                                let (lx, ly) = to_l(x, y);
                                Some(Event::PointerDown { x: lx, y: ly })
                            }
                            (None, Some((lx, ly))) => {
                                unsafe { LAST_TOUCH = None; }
                                let (lx, ly) = to_l(lx, ly);
                                Some(Event::PointerUp { x: lx, y: ly })
                            }
                            (None, None) => None,
                        }
                    };

                    if let Some(ref evt) = evt {
                        // Gesture pipeline — only dispatch action
                        // events (PressRelease/DoubleTap) to root,
                        // and only when in an interactive zone.
                        // ActionHotspot doesn't check bounds, so
                        // stray touches fire the first hotspot.
                        if let Some(gesture) = tap.process(evt) {
                            let (a, b) = dtap.process(&gesture);
                            for g in a.into_iter().chain(b) {
                                let in_zone = match &g {
                                    Event::PressRelease { x, .. }
                                    | Event::DoubleTap { x, .. } => {
                                        // Icon strip: right edge (x≥700)
                                        // Wing area: left edge (x<80)
                                        *x >= 700 || *x < 80
                                    }
                                    _ => false,
                                };
                                if in_zone {
                                    unsafe {
                                        core::ptr::write_volatile(
                                            core::ptr::addr_of_mut!(DIRTY_FRAMES),
                                            1,
                                        );
                                    }
                                    // Touch PressRelease dispatch disabled —
                                    // ActionHotspot bounds bug. Use joystick.
                                    let _ = &g;
                                }
                            }
                        }
                    }
                }

                // Advance gesture timers — zone-gated dispatch.
                if let Some(gesture) = tap.tick() {
                    let (a, b) = dtap.process(&gesture);
                    for g in a.into_iter().chain(b) {
                        let in_zone = match &g {
                            Event::PressRelease { x, .. }
                            | Event::DoubleTap { x, .. } => *x >= 700 || *x < 80,
                            _ => false,
                        };
                        if in_zone {
                            unsafe {
                                core::ptr::write_volatile(
                                    core::ptr::addr_of_mut!(DIRTY_FRAMES),
                                    1,
                                );
                            }
                            ctrl.root().borrow_mut().dispatch_event(&g);
                        }
                    }
                }
                if let Some(gesture) = dtap.tick() {
                    let in_zone = match &gesture {
                        Event::PressRelease { x, .. }
                        | Event::DoubleTap { x, .. } => *x >= 700 || *x < 80,
                        _ => false,
                    };
                    if in_zone {
                        unsafe {
                            core::ptr::write_volatile(
                                core::ptr::addr_of_mut!(DIRTY_FRAMES),
                                1,
                            );
                        }
                        // Touch PressRelease dispatch disabled —
                        // ActionHotspot bounds bug. Use joystick.
                        let _ = &gesture;
                    }
                }
            }

            // Tick controller: advances tick_count (live stats),
            // re-renders active info page, syncs focus highlights.
            ctrl.tick();

            // Slow periodic refresh (~3s) for live stats updates.
            // Faster rates cause visible flicker on settings wing
            // (5 icon RLE decode is expensive).
            static mut REFRESH_COUNTER: u8 = 0;
            unsafe {
                let rc = core::ptr::read_volatile(core::ptr::addr_of!(REFRESH_COUNTER));
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!(REFRESH_COUNTER),
                    rc.wrapping_add(1),
                );
                if rc == 0 { // wraps every 256 ticks ≈ 14s at 18Hz
                    core::ptr::write_volatile(
                        core::ptr::addr_of_mut!(DIRTY_FRAMES),
                        core::ptr::read_volatile(
                            core::ptr::addr_of!(DIRTY_FRAMES),
                        ).max(1),
                    );
                }
            }

            // Drain platform commands from the controller.
            // Any command means UI state changed — need re-render.
            {
                let cmds = ctrl.drain_commands();
                if !cmds.is_empty() {
                    unsafe {
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!(DIRTY_FRAMES),
                            core::ptr::read_volatile(
                                core::ptr::addr_of!(DIRTY_FRAMES),
                            ).max(1),
                        );
                    }
                }
                for cmd in cmds {
                    use rlvgl_app_disco_demo::{DiscoCommand, DiscoEffect};
                    match cmd {
                        DiscoCommand::StartEffect(DiscoEffect::StarCrawl) => {
                            CRAWL_REQ.store(true, Ordering::Release);
                        }
                        _ => {}
                    }
                }
            }

            let df = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(DIRTY_FRAMES)) };
            if df > 0 {
                unsafe {
                    core::ptr::write_volatile(
                        core::ptr::addr_of_mut!(DIRTY_FRAMES),
                        df - 1,
                    );
                }

                // Render directly into FRONT — single-buffer, no swap.
                const DESKTOP_PRISTINE: u32 = 0xD030_0000;
                let front = FRONT_FB_ADDR.load(Ordering::Acquire);

                // Pristine restore only when gesture changed widget
                // state. Periodic refreshes draw on existing content
                // to avoid flash on splash icons.
                let np = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(NEEDS_PRISTINE)) };
                if np {
                    unsafe {
                        core::ptr::write_volatile(core::ptr::addr_of_mut!(NEEDS_PRISTINE), false);
                        core::ptr::copy_nonoverlapping(
                            DESKTOP_PRISTINE as *const u8,
                            front as *mut u8,
                            bytes as usize,
                        );
                    }
                }

                {
                    let buf = unsafe {
                        core::slice::from_raw_parts_mut(
                            front as *mut u8,
                            bytes as usize,
                        )
                    };
                    let stride = (w as usize) * 4;
                    let surface =
                        Surface::new(buf, stride, PixelFmt::Argb8888, w, h);
                    let mut blit_renderer: BlitterRenderer<'_, CpuBlitter, 32> =
                        BlitterRenderer::new(&mut cpu_blitter, surface);
                    let mut renderer =
                        RotatedRenderer::new(&mut blit_renderer, w);
                    ctrl.root().borrow().draw(&mut renderer);

                    {
                        let mut cp = unsafe { cortex_m::Peripherals::steal() };
                        cp.SCB.clean_dcache_by_address(
                            front as usize,
                            bytes as usize,
                        );
                    }
                    cortex_m::asm::dsb();
                }
                // No buf_ready — present retriggers the same FRONT.
                // Single-buffer = zero flicker.
            }
        }
    }
}

unsafe extern "C" fn touch_task(_arg: *mut core::ffi::c_void) {
    use crate::touch_i2c;

    // FT5336 CTRL init done in start(). Periodically re-check
    // CTRL and G_MODE — the chip can auto-revert to monitor mode.
    let mut ctrl_check_counter: u32 = 0;

    loop {
        unsafe { hb_inc(HB_TOUCH_TICKS) };

        // CTRL re-check removed — blocking I2C4 kills interrupt path.
        // CTRL=0x00 written at boot; second PG3 pulse provides reset.
        let _ = ctrl_check_counter;

        // Pause when the F command needs clean I2C4 access.
        if TOUCH_PAUSE.load(Ordering::Relaxed) {
            unsafe { vTaskDelay(50) };
            continue;
        }

        // Read touch sample via interrupt-driven I2C4.
        static mut PREV_TOUCH: bool = false;
        static mut TOUCH_DIAG_CNT: u32 = 0;
        let prev = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(PREV_TOUCH)) };
        {
            let s = unsafe { touch_i2c::read_sample_irq() };
            // One-shot diagnostic: first touch or first 100 idle reads
            unsafe {
                let c = core::ptr::read_volatile(core::ptr::addr_of!(TOUCH_DIAG_CNT));
                core::ptr::write_volatile(core::ptr::addr_of_mut!(TOUCH_DIAG_CNT), c + 1);
                if c == 100 && s.count == 0 {
                    crate::runtime_serial::write_bytes(b"TT:100idle\r\n");
                    crate::runtime_serial::kick_tx();
                }
                if s.count > 0 && c < 1000 {
                    crate::runtime_serial::write_bytes(b"TT:hit\r\n");
                    crate::runtime_serial::kick_tx();
                }
            }
            if s.count > 0 {
                unsafe { hb_inc(HB_TOUCH_HITS) };
                let (_id, ef, x, y) = s.points[0];
                let packed = ((s.count as u32) << 28)
                    | ((ef as u32) << 24)
                    | ((x as u32) << 12)
                    | (y as u32 & 0xFFF);
                unsafe { HB_TOUCH_LAST.write_volatile(packed) };
                touch_evt_push(s);
            } else if prev {
                // Push a count=0 sample so render_task sees the release
                touch_evt_push(RawTouchSample::EMPTY);
            }
            unsafe {
                core::ptr::write_volatile(core::ptr::addr_of_mut!(PREV_TOUCH), s.count > 0);
            }
        }

        // ~120 Hz poll rate. FreeRTOS tick is 1 kHz so 8 ticks ≈ 8 ms.
        unsafe { vTaskDelay(8) };
    }
}

// ── Playit command task ──────────────────────────────────────────────────────
//
// Minimal command-line processor on USART1 so a probe-rs / host tool
// can toggle the star crawl (`C`) and query perf counters (`?`). Uses
// the bare-metal `runtime_serial` ring (its USART1 ISR still fires in
// FreeRTOS builds) and dispatches on newline-terminated commands.
//
// A full playit `PlayitExecutor` wants a widget tree we don't have
// yet under FreeRTOS; this handler covers the two commands that
// matter for instrumentation until the widget tree lands.

unsafe extern "C" fn playit_task(_arg: *mut core::ffi::c_void) {
    // Line accumulator. 64 bytes is comfortably larger than any of
    // our supported commands.
    const LINE_CAP: usize = 64;
    let mut line = [0u8; LINE_CAP];
    let mut line_len: usize = 0;

    loop {
        unsafe { hb_inc(HB_PLAYIT_POLLS) };

        // Drain all bytes currently in the RX ring.
        while let Some(b) = crate::runtime_serial::pop_rx() {
            match b {
                b'\r' => { /* swallow */ }
                b'\n' => {
                    if line_len > 0 {
                        handle_command(&line[..line_len]);
                    }
                    line_len = 0;
                }
                _ => {
                    if line_len < LINE_CAP {
                        line[line_len] = b;
                        line_len += 1;
                    } else {
                        // overflow — reset and drop until next newline
                        line_len = 0;
                    }
                }
            }
        }

        // 20 ms poll — responsive enough for interactive commands,
        // low enough to stay out of the render task's way.
        unsafe { vTaskDelay(20) };
    }
}

/// Dispatch a single command line. Responds on USART1 via
/// `runtime_serial::write_bytes` + `kick_tx`.
fn handle_command(line: &[u8]) {
    let first = line.first().copied().unwrap_or(0);
    match first {
        b'C' | b'c' => {
            CRAWL_REQ.store(true, Ordering::Release);
            crate::runtime_serial::write_bytes(b"CRAWL:toggled\r\n");
            crate::runtime_serial::kick_tx();
        }
        b'F' | b'f' => {
            // Pause touch_task, then do clean I2C4 reads
            use crate::touch_i2c;
            TOUCH_PAUSE.store(true, Ordering::Release);
            unsafe { vTaskDelay(20) }; // let touch_task yield

            let int_pin = touch_i2c::int_asserted();
            // Raw 31-byte read (same as read_sample) — dump hex
            let sample = unsafe { touch_i2c::read_sample() };
            // Also read key config registers individually
            let id = unsafe { touch_i2c::read_reg(0xA3) };
            let ctrl = unsafe { touch_i2c::read_reg(0x86) };
            let gm = unsafe { touch_i2c::read_reg(0xA4) };
            let th = unsafe { touch_i2c::read_reg(0x80) };

            TOUCH_PAUSE.store(false, Ordering::Release);

            let mut out = [0u8; 128];
            let mut p = 0;
            p = write_slice(&mut out, p, b"FT: cnt=");
            p = write_hex_u8(&mut out, p, sample.count);
            if sample.count > 0 {
                let (tid, ef, x, y) = sample.points[0];
                p = write_slice(&mut out, p, b" p0=");
                p = write_hex_u8(&mut out, p, tid);
                p = write_slice(&mut out, p, b":");
                p = write_hex_u8(&mut out, p, ef);
                p = write_slice(&mut out, p, b":");
                p = write_u32(&mut out, p, x as u32);
                p = write_slice(&mut out, p, b",");
                p = write_u32(&mut out, p, y as u32);
            }
            p = write_slice(&mut out, p, b" id=");
            p = write_hex_u8(&mut out, p, id.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b" ct=");
            p = write_hex_u8(&mut out, p, ctrl.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b" gm=");
            p = write_hex_u8(&mut out, p, gm.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b" th=");
            p = write_hex_u8(&mut out, p, th.unwrap_or(0xFF));
            p = write_slice(&mut out, p, if int_pin { b" I=L" } else { b" I=H" });
            p = write_slice(&mut out, p, b"\r\n");
            crate::runtime_serial::write_bytes(&out[..p]);
            crate::runtime_serial::kick_tx();
        }
        b'?' => {
            // Snapshot the breadcrumbs and emit a one-line status.
            // Format: ?:tick=<present_ticks>,erif=<erif_wakes>,crawl_fr=<framed>,crawl_rdy=<ready>,touches=<hits>
            let present_ticks = unsafe { HB_PRESENT_TICKS.read_volatile() };
            let erif_wakes = unsafe { HB_ERIF_WAKES.read_volatile() };
            let crawl_fr = unsafe { HB_CRAWL_FRAMEID.read_volatile() };
            let crawl_rdy = unsafe { HB_CRAWL_READY.read_volatile() };
            let touches = unsafe { HB_TOUCH_HITS.read_volatile() };
            let touch_last = unsafe { HB_TOUCH_LAST.read_volatile() };
            // Unpack: [31:28]=count, [27:24]=event_flag, [23:12]=x, [11:0]=y
            let tc = (touch_last >> 28) & 0xF;
            let ef = (touch_last >> 24) & 0xF;
            let tx = (touch_last >> 12) & 0xFFF;
            let ty = touch_last & 0xFFF;

            let mut out = [0u8; 128];
            let mut p = 0usize;
            let prefix = b"?:tick=";
            p = write_slice(&mut out, p, prefix);
            p = write_u32(&mut out, p, present_ticks);
            p = write_slice(&mut out, p, b",erif=");
            p = write_u32(&mut out, p, erif_wakes);
            p = write_slice(&mut out, p, b",crawl_fr=");
            p = write_u32(&mut out, p, crawl_fr);
            p = write_slice(&mut out, p, b",crawl_rdy=");
            p = write_u32(&mut out, p, crawl_rdy);
            p = write_slice(&mut out, p, b",touches=");
            p = write_u32(&mut out, p, touches);
            p = write_slice(&mut out, p, b",t=");
            p = write_u32(&mut out, p, tc);
            p = write_slice(&mut out, p, b":");
            p = write_u32(&mut out, p, ef);
            p = write_slice(&mut out, p, b":");
            p = write_u32(&mut out, p, tx);
            p = write_slice(&mut out, p, b",");
            p = write_u32(&mut out, p, ty);
            p = write_slice(&mut out, p, b"\r\n");

            crate::runtime_serial::write_bytes(&out[..p]);
            crate::runtime_serial::kick_tx();
        }
        _ => {
            crate::runtime_serial::write_bytes(b"?\r\n");
            crate::runtime_serial::kick_tx();
        }
    }
}

fn write_slice(dst: &mut [u8], mut p: usize, s: &[u8]) -> usize {
    for &b in s {
        if p >= dst.len() {
            return p;
        }
        dst[p] = b;
        p += 1;
    }
    p
}

fn write_u32(dst: &mut [u8], mut p: usize, mut v: u32) -> usize {
    let mut tmp = [0u8; 10];
    let mut n = 0;
    if v == 0 {
        if p < dst.len() {
            dst[p] = b'0';
            p += 1;
        }
        return p;
    }
    while v > 0 {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    while n > 0 {
        n -= 1;
        if p >= dst.len() {
            return p;
        }
        dst[p] = tmp[n];
        p += 1;
    }
    p
}

fn write_hex_u8(dst: &mut [u8], mut p: usize, v: u8) -> usize {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if p + 1 < dst.len() {
        dst[p] = HEX[(v >> 4) as usize];
        dst[p + 1] = HEX[(v & 0xF) as usize];
        p += 2;
    }
    p
}

// ── Entry point called from main.rs ───────────────────────────────────────────

/// Start the FreeRTOS scheduler. Never returns.
///
/// Called from `#[entry] fn main()` after the bare-metal hardware init
/// has finished (clocks, SDRAM, DSI, LTDC all online).
///
/// # Safety
///
/// Must be called exactly once, on the CM7, with all display peripherals
/// already initialized and interrupts still globally disabled.
pub unsafe fn start() -> ! {
    unsafe {
        // 0. Stop TIM6 — bare-metal uses it for the touch ISR.
        //    FreeRTOS uses touch_task. Concurrent I2C4 access = bus hang.
        (0x4000_1000u32 as *mut u32).write_volatile(0); // TIM6_CR1 CEN=0

        // 0b. FT5336 init: write CTRL=0x00 (keep-active scan mode).
        //     300+ ms have elapsed since the last PG3 reset; chip is booted.
        {
            use crate::touch_i2c;
            let mut chip_id = touch_i2c::read_reg(0xA3);
            if chip_id.is_none() {
                touch_i2c::i2c4_bus_recover();
                chip_id = touch_i2c::read_reg(0xA3);
            }
            let ctrl_pre = touch_i2c::read_reg(0x86);
            touch_i2c::init_ctrl();
            let ctrl_post = touch_i2c::read_reg(0x86);

            let mut out = [0u8; 48];
            let mut p = 0;
            p = write_slice(&mut out, p, b"FT5336: id=0x");
            p = write_hex_u8(&mut out, p, chip_id.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b" ctrl=0x");
            p = write_hex_u8(&mut out, p, ctrl_pre.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b"->0x");
            p = write_hex_u8(&mut out, p, ctrl_post.unwrap_or(0xFF));
            p = write_slice(&mut out, p, b"\r\n");
            crate::runtime_serial::write_bytes(&out[..p]);
            crate::runtime_serial::kick_tx();
        }

        // 1. Create binary semaphores in static storage.
        let erif_sem =
            freertos_sync::rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(ERIF_SEM_BUF));
        let dma2d_sem =
            freertos_sync::rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(DMA2D_SEM_BUF));
        let buf_ready_sem = freertos_sync::rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(
            BUF_READY_SEM_BUF
        ));
        BUF_READY_SEM.store(buf_ready_sem, Ordering::Release);
        let present_gate_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(PRESENT_GATE_SEM_BUF),
        );
        PRESENT_GATE_SEM.store(present_gate_sem, Ordering::Release);
        let render_start_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(RENDER_START_SEM_BUF),
        );
        RENDER_START_SEM.store(render_start_sem, Ordering::Release);

        // 1c. I2C4 interrupt-driven touch: semaphore
        let i2c4_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(I2C4_SEM_BUF),
        );
        crate::touch_i2c::set_i2c4_sem(i2c4_sem as *mut core::ffi::c_void);

        // 1b. Initialize TIM7 for the ERIF-phase-locked present gate.
        tim7_init();

        // 2. Initialize the sync object in static storage.
        let sync_ptr = core::ptr::addr_of_mut!(SYNC_STORAGE);
        (*sync_ptr).write(FreeRtosFrameSync::new(erif_sem, dma2d_sem));
        SYNC.store((*sync_ptr).as_mut_ptr(), Ordering::Release);

        // 3. Create tasks (static allocation).
        xTaskCreateStatic(
            present_task,
            b"present\0".as_ptr(),
            PRESENT_STACK_WORDS as u32,
            core::ptr::null_mut(),
            3,
            core::ptr::addr_of_mut!(PRESENT_STACK) as *mut StackType_t,
            core::ptr::addr_of_mut!(PRESENT_TCB),
        );

        xTaskCreateStatic(
            render_task,
            b"render\0".as_ptr(),
            RENDER_STACK_WORDS as u32,
            core::ptr::null_mut(),
            1,
            core::ptr::addr_of_mut!(RENDER_STACK) as *mut StackType_t,
            core::ptr::addr_of_mut!(RENDER_TCB),
        );

        xTaskCreateStatic(
            touch_task,
            b"touch\0".as_ptr(),
            TOUCH_STACK_WORDS as u32,
            core::ptr::null_mut(),
            2,
            core::ptr::addr_of_mut!(TOUCH_STACK) as *mut StackType_t,
            core::ptr::addr_of_mut!(TOUCH_TCB),
        );

        xTaskCreateStatic(
            playit_task,
            b"playit\0".as_ptr(),
            PLAYIT_STACK_WORDS as u32,
            core::ptr::null_mut(),
            2,
            core::ptr::addr_of_mut!(PLAYIT_STACK) as *mut StackType_t,
            core::ptr::addr_of_mut!(PLAYIT_TCB),
        );

        // 4. Enable DSI + DMA2D IRQs at priorities above the syscall
        //    ceiling so xSemaphoreGiveFromISR remains safe. Tick runs
        //    at the lowest NVIC priority (15); FreeRTOS clamps to
        //    configKERNEL_INTERRUPT_PRIORITY at scheduler start.
        let mut cp = cortex_m::Peripherals::steal();
        cp.NVIC
            .set_priority(stm32h7::stm32h747cm7::Interrupt::DSI, 6 << 4);
        cp.NVIC
            .set_priority(stm32h7::stm32h747cm7::Interrupt::DMA2D, 7 << 4);
        // TIM7 at priority 6 matches DSI — both feed the present task
        // directly (ERIF sem + gate sem) and must be ISR-safe for
        // FromISR API (priority > configLIBRARY_MAX_SYSCALL = 5).
        cp.NVIC
            .set_priority(stm32h7::stm32h747cm7::Interrupt::TIM7, 6 << 4);
        // I2C4_EV at priority 7 — ISR-safe for FromISR semaphore give
        cp.NVIC
            .set_priority(stm32h7::stm32h747cm7::Interrupt::I2C4_EV, 7 << 4);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DSI);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DMA2D);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::TIM7);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::I2C4_EV);

        // 5. Start the scheduler — never returns.
        //    Enable SysTick routing to FreeRTOS just before launch.
        SYSTICK_READY.store(true, Ordering::Release);
        vTaskStartScheduler();
    }
}

// ── FreeRTOS idle / stack-overflow hooks (required by linker even when
// ── the corresponding config macros are 0 if the kernel references
// ── them at compile time; safe minimal stubs) ────────────────────────
//
// configCHECK_FOR_STACK_OVERFLOW = 0 and configUSE_IDLE_HOOK = 0, so
// FreeRTOS does not emit calls to these. They are left out for now.

// ── Static idle + timer task support (required by static allocation) ──────────
//
// With configSUPPORT_STATIC_ALLOCATION = 1 and configUSE_TIMERS = 1,
// FreeRTOS calls these to obtain storage for its internal tasks.

static mut IDLE_TCB: StaticTask = StaticTask::new();
static mut IDLE_STACK: [StackType_t; 128] = [0; 128];

static mut TIMER_TCB: StaticTask = StaticTask::new();
static mut TIMER_STACK: [StackType_t; 256] = [0; 256];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vApplicationGetIdleTaskMemory(
    ppxIdleTaskTCBBuffer: *mut *mut StaticTask,
    ppxIdleTaskStackBuffer: *mut *mut StackType_t,
    pulIdleTaskStackSize: *mut u32,
) {
    unsafe {
        *ppxIdleTaskTCBBuffer = core::ptr::addr_of_mut!(IDLE_TCB);
        *ppxIdleTaskStackBuffer = core::ptr::addr_of_mut!(IDLE_STACK) as *mut StackType_t;
        *pulIdleTaskStackSize = 128;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vApplicationGetTimerTaskMemory(
    ppxTimerTaskTCBBuffer: *mut *mut StaticTask,
    ppxTimerTaskStackBuffer: *mut *mut StackType_t,
    pulTimerTaskStackSize: *mut u32,
) {
    unsafe {
        *ppxTimerTaskTCBBuffer = core::ptr::addr_of_mut!(TIMER_TCB);
        *ppxTimerTaskStackBuffer = core::ptr::addr_of_mut!(TIMER_STACK) as *mut StackType_t;
        *pulTimerTaskStackSize = 256;
    }
}
