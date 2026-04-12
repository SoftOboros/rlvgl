<!--
README.md - Publish-facing overview for the rlvgl-chips-rp2040 crate.
-->

# rlvgl-chips-rp2040
Package: `rlvgl-chips-rp2040`

`rlvgl-chips-rp2040` is the Raspberry Pi RP2040/RP2350 chip and board database
used by `rlvgl-creator` and related code-generation tooling. It ships YAML chip
inventories (memory map, clock tree, RESETS register, FUNCSEL per-GPIO table,
PIO state machines) and board pin assignments embedded directly into the library.

## What It Provides

### Data APIs

- `chip_yaml(name)` — raw YAML source for a chip (e.g. `"rp2040"`).
- `board_yaml(name)` — raw YAML source for a board (e.g. `"pico"`).
- `chip_names()` — list of chip spec file stems currently in the database.
- `board_names()` — list of board spec file stems currently in the database.

### Compat APIs (vendor uniformity)

- `vendor()` returning the stable vendor key: `"rp2040"`.
- `boards()` returning a lightweight `&[BoardInfo]` slice built from the YAML
  board specs at build time.
- `find(board_name)` for exact-name board lookup against the `BoardInfo`
  slice.

## Embedded Chipdb Structure

```
chipdb/rlvgl-chips-rp2040/db/
  chips/
    rp2040.yaml               # full chip inventory from RP2040 datasheet
  boards/
    pico.yaml                  # Raspberry Pi Pico pin assignments
```

`build.rs` scans these directories at build time and embeds each file into
the library via `include_str!`. Adding a new chip or board is a data-only
change — create a new YAML file in the appropriate subdirectory and rebuild.

### Chip spec shape

Each chip YAML file documents:

- `name`, `arch` (cortex-m0plus), `cores`, `max_freq_hz`, `pac_crate`
  (rp2040_pac), `gpio_count`
- `memory` — XIP flash region plus 6 SRAM banks with `base`, `size`, `access`
- `clock_tree` — XOSC, ROSC, PLLs (sys and usb), and `clk_domains`
  (ref, sys, peri, usb, adc, rtc) with source/divider/freq
- `resets` — one bit per peripheral for the RESETS register
- `peripherals` — per-instance base address, IRQ, and `resets_bit`
- `funcsel` — per-GPIO function table mapping each FUNCSEL index to a
  peripheral signal

### Board spec shape

Each board YAML references a chip by name and supplies:

- `flash_mb`
- `pins` — GPIO assignments with `signal`, `peripheral`, `role`,
  `direction`, and `label`
- `features` — free-form map
- `console` — peripheral/baud for `println!` output

## Build-Time Data Source

`RLVGL_CHIP_SRC` is still honoured as an overlay: when set, YAML files under
`$RLVGL_CHIP_SRC/chips/` and `$RLVGL_CHIP_SRC/boards/` take precedence over
the in-tree db for matching stems. This lets downstream consumers override
or extend the chipdb without forking the crate.

## Using With rlvgl-creator

```sh
rlvgl-creator bsp from-yaml \
  --vendor rp \
  --board pico \
  --out gen/ \
  --emit-pac
```

Produces a PAC-style BSP module at `gen/pico/` targeting the `rp2040_pac`
PAC crate.

## Status

RP2040 (`rp2040.yaml`) and Pico (`pico.yaml`) are the first chip/board pair
with full inventory. RP2350 + Pico 2 chip and board data is pending — the
schema is designed to scale, so adding them is a new YAML file plus test
fixtures.

## Features

- `std` (default): enable `serde` + `serde_yaml` + `indexmap` for YAML
  loading. Disabling this drops the YAML loader and leaves only the
  `BoardInfo` list and name lookups.

## License

MIT

## More Information

For more information, visit [softoboros.com](https://softoboros.com).

<p>
  <a href="https://softoboros.com">
    <img src="../../assets/branding/Softoboros-Letter-Logo.svg" alt="Softoboros" width="240" />
  </a>
</p>
