#![no_std]
#![no_main]

//! Entry point for the STM32H747I-DISCO hardware demo.
//!
//! Initializes placeholder display and touch drivers for the board and
//! constructs the shared widget demonstration. Real MIPI-DSI and touch
//! handling will be added in future iterations.

use cortex_m_rt::entry;
use panic_halt as _;
use rlvgl::platform::stm32h747i_disco::{Stm32h747iDiscoDisplay, Stm32h747iDiscoInput};

#[path = "../../common_demo/lib.rs"]
mod common_demo;
use common_demo::build_demo;

/// Application entry point.
#[entry]
fn main() -> ! {
    let _display = Stm32h747iDiscoDisplay;
    let _touch = Stm32h747iDiscoInput;
    let _demo = build_demo();

    loop {
        cortex_m::asm::nop();
    }
}
