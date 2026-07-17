# RLVGL Device Crates

This directory contains transport-agnostic hardware drivers and board
integration crates. Device drivers use the `embedded-hal` 1.0 traits and stay
independent of RLVGL rendering, operating systems, and concrete controllers.

The Kria I2C initiative is governed by
[`KI2C-00-CONCEPTS.md`](../docs/concepts/KI2C-00-CONCEPTS.md).

- `rlvgl-i2c-test-support` is an unpublished strict transaction recorder used
  by device-driver acceptance tests.
- `rlvgl-kria-i2c` owns the stable Kria logical-bus and fitted-device map,
  three-controller backend bundle, typed shared-bus leaf factories,
  allocation-free smoke diagnostics, and optional Linux mapping adapter.
- `rlvgl-device-stts22h` is the KI2C-02 temperature-sensor leaf driver.
- `rlvgl-device-veml3235sl` is the KI2C-03 ambient-light-sensor leaf driver.
- `rlvgl-device-ptn3460` is the KI2C-04 eDP-to-LVDS bridge control-plane
  leaf driver.
- `rlvgl-device-pcm3168a` is the KI2C-05 audio-codec control-plane leaf
  driver.
