<!--
README.md - Publish-facing overview for the rlvgl-chips-esp crate.
-->

# rlvgl-chips-esp
Package: `rlvgl-chips-esp`

`rlvgl-chips-esp` is the Espressif chip and board database used by
`rlvgl-creator` and related code-generation tooling. It ships YAML chip
inventories (memory map, clock tree, peripheral gates, IO MUX, GPIO matrix)
and board pin assignments embedded directly into the library.

## What It Provides

### Data APIs

- `chip_yaml(name)` — raw YAML source for a chip (e.g. `"esp32c3"`).
- `board_yaml(name)` — raw YAML source for a board (e.g. `"esp32c3_devkitm_1"`).
- `chip_names()` — list of chip spec file stems currently in the database.
- `board_names()` — list of board spec file stems currently in the database.

### Compat APIs (vendor uniformity)

- `vendor()` returning the stable vendor key: `"esp"`.
- `boards()` returning a lightweight `&[BoardInfo]` slice built from the YAML
  board specs at build time.
- `find(board_name)` for exact-name board lookup against the `BoardInfo`
  slice.

## Embedded Chipdb Structure

```
chipdb/rlvgl-chips-esp/db/
  chips/
    esp32c3.yaml          # full chip inventory from ESP32-C3 TRM v1.4
  boards/
    esp32c3_devkitm_1.yaml  # ESP32-C3-DevKitM-1 pin assignments
```

`build.rs` scans these directories at build time and embeds each file into
the library via `include_str!`. Adding a new chip or board is a data-only
change — create a new YAML file in the appropriate subdirectory and rebuild.

### Chip spec shape

Each chip YAML file documents:

- `name`, `arch`, `package`, `pac_crate`, `gpio_count`
- `memory` — a list of contiguous regions with `base`, `size`, `access`
- `clock_tree` — XTAL / PLL / CPU / APB / RTC frequencies plus a
  `system_gates` table mapping each peripheral instance to its
  clock-enable and reset register paths
- `peripherals` — per-instance base address, IRQ, and signal list
  (role, direction, GPIO matrix id, and/or direct IO MUX fast-path pin)
- `io_mux` — per-pin function table (F0..F3 on C3) with strapping and
  flash-reserved flags
- `gpio_matrix` — signal-id ↔ name table for GPIO matrix routing

### Board spec shape

Each board YAML references a chip by name and supplies:

- `flash_mb`, `psram_mb`, optional `module` identifier
- `pins` — GPIO assignments with optional `peripheral`, `direction`,
  `pull`, `drive`, and `label`
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
  --vendor esp \
  --board esp32c3_devkitm_1 \
  --out gen/ \
  --emit-pac
```

Produces a PAC-style BSP module at `gen/esp32_c3_dev_kit_m_1/` targeting the
`esp32c3` PAC crate.

## Status

ESP32-C3 (`esp32c3.yaml`) is the first chip with full inventory. ESP32-C6,
S3, H2, and P4 all have comprehensive datasheets in the memalpha corpus the
data was sourced from, so the schema is designed to scale — adding any of
them is a new YAML file plus test fixtures.

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
