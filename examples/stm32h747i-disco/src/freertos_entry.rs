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

#[inline(always)]
unsafe fn hb_inc(addr: *mut u32) {
    unsafe {
        let v = addr.read_volatile().wrapping_add(1);
        addr.write_volatile(v);
    }
}

unsafe extern "C" fn present_task(_arg: *mut core::ffi::c_void) {
    use core::sync::atomic::Ordering;

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

        // Holdoff: 15 ms after ERIF ensures we hit the same TE slot
        // each frame. Matches bare-metal PRESENT_HOLDOFF = 6M cycles.
        unsafe { vTaskDelay(15) };

        // If the render task has signalled a fresh back buffer, swap
        // FRONT and BACK atomically — the new front is what LTDC will
        // scan on the next re-trigger; the old front becomes the new
        // back for the next render pass.
        let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
        if !buf_ready.is_null()
            && unsafe { freertos_sync::rlvgl_sem_take(buf_ready, 0) } == freertos_sync::pdTRUE
        {
            let front = FRONT_FB_ADDR.load(Ordering::Acquire);
            let back = BACK_FB_ADDR.load(Ordering::Acquire);
            FRONT_FB_ADDR.store(back, Ordering::Release);
            BACK_FB_ADDR.store(front, Ordering::Release);
        }

        let fb = FRONT_FB_ADDR.load(Ordering::Acquire);
        if fb != 0 {
            // Mark the previous scan's completion consumed before
            // starting a new one. Render tasks (star_crawl) poll this
            // via `sync.erif_is_set` to gate on "prior scan done".
            sync.scan_complete.store(false, Ordering::Release);
            unsafe { ltdc_retrigger(fb) };
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
    // Per-outer-iteration ceiling on Pending iterations. A full crawl
    // frame needs roughly 500-800 ticks (one tick per state transition
    // in the star_crawl state machine). Exiting the inner loop early
    // forces the render task back through lazy-init checks and the
    // toggle handshake, which costs ~20-30 µs per round-trip; at 1.6
    // fps with a 50-tick cap, that adds up to substantial overhead.
    // A generous ceiling keeps us in the inner loop for most of a
    // frame while still bounding time if crawl stalls.
    const CRAWL_TIMEOUT_TICKS: u32 = 5000;

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
                        // No voluntary yield on Pending. Star crawl
                        // returns Pending when DMA2D is still in
                        // flight; at the lowest task priority (1),
                        // any higher-priority wake (DSI ISR → present
                        // sem-give → present preempts us; touch /
                        // playit likewise) already preempts this
                        // loop automatically. Voluntary yields only
                        // add vTaskDelay latency without enabling
                        // any work that isn't already preemptible.
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

        unsafe { vTaskDelay(IDLE_PERIOD_MS) };
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
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DSI);
        cortex_m::peripheral::NVIC::unmask(stm32h7::stm32h747cm7::Interrupt::DMA2D);

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
