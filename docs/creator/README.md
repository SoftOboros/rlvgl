# rlvgl-creator

Design docs for the `rlvgl-creator` binary — asset pipeline, BSP generator,
desktop UI, workspace scaffolding.

## Documents

- [CLI.md](./CLI.md) — command-line reference and workflows.
- [TEMPLATES.md](./TEMPLATES.md) — MiniJinja template guidelines for BSP generation.
- [ASSET-PIPELINE.md](./ASSET-PIPELINE.md) — asset manifests, packing, and dual-mode crates.
- [BSP-STATUS.md](./BSP-STATUS.md) — BSP generator status across all vendors.
- [UI-DESIGN.md](./UI-DESIGN.md) — desktop UI menus, wizards, command palette.
- [WORKSPACE-INTEGRATION.md](./WORKSPACE-INTEGRATION.md) — workspace scaffolding and simulator wiring.
- [QT-INGEST.md](./QT-INGEST.md) — Qt/QML ingestion (`qt ingest` subcommand) and the canonical list of creator's non-Cargo external dependencies.

## See also

- [../bsp/](../bsp/) — vendor-specific BSP generation details.
- [../../src/bin/creator/README.md](../../src/bin/creator/README.md) — binary-level overview.
- [../../README-CREATOR.md](../../README-CREATOR.md) — quickstart.
