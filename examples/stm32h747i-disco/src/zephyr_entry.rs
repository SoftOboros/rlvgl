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
use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

// ── Touch input from Zephyr input subsystem ───────────────────────────────────

/// Matches C `struct rlvgl_touch_event`.
#[repr(C)]
struct TouchEventC {
    x: i16,
    y: i16,
    pressed: u8,
}

/// Packed touch state: x[15:0] | y[31:16] in one atomic, pressed in another.
static TOUCH_XY: AtomicU32 = AtomicU32::new(0);
static TOUCH_PRESSED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
static TOUCH_DIRTY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Called from C input callback — stores latest touch state atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_touch_event(evt: *const TouchEventC) {
    unsafe {
        let e = &*evt;
        let packed = (e.x as u16 as u32) | ((e.y as u16 as u32) << 16);
        TOUCH_XY.store(packed, Ordering::Relaxed);
        TOUCH_PRESSED.store(e.pressed != 0, Ordering::Relaxed);
        TOUCH_DIRTY.store(true, Ordering::Release);
    }
}

// ── Key input from Zephyr GPIO keys (joystick) ───────────────────────────────

/// Linux input key codes matching Zephyr's `zephyr,code` DTS values.
const KEY_ENTER: u16 = 28;
const KEY_UP: u16 = 103;
const KEY_DOWN: u16 = 108;
const KEY_LEFT: u16 = 105;
const KEY_RIGHT: u16 = 106;

/// Simple key event ring buffer (4 entries, enough for joystick).
static KEY_BUF: [AtomicU32; 4] = [
    AtomicU32::new(0), AtomicU32::new(0),
    AtomicU32::new(0), AtomicU32::new(0),
];
static KEY_WRITE: AtomicU32 = AtomicU32::new(0);
static KEY_READ: AtomicU32 = AtomicU32::new(0);

/// Called from C input callback for joystick key events.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rlvgl_key_event(code: u16, pressed: u8) {
    // Pack: code in low 16, pressed in bit 16, valid in bit 31
    let packed = (code as u32) | ((pressed as u32) << 16) | (1 << 31);
    let idx = KEY_WRITE.fetch_add(1, Ordering::Relaxed) as usize % KEY_BUF.len();
    KEY_BUF[idx].store(packed, Ordering::Release);
}

/// Consume the next key event if one is pending.
fn take_key() -> Option<(u16, bool)> {
    let r = KEY_READ.load(Ordering::Relaxed);
    let w = KEY_WRITE.load(Ordering::Acquire);
    if r == w {
        return None;
    }
    let idx = r as usize % KEY_BUF.len();
    let packed = KEY_BUF[idx].swap(0, Ordering::Acquire);
    if packed & (1 << 31) == 0 {
        return None;
    }
    KEY_READ.store(r.wrapping_add(1), Ordering::Relaxed);
    let code = (packed & 0xFFFF) as u16;
    let pressed = (packed >> 16) & 1 != 0;
    Some((code, pressed))
}

/// Consume the latest touch event if one is pending.
fn take_touch() -> Option<(i16, i16, bool)> {
    if !TOUCH_DIRTY.swap(false, Ordering::Acquire) {
        return None;
    }
    let packed = TOUCH_XY.load(Ordering::Relaxed);
    let x = packed as u16 as i16;
    let y = (packed >> 16) as u16 as i16;
    let pressed = TOUCH_PRESSED.load(Ordering::Relaxed);
    Some((x, y, pressed))
}

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
        // Delegate to shared dsi_cmd_mode module for register handling.
        if let Some(cyc) = rlvgl_platform::dsi_cmd_mode::handle_erif_isr() {
            if let Some(sync) = get_sync() {
                sync.isr_record_erif(cyc);
                zephyr_sync::k_sem_give(sync.erif_sem);
            }
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
        // The Dma2dBlitter doesn't enable any DMA2D interrupt sources
        // (CR.TCIE/TEIE/CEIE all stay 0), so this handler currently
        // never fires in practice. Kept registered as a safety net in
        // case future code enables IRQs — clear non-TC error flags so
        // the IRQ won't storm if/when that happens. TCIF is owned by
        // the polling `take_complete()` path in zephyr_sync.rs.
        const DMA2D_ISR: *const u32 = 0x5200_1004 as *const u32;
        const DMA2D_IFCR: *mut u32 = 0x5200_1008 as *mut u32;
        let isr = DMA2D_ISR.read_volatile();
        // Clear errors (CEIF=0, TEIF=5, TWIF=2, AEIF=4) but NOT TCIF (1).
        let clear = isr & 0b00111101;
        if clear != 0 {
            DMA2D_IFCR.write_volatile(clear);
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

// ── Zephyr StorageBrowser implementation ──────────────────────────────────────

/// Directory entry from C `rlvgl_readdir` callback.
#[repr(C)]
struct CDirent {
    name: [u8; 256],
    is_dir: u8,
    size: u32,
}

type ReaddirCb = unsafe extern "C" fn(entry: *const CDirent, ctx: *mut core::ffi::c_void);

unsafe extern "C" {
    fn rlvgl_readdir(
        path: *const u8,
        cb: ReaddirCb,
        ctx: *mut core::ffi::c_void,
    ) -> i32;
}

/// Collects directory entries from the C callback into a Vec.
unsafe extern "C" fn readdir_collect(entry: *const CDirent, ctx: *mut core::ffi::c_void) {
    unsafe {
        let entries = &mut *(ctx as *mut alloc::vec::Vec<rlvgl_ui::file_browser::FileEntry>);
        let e = &*entry;
        // Extract name (NUL-terminated C string in the buffer)
        let name_len = e.name.iter().position(|&b| b == 0).unwrap_or(e.name.len());
        let name = core::str::from_utf8_unchecked(&e.name[..name_len]);
        let kind = if e.is_dir != 0 {
            rlvgl_ui::file_browser::EntryKind::Directory
        } else if name.ends_with(".wav") || name.ends_with(".WAV") {
            rlvgl_ui::file_browser::EntryKind::WavFile
        } else {
            rlvgl_ui::file_browser::EntryKind::OtherFile
        };
        entries.push(rlvgl_ui::file_browser::FileEntry {
            name: alloc::string::String::from(name),
            kind,
        });
    }
}

/// StorageBrowser backed by Zephyr's filesystem API.
pub struct ZephyrStorageBrowser;

impl rlvgl_ui::file_browser::StorageBrowser for ZephyrStorageBrowser {
    fn list_devices(&mut self) -> alloc::vec::Vec<rlvgl_ui::file_browser::FileEntry> {
        alloc::vec![rlvgl_ui::file_browser::FileEntry {
            name: alloc::string::String::from("SD Card"),
            kind: rlvgl_ui::file_browser::EntryKind::Device,
        }]
    }

    fn list_directory(
        &mut self,
        _device_index: usize,
        path: &str,
    ) -> Result<
        alloc::vec::Vec<rlvgl_ui::file_browser::FileEntry>,
        rlvgl_ui::file_browser::StorageBrowserError,
    > {
        // Build the full path: "/SD:" + path
        let mut full_path = alloc::string::String::from("/SD:");
        if !path.starts_with('/') {
            full_path.push('/');
        }
        full_path.push_str(path);
        full_path.push('\0'); // NUL terminator for C

        let mut entries = alloc::vec::Vec::new();
        let ret = unsafe {
            rlvgl_readdir(
                full_path.as_ptr(),
                readdir_collect,
                &mut entries as *mut _ as *mut core::ffi::c_void,
            )
        };
        if ret < 0 {
            return Err(rlvgl_ui::file_browser::StorageBrowserError::Unavailable);
        }
        Ok(entries)
    }
}

// ── FFI: present via Zephyr display_write ─────────────────────────────────────

unsafe extern "C" {
    fn rlvgl_present(back_buf: *const u8, width: u16, height: u16) -> i32;
}

// ── Debug: one-shot RCC/LTDC/DSI clock-gate dump ──────────────────────────────
//
// Used to diagnose the v0.2.0 ACM star-crawl lockup. RM0433/RM0468 §8.7.1:
// on dual-core H747 (RM0399) the per-CPU LPENR views (`RCC_C1_*LPENR` at
// 0x130+) are SEPARATE registers from the D-domain views (`RCC_*LPENR` at
// 0x0D0+); on single-core H743 they alias the same physical register.
// CSleep gating for code running on CM7 uses the C1 view. We dump both
// so a discrepancy proves whether `feedback_h747_c1_lpenr.md` is correct.
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
unsafe fn dump_clock_state_oneshot(tag: &[u8]) {
    use core::sync::atomic::{AtomicBool, Ordering};
    static DUMPED: AtomicBool = AtomicBool::new(false);
    if DUMPED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    // Spin-wait for USART1 TXE then push one byte.
    fn u1c(c: u8) {
        unsafe {
            let isr = 0x4001_101C as *const u32;
            let tdr = 0x4001_1028 as *mut u32;
            let mut t = 100_000u32;
            while isr.read_volatile() & (1 << 7) == 0 {
                t -= 1;
                if t == 0 {
                    return;
                }
            }
            tdr.write_volatile(c as u32);
        }
    }
    fn u1s(s: &[u8]) {
        for &b in s {
            u1c(b);
        }
    }
    fn u1hex(v: u32) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for i in (0..8).rev() {
            let n = ((v >> (i * 4)) & 0xF) as usize;
            u1c(HEX[n]);
        }
    }
    fn pair(label: &[u8], addr: usize) {
        u1s(label);
        u1c(b'=');
        unsafe { u1hex((addr as *const u32).read_volatile()) };
        u1c(b' ');
    }

    const RCC: usize = 0x5802_4400;
    u1s(b"\r\n=== CLK DUMP ");
    u1s(tag);
    u1s(b" ===\r\n");

    // D-domain ENR (what we wrote)
    pair(b"AHB3ENR  ", RCC + 0x0D4);
    pair(b"AHB4ENR  ", RCC + 0x0E0);
    pair(b"APB3ENR  ", RCC + 0x0E4);
    u1s(b"\r\n");

    // D-domain LPENR (what we wrote)
    pair(b"AHB3LPENR", RCC + 0x13C);
    pair(b"AHB4LPENR", RCC + 0x148);
    pair(b"APB3LPENR", RCC + 0x14C);
    u1s(b"\r\n");

    // CPU1 (CM7) view — H747 RM0399 specific. Per RM0468 Table 71/75
    // and the §8.7.1 mapping diagram, C1_*ENR/LPENR live at +0x60 from
    // their D-domain counterparts. If RM0399 numbers H747 differently
    // these reads will simply show the alias value — no harm.
    pair(b"C1_AHB3EN", RCC + 0x134);
    pair(b"C1_AHB4EN", RCC + 0x140);
    pair(b"C1_APB3EN", RCC + 0x144);
    u1s(b"\r\n");
    pair(b"C1_AHB3LP", RCC + 0x19C);
    pair(b"C1_AHB4LP", RCC + 0x1A8);
    pair(b"C1_APB3LP", RCC + 0x1AC);
    u1s(b"\r\n");

    // PLL + LCDCLK source
    pair(b"RCC_CR   ", RCC + 0x000);
    pair(b"PLL3DIVR ", RCC + 0x040);
    pair(b"D1CCIPR  ", RCC + 0x04C);
    u1s(b"\r\n");

    // LTDC sanity — READ-only at the correct H747 base (0x5000_1000).
    // Earlier version wrote GCR at 0x5001_0018 (off-by-nibble) which
    // causes a precise data bus fault on H747. Keep this a pure read.
    const LTDC: usize = 0x5000_1000;
    let gcr_p = (LTDC + 0x18) as *const u32;
    let gcr = gcr_p.read_volatile();
    u1s(b"LTDC_GCR=");
    u1hex(gcr);
    u1s(b"\r\n");

    // DSI host CR / WCR / WCFGR / WISR
    const DSI: usize = 0x5000_0000;
    pair(b"DSI_CR   ", DSI + 0x000);
    pair(b"DSI_WCR  ", DSI + 0x404);
    pair(b"DSI_WCFGR", DSI + 0x400);
    pair(b"DSI_WISR ", DSI + 0x40C);
    u1s(b"\r\n=== END ===\r\n");
}

/// Present a frame — delegates to either dsi_cmd_mode::present (adapted cmd)
/// or Zephyr display_write (video mode) depending on the feature.
#[inline]
unsafe fn do_present(buf: *mut u8, width: u16, height: u16) {
    #[cfg(feature = "adapted_cmd")]
    {
        let _ = (width, height); // unused in adapted cmd mode
        rlvgl_platform::dsi_cmd_mode::present(buf as u32);
    }
    #[cfg(not(feature = "adapted_cmd"))]
    {
        rlvgl_present(buf, width, height);
    }
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
        // SRAM3 breadcrumb: rlvgl_init entered
        (0x3800_0204 as *mut u32).write_volatile(0xB1A1_0001);

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
        let bpp = di.pixel_size as u32;

        // ── 3. Enable DMA2D clock (RCC AHB3ENR bit 4) ────────────────────
        let ahb3enr = 0x5802_44D4 as *mut u32;
        ahb3enr.write_volatile(ahb3enr.read_volatile() | (1 << 4));

        // ── 3b. Fix DSI color coding ─────────────────────────────────────
        // Zephyr's HAL_DSI_ConfigVideoMode should set LCOLCR to RGB888
        // (COLC=5, LPE=1) but the register reads 0 (RGB565). Force it.
        let lcolcr = 0x5000_0028 as *mut u32;
        lcolcr.write_volatile((1 << 8) | 5); // LPE=1, COLC=5 (RGB888)

        // ── 3c. Adapted command mode switch (optional) ───────────────────
        // When `adapted_cmd` feature is enabled, reconfigure the DSI from
        // Zephyr's video mode to adapted command mode. This gives DMA2D
        // exclusive SDRAM access after each scan (ERIF ISR clears LTDCEN).
        //
        // RM0399 §34.16.1: "DSIM must only be changed when DSI_CR.EN = 0"
        // ── 3c. adapted_cmd: full Rust DSI+LTDC init ────────────────────
        // Plan B: bypass Zephyr's video-mode display drivers entirely.
        // Requires the `adapted_cmd.overlay` DTS overlay to disable
        // &zephyr_mipi_dsi, &nt35510, &zephyr_lcd_controller. Zephyr
        // still provides clocks (PLL3), SDRAM, GPIO, I2C, and kernel.
        //
        // Build with:
        //   west build -- -DEXTRA_DTC_OVERLAY_FILE=adapted_cmd.overlay
        #[cfg(feature = "adapted_cmd")]
        {
            use rlvgl_platform::display_init;
            (0x3800_0204 as *mut u32).write_volatile(0xB1A1_0010);
            // Ensure peripheral clocks (DMA2D/LTDC/DSI) are enabled,
            // and PLL3 is locked. Safe even if Zephyr already did so.
            display_init::enable_display_peripheral_clocks();
            (0x3800_0204 as *mut u32).write_volatile(0xB1A1_0011);
            display_init::ensure_pll3_running();
            (0x3800_0204 as *mut u32).write_volatile(0xB1A1_0012);
            // Full DSI+LTDC bring-up in adapted command mode.
            // Uses fb_front (= 0xD0000000) as the initial scan buffer.
            let ok = display_init::init_full_adapted_cmd(di.fb_front as u32);
            (0x3800_0204 as *mut u32).write_volatile(if ok { 0xB1A1_0013 } else { 0xDEAD_D51A });
        }

        // ── 3d. Determine FB layout ──────────────────────────────────────
        // In adapted_cmd mode, LTDC scans portrait (480×800) like bare-metal.
        // In video mode, LTDC scans landscape (800×480).
        #[cfg(feature = "adapted_cmd")]
        let (fb_w, fb_h) = (480u32, 800u32);
        #[cfg(not(feature = "adapted_cmd"))]
        let (fb_w, fb_h) = (di.width as u32, di.height as u32);

        // ── 4. Decode splash into BOTH framebuffers ─────────────────────
        //
        // splash.rle is 480×800 portrait ARGB8888.
        //
        // In adapted_cmd mode, the LTDC scans portrait (480×800) matching
        // bare-metal — decode directly, no rotation needed.
        //
        // In video mode, the LTDC scans landscape (800×480) — decode into
        // scratch SDRAM, then copy-rotate 90° CW into landscape FBs.
        let fb_front = di.fb_front;
        let fb_bytes = (fb_w * fb_h * bpp) as usize;

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

            #[cfg(feature = "adapted_cmd")]
            {
                // Portrait FB (480×800) — decode directly into both FBs.
                let fb0 = core::slice::from_raw_parts_mut(fb_front, splash_bytes);
                rlvgl_decomp::decode_argb_into(
                    splash_w, splash_h,
                    &palette[..pal_count], stream, fb0,
                ).ok()?;
                // Copy to back buffer
                core::ptr::copy_nonoverlapping(fb_front, fb_back, splash_bytes);
            }

            #[cfg(not(feature = "adapted_cmd"))]
            {
                // Landscape FB (800×480) — decode portrait into scratch,
                // then copy-rotate 90° CW.
                let scratch = core::slice::from_raw_parts_mut(scratch_base, splash_bytes);
                rlvgl_decomp::decode_argb_into(
                    splash_w, splash_h,
                    &palette[..pal_count], stream, scratch,
                ).ok()?;

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

        // ── 4b. Save pristine desktop for background restoration ────────
        // Copy front buffer (rotated splash) to a third SDRAM region.
        // Fixed at 0xD030_0000 to match bare-metal's `PRISTINE` constant
        // in `star_crawl::blit_splash_crop_a8` so the splash-graphic
        // A8 pre-render reads valid pixels on both build flavors. SDRAM
        // gap between fb_back end (0xD097_6E00) and crawl starfield at
        // 0xD100_0000 has 6.5 MB free; 0xD030_0000..0xD047_6E00 fits.
        let pristine_base = 0xD030_0000usize as *mut u8;
        core::ptr::copy_nonoverlapping(fb_front, pristine_base, fb_bytes);

        dcache_clean_all();

        // adapted_cmd: re-write ALL LTDC config (timing + layer + GCR).
        // Some post-display_init code path was clearing these — likely
        // Zephyr SYS_INIT for the disabled LTDC node still touches the
        // peripheral. Re-writing them here makes them stick.
        #[cfg(feature = "adapted_cmd")]
        {
            use rlvgl_platform::display_init;
            // Re-enable peripheral clocks in case Zephyr power management
            // disabled them while the OS was settling.
            display_init::enable_display_peripheral_clocks();
            display_init::configure_ltdc_timing(480, 800, 2, 34, 34, 120, 150, 150);
            display_init::setup_ltdc_layer(
                fb_front as u32, 480, 800, 2, 34, 120, 150,
            );
            display_init::enable_ltdc();
            // Diagnostic dump — confirms LTDC/DSI/PLL3 register state matches
            // expectations for the adapted_cmd path.
            display_init::dump_registers();
        }

        // Present splash immediately so it's visible during widget init.
        do_present(fb_back, di.width, di.height);

        // ── 5. Initialize heap ───────────────────────────────────────────
        {
            const HEAP_SIZE: usize = 64 * 1024;
            static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
            let start = core::ptr::addr_of_mut!(HEAP_MEM) as usize;
            crate::ALLOC.init(start, HEAP_SIZE);
        }

        // ── 6. Build widget tree and run render loop ─────────────────────
        {
            use alloc::rc::Rc;
            use core::cell::RefCell;
            use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoCommand, DiscoController};
            use rlvgl_platform::blit::{BlitterRenderer, PixelFmt, Surface};
            use rlvgl_platform::cpu_blitter::CpuBlitter;
            use rlvgl_platform::screen::Screen;
            use rlvgl_core::WidgetNode;

            // Controller lays out widgets in *landscape* logical coords —
            // 800w × 480h. The RotatedRenderer below rotates 90° CCW into
            // the physical portrait FB (480w × 800h) so that on the user's
            // view (panel mounted with long edge horizontal), an icon at
            // logical x=730 ends up at user's right edge.
            //
            // For the non-ACM (Zephyr video mode) build, the FB is already
            // landscape and no rotation is needed; the controller still
            // uses the same logical coords.
            #[cfg(feature = "adapted_cmd")]
            let screen = Screen::landscape(800, 480);
            #[cfg(not(feature = "adapted_cmd"))]
            let screen = Screen::landscape(fb_w, fb_h);
            let mut controller = DiscoController::new(
                screen,
                DiscoCapabilities::zephyr(),
            );
            let root = controller.root();

            // File browser panel backed by Zephyr filesystem
            static FONT_DATA: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-24.bin");
            static UI_FONT_FB: rlvgl_core::packed_font::PackedFont =
                rlvgl_core::packed_font::PackedFont {
                    height: 24,
                    ascent: 22,
                    glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                    data: FONT_DATA,
                };
            let storage: Rc<RefCell<dyn rlvgl_ui::file_browser::StorageBrowser>> =
                Rc::new(RefCell::new(ZephyrStorageBrowser));
            let file_browser = Rc::new(RefCell::new(
                crate::file_browser_panel::FileBrowserPanel::new(&UI_FONT_FB, storage),
            ));
            root.borrow_mut().children.push(WidgetNode {
                widget: file_browser.clone(),
                children: alloc::vec::Vec::new(),
                tag: Some("disco.file_browser"),
            });

            // ── Star crawl setup ─────────────────────────────────────
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            let mut star_crawl = {
                use crate::star_crawl::StarCrawl;
                // Use the same bold 32 pt font and README crawl text
                // as the bare-metal path so the two platforms render
                // the same content.
                static BOLD_FONT_DATA: &[u8] =
                    include_bytes!("../assets/fonts/DejaVuSans-Bold-32.bin");
                static CRAWL_FONT: rlvgl_core::packed_font::PackedFont =
                    rlvgl_core::packed_font::PackedFont {
                        height: 32,
                        ascent: 30,
                        glyphs: &crate::fonts::DEJAVU_SANS_BOLD_32_GLYPHS,
                        data: BOLD_FONT_DATA,
                    };
                let crawl_lines = crate::readme_crawl::README_CRAWL;
                // Frame rate hint — star_crawl scales pixels-per-frame
                // from SCROLL_PX_PER_SEC (40) / frame_hz.
                let mut c = StarCrawl::new(&CRAWL_FONT, crawl_lines, 30);
                // Zephyr path uses a persistent scratch buffer
                // (0xD180_0000) across frames, so the crawl can shift
                // the existing back_buf up by the scroll delta and
                // only re-fill the newly-exposed bottom rows. See
                // task #40 / RenderMode::Incremental doc.
                c.set_render_mode(crate::star_crawl::RenderMode::Incremental);
                c
            };
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            let mut crawl_dma2d: Option<rlvgl_platform::dma2d::Dma2dBlitter> = None;
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            let mut crawl_active = false;
            // Portrait scratch buffer for crawl output (480×720×4 = 1.3MB)
            // Lives at CRAWL_BASE in SDRAM Bank 2
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            const CRAWL_FB_W: u32 = 480;
            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
            const CRAWL_FB_H: u32 = 800;

            // After first present, LTDC displays back buffer (now front).
            // The "back" for rendering is the original front buffer.
            // Track which buffer to render into.
            let mut render_buf = fb_front; // original front is now the back

            // Enable DWT cycle counter for frame-timing instrumentation.
            // CoreDebug->DEMCR |= TRCENA, DWT->CTRL |= CYCCNTENA.
            unsafe {
                const DEMCR: *mut u32 = 0xE000_EDFC as *mut u32;
                const DWT_CTRL: *mut u32 = 0xE000_1000 as *mut u32;
                DEMCR.write_volatile(DEMCR.read_volatile() | (1 << 24));
                DWT_CTRL.write_volatile(DWT_CTRL.read_volatile() | 1);
            }

            // Edge-detect touch state so each finger-down generates exactly
            // one PressRelease + DoubleTap pair, not one per Zephyr input
            // sample. Without this the file-menu icon was being toggled
            // open/closed at the input sample rate (~2-4 Hz) which read
            // as flicker.
            let mut prev_touch_pressed: bool = false;

            // Loop heartbeat: prints '.' every N iterations so we can tell
            // if the render loop is alive vs locked. Remove once stable.
            let mut hb_count: u32 = 0;
            loop {
                // Heartbeat — print on first iteration, then every 30
                // (~1s at 33ms sleep). Char '!' = first, '.' = periodic,
                // '0'..'9' = which point in the loop body we last reached.
                hb_count = hb_count.wrapping_add(1);
                // Suppress the per-iter digit when crawl is active — each
                // digit is ~90us of USART1 TX and with ~160 outer iters
                // per crawl frame that's ~15ms/frame just in heartbeat
                // serial chars. F-line summary carries the info we need.
                let want_print = !crawl_active;
                let hb_emit = |c: u8| {
                    let isr = 0x4001_101C as *const u32;
                    let tdr = 0x4001_1028 as *mut u32;
                    let mut t = 100_000u32;
                    while unsafe { isr.read_volatile() } & (1 << 7) == 0 {
                        t -= 1;
                        if t == 0 { return; }
                    }
                    unsafe { tdr.write_volatile(c as u32) };
                };
                if want_print {
                    // Print iter count modulo 10 so we get '0','1','2',...,'9'
                    // rolling. Newline every 10 to keep output readable.
                    let d = b'0' + (hb_count % 10) as u8;
                    hb_emit(d);
                    if hb_count % 10 == 0 { hb_emit(b'\n'); }
                }
                // First-iters checkpoint helper: prints char on iters 1-3
                // so we can localize where the lock occurs.
                let mark1 = |c: u8| {
                    if hb_count <= 3 { hb_emit(c); }
                };
                if hb_count == 2 { hb_emit(b'\n'); }
                if hb_count == 3 { hb_emit(b'\n'); }
                mark1(b'A'); // entered loop body

                // Drain serial RX for debug triggers. 'c'/'C' fires the
                // star crawl. Implements a tiny subset of the playit
                // protocol for remote test automation; full playit
                // wiring is a follow-up task.
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                loop {
                    let b = rlvgl_serial_poll_rx();
                    if b < 0 {
                        break;
                    }
                    match b as u8 {
                        b'c' | b'C' => {
                            if crawl_dma2d.is_none() {
                                crawl_dma2d = Some(
                                    rlvgl_platform::dma2d::Dma2dBlitter::steal(),
                                );
                            }
                            if let Some(ref mut dma2d) = crawl_dma2d {
                                star_crawl.activate(dma2d);
                            }
                            crawl_active = true;
                            controller.publish_status("serial 'c' → crawl");
                            dump_clock_state_oneshot(b"serial-c-crawl");
                        }
                        _ => {}
                    }
                }
                // Process joystick key events
                {
                    use rlvgl_core::event::{Event, Key};
                    while let Some((code, pressed)) = take_key() {
                        let key = match code {
                            KEY_UP => Some(Key::ArrowUp),
                            KEY_DOWN => Some(Key::ArrowDown),
                            KEY_LEFT => Some(Key::ArrowLeft),
                            KEY_RIGHT => Some(Key::ArrowRight),
                            KEY_ENTER => Some(Key::Enter),
                            _ => None,
                        };
                        if let Some(k) = key {
                            if pressed {
                                let is_enter = matches!(k, Key::Enter);
                                controller.dispatch_event(&Event::KeyDown { key: k });
                                // DEBUG: Enter key fires the star crawl
                                // trigger inline, bypassing the normal
                                // UI menu navigation. Lets us validate
                                // the ACM crawl path without needing to
                                // reach InfoSlot::StarCrawl via touch.
                                // Remove once the regular menu hook is
                                // wired on Zephyr.
                                if is_enter {
                                    #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                                    {
                                        if crawl_dma2d.is_none() {
                                            crawl_dma2d = Some(
                                                rlvgl_platform::dma2d::Dma2dBlitter::steal(),
                                            );
                                        }
                                        if let Some(ref mut dma2d) = crawl_dma2d {
                                            star_crawl.activate(dma2d);
                                        }
                                        crawl_active = true;
                                        controller.publish_status(
                                            "Enter → crawl activated",
                                        );
                                        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                                        dump_clock_state_oneshot(b"enter-key-crawl");
                                    }
                                }
                            }
                        }
                    }
                }

                // Process touch input.
                // Zephyr FT5336 reports raw panel coordinates. With
                // rotation=90, transform to landscape:
                //   landscape_x = raw_y
                //   landscape_y = (panel_height - 1) - raw_x
                if let Some((raw_x, raw_y, pressed)) = take_touch() {
                    // Zephyr FT5336 driver already reports landscape coords
                    // but Y is inverted (high=top, low=bottom).
                    let lx = raw_x as i32;
                    let ly = 479 - raw_y as i32;

                    // Trace touch to serial
                    fn u1_putc(c: u8) {
                        unsafe {
                            let isr = 0x4001_101C as *const u32;
                            let tdr = 0x4001_1028 as *mut u32;
                            while isr.read_volatile() & (1 << 7) == 0 {}
                            tdr.write_volatile(c as u32);
                        }
                    }
                    fn u1_dec(mut v: i32) {
                        if v < 0 { u1_putc(b'-'); v = -v; }
                        let mut buf = [0u8; 10];
                        let mut i = 0;
                        if v == 0 { u1_putc(b'0'); return; }
                        while v > 0 { buf[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
                        while i > 0 { i -= 1; u1_putc(buf[i]); }
                    }
                    for &c in b"T:" { u1_putc(c); }
                    u1_dec(raw_x as i32);
                    u1_putc(b',');
                    u1_dec(raw_y as i32);
                    for &c in b"->" { u1_putc(c); }
                    u1_dec(lx);
                    u1_putc(b',');
                    u1_dec(ly);
                    u1_putc(if pressed { b'D' } else { b'U' });
                    for &c in b"\r\n" { u1_putc(c); }

                    // Only dispatch on the rising edge (finger newly down).
                    // Held / repeat samples are ignored so the file-menu
                    // hotspot doesn't toggle every input frame.
                    let press_edge = pressed && !prev_touch_pressed;
                    prev_touch_pressed = pressed;

                    if press_edge {
                        use rlvgl_core::event::Event;
                        // ── DEBUG HOTSPOT (top-right corner) ───────────
                        // Tap the top-right ~60×60 px corner to force the
                        // star crawl to start, bypassing the normal menu
                        // navigation. Used to validate the H0 (C1_LPENR)
                        // fix without depending on InfoSlot::StarCrawl
                        // being reachable from the current UI state.
                        // Remove once the regular play path is wired.
                        if ly < 150 && lx > (di.width as i32 - 150) {
                            // Inline the crawl bring-up — bypasses
                            // controller queue (no public `queue_command`).
                            controller.publish_status("Crawl hotspot tapped");
                            #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                            {
                                if crawl_dma2d.is_none() {
                                    crawl_dma2d = Some(
                                        rlvgl_platform::dma2d::Dma2dBlitter::steal(),
                                    );
                                }
                                if let Some(ref mut dma2d) = crawl_dma2d {
                                    star_crawl.activate(dma2d);
                                }
                                crawl_active = true;
                                #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                                dump_clock_state_oneshot(b"hotspot-trigger");
                            }
                        } else {
                            // Send both PressRelease (select) and DoubleTap
                            // (navigate) — crude until gesture recognizer is
                            // integrated. FileBrowser uses DoubleTap to enter.
                            controller.dispatch_event(&Event::PressRelease {
                                x: lx,
                                y: ly,
                            });
                            controller.dispatch_event(&Event::DoubleTap {
                                x: lx,
                                y: ly,
                            });
                        }
                    }
                }

                mark1(b'B'); // input processed
                // Skip UI controller work while the star crawl is
                // running. The widget tree is hidden and UI events
                // are ignored during the crawl; walking it every
                // outer iter was adding ~80 ms per crawl frame.
                if !crawl_active {
                    controller.tick();
                }
                mark1(b'C'); // controller.tick done

                // Process commands from the controller
                for cmd in controller.drain_commands() {
                    match cmd {
                        DiscoCommand::LoadStorageSummary => {
                            file_browser.borrow_mut().toggle();
                        }
                        #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                        DiscoCommand::StartEffect(rlvgl_app_disco_demo::DiscoEffect::StarCrawl) => {
                            #[cfg(feature = "adapted_cmd")]
                            {
                                // Adapted command mode: DMA2D M2M works.
                                // Bring up the crawl renderer state and
                                // mark it active. Note: as of v0.2.0 the
                                // tick path itself still has open issues
                                // (see comments around the `crawl_active`
                                // block below); enabling this triggers a
                                // present-pipeline lock under crawl_active.
                                if crawl_dma2d.is_none() {
                                    crawl_dma2d = Some(rlvgl_platform::dma2d::Dma2dBlitter::steal());
                                }
                                if let Some(ref mut dma2d) = crawl_dma2d {
                                    star_crawl.activate(dma2d);
                                }
                                crawl_active = true;
                                // Snapshot RCC/LTDC/DSI state at the moment
                                // the crawl is requested. See dump_clock_state_oneshot
                                // header comment + memory `feedback_h747_c1_lpenr.md`.
                                #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
                                dump_clock_state_oneshot(b"crawl-trigger");
                                controller.publish_status("Star crawl started");
                            }
                            #[cfg(not(feature = "adapted_cmd"))]
                            {
                                // Video mode: DMA2D M2M hangs (AXI bus starvation).
                                controller.publish_status("Star crawl requires adapted_cmd feature");
                            }
                        }
                        _ => {}
                    }
                }

                // ── Star crawl rendering ──────────────────────────────
                // PREVIOUS ISSUE (resolved 2026-04-15): this block locked
                // the display because CM7 CSleep gating during the widget
                // loop's k_sleep was killing LTDC/DSI/DMA2D clocks. The
                // root cause was that on the H747 dual-core, *LPENR has a
                // separate per-CPU C1 view (RCC_C1_*LPENR) that is NOT
                // aliased to the D-domain register, and our previous fix
                // wrote only the D-domain view. Fixed in
                // display_init::enable_display_peripheral_clocks by
                // mirroring writes to RCC_C1_*LPENR. Re-enabled here.
                // See feedback_h747_c1_lpenr.md.
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                if crawl_active {
                    if let Some(ref mut dma2d) = crawl_dma2d {
                        // Reverted: render to a scratch buffer, compose
                        // via DMA2D blit to render_buf. Rendering
                        // directly into render_buf tripled tick count
                        // per frame (11k → 32k) despite double-
                        // buffering — likely SDRAM bank / AXI port
                        // contention between DMA2D row blits and the
                        // current/prior LTDC scan target. TBD.
                        let crawl_buf = 0xD180_0000usize as *mut u8;
                        let sync_ref = get_sync().unwrap_unchecked();
                        // Per-iteration trace so we can see where tick stalls.
                        // 'T' = entering tick; result tag follows.
                        fn u1c(c: u8) {
                            unsafe {
                                let isr = 0x4001_101C as *const u32;
                                let tdr = 0x4001_1028 as *mut u32;
                                let mut t = 100_000u32;
                                while isr.read_volatile() & (1 << 7) == 0 {
                                    t -= 1;
                                    if t == 0 { return; }
                                }
                                tdr.write_volatile(c as u32);
                            }
                        }
                        fn u1s(s: &[u8]) { for &b in s { u1c(b); } }
                        fn u1hex(v: u32) {
                            const HEX: &[u8; 16] = b"0123456789abcdef";
                            for i in (0..8).rev() {
                                let n = ((v >> (i * 4)) & 0xF) as usize;
                                u1c(HEX[n]);
                            }
                        }
                        // Dump star_crawl diag_words every ~16 ticks so we
                        // see progress promptly. Iter rate is slow (~2 iter/s)
                        // when crawl is active so 256 would take > 2 minutes.
                        static mut TICK_COUNT: u32 = 0;
                        unsafe {
                            TICK_COUNT = TICK_COUNT.wrapping_add(1);
                            if TICK_COUNT % 16 == 0 {
                                let (w0, w1, w2, w3) = star_crawl.diag_words();
                                u1s(b"\r\nDIAG ");
                                u1hex(w0);
                                u1c(b' ');
                                u1hex(w1);
                                u1c(b' ');
                                u1hex(w2);
                                u1c(b' ');
                                u1hex(w3);
                                u1s(b"\r\n");
                                // Also DMA2D ISR + CR
                                let dma2d_isr = (0x5200_1004 as *const u32).read_volatile();
                                let dma2d_cr = (0x5200_1000 as *const u32).read_volatile();
                                u1s(b"DMA2D ISR=");
                                u1hex(dma2d_isr);
                                u1s(b" CR=");
                                u1hex(dma2d_cr);
                                u1s(b"\r\n");
                            }
                        }
                        // Batch many ticks per loop iteration. Each tick
                        // is ~1 starfield row blit + 1 FIR text row — the
                        // whole frame needs ~800 ticks. Without batching,
                        // each outer-loop iter prints ~4 serial chars
                        // (~350us at 115200 baud) which dominates
                        // per-tick cost. Batching 64 lets us amortize
                        // that and lets the DMA2D chain back-to-back.
                        //
                        // Instrumentation: count ticks spent in this
                        // batch + how many of them returned Pending
                        // because DMA2D was still in-flight (detected
                        // via star_crawl.waiting_for_dma()). Also time
                        // the batch via DWT cycles for per-frame cost.
                        const DWT_CYC: *const u32 = 0xE000_1004 as *const u32;
                        static mut TICKS_THIS_FRAME: u32 = 0;
                        static mut DMA_WAIT_TICKS: u32 = 0;
                        static mut BATCH_CYC_ACCUM: u32 = 0;
                        static mut LAST_FRAME_CYC: u32 = 0;
                        let batch_start = DWT_CYC.read_volatile();
                        let mut result = crate::star_crawl::StepResult::Pending;
                        for _ in 0..1024 {
                            result = star_crawl.tick(
                                dma2d, crawl_buf, CRAWL_FB_W, CRAWL_FB_H, sync_ref,
                            );
                            TICKS_THIS_FRAME += 1;
                            if star_crawl.waiting_for_dma() {
                                DMA_WAIT_TICKS += 1;
                            }
                            if !matches!(result, crate::star_crawl::StepResult::Pending) {
                                break;
                            }
                        }
                        let batch_end = DWT_CYC.read_volatile();
                        BATCH_CYC_ACCUM =
                            BATCH_CYC_ACCUM.wrapping_add(batch_end.wrapping_sub(batch_start));
                        // Print one tag char on meaningful transitions
                        // only (FrameReady / Finished / Idle). Skip the
                        // noisy per-Pending-batch to keep UART free.
                        let tag = match result {
                            crate::star_crawl::StepResult::FrameReady => Some(b'F'),
                            crate::star_crawl::StepResult::Idle => Some(b'I'),
                            crate::star_crawl::StepResult::Finished => Some(b'X'),
                            crate::star_crawl::StepResult::Pending => None,
                        };
                        if let Some(t) = tag {
                            u1c(t);
                            // On FrameReady, print a compact timing line:
                            //   F <ticks> <dma_wait_ticks> <frame_ms> <last_ms>
                            // where:
                            //   ticks        = total star_crawl.tick calls in frame
                            //   dma_wait     = ticks where waiting_for_dma() was true
                            //   frame_ms     = wall-clock ms in the tick loops for this frame
                            //   last_ms      = ms since previous FrameReady (includes blit+present)
                            if t == b'F' {
                                let now_cyc = DWT_CYC.read_volatile();
                                let since_last = now_cyc.wrapping_sub(LAST_FRAME_CYC);
                                LAST_FRAME_CYC = now_cyc;
                                let frame_ms = BATCH_CYC_ACCUM / 400_000;
                                let last_ms = since_last / 400_000;
                                u1c(b' ');
                                u1hex(TICKS_THIS_FRAME);
                                u1c(b' ');
                                u1hex(DMA_WAIT_TICKS);
                                u1c(b' ');
                                u1hex(frame_ms);
                                u1c(b' ');
                                u1hex(last_ms);
                                TICKS_THIS_FRAME = 0;
                                DMA_WAIT_TICKS = 0;
                                BATCH_CYC_ACCUM = 0;
                            }
                            u1c(b'\r');
                            u1c(b'\n');
                        }
                        match result {
                            crate::star_crawl::StepResult::FrameReady => {
                                // Direct blit from crawl scratch (480×800) to
                                // the portrait FB (480×800). Both buffers are
                                // the same shape and orientation since
                                // star_crawl now renders at the full portrait
                                // height (FB_H=800); no rotation needed.
                                //
                                // Architecture per design note: the crawl is
                                // logically independent of display dimensions
                                // — it renders to its own SRAM scratch at its
                                // chosen aspect, and a COMPOSITION step here
                                // blits it onto the final display surface.
                                // Today that composition is a 1:1 memcpy; in
                                // future it can scale / offset / clip as
                                // needed.
                                // DMA2D M2M blit of the full 480×800 crawl
                                // scratch to the display FB. Much faster
                                // than a CPU memcpy of the 1.5 MB buffer
                                // and bypasses D-cache entirely (DMA2D
                                // reads/writes SDRAM directly, so the
                                // later dcache_clean_all is only needed
                                // to flush any stale cache lines that
                                // would otherwise clobber our just-written
                                // pixels via eviction).
                                let t0 = DWT_CYC.read_volatile();
                                let stride_bytes = (fb_w * bpp) as u32;
                                dma2d.blit_raw(
                                    crawl_buf as *const u8,
                                    stride_bytes,
                                    render_buf,
                                    stride_bytes,
                                    fb_w as u32,
                                    fb_h as u32,
                                    rlvgl_platform::PixelFmt::Argb8888,
                                );
                                let t1 = DWT_CYC.read_volatile();
                                do_present(render_buf, di.width, di.height);
                                let t2 = DWT_CYC.read_volatile();
                                u1s(b" bl=");
                                u1hex(t1.wrapping_sub(t0) / 400);
                                u1s(b"us pr=");
                                u1hex(t2.wrapping_sub(t1) / 400);
                                u1s(b"us");
                                // Advance scroll so the next frame uses a
                                // new star_scroll_q8 + text scroll_q8. Without
                                // this call the crawl re-renders the identical
                                // frame forever (stationary starfield, no
                                // text motion). Bare-metal calls this after
                                // its present; mirroring here.
                                star_crawl.advance_scroll();
                                render_buf = if render_buf == fb_front { fb_back } else { fb_front };
                                continue; // skip normal widget render this frame
                            }
                            crate::star_crawl::StepResult::Finished => {
                                crawl_active = false;
                                crawl_dma2d = None;
                            }
                            _ => {
                                // Pending or Idle — keep ticking
                                continue; // don't render widgets while crawl runs
                            }
                        }
                    }
                }

                // Restore pristine desktop into the render buffer
                core::ptr::copy_nonoverlapping(pristine_base, render_buf, fb_bytes);

                // Render widget tree on top (landscape, no rotation)
                let fb_slice = core::slice::from_raw_parts_mut(render_buf, fb_bytes);
                let surface = Surface::new(
                    fb_slice,
                    (fb_w * bpp) as usize,
                    PixelFmt::Argb8888,
                    fb_w,
                    fb_h,
                );
                let mut blitter = CpuBlitter;
                let mut blit_renderer: BlitterRenderer<'_, CpuBlitter, 32> =
                    BlitterRenderer::new(&mut blitter, surface);
                // Bare-metal wraps the BlitterRenderer in RotatedRenderer so
                // the controller (which lays out widgets in landscape coord
                // space) renders correctly into the portrait FB. The Zephyr
                // ACM path was drawing directly into the portrait FB, which
                // produced "top-left across" icon placement instead of
                // "top-right down". Match bare-metal here.
                #[cfg(feature = "adapted_cmd")]
                {
                    use rlvgl_platform::blit::RotatedRenderer;
                    let mut renderer = RotatedRenderer::new(&mut blit_renderer, fb_w);
                    root.borrow().draw(&mut renderer);
                }
                #[cfg(not(feature = "adapted_cmd"))]
                {
                    root.borrow().draw(&mut blit_renderer);
                }

                dcache_clean_all();

                // Frame pacing — fixed sleep matches the bare-metal
                // SysTick approach, which doesn't flicker. Presenting at
                // a stable cadence is more important than syncing to
                // every TE: shadow reload of L1CFBAR happens between
                // scans because the scan time (~22 ms total) is shorter
                // than this 33 ms loop period.
                //
                // ERIF-based pacing was attempted but interacted poorly
                // with our render time (~20 ms) and the panel's 60 Hz TE
                // — see commit history for details.
                mark1(b'D'); // before widget present
                do_present(render_buf, di.width, di.height);
                mark1(b'E'); // after widget present

                // After present, the buffer we just rendered becomes
                // the displayed front. The other buffer becomes our
                // new render target.
                render_buf = if render_buf == fb_front {
                    fb_back
                } else {
                    fb_front
                };

                mark1(b'F'); // before sleep
                rlvgl_k_sleep_ms(33); // ~30 fps target
                mark1(b'G'); // after sleep
            }
        }
    }
}

unsafe extern "C" {
    fn rlvgl_k_sleep_ms(ms: u32);
    /// Non-blocking USART1 RX poll. Returns byte value (0..=255) or -1
    /// if no byte is available. See main.c for impl.
    fn rlvgl_serial_poll_rx() -> i32;
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
