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
- [ ] Expand APNG builder to choose frames directory, delay, and loop count.
- [x] Add manifest schema export option running `schema::run()`.
- [ ] Expose font packer UI for root path, size, and character set.
- [x] Integrate Lottie importer (in-process and external CLI paths).
- [ ] Add SVG renderer dialog with DPI list and threshold configuration.

## Asset Browser
- [ ] Replace flat list with hierarchical tree reflecting `assets/raw`.
- [ ] Add "Add Asset" action using a file dialog to copy files and update manifest.
- [x] Allow deletion of selected assets with confirmation dialog and manifest persistence.
- [ ] Display full archive contents and refresh view after add/delete operations.

