<!--
README.md - Usage and format notes for the rlvgl-chips-rp2040 vendor crate.
-->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-chips-rp2040
Package: `rlvgl-chips-rp2040`

Provides a board database for generic RP2040 devices used by `rlvgl-creator`.

## Usage

This crate expects board definition files extracted by [`tools/st_extract_af.py`](../../tools/st_extract_af.py). During build, set the
`RLVGL_CHIP_SRC` environment variable to the directory containing those
extracted files:

```sh
RLVGL_CHIP_SRC=build/chipdb/rp2040 cargo build -p rlvgl-chips-rp2040
```

The library exposes helper functions for consumers:

- `vendor()` – returns `"rp2040"`.
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
