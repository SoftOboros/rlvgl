//! Board-support module for the BeagleBone Black + NHD-7.0CTP-CAPE-P.
//!
//! The register definitions, PRCM helpers, pinmux helpers, and LCDC raster
//! init live here and are shared between the Linux (`/dev/mem`) and
//! bare-metal / FreeRTOS / Zephyr runtime paths. Only the userspace-only
//! modules (`devmem`, `devmem_display`) are gated on `linux`.

/// AM335x register address map (hand-written PAC).
pub mod am335x;

/// LCDC timing constants and raster controller driver.
pub mod lcdc;

/// PRCM clock enable helpers.
pub mod prcm;

/// CTRLMOD pin mux configuration.
pub mod pinmux;

/// Polled UART0 diagnostic output. Bare-metal path only; Linux has stderr.
#[cfg(feature = "bare_metal")]
pub mod uart0;

/// USR0..USR3 LED control for bare-metal visual stage breadcrumbs.
#[cfg(feature = "bare_metal")]
pub mod leds;

/// Watchdog timer disable (U-Boot leaves WDT1 running at boot).
#[cfg(feature = "bare_metal")]
pub mod wdt;

/// `/dev/mem` page-cache for Linux userspace register and framebuffer access.
#[cfg(feature = "linux")]
pub mod devmem;

/// `DisplayDriver` backed by a register-driven LCDC over `/dev/mem`.
#[cfg(feature = "linux")]
pub mod devmem_display;
