//! WDT1 disable helper.
//!
//! U-Boot on AM335x enables WDT1 with a ~60 s timeout so that a hung
//! bootloader reboots rather than wedging the SoC. Linux's kernel
//! takes over and kicks the timer on its own schedule. Bare-metal
//! code that doesn't kick the WDT will hit the timeout and reset —
//! which looks like "screen resets every so often" from the user's
//! side (the whole SoC reboots, U-Boot rerun our uenvcmd, LCDC is
//! reprogrammed, panel flickers).
//!
//! Disabling WDT1 requires a two-write unlock sequence to WDT_WSPR.
//! Each write must wait for WWPS (Write Posting Status) to clear
//! before the next access.
//!
//! Reference: AM335x TRM SPRUH73Q §20.4.1.2 (watchdog disable sequence).

use super::am335x::{WDT1_WSPR, WDT1_WWPS, reg_read, reg_write};

pub unsafe fn disable() {
    unsafe {
        // Wait for any pending WSPR write
        while reg_read(WDT1_WWPS) & (1 << 4) != 0 {}
        reg_write(WDT1_WSPR, 0x0000_AAAA);
        while reg_read(WDT1_WWPS) & (1 << 4) != 0 {}
        reg_write(WDT1_WSPR, 0x0000_5555);
        while reg_read(WDT1_WWPS) & (1 << 4) != 0 {}
    }
}
