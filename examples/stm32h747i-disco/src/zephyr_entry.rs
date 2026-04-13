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

// D-cache clean via Zephyr's SCB_CleanDCache (C wrapper).
unsafe extern "C" {
    fn rlvgl_dcache_clean();
}

/// Flush all dirty D-cache lines to SDRAM.
fn dcache_clean_all() {
    unsafe { rlvgl_dcache_clean() };
}

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

// ── Display info struct (matches C side) ──────────────────────────────────────

#[repr(C)]
pub struct RlvglDisplayInfo {
    pub fb_front: *mut u8,
    pub fb_back: *mut u8,
    pub fb_len: u32,
    pub width: u16,
    pub height: u16,
    pub pixel_size: u16,
}

// ── FFI: present via Zephyr display_write ─────────────────────────────────────

unsafe extern "C" {
    fn rlvgl_present(back_buf: *const u8, width: u16, height: u16) -> i32;
}

// ── Initialization entry point ────────────────────────────────────────────────

/// Called from Zephyr C `main()` to initialize the rlvgl display system.
///
/// Receives kernel object pointers and display info from the C side.
/// Sets up the sync object, initializes the heap, enables DMA2D,
/// and renders a test pattern to verify the display pipeline.
///
/// # Safety
///
/// All pointers must be valid with `'static` lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_init(
    erif_sem: *mut zephyr_sync::k_sem,
    dma2d_done_sem: *mut zephyr_sync::k_sem,
    display_info: *const RlvglDisplayInfo,
) {
    unsafe {
        // ── 1. Construct sync object ──────────────────────────────────────
        use core::sync::atomic::AtomicBool;
        static INIT_DONE: AtomicBool = AtomicBool::new(false);
        static mut SYNC_STORAGE: core::mem::MaybeUninit<ZephyrFrameSync> =
            core::mem::MaybeUninit::uninit();

        if INIT_DONE.swap(true, Ordering::AcqRel) {
            return;
        }

        let ptr = core::ptr::addr_of_mut!(SYNC_STORAGE);
        (*ptr).write(ZephyrFrameSync::new(erif_sem, dma2d_done_sem));
        SYNC.store((*ptr).as_mut_ptr(), Ordering::Release);

        // ── 2. Read display info ──────────────────────────────────────────
        let di = &*display_info;
        let fb_back = di.fb_back;
        let fb_w = di.width as u32;
        let fb_h = di.height as u32;
        let bpp = di.pixel_size as u32;

        // ── 3. Enable DMA2D clock (RCC AHB3ENR bit 4) ────────────────────
        let ahb3enr = 0x5802_44D4 as *mut u32;
        ahb3enr.write_volatile(ahb3enr.read_volatile() | (1 << 4));

        // ── 3b. Fix DSI color coding ─────────────────────────────────────
        // Zephyr's HAL_DSI_ConfigVideoMode should set LCOLCR to RGB888
        // (COLC=5, LPE=1) but the register reads 0 (RGB565). Force it.
        let lcolcr = 0x5000_0028 as *mut u32;
        lcolcr.write_volatile((1 << 8) | 5); // LPE=1, COLC=5 (RGB888)

        // ── 4. Decode splash into BOTH framebuffers ─────────────────────
        //
        // splash.rle is 480×800 portrait. Zephyr's LTDC FB is 800×480
        // landscape (800 pixels/line, 480 lines) because the panel MADCTL
        // handles rotation. We decode portrait into scratch SDRAM, then
        // copy-rotate 90° CW into landscape FBs.
        //
        // 90° CW rotation: portrait(px, py) → landscape(dst_x, dst_y)
        //   dst_x = py
        //   dst_y = (portrait_w - 1) - px
        //
        // Scratch buffer lives past the two FBs in SDRAM.
        let fb_front = di.fb_front;
        let fb_bytes = di.fb_len as usize;

        // Scratch at fb_front + 2 * fb_len (past both FBs)
        let scratch_base = fb_front.add(2 * fb_bytes);
        // Portrait dimensions for the splash asset
        let splash_w: usize = 480;
        let splash_h: usize = 800;
        let splash_bytes = splash_w * splash_h * 4;

        #[cfg(feature = "splash")]
        let splash_ok = (|| -> Option<()> {
            let blob = crate::SPLASH_RLE;
            let (w, h, pal_bytes, stream) =
                rlvgl_decomp::parse_rle_blob(blob).ok()?;
            if w as usize != splash_w || h as usize != splash_h {
                return None;
            }
            let pal_count = pal_bytes.len() / 2;
            let mut palette = [0u16; 192];
            for i in 0..pal_count {
                palette[i] =
                    u16::from_le_bytes([pal_bytes[i * 2], pal_bytes[i * 2 + 1]]);
            }

            // Decode portrait into scratch buffer
            let scratch = core::slice::from_raw_parts_mut(scratch_base, splash_bytes);
            rlvgl_decomp::decode_argb_into(
                splash_w, splash_h,
                &palette[..pal_count], stream, scratch,
            ).ok()?;

            // Copy-rotate 90° CW from portrait scratch into landscape FBs.
            // portrait(px, py) → landscape(dst_x=py, dst_y=479-px)
            let src = scratch_base as *const u32;
            let dst0 = fb_front as *mut u32;
            let dst1 = fb_back as *mut u32;
            let dst_stride = fb_w as usize; // 800 pixels per line

            for py in 0..splash_h {
                for px in 0..splash_w {
                    let pixel = src.add(py * splash_w + px).read_volatile();
                    let dx = py;
                    let dy = (splash_w - 1) - px;
                    let dst_idx = dy * dst_stride + dx;
                    dst0.add(dst_idx).write_volatile(pixel);
                    dst1.add(dst_idx).write_volatile(pixel);
                }
            }
            Some(())
        })().is_some();

        #[cfg(not(feature = "splash"))]
        let splash_ok = false;

        if !splash_ok {
            // Solid black fallback for both buffers
            let total = (fb_w * fb_h) as usize;
            for i in 0..total {
                (fb_front as *mut u32).add(i).write_volatile(0xFF00_0000);
                (fb_back as *mut u32).add(i).write_volatile(0xFF00_0000);
            }
        }

        dcache_clean_all();

        // Present back buffer (which now has splash or black).
        // Both buffers have identical content so either is fine.
        rlvgl_present(fb_back, di.width, di.height);
    }
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
