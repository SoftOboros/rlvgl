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

#[cortex_m_rt::exception]
fn SysTick() {
    unsafe { xPortSysTickHandler() }
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
            // Note: `render_start_sem` is NOT given here. Waking
            // render on the ERIF edge would start it writing to BACK
            // while present_task has not yet done the front/back
            // swap — race. The give lives in `present_task` *after*
            // `ltdc_retrigger`, so render only wakes once the new
            // BACK is guaranteed to be off-screen.
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

/// Shared flag — playit `C` command sets this; render task reads to
/// toggle the star crawl on/off.
pub static CRAWL_REQ: core::sync::atomic::AtomicBool =
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
        const PRESENT_HOLDOFF_CYC: u32 = 6_000_000; // 15 ms @ 400 MHz
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

        // If the render task has signalled a fresh back buffer, swap
        // FRONT and BACK atomically — unless CRAWL_ACTIVE requires a
        // 2-cycle hold to smooth the 55 ms crawl / 33 ms present
        // rate mismatch.
        let crawl_active = CRAWL_ACTIVE.load(Ordering::Acquire);
        let hold_more = crawl_active && cycles_since_swap < 1;
        let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
        if !hold_more
            && !buf_ready.is_null()
            && unsafe { freertos_sync::rlvgl_sem_take(buf_ready, 0) } == freertos_sync::pdTRUE
        {
            let front = FRONT_FB_ADDR.load(Ordering::Acquire);
            let back = BACK_FB_ADDR.load(Ordering::Acquire);
            FRONT_FB_ADDR.store(back, Ordering::Release);
            BACK_FB_ADDR.store(front, Ordering::Release);
            cycles_since_swap = 0;
        } else {
            cycles_since_swap = cycles_since_swap.saturating_add(1);
        }

        let fb = FRONT_FB_ADDR.load(Ordering::Acquire);
        if fb != 0 {
            // Mark the previous scan's completion consumed before
            // starting a new one. Render tasks (star_crawl) poll this
            // via `sync.erif_is_set` to gate on "prior scan done".
            sync.scan_complete.store(false, Ordering::Release);
            unsafe { ltdc_retrigger(fb) };
        }

        // Wake render_task. This gate is given *after* the
        // front/back swap + retrigger, not on the DSI ERIF edge —
        // so when render_task resumes, the BACK atomic already
        // points to a buffer that is off-screen (LTDC is scanning
        // the new FRONT we just retriggered). Waking on ERIF
        // instead would race the swap: render would start writing
        // to a buffer that is about to become FRONT.
        let rs = RENDER_START_SEM.load(Ordering::Acquire);
        if !rs.is_null() {
            unsafe { freertos_sync::rlvgl_sem_give(rs) };
        }
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

    // Frame pacing when idle (non-crawl compositor demo). Crawl mode
    // yields more aggressively between DMA2D waits (see tick loop).
    const IDLE_PERIOD_MS: u32 = 16; // ~62 Hz
    const CRAWL_YIELD_MS: u32 = 1; // short yield between Pending ticks
    // Per-outer-iteration ceiling on Pending iterations. With the
    // render_start_sem gate (given by present_task AFTER retrigger),
    // render wakes while the panel scan is just starting — not yet
    // complete. star_crawl's internal `erif_is_set()` gate blocks on
    // the sticky `scan_complete` flag, which DSI ISR won't set until
    // ~14 ms later when the scan finishes. The tick loop must stay
    // in Pending long enough for ERIF to fire, or the frame never
    // completes.
    //
    // 200_000 iterations is a bounded ceiling (still exits cleanly
    // if DMA2D truly stalls) but large enough that the ~14 ms wait
    // for ERIF is well within budget — even if every inner iteration
    // took a microsecond, 200_000 is 200 ms. In practice the loop
    // exits on FrameReady well before this.
    const CRAWL_TIMEOUT_TICKS: u32 = 200_000;

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

        // One-shot double-buffer initial fill. Both front and back
        // SDRAM regions get cleared to the compositor's background
        // color via DMA2D (≈ 1 ms each at register-level R2M) so the
        // splash that bare-metal init decoded into the original front
        // is wiped BEFORE any render runs. Doing this via CpuBlitter
        // inside the compositor's `seed_remaining` mechanism is unsafe
        // under load: if a render iteration is lost (present TIM7
        // fires before render's CpuBlitter finishes), the seed gets
        // applied twice to the SAME buffer and the other one keeps
        // the splash — which then flashes on alternate scans forever.
        // Hardware fill before entering the loop is deterministic.
        static mut DOUBLE_FILL_DONE: bool = false;
        if unsafe { !DOUBLE_FILL_DONE } {
            let front = FRONT_FB_ADDR.load(Ordering::Acquire);
            let bg = 0xFF08_0820u32;
            for &addr in &[front, back] {
                if addr != 0 {
                    dma2d.as_mut().unwrap().fill_raw(
                        addr as *mut u8,
                        w * 4,
                        w,
                        h,
                        bg,
                        rlvgl_platform::blit::PixelFmt::Argb8888,
                    );
                }
            }
            // Clean D-cache: bare-metal may have touched SDRAM via
            // CPU (splash decode) and those lines could still be in
            // cache. After DMA2D filled SDRAM directly, the cache
            // still holds stale splash bytes — an LTDC scan would
            // pick up cache-backed reads through any CPU path, and a
            // later CPU write would flush those stale lines back over
            // our DMA2D fill. Invalidate-clean ensures parity.
            {
                let mut cp = unsafe { cortex_m::Peripherals::steal() };
                if front != 0 {
                    cp.SCB
                        .clean_invalidate_dcache_by_address(front as usize, (w * h * 4) as usize);
                }
                if back != 0 {
                    cp.SCB
                        .clean_invalidate_dcache_by_address(back as usize, (w * h * 4) as usize);
                }
            }
            cortex_m::asm::dsb();
            unsafe { DOUBLE_FILL_DONE = true };
        }

        let dma = dma2d.as_mut().unwrap();
        let cr = crawl.as_mut().unwrap();

        // ── Toggle handshake with playit_task ─────────────────────
        let req = CRAWL_REQ.swap(false, Ordering::AcqRel);
        if req {
            if cr.is_active() {
                cr.deactivate();
            } else {
                cr.activate(dma);
                cr.set_layer_buf(0xD180_0000usize as *mut u8);
            }
        }
        // Publish current crawl state for present_task's pacing
        // decision. See CRAWL_ACTIVE doc for why this matters.
        CRAWL_ACTIVE.store(cr.is_active(), Ordering::Release);

        // ── Crawl mode: non-blocking tick loop ────────────────────
        if cr.is_active() {
            let sync = get_sync().unwrap();
            let mut deadline_hits = 0u32;
            let mut frame_ready = false;
            let mut finished = false;

            // Run tick() until the frame is ready, the crawl finishes,
            // or we hit a per-frame ceiling (prevents the render task
            // from monopolizing the CPU if DMA2D stalls). Between
            // `Pending` returns yield briefly so touch / present /
            // idle can run.
            while deadline_hits < CRAWL_TIMEOUT_TICKS {
                unsafe { hb_inc(HB_CRAWL_TICKS) };
                match cr.tick(dma, back as *mut u8, w, h, sync) {
                    StepResult::Idle => break,
                    StepResult::Pending => {
                        deadline_hits += 1;
                        // Yield once every ~4096 Pending ticks so the
                        // FreeRTOS idle hook gets a crack at stack-
                        // watermark checks and stats while we're
                        // spinning on DMA2D / ERIF. Priority-6 ISRs
                        // (DSI / DMA2D / TIM7) already preempt us
                        // automatically; this yield is just for the
                        // idle task. Every ~4 ms at 1 µs/tick.
                        if deadline_hits & 0xFFF == 0 {
                            unsafe { vTaskDelay(1) };
                        }
                    }
                    StepResult::FrameReady => {
                        frame_ready = true;
                        break;
                    }
                    StepResult::Finished => {
                        finished = true;
                        break;
                    }
                }
            }

            if frame_ready {
                cortex_m::asm::dsb();
                unsafe {
                    HB_CRAWL_FRAMEID.write_volatile(cr.frame_id());
                    hb_inc(HB_CRAWL_READY);
                }
                let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
                if !buf_ready.is_null() {
                    unsafe { freertos_sync::rlvgl_sem_give(buf_ready) };
                }
                cr.advance_scroll();
            }

            if finished {
                cr.deactivate();
            }

            // Stay in crawl mode — loop back to tick the next frame.
            continue;
        }

        // ── Idle mode: simple compositor demo, CpuBlitter-backed ──
        let c = compositor.get_or_insert_with(|| {
            let mut c = Compositor::new();
            c.push(Box::new(SolidBackgroundLayer { color: 0xFF08_0820 }));
            let block_h = 80u32;
            let block_w = 80u32;
            let block = MotionBlockLayer::new(
                Rect {
                    x: 0,
                    y: ((h - block_h) / 2) as i32,
                    w: block_w,
                    h: block_h,
                },
                0xFFE5_3E3E,
                4,
                (w, h),
            );
            c.push(Box::new(block));
            c.prime_full(Rect { x: 0, y: 0, w, h });
            c
        });

        let buf =
            unsafe { core::slice::from_raw_parts_mut(back as *mut u8, bytes as usize) };
        let stride = (w as usize) * 4;
        let mut surf = Surface::new(buf, stride, PixelFmt::Argb8888, w, h);

        let stats = c.render_frame(&mut cpu_blitter, &mut surf);

        // Clean the D-cache for the region we just wrote. CpuBlitter
        // issues normal CPU stores which land in the D-cache (write-
        // back on this part), and LTDC reads SDRAM directly via AXI
        // — so without a cache-clean it sees stale pixels and shows
        // "lines alternating" tearing as the cache drains lazily.
        // `dsb()` is a memory barrier only; it does NOT flush cache
        // to SDRAM.
        {
            let mut cp = unsafe { cortex_m::Peripherals::steal() };
            cp.SCB
                .clean_dcache_by_address(back as usize, bytes as usize);
        }
        cortex_m::asm::dsb();

        unsafe {
            HB_RENDER_PIXELS.write_volatile(stats.pixels_touched);
            HB_RENDER_RECTS.write_volatile(stats.rect_count);
            HB_RENDER_CYC.write_volatile(stats.cycles);
            let prev = HB_RENDER_PX_TOT.read_volatile();
            HB_RENDER_PX_TOT.write_volatile(prev.wrapping_add(stats.pixels_touched));
        }

        let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
        if !buf_ready.is_null() {
            unsafe { freertos_sync::rlvgl_sem_give(buf_ready) };
        }
    }
}

unsafe extern "C" fn touch_task(_arg: *mut core::ffi::c_void) {
    use crate::touch_i2c;

    loop {
        unsafe { hb_inc(HB_TOUCH_TICKS) };

        // Only poll the chip when its INT line is asserted or we just
        // saw a touch (to catch the release event). This keeps the
        // I2C bus quiet in the common no-touch case.
        static mut PREV_TOUCH: bool = false;
        let int_low = touch_i2c::int_asserted();
        let prev = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(PREV_TOUCH)) };

        if int_low || prev {
            let s = unsafe { touch_i2c::read_sample() };
            if s.count > 0 {
                unsafe { hb_inc(HB_TOUCH_HITS) };
                // Pack the first point for debug visibility.
                let (_id, ef, x, y) = s.points[0];
                let packed = ((s.count as u32) << 28)
                    | ((ef as u32) << 24)
                    | ((x as u32) << 12)
                    | (y as u32 & 0xFFF);
                unsafe { HB_TOUCH_LAST.write_volatile(packed) };
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
        b'?' => {
            // Snapshot the breadcrumbs and emit a one-line status.
            // Format: ?:tick=<present_ticks>,erif=<erif_wakes>,crawl_fr=<framed>,crawl_rdy=<ready>,touches=<hits>
            let present_ticks = unsafe { HB_PRESENT_TICKS.read_volatile() };
            let erif_wakes = unsafe { HB_ERIF_WAKES.read_volatile() };
            let crawl_fr = unsafe { HB_CRAWL_FRAMEID.read_volatile() };
            let crawl_rdy = unsafe { HB_CRAWL_READY.read_volatile() };
            let touches = unsafe { HB_TOUCH_HITS.read_volatile() };

            let mut out = [0u8; 96];
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
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DSI);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DMA2D);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::TIM7);

        // 5. Start the scheduler — never returns.
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
