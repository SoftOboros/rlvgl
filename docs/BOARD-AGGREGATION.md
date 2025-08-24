# Board Aggregation Workflow

This document explains how STM32 board definition files are gathered and merged into a single `boards.json` used by the chip database.

## Data Assumptions

- Each board is provided as a standalone JSON file with a `board` field naming the board.
- `mcu.json` contains MCU pin data and is emitted separately.

## Generation Process

1. `tools/gen_pins.py` scans an input directory for `*.json` files.
2. All board files except `mcu.json` are indexed by their `board` name.
3. The resulting mapping is written to `boards.json` under a top-level `boards` object.
4. If `mcu.json` is present in the input or its parent directory, it is copied alongside `boards.json`.

The script is generic and acts as a placeholder for vendor-specific converters.

## Continuous Integration

The main CI pipeline runs `tools/gen_pins.py` against sample data and uploads the resulting `boards.json` as an artifact, ensuring the aggregation step stays functional.
