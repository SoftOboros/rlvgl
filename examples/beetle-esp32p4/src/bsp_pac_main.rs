//! Raw PAC BSP path for DFR1172 FireBeetle 2 ESP32-P4 + DFR0550-V2 panel.
//!
//! Goal of this binary: drive the 5" 800×480 IPS DSI panel attached via
//! the DFR1237 IO-expansion shield's Pi-style DSI FFC, using the rlvgl
//! widget tree (shared with the disco-demo) as the application payload.
//!
//! The IDF C reference at
//! `/tmp/dfr_bringup/dfr0550_first_light/main/dfr0550_first_light.c`
//! is the verified-working source. This binary mirrors its `app_main`
//! step-by-step against raw PAC.
//!
//! Regenerate the BSP with:
//! ```sh
//! cargo run --features creator --bin rlvgl-creator -- --silent bsp from-yaml \
//!     --vendor esp --board beetle_esp32p4 --chip esp32p4 \
//!     --out /tmp/rlvgl-bsp-p4 --emit-pac
//! cp /tmp/rlvgl-bsp-p4/dfr1172_fire_beetle_2_p4/{board,clocks,io_mux,pac,peripherals}.rs \
//!    examples/beetle-esp32p4/src/bsp_generated/
//! ```

#![no_std]
#![no_main]

use esp_riscv_rt::entry;
use esp32p4::Peripherals;
use panic_halt as _;

mod bsp_generated;
mod dfr0550;

#[entry]
fn main() -> ! {
    let _dp = Peripherals::take().unwrap();

    // BSP-managed clocks, IO MUX, peripheral init.
    bsp_generated::init();

    // The BSP generator currently emits I2C pins as plain GPIOs; route
    // them through the GPIO matrix to the I2C0 peripheral here until the
    // generator learns to do this itself.
    unsafe {
        dfr0550::i2c0::route_pins();
    }

    // DFR0550 bring-up phases — see dfr0550/mod.rs for the full sequence.
    // PSRAM, DSI host PHY, and DPI panel still return `Err(Unimplemented)`;
    // we ignore those results so the binary still links and falls through
    // to the LED-blink sanity check while the PHY layer is staged.
    unsafe {
        // Phase 1: PSRAM octal HEX @ 200 MHz.
        let _ = dfr0550::psram::init();

        // Phase 2: DSI DPHY rail (LDO_VO3 @ 2500 mV).
        let _dphy_ldo = dfr0550::ldo::LdoChannel::acquire_dphy();

        // Phase 3a: ungate DSI bus + bridge clocks, pulse bridge reset.
        dfr0550::dsi_host::clocks::enable_bus_and_reset();
        // Phase 3b: ungate PHY config + PLL ref clocks (default = PLL_F20M).
        dfr0550::dsi_host::clocks::enable_phy_clocks(
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m,
        );
        // Phase 3c: configure DPI pixel clock — 26 MHz from PLL_F240M
        // (actual 240/9 ≈ 26.67 MHz; matches IDF behavior).
        dfr0550::dsi_host::clocks::enable_dpi_clock(
            dfr0550::dsi_host::clocks::DpiClockSource::PllF240m,
            dfr0550::DPI_PIXEL_CLK_MHZ,
        );

        // Phase 4: wake the panel bridge (Pi-7"-Atmel protocol).
        let _ = dfr0550::i2c_bridge::wake();

        // Phase 5: DSI host @ 1 lane × 750 Mbps (PHY M/N PLL config).
        if let Ok(dsi) = dfr0550::dsi_host::init(
            dfr0550::DSI_LANES,
            dfr0550::DSI_LANE_MBPS,
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m.freq_mhz(),
        ) {
            // Phase 6: DPI controller — first-light path drives the
            // host's built-in vertical-bar pattern generator. No DMA,
            // no FB. If we get past this, the panel is showing color
            // bars and the entire LDO/clock/PHY/bridge stack is alive.
            let _ = dfr0550::dpi_panel::DpiPanel::init_pattern(
                &dsi,
                dfr0550::DPI_PIXEL_CLK_MHZ,
                dfr0550::dpi_panel::PatternType::BarVertical,
            );
        }
    }

    // Sanity: blink the LED so we can tell when the bring-up stubs are
    // still no-ops (binary returned but DSI never came up).
    led_blink_loop()
}

/// Verified-working color cycle from the IDF reference. Each iteration
/// writes a solid color into the framebuffer and triggers a cache
/// writeback so the DSI DMA picks up fresh data.
///
/// Currently unreachable — `DpiPanel::init_with_fb` (Phase 5b.6 DW-GDMA
/// bring-up) returns `Err(Unimplemented)`. Kept to lock in the future
/// shape so the FB API stays stable across that landing.
#[allow(dead_code)]
unsafe fn run_color_cycle(fb: dfr0550::dpi_panel::FrameBuffer<'static>) -> ! {
    const CYCLE: &[(u8, u8, u8)] = &[
        (255, 0, 0),     // RED
        (0, 255, 0),     // GREEN
        (0, 0, 255),     // BLUE
        (255, 255, 255), // WHITE
        (0, 0, 0),       // BLACK
    ];
    let mut idx = 0usize;
    let mut frame = 0u32;
    loop {
        let (r, g, b) = CYCLE[idx];
        let n_pixels = dfr0550::H_RES as usize * dfr0550::V_RES as usize;
        // SAFETY: fb.ptr/len come from DpiPanel::init, which guarantees a
        // valid PSRAM region of at least fb.len bytes for the lifetime of
        // the panel. Single-writer (this loop), no concurrent CPU writers.
        let px = unsafe { core::slice::from_raw_parts_mut(fb.ptr, n_pixels * 3) };
        for p in 0..n_pixels {
            px[p * 3] = r;
            px[p * 3 + 1] = g;
            px[p * 3 + 2] = b;
        }
        // SAFETY: same region as above; cache writeback prepares it for
        // the DSI DMA scanout engine.
        unsafe { dfr0550::cache::writeback(fb.ptr, fb.len) };
        frame = frame.wrapping_add(1);
        if frame.is_multiple_of(30) {
            idx = (idx + 1) % CYCLE.len();
        }
    }
}

fn led_blink_loop() -> ! {
    let led_mask = 1u32 << bsp_generated::board::LED;
    loop {
        unsafe {
            let gpio = &*esp32p4::GPIO::PTR;
            gpio.out_w1ts().write(|w| w.bits(led_mask));
        }
        for _ in 0..500_000 {
            unsafe { core::arch::asm!("nop") };
        }
        unsafe {
            let gpio = &*esp32p4::GPIO::PTR;
            gpio.out_w1tc().write(|w| w.bits(led_mask));
        }
        for _ in 0..500_000 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}
