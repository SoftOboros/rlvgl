<!--
Tracks tasks for the Chip & Board Support workstream.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Chip & Board Support Workstream TODO

## Purpose
Unify chip and board configuration data into self-contained crates per vendor so that `rlvgl-creator` and its UI can dynamically populate drop-downs of vendors, microcontrollers and boards. Today the creator references a single `.ioc` or CSV file directly; this work stream extracts all supported devices into dedicated vendor crates and wires them into the build and publish pipeline.

## Intermediate Representation (IR) Plan
- Standardise on a unified IR shared by vendor crates and `rlvgl-creator`.
- Canonical device definitions live under `mcu/` and are scraped from vendor XML sources (e.g. the `STM32_open_pin_data` submodule’s `mcu/` and `ip/` trees).
- Board overlays are stored under `boards/` and are produced by converting `.ioc` examples or user-supplied CubeMX files using the canonical `mcu` data.
- Both `mcu` and `boards` datasets are bundled with creator, which also exposes a converter for arbitrary `.ioc` files.

## Epic Plan A–H: Unified Config IR and Tooling

The following epics define a cross‑vendor plan covering IR definition, ingestion, parsing, validation, packaging, CI, and documentation.

### Epic A — Unified IR & Canonicalization

- A1. Define vendor‑agnostic Config IR v0.1
  - Goal: One JSON schema for pins, clocks, peripherals, and metadata.
  - I/O: `schemas/config.schema.json`; examples in `examples/ir/*.json`.
  - AC: Covers board, MCU, package, pins (mux/pad/electrical), clocks (sources, PLL, dividers, gates), peripheral bindings (UART/I2C/SPI/GPIO/Timers). Supports `other_attributes` for unknown vendor bits. Validated with JSON Schema tests. Done when schema is committed, CI validates samples, and a changelog is created.
- A2. Canonicalization rules
  - Goal: Stable field ordering & normalization (e.g., sort pins, normalize AF names).
  - I/O: `tools/canonicalize_ir.py` (or Rust bin), unit tests.
  - AC: Same semantic IR → identical canonical JSON; structural reordering does not change canonical form. Done when canonicalizer runs in CI and goldens are updated.

### Epic B — Ingestion & Discovery

- B1. Repo manifest & fetcher
  - Goal: Periodically pull vendor examples (ST Cube, NXP SDK boards, Microchip MCC/Harmony, Renesas FSP, TI SimpleLink).
  - I/O: `manifests/vendors.yaml`; `tools/fetch_examples.py`.
  - AC: Vendor URL + version mapping; downloads & caches archives. Idempotent; supports per‑vendor rate limiting. Done when CI job populates `cache/vendor/<vendor>@<version>/`.
- B2. Project scanner
  - Goal: Detect config artefacts per vendor.
  - I/O: `tools/scan_projects.py` → `scan-index.json`.
  - AC: ST: finds `.ioc`, `*_hal_msp.c`, `main.c`. NXP: finds `.mex`, `pin_mux.c/h`, `clock_config.c/h`. Microchip: finds `mcc_generated_files/` or `pin_manager.c/h`. Renesas: finds `.pincfg`, `r_pin_cfg.c/h`. TI: finds `pinmux.c/h` or DriverLib pin arrays. Done when the index lists projects with artefact types and MCU IDs.

### Epic C — Parsers (Config Files → IR)

- C1. ST `.ioc` parser
  - Goal: Extract pins (mux/pad), RCC clock tree, and peripheral bindings.
  - I/O: `parsers/st_ioc.py` (or Rust equivalent) → IR JSON.
  - AC: Validates against 5+ boards; supports HSE/HSI/PLL; maps `GPIO_AFx_*` alternates. Done when tests pass with golden IRs.
- C2. NXP `.mex` parser
  - Goal: Extract pins (IOMUXC/IOCON), pad cfg, clocks (PLL/mux/div), peripherals.
  - I/O: `parsers/nxp_mex.py` → IR JSON.
  - AC: Works for 5+ MCUXpresso boards; computes derived clock frequencies. Done with sample fixtures.
- C3. Microchip MCC parser
  - Goal: Parse `pin_manager.c/h` (TRIS/ANSEL/PPS) into IR.
  - I/O: `parsers/mcc_pinmgr.py` → IR JSON.
  - AC: Extracts digital/analog, direction, pullups, PPS routes; 3+ PIC families. Done with golden IRs.
- C4. Renesas FSP parser
  - Goal: Parse `.pincfg/C` tables into IR.
  - I/O: `parsers/renesas_fsp.py` → IR JSON.
  - AC: Extracts PFS values, port modes; handles RA & RX sample projects. Done with fixtures.
- C5. TI pinmux/DriverLib parser
  - Goal: Extract pin config arrays and clock settings.
  - I/O: `parsers/ti_pinmux.py` → IR JSON.
  - AC: Maps `PIN_Config` flags to mux/electrical semantics. Done for 2+ TI boards.

### Epic D — C‑AST Extraction (Generated C → IR)

- D1. Preprocessing harness
  - Goal: Materialize compile flags/include paths per project.
  - I/O: `tools/materialize_tu.py`; uses CMake/Make or heuristics.
  - AC: Produces TU for `pin_mux.c`, `clock_config.c`, HAL MSP files. Done when TU logs are stored and reproducible.
- D2. AST extractor (tree‑sitter or libclang)
  - Goal: Match vendor call patterns & struct initializers to recover semantics.
  - I/O: `extractors/ast_core.rs` + vendor adapters.
  - AC: ST: `HAL_GPIO_Init`, `HAL_RCC_*` captured. NXP: `IOMUXC_*`, `CLOCK_*`. Microchip: TRIS/ANSEL/PPS writes. Emits IR with numeric and symbolic values (via enum/define maps). Done with snapshot tests vs. file‑parsers (C1–C5).
- D3. Constant folder & symbol maps
  - Goal: Resolve `#define`/enum to numbers & semantic names.
  - I/O: `extractors/symbol_map_gen.rs` (header scrapers), cache JSON.
  - AC: Covers AF enums, pin IDs, clock mux enums for ST/NXP/TI. Done when ≥95% constants resolve on the test corpus.

### Epic E — Validation & Round‑Trip

- E1. ST cross‑validation with STM32_open_pin_data
  - Goal: Assert pins/AFs exist for selected package.
  - I/O: `validators/st_crosscheck.py`.
  - AC: 100% pin/function pairs validated or flagged; reports diffs. Done in CI with per‑board reports.
- E2. Register oracle checks
  - Goal: Compare emitted values with “export registers” (when available) or vendor code.
  - I/O: `validators/reg_oracle.py`.
  - AC: Tolerates benign ordering; flags semantic diffs. Done in nightly CI.
- E3. Round‑trip codegen & diff
  - Goal: IR → codegen (C or Rust) → textual/semantic diff to source.
  - I/O: `codegen/{st,nxp,...}/emit_*`; `validators/code_diff.py`.
  - AC: Pins/clocks/peripheral calls regenerate with ≥90% textual match or 100% semantic match. Done on representative samples.

### Epic F — Database & API

- F1. Versioned config‑db repository
  - Goal: Store one JSON per `vendor.board@version`.
  - I/O: `config-db/<vendor>/<board>/<ver>/config.json`.
  - AC: Commit only on change; include provenance (source paths, hashes). Done when CI publishes artifacts and a changelog is generated.
- F2. Query CLI
  - Goal: `cfgdb query --board NUCLEO-H743ZI --peripheral UART3` → pin map, clocks.
  - I/O: `cli/cfgdb` (Python or Rust).
  - AC: Filters by board, MCU, peripheral; outputs JSON/pretty table. Done with unit and smoke tests.
- F3. Embeddable library
  - Goal: Small Rust crate for consumers (`rlvgl-creator`).
  - I/O: `crates/cfgdb`.
  - AC: `get_pins()`, `get_clock_tree()`, `find_peripheral_routes()`. Done when published.

### Epic G — CI/CD & Ops

- G1. CI matrix (vendors × boards)
  - Goal: Run fetch → parse → validate → publish on schedule.
  - I/O: `.github/workflows/configdb.yml`.
  - AC: Caches SDKs; retries transient network; artifacts uploaded. Done with badges and nightly summary.
- G2. Diff reports
  - Goal: HTML/Markdown reports of changes per run.
  - I/O: `reports/*`.
  - AC: Lists new/changed/failed projects; links to logs/fixtures. Done when linked from CI summary.
- G3. Licensing guard
  - Goal: Ensure we only store derived IR, never vendor source.
  - I/O: `tools/license_guard.py`.
  - AC: Fails CI if non‑IR vendor sources leak into repo; SPDX checks. Done when policy is documented.

### Epic H — Developer Experience & Docs

- H1. CONTRIBUTING.md & STYLE.md
  - Goal: How to add a new vendor/board/parser; coding standards.
  - AC: New contributors can add a parser with a worked example PR. Done when docs and a PR template exist.
- H2. Fixtures & golden tests
  - Goal: Curated corpus with minimal, typical, and edge configs per vendor.
  - I/O: `fixtures/<vendor>/<board>/…`.
  - AC: Tests cover empty/analog/alternate/OD/pull cases; clock edge cases (HSI/HSE/PLL). Done with coverage report.
- H3. Quickstart
  - Goal: 5‑minute guide: fetch one board, parse, query pins.
  - AC: Runs locally on macOS/Linux with documented prerequisites. Done when verified by CI “docs test”.

## Repo Status vs. Plan (snapshot)

Summary of what exists in this repository today versus the plan above.

- A1 Config IR schema: Partial. Canonical STM32 schemas exist (`schemas/mcu_canonical.schema.json`, `schemas/ip_canonical.schema.json`) and a minimal Rust IR for BSP tests exists (`src/bin/creator/bsp/ir.rs`). A unified cross‑vendor `schemas/config.schema.json` and samples under `examples/ir/` are not yet present.
- A2 Canonicalizer: Not started. No `tools/canonicalize_ir.py` or Rust equivalent. AFDB pipeline normalizes STM32 XML, but there’s no canonicalizer for a unified Config IR artifact yet.
- B1 Fetcher: Not started. No `manifests/vendors.yaml` or fetcher; current tests rely on local fixtures under `tests/data/`.
- B2 Project scanner: Not started. No `tools/scan_projects.py` or `scan-index.json` artifacts.
- C1 ST `.ioc` parser: Done (core path). Implemented in Rust (`src/bin/creator/bsp/ioc.rs`) with tests (`tests/bsp_roundtrip.rs`) and Python AFDB helpers (`tools/afdb/pin_context.py`, `tools/afdb/st_ioc_board.py`). Extracts pins, labels, PLLs, and kernel clocks; maps AF via canonical MCU DB.
- C2–C5 Other vendor parsers: Not started. No NXP `.mex`, Microchip MCC, Renesas FSP, or TI pinmux parsers yet.
- D1 C‑AST preprocessing harness: Not started. No TU materialization yet.
- D2 C‑AST extractor: Partial. Initial regex-based extractor in Rust (`src/bin/creator/ast.rs`) captures common `HAL_GPIO_Init` blocks (including multi‑pin bitmasks) and `__HAL_RCC_*_CLK_ENABLE()` hints, emits IR, and is exposed via new CLI commands `ast from-c` and `bsp from-c`. A smoke test exists (`tests/ast_from_c.rs`).
- D3 Constant folder: Not started. No enum/define evaluation yet.
- E1 ST cross‑validation: Partial. AFDB uses `STM32_open_pin_data`; label/AF mapping is validated in unit tests (`tests/test_parse_mcu.py`, `tests/test_parse_ip.py`), but no explicit `validators/st_crosscheck.py` or CI report exists.
- E2 Register oracle: Not started.
- E3 Round‑trip codegen & diff: Partial. IR → MiniJinja codegen with snapshot tests (`tests/bsp_roundtrip.rs`) exists. Added `render_from_ir` to support non‑IOC IR sources (e.g., C‑AST). No textual diff against vendor sources yet.
- F1 Versioned config‑db: Not started.
- F2 Query CLI: Not started. Creator has helpers (`src/bin/creator/boards.rs`) but no standalone `cfgdb` tool.
- F3 Embeddable library: Not started.
- G1 CI matrix: Not started for config‑db. CI and `scripts/pre-commit.sh` cover formatting, clippy, docs, and an embedded example build but not scheduled vendor ingestion.
- G2 Diff reports: Not started.
- G3 Licensing guard: Partial. `deny.toml` and `NOTICES.md` document licensing; vendor ingestion scripts stamp licenses (`tools/build_vendor.sh`). No explicit `tools/license_guard.py`.
- H1 CONTRIBUTING/STYLE: Not started. `CODE_OF_CONDUCT.md` exists; no `CONTRIBUTING.md`/`STYLE.md` yet.
- H2 Fixtures & goldens: Partial→Strong. Rich Rust and Python tests under `tests/` with fixtures and schemas; expand to cover additional vendors once parsers exist.
- H3 Quickstart: Partial. `README-CREATOR.md` and example docs exist; add a focused 5‑minute “fetch → parse → query” for config‑db once B1/B2 land.

## Immediate Next Steps

- Define `schemas/config.schema.json` and create `examples/ir/` samples (Epic A1).
- Add a simple canonicalizer script/binary to sort fields and normalize AF strings; wire basic tests (Epic A2).
- Land STM32 pipeline as the first “vendor” in the unified IR: adapt existing AFDB output to emit the Config IR alongside current canonical overlays (Epics A/C/E intersection).
- Draft `manifests/vendors.yaml` and a minimal `tools/fetch_examples.py` that only supports STM32 Cube projects to seed the ingestion flow (Epic B1).
- Add `docs/IOC-IR-ALIGNMENT.md` references into this plan and link from `README-CREATOR.md`; extend docs with a Quickstart once B1/B2 exist (Epic H3).
- C‑AST: expand pattern coverage to infer signal roles (TX/RX/SCK/…); recognize I2C/SPI/TIM/SDMMC/ULPI blocks; capture pull/speed/otype.
- C‑AST: add a preprocessing harness (`tools/materialize_tu.py`) to record TU flags and includes for future tree‑sitter/libclang parsing.
- Creator docs: document `ast from-c` and `bsp from-c` usage in `README-CREATOR.md` with examples.

### Level D — C‑AST (experimental)
- [x] `ast from-c` CLI to emit IR from C sources.
- [x] `bsp from-c` CLI to render HAL/PAC from extracted IR.
- [x] Minimal extractor with multi‑pin bitmask and RCC enable recognition.
- [x] `render_from_ir` in `bsp_gen.rs` to decouple rendering from `.ioc`.
- [ ] Role inference (TX/RX/SCK/CS/…) and peripheral signal mapping.
- [ ] Constant folding for enums/defines and struct initializers.
- [ ] Preprocessing harness to materialize TU (flags/includes).

### User Pin Labels (GPIO_Label) Propagation
- [x] Extend IR `Pin` with optional `label: Option<String>`.
- [x] Parse `*.GPIO_Label` from CubeMX `.ioc` (supports `PAx` and `PAx_C` forms) and attach to IR.
- [x] Surface labels in generated code comments (PAC/HAL templates).
- [x] Add unit tests for label parsing and template rendering.
- [ ] CLI flags for label behavior in BSP generation:
  - [x] `--use-label-names` to use sanitized labels as identifiers (HAL template).
  - [x] `--label-prefix <str>` to prefix identifiers starting with digits/underscores.
  - [x] `--fail-on-duplicate-labels` to error on collisions after sanitization.
  - [ ] Document flags in `README-CREATOR.md` with examples.
- [ ] Optional: generate a `pins` helper module exporting `pub const` aliases for labels in PAC output.

## Pre-setup Instructions
1. **Establish vendor crate locations**
   - Create a new top-level directory (e.g. `chipdb/`) or reuse an existing `support/` subfolder to house vendor crates. Each vendor gets its own crate, such as `rlvgl-stm-pins` for STMicroelectronics devices, `rlvgl-nrf-pins` for Nordic, `rlvgl-esp-pins` for Espressif, `rlvgl-nxp-pins` for NXP, `rlvgl-silabs-pins` for Silicon Labs, `rlvgl-microchip-pins` for Microchip, `rlvgl-renesas-pins` for Renesas, `rlvgl-ti-pins` for Texas Instruments, and `rlvgl-rp2040-pins` for generic RP2040 support. All crates live under the workspace and share the same edition while carrying vendor-specific license files.
   - Include a `README.md` in each vendor crate describing the embedded board database format and how the crate integrates with `rlvgl-creator` so that it can be used independently.
2. **Add workspace members**
   - Update the root `Cargo.toml` `[workspace]` section to include the new vendor crates. This ensures they are compiled and published along with the rest of the repository.
   - For each vendor crate add a bare-bones `Cargo.toml` with:
     ```toml
     [package]
     name = "rlvgl-stm-pins"
     version = "0.0.1"
     edition = "2021"
     publish = true
     license = "BSD-3-Clause" # adjust per vendor

     [lib]
     crate-type = ["rlib"]

     [dependencies]
     serde = { version = "1", features = ["derive"], optional = true }
     ```
     You can derive a similar template for other vendors. The optional `serde` feature allows the crate to optionally serialise/deserialise its board database.
3. **Vendor data submodules & licensing**
   - Store vendor source configuration files only as git submodules under `vendor/` to avoid redistributing third-party data.
   - Ignore generated databases (e.g. `crates/rlvgl-stm-pins/src/**`) via `.gitignore`; they are produced during build or publish.
   - Each vendor crate carries its own `LICENSE` file matching the vendor’s requirements (e.g. BSD-3 for ST) while the repository root remains MIT and lists vendor crates in `LICENSE-THIRD-PARTY.md`.
   - Provide `tools/build_vendor.sh` and `tools/gen_pins.py` to initialise submodules, convert vendor files into canonical outputs, stamp the appropriate license, and populate the vendor crate prior to `cargo build` or `cargo publish`.
   - For STM32 support, add the `STM32_open_pin_data` repository as a submodule and scrape its XML. Convert `mcu/` and `ip/` into canonical `mcu` IR and translate bundled `.ioc` files into `boards` overlays.
4. **Bootstrapping `build.rs` for asset embedding**
   - Each vendor crate should contain a `build.rs` script that locates the extracted board definition files (generated by the conversion tools above) and embeds them into the binary using `include_bytes!` or a custom archive builder. Use an environment variable (e.g. `RLVGL_CHIP_SRC`) to point to the directory of extracted definitions at build time. The build script can then iterate over that directory, compress or pack the files into a single archive (e.g. tar or zip), and emit a Rust source file that exposes a static `VENDOR_DB: &'static [u8]` or a typed wrapper around it.
   - Write a small wrapper API in `lib.rs` that exposes helper functions:
     - `fn vendor() -> &'static str` – returns the vendor name used by the UI.
     - `fn boards() -> &'static [BoardInfo]` – returns a list of available boards and microcontrollers.
     - `fn find(board_name: &str) -> Option<&'static BoardInfo>` – lookup by exact board or chip name.
   - A `BoardInfo` struct should include at minimum: board name, chip name, package, pin configuration blob offset/length and maybe a description.
5. **Extend the Python extraction tool**
   - The existing `tools/st_extract_af.py` extracts `.ioc` files under `chips/stm/...` into a normalised format. Extend this script to handle both `.ioc` and `.csv` sources. Detect the file type by extension; parse `.csv` pin descriptions into the same intermediate representation used for `.ioc` files (a JSON or YAML dictionary keyed by pin name and function). The output should be a single JSON or YAML file per board containing all pins, metadata, and the board/chip names. Place the generated files into a build cache directory (e.g. `build/chipdb/stm`).
   - Provide a command-line interface: `python tools/st_extract_af.py --input path/to/board_dir --output build/chipdb/stm`. Document usage in the script header and in this TODO; update `README.md` if needed.
   - Add unit tests for the parser that feed sample `.ioc` and `.csv` files and assert on the resulting dictionary keys/values. Place these tests under `tests/tools_st_extract_af.rs` so they run in CI.
6. **Update the publish workflow**
   - Modify the GitHub Actions or GitLab CI configuration to run `tools/build_vendor.sh` before packaging the vendor crates. The script should initialise submodules, invoke `tools/gen_pins.py`/`tools/st_extract_af.py` with appropriate input/output directories, and then set `RLVGL_CHIP_SRC` for the subsequent `cargo publish` step.
   - Ensure that the vendor crates are versioned and published whenever the generation output changes. Use `cargo publish --dry-run` to validate before release.

## Tasks
### Level 1 – Vendor crate scaffolding
- [x] **Create rlvgl-stm-pins crate** – Set up the first vendor crate: directory, `Cargo.toml` (license = BSD-3-Clause), `build.rs`, `lib.rs` skeleton, and `README.md` describing usage and format. Depends on: Pre-setup
- [x] **Create rlvgl-…-pins crates** – For each additional vendor identified (Nordic, Espressif, NXP, Silicon Labs, Microchip, Renesas, Texas Instruments, generic RP2040), copy the STM template, including the `README.md` and appropriate license text, and adjust package names. Depends on: STM crate
- [x] **Generation scripts** – Provide `tools/gen_pins.py` and `tools/build_vendor.sh` to fetch submodules, convert vendor data, stamp license files, and populate the vendor crates before building. Document usage in `README.md`. Depends on: Vendor crate scaffolding
- [x] **Implement BoardInfo API** – Define the `BoardInfo` struct and helper functions (`vendor`, `boards`, `find`) in each vendor crate. Depends on: STM crate
- [x] **Embed extracted definitions** – Write build logic to read the JSON/YAML files produced by `gen_pins.py`/`st_extract_af.py` and pack them into the binary. Depends on: Python tool
- [x] **Unit tests for vendor crates** – Add tests verifying that `boards()` returns the expected number of entries and that `find()` resolves known names. Depends on: API impl

### Level 2 – Python extraction & conversion
- [x] **STM32 XML scraper** – Parse `STM32_open_pin_data` `mcu/` and `ip/` directories into canonical `mcu` IR. Depends on: Pre-setup
- [ ] **Ignore undefined MCUs** – Skip or delete MCUs without pin definitions after scraping to prevent `.ioc` conversion failures; accepts `--skip-list` for bulk exclusions. Depends on: STM32 XML scraper
- [ ] **.ioc overlay generation** – Populate IR JSON with pin contexts when converting `.ioc` files into `boards` entries using the canonical `mcu` data. Depends on: STM32 XML scraper
- [ ] **User .ioc conversion** – Provide a CLI that accepts a CubeMX `.ioc` and emits a `boards` overlay using the canonical schema. Depends on: .ioc overlay generation
- [x] **CSV parser integration** – Extend `tools/st_extract_af.py` to parse CSV pin descriptions into the same intermediate representation. Depends on: Pre-setup
- [x] **Unified output format** – Ensure both `.ioc` and `.csv` sources produce a consistent JSON/YAML schema used by the vendor crates. Depends on: CSV parser
- [x] **Command-line interface** – Add CLI flags for input directory, output directory and vendor name. Depends on: Unified format
- [x] **Sample file tests** – Add automated tests in Rust or Python that process sample `.ioc` and `.csv` files and compare against expected snapshots. Depends on: CSV parser
- [x] **Documentation** – Document the usage of the script and the expected file formats in the repository’s `README.md` and in this TODO. Depends on: CLI

### Level 2a – Board IR normalisation & Rust init templates
- [ ] **Canonical pin context** – Build per-pin objects with fields: `name`, `port`, `index`, `class`, `sig_full`, `instance`, `signal`, `af`, `mode`, `pull`, `speed`, `otype`, `label`, `is_exti`, `exti_line`, `exti_port_index`, `exti_rising`, `exti_falling`, `moder_bits`, `pupd_bits`, `speed_bits`, `otype_bit`, `hal_speed`, and `hal_pull`.
- [ ] **Lookup tables** – Map Cube strings for mode, pull, speed, and otype to register bits and HAL enum names.
- [ ] **Board overlay emission** – Save canonical pin contexts under `boards/<board>.json` so all boards share the same schema.
- [ ] **HAL template rules** – Render `into_alternate`, `into_push_pull_output`, `into_analog`, etc., using helpers for speed, pull, otype, and AF.
- [ ] **PAC template rules** – Emit register writes for MODER/OTYPER/OSPEEDR/PUPDR and AFR; include EXTI routing when `is_exti` is true.
- [ ] **EXTI fields** – Derive `exti_port_index`, `exti_rising`, and `exti_falling` from `.ioc` mode strings for interrupt-capable pins.
- [ ] **Snapshot & template tests** – Verify `.ioc` → context conversion and HAL/PAC generation.

### Level 3 – Creator integration
- [x] **Expose canonical IR** – Update creator to load `mcu` definitions alongside `boards` overlays and surface both in the UI and CLI. Depends on: STM32 XML scraper
- [x] **Custom .ioc import** – Add a conversion flow in creator allowing users to import their own CubeMX `.ioc` files. Depends on: Expose canonical IR
- [x] **Add vendor crate dependencies** – Update `rlvgl-creator`'s `Cargo.toml` to depend on each vendor crate via path or workspace. Depends on: Vendor crates
- [x] **Public API to enumerate boards** – Add functions in creator to iterate over all vendor crates and build a flat list of Vendor → Boards entries. Depends on: Vendor API
- [x] **UI drop-down support** – On the creator branch’s UI (see `rlvgl/creator-ui`), update the board selection component to present vendor, chip and board options loaded from the crates. Depends on: Enumeration API
- [x] **Exact name matching** – Ensure that the UI selection uses exact matching to find board definitions in the vendor crates. Depends on: UI support
- [x] **Fallback / error handling** – Define behaviour when a vendor crate or board is missing; display a clear message rather than panicking. Depends on: UI support

### Level 3a – Compressed IR loading
- [x] **Embed zstd databases** – Vendor crates emit `chipdb.bin.zst` archives and expose them via `raw_db()`.
- [x] **Creator decompression** – Teach `rlvgl-creator` to locate and decompress the `.bin.zst` data through vendor crate APIs.
- [x] **Round-trip tests** – Exercise compressed archives end to end, ensuring `load_ir` loads board and MCU IR.

### Level 4 – Publish & CI integration
- [x] **CI extraction step** – Add a CI job that runs `tools/build_vendor.sh` against the vendor submodules. Depends on: Python CLI. Covered by `tools/tests/test_build_vendor.py`.
- [x] **Environment variable wiring** – Pass `RLVGL_CHIP_SRC` or similar environment variables into the vendor crate build to locate the generated files. Depends on: CI extraction. Verified via `tools/tests/test_build_vendor.py`.
- [x] **Cargo publish matrix** – Extend the release workflow to publish each vendor crate along with core, ui, and other crates. Depends on: Vendor crates
- [x] **Version bump automation** – Write a script or adopt `cargo release` to bump versions across vendor crates when new definitions are generated. Depends on: Publish matrix
- [x] **Update docs and changelog** – Add release notes summarising supported chips/boards and instructions for using the new crates with creator. Depends on: Publish matrix
- [x] **Pack chip databases** – `tools/pack_chipdb.py` creates `chipdb.bin.zst` archives during publish so crates ship prebuilt data.
- [x] **Vendor build verification** – Tests cover `pack_chipdb.py` round trips and the version-bump script, keeping publish artifacts reproducible.

## Codex Playbook — STM32 Open Pin Data ➜ Canonical JSON (lossless + canonical)
Track a new Python-based pipeline (`afdb`) that converts STM32 vendor XML into lossless and canonical JSON. Facts: ST ships no public XSD for per-MCU or IP XML, so we ingest raw data and validate the canonical overlay with local JSON Schemas.
- [x] **Phase 0 – Repo scaffold & deps** – Create `tools/afdb` package with CLI, schemas, fixtures, and minimal `pyproject.toml`.
- [x] **Phase 1 – Lossless XML ingest** – Implement `ingest_raw.py` and `util_xml.py` to parse XML safely into raw JSON trees preserving order, attributes, and line numbers.
- [x] **Phase 2 – Canonical MCU overlay** – Build `parse_mcu.py` to overlay structured MCU data (meta, instances, pins) while retaining the raw snapshot.
- [x] **Phase 3 – Canonical IP overlay** – Write `parse_ip.py` to normalize IP `_Modes.xml` files into peripheral signal dictionaries.
- [x] **Phase 4 – Catalog builder** – Fuse MCU and IP overlays via `build_catalog.py`, producing per-part catalogs keyed by pins and instances.
- [x] **Phase 5 – JSON Schemas** – Define `schemas/mcu_canonical.schema.json` and `schemas/ip_canonical.schema.json` to validate canonical outputs.
- [x] **Phase 6 – CLI wiring** – Expose `afdb` subcommands (`import-mcu`, `import-ip`, `build-catalog`) that emit canonical JSON.
- [x] **Phase 7 – Tests** – Add fixture-driven `pytest` suites covering ingest, MCU/IP parsing, and catalog building.
- [x] **Phase 8 – Reports** – Generate optional human-readable pin/function tables under `reports/`.
- [x] **Phase 9 – Integration hooks** – Store generated catalogs under `afdb/<family>/<refname>.json` and retain `raw_xml_path` for provenance.

#### Ready-to-run prompts
- Prompt A – scaffold & dependencies
- Prompt B – implement `parse_ip.py` and `build_catalog.py`
- Prompt C – add JSON Schemas and tests
- Prompt D – wire CLI and demonstrate end-to-end run

#### Notes on field retention
- Every input element or attribute persists either in the raw snapshot or in an `other_attributes` map on the canonical nodes.
- Child order is preserved via `raw_tree.children[]`, and duplicate scalar tags become arrays in the canonical overlay.
- Namespaces are stored in `qname.ns` so provenance can always be traced back to the vendor XML.

### STM32 data crate size plan
- [x] Optimise the 11.4 MB zipped MCU XML into a compact IR and Zstd artifact targeting ≤ 4–6 MB per family.
- [x] Shard runtime data by MCU family (e.g. `stm32-data-f`, `stm32-data-h`) embedding a single `.bin.zst` per crate.
- [ ] Re‑evaluate the need for further splits if size limits are still tight.

## Definition of Done
- All vendor crates compile and expose a stable API returning board definitions.
- The Python extraction script can parse both `.ioc` and `.csv` and produces consistent outputs; tests cover representative samples.
- `rlvgl-creator` can list vendors, chips and boards via the new crates, and the UI drop-downs populate correctly.
- CI runs extraction and publishes updated vendor crates; version numbers bump automatically when definitions change.
- Documentation in `README.md` and this TODO is up to date, and `docs/TEST-TODO.md` includes new test IDs covering chip support (e.g. T-19 for vendor enumeration, T-20 for board loading smoke test).
- Canonical `mcu` and `boards` IR derived from `STM32_open_pin_data` is consumable through `rlvgl-creator`, which can also convert user-supplied `.ioc` files.
- Each STM32 data crate embeds a compressed `.bin.zst` artifact per MCU family and stays well under the 10 MB publish limit.
