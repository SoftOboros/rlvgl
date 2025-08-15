#![no_std]
#![no_main]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

extern crate alloc;

use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
use panic_halt as _;
use rlvgl::platform::stm32h747i_disco::{Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};

#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::build_demo;

/// Global allocator backed by a fixed-size heap in RAM.
#[global_allocator]
static ALLOC: Heap = Heap::empty();

/// Heap size in bytes.
const HEAP_SIZE: usize = 64 * 1024;

/// Static memory region used to service heap allocations.
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

/// Application entry point.
#[entry]
fn main() -> ! {
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    let _display = Stm32h747iDiscoDisplay;
    let _touch = Stm32h747iDiscoInput;
    let _demo = build_demo();

    loop {
        cortex_m::asm::nop();
    }
}
