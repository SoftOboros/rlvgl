<!--
README-CREATOR.md - Guide to rlvgl-creator asset workflows.
-->
# rlvgl-creator

## Overview
`rlvgl-creator` assembles and transforms assets for rlvgl applications. It groups icons, fonts, and media into an asset pack and records metadata in a manifest to simplify reuse across targets.

### Terms
- **Asset pack**: A directory tree containing `icons/`, `fonts/`, `media/`, and a `manifest.yml` that tracks each resource.
- **Target**: A named output location where processed assets are written, such as the simulator `host` path or a vendor-specific board directory.
- **Manifest**: `manifest.yml`; stores hashes and conversion settings for every asset.
- **Thumbnail**: 64×64 preview image generated under `thumbs/`.

## Command-line workflows

### Initialize a new asset pack
```bash
rlvgl-creator init
rlvgl-creator add-target host vendor
```
`init` creates the asset directories and an empty manifest. `add-target` registers a `host` target whose converted assets will be placed in the `vendor` directory used by the simulator.

### Import and convert assets
Place raw files into the asset directories, then scan and convert them:
```bash
rlvgl-creator scan
rlvgl-creator convert
```
`scan` computes hashes for new or changed assets and refreshes `manifest.yml`. `convert` normalizes images to raw RGBA and prepares fonts or media for each target.

### Preview and scaffold
```bash
rlvgl-creator preview
rlvgl-creator scaffold assets-pack
```
`preview` writes thumbnails under `thumbs/` so assets can be reviewed quickly. `scaffold` generates a crate named `assets-pack` that either embeds assets or vendors them into the simulator at build time.

## UI workflows
The graphical interface mirrors the CLI steps:
- **Initialization** – Starting a new project creates the asset directories and manifest automatically, showing log messages as each folder is prepared.
- **Scanning and conversion** – A progress indicator reports hashing and transformation status. Errors surface inline so fixes can be applied immediately.
- **Previews** – Thumbnails appear in a gallery; selecting one shows metadata from the manifest.
- **Scaffolding** – When generating an assets crate, the UI lists output paths and confirms when files are written.

Throughout the UI, status bars and log panes provide feedback, ensuring each action yields visible results.

