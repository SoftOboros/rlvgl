//! Cache management for PSRAM-backed framebuffers.
//!
//! Equivalent of:
//! ```c
//! esp_cache_msync(fb, fb_bytes, ESP_CACHE_MSYNC_FLAG_DIR_C2M);
//! ```
//!
//! Direction `C2M` (CPU-to-Memory) writes back dirty cache lines so the
//! DSI DMA reads fresh data. Without this call, colors briefly appear
//! then fade as the DMA picks up stale PSRAM.
//!
//! Derived from `components/hal/esp32p4/include/hal/cache_ll.h`. The IDF
//! L1 helper calls into the BootROM `Cache_WriteBack_Addr`, but the
//! underlying register interface is the CACHE peripheral's
//! `SYNC_{MAP,ADDR,SIZE,CTRL}` block — the ROM helper just sets those
//! same registers and spins on `SYNC_DONE`. We drive them directly here
//! to keep the bring-up free of ROM-symbol address bindings.
//!
//! `SYNC_MAP` bit assignment per the PAC docs:
//!   bit 0 L1-ICache0   bit 3 L1-ICache3
//!   bit 1 L1-ICache1   bit 4 L1-DCache    ← writeback target
//!   bit 2 L1-ICache2   bit 5 L2-Cache     ← writeback target
//!
//! `SYNC_CTRL.WRITEBACK_ENA` self-clears when the operation completes;
//! `SYNC_CTRL.SYNC_DONE` reads as 1 once finished. The four sync-enable
//! bits (invalidate / clean / writeback / writeback-invalidate) are
//! mutually exclusive — only one may be set per submission.
//!
//! Cache-line alignment: ESP32-P4 L1 D-cache is 64 B, L2 is 64 B. The
//! IDF helper rounds the start down and the end up to 64 B; matching
//! that here.

#![allow(dead_code)]

const CACHE_LINE_BYTES: usize = 64;
const SYNC_MAP_L1_DCACHE: u8 = 1 << 4;
const SYNC_MAP_L2_CACHE: u8 = 1 << 5;

/// Writeback the cache lines covering `[ptr, ptr + len)` so a DMA peer
/// (here: the DSI scanout engine) reads the current CPU-written contents.
///
/// # Safety
/// `ptr` must point to a CPU-writable cached region of at least `len`
/// bytes. Alignment is handled internally — the operation is rounded out
/// to 64-byte cache-line boundaries.
pub unsafe fn writeback(ptr: *const u8, len: usize) {
    use esp32p4 as pac;

    if len == 0 {
        return;
    }

    let start = ptr as usize;
    let end = start.saturating_add(len);
    let aligned_start = start & !(CACHE_LINE_BYTES - 1);
    let aligned_end = (end + CACHE_LINE_BYTES - 1) & !(CACHE_LINE_BYTES - 1);
    let aligned_len = aligned_end - aligned_start;

    let p = unsafe { pac::Peripherals::steal() };
    let cache = &p.CACHE;

    // Wait for any prior sync to complete before issuing a new one.
    while cache.sync_ctrl().read().sync_done().bit_is_clear() {
        core::hint::spin_loop();
    }

    unsafe {
        cache
            .sync_addr()
            .write(|w| w.sync_addr().bits(aligned_start as u32));
        cache
            .sync_size()
            .write(|w| w.sync_size().bits(aligned_len as u32));
        cache
            .sync_map()
            .write(|w| w.sync_map().bits(SYNC_MAP_L1_DCACHE | SYNC_MAP_L2_CACHE));
    }
    cache
        .sync_ctrl()
        .modify(|_, w| w.writeback_ena().set_bit());

    // Spin until WRITEBACK_ENA self-clears (sync_done==1).
    while cache.sync_ctrl().read().sync_done().bit_is_clear() {
        core::hint::spin_loop();
    }
}
