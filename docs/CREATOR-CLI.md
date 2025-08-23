<!--
CREATOR-CLI.md - Command-line reference and workflows for rlvgl-creator.
-->
# rlvgl-creator CLI

## Overview
`rlvgl-creator` is a command-line utility that prepares assets and board support packages (BSPs) for `rlvgl` projects. It converts raw files into formats suitable for embedded targets, manages manifests that track asset metadata, and can translate vendor configuration files into Rust source through template rendering.

A typical workflow initializes an asset pack, imports resources, converts them for a target, and scaffolds a crate that exposes the assets at build time. Hardware workflows parse vendor files like STM32CubeMX `.ioc` descriptions and render BSP code using MiniJinja templates.

## Quick start workflow
```bash
rlvgl-creator init
rlvgl-creator add-target host vendor
rlvgl-creator scan assets/
rlvgl-creator convert assets/
rlvgl-creator preview assets/
rlvgl-creator scaffold assets-pack
```
This sequence creates a new asset pack, registers a `host` target whose converted assets are written under `vendor/`, scans the raw asset directories, converts the assets into normalized forms, generates thumbnails for quick review, and finally scaffolds a dual-mode crate named `assets-pack` for embedding or vendoring resources.

## Command reference
### init
Initializes asset directories (`icons/`, `fonts/`, `media/`) and writes an empty `manifest.yml`.

```
rlvgl-creator init
```

### scan
Scans a directory tree for assets, computes hashes, and updates the manifest.

```
rlvgl-creator scan <path>
```
* `path` – root directory containing raw assets.

### check
Validates manifest entries against asset files.

```
rlvgl-creator check <path> [--fix]
```
* `path` – root directory containing assets.
* `--fix` – write corrections to the manifest when discrepancies are found.

### vendor
Copies processed assets into an output directory and emits an `rlvgl_assets.rs` helper module.

```
rlvgl-creator vendor <path> <out> [--allow LICENSE] [--deny LICENSE]
```
* `path` – root directory containing assets.
* `out` – directory where vendored assets are written.
* `--allow` – whitelist of permitted licenses.
* `--deny` – blacklist of disallowed licenses.

### convert
Normalizes assets (fonts, images, media) and refreshes manifest metadata.

```
rlvgl-creator convert <path> [--force]
```
* `path` – root directory containing assets.
* `--force` – rebuild all assets even if cached outputs exist.

### preview
Generates thumbnails under `thumbs/` for quick visual inspection.

```
rlvgl-creator preview <path>
```
* `path` – root directory containing assets.

### add-target
Registers a named target and the directory where its vendored assets will be placed.

```
rlvgl-creator add-target <name> <vendor_dir>
```
* `name` – identifier used in `manifest.yml`.
* `vendor_dir` – path where converted assets are vendored.

### sync
Regenerates Cargo feature lists and an asset index from the manifest.

```
rlvgl-creator sync <out> [--dry-run]
```
* `out` – directory to write generated files.
* `--dry-run` – print changes without writing to disk.

### scaffold
Creates a dual-mode assets crate that can either embed resources or vendor them at build time.

```
rlvgl-creator scaffold <path>
```
* `path` – destination directory for the generated crate.

### apng
Builds an animated PNG from a sequence of frames.

```
rlvgl-creator apng <frames> <out> [--delay MS] [--loops N]
```
* `frames` – directory containing sequential PNG frames.
* `out` – output APNG file.
* `--delay` – frame delay in milliseconds (default 100).
* `--loops` – loop count (`0` for infinite).

### schema
Prints the JSON schema for `manifest.yml` to stdout.

```
rlvgl-creator schema
```

### fonts pack
Rasterizes TTF/OTF fonts into bitmap data and metrics files.

```
rlvgl-creator fonts pack <path> [--size PX] [--chars STRING]
```
* `path` – directory containing font files.
* `--size` – point size for rasterization (default `32`).
* `--chars` – string of characters to include in the pack.

### lottie import
Imports a Lottie JSON animation into PNG frames and optionally an APNG.

```
rlvgl-creator lottie import <json> <out> [--apng FILE]
```
* `json` – path to the Lottie JSON file.
* `out` – directory where frames are written.
* `--apng` – optional APNG file to generate.

### lottie cli
Uses an external CLI to convert a Lottie JSON animation.

```
rlvgl-creator lottie cli [--bin PATH] <json> <out> [--apng FILE]
```
* `--bin` – external binary (default `lottie-cli`).
* `json` – path to the Lottie JSON file.
* `out` – directory where frames are written.
* `--apng` – optional APNG file to generate.

### svg
Renders an SVG into raw image files.

```
rlvgl-creator svg <svg> <out> [--dpi DPI...] [--threshold VAL]
```
* `svg` – path to the SVG file.
* `out` – directory where raw images are written.
* `--dpi` – one or more DPI values to render at (default `96`).
* `--threshold` – monochrome threshold (0–255).

### board from-ioc
Converts a CubeMX project into a board overlay JSON.

```
rlvgl-creator board from-ioc <ioc> <board> <out>
```
* `ioc` – path to the CubeMX `.ioc` file.
* `board` – name to embed in the overlay.
* `out` – path to write the generated JSON.

## Workflow: STM32 `.ioc` to BSP
1. Generate an alternate-function database:
   ```bash
   python3 tools/afdb/st_extract_af.py --db tests/fixtures/stm32_af.csv --out stm32_af.json
   ```
2. Convert the `.ioc` file into intermediate representation and render a BSP crate:
   ```bash
   rlvgl-creator bsp from-ioc simple.ioc stm32_af.json --template src/bin/creator/bsp/templates/simple.rs.jinja --out bsp
   ```
   The command parses pin assignments and clock configuration from `simple.ioc`, maps signals using `stm32_af.json`, and renders Rust source into the `bsp/` directory.
3. Use the generated BSP in a project:
   ```rust
   // Cargo.toml
   // [dependencies]
   // board = { path = "bsp" }

   // main.rs
   board::init();
   ```

## Workflow: create and finalize an asset library
1. Initialize a new pack and register a `host` target:
   ```bash
   rlvgl-creator init
   rlvgl-creator add-target host vendor
   ```
2. Add raw assets:
   * Place image files under `icons/` or `media/`.
   * Copy fonts (`.ttf`, `.otf`) into `fonts/`.
3. Scan and convert the assets:
   ```bash
   rlvgl-creator scan assets/
   rlvgl-creator convert assets/
   ```
4. Generate previews and synchronize feature lists:
   ```bash
   rlvgl-creator preview assets/
   rlvgl-creator sync vendor
   ```
5. Scaffold a crate exposing the assets:
   ```bash
   rlvgl-creator scaffold assets-pack
   ```
6. Use the asset crate:
   ```rust
   // Cargo.toml
   // [dependencies]
   // assets_pack = { path = "assets-pack" }

   // main.rs
   use assets_pack::fonts::PRIMARY_FONT;
   use assets_pack::images::LOGO;
   ```
   The crate provides strongly typed accessors for fonts and graphics that can be embedded or vendored depending on build features.

## Examples of use
* **BSP** – Include the generated board crate in a firmware project and call `board::init()` to configure clocks and pin multiplexing.
* **Asset library** – Depend on the scaffolded assets crate and reference exported items such as `assets_pack::images::LOGO` when constructing widgets.
