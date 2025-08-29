<!--
README-CREATOR.md - Guide to rlvgl-creator 🆕 asset workflows.
-->
<p align="center">
  <img src="./rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator 🆕

Package: `rlvgl-creator` 🆕.

## Overview
`rlvgl-creator` 🆕 assembles and transforms assets for rlvgl applications. It groups icons, fonts, and media into an asset pack and records metadata in a manifest to simplify reuse across targets. It can also generate Rust BSP code from STM32CubeMX `.ioc` files.

### Terms
- **Asset pack**: A directory tree containing `icons/`, `fonts/`, `media/`, and a `manifest.yml` that tracks each resource.
- **Target**: A named output location where processed assets are written, such as the simulator `host` path or a vendor-specific board directory.
- **Manifest**: `manifest.yml`; stores hashes and conversion settings for every asset.
- **Thumbnail**: 64×64 preview image generated under `thumbs/`.

## Command-line workflows

### Initialize a new asset pack
```bash
# rlvgl-creator 🆕
rlvgl-creator init # 🆕
rlvgl-creator add-target host vendor # 🆕
```
`init` creates the asset directories and an empty manifest. `add-target` registers a `host` target whose converted assets will be placed in the `vendor` directory used by the simulator.

### Import and convert assets
Place raw files into the asset directories, then scan and convert them:
```bash
# rlvgl-creator 🆕
rlvgl-creator scan # 🆕
rlvgl-creator convert # 🆕
```
`scan` computes hashes for new or changed assets and refreshes `manifest.yml`. `convert` normalizes images to raw RGBA and prepares fonts or media for each target.

### Preview and scaffold
```bash
# rlvgl-creator 🆕
rlvgl-creator preview # 🆕
rlvgl-creator scaffold assets-pack # 🆕
```
`preview` writes thumbnails under `thumbs/` so assets can be reviewed quickly. `scaffold` generates a crate named `assets-pack` that either embeds assets or vendors them into the simulator at build time.

## UI workflows
The graphical interface mirrors the CLI steps:
- **Initialization** – Starting a new project creates the asset directories and manifest automatically, showing log messages as each folder is prepared.
- **Scanning and conversion** – A progress indicator reports hashing and transformation status. Errors surface inline so fixes can be applied immediately.
- **Previews** – Thumbnails appear in a gallery; selecting one shows metadata from the manifest.
- **Scaffolding** – When generating an assets crate, the UI lists output paths and confirms when files are written.

Throughout the UI, status bars and log panes provide feedback, ensuring each action yields visible results.

For detailed CLI and UI flags see [src/bin/creator/README.md](./src/bin/creator/README.md).

## Template notes

The creator's board and asset generators rely on [MiniJinja](https://github.com/mitsuhiko/minijinja),
which does not implement Python-style `dict.get` methods. When accessing optional keys in a mapping,
use bracket notation combined with the `default` filter instead. For example:

```
{%- for irq in (irq_map[name] | default([])) %}
    ...
{%- endfor %}
```

This pattern safely expands to an empty list when `name` is absent.

## Chip and board database integration

`rlvgl-creator` 🆕 consumes chip and board definitions from the `rlvgl-chips-*` crates under
`chipdb/`. These crates embed vendor JSON data so the CLI and UI can populate vendor,
microcontroller and board selections. When regenerating pin data, bump the crate versions
before publishing:

```bash
python tools/bump_vendor_versions.py --path chipdb
```

No additional configuration is required; the creator automatically loads all available
vendor crates on startup.

> ⚠️ The legacy `board` subcommand remains but is deprecated in favor of BSP generation.

To convert a custom CubeMX project into a board overlay, run:

```bash
# rlvgl-creator 🆕
rlvgl-creator board from-ioc project.ioc MyBoard MyBoard.json # 🆕
```

The CLI detects the MCU automatically and resolves alternate-function numbers
using the bundled database. The resulting JSON can be placed under `boards/`
for use by `rlvgl-creator` 🆕.

## Batch BSP generation

Run `scripts/gen_ioc_bsps.sh` to convert every CubeMX `.ioc` under
`chips/stm/STM32_open_pin_data/boards`. The script invokes
`rlvgl-creator` 🆕 for each file and relies on the `rlvgl-chips-stm`
archive for MCU metadata, so no standalone `mcu.json` is required.

Generated modules are published as [`rlvgl-bsps-stm` 🆕](./chips/stm/bsps/README.md).
Include a module in your project:

```rust
use rlvgl_bsps_stm::f407_demo as bsp;
```

## BSP generation from CubeMX

Generate PAC or HAL code from a CubeMX `.ioc` file using the bundled AF database:

```bash
rlvgl-creator bsp from-ioc board.ioc \
  --out bsp \
  --emit-pac \
  --grouped-writes \
  --with-deinit
```

### Using GPIO labels from `.ioc`

CubeMX projects often assign `GPIO_Label` to pins (e.g., `PA9.GPIO_Label=STLINK_RX`). `rlvgl-creator` can propagate these through to the BSP IR and templates.

- Add labels to comments (default): enabled automatically in both PAC and HAL templates.
- Use labels as identifiers (HAL only):

```bash
rlvgl-creator bsp from-ioc board.ioc \
  --out bsp --emit-hal \
  --use-label-names \
  --label-prefix pin_ \
  --fail-on-duplicate-labels
```

This sanitizes labels into snake_case (prefixing identifiers that start with digits/underscores, and avoiding Rust keywords) and uses them as local variable names in the HAL template. Duplicate labels after sanitization can be rejected (`--fail-on-duplicate-labels`) or deduplicated with numeric suffixes.

- Emit label constants (PAC):

```bash
rlvgl-creator bsp from-ioc board.ioc \
  --out bsp --emit-pac \
  --emit-label-consts
```

This adds a `pins` module with constants like `pub const STLINK_RX: PinLabel = PinLabel { pin: "PA9", func: "USART1_TX", af: 7 };` to make it easy to reference labeled pins from application code.
