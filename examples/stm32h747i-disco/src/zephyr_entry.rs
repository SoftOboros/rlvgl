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
        // Copy front buffer (rotated splash) to a third SDRAM region
        // past the scratch area. Each frame we restore from this pristine
        // copy before drawing widgets.
        let pristine_base = scratch_base.add(splash_bytes);
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
                static CRAWL_FONT_DATA: &[u8] =
                    include_bytes!("../assets/fonts/DejaVuSans-24.bin");
                static CRAWL_FONT: rlvgl_core::packed_font::PackedFont =
                    rlvgl_core::packed_font::PackedFont {
                        height: 24,
                        ascent: 22,
                        glyphs: &crate::fonts::DEJAVU_SANS_24_GLYPHS,
                        data: CRAWL_FONT_DATA,
                    };
                static CRAWL_LINES: &[&str] = &[
                    "rlvgl",
                    "",
                    "A Rust UI framework for",
                    "embedded displays",
                    "",
                    "Running on Zephyr RTOS",
                    "STM32H747I-DISCO",
                    "",
                    "DMA2D accelerated",
                    "star field rendering",
                ];
                StarCrawl::new(&CRAWL_FONT, CRAWL_LINES, 60)
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
            const CRAWL_FB_H: u32 = 720;

            // After first present, LTDC displays back buffer (now front).
            // The "back" for rendering is the original front buffer.
            // Track which buffer to render into.
            let mut render_buf = fb_front; // original front is now the back

            loop {
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
                                controller.dispatch_event(&Event::KeyDown { key: k });
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

                    if pressed {
                        use rlvgl_core::event::Event;
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

                controller.tick();

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
                                // Adapted command mode: DMA2D M2M works — start the crawl.
                                if crawl_dma2d.is_none() {
                                    crawl_dma2d = Some(rlvgl_platform::dma2d::Dma2dBlitter::steal());
                                }
                                crawl_active = true;
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
                #[cfg(all(feature = "dma2d", any(target_arch = "arm", target_arch = "aarch64")))]
                if crawl_active {
                    if let Some(ref mut dma2d) = crawl_dma2d {
                        // Portrait scratch buffer at CRAWL_BASE (0xD100_0000)
                        let crawl_buf = 0xD100_0000usize as *mut u8;
                        let sync_ref = get_sync().unwrap_unchecked();
                        let result = star_crawl.tick(
                            dma2d, crawl_buf, CRAWL_FB_W, CRAWL_FB_H, sync_ref,
                        );
                        match result {
                            crate::star_crawl::StepResult::FrameReady => {
                                // Rotate portrait crawl output 90° CW into
                                // the landscape render buffer.
                                let src = crawl_buf as *const u32;
                                let dst = render_buf as *mut u32;
                                let dst_stride = fb_w as usize;
                                for py in 0..CRAWL_FB_H as usize {
                                    for px in 0..CRAWL_FB_W as usize {
                                        let pixel = src.add(py * CRAWL_FB_W as usize + px).read();
                                        let dx = py;
                                        let dy = (CRAWL_FB_W as usize - 1) - px;
                                        if dx < fb_w as usize && dy < fb_h as usize {
                                            dst.add(dy * dst_stride + dx).write(pixel);
                                        }
                                    }
                                }
                                dcache_clean_all();
                                do_present(render_buf, di.width, di.height);
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
                do_present(render_buf, di.width, di.height);

                // After present, the buffer we just rendered becomes
                // the displayed front. The other buffer becomes our
                // new render target.
                render_buf = if render_buf == fb_front {
                    fb_back
                } else {
                    fb_front
                };

                // Frame pacing — wait for the DSI ERIF (end-of-refresh)
                // semaphore the ISR gives, then run the next render
                // iteration. This synchronises the render loop with the
                // panel's TE and avoids the 30 fps / 60 Hz beat that
                // showed up as icon flicker when we slept a fixed 33 ms.
                //
                // Falls back to a fixed sleep if the sem doesn't fire
                // within ~25 ms (e.g. ERIF stalled): keeps the loop
                // making progress even if the panel TE goes silent.
                #[cfg(feature = "adapted_cmd")]
                {
                    let mut waited_ms = 0u32;
                    loop {
                        if let Some(sync) = get_sync() {
                            if zephyr_sync::k_sem_take(
                                sync.erif_sem,
                                zephyr_sync::K_NO_WAIT,
                            ) == 0
                            {
                                break;
                            }
                        }
                        if waited_ms >= 25 {
                            break;
                        }
                        rlvgl_k_sleep_ms(1);
                        waited_ms += 1;
                    }
                }
                #[cfg(not(feature = "adapted_cmd"))]
                {
                    rlvgl_k_sleep_ms(33); // ~30 fps fallback for video mode
                }
            }
        }
    }
}

unsafe extern "C" {
    fn rlvgl_k_sleep_ms(ms: u32);
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
