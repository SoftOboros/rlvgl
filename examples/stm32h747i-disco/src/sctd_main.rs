// SPDX-License-Identifier: MIT
//! Bare-metal STM32H747I-DISCO entrypoint for the SCXML Tutorial Demo app.
//!
//! This is the SCTD-00 §7.2 / §12 build gate for the STM32H747I-DISCO
//! bare-metal target. It proves that `rlvgl-app-sctd-demo` compiles for
//! `thumbv7em-none-eabihf` with the cm7 feature set.
//!
//! Hardware bring-up (display init, SDRAM, DMA2D, touch, LTDC/DSI) is
//! out of scope per SCTD-00 §7.2 ("without adding new hardware bring-up")
//! and §11 ("No board bring-up work for targets that cannot already mount
//! rlvgl app payloads"). The gate is `cargo check`, NOT `cargo build`
//! to a flashable ELF — hardware flashing is not required to ratify the
//! concept per SCTD-00 §7.3.
//!
//! The FreeRTOS variant (SCTD-00 §7.2 target 4) is deferred with an
//! explicit exception: the FreeRTOS entry (`freertos_entry.rs`) hardcodes
//! `DiscoController` as its app payload and the task-model wiring is not
//! a payload swap. Mounting SCTD under FreeRTOS requires a parallel
//! FreeRTOS entry analogous to this file, which is deferred to a follow-up
//! SCTD phase once the SCTD-01 acceptance gates pass on the bare-metal
//! and simulator paths first.
//!
//! # Gate command
//!
//! ```sh
//! RUSTFLAGS="-C target-cpu=cortex-m7" \
//! cargo check \
//!   --target thumbv7em-none-eabihf \
//!   -p rlvgl-example-disco \
//!   --bin rlvgl-stm32h747i-sctd \
//!   --features cm7,sctd,dma2d
//! ```

#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

extern crate alloc;

use core::ptr::addr_of_mut;

use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
use panic_halt as _;

use rlvgl_app_sctd_demo::SctdController;
use rlvgl_platform::Screen;

/// Global heap allocator (same size as the disco bare-metal build).
#[global_allocator]
static ALLOC: Heap = Heap::empty();

const HEAP_SIZE: usize = 64 * 1024;

/// Static heap memory region.
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE]; // rlvgl-discipline: allow(static_mut)

/// Entry point.
///
/// Initialises the heap and constructs the `SctdController` to prove the
/// SCTD app payload compiles for thumbv7em-none-eabihf. Display bring-up is
/// intentionally omitted — this file is a build gate, not a flashable demo.
#[entry]
fn main() -> ! {
    // SAFETY: HEAP_MEM is static, this is the sole writer, and we call
    // init() exactly once before any allocation.
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    // Construct the SCTD controller to prove the app payload links for the
    // embedded target. The nominal disco display is 800x480.
    let _controller = SctdController::new(Screen::landscape(800, 480));

    // Bare-metal binaries never return.
    #[allow(clippy::empty_loop)]
    loop {}
}
