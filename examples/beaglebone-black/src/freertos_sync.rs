//! FreeRTOS semaphore and task FFI wrappers.
//!
//! Thin wrappers around the C shims in `freertos/src/ffi_shims.c`.
//! FreeRTOS semaphore/task APIs are macros in C and cannot be called
//! directly from Rust FFI; the C shims wrap them as real functions.

#![allow(dead_code)]
#![allow(non_camel_case_types)]

/// Opaque FreeRTOS semaphore handle.
pub type SemaphoreHandle_t = *mut core::ffi::c_void;

/// Opaque FreeRTOS task handle.
pub type TaskHandle_t = *mut core::ffi::c_void;

/// FreeRTOS stack word type (32-bit on ARM).
pub type StackType_t = u32;

/// FreeRTOS task function signature.
pub type TaskFunction_t = unsafe extern "C" fn(*mut core::ffi::c_void);

/// FreeRTOS boolean true.
pub const pdTRUE: i32 = 1;
/// FreeRTOS boolean false.
pub const pdFALSE: i32 = 0;
/// Maximum blocking time (wait forever).
pub const portMAX_DELAY: u32 = 0xFFFF_FFFF;

/// Static storage for a FreeRTOS semaphore (opaque, must be aligned).
#[repr(C, align(8))]
pub struct StaticSemaphore {
    _storage: [u8; 96],
}

impl StaticSemaphore {
    pub const fn new() -> Self {
        Self { _storage: [0; 96] }
    }
}

/// Static storage for a FreeRTOS TCB.
#[repr(C, align(8))]
pub struct StaticTask {
    _storage: [u8; 128],
}

impl StaticTask {
    pub const fn new() -> Self {
        Self { _storage: [0; 128] }
    }
}

// ---------------------------------------------------------------------------
// FFI declarations — implemented in freertos/src/ffi_shims.c
// ---------------------------------------------------------------------------

unsafe extern "C" {
    /// Create a binary semaphore using caller-provided static storage.
    pub fn rlvgl_sem_create_binary_static(buf: *mut StaticSemaphore) -> SemaphoreHandle_t;

    /// Take (wait on) a semaphore from task context.
    pub fn rlvgl_sem_take(sem: SemaphoreHandle_t, ticks: u32) -> i32;

    /// Give (signal) a semaphore from task context.
    pub fn rlvgl_sem_give(sem: SemaphoreHandle_t) -> i32;

    /// Give a semaphore from ISR context (handles portYIELD_FROM_ISR).
    pub fn rlvgl_sem_give_from_isr(sem: SemaphoreHandle_t);

    /// Create a task using static allocation.
    pub fn xTaskCreateStatic(
        pxTaskCode: TaskFunction_t,
        pcName: *const u8,
        ulStackDepth: u32,
        pvParameters: *mut core::ffi::c_void,
        uxPriority: u32,
        puxStackBuffer: *mut StackType_t,
        pxTaskBuffer: *mut StaticTask,
    ) -> TaskHandle_t;

    /// Start the FreeRTOS scheduler. Never returns.
    pub fn vTaskStartScheduler() -> !;

    /// Block the calling task for the specified number of ticks.
    pub fn vTaskDelay(xTicksToDelay: u32);
}
