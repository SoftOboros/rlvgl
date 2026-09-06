<!-- README.md - Portable low-rate monochrome network-time application. -->

# Network-time application

This crate owns the target-independent 128x64 network-clock presentation. It
implements `rlvgl_core::application::Application` and draws through the rlvgl
`Renderer` trait. A platform runner updates its small copyable model with
connection progress, SNTP results, holdover status, and an optional temperature
sample.

The app does not own I2C, SSD1306, Wi-Fi, DHCP, sockets, flash, or a temperature
sensor driver. The DFR0868 Beetle ESP32-C3 and DFR1117 Beetle ESP32-C6 mount it
through one shared ESP-HAL runtime while retaining board-specific GPIO maps.
