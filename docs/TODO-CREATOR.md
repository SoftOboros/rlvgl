<!--
docs/TODO-CREATOR.md - rlvgl-creator — Epic & Sectioned Tables.
-->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# rlvgl-creator — Epic & Sectioned Tables

_A single markdown that structures the work as one **Epic** with sectioned user‑story tables. Each section begins with a brief description (user story) and a checklist table._

---

## Epic Overview
**Epic:** Build **rlvgl-creator**, a UI + CLI tool that imports, normalizes, previews, and vendors assets for rlvgl projects while scaffolding dual‑mode assets crates and minimizing footprint on `no_std + alloc` targets.

**Outcomes:**
- Repeatable pipelines for raw RGBA image sequences, fonts, and Lottie.
- Strict naming/path policies with auto‑fix guidance.
- Dual delivery (embed vs vendor) for asset packs.
- A desktop UI for preview, sizing, and packaging.

---

## 0) Locked Decisions & Policies
_User story: As a maintainer, I want guardrails so teams can scale assets safely without drift._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Enforce folder roots `icons/`, `fonts/`, `media/`; reject others with fix‑it guidance. | creator core | Policy checker + `--fix` rename.
| [x] | Generate const/feature names; forbid manual edits (SCREAMING_SNAKE; `ICON_`, `FONT_`, `MEDIA_`). | creator core | Deterministic name map; diff output.
| [x] | Creator is `std`; targets are `no_std + alloc` friendly; pre‑size/pack assets. | N/A | Design constraint across features.
| [x] | Base assets stored as raw RGBA images/sequences; no PNG/APNG at runtime. | internal | Replaces std-dependent formats. |
| [x] | Support both direct Lottie playout and Lottie→APNG conversion. | rlottie (FFI) or Conan CLI | Per‑asset choice recorded in manifest.
| [x] | Optional bundle compression using RLE + token table; core path decodes with a tiny gated decoder. | internal | no_std-friendly; prefer build-time compress in vendor path. |

---

## 1) CLI Surface & UX
_User story: As a developer, I can manage assets via clear commands with helpful validation and examples._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | `init` — bootstrap folders and default `manifest.yml`. | clap, anyhow | Idempotent; prints next steps.
| [x] | `scan <path>` — discover new/changed assets and update manifest. | blake3, walkdir | Hash‑based; respects roots policy.
| [x] | `convert` — normalize to raw RGBA sequences; pack fonts; write metadata. | image, fontdue/ab_glyph | Deterministic outputs. |
| [x] | `vendor` — copy to `$OUT_DIR`/repo and generate `rlvgl_assets.rs`. | std fs, tera | Supports per‑target preset.
| [x] | `scaffold assets-crate` — generate dual‑mode crate. | tera | Embed & vendor features.
| [x] | `preview` — thumbnails/sprite sheets. | image | Stores in `assets/thumbs/`.
| [x] | `add-target` — register local crate + `vendor_dir` and presets. | serde_yaml | Updates manifest.
| [x] | `sync` — regenerate Cargo features, consts, index from manifest. | tera | Dry‑run mode prints diff.
| [x] | `apng` — build APNG from raw frame groups; set timing/loops. | apng | First frame PNG export. |
| [x] | `lottie import` — Lottie→frames/APNG; export timing map. | rlottie/CLI | Records chosen path.
| [x] | `fonts pack` — sizes, glyph sets, packing/metrics. | fontdue/ab_glyph | Optional subsetting.
| [x] | `check` — strict policy validation; `--fix` auto‑normalize. | creator core | Non‑zero exit on violations.
| [ ] | `ui` — launch desktop UI. | Tauri or eframe/wgpu | Shares core libs.
| [x] | Provide global flags and rich help with examples. | clap | Standardized exit codes.
| [x] | Split CLI implementation across modules. | internal | Keeps binaries maintainable.

---

## 2) Manifest & Conventions
_User story: As a maintainer, I want a machine‑owned manifest that encodes policy and targets._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Define `manifest.yml` v1 (`packages`, `groups`, `features`, `expose`, `targets`). | serde_yaml, schemars | Emits JSON schema for editor tooling.
| [x] | Enforce path policy: public paths under `icons/`, `fonts/`, or `media/`. | creator core | Actionable errors + `--fix`.
| [x] | Generate feature names from groups; emit `*_all` aggregates. | creator core | Stable order.
| [x] | Generate const names from manifest entries; reject manual renames. | creator core | Diff prints old→new mapping.
| [x] | License metadata per asset/group with allow/deny list. | SPDX table | Block vendor if missing.
| [x] | `naming` config (prefix map + case policy) for docs; generator is source of truth. | N/A | Keeps policy explicit.
| [x] | Per‑target presets (screen size, depth, storage) for auto sizing. | presets file | Hooked into `vendor`.

---

## 3) Assets Crate Scaffolding (Dual‑Mode)
_User story: As a user, I can consume assets by embedding bytes or vendoring files without runtime deps._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Generate `Cargo.toml` with `embed`, `vendor`, and group features. | tera | No default features.
| [x] | Generate `src/lib.rs` — embed: `include_bytes!` consts. | tera | One const per exposed asset.
| [x] | Generate `src/lib.rs` — vendor: `vendor_api::{copy_all, generate_rust_module}`. | std fs | `$OUT_DIR` safe paths.
| [x] | Optional `build.rs` self‑test for crate. | std | Smoke test in CI.
| [x] | Generate README with embed vs vendor usage. | tera | Copy‑paste snippets.
| [x] | Snapshot tests for generated files. | insta | Guard regressions.
| [x] | `cargo publish --dry-run` passes. | cargo | CI gate.

---

## 4) Conversion Pipelines
_User story: As a designer, I can drop common formats and get normalized, fast‑to‑load outputs._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Raw RGBA sequence format with max-frame header; per-frame size/position; single images drop frame headers. | internal | Replaces PNG/APNG base. |
| [x] | Encode `.raw` files from common inputs. | creator core | Normalizes raster assets. |
| [x] | Ingest `.raw` files into pipeline. | creator core | Parse header and frames. |
| [x] | SVG→sized raw images (DPI list; monochrome/e-ink thresholds). | resvg/usvg (opt.) | Fallback to external if needed. |
| [x] | APNG builder from raw frames with per-frame delay and loop count; first-frame PNG. | apng | Frame ordering checks. |
| [x] | Lottie via FFI (`lottie-ffi`) using `rlottie`. | rlottie, Conan | Feature gate; platform notes.
| [x] | Lottie via external CLI (`lottie-cli`) to frames/APNG. | Conan recipe | Record path in manifest.
| [x] | Fonts: TTF/OTF→bitmap packs (`.bin`) + metrics (`.json`); optional subset. | fontdue/ab_glyph | Glyph set per target.
| [x] | Simple RLE + token table compression for raw files. | internal | Tiny decoder for no_std targets. |

---

## 5) UI Application (Creator UI)
_User story: As a developer/designer, I can preview, zoom/pan, group, and export assets visually._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Choose stack and bootstrap UI project. | eframe/egui | Initial window and manifest loading.
| [x] | Asset Browser panel (tree, filters, search, license badges). | UI kit | Reflects manifest groups.
| [x] | Canvas Viewer (zoom/pan, pixel grid, checkerboard BG). | wgpu/pixels | APNG/Lottie scrubber pending.
| [x] | Inspector: Meta (size/DPI/hash/license/tags/groups). | serde | Live‑edit writes manifest.
| [x] | Inspector: Export (sizes, color space, premult alpha, compression). | creator core | Applies per‑asset.
| [x] | Inspector: Animation (timing/loops; Lottie→APNG options). | apng/rlottie | Scrubber UI.
| [x] | Inspector: Fonts (glyph set, sizes, hinting, packing). | fontdue | Preview pangrams.
| [x] | Drag‑drop to `assets/raw/` with immediate `scan`. | notify | Shows toasts.
| [x] | Size‑to‑screen presets (e.g., `stm32h7‑480x272`) with live preview. | presets | Renders bounding boxes.
| [x] | Actions: "Make APNG from selection", "Add to group", "Reveal in manifest". | UI kit | Multi‑select support.
| [x] | Thumbnails pipeline + hot‑reload. | image, notify | Cache invalidation via hash.
| [x] | Layout preview/editor for quick UI prototyping. | later | Basic drag-and-drop layout canvas.

---

## 6) Vendor & Embed Integration
_User story: As an app author, I can choose embed or vendor and get identical bytes._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Embed examples (`default-features=false`, per‑group features; const usage). | examples | CI builds them.
| [ ] | Vendor examples (consumer `build.rs` + `include!(.../rlvgl_assets.rs)`). | examples | `$OUT_DIR` safe.
| [x] | Optional `get(path)` API in embed mode (path→bytes). | phf/lite map | Generated index.
| [x] | Byte‑equality test: embed vs vendor for same asset IDs. | tests | CI assertion.

---

## 7) Caching & Incremental Builds
_User story: As a user, I want fast re‑runs with deterministic outputs._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Content‑hash cache in `assets/.cache` (hash→outputs/timestamps/sizes). | blake3, serde | JSON/CBOR store.
| [x] | `--force` invalidation and smart rebuild by hash/mtime. | creator core | Clear messaging.
| [x] | Parallelize conversions with stable ordering. | rayon (opt.) | Guard race conditions.
| [x] | Emit `cargo:rerun-if-changed` hints for vendor/build steps. | build.rs API | Good DX for consumers.

---

## 8) Validation, Lints & CI
_User story: As a maintainer, I can trust every PR to enforce policy and stay green._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | `creator check` covers paths, names, license, duplicates, size thresholds. | creator core | Non‑zero exit.
| [x] | Pre‑commit hook template (scan/convert/check). | git hooks | Optional but encouraged.
| [x] | CI job runs end‑to‑end: `scan → convert → sync → scaffold → vendor`. | GH Actions | Caches toolchains.
| [x] | Golden tests for APNG timing and font samples. | apng, fontdue | Deterministic fixtures.
| [x] | Snapshot tests for generated `Cargo.toml`, `lib.rs`, `rlvgl_assets.rs`. | insta | Stored in repo.

---

## 9) Acceptance Criteria (MVP)
_User story: As a stakeholder, I can verify value quickly with a working vertical slice._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Dual‑mode assets crate compiles from scaffold. | cargo | smoke test.
| [ ] | `scan + convert + sync` match manifest; no stray files. | creator core | CI check.
| [x] | Vendor and embed yield identical bytes for same asset IDs. | tests | Byte compare.
| [ ] | APNG from simple frames plays with correct timing in a reference viewer. | apng | Viewer in CI (headless).
| [ ] | `cargo publish --dry-run` for generated crate succeeds. | cargo | Versioning rules.
| [ ] | Non‑conforming inputs get actionable errors and `--fix` resolves them. | creator core | Human‑friendly output.

---

## 10) Roadmap / Phases
_User story: As a planner, I can stage delivery to land value early and often._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Phase 1 – MVP: scan/convert/vendor; scaffold crate; strict check. | core pieces | Baseline release.
| [x] | Phase 2 – Fonts: subsetting/packing/metrics; feature groups by size/family. | fontdue | Improves load perf.
| [x] | Phase 3 – Lottie: import + APNG; sprite sheets + timing meta. | rlottie/apng | Broader animation support.
| [x] | Phase 4 – Preview: thumbs + CLI/GUI viewer; size/hotpath profiling. | UI + image | Developer speed.
| [x] | Phase 5 – GUI: full UI with layout preview/editor and presets. | UI stack | Designer speed.
| [ ] | Phase 6 – Advanced: wasm pipelines; remote catalogs; CDN packaging. | wasm-bindgen | Stretch.

---

## 11) Stretch & Nice‑to‑Haves
_User story: As a power user, I can optimize pipelines and packaging further._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Sprite sheet/atlas builder (+ JSON/RON atlas). | image, serde | Option for particle/UI.
| [ ] | Per‑target presets & wizards (display/bpp/storage constraints). | presets | Wizard UX.
| [ ] | License gate on vendor (block incompatible assets). | SPDX | Legal safety.
| [ ] | Local telemetry: bytes saved, load‑time and RAM/flash estimates. | stats module | Opt‑in.
| [ ] | Plugin points for custom converters/optimizations. | trait APIs | Load from TOML.

---

## 12) Deliverables & Docs
_User story: As a newcomer, I can get productive with clear examples and guides._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Example assets pack (icons/fonts/media) with manifest. | repo data | Used in tests.
| [ ] | Two consumer examples: **embed** and **vendor** patterns. | examples | CI builds & runs.
| [x] | User guide (README) with end‑to‑end workflow. | mdbook/README | Screenshots/gifs.
| [x] | Developer docs for templates (Tera) and pipeline hooks. | rustdoc | API + templates directory.
