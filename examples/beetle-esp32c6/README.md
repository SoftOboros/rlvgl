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

The OLED now rotates clock/temperature with nearby Wi-Fi scan pages. Saved
credentials remain first choice; if unavailable, the shared host tries only
explicitly open networks without writing them to NVS. See
[shared example support](../common/README.md#wi-fi-discovery-and-open-network-fallback)
for the display legend, retry limits, and captive-portal/security limitations.

For an already provisioned C6, build without including either seed:

```zsh
unset RLVGL_WIFI_SSID RLVGL_WIFI_PASSWORD
cargo build --release \
  --bin rlvgl-beetle-esp32c6-network-time \
  --features esp_hal_network_time
```

For an offline image-layout check, `espflash save-image --merge` must place
descriptor magic bytes `32 54 cd ab` at file offset `0x10020` (application
offset `0x20`). The linker also rejects builds whose descriptor or executable
MMU-page placement drifts from that contract.

## Saved-network bench regression: 2026-09-30

The DFR1117 ESP32-C6 revision 0.2 with 4 MB flash, DFR0650 OLED, and STTS22H
at `0x38` passed two consecutive application flashes of the final image. Both
build-time credential variables were omitted. The second flash used
`--no-skip` to force an image rewrite; neither flash erased the NVS partition.

Both final boots reported configuration origin `Stored`, generation `1`, then
associated with the preexisting WPA2 network on attempt one, acquired a DHCP
lease, and accepted a stratum-3 SNTP response. Round trips were 41 ms and 42 ms.
STTS22H probing and low-ODR configuration succeeded. Background scans completed
without a reported link loss during the observation window.

An earlier run hit a first-error OLED initialization panic following the
monitor's extra USB reset; a subsequent controlled reset recovered the board.
The shared host now permits three logged initialization attempts, with 25 ms
between failures. The two final flash runs completed without an initialization
failure. This is a bounded mitigation, not exhaustive warm-reset qualification.

Tested C6 ELF SHA-256:

```text
968dec52cf89865fdd167023ddbafb9f34660346916fe81c46a06558fa4bf2ed
```

All observed scan results required authentication, so open-network fallback and
captive-portal behavior were not exercised on hardware. Automatic selection
rules, credential persistence, and portable rendering passed 18 host tests;
C3 and C6 release builds and Clippy checks passed. OLED visual confirmation and
a hardware open-network test remain separate acceptance checks.
