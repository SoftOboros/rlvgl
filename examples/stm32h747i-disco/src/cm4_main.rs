#![cfg_attr(not(doc), no_std)]
#![cfg_attr(not(doc), no_main)]

//! CM4 entry for STM32H747I-DISCO.
//!
//! Two paths:
//! - `c_hal_cm4` feature: C BSP handles MPU, D2 clocks, peripheral enables,
//!   then calls rlvgl_cm4_main() in Rust.
//! - Without `c_hal_cm4`: uses PAC-generated init_power() and waits for
//!   CM7 clock signal via mailbox.

extern crate alloc;

use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use embedded_alloc::Heap;
#[cfg(target_os = "none")]
#[cfg(not(doc))]
use panic_halt as _;

// Auto-generated board support — pin constants and PAC helpers are a reference
// library; not all are consumed in every build configuration.
#[allow(dead_code, unused_imports, unused_macros, unused_unsafe, unknown_lints)]
#[path = "bsp/cm4/pac.rs"]
mod bsp_pac;
mod ipc;

/// Global allocator backed by a fixed-size heap in RAM.
#[global_allocator]
static ALLOC: Heap = Heap::empty();

/// Heap backing store in D2 SRAM.
const HEAP_SIZE: usize = 16 * 1024;
#[unsafe(link_section = ".uninit.HEAP")]
static mut HEAP_MEM: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];

#[cfg(not(doc))]
#[entry]
fn main() -> ! {
    // Heap must be ready before any Rust allocation.
    unsafe {
        let start = addr_of_mut!(HEAP_MEM) as usize;
        ALLOC.init(start, HEAP_SIZE);
    }

    // ── C HAL path ──────────────────────────────────────────────────────────
    #[cfg(all(
        feature = "c_hal_cm4",
        feature = "stm32h747i_disco_cm4",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        // Force-link the BSP crate so its native C library is included.
        extern crate rlvgl_bsps_stm;
        unsafe extern "C" {
            fn c_bsp_init_cm4() -> !;
        }
        unsafe { c_bsp_init_cm4() }
    }

    // ── PAC-only path (no c_hal_cm4 feature) ────────────────────────────────
    #[cfg(all(
        not(feature = "c_hal_cm4"),
        feature = "stm32h747i_disco_cm4",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    {
        let dp = stm32h7::stm32h747cm4::Peripherals::take().unwrap();
        bsp_pac::init_power(&dp);
        #[allow(clippy::let_unit_value)]
        {
            let _ = bsp_pac::wait_for_clocks();
        }

        ipc::init_cm4();
        cm4_app_loop();
    }

    // Fallback: non-ARM / non-disco / doc builds
    #[cfg(not(all(
        feature = "stm32h747i_disco_cm4",
        any(target_arch = "arm", target_arch = "aarch64")
    )))]
    loop {
        cortex_m::asm::nop();
    }
}

// ── c_hal_cm4 application entry ─────────────────────────────────────────────
//
// Called by c_bsp_init_cm4() after CM4 MPU and D2 peripheral clocks are ready.
#[cfg(all(
    feature = "c_hal_cm4",
    feature = "stm32h747i_disco_cm4",
    any(target_arch = "arm", target_arch = "aarch64")
))]
#[unsafe(no_mangle)]
pub extern "C" fn rlvgl_cm4_main() -> ! {
    ipc::init_cm4();
    cm4_app_loop();
}

// ── Application controller loop ─────────────────────────────────────────────
//
// Shared by both c_hal_cm4 and PAC-only paths.  CM4 acts as the application
// controller: it receives UI events from CM7 (touch, button presses, frame
// sync) and sends high-level commands back (update labels, navigate, etc.).

/// Write a u32 as decimal ASCII into `buf`, returning the number of bytes written.
fn write_u32_ascii(buf: &mut [u8], mut val: u32) -> usize {
    if val == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
        }
        return 1;
    }
    // Write digits in reverse, then reverse them
    let mut tmp = [0u8; 10];
    let mut len = 0;
    while val > 0 && len < tmp.len() {
        tmp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    let out_len = len.min(buf.len());
    for i in 0..out_len {
        buf[i] = tmp[len - 1 - i];
    }
    out_len
}

fn cm4_app_loop() -> ! {
    // Wait for CM7's first FrameRendered event before sending any commands.
    // This ensures the display server widget tree is built and ready.
    loop {
        if let Some(evt) = ipc::event_pop() {
            if evt.kind == ipc::EventKind::FrameRendered as u32 {
                break;
            }
        }
        cortex_m::asm::nop();
    }

    // Send initial status message
    let _ = ipc::cmd_push(ipc::cmd_update_label(
        ipc::widget_id::STATUS_LABEL,
        b"CM4: online",
    ));

    let mut click_count: u32 = 0;
    let mut tick_count: u32 = 0;

    loop {
        // 1. Process incoming events from CM7 (display server)
        while let Some(evt) = ipc::event_pop() {
            match ipc::EventKind::from_u32(evt.kind) {
                ipc::EventKind::ButtonPressed => {
                    if evt.a == ipc::widget_id::CLICK_COUNTER {
                        click_count += 1;
                        // Format "Clicks: N" and send back to CM7
                        let mut buf = [0u8; 12];
                        let prefix = b"Clicks: ";
                        let plen = prefix.len().min(12);
                        buf[..plen].copy_from_slice(&prefix[..plen]);
                        let nlen = write_u32_ascii(&mut buf[plen..], click_count);
                        let total = plen + nlen;
                        let _ = ipc::cmd_push(ipc::cmd_update_label(
                            ipc::widget_id::CLICK_COUNTER,
                            &buf[..total],
                        ));
                    }
                }
                ipc::EventKind::FrameRendered => {
                    tick_count += 1;
                    // Periodically update the status label to show liveness
                    if tick_count % 30 == 0 {
                        let mut buf = [0u8; 12];
                        let prefix = b"CM4: ";
                        let plen = prefix.len().min(12);
                        buf[..plen].copy_from_slice(&prefix[..plen]);
                        let nlen = write_u32_ascii(&mut buf[plen..], tick_count / 30);
                        let total = plen + nlen;
                        let _ = ipc::cmd_push(ipc::cmd_update_label(
                            ipc::widget_id::STATUS_LABEL,
                            &buf[..total],
                        ));
                    }
                }
                ipc::EventKind::PointerDown
                | ipc::EventKind::PointerMove
                | ipc::EventKind::PointerUp => {
                    // Touch events available for application use —
                    // currently passed through for future gesture recognition
                }
                ipc::EventKind::None => {}
            }
        }

        // 2. Application logic placeholder: future sensor polling,
        //    state machines, communication protocols, etc. go here.

        // 3. Yield — crude pacing; will be replaced with WFE + HSEM
        //    interrupt in a future iteration.
        cortex_m::asm::delay(1_000_000);
    }
}
