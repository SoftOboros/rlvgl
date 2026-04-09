<!--
CREATOR-CLI.md - Command-line reference and workflows for rlvgl-creator.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

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

### compress
Encodes an image as an RLEC binary blob for firmware use. Accepts standard image
formats (PNG, BMP, JPEG, etc.) as well as the creator's own `.raw` files
produced by the `svg` command. The output can be loaded at runtime via
`rlvgl-decomp`.

```
rlvgl-creator compress <input> <output>
```
* `input` – source image or `.raw` file.
* `output` – path for the RLEC `.rle` blob.

### decompress
Decodes an RLEC `.rle` blob back to a PNG image. Useful for inspecting
compressed assets or generating preview images for documentation.

```
rlvgl-creator decompress <input> <output>
```
* `input` – RLEC `.rle` blob file.
* `output` – output PNG file.

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

### lottie import (Linux only)
Imports a Lottie JSON animation into PNG frames and optionally an APNG using the
`rlottie` FFI library. This subcommand is only available on Linux where
`librlottie` can be installed. On macOS and other platforms use `lottie cli`
with an external converter instead.

```
rlvgl-creator lottie import <json> <out> [--apng FILE]
```
* `json` – path to the Lottie JSON file.
* `out` – directory where frames are written.
* `--apng` – optional APNG file to generate.

### lottie cli
Uses an external CLI to convert a Lottie JSON animation. Works on all platforms.

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

### bsp from-ioc
Renders Rust source from a CubeMX project using a MiniJinja template.

```
rlvgl-creator bsp from-ioc <ioc> [--emit-hal] [--emit-pac] [--template <template>]
    --out <dir> [--grouped-writes] [--one-file | --per-peripheral] [--with-deinit]
    [--allow-reserved]
```
* `ioc` – input CubeMX `.ioc` file.
* `--emit-hal` – render using the built-in HAL template.
* `--emit-pac` – render using the built-in PAC template.
* `--template` – path to a custom MiniJinja template.
* `--out` – directory to place the generated source file.
* `--grouped-writes` – collapse RCC writes by register.
  Automatically selects family-specific bus names across F0, F1, F2,
  F3, F4, F7, G0, G4, H5, H7, L0, L1, L4, L5, U5, WB, and WL families.
* `--one-file` – emit a single consolidated source file.
* `--per-peripheral` – emit one file per peripheral with feature gating.
* `--with-deinit` – include optional de-initialization helpers.
* `--allow-reserved` – permit configuration of reserved SWD pins (`PA13`, `PA14`).
  Helpers gate clocks, mask IRQs, and reset DMA/BDMA/MDMA configuration
  registers, including DMAMUX routing and stream/channel edge cases.
  Covers controllers across F0, F1, F2, F3, F4, F7, H5, H7, L0, L1, L4,
  L5, G0, G4, U5, WB, and WL variants.

See also: STM32 BSP generation behavior, flags, and roadmap in
[STM_BSP_GENERATION.md](./STM_BSP_GENERATION.md) — includes dual-core split,
power (SCUEN/VOS), and clock (PLL1/prescalers) details.

#### Advanced configuration examples

Generate HAL and PAC BSPs with grouped RCC writes, per-peripheral layout, and deinit hooks:

```bash
rlvgl-creator bsp from-ioc board.ioc \
    --emit-hal --emit-pac --grouped-writes \
    --per-peripheral --with-deinit --out bsp
```

Render a minimal PAC-only BSP in a single file for early bring-up:

```bash
rlvgl-creator bsp from-ioc bringup.ioc \
    --emit-pac --one-file --out bsp
```

Generate a HAL-only BSP with ungrouped RCC writes in a single file:

```bash
rlvgl-creator bsp from-ioc minimal.ioc \
    --emit-hal --one-file --out bsp
```

Walk through a bus-aware STM32F769I-DISCO BSP with full DMA cleanup:

1. Generate HAL and PAC code with grouped writes, per-peripheral layout, and deinit hooks:
   ```bash
   rlvgl-creator bsp from-ioc f769.ioc \\
       --emit-hal --emit-pac --grouped-writes \\
       --per-peripheral --with-deinit --out bsp
   ```
2. Call `board::deinit()` during shutdown to gate clocks, mask interrupts, and reset DMA/BDMA/MDMA state.

Walk through a bus-aware STM32H573I-DISCO BSP with ungrouped writes:

 1. Generate HAL code in a single file without grouped RCC writes:
  ```bash
  rlvgl-creator bsp from-ioc h573.ioc \\
      --emit-hal --one-file --out bsp
  ```
2. Call `board::deinit()` during shutdown to gate clocks and reset pin state.

### Edge cases and gotchas

* Peripheral clock registers vary across low-power families such as L0 and
  L1. Review generated RCC writes when targeting newly added parts.
* DMA cleanup clears DMAMUX channels and stream registers but does not yet
  handle linked-list or double-buffer modes.
* Some peripherals require extra reset steps beyond clock gating; verify
  deinit hooks for custom or rare IP blocks.

## Workflow: STM32 `.ioc` to BSP
1. Convert the `.ioc` file into intermediate representation and render a BSP crate (AFs derived from embedded vendor data):
   ```bash
   rlvgl-creator bsp from-ioc simple.ioc --emit-hal --out bsp
   ```
2. Use the generated BSP in a project:
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

## Workflow: Lucide icon extraction

Icons are sourced from the [Lucide](https://lucide.dev) icon set (ISC license)
via the `lucide-react` npm package installed in the sibling `softoboros`
frontend. A TypeScript extractor parses the JS icon modules and reconstructs
standalone SVG files, which are then rendered and compressed for firmware use.

### End-to-end pipeline

```bash
make import-icons
```

This runs three stages:

1. **Extract** – `npx tsx scripts/extract-lucide-icons.ts` reads Lucide JS
   modules and writes SVG files to `assets/icons/`.
2. **Render** – `rlvgl-creator svg` converts each SVG to RLVGLRAW format at
   96 DPI in `assets/icons/raw/`.
3. **Compress** – `rlvgl-creator compress` encodes each `.raw` file to an RLEC
   blob in `assets/icons/rle/` for runtime loading via `rlvgl-decomp`.

### Example icons

The default icon set after running `make import-icons`:

| Settings | File | Trash | Folder Open | Close |
|:--------:|:----:|:-----:|:-----------:|:-----:|
| ![settings](icons/settings_96dpi.png) | ![file](icons/file_96dpi.png) | ![trash](icons/trash_96dpi.png) | ![folder-open](icons/folder-open_96dpi.png) | ![close](icons/close_96dpi.png) |
| 414 B | 241 B | 276 B | 255 B | 188 B |

Sizes shown are RLEC blob sizes. All icons are 24x24 pixels at 96 DPI.

### Adding new icons

Edit the `ICONS` map in `scripts/extract-lucide-icons.ts` to add or remove
icons. Keys are the output filenames; values are Lucide module filenames in
`node_modules/lucide-react/dist/esm/icons/`. Browse available icons at
<https://lucide.dev/icons>.

```typescript
const DEFAULT_ICONS: Record<string, string> = {
  "settings":    "settings.js",     // gear/config
  "file":        "file.js",         // file document
  "trash":       "trash-2.js",      // delete
  "folder-open": "folder-open.js",  // open file
  "close":       "x.js",            // close/dismiss
};
```

After editing, run `make import-icons` to regenerate all stages.

### Extractor options

```
npx tsx scripts/extract-lucide-icons.ts [options]
```
* `--lucide-path <path>` – Lucide icons directory (default: `../softoboros/frontend/node_modules/lucide-react/dist/esm/icons`).
* `--out <dir>` – output directory for SVG files (default: `assets/icons`).
* `--stroke <color>` – SVG stroke color (default: `#FFFFFF`).
* `--size <px>` – icon viewport size (default: `24`).
* `--icons <list>` – comma-separated icon names to extract.

## Platform notes

### rlottie (Linux only)

The `lottie import` subcommand uses `rlottie` (C++ FFI) for in-process Lottie
rendering. This library is only available on Linux. On macOS the `rlottie`
dependency is excluded at compile time and the `lottie import` subcommand is not
present. Use `lottie cli` with an external converter on non-Linux platforms.

All other creator commands (svg, compress, fonts, apng, bsp, etc.) work on all
platforms.

## Examples of use
* **BSP** – Include the generated board crate in a firmware project and call `board::init()` to configure clocks and pin multiplexing.
* **Asset library** – Depend on the scaffolded assets crate and reference exported items such as `assets_pack::images::LOGO` when constructing widgets.
