<!--
CHANGELOG.md - Notes on chip & board database releases.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Changelog

## v0.2.0

### Multi-vendor BSP generation
- **5 vendors with full YAML→IR→render pipelines**: Espressif (9 chips,
  14 boards), Nordic nRF (nRF52840 + DK), NXP i.MX RT (MIMXRT1062 + EVKB),
  RP2040 (Pico), Renesas RA (R7FA6M5BH + EK-RA6M5).
- Each vendor has a distinct pin routing model captured in vendor-specific IR
  types: ESP IO MUX + GPIO matrix, Nordic PSEL, NXP IOMUX ALT + daisy chain,
  RP FUNCSEL + RESETS, Renesas PFS PSEL + MSTP.
- 6 MiniJinja templates per vendor generating real PAC register writes.
- `rlvgl-creator bsp from-yaml --vendor <v>` CLI dispatch for all 5 vendors.
- `bsp list-chips --vendor <v>` and `bsp list-boards --vendor <v>` commands.
- All 9 chipdb crates overhauled from legacy binary-blob stubs to YAML
  auto-discovery with `chip_yaml()` / `board_yaml()` / `chip_names()` /
  `board_names()` APIs.

### UEFI backend
- `rlvgl-platform` gains a `uefi` feature with GOP framebuffer display,
  SimpleTextInput keyboard polling (with synthesized KeyUp events), and Serial
  I/O playit transport for test automation over QEMU virtio-serial.
- `rlvgl-example-uefi-disco` runs the disco-demo controller as a UEFI boot
  application.

### Test automation (rlvgl-playit)
- Serial test driver with touch injection, pointer/key events, multi-touch
  frames, widget queries, framebuffer pixel dumps, and event recording.
- Node.js test harness for end-to-end simulator automation.
- UEFI serial transport for pre-OS test driving.

### STM32H747I-DISCO platform
- DMA2D hardware acceleration with ISR-driven completion.
- WM8994 audio codec over I2C4 + SAI1 I2S TX + SAI4 PDM microphone.
- SDMMC block device with FATFS adapter for SD card assets.
- QSPI flash support.
- Star crawl motion system (mirrored starfield + FIR text overlay).
- CPU stats via DWT/D3 SRAM telemetry.

### Rendering and UI
- Anti-aliased rounded corners in core draw.
- Compositor save-under and dirty-region restoration.
- Focus highlights and keyboard navigation.
- ColorFormat profile on Screen/DisplayDriver.
- Motion primitives: Direction, MotionRate, JumboBuffer, BackgroundPattern,
  Crawl trait, TextCrawl engine, CrawlWindow composable widget.

### Infrastructure
- All 31 workspace crates at version 0.2.0.
- Publish script with 24-crate dependency-ordered publish and crates.io
  index-wait between dependents.
- Pre-publish validation: 7 phases (fmt, clippy, tests, playit, creator,
  embedded build, docs).

## v0.1.9
- See [RELEASE-v0.1.9.md](./RELEASE-v0.1.9.md) for detailed notes.
- STM32 BSP generation from CubeMX `.ioc` files.
- Initial vendor chipdb crate stubs (9 vendors).
- STM32H747I-DISCO board bring-up: display, touch, SD, backlight.
- i18n crate with compile-time translation blobs.

## v0.1.7
- Initial vendor crates for STM, Nordic, Espressif, NXP, Silicon Labs,
  Microchip, Renesas, Texas Instruments, and RP2040 boards.
- Added `scripts/gen_ioc_bsps.sh` to batch-convert CubeMX `.ioc` files.
- `rlvgl-creator` can now load canonical MCU definitions alongside board
  overlays from vendor archives.

## v0.1.6
- FATFS adapter (`platform::sd_fatfs_adapter`) and optional SD assets demo.
- DISCO docs: linker script, touch I2C, backlight ramp, SDMMC checklist.
- `--allow-reserved` flag for SWD pins in `bsp from-ioc`.
