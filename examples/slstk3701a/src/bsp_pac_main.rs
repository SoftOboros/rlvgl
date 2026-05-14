//! Raw-PAC v0 bring-up for the Silicon Labs SLSTK3701A Giant Gecko Starter Kit.
//!
//! This binary proves the full chipdb → generator → BSP → cortex-m-rt
//! pipeline ratified by CHIPS-SILABS-06:
//!
//! 1. `chipdb/rlvgl-chips-silabs/db/chips/EFM32GG11.yaml` +
//!    `chipdb/rlvgl-chips-silabs/db/boards/slstk3701a.yaml` describe
//!    the board.
//! 2. `rlvgl-creator bsp from-yaml --vendor silabs --board slstk3701a`
//!    mechanically emits the 8-file BSP set under
//!    `src/bsp_generated/slstk3701_a/`.
//! 3. `bsp_generated::slstk3701_a::pac::init()` brings up CMU clock
//!    gates, GPIO MODE bits + ROUTELOC/ROUTEPEN routing, and the
//!    per-peripheral init stubs from the SILABS templates.
//! 4. CHIPS-SILABS-06a — the reset handler now toggles PH10 (LED0_R)
//!    in a busy-wait loop using `cortex_m::asm::delay`. This exercises
//!    the full BSP integration end-to-end (CMU `HFBUSCLKEN0.GPIO`
//!    from `clocks::init`, the slate-8 absolute-pin-index `mode10`
//!    field on `ph_modeh` from `io_mux::init`, and a live GPIO
//!    `ph_doutset`/`ph_doutclr` write from this entry point).
//! 5. CHIPS-SILABS-06b — before entering the LED toggle loop the reset
//!    handler now also sends `"hello\r\n"` over USART4 VCOM (PH4 TX,
//!    routed through the FTDI bridge that is gated by PE1 =
//!    VCOM_ENABLE, which `io_mux::init` already drives HIGH per the
//!    board YAML `initial: high` annotation). This validates the
//!    slate-6 `-02b` field-style amendment end-to-end: `clkdiv`,
//!    `frame`, `cmd`, `routeloc0`, `routepen`, `status`, and `txdata`
//!    on the pinned `efm32gg11b-pac 0.1.4` are all direct `#[repr(C)]`
//!    struct fields, not method accessors, and the generated
//!    `peripherals::init_usart4` plus our send loop here both consume
//!    them in that form. rlvgl widget tree is deferred to
//!    CHIPS-SILABS-06c.
//!
//! The example crate also exercises the linker scripts emitted by
//! CHIPS-SILABS-05a (`memory.x` + `efm32_gg11.x`), wired in via
//! `build.rs`. This is the complementary gate to
//! `tests/bsp_silabs_slstk3701a_compile.rs`: `cargo check` here pulls
//! in `cortex-m-rt`'s bundled `link.x` and actually invokes the
//! linker against the chipdb-derived `memory.x`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

#[cfg(feature = "bsp_pac")]
mod bsp_generated;

#[cfg(feature = "bsp_pac")]
use efm32gg11b_pac::efm32gg11b820 as pac;

/// Block until USART4's TX buffer level reports "empty" (ready for a
/// new byte), then push `b` into TXDATA.
///
/// `STATUS.TXBL` is a single-bit reader on the pinned
/// `efm32gg11b-pac 0.1.4` (bit 6 of STATUS): `1` = TX buffer has room.
/// `TXDATA.TXDATA` is an 8-bit FieldWriter at offset 0 of a u32
/// register, so `w.txdata().bits(b)` is the canonical write — the
/// reserved upper bits stay zero.
#[cfg(feature = "bsp_pac")]
fn usart4_tx_byte(p: &pac::Peripherals, b: u8) {
    while p.USART4.status.read().txbl().bit_is_clear() {}
    p.USART4
        .txdata
        .write(|w| unsafe { w.txdata().bits(b) });
}

#[entry]
fn main() -> ! {
    #[cfg(feature = "bsp_pac")]
    {
        // Bring up CMU clock gates, GPIO MODE bits + ROUTELOC routing,
        // and per-peripheral init. After this:
        //   • PH10 is push-pull output with DOUT high (LED0_R off,
        //     active-low).
        //   • PE1 (VCOM_ENABLE) is push-pull output with DOUT high so
        //     the FTDI level translator passes USART4 TX through to
        //     the host VCOM port — see `io_mux.rs` line for PE1.
        //   • USART4 is configured for 115200-8N1 async UART with
        //     ROUTELOC0 = 4 (PH4 TX / PH5 RX) and TX+RX enabled
        //     via `CMD.TXEN | CMD.RXEN` — see `peripherals.rs`
        //     `init_usart4`.
        bsp_generated::slstk3701_a::pac::init();

        // CHIPS-SILABS-06b — send "hello\r\n" over USART4 VCOM.
        //
        // The slate-6 SKU-flatten amendment puts `Peripherals` under
        // the per-SKU sub-module, so the path is
        // `efm32gg11b_pac::efm32gg11b820::Peripherals::steal()`.
        // The pinned `efm32gg11b-pac 0.1.4` is the pre-method-accessor
        // svd2rust era — `p.USART4.status` / `p.USART4.txdata` are
        // direct `#[repr(C)]` struct members, not `p.USART4.status()`.
        let p = unsafe { pac::Peripherals::steal() };

        for &b in b"hello\r\n" {
            usart4_tx_byte(&p, b);
        }

        // CHIPS-SILABS-06a — busy-wait toggle on PH10 (LED0_R).
        //
        // This PAC vintage exposes `Px_DOUT` (level) and `Px_DOUTTGL`
        // (atomic XOR) but does not expose the separate `DOUTSET` /
        // `DOUTCLR` registers per port. `ph_douttgl` is the right
        // primitive for a busy-wait blink: one write atomically XORs
        // the bit, so there is no read-modify-write race against
        // adjacent LED pins on the same port.
        loop {
            // Atomic XOR of PH10 — toggles LED0_R on/off each pass.
            // Active-LOW per the SLSTK3701A schematic; io_mux::init
            // initialised DOUT high (LED off) so the first pass turns
            // the LED on.
            p.GPIO
                .ph_douttgl
                .write(|w| unsafe { w.bits(1 << 10) });
            cortex_m::asm::delay(1_000_000);
        }
    }
    #[cfg(not(feature = "bsp_pac"))]
    loop {
        cortex_m::asm::wfi();
    }
}
