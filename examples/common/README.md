<!-- README.md - Shared source modules for board-specific rlvgl examples. -->

# Shared example support

`esp_network_time.rs` is the common ESP-HAL host for the portable network-time
application. Board entry points initialize their chip, select typed GPIOs, and
hand the resulting I2C and radio peripherals to this one runner. The DFR0868
ESP32-C3 and DFR1117 ESP32-C6 therefore share display, STTS22H, Wi-Fi, DHCP,
SNTP, holdover, and 1 Hz rendering behavior without pretending their MCU GPIO
numbers are the same.

The stable network policy stays in `network/`; ESP-IDF NVS partition discovery
and flash glue stay in `network/esp-nvs`. This directory is example-host glue,
not another owner of the credential record or network state machine.

The application-image linker support is chip-specific. C3 only needs the
small `app-desc-c3.x` placement fragment. The pinned beta HAL orders C6 text
before constants, so `linkall-c6.x` supplies the corrected complete ordering:
descriptor/constants first and page-aligned executable flash second. This
keeps the descriptor at application offset `0x20` and limits the bootloader to
two cache-mapped segments.
