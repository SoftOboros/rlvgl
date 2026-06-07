<!--
README.md - Publish-facing overview for the rlvgl-chips-renesas crate.
-->

# rlvgl-chips-renesas
Package: `rlvgl-chips-renesas`

`rlvgl-chips-renesas` is the Renesas RA chip and board database used by
`rlvgl-creator` and related code-generation tooling. It ships YAML chip
inventories (memory map, clock tree, MSTP gates, PFS pin function table, SCI
multiplexing) and board pin assignments embedded directly into the library.

## What It Provides

### Data APIs

- `chip_yaml(name)` — raw YAML source for a chip (e.g. `"r7fa6m5bh"`).
- `board_yaml(name)` — raw YAML source for a board (e.g. `"ek_ra6m5"`).
- `chip_names()` — list of chip spec file stems currently in the database.
- `board_names()` — list of board spec file stems currently in the database.

### Compat APIs (vendor uniformity)

- `vendor()` returning the stable vendor key: `"renesas"`.
- `boards()` returning a lightweight `&[BoardInfo]` slice built from the YAML
  board specs at build time.
- `find(board_name)` for exact-name board lookup against the `BoardInfo`
  slice.

## Embedded Chipdb Structure

```
chipdb/rlvgl-chips-renesas/db/
  chips/
    r7fa6m5bh.yaml           # full chip inventory from RA6M5 hardware manual
  boards/
    ek_ra6m5.yaml             # EK-RA6M5 board pin assignments
```

`build.rs` scans these directories at build time and embeds each file into
the library via `include_str!`. Adding a new chip or board is a data-only
change — create a new YAML file in the appropriate subdirectory and rebuild.

### Chip spec shape

Each chip YAML file documents:

- `name`, `arch` (cortex-m33), `core_features`, `cpu_hz`
- `pac_crate` — null (no community PAC for RA6M5 yet)
- `fsp_version` — FSP version the spec was derived from
- `memory` — a list of contiguous regions with `base`, `size`, `access`
- `gpio_ports` — port/pin groupings
- `clock_tree` — HOCO / MOCO / LOCO / main_osc / PLL sources, derived
  PCLKA-D bus clocks, and an `mstp_gates` table mapping each peripheral
  instance to its module-stop clock-gate register and bit
- `peripherals` — SCI (with modes: UART, SPI, I2C), IIC, SPI instances
- `pfs_table` — per-pin PSEL-to-signal mapping for the Pin Function Select
  register

### Board spec shape

Each board YAML references a chip by name and supplies:

- `name`, `chip`, `flash_mb`
- `console` — peripheral and baud rate, with `sci_mode`
- `pins` — GPIO assignments with `port`, `pin`, `signal`, `peripheral`,
  `role`, `direction`, `psel`, `label`, `pull`
- `sci_modes` — per-SCI channel mode selection
- `features` — free-form map

## Build-Time Data Source

`RLVGL_CHIP_SRC` is still honoured as an overlay: when set, YAML files under
`$RLVGL_CHIP_SRC/chips/` and `$RLVGL_CHIP_SRC/boards/` take precedence over
the in-tree db for matching stems. This lets downstream consumers override
or extend the chipdb without forking the crate.

## Using With rlvgl-creator

```sh
rlvgl-creator bsp from-yaml \
  --vendor renesas \
  --board ek_ra6m5 \
  --out gen/ \
  --emit-pac
```

## Status

R7FA6M5BH (RA6M5) is the first chip with full inventory. RA8D1 with
Dave2D GPU and GLCDC display controller is pending.

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
