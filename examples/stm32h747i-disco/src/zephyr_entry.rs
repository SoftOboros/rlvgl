//! Zephyr RTOS entry point and ISR handlers.
//!
//! This module provides the `extern "C"` interface between the Zephyr C
//! application shell and the Rust rlvgl demo. The C side defines kernel
//! objects (`K_SEM_DEFINE`, `K_THREAD_DEFINE`) and calls `rlvgl_init()`
//! from its `main()`.
//!
//! ## Thread model
//!
//! | Thread   | Priority | Role                                    |
//! |----------|----------|-----------------------------------------|
//! | main     | —        | Calls `rlvgl_init`, returns to Zephyr   |
//! | present  | 3        | ERIF sem → holdoff → present → give     |
//! | render   | 5        | Render pipeline, DMA2D, gives buf_ready |
//! | touch    | 4        | 120 Hz FT5336 read, pushes to msgq      |
//!
//! ## ISR model
//!
//! DSI (IRQ 123) and DMA2D ISRs are registered from the C side via
//! `IRQ_CONNECT`. They call thin Rust `extern "C"` handlers here.

use crate::zephyr_sync::{self, ZephyrFrameSync};
use core::sync::atomic::{AtomicPtr, Ordering};

// ── Exported ISR handlers ─────────────────────────────────────────────────────
//
// Called from C-side ISR wrappers registered via IRQ_CONNECT.

/// Pointer to the global `ZephyrFrameSync`. Set by `rlvgl_init()`.
/// Uses `AtomicPtr` instead of `static mut` to satisfy Rust 2024 rules.
static SYNC: AtomicPtr<ZephyrFrameSync> = AtomicPtr::new(core::ptr::null_mut());

/// Get the sync reference. Returns `None` before `rlvgl_init()`.
#[inline]
fn get_sync() -> Option<&'static ZephyrFrameSync> {
    let ptr = SYNC.load(Ordering::Acquire);
    if ptr.is_null() { None } else { Some(unsafe { &*ptr }) }
}

/// DSI ISR handler — called from C when DSI IRQ 123 fires.
///
/// Mirrors the bare-metal `_dsi_isr::DSI()` logic:
/// 1. Read DSI_WISR, clear all wrapper flags
/// 2. On ERIF (bit 1): snapshot DWT_CYCCNT, clear LTDCEN, PJ0 LOW
/// 3. Give erif_sem so present thread wakes
/// 4. Clear host-level flags (FIR0/FIR1)
///
/// # Safety
///
/// Must be called from interrupt context only. `SYNC` must be initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_dsi_isr() {
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
            DSI_WCR.write_volatile(0x08); // DSIEN only

            if let Some(sync) = get_sync() {
                sync.isr_record_erif(cyc);
                zephyr_sync::k_sem_give(sync.erif_sem);
            }
        }

        // Clear host-level flags
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

/// DMA2D ISR handler — called from C when DMA2D IRQ fires.
///
/// Clears ISR flags and gives `dma2d_done_sem`.
///
/// # Safety
///
/// Must be called from interrupt context only. `SYNC` must be initialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_dma2d_isr() {
    unsafe {
        const DMA2D_ISR: *const u32 = 0x5200_1004 as *const u32;
        const DMA2D_IFCR: *mut u32 = 0x5200_1008 as *mut u32;

        let isr = DMA2D_ISR.read_volatile();
        let clear = isr & 0x3F;
        if clear != 0 {
            DMA2D_IFCR.write_volatile(clear);
        }

        // TC (bit 1) = transfer complete
        if isr & (1 << 1) != 0 {
            if let Some(sync) = get_sync() {
                zephyr_sync::k_sem_give(sync.dma2d_done_sem);
            }
        }
    }
}

// ── Initialization entry point ────────────────────────────────────────────────

/// Called from Zephyr C `main()` to initialize the rlvgl display system.
///
/// Receives pointers to kernel objects allocated on the C side.
/// Initializes DSI/LTDC, sets up the sync object, and spawns threads.
///
/// # Safety
///
/// `erif_sem` and `dma2d_done_sem` must be valid, initialized `k_sem`
/// pointers with `'static` lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_init(
    erif_sem: *mut zephyr_sync::k_sem,
    dma2d_done_sem: *mut zephyr_sync::k_sem,
) {
    unsafe {
        // Construct the sync object in static storage. rlvgl_init is called
        // exactly once from Zephyr main (single-threaded at this point).
        use core::sync::atomic::AtomicBool;
        static INIT_DONE: AtomicBool = AtomicBool::new(false);
        static mut SYNC_STORAGE: core::mem::MaybeUninit<ZephyrFrameSync> =
            core::mem::MaybeUninit::uninit();

        if INIT_DONE.swap(true, Ordering::AcqRel) {
            return; // double-init guard
        }

        let ptr = core::ptr::addr_of_mut!(SYNC_STORAGE);
        (*ptr).write(ZephyrFrameSync::new(erif_sem, dma2d_done_sem));
        SYNC.store((*ptr).as_mut_ptr(), Ordering::Release);
    }

    // TODO (Phase 5): Call the shared DSI/LTDC init from
    // platform/src/stm32h747i_disco.rs here, then start the
    // present/render/touch threads.
    //
    // For now this is a skeleton that proves the FFI and trait
    // plumbing compiles. The full display init and thread spawning
    // will be added when the Zephyr west build environment is wired up.
}

// ── Frame budget update (called from present thread after each frame) ─────────

/// Update the ERIF-to-ERIF period estimate (EMA, α=1/8).
///
/// Called from the present thread after each successful present().
pub fn update_frame_budget(sync: &ZephyrFrameSync, prev_erif_cyc: &mut u32) {
    let now_cyc = sync.erif_cyccnt.load(Ordering::Acquire);
    let delta = now_cyc.wrapping_sub(*prev_erif_cyc);
    // Sanity: 8ms..80ms at 400MHz (3.2M..32M cycles)
    if *prev_erif_cyc != 0 && delta > 3_200_000 && delta < 32_000_000 {
        let old = sync.frame_budget.load(Ordering::Relaxed);
        let smoothed = (old / 8) * 7 + delta / 8;
        sync.frame_budget.store(smoothed, Ordering::Relaxed);
    }
    *prev_erif_cyc = now_cyc;
}
