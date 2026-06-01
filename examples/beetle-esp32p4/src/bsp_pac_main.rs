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

    // Set up GPIO 5 (marker) as a Saleae debug output. GPIO 5 carries
    // *all* phase information: short ~150 µs pulses before each long
    // bracket (one per phase boundary), then sustained HIGH for wake()
    // and dsi_host::init. GPIO 4 and GPIO 6 were tried as a second
    // "refresh" channel and both hung bring-up cold — presumed wired
    // to something on the DFR1237 shield that we haven't accounted for.
    unsafe { debug_pins_init() };

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

    // DO NOT add any delay between route_pins() / led_init() and
    // run_bringup() — 2026-06-01 bench round confirmed that a multi-
    // second gap (3 boot blinks = ~6s) between route_pins and the first
    // I2C0 transaction causes wake() to hang on the very first MMIO
    // write (no dips on the marker). The 2026-05-30 wake-works state
    // had only ~µs between route_pins and wake (the removed
    // probe_init_state() call was µs-scale, not a load-bearing
    // initialization). The I2C0 master appears to lose configuration
    // if not exercised within a short window after route_pins.

    // Full bring-up flow. Diagnostic checkpoints AFTER wake / DSI run
    // inside run_bringup_instrumented to avoid the same trap.
    let status = unsafe { run_bringup_instrumented() };
    led_status_loop(status);
}

/// Simple LED blink primitive — no wdt feeding, no markers, just N
/// ON/OFF cycles at ~1Hz. Used as diagnostic checkpoints.
fn led_blink_simple(n: u8) {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << bsp_generated::board::LED;
    for _ in 0..n {
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) }); // ON
        for _ in 0..16_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) }); // OFF
        for _ in 0..16_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}

/// Wrapper around run_bringup that adds slow-LED-blink checkpoints
/// between phases so we can SEE on the LED how far we got. Unlike the
/// existing GPIO-5 phase pulses (microsecond scale), these are visible
/// to the eye without a Saleae.
unsafe fn run_bringup_instrumented() -> u8 {
    // After Phase 1-3c, BEFORE wake.
    let _ = unsafe { dfr0550::psram::init() };
    let _dphy_ldo = dfr0550::ldo::LdoChannel::acquire_dphy();
    unsafe { dfr0550::dsi_host::clocks::enable_bus_and_reset() };
    unsafe {
        dfr0550::dsi_host::clocks::enable_phy_clocks(
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m,
        );
    }
    unsafe {
        dfr0550::dsi_host::clocks::enable_dpi_clock(
            dfr0550::dsi_host::clocks::DpiClockSource::PllF240m,
            dfr0550::DPI_PIXEL_CLK_MHZ,
        );
    }

    // (Checkpoint A removed — adding a 2s delay here makes wake() hang.
    // The original run_bringup() had only ~150µs phase pulses between
    // DSI clock setup and wake; a 2s delay breaks that ordering. So
    // diagnostic blinks must come AFTER wake completes, not before it.)

    // Phase 4: wake.
    let wake_result = unsafe { wake_instrumented() };
    use dfr0550::i2c0::I2cError;
    use dfr0550::i2c_bridge::BridgeError;
    use core::sync::atomic::Ordering;
    match wake_result {
        Ok(()) => {}
        Err(BridgeError::I2c(I2cError::Nack)) => return 5,
        Err(BridgeError::I2c(I2cError::Hang)) => {
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

    // Checkpoint B: 4 slow blinks = "wake() succeeded, about to DSI".
    led_blink_simple(4);

    // Phase 5: DSI host.
    let dsi_result = dfr0550::dsi_host::init(
        dfr0550::DSI_LANES,
        dfr0550::DSI_LANE_MBPS,
        dfr0550::dsi_host::clocks::PhyClockSource::PllF20m.freq_mhz(),
    );
    let dsi = match dsi_result {
        Ok(b) => b,
        Err(dfr0550::dsi_host::DsiError::PllLock) => return 2,
        Err(dfr0550::dsi_host::DsiError::LaneCal) => return 3,
        Err(_) => return 4,
    };

    // Checkpoint C: 6 slow blinks = "DSI succeeded, about to DPI".
    led_blink_simple(6);

    let _ = dsi;
    let _ = dfr0550::dpi_panel::DpiPanel::init(&dsi);
    0
}

fn led_blink_n(n: u8) {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << bsp_generated::board::LED;
    for _ in 0..n {
        feed_watchdogs();
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) }); // ON
        for _ in 0..16_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        feed_watchdogs();
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) }); // OFF
        for _ in 0..16_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}

fn led_pause_long() {
    // Break the long pause into smaller chunks with WDT feeds between
    // — the WDT counter still ticks even if our disable bit is set.
    for _ in 0..5 {
        feed_watchdogs();
        for _ in 0..16_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}

fn led_solid_on() -> ! {
    // Heartbeat pattern: short blip, brief gap, short blip, long pause.
    // ON 200ms, OFF 200ms, ON 200ms, OFF 1.5s. Clearly visible and
    // distinguishable from both solid-ON (hang) and led_blink_n's
    // regular pattern.
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << bsp_generated::board::LED;
    loop {
        // Beat 1: ON 200ms.
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
        for _ in 0..8_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        // OFF 200ms.
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
        for _ in 0..8_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        // Beat 2: ON 200ms.
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
        for _ in 0..8_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
        // OFF 1.5s (long pause between heartbeats).
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
        for _ in 0..60_000_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }
}

unsafe fn run_bringup() -> u8 {
    unsafe {
        // Phase pulses on GPIO 5: one short ~150 µs HIGH pulse *after*
        // each phase completes. The wake() bracket then takes GPIO 5
        // HIGH for the duration of wake() (visibly sustained vs. the
        // short pulses). Counting short pulses on GPIO 5 = how far we
        // got before any hang.
        //   1 short pulse  = past Phase 1 (PSRAM)
        //   2 short pulses = past Phase 2 (LDO)
        //   3 short pulses = past Phase 3a (DSI bus + reset)
        //   4 short pulses = past Phase 3b (PHY clocks)
        //   5 short pulses = past Phase 3c (DPI clock)
        //   then GPIO 5 sustained HIGH = inside wake()
        //   6th short pulse after the bracket drops = wake() completed

        // Phase 1: PSRAM octal HEX @ 200 MHz (stub).
        let _ = dfr0550::psram::init();
        debug_phase_pulse();

        // Phase 2: DSI DPHY rail (LDO_VO3 @ 2500 mV).
        let _dphy_ldo = dfr0550::ldo::LdoChannel::acquire_dphy();
        debug_phase_pulse();

        // Phase 3a: ungate DSI bus + bridge clocks, pulse bridge reset.
        dfr0550::dsi_host::clocks::enable_bus_and_reset();
        debug_phase_pulse();
        // Phase 3b: ungate PHY config + PLL ref clocks (default = PLL_F20M).
        dfr0550::dsi_host::clocks::enable_phy_clocks(
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m,
        );
        debug_phase_pulse();
        // Phase 3c: configure DPI pixel clock — 26 MHz from PLL_F240M.
        dfr0550::dsi_host::clocks::enable_dpi_clock(
            dfr0550::dsi_host::clocks::DpiClockSource::PllF240m,
            dfr0550::DPI_PIXEL_CLK_MHZ,
        );
        debug_phase_pulse();

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
        // Inlined wake() with progress dips on the marker pin. The
        // bracket is HIGH for the whole transaction; brief LOW dips
        // (~150 µs) inside the HIGH band mark each completed sub-step:
        //   dip 1 = POWERON write returned
        //   dip 2 = 20 ms post-POWERON delay elapsed
        //   dip 3 = PORTB poll returned (success or NotReady)
        //   dip 4 = PORTA write returned
        //   dip 5 = PWM write returned
        // Whichever dip is missing identifies the hung step.
        let wake_result = wake_instrumented();
        debug_marker_set(false);
        debug_phase_pulse();
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

        // WAKE-SUCCESS DANCE — five distinct LED flashes (active-low)
        // that only run if wake() returned Ok. Easy to spot by eye
        // without a Saleae. If the operator does NOT see five quick
        // flashes a moment after the boot beacon, wake() is still
        // hanging despite the marker-dip evidence.
        {
            let p = unsafe { esp32p4::Peripherals::steal() };
            let led_mask = 1u32 << bsp_generated::board::LED;
            for _ in 0..5 {
                p.GPIO.out_w1tc().write(|w| unsafe { w.bits(led_mask) }); // ON
                for _ in 0..8_000_000u32 {
                    unsafe { core::arch::asm!("nop") };
                }
                p.GPIO.out_w1ts().write(|w| unsafe { w.bits(led_mask) }); // OFF
                for _ in 0..8_000_000u32 {
                    unsafe { core::arch::asm!("nop") };
                }
            }
        }

        // Phase 5: DSI host @ 1 lane × 750 Mbps. Bracket the call with
        // a stage-marker HIGH pulse. Distinct from the wake() pulse:
        // there's no I2C activity inside this pulse, AND there's now a
        // visible ~100 ms LOW gap preceding it (the wait loop above).
        debug_marker_set(true);
        let dsi_result = dfr0550::dsi_host::init(
            dfr0550::DSI_LANES,
            dfr0550::DSI_LANE_MBPS,
            dfr0550::dsi_host::clocks::PhyClockSource::PllF20m.freq_mhz(),
        );
        debug_marker_set(false);
        let dsi = match dsi_result {
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

    // ALL FOUR wprotect registers on ESP32-P4 (LP_WDT main, LP_WDT SWD,
    // TIMG0, TIMG1) use the SAME magic 0x50D8_3AA1 (the older-chip SWD
    // magic 0x8F1D_312A silently no-ops on P4 — see ERRATA-007).
    // Re-lock each wprotect with write(0) at the end per esp-hal's
    // pattern (defensive against stray writes re-arming the WDT).
    p.LP_WDT.wprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.LP_WDT.config0().write(|w| unsafe { w.bits(0) });
    p.LP_WDT.feed().write(|w| w.feed().set_bit());
    p.LP_WDT.wprotect().write(|w| unsafe { w.bits(0) });

    // SWD: enable swd_auto_feed_en ONLY (matches esp-hal; setting
    // swd_disable was tested 2026-06-01 and is NOT the wake regression
    // source — bench-confirmed by reverting to 2026-05-30 bytes for
    // disable_watchdogs() with no change in observed wake hang).
    p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.LP_WDT.swd_config().modify(|_, w| w.swd_auto_feed_en().set_bit());
    p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0) });

    p.TIMG0.wdtwprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG0.wdtconfig0().write(|w| unsafe { w.bits(0) });
    p.TIMG0.wdtfeed().write(|w| unsafe { w.bits(1) });
    p.TIMG0.wdtwprotect().write(|w| unsafe { w.bits(0) });

    p.TIMG1.wdtwprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG1.wdtconfig0().write(|w| unsafe { w.bits(0) });
    p.TIMG1.wdtfeed().write(|w| unsafe { w.bits(1) });
    p.TIMG1.wdtwprotect().write(|w| unsafe { w.bits(0) });
}

/// Feed all watchdogs once. Belt-and-suspenders: even with the
/// disable sequence in [`disable_watchdogs`], call this before any
/// long-running spin loop (PORTB poll, DSI PLL lock, NOP delays)
/// to be safe against any WDT that snuck through the disable.
///
/// Uses the correct ESP32-P4 wprotect magic `0x50D8_3AA1` for all
/// four registers (LP_WDT main, LP_WDT SWD, TIMG0, TIMG1). The
/// previous `0x8F1D_312A` value used for SWD was the S3/C3 magic and
/// silently failed on P4 — every "SWD feed" write went into a locked
/// register. See ERRATA-007.
fn feed_watchdogs() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    p.LP_WDT.wprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.LP_WDT.feed().write(|w| w.feed().set_bit());
    p.LP_WDT.wprotect().write(|w| unsafe { w.bits(0) });

    p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.LP_WDT.swd_config().modify(|_, w| w.swd_feed().set_bit());
    p.LP_WDT.swd_wprotect().write(|w| unsafe { w.bits(0) });

    p.TIMG0.wdtwprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG0.wdtfeed().write(|w| unsafe { w.bits(1) });
    p.TIMG0.wdtwprotect().write(|w| unsafe { w.bits(0) });

    p.TIMG1.wdtwprotect().write(|w| unsafe { w.bits(0x50D8_3AA1) });
    p.TIMG1.wdtfeed().write(|w| unsafe { w.bits(1) });
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
    // `debug_pins_init` (which is confirmed working on GPIO 5). If
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

/// GPIO pin number used as the Saleae stage marker (HIGH during a
/// long-running phase like wake() or DSI init). In the 0-31 range so
/// standard `out_w1ts` / `out_w1tc` apply.
const DBG_MARKER_PIN: usize = 5;

/// Configure GPIO5 (marker) as a push-pull output. GPIO 4 was tried
/// as a refresh-tick output and hung bring-up inside wake() with the
/// marker stuck HIGH (10+ s, no Morse, no I2C activity). GPIO 6 had
/// the same effect at an even earlier stage. Both pins are presumed
/// physically tied to something on the DFR1237 shield (bridge reset?
/// I2C pull-up? unknown without schematic) and are off-limits until
/// verified. Phase progress is now encoded via short pre-wake pulses
/// on GPIO 5 itself.
unsafe fn debug_pins_init() {
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

/// Drive the stage marker high or low. Used to bracket long-running
/// phases (I2C wake, DSI init) on the Saleae trace.
fn debug_marker_set(high: bool) {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << DBG_MARKER_PIN;
    if high {
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
    } else {
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
    }
}

/// Emit a short HIGH pulse on the **marker** pin (GPIO 5). Used to
/// count phase progress on the Saleae before wake()'s long bracket
/// starts. Pulse width is short (~150 µs at 400 MHz) so individual
/// pulses are visibly distinct from the sustained wake / DSI brackets.
fn debug_phase_pulse() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << DBG_MARKER_PIN;
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
    for _ in 0..60_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
}

/// Brief LOW dip (~150 µs) on the marker pin, then back HIGH. Inverse
/// of debug_phase_pulse — for sub-step progress markers *inside* a
/// sustained-HIGH bracket. Caller must ensure GPIO 5 is already HIGH.
fn debug_marker_dip() {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let mask = 1u32 << DBG_MARKER_PIN;
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
    for _ in 0..60_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
}

/// Inlined version of `dfr0550::i2c_bridge::wake()` with a marker dip
/// after each sub-step. The marker must be HIGH on entry and is
/// guaranteed HIGH on every return path so the caller's bracket
/// boundary is preserved.
unsafe fn wake_instrumented() -> Result<(), dfr0550::i2c_bridge::BridgeError> {
    use dfr0550::i2c0;
    use dfr0550::i2c_bridge::{
        BRIDGE_ADDR, BridgeError, PORTA_KERNEL_DEFAULT, REG_PORTA, REG_PORTB, REG_POWERON, REG_PWM,
    };

    i2c0::write_reg(BRIDGE_ADDR, REG_POWERON, 1)?;
    debug_marker_dip(); // dip 1

    for _ in 0..7_200_000u32 {
        unsafe { core::arch::asm!("nop") };
    }
    debug_marker_dip(); // dip 2

    let mut tries = 100;
    loop {
        match i2c0::read_reg(BRIDGE_ADDR, REG_PORTB) {
            Ok(pb) if pb & 0x01 != 0 => break,
            Ok(_) => {}
            Err(i2c0::I2cError::Nack) => {}
            Err(e) => return Err(BridgeError::I2c(e)),
        }
        tries -= 1;
        if tries == 0 {
            return Err(BridgeError::NotReady);
        }
        for _ in 0..3_600_000u32 {
            unsafe { core::arch::asm!("nop") };
        }
    }
    debug_marker_dip(); // dip 3

    i2c0::write_reg(BRIDGE_ADDR, REG_PORTA, PORTA_KERNEL_DEFAULT)?;
    debug_marker_dip(); // dip 4

    i2c0::write_reg(BRIDGE_ADDR, REG_PWM, 255)?;
    debug_marker_dip(); // dip 5

    Ok(())
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

/// One Morse "unit" in NOPs — tuned for ~100 ms at 400 MHz CPU
/// (matches the `delay_short` calibration of ~62 ms per 10 M nops →
/// ~100 ms per 16 M nops). Standard ITU Morse timing:
///   dot = 1 unit, dash = 3 units, intra-element gap = 1 unit,
///   inter-character gap = 3 units (i.e. 2 additional after trailing
///   intra-element), inter-message gap = 7+ units (we use more).
const MORSE_UNIT_NOPS: u32 = 16_000_000;

/// Morse encoding of digits 0-9 (ITU-R M.1677-1). Each digit is a
/// fixed 5-element pattern. 0 = dot, 1 = dash.
const MORSE_DIGITS: [[u8; 5]; 10] = [
    [1, 1, 1, 1, 1], // 0: -----
    [0, 1, 1, 1, 1], // 1: .----
    [0, 0, 1, 1, 1], // 2: ..---
    [0, 0, 0, 1, 1], // 3: ...--
    [0, 0, 0, 0, 1], // 4: ....-
    [0, 0, 0, 0, 0], // 5: .....
    [1, 0, 0, 0, 0], // 6: -....
    [1, 1, 0, 0, 0], // 7: --...
    [1, 1, 1, 0, 0], // 8: ---..
    [1, 1, 1, 1, 0], // 9: ----.
];

fn morse_delay(units: u32) {
    let total = MORSE_UNIT_NOPS.saturating_mul(units);
    for _ in 0..total {
        unsafe { core::arch::asm!("nop") };
    }
}

fn morse_element(p: &esp32p4::Peripherals, mask: u32, is_dash: bool) {
    // Active-low LED: pin LOW (out_w1tc) = LED ON, pin HIGH = LED OFF.
    p.GPIO.out_w1tc().write(|w| unsafe { w.bits(mask) });
    morse_delay(if is_dash { 3 } else { 1 });
    p.GPIO.out_w1ts().write(|w| unsafe { w.bits(mask) });
    morse_delay(1); // intra-element gap
}

fn morse_digit(p: &esp32p4::Peripherals, mask: u32, digit: u8) {
    for &el in &MORSE_DIGITS[digit as usize] {
        morse_element(p, mask, el == 1);
    }
}

/// ITU prosign HH (`........`) — universal "error / correction" cue.
/// Transmitted as 8 dots with intra-element gaps but no inter-character
/// gap inside the prosign itself.
fn morse_hh(p: &esp32p4::Peripherals, mask: u32) {
    for _ in 0..8 {
        morse_element(p, mask, false);
    }
}

/// Encode the bring-up result on the on-board LED as Morse.
///   status = 0   → solid ON (every step succeeded)
///   status > 0   → preamble "HH HH" (Morse error prosign x2) followed
///                  by the status code as a 3-digit zero-padded decimal
///                  (e.g. 30 → "030", 5 → "005"). Long blank silence
///                  between messages so the preamble is easy to pick
///                  out as the start of each repeat.
///
/// DFR1172 user LED on GPIO 3 is **active-low** — driving the pin LOW
/// turns the LED ON, driving it HIGH turns it OFF. Confirmed via the
/// blink-all sanity test (LED visible in the low-phase only).
///
/// The debug marker (GPIO5) is held LOW throughout the status loop so
/// the only marker events on the Saleae trace are the HIGH pulses
/// bracketing `wake()` and `dsi_host::init()` in `run_bringup`.
fn led_status_loop(status: u8) -> ! {
    let p = unsafe { esp32p4::Peripherals::steal() };
    let led_mask = 1u32 << bsp_generated::board::LED;

    if status == 0 {
        // Active-low: pin LOW = LED ON.
        p.GPIO.out_w1tc().write(|w| unsafe { w.bits(led_mask) });
        loop {
            morse_delay(20);
        }
    }

    let digits = [
        (status / 100) % 10,
        (status / 10) % 10,
        status % 10,
    ];

    loop {
        // Quiet baseline at the start of every transmission.
        p.GPIO.out_w1ts().write(|w| unsafe { w.bits(led_mask) });
        morse_delay(3);

        // Preamble: HH (error prosign) twice. Each HH already trails
        // one intra-element gap (1 unit); add 2 more for the standard
        // inter-character gap (3 units total).
        morse_hh(&p, led_mask);
        morse_delay(2);
        morse_hh(&p, led_mask);
        morse_delay(2);

        // 3-digit zero-padded status code.
        morse_digit(&p, led_mask, digits[0]);
        morse_delay(2);
        morse_digit(&p, led_mask, digits[1]);
        morse_delay(2);
        morse_digit(&p, led_mask, digits[2]);
        morse_delay(2);

        // ITU FULL STOP `.-.-.-` — explicit end-of-message terminator
        // so the operator knows the digit group has fully transmitted
        // before the inter-message silence begins.
        for &el in &MORSE_FULL_STOP {
            morse_element(&p, led_mask, el == 1);
        }

        // Long inter-message silence — much longer than ITU's 7 units
        // so the next preamble reads as a fresh start, not a trailing
        // continuation of the previous code.
        morse_delay(20);
    }
}

/// ITU FULL STOP (period): `.-.-.-` — 6 elements alternating.
const MORSE_FULL_STOP: [u8; 6] = [0, 1, 0, 1, 0, 1];
