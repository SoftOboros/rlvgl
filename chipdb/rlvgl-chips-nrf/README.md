<!--
README.md - Publish-facing overview for the rlvgl-chips-nrf crate.
-->

# rlvgl-chips-nrf
Package: `rlvgl-chips-nrf`

`rlvgl-chips-nrf` is the Nordic Semiconductor nRF chip and board database used
by `rlvgl-creator` and related code-generation tooling. It ships YAML chip
inventories (memory map, clock tree, PSEL pin routing, peripheral slot sharing,
ENABLE values) and board pin assignments embedded directly into the library.

## What It Provides

### Data APIs

- `chip_yaml(name)` — raw YAML source for a chip (e.g. `"nrf52840"`).
- `board_yaml(name)` — raw YAML source for a board (e.g. `"nrf52840_dk"`).
- `chip_names()` — list of chip spec file stems currently in the database.
- `board_names()` — list of board spec file stems currently in the database.

### Compat APIs (vendor uniformity)

- `vendor()` returning the stable vendor key: `"nrf"`.
- `boards()` returning a lightweight `&[BoardInfo]` slice built from the YAML
  board specs at build time.
- `find(board_name)` for exact-name board lookup against the `BoardInfo`
  slice.

## Embedded Chipdb Structure

```
chipdb/rlvgl-chips-nrf/db/
  chips/
    nrf52840.yaml             # full chip inventory from nRF52840 PS
  boards/
    nrf52840_dk.yaml          # nRF52840 DK pin assignments
```

`build.rs` scans these directories at build time and embeds each file into
the library via `include_str!`. Adding a new chip or board is a data-only
change — create a new YAML file in the appropriate subdirectory and rebuild.

### Chip spec shape

Each chip YAML file documents:

- `name`, `arch`, `pac_crate` (`nrf52840_pac`)
- `gpio_ports` — port and pin_count groupings
- `memory` — a list of contiguous regions with `base`, `size`, `access`
- `clock_tree` — HFCLK / LFCLK sources and configuration
- `peripheral_slots` — shared instances (e.g. TWIM0 / SPIM0 on slot 0)
  documenting mutual exclusion
- `peripherals` — per-instance configuration with `psel` roles,
  `enable_val`, and signal lists

### Board spec shape

Each board YAML references a chip by name and supplies:

- `name`, `chip`, `flash_mb`
- `console` — peripheral and baud rate
- `pins` — GPIO assignments with `port`, `pin`, `signal`, `peripheral`,
  `role`, `direction`, `label`, `pull`
- `features` — free-form map

## Build-Time Data Source

`RLVGL_CHIP_SRC` is still honoured as an overlay: when set, YAML files under
`$RLVGL_CHIP_SRC/chips/` and `$RLVGL_CHIP_SRC/boards/` take precedence over
the in-tree db for matching stems. This lets downstream consumers override
or extend the chipdb without forking the crate.

## Using With rlvgl-creator

```sh
rlvgl-creator bsp from-yaml \
  --vendor nrf \
  --board nrf52840_dk \
  --out gen/ \
  --emit-pac
```

## Status

nRF52840 (`nrf52840.yaml`) and the nRF52840 DK are the first chip and board
with full inventory. nRF5340 and nRF9160 are pending.

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
