<!--
OPTIONS.md - Cargo feature reference for the generated disco-assets crate.
-->
# disco-assets Options

`disco-assets` is a generated asset crate produced by `rlvgl-creator`.

## Default configuration

- Default features: none.
- Runtime model: `no_std` by default.

## Feature flags

| Feature | Effect | Target / std notes | Performance / size notes |
| --- | --- | --- | --- |
| `embed` | Embeds all generated asset bytes directly into the binary and exposes them through `disco_assets::embed`. | `no_std`-friendly. | Increases final binary size in proportion to the embedded assets, but eliminates runtime file I/O. |
| `vendor` | Enables build-time copy helpers that stage asset files into `OUT_DIR` and generate `rlvgl_assets.rs`. | Uses `std` inside the crate for the vendor helpers. | Keeps firmware or app binaries smaller than `embed`, but shifts the cost to build-time file copying and deployment of external asset files. |
