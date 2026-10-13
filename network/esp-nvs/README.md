<!-- README.md - ESP-IDF-compatible NVS adapter for rlvgl network config. -->

# rlvgl network ESP NVS adapter

This crate maps `rlvgl_network::NetworkConfigStore` onto an initialized
`esp_nvs::Nvs` instance. The format is ESP-IDF-compatible at the flash layer,
while the stored blob uses rlvgl's versioned and checksummed portable network
record.

By default, callers provide an initialized NVS instance. RISC-V ESP board
runners can additionally enable the `esp-storage` feature and call
`open_store()` to discover the ESP-IDF data/NVS partition and bind the common
store directly to `esp-storage`. The consuming board selects the concrete chip
feature, so the partition and flash glue remains shared across ESP32-C3 and
ESP32-C6 rather than being copied into each runner.
