<!--
README.md - ESP-IDF comparison app for the FireBeetle ESP32-P4 + DFR0550 panel.
-->

# FireBeetle ESP32-P4 DFR0550 ESP-IDF Comparison

This is a standalone ESP-IDF app for the DFRobot FireBeetle 2 ESP32-P4
AI Vision board on the DFR1237 shield, driving the DFR0550-V2 5" 800x480
DSI display through Espressif's normal `esp_lcd` MIPI-DSI/DPI path.

It is intentionally separate from the raw-PAC Rust example in
`../beetle-esp32p4`. Use it as the control case for BEETLE-06/ERRATA-009:

- wake the Pi-style DFR0550 bridge at I2C address `0x45` using the IDF
  I2C master driver on GPIO7/GPIO8,
- power LDO_VO3 at 2500 mV with `esp_ldo_acquire_channel`,
- create the DSI bus with `esp_lcd_new_dsi_bus`,
- create the generic DPI panel with `esp_lcd_new_panel_dpi`, and
- continuously fill the RGB888 framebuffer with red, green, blue, white,
  and black frames.

## rlvgl Hybrid (BEETLE-IDF track)

> **Docs:** the full milestone history and conformance frame for this
> hybrid live in [`docs/beetle-esp32p4-idf/`](../../docs/beetle-esp32p4-idf/README.md)
> (concepts gate + chapters). Milestones to date: **M1** render bridge,
> **M3** FT5x06 touch, **M4** shared disco-demo mount, **M5** live
> backlight (bridge PWM + slider), and the **software star crawl**
> (BEETLE-IDF-05) on the Info → Star Crawl item.

By default this app no longer cycles solid colors — it renders an **rlvgl
widget tree** into the DPI framebuffer, interactively (capacitive touch +
live backlight). The split is deliberate:

- **C owns the hardware.** `main/dfr0550_idf_compare.c` keeps the full,
  known-locking IDF bring-up (PSRAM, LDO_VO3, I2C bridge wake,
  `esp_lcd_new_dsi_bus`, `esp_lcd_new_panel_dpi`). None of the DSI/DPHY
  path changed, so ERRATA-009 is side-stepped rather than fought.
- **Rust owns the pixels.** `components/rlvgl_app/` builds a no_std Rust
  staticlib (`librlvgl_app.a`, target `riscv32imafc-unknown-none-elf`,
  ilp32f to match the IDF toolchain) that exposes one C ABI entry point,
  `rlvgl_app_render(fb, w, h)` (see `components/rlvgl_app/include/rlvgl_app.h`).
  It draws a real `rlvgl-core` + `rlvgl-widgets` tree through a small
  self-contained RGB888 software renderer, straight into the DPI
  framebuffer. The C loop calls it each refill iteration (the bridge needs
  a continuous re-fill; a one-shot paint desyncs it) and then runs the
  same `esp_cache_msync(..., C2M)` writeback the color fill used.

The component's `CMakeLists.txt` runs `cargo build` via `ExternalProject`
and imports the archive, so a normal `idf.py build` builds the Rust payload
too. Prerequisites: `cargo` on `PATH` and
`rustup target add riscv32imafc-unknown-none-elf`.

To run the original solid-color control loop instead, enable
`CONFIG_DFR0550_COLOR_CYCLE` (under "DFR0550 comparison" in `menuconfig`).

## Build And Flash

```sh
cd examples/beetle-esp32p4-idf
. /Users/iraabbott/esp/esp-idf/export.sh
idf.py set-target esp32p4
idf.py build
idf.py flash monitor
```

The app logs `MIPI_DSI_HOST.phy_status` after DSI bus creation. A locking
run should show bit 0 set and should visibly cycle solid colors on the
panel.

Observed control result on 2026-06-14: normal IDF locked with
`phy_status=0x0000153d`, allocated a 1,152,000-byte RGB888 framebuffer at
`0x48001180`, and logged the red/green/blue/white/black fill loop.

## Isolation Modes

Wake the DFR0550 bridge and backlight, but skip DSI/DPI/framebuffer setup:

```sh
idf.py -B build-wake-only \
  -DSDKCONFIG_DEFAULTS='sdkconfig.defaults;sdkconfig.defaults.esp32p4;sdkconfig.wake-only' \
  set-target esp32p4 build flash monitor
```

Wake the bridge, then send `PWM=0` and `POWERON=0`, again without DSI/DPI:

```sh
idf.py -B build-power-off \
  -DSDKCONFIG_DEFAULTS='sdkconfig.defaults;sdkconfig.defaults.esp32p4;sdkconfig.power-off' \
  set-target esp32p4 build flash monitor
```

Boot and idle without touching the bridge, DSI host, DPI bridge, framebuffer,
or DSI PHY LDO:

```sh
idf.py -B build-no-touch \
  -DSDKCONFIG_DEFAULTS='sdkconfig.defaults;sdkconfig.defaults.esp32p4;sdkconfig.no-touch' \
  set-target esp32p4 build flash monitor
```

Observed isolation result on 2026-06-14: the panel's visible color cycle
persisted through wake-only, power-off-after-wake, and no-touch firmware.
Treat that visible cycle as panel/bridge-side retained or autonomous
behavior, not as proof that the ESP32-P4 framebuffer path is active.
