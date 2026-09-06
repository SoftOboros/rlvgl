<!-- README.md - Architecture and use of the portable rlvgl network core. -->

# rlvgl network core

`rlvgl-network` contains the target-independent parts of networked rlvgl
applications. It is `no_std`, does not select a radio or network stack, and
does not know about a display bus.

The crate currently provides:

- bounded Wi-Fi credentials with password-redacting debug output;
- a versioned, checksummed configuration record and a storage trait;
- load-or-seed behavior that avoids rewriting unchanged credentials;
- explicit connection states and bounded exponential retry policy;
- validated SNTP request/response handling; and
- UTC conversion plus monotonic-clock holdover.

Platform runners implement `NetworkConfigStore` using their native storage.
For example, the Beetle ESP32-C3 and ESP32-C6 runners use the same
ESP-IDF-compatible NVS data-partition adapter, while an ESP-IDF runner can map
the policy to its native NVS API. Radio setup, DHCP/socket polling, serial
provisioning transports, and application-specific broker or device identity
remain platform or product adapters.

The sibling [`rlvgl-network-esp-nvs`](./esp-nvs/README.md) crate supplies the
reusable NVS-to-store mapping plus optional ESP flash/partition discovery.
