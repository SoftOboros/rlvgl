//! FreeRTOS preemptive task entry point for BeagleBone Black.
//!
//! Creates three tasks on the bare-metal BSP:
//!
//! | Task | Priority | Stack | Blocks on |
//! |------|----------|-------|-----------|
//! | present | 3 | 2 KB | LCDC EOF0 semaphore |
//! | render  | 1 | 8 KB | render-start semaphore |
//! | touch   | 2 | 1 KB | vTaskDelay (120 Hz) |
//!
//! The LCDC EOF0 interrupt gives `eof_sem` from ISR context, waking
//! the present task to swap framebuffers. The render task composes the
//! widget tree into the back buffer and signals `buf_ready_sem`.

#![allow(dead_code)]

use crate::freertos_sync::*;
use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Static storage for kernel objects
// ---------------------------------------------------------------------------

static mut EOF_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut BUF_READY_SEM_BUF: StaticSemaphore = StaticSemaphore::new();
static mut RENDER_START_SEM_BUF: StaticSemaphore = StaticSemaphore::new();

static EOF_SEM: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());
static BUF_READY_SEM: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());
static RENDER_START_SEM: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(core::ptr::null_mut());

// Task stacks (in words = u32)
const PRESENT_STACK_WORDS: usize = 512; // 2 KB
const RENDER_STACK_WORDS: usize = 2048; // 8 KB
const TOUCH_STACK_WORDS: usize = 256; // 1 KB

static mut PRESENT_STACK: [StackType_t; PRESENT_STACK_WORDS] = [0; PRESENT_STACK_WORDS];
static mut RENDER_STACK: [StackType_t; RENDER_STACK_WORDS] = [0; RENDER_STACK_WORDS];
static mut TOUCH_STACK: [StackType_t; TOUCH_STACK_WORDS] = [0; TOUCH_STACK_WORDS];

static mut PRESENT_TCB: StaticTask = StaticTask::new();
static mut RENDER_TCB: StaticTask = StaticTask::new();
static mut TOUCH_TCB: StaticTask = StaticTask::new();

// Framebuffer addresses (swapped by present task)
static FRONT_FB: AtomicU32 = AtomicU32::new(0);
static BACK_FB: AtomicU32 = AtomicU32::new(0);

// Frame counter for diagnostics
static FRAME_COUNT: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------
// LCDC EOF0 ISR body — called from interrupt context
// ---------------------------------------------------------------------------

/// Called by the LCDC EOF0 interrupt handler.
/// Gives the `eof_sem` semaphore to wake the present task.
pub unsafe fn lcdc_eof_isr_body() {
    // Clear the interrupt
    unsafe {
        crate::bsp::lcdc::clear_eof_irq();
    }

    // Wake present task
    let sem = EOF_SEM.load(Ordering::Acquire);
    if !sem.is_null() {
        unsafe {
            rlvgl_sem_give_from_isr(sem);
        }
    }
}

// ---------------------------------------------------------------------------
// Task functions
// ---------------------------------------------------------------------------

/// Present task: waits for LCDC EOF, then signals render to compose next frame.
unsafe extern "C" fn present_task(_arg: *mut core::ffi::c_void) {
    loop {
        // Block until LCDC scans a complete frame
        let eof = EOF_SEM.load(Ordering::Acquire);
        if !eof.is_null() {
            unsafe {
                rlvgl_sem_take(eof, portMAX_DELAY);
            }
        }

        FRAME_COUNT.fetch_add(1, Ordering::Relaxed);

        // Check if render has a new frame ready
        let br = BUF_READY_SEM.load(Ordering::Acquire);
        if !br.is_null() {
            unsafe {
                if rlvgl_sem_take(br, 0) == pdTRUE {
                    // Swap front/back framebuffers
                    let front = FRONT_FB.load(Ordering::Acquire);
                    let back = BACK_FB.load(Ordering::Acquire);
                    FRONT_FB.store(back, Ordering::Release);
                    BACK_FB.store(front, Ordering::Release);

                    // Update LCDC DMA to scan from new front buffer
                    let fb_size = crate::bsp::lcdc::HACTIVE * crate::bsp::lcdc::VACTIVE * 4;
                    crate::bsp::am335x::reg_write(
                        crate::bsp::am335x::LCDDMA_FB0_BASE,
                        back, // new front = old back
                    );
                    crate::bsp::am335x::reg_write(
                        crate::bsp::am335x::LCDDMA_FB0_CEILING,
                        back + fb_size - 4,
                    );
                }
            }
        }

        // Signal render task to start composing into (now-free) back buffer
        let rs = RENDER_START_SEM.load(Ordering::Acquire);
        if !rs.is_null() {
            unsafe {
                rlvgl_sem_give(rs);
            }
        }
    }
}

/// Render task: composes the widget tree into the back framebuffer.
///
/// Currently a placeholder — fills with a cycling color pattern.
/// Full DiscoController integration requires the heap allocator.
unsafe extern "C" fn render_task(_arg: *mut core::ffi::c_void) {
    let mut tick: u32 = 0;

    loop {
        // Wait for present task to signal that back buffer is free
        let rs = RENDER_START_SEM.load(Ordering::Acquire);
        if !rs.is_null() {
            unsafe {
                rlvgl_sem_take(rs, portMAX_DELAY);
            }
        } else {
            unsafe {
                vTaskDelay(16);
            } // ~60 Hz fallback
        }

        // Render into back buffer (placeholder: cycling color fill)
        let back_addr = BACK_FB.load(Ordering::Acquire);
        if back_addr != 0 {
            let w = crate::bsp::lcdc::HACTIVE;
            let h = crate::bsp::lcdc::VACTIVE;
            let fb =
                unsafe { core::slice::from_raw_parts_mut(back_addr as *mut u32, (w * h) as usize) };

            // Simple gradient that shifts each frame
            let phase = tick & 0xFF;
            for (i, pixel) in fb.iter_mut().enumerate() {
                let x = (i as u32) % w;
                let y = (i as u32) / w;
                let r = ((x + phase) & 0xFF) as u8;
                let g = ((y + phase) & 0xFF) as u8;
                let b = phase as u8;
                *pixel = 0xFF00_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
            }
        }

        tick = tick.wrapping_add(1);

        // Signal that a new frame is ready
        let br = BUF_READY_SEM.load(Ordering::Acquire);
        if !br.is_null() {
            unsafe {
                rlvgl_sem_give(br);
            }
        }
    }
}

/// Touch task: polls FT5x06 capacitive touch controller via I2C2 at 120 Hz.
///
/// Currently a placeholder — the I2C master driver is not yet implemented.
unsafe extern "C" fn touch_task(_arg: *mut core::ffi::c_void) {
    loop {
        // TODO: read FT5x06 via I2C2
        // let sample = i2c2_read_touch();
        // if sample.count > 0 { dispatch_touch_event(sample); }

        unsafe {
            vTaskDelay(8);
        } // 8 ms ≈ 120 Hz
    }
}

// ---------------------------------------------------------------------------
// FreeRTOS start — called from bare_metal.rs after BSP init
// ---------------------------------------------------------------------------

/// Initialize framebuffer addresses and start the FreeRTOS scheduler.
///
/// `fb0` and `fb1` are physical addresses of two framebuffers in DDR.
/// Each must be `HACTIVE * VACTIVE * 4` bytes (1,536,000 bytes for 800x480 ARGB).
///
/// # Safety
///
/// Must be called exactly once, after BSP init (PRCM, pin mux, LCDC).
/// Never returns — calls `vTaskStartScheduler()`.
pub unsafe fn start(fb0: u32, fb1: u32) -> ! {
    FRONT_FB.store(fb0, Ordering::Release);
    BACK_FB.store(fb1, Ordering::Release);

    unsafe {
        // Create binary semaphores
        let eof = rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(EOF_SEM_BUF));
        EOF_SEM.store(eof, Ordering::Release);

        let br = rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(BUF_READY_SEM_BUF));
        BUF_READY_SEM.store(br, Ordering::Release);

        let rs = rlvgl_sem_create_binary_static(core::ptr::addr_of_mut!(RENDER_START_SEM_BUF));
        RENDER_START_SEM.store(rs, Ordering::Release);

        // Give render_start once so the first frame renders immediately
        rlvgl_sem_give(rs);

        // Create tasks
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

        // TODO: Configure AM335x INTC for LCDC EOF0 (IRQ 36)
        // The INTC is memory-mapped at 0x4820_0000 (not NVIC).
        // For now, the present task will spin-poll is_eof_pending()
        // until INTC routing is implemented.

        // Launch scheduler — never returns
        vTaskStartScheduler();
    }
}
