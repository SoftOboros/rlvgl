<!--
platform/README.md - Traits and utilities for hardware and simulator integration.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-platform
Package: `rlvgl-platform`

Traits and utility types for hooking rlvgl to real hardware or simulators.

Pairs with the [core](../core/README.md) and
[widgets](../widgets/README.md) crates.

See [README-VENDOR.md](./README-VENDOR.md) for the vendor support policy.

Currently provided pieces:

- `DisplayDriver` trait for pushing pixel data to a framebuffer or LCD
- `InputDevice` trait for reading pointer or key events
- Dummy implementations used for headless testing

## stm32h747i_disco backend

The optional `stm32h747i_disco` feature enables placeholder display and touch
drivers for the STM32H747I-DISCO board's MIPI-DSI panel and FT5336 capacitive
controller. These stubs establish the module structure for future hardware
integration.
