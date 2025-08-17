# rlvgl-creator – UI Full Functionality TODO

This file tracks the remaining work to bring `rlvgl-creator`'s desktop UI up to parity with its CLI and provide complete asset management.

## Command Surface
- [x] Add a global command menu listing all CLI actions with dedicated handlers and toast feedback.
- [x] Expose `init` command via dialog to create asset roots and default manifest.
- [x] Add `scan` action with directory picker and manifest refresh.
- [x] Add `check` command with root selector and optional fix toggle.
- [x] Implement `vendor` operation UI for copying assets and generating embed modules.
- [x] Expose `convert` command with root chooser and force flag.
- [x] Add `preview` command to regenerate thumbnails on demand.
- [x] Provide `add-target` registration dialog for name and vendor directory.
- [x] Expose `sync` command with output directory and dry-run option.
- [x] Implement `scaffold` UI to generate a dual-mode assets crate.

## Conversion & Export Tools
- [x] Expand APNG builder to allow setting delay and loop count; frames directory,
      output path, delay, and loops are configurable.
- [x] Add manifest schema export option running `schema::run()`.
- [x] Expose font packer UI for size and character set; root path,
      size, and glyphs are configurable.
- [x] Integrate Lottie importer (in-process and external CLI paths).
 - [x] Add SVG renderer dialog with configurable DPI list and threshold; both settings are user-configurable before rendering.

## Asset Browser
- [x] Replace flat list with hierarchical tree reflecting `assets/raw`; directories mirror the on-disk hierarchy.
- [x] Add "Add Asset" action using a file dialog to copy files and update manifest
      (no import workflow yet).
- [x] Allow deletion of selected assets with confirmation dialog and manifest persistence.
- [x] Display full archive contents with automatic refresh when files are added externally.
 
## Workflow & UX Enhancements
- [x] Group related commands into top-level menus (Assets, Build, Deploy) to replace one-button-per-command clutter.
  - **Assets**: init, scan, check, vendor, convert, preview.
  - **Build**: add-target, scaffold, schema export, font pack, SVG render.
  - **Deploy**: sync, automation presets.
- [x] Introduce wizards that walk through common sequences like scan → convert → preview with progress indication.
  - Wizard steps: select root → scan assets → convert formats → preview results → summary.
- [x] Support automation presets or macros to chain commands and replay frequent workflows.
  - Allow saving command sequences as named presets in a JSON file and expose a "Run Preset" dialog.

