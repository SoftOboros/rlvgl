# rlvgl-creator

A command-line tool for normalizing assets and generating dual-mode assets crates for rlvgl projects. This guide covers the end-to-end workflow from initialization to consumption.

## Workflow

1. **Initialize folders and manifest**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- init
   ```
   Creates `icons/`, `fonts/`, `media/`, and a `manifest.yml` in the working directory.

2. **Scan for new or changed assets**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- scan .
   ```
   Updates hashes in the manifest for assets under the allowed roots.

3. **Convert assets into raw sequences and font packs**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- convert
   ```
   Raster images become raw RGBA sequences, and fonts are packed into bitmap binaries and metrics.

4. **Synchronize feature flags, constants, and index**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- sync
   ```
   Regenerates manifest-driven code without touching asset bytes.

5. **Scaffold a consumer assets crate**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- scaffold assets-crate
   ```
   Generates a crate with `embed` and `vendor` features that exposes your processed assets.

6. **Vendor assets for build output**
   ```sh
   cargo run --bin rlvgl-creator --features creator,fontdue -- vendor
   ```
   Copies processed assets to `$OUT_DIR` and emits an `rlvgl_assets.rs` module for inclusion.

The resulting crate can be built with `--features embed` to include raw bytes or `--features vendor` to copy files at build time while importing the generated module.
