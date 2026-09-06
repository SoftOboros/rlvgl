<!-- README.md - Bench runbook for the DFR0868 Beetle ESP32-C3 examples. -->

# Beetle ESP32-C3 + SSD1306 + STTS22H

This package contains two `esp-hal` display binaries for the DFRobot DFR0868
Beetle ESP32-C3:

- `rlvgl-beetle-esp32c3` is the original local display/activity proof.
- `rlvgl-beetle-esp32c3-network-time` joins Wi-Fi, obtains an address with
  DHCP, requests UTC from Cloudflare's published NTP anycast endpoints, and
  shows the clock through the rlvgl SSD1306 display adapter.

The second binary is a board runner, not the application itself. Its reusable
pieces live in the rlvgl workspace:

- [`rlvgl-app-network-time`](../apps/network-time/README.md) owns the portable
  128x64 monochrome view and application model;
- [`rlvgl-network`](../../network/README.md) owns credential validation,
  versioned configuration, connection/retry policy, SNTP, UTC, and holdover;
- [`rlvgl-network-esp-nvs`](../../network/esp-nvs/README.md) maps the portable
  store contract to ESP-IDF-compatible NVS; and
- [`rlvgl-device-stts22h`](../../devices/rlvgl-device-stts22h/src/lib.rs) owns
  the shared temperature-sensor driver.

This package retains only ESP32-C3 initialization and typed GPIO selection.
The source in [`../common/esp_network_time.rs`](../common/esp_network_time.rs)
is shared with the DFR1117 C6 host and owns the I2C devices, `esp-wifi`, and
smoltcp lifecycle. ESP flash partition discovery now lives alongside the NVS
adapter in `network/esp-nvs`.

The network stack continues to poll at about 100 Hz, but the network-time binary
flushes the I2C panel only when the displayed second changes. Its steady-state
display cadence is therefore 1 Hz rather than the original demo's 20 Hz.

## DFR0650 wiring

Use the module in its default I2C mode. Power it from the Beetle's 3.3 V rail.

| DFR0650 | Beetle ESP32-C3 |
| --- | --- |
| VCC | 3V3 |
| GND | GND |
| SDA | GPIO8/SDA |
| SCL | GPIO9/SCL |

The verified GPIO8/GPIO9 bus runs at 100 kHz and initializes the DFR0650 at its
factory-default `0x3c` address. The D/C and CS pins are not connected for the
module's default I2C interface.

## STTS22H wiring

The full-size SparkFun STTS22H board shares the same four-wire bus. Its factory
address is `0x3c`, which conflicts with the DFR0650, so bridge the `ADDR1`
center pad toward the pad marked `0x38`. Because this bench already has 4.7 kΩ
pull-ups, cut the SparkFun board's `I2C_PU` trace to remove its additional
2.2 kΩ pair.

| SparkFun STTS22H | Beetle ESP32-C3 |
| --- | --- |
| 3V3 | 3V3 |
| GND | GND |
| SDA | GPIO8/SDA |
| SCL | GPIO9/SCL |

At startup the shared STTS22H driver reads `WHO_AM_I` at `0x38`, requires the
documented identity, and selects the sensor's 1 Hz low-ODR mode. The display and
sensor use a shared single-threaded I2C bus. Temperature and the OLED are both
sampled or refreshed once per displayed second.

## Provision once, then build without credentials

The first firmware can take credentials as build-time provisioning seeds. At
boot, the runner discovers the ESP-IDF data/NVS partition and writes a
versioned record under namespace `rlvgl_net`, key `config_v1`. The password is
never printed, debug formatting redacts it, and unchanged seeds do not rewrite
flash. This bench storage is unencrypted, matching the present P4 bench policy.

Seed and flash once from this directory:

```zsh
export RLVGL_WIFI_SSID='your-ssid'
printf 'Wi-Fi password: ' >&2
read -r -s RLVGL_WIFI_PASSWORD
printf '\n' >&2
export RLVGL_WIFI_PASSWORD

cargo build \
  --release \
  --bin rlvgl-beetle-esp32c3-network-time \
  --features esp_hal_network_time
unset RLVGL_WIFI_PASSWORD

espflash flash \
  --monitor \
  --port /dev/cu.usbmodem1433101 \
  ../../target/riscv32imc-unknown-none-elf/release/rlvgl-beetle-esp32c3-network-time
```

After that boot has reported `Seeded`, build and flash normally with neither
credential in the environment:

```sh
cargo build \
  --release \
  --bin rlvgl-beetle-esp32c3-network-time \
  --features esp_hal_network_time

espflash flash \
  --monitor \
  --port /dev/cu.usbmodem1433101 \
  ../../target/riscv32imc-unknown-none-elf/release/rlvgl-beetle-esp32c3-network-time
```

Normal application flashing preserves the NVS partition. An explicit whole
chip erase removes the stored configuration. If no valid stored record and no
seed are available, the app parks on `Wi-Fi not set`. Supplying a different
seed advances the stored generation and replaces the credentials. An empty
password is allowed for an open bench network.

The P4 `@CCPSCFG/1` USB protocol remains with the CCPS product because it also
carries device identity and MQTT configuration. This first common slice shares
the storage and connection policy; a future transport-neutral provisioning
adapter can write the same `NetworkConfigStore` without changing this app.

The screen progresses through radio, Wi-Fi, DHCP, and SNTP states. Once
synchronized it shows Gregorian UTC and `SYNC age`; if Wi-Fi drops, the clock
continues from the ESP32-C3 monotonic timer and marks the display `HOLD` while
requesting reconnection. It resynchronizes hourly and retries a failed refresh
after one minute.

## Verification

Run the common network, NVS adapter, portable app, and board checks from the
rlvgl workspace root:

```sh
cargo test \
  -p rlvgl-network \
  -p rlvgl-network-esp-nvs \
  -p rlvgl-app-network-time \
  -p rlvgl-device-stts22h
cargo check -p rlvgl-example-beetle-esp32c3 \
  --features esp_hal \
  --target riscv32imc-unknown-none-elf
cargo check -p rlvgl-example-beetle-esp32c3 \
  --bin rlvgl-beetle-esp32c3-network-time \
  --features esp_hal_network_time \
  --target riscv32imc-unknown-none-elf
```

Successful compilation is static evidence only. A complete bench result also
requires observing the wired DFR0650, a DHCP lease, a valid SNTP response, an
STTS22H identity response at `0x38`, a plausible temperature, and the one-second
display cadence on the target board.
