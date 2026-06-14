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
