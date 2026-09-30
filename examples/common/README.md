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

## Wi-Fi discovery and open-network fallback

Both board hosts scan at boot and refresh the list every 60 seconds while
connected. Up to 16 access points are retained, strongest first; the serial log
includes SSID, RSSI, channel, and authentication. The OLED rotates eight seconds
of UTC/temperature with four seconds per three-entry scan page. `O` means open,
`L` means authentication required, and `?` means unknown. The page heading shows
the total found, which may exceed the retained list. `ON` identifies the current
usable network. Short synchronous scans and connection tests can pause display
updates; the normal rendering/sensor cadence remains 1 Hz.

OLED initialization allows three attempts, 25 ms apart after a failure. The
pinned ESP HAL recovers its I2C peripheral after transaction errors, allowing a
transient error after USB warm reset to be retried. Each failure is logged; a
persistent fault still exhausts the bounded budget instead of being hidden.

Stored credentials remain first choice, including hidden provisioned networks.
If absent or unusable, explicitly open, named scan results are tried in RSSI
order, pinned to the scanned BSSID/channel. Protected or unknown-authentication
networks are never tried without credentials. Discovered connections stay in
RAM and **never replace the NVS configuration**. A working connection is kept;
the app does not roam away just to test another open access point.

Each association attempt has a 20-second deadline (five attempts for the saved
network, one for each discovered open network). DHCP has a 20-second deadline.
Two Cloudflare NTP endpoints are tested with five seconds per endpoint. An AP
counts as usable only after a valid SNTP reply; DHCP alone does not establish
Internet access. Captive-portal interaction, credential guessing, and HTTP
access tests are not implemented. This is an unauthenticated time test, not
network trust verification. Open Wi-Fi provides no link-layer confidentiality;
use this fallback only where connecting is permitted.

Between candidates the host stops the radio and discards the old DHCP lease,
routes, and queued UDP packets. If all candidates fail it shows the list (and
holdover time if previously synchronized), waits 60 seconds, and scans again.
Link loss restarts saved-first selection after ten seconds. Hourly time refresh
and one-minute retry after a failed refresh are unchanged. No credentials are
needed in a rebuild once the board is provisioned, and an unprovisioned board
can now scan and use an open network without a seed.

The application-image linker support is chip-specific. C3 only needs the
small `app-desc-c3.x` placement fragment. The pinned beta HAL orders C6 text
before constants, so `linkall-c6.x` supplies the corrected complete ordering:
descriptor/constants first and page-aligned executable flash second. This
keeps the descriptor at application offset `0x20` and limits the bootloader to
two cache-mapped segments.
