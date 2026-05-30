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

    // Disable every watchdog the IDF bootloader left armed before doing
    // anything else. Without this, the LED diagnostic and any spin loops
    // (I2C wait, DSI PHY lock poll) get cut short by an RTC WDT reset at
    // ~3 s, looping the LED pattern back to the bootloader and producing
    // a hard-to-read blink train. See [`disable_watchdogs`].
    unsafe { disable_watchdogs() };

    // BSP-managed clocks, IO MUX, peripheral init.
    bsp_generated::init();

    // Set up GPIO5 as a debug marker output. The Saleae trace
    // captures a HIGH pulse on this pin spanning each I2C wake
    // attempt — so the operator can see whether SDA/SCL actually
    // moved during that window vs the master sitting silent. GPIO5
    // is broken out on the DFR1237 IO-expansion shield (pin 16 of
    // U2 ESP32-P4_L, memalpha doc 427), inside the 0-31 range so
    // standard `enable_w1ts` / `out_w1ts` work without going to
    // the upper-GPIO register bank. Init the marker BEFORE the pad
    // sanity test so it can bracket that test window.
    unsafe { debug_marker_init() };

    // Pad sanity test removed — confirmed via LED-loop mirror that
    // GPIO 4, 7, 8 can all be driven by software. The pads are fine;
    // the I2C0 master itself is what fails to start.

    // The BSP generator currently emits I2C pins as plain GPIOs; route
    // them through the GPIO matrix to the I2C0 peripheral here until the
    // generator learns to do this itself.
    unsafe {
        dfr0550::i2c0::route_pins();
    }

    // Set up the user LED so we can encode bring-up status in blink count.
    unsafe { led_init() };

    // Path (1) of BEETLE ERRATA-005: read back the I2C0 init registers
    // and surface a 20-29 LED code if any write didn't stick. If this
    // returns 0, write-sticking is confirmed and the failure is
    // elsewhere — at which point we fall through to run_bringup() and
    // expect the same status=11 (Hang) we've been getting, which routes
    // the investigation to path (2) (IDF first-light register diff).
    //
    // Diagnostic blink count:
    //   1-4   = DSI / DPI bring-up step failures (run_bringup)
    //   5-9   = I2C0 wake-step failures (run_bringup)
    //   11    = I2C0 master Hang sentinel (run_bringup)
    //   20-29 = I2C0 init register read-back probe failed (this call)
    let probe = unsafe { dfr0550::i2c0::probe_init_state() };
    let status: u8 = if probe != 0 { probe } else { unsafe { run_bringup() } };
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

        // Bracket the I2C wake call with a debug marker pulse so the
        // Saleae trace shows exactly when we're inside `wake()`. Goes
        // HIGH right before the first I2C transaction, LOW right after
        // (regardless of result).
        debug_marker_set(true);

        // Phase 4: wake the panel bridge. Diagnostic split per ERRATA-002 —
        // each I2C-side failure mode gets its own LED-blink count so the
        // bench operator can tell which step broke without serial output.
        //   5 blinks: I2C NACK on first write → bus working but no slave
        //             at 0x45 (likely matrix routing / pin selection bug).
        //   6 blinks: I2C Hang → master controller never asserted
        //             MST_COMPLETE (clock gate / SCL not toggling).
        //   7 blinks: I2C Timeout → SCL stuck, bus error.
        //   8 blinks: I2C Arbitration → bus contention.
        //   9 blinks: BridgeError::NotReady → POWERON write succeeded but
        //             PORTB.0 poll never went high in 1 s (bridge powered
        //             but not coming up — could be panel power, BOOT0
        //             strap, or wrong PORTB bit).
        use dfr0550::i2c0::I2cError;
        use dfr0550::i2c_bridge::BridgeError;
        let wake_result = dfr0550::i2c_bridge::wake();
        debug_marker_set(false);
        // Hang now returns 30 + scl_main_state_last so the bench operator
        // can decode how far the master FSM got before the spin loop
        // gave up: 30=Idle (never moved), 31=AddressShift, 32=AckAddress,
        // 33=RxData, 34=TxData, 35=SendAck, 36=WaitAck. See BEETLE
        // ERRATA-005 — distinguishes "master never started" from
        // "master started but stalled mid-byte".
        use core::sync::atomic::Ordering;
        match wake_result {
            Ok(()) => {}
            Err(BridgeError::I2c(I2cError::Nack)) => return 5,
            Err(BridgeError::I2c(I2cError::Hang)) => {
                // Encoded: (txfifo_cnt << 3) | scl_main_state_last.
                // Priority: txfifo_cnt == 0 outranks FSM state because
                // an empty FIFO means our bytes never made it to the
                // hardware — FSM state is meaningless in that case.
                let st = dfr0550::i2c0::LAST_HANG_STATE.load(Ordering::Relaxed);
                let txcnt = st >> 3;
                let scl_state = st & 0x07;
                if txcnt == 0 {
                    return 50;
                }
                return 30u8.saturating_add(scl_state);
            }
            Err(BridgeError::I2c(I2cError::Timeout)) => return 7,
            Err(BridgeError::I2c(I2cError::Arbitration)) => return 8,
            Err(BridgeError::NotReady) => return 9,
        }

        // Round-11 BEETLE ERRATA-005: short-circuit on wake success.
        // We return 1 instead of 0 because the on-board LED appears to
        // be active-low (in blink-all every pin goes high then low
        // every ~1 s; LED visible in the low phase). status=0 puts the
        // LED solid HIGH which on an active-low LED looks indistinguishable
        // from "no power". status=1 produces a clear 1-blink pattern so
        // the bench operator can distinguish success from "chip dead".
        return 1;
        #[allow(unreachable_code)]
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

/// Disable LP_WDT (RTC main + Super), TIMG0 WDT, TIMG1 WDT.
///
/// The IDF bootloader v5.5.3 leaves the RTC main WDT armed with
/// ~9 s timeout (CONFIG_BOOTLOADER_WDT_TIME_MS) and the Super WDT
/// running. Empirically the cycle observed on the FireBeetle is closer
/// to ~3 s — the Super WDT (LP_WDT.swd) is likely the load-bearing one
/// since it runs against the slow clock with shorter limits.
///
/// Each WDT has a write-protect register that must be unlocked first
/// with the magic value, then the enable bit cleared in the config
/// register, then the protect re-locked (write any non-magic value).
///
/// Magic values per ESP32-P4 TRM:
///   LP_WDT.WPROTECT      = 0x50D83AA1 (RTC main WDT)
///   LP_WDT.SWD_WPROTECT  = 0x8F1D312A (Super WDT)
///   TIMG.WDTWPROTECT     = 0x50D83AA1 (TIMG0/1 WDT)
unsafe fn disable_watchdogs() {
    let p = unsafe { esp32p4::Peripherals::steal() };

    // LP_WDT (RTC main): unlock, disable, relock.
    p.LP_WDT
        .wprotect()
        .write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.LP_WDT.config0().modify(|_, w| w.wdt_en().clear_bit());
    p.LP_WDT.wprotect().write(|w| unsafe { w.bits(0) });

    // LP_WDT (Super WDT): unlock, disable, relock. The SWD's "disable"
    // bit is `swd_disable` in `swd_config`.
    p.LP_WDT
        .swd_wprotect()
        .write(|w| unsafe { w.bits(0x8F1D_312A) });
    p.LP_WDT
        .swd_config()
        .modify(|_, w| w.swd_disable().set_bit());
    p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0) });

    // TIMG0 / TIMG1 WDT: unlock, disable, relock. Both share the same
    // magic value for the write-protect.
    p.TIMG0
        .wdtwprotect()
        .write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG0.wdtconfig0().modify(|_, w| w.wdt_en().clear_bit());
    p.TIMG0.wdtwprotect().write(|w| unsafe { w.bits(0) });

    p.TIMG1
        .wdtwprotect()
        .write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG1.wdtconfig0().modify(|_, w| w.wdt_en().clear_bit());
    p.TIMG1.wdtwprotect().write(|w| unsafe { w.bits(0) });
}

/// Drive GPIO 7 and GPIO 8 directly as plain push-pull GPIO outputs
/// for ~200 ms with a clear toggle pattern. The Saleae trace should
/// show these pins TOGGLING during the test window. If they stay
/// flat, the pad-output path itself is broken (wrong header pin,
/// broken shield trace, IO MUX still locked to a non-GPIO function,
/// pad_driver / pull-up override).
///
/// The marker (GPIO5) is driven HIGH for the duration of the test so
/// you can find this window on the Saleae by triggering on rising
/// edge of ch 2 (marker) and looking BEFORE the next I2C wake attempt.
unsafe fn pad_sanity_test() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    const SCL_PIN: usize = 8;
    const SDA_PIN: usize = 7;
    const CTRL_PIN: usize = 4;
    let scl_mask = 1u32 << SCL_PIN;
    let sda_mask = 1u32 << SDA_PIN;
    let ctrl_mask = 1u32 << CTRL_PIN;
    let all_mask = scl_mask | sda_mask | ctrl_mask;

    // Configure each pin with the EXACT same write pattern as
    // `debug_marker_init` (which is confirmed working on GPIO 5). If
    // GPIO 5 toggles but GPIO 4/7/8 don't using identical writes, the
    // issue isn't this code path — it's the pads themselves (module-
    // internal wiring, alt-function lock, or wrong probe placement).
    for pin in [SCL_PIN, SDA_PIN, CTRL_PIN] {
        p.IO_MUX
            .gpio(pin)
            .modify(|_, w| unsafe { w.mcu_sel().bits(1) }.fun_ie().clear_bit());
        p.GPIO
            .func_out_sel_cfg(pin)
            .modify(|_, w| unsafe { w.out_sel().bits(256) });
    }
    // Output enable for all three.
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(all_mask) });

    // Marker HIGH during the whole test.
    let marker_mask = 1u32 << DBG_MARKER_PIN;
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(marker_mask) });

    // Toggle pattern: ~10 cycles, each ~80 ms. Distinct pattern: SCL up,
    // SDA up, CTRL up (synchronization marker), all down. CTRL is the
    // confidence channel — it MUST toggle for the test code to be
    // working at all.
    for _ in 0..10 {
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(scl_mask) });
        for _ in 0..200_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(sda_mask) });
        for _ in 0..200_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(ctrl_mask) });
        for _ in 0..200_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(all_mask & !marker_mask) });
        for _ in 0..200_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }

    // Leave the pins disabled (gpio_enable cleared) so the subsequent
    // I2C0 matrix routing takes over cleanly. The peripheral OEN will
    // re-enable output via OEN_SEL=0 default.
    p.GPIO.enable_w1tc().write(|w| unsafe { w.bits(all_mask) });

    // Marker LOW so the I2C wake pulse later is distinguishable.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(marker_mask) });
}

/// GPIO pin number used as the Saleae debug marker. Inside the 0-31
/// range so we don't have to use the upper-GPIO register bank.
const DBG_MARKER_PIN: usize = 5;

/// Configure GPIO48 as a push-pull output for the Saleae debug marker.
/// Same routing pattern as `led_init` (mcu_sel=1, sig_out_sel=256
/// "simple GPIO out from gpio_out reg"), just on a different pin.
unsafe fn debug_marker_init() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << DBG_MARKER_PIN;
    p.IO_MUX
        .gpio(DBG_MARKER_PIN)
        .modify(|_, w| unsafe { w.mcu_sel().bits(1) }.fun_ie().clear_bit());
    p.GPIO
        .func_out_sel_cfg(DBG_MARKER_PIN)
        .modify(|_, w| unsafe { w.out_sel().bits(256) });
    p.GPIO.enable_w1ts().write(|w| unsafe { w.bits(mask) });
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
}

/// Drive the debug marker high or low. Used to bracket I2C transactions
/// on the Saleae trace.
fn debug_marker_set(high: bool) {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << DBG_MARKER_PIN;
    if high {
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
    } else {
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
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
///
/// The debug marker (GPIO5) mirrors the LED state during the status
/// loop so a bench operator can probe-hunt to find which physical
/// header pin actually maps to GPIO5 — if the probe is on the right
/// pin, the marker trace will match the LED's blink pattern. The
/// marker pulse around `i2c_bridge::wake()` (set in `run_bringup`) is
/// still the primary diagnostic; this status-loop mirror is just a
/// "is the probe even on the right pin?" cross-check.
fn led_status_loop(status: u8) -> ! {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let led_mask = 1u32 << bsp_generated::board::LED;
    let marker_mask = 1u32 << DBG_MARKER_PIN;
    let combined_mask = led_mask | marker_mask;
    loop {
        if status == 0 {
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(combined_mask) });
            delay_long();
            continue;
        }
        for _ in 0..status {
            p.GPIO.out_w1ts().write(|w| unsafe { w.bits(combined_mask) });
            delay_short();
            p.GPIO.out_w1tc().write(|w| unsafe { w.bits(combined_mask) });
            delay_short();
        }
        delay_long();
    }
}
