<!-- README.md - Bench runbook for the DFR1117 Beetle ESP32-C6 example. -->

# Beetle ESP32-C6 + SSD1306 + STTS22H

This package hosts the portable rlvgl network-time application on the DFRobot
DFR1117 Beetle ESP32-C6. It uses the same shared ESP runtime as the DFR0868 C3;
the board entry point owns only C6 initialization and pin selection.

The DFR1117 header is physically pin-compatible with the DFR0868 header, but
the labeled I2C positions map to different MCU GPIOs:

| I2C device | DFR1117 Beetle ESP32-C6 |
| --- | --- |
| VCC / 3V3 | 3V3 |
| GND | GND |
| SDA | GPIO19 / SDA |
| SCL | GPIO20 / SCL |
| STTS22H INT (optional) | GPIO6 / LP_SDA |

The DFR0650 SSD1306 remains at `0x3c` by default. The SparkFun STTS22H remains
at the bench-selected `0x38` address, shares the bus, and should leave its
additional `I2C_PU` pair disconnected because the display board already
provides 4.7 kOhm pull-ups. Both devices run from 3.3 V. The polling demo does
not configure GPIO6; the `INT` connection is reserved for a later interrupt-
driven mode and is separate from the primary SDA line on GPIO19.

The C6 has its own flash, so the C3's stored network record does not follow it.
Seed the C6 once, then subsequent credential-free application flashes load the
same versioned `rlvgl_net/config_v1` record from its NVS partition.

This package uses the shared `examples/common/linkall-c6.x` linker root rather
than the beta HAL's generic `linkall.x`. The compatibility layout puts the
256-byte application descriptor and constants in the first cache-mapped
segment and page-aligns code into the second, matching the current ESP-IDF
bootloader contract.

From this directory:

```zsh
export RLVGL_WIFI_SSID='your-ssid'
printf 'Wi-Fi password: ' >&2
IFS= read -r -s RLVGL_WIFI_PASSWORD
printf '\n' >&2
export RLVGL_WIFI_PASSWORD

cargo build --release \
  --bin rlvgl-beetle-esp32c6-network-time \
  --features esp_hal_network_time
unset RLVGL_WIFI_PASSWORD

espflash flash --monitor \
  --chip esp32c6 \
  --flash-size 4mb \
  --port /dev/cu.usbmodem1433101 \
  ../../target/riscv32imac-unknown-none-elf/release/rlvgl-beetle-esp32c6-network-time
```

After the first boot reports `Seeded`, omit both credential variables when
building. Normal application flashing preserves NVS; a whole-chip erase does
not. The screen and sensor update once per displayed second while the ESP
network stack is serviced at about 100 Hz.

For an offline image-layout check, `espflash save-image --merge` must place
descriptor magic bytes `32 54 cd ab` at file offset `0x10020` (application
offset `0x20`). The linker also rejects builds whose descriptor or executable
MMU-page placement drifts from that contract.
