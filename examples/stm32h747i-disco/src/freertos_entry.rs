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

use crate::freertos_sync::{
    self, FreeRtosFrameSync, SemaphoreHandle_t, StaticSemaphore,
};

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

static mut ERIF_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut DMA2D_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut BUF_READY_SEM_BUF: StaticSemaphore = StaticSemaphore::new();

static mut PRESENT_TCB: StaticTask = StaticTask::new();
static mut RENDER_TCB: StaticTask = StaticTask::new();
static mut TOUCH_TCB: StaticTask = StaticTask::new();

static mut PRESENT_STACK: [StackType_t; PRESENT_STACK_WORDS] = [0; PRESENT_STACK_WORDS];
static mut RENDER_STACK: [StackType_t; RENDER_STACK_WORDS] = [0; RENDER_STACK_WORDS];
static mut TOUCH_STACK: [StackType_t; TOUCH_STACK_WORDS] = [0; TOUCH_STACK_WORDS];

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

static FRONT_FB_ADDR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);
static BACK_FB_ADDR: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);
static FB_BYTES: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

/// Record the framebuffer addresses and byte size for the present /
/// render tasks. Called once from `main()` just before
/// `freertos_entry::start()`.
pub fn init_fbs(front: u32, back: u32, bytes: u32) {
    FRONT_FB_ADDR.store(front, core::sync::atomic::Ordering::Release);
    BACK_FB_ADDR.store(back, core::sync::atomic::Ordering::Release);
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

const HB_PRESENT_TICKS: *mut u32 = 0x3800_0700 as *mut u32;
const HB_RENDER_TICKS:  *mut u32 = 0x3800_0704 as *mut u32;
const HB_TOUCH_TICKS:   *mut u32 = 0x3800_0708 as *mut u32;
const HB_ERIF_WAKES:    *mut u32 = 0x3800_070C as *mut u32;
const HB_TOUCH_HITS:    *mut u32 = 0x3800_0710 as *mut u32;
const HB_TOUCH_LAST:    *mut u32 = 0x3800_0714 as *mut u32;

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
            unsafe { ltdc_retrigger(fb) };
        }
    }
}

unsafe extern "C" fn render_task(_arg: *mut core::ffi::c_void) {
    use core::sync::atomic::Ordering;

    // Simple visible proof-of-life: fill the back buffer with a solid
    // color that cycles red → green → blue every render pass. When
    // the present task swaps to this buffer, the whole panel changes
    // color, proving the render → present pipeline is live.
    //
    // Rate-limited to ~2 Hz so the color change is eye-pace rather
    // than strobe-pace, and so the back-buffer write stays well below
    // any present-side swap cadence (no tearing risk).

    const RENDER_PERIOD_MS: u32 = 500;
    let colors: [u32; 3] = [0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF];
    let mut frame: u32 = 0;

    loop {
        unsafe { hb_inc(HB_RENDER_TICKS) };

        let back = BACK_FB_ADDR.load(Ordering::Acquire);
        let bytes = FB_BYTES.load(Ordering::Acquire);
        if back != 0 && bytes != 0 {
            let color = colors[(frame as usize) % colors.len()];
            let pixels = (bytes / 4) as usize;
            let ptr = back as *mut u32;
            // Simple word-stride fill. Not the fastest possible but
            // fine for ~2 Hz demo cadence.
            for i in 0..pixels {
                unsafe { ptr.add(i).write_volatile(color) };
            }
            cortex_m::asm::dsb();

            // Signal the present task that the back buffer is ready.
            let buf_ready = BUF_READY_SEM.load(Ordering::Acquire);
            if !buf_ready.is_null() {
                unsafe { freertos_sync::rlvgl_sem_give(buf_ready) };
            }

            frame = frame.wrapping_add(1);
        }

        unsafe { vTaskDelay(RENDER_PERIOD_MS) };
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
                let packed =
                    ((s.count as u32) << 28) | ((ef as u32) << 24) | ((x as u32) << 12) | (y as u32 & 0xFFF);
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
        let erif_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(ERIF_SEM_BUF),
        );
        let dma2d_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(DMA2D_SEM_BUF),
        );
        let buf_ready_sem = freertos_sync::rlvgl_sem_create_binary_static(
            core::ptr::addr_of_mut!(BUF_READY_SEM_BUF),
        );
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

        // 4. Enable DSI + DMA2D IRQs at priorities above the syscall
        //    ceiling so xSemaphoreGiveFromISR remains safe. Tick runs
        //    at the lowest NVIC priority (15); FreeRTOS clamps to
        //    configKERNEL_INTERRUPT_PRIORITY at scheduler start.
        let mut cp = cortex_m::Peripherals::steal();
        cp.NVIC.set_priority(stm32h7::stm32h747cm7::Interrupt::DSI, 6 << 4);
        cp.NVIC.set_priority(stm32h7::stm32h747cm7::Interrupt::DMA2D, 7 << 4);
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
