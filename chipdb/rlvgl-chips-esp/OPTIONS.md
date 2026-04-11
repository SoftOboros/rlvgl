<!--
OPTIONS.md - Cargo feature reference for the rlvgl-chips-esp crate.
-->
# rlvgl-chips-esp Options

`rlvgl-chips-esp` packages Espressif chip inventories and board metadata
for `rlvgl-creator` BSP generation workflows.

## Default configuration

- Default features: `std`.
- Runtime model: `no_std` compatible when `std` is disabled (only the name
  list and `BoardInfo` slice are available in that mode).

## Feature flags

| Flag  | Default | Effect                                                                 |
|-------|---------|------------------------------------------------------------------------|
| `std` | on      | Pulls in `serde`, `serde_yaml`, and `indexmap` so consumers can parse the embedded chip/board YAML through [`chip_yaml`] / [`board_yaml`]. Disabling drops the YAML loader; only `vendor()`, `boards() -> &[BoardInfo]`, `find()`, `chip_names()`, and `board_names()` remain callable. |

## Useful notes

- Chip and board YAML files live in `db/chips/` and `db/boards/` inside the
  crate. Adding a new entry is a data-only change — no Rust edits required.
- `RLVGL_CHIP_SRC` is still honoured as an overlay: files under
  `$RLVGL_CHIP_SRC/chips/` and `$RLVGL_CHIP_SRC/boards/` override the
  in-tree db for matching file stems.
- ESP32-C3 (`esp32c3.yaml`) is the first chip with a full inventory sourced
  from the ESP32-C3 Technical Reference Manual v1.4. Additional chips are
  expected to follow the same schema.
