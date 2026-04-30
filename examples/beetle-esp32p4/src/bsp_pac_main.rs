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

mod app_desc;
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

    // Set up the user LED so we can encode bring-up status in blink count.
    unsafe { led_init() };

    // Diagnostic blink count = which step failed (1=I2C wake, 2=PHY lock,
    // 3=PHY lane cal, 4=DPI panel, 0=all OK = solid ON).
    let status: u8 = unsafe { run_bringup() };
    led_status_loop(status)
}

unsafe fn run_bringup() -> u8 {
    unsafe {
        // Phase 1: PSRAM octal HEX @ 200 MHz (stub).
        let _ = dfr0550::psram::init();

        // Phase 2: DSI DPHY rail (LDO_VO3 @ 2500 mV).
        let _dphy_ldo = dfr0550::ldo::LdoChannel::acquire_dphy();

        // Phase 3a: ungate DSI bus + bridge clocks, pulse bridge reset.
        dfr0550::dsi_host::clocks::enable_bus_and_reset();
        // Phase 3b: ungate PHY config + PLL ref clocks (default = PLL_F20M).
        dfr0550::dsi_host::clocks::enable_phy_clocks(
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m,
        );
        // Phase 3c: configure DPI pixel clock — 26 MHz from PLL_F240M.
        dfr0550::dsi_host::clocks::enable_dpi_clock(
            dfr0550::dsi_host::clocks::DpiClockSource::PllF240m,
            dfr0550::DPI_PIXEL_CLK_MHZ,
        );

        // Phase 4: wake the panel bridge.
        if dfr0550::i2c_bridge::wake().is_err() {
            return 1;
        }

        // Phase 5: DSI host @ 1 lane × 750 Mbps.
        let dsi = match dfr0550::dsi_host::init(
            dfr0550::DSI_LANES,
            dfr0550::DSI_LANE_MBPS,
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m.freq_mhz(),
        ) {
            Ok(b) => b,
            Err(dfr0550::dsi_host::DsiError::PllLock) => return 2,
            Err(dfr0550::dsi_host::DsiError::LaneCal) => return 3,
            Err(_) => return 4,
        };

        // Phase 6: DPI controller — currently returns Err(Unimplemented).
        // Drop the bus handle so it isn't optimized out before the LED loop.
        let _ = dsi;
        let _ = dfr0550::dpi_panel::DpiPanel::init(&dsi);

        0
    }
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

/// Configure GPIO3 (the user LED) as a push-pull output. Required because
/// the BSP only ungates I2C0 — GPIO3 is left in reset-default Hi-Z state
/// otherwise.
unsafe fn led_init() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let pin = bsp_generated::board::LED as usize;
    let mask = 1u32 << pin;

    // IO MUX: select the simple GPIO function for this pin (mcu_sel = 1).
    p.IO_MUX
        .gpio(pin)
        .modify(|_, w| unsafe { w.mcu_sel().bits(1) }.fun_ie().clear_bit());

    // GPIO matrix: drive output from gpio_out reg (sig_out_sel = 256 = simple).
    p.GPIO
        .func_out_sel_cfg(pin)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });

    // Output enable.
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(mask) });
    // Start with LED off.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
}

/// Long delay loop tuned for ~250 ms at 400 MHz CPU. Adjust if the BSP
/// configures a different CPU clock.
fn delay_long() {
    for _ in 0..40_000_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
}

fn delay_short() {
    for _ in 0..10_000_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
}

/// Encode the bring-up result on the on-board LED.
///   status = 0   → solid ON (every step succeeded)
///   status = N>0 → N short blinks, long pause, repeat
fn led_status_loop(status: u8) -> ! {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << bsp_generated::board::LED;
    loop {
        if status == 0 {
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
            delay_long();
            continue;
        }
        for _ in 0..status {
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
            delay_short();
            p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
            delay_short();
        }
        delay_long();
    }
}
