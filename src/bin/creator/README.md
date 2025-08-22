<!--
src/bin/creator/README.md - Guide to the rlvgl-creator binary workflows.
-->
# rlgvl-creator

A combined UI and command-line tool for normalizing assets and generating dual-mode assets crates for rlvgl projects. Running without arguments launches the desktop UI; providing arguments executes the CLI. This guide covers the end-to-end workflow from initialization to consumption.

## Workflow

1. **Initialize folders and manifest**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- init
   ```
   Creates `icons/`, `fonts/`, `media/`, and a `manifest.yml` in the working directory.

2. **Scan for new or changed assets**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- scan .
   ```
   Updates hashes in the manifest for assets under the allowed roots.

3. **Convert assets into raw sequences and font packs**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- convert
   ```
   Raster images become raw RGBA sequences, and fonts are packed into bitmap binaries and metrics. Conversions run in parallel
   with stable ordering. Use `--force` to rebuild all assets regardless of cache.

   To render vector assets, the `svg` command converts an SVG into one or more raw images at chosen DPI values:
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui -- svg logo.svg out/ --dpi 96 --dpi 192
   ```
   Supply `--threshold <VAL>` to apply a monochrome cutoff suitable for e-ink displays.

4. **Synchronize feature flags, constants, and index**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- sync
   ```
   Regenerates manifest-driven code without touching asset bytes.

5. **Scaffold a consumer assets crate**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- scaffold assets-crate
   ```
   Generates a crate with `embed` and `vendor` features that exposes your processed assets.

6. **Vendor assets for build output**
   ```sh
   cargo run --bin rlgvl-creator --features creator,creator_ui,fontdue -- vendor
   ```
   Copies processed assets to `$OUT_DIR` and emits an `rlvgl_assets.rs` module for inclusion.

The resulting crate can be built with `--features embed` to include raw bytes or `--features vendor` to copy files at build time while importing the generated module.

## Developer Notes

For details on customizing scaffold templates and extending the conversion pipeline, see
[`docs/CREATOR-TEMPLATES.md`](../../../docs/CREATOR-TEMPLATES.md).
