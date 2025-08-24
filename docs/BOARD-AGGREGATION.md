# Board Aggregation Workflow

This document explains how STM32 board definition files are gathered and merged into a single `boards.json` used by the chip database.

## Data Assumptions

- Each board is provided as a standalone JSON file with a `board` field naming the board.
- `mcu.json` contains MCU pin data and is emitted separately.

## Generation Process

1. `st_ioc_board.py` converts CubeMX `.ioc` files into board overlay JSON documents with `board` and `chip` fields.
2. `tools/gen_pins.py` scans the generated JSON directory for `*.json` files, skipping any without a `chip` field.
3. All valid board files except `mcu.json` are indexed by their `board` name.
4. The resulting mapping is written to `boards.json` under a top-level `boards` object.
5. If `mcu.json` is present in the input or its parent directory, it is copied alongside `boards.json`.

The script is generic and acts as a placeholder for vendor-specific converters.

## Continuous Integration

The main CI pipeline converts sample `.ioc` files with `st_ioc_board.py`, aggregates them with `tools/gen_pins.py`, and uploads the resulting `boards.json` as an artifact. The full dataset is packed into the `chipdb.bin.zst` archive for use by the Rust crate and is not checked into the repository.
