<!--
README.md - Publish-facing overview for the rlvgl-chips-nxp crate.
-->

# rlvgl-chips-nxp
Package: `rlvgl-chips-nxp`

`rlvgl-chips-nxp` is the NXP i.MX RT chip and board database used by
`rlvgl-creator` and related code-generation tooling. It ships YAML chip
inventories (memory map, clock tree, CCGR gates, IOMUX pad table, daisy chain)
and board pin assignments embedded directly into the library.

## What It Provides

### Data APIs

- `chip_yaml(name)` — raw YAML source for a chip (e.g. `"mimxrt1062"`).
- `board_yaml(name)` — raw YAML source for a board (e.g. `"mimxrt1060_evkb"`).
- `chip_names()` — list of chip spec file stems currently in the database.
- `board_names()` — list of board spec file stems currently in the database.

### Compat APIs (vendor uniformity)

- `vendor()` returning the stable vendor key: `"nxp"`.
- `boards()` returning a lightweight `&[BoardInfo]` slice built from the YAML
  board specs at build time.
- `find(board_name)` for exact-name board lookup against the `BoardInfo`
  slice.

## Embedded Chipdb Structure

```
chipdb/rlvgl-chips-nxp/db/
  chips/
    mimxrt1062.yaml           # full chip inventory from MIMXRT1062 TRM
  boards/
    mimxrt1060_evkb.yaml      # MIMXRT1060-EVKB pin assignments
```

`build.rs` scans these directories at build time and embeds each file into
the library via `include_str!`. Adding a new chip or board is a data-only
change — create a new YAML file in the appropriate subdirectory and rebuild.

### Chip spec shape

Each chip YAML file documents:

- `name`, `arch`, `package`, `pac_crate` (imxrt_ral), `cpu_hz_max`
- `memory` — a list of contiguous regions with `base`, `size`, `access`
- `clock_tree` — XTAL, PLLs, CPU / AHB / IPG frequencies plus a
  `ccgr_gates` table mapping each peripheral instance to its
  clock-gate register and bit field
- `peripherals` — per-instance base address, IRQ, and signal list
- `iomux` — per-pad function table (`pad`, `mux_reg`, `pad_reg`,
  `alt0`..`alt7`, `gpio`)
- `daisy_chain` — input-select register entries for pad muxing

### Board spec shape

Each board YAML references a chip by name and supplies:

- `flash_type`, `flash_mb`, `sdram_mb`
- `pins` — pad assignments with `signal`, `peripheral`, `role`, `alt`,
  `direction`, `label`, and `pull`
- `features` — free-form map
- `console` — peripheral/baud for `println!` output
- `i2c_configs` — optional SCL frequency map

## Build-Time Data Source

`RLVGL_CHIP_SRC` is still honoured as an overlay: when set, YAML files under
`$RLVGL_CHIP_SRC/chips/` and `$RLVGL_CHIP_SRC/boards/` take precedence over
the in-tree db for matching stems. This lets downstream consumers override
or extend the chipdb without forking the crate.

## Using With rlvgl-creator

```sh
rlvgl-creator bsp from-yaml \
  --vendor nxp \
  --board mimxrt1060_evkb \
  --out gen/ \
  --emit-pac
```

Produces a PAC-style BSP module at `gen/mimxrt1060_evkb/` targeting the
`imxrt_ral` PAC crate.

## Status

MIMXRT1062 (`mimxrt1062.yaml`) is the first chip with full inventory. RT1170
data is pending — the schema is designed to scale, so adding it is a new YAML
file plus test fixtures.

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
