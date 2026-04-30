//! Cache management for PSRAM-backed framebuffers.
//!
//! ESP-IDF reference (verified-working call):
//!
//! ```c
//! esp_cache_msync(fb, fb_bytes, ESP_CACHE_MSYNC_FLAG_DIR_C2M);
//! ```
//!
//! Direction `C2M` (CPU-to-Memory) writes back dirty cache lines so the
//! DSI DMA reads fresh data. Without this call, colors briefly appear
//! then fade as the DMA picks up stale PSRAM. See "Failed configurations"
//! in the memory file.
//!
//! TODO(TRM §Cache): on ESP32-P4, the L1 D-cache is configured per
//!   address window. Use `cache_writeback_addr` / `cache_invalidate_addr`
//!   semantics — likely via a small inline asm wrapper around the
//!   relevant `Cache_Writeback_Addr` ROM call, or direct register writes
//!   to the cache controller block.

#![allow(dead_code)]

/// Writeback the cache lines covering `[ptr, ptr + len)` so a DMA peer
/// (here: the DSI scanout engine) reads the current CPU-written contents.
///
/// # Safety
/// `ptr` must point to a CPU-writable cached region of at least `len`
/// bytes; alignment to the cache line is required by the underlying
/// hardware (ESP32-P4: 64 B).
pub unsafe fn writeback(_ptr: *const u8, _len: usize) {
    // TODO(phase 5b): cache controller writeback.
}
