# rlvgl-stm-bsps

Board support package stubs for STM32 boards used by `rlvgl-creator`.
This crate now includes simple modules generated from CubeMX `.ioc`
files with basic pin mappings.

Use `tools/gen_bsps.py` to convert STM32CubeMX `.ioc` files into
Rust modules within this crate:

```
python tools/gen_bsps.py --input path/to/ioc --output chips/stm/bsps/src
```
