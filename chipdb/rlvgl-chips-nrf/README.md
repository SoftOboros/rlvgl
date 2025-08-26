<!--
README.md - Usage and format notes for the rlvgl-chips-nrf vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-nrf
Package: `rlvgl-chips-nrf`

Provides a board database for Nordic Semiconductor devices used by `rlvgl-creator`.

## Usage

This crate expects board definition files extracted by [`tools/st_extract_af.py`](../../tools/st_extract_af.py). During build, set the
`RLVGL_CHIP_SRC` environment variable to the directory containing those
extracted files:

```sh
RLVGL_CHIP_SRC=build/chipdb/nrf cargo build -p rlvgl-chips-nrf
```

The library exposes helper functions for consumers:

- `vendor()` – returns `"nrf"`.
- `boards()` – lists supported boards as `BoardInfo` entries.
- `find(name)` – looks up a board by its exact name.

`rlvgl-creator` integrates this crate to populate vendor and board drop-downs.
Other vendor crates follow the same layout and API.

## BoardInfo format

Each `BoardInfo` describes a board with at least a human-friendly board name
and associated chip. Future versions may include package information and pin
configuration offsets.

## Features

- Optional `serde` support for serialising the board database: enable the
  `serde` feature if integration with external tooling requires it.
