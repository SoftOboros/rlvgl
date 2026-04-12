<!-- SVELTE-DESIGN-TOKEN-ALIGNMENT.md - Svelte design token alignment. -->
<p align="center">
  <img src="../rlvgl-logo.png" alt="rlvgl" />
</p>

# Svelte Design Token Alignment

_A single markdown that structures the work as one **Epic** with sectioned user‑story tables. Each section begins with a brief description (user story) and a checklist table._

---

## Epic Overview
**Epic:** Align Svelte (design tokens, component authoring, and runes state) with `rlvgl` by extending `rlvgl-creator` to generate files for web and embedded targets from shared UI sources.

**Outcomes:**
- Shared token source produces web CSS/Tailwind outputs and `rlvgl-ui` theme outputs.
- Svelte component authoring maps to `rlvgl` widget trees (subset with clear constraints).
- Svelte 5 runes map to embedded state bindings and derived updates.
- A future dual build (web simulator + embedded) is enabled by shared IR and generator hooks.

---

## 0) Locked Decisions & Constraints
_User story: As a maintainer, I want clear boundaries so the integration remains file‑generation only and aligned with existing crates._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [x] | Creator remains file‑generation only (no runtime execution). | policy | Outputs Rust/JS/CSS/config only. |
| [x] | No new crates for this alignment phase. | workspace | Add modules under `src/bin/creator/`. |
| [x] | Primary direction is **Shared Design System** (B), with prototyping (A) later. | product | Tokens first, UI authoring second. |
| [x] | Start with Option 5 (Svelte → WASM → rlvgl renderer) design, but only generate files and hooks. | architecture | Runtime work deferred. |
| [x] | Provide hooks for dual build (Option 4) early; deliver it later. | architecture | IR and manifests must support both. |

---

## 1) CLI Surface: New `svelte` Command
_User story: As a developer, I can run explicit creator commands to generate token outputs, component targets, and glue code from Svelte sources._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Add `rlvgl-creator svelte` top-level command with subcommands. | clap | New command family. |
| [ ] | `svelte tokens` — read token YAML and emit web + rlvgl outputs. | serde_yaml | Emits CSS/Tailwind + Rust. |
| [ ] | `svelte compile` — compile `.svelte` to IR and emit rlvgl widget Rust. | Svelte parser/CLI | File-only output. |
| [ ] | `svelte wasm` — emit renderer glue and build configs for Svelte→WASM→rlvgl. | templates | Generates shims only. |
| [ ] | `svelte schema` — emit JSON schema for tokens and UI IR. | schemars | Editor support. |
| [ ] | `svelte check` — validate tokens + Svelte subset constraints. | creator core | Non-zero exit on violations. |

---

## 2) Shared Token Layer
_User story: As a designer, I define tokens once and consume them on web and embedded targets consistently._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Define `shared-tokens.yaml` schema (colors, spacing, radii, typography, motion). | schemars | Colors allow hex/rgb/rgba; spacing/radii are px; motion is ms + easing tokens. |
| [ ] | Add base + semantic token layers with optional modes (light/dark/high-contrast). | creator core | V1 uses a single mode; if multiple exist, use default/first or require explicit selection. |
| [ ] | Allow token aliases with cycle detection. | creator core | Error on circular refs. |
| [ ] | Normalize token names into deterministic identifiers. | creator core | Case policy + prefix map. |
| [ ] | Define token reference syntax for UI sources and generated code. | docs | Use `token("colors.primary")` in Svelte sources. |
| [ ] | Emit normalized `tokens.json` for IR consumers. | serde_json | Canonical token map for compilers. |
| [ ] | Generate CSS custom properties output (`tokens.css`). | templates | Output for Svelte/web. |
| [ ] | Generate Tailwind config snippet (`tailwind.tokens.cjs`). | templates | Optional integration. |
| [ ] | Generate `rlvgl-ui` theme Rust module (`theme.rs`). | templates | `Theme`/`Palette` structs. |
| [ ] | Add manifest section for token provenance and versioning. | manifest | Track source + hash. |

---

## 3) Svelte Component IR (Subset)
_User story: As a developer, I can author a constrained Svelte component that maps cleanly to embedded UI output._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Define Svelte subset rules (no DOM APIs, no `{@html}`, limited slots). | docs | Allow default slot only; validate in `svelte check`. |
| [ ] | Define allowed blocks/directives (`{#if}`, `{#each}` keyed, `on:` events, `bind:`). | docs | No `{#await}`, no `use:`, no `transition:` yet. |
| [ ] | Define allowed tags/components (rlvgl-only tags, no raw HTML). | docs | Start with Button/Text/Image/Stack/Row/Column. |
| [ ] | Implement `.svelte` parsing to a creator IR (components, props, children, styles). | parser/CLI | Prefer external Svelte parser if needed. |
| [ ] | Define IR fields for dynamic bindings (token refs vs state refs). | IR | Distinguish static vs derived values. |
| [ ] | Map Svelte `style:` bindings to token references and rlvgl styles. | creator core | Tokens as source of truth. |
| [ ] | Normalize events (`on:click`, etc.) to rlvgl callbacks. | IR | Define handler signature rules. |
| [ ] | Serialize IR to JSON for future tooling. | serde_json | Enables dual build later. |

---

## 4) Svelte → rlvgl Target (Direction B)
_User story: As a developer, I can compile a Svelte component into an rlvgl widget tree with styles and events._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Build widget mapping table (Svelte tag → rlvgl widget). | docs | Start with Button, Text, Image, Stack, Row, Column. |
| [ ] | Define layout props mapping (size, padding, gap, align, justify). | rlvgl-ui | Ensure deterministic defaults. |
| [ ] | Generate Rust builder code for widget trees. | templates | Output only. |
| [ ] | Support style mapping (bg, padding, radius, font, color). | rlvgl-ui | Bind to token output. |
| [ ] | Emit component modules with stable public APIs. | templates | Match `rlvgl` conventions. |
| [ ] | Add tests that compile a sample Svelte file into Rust output. | tests | Golden snapshots. |

---

## 5) Svelte 5 Runes → rlvgl State Model
_User story: As a developer, I can map Svelte’s `$state`, `$derived`, `$effect` to embedded state primitives._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Define a minimal state IR (`State`, `Derived`, `Effect`). | creator core | File-only output. |
| [ ] | Map `$state` to `State<T>` and `$derived` to computed callbacks. | rlvgl-ui | Add or reuse state helpers. |
| [ ] | Define allowed script patterns (no async, no DOM, no external stores). | docs | Only runes + local functions. |
| [ ] | Define binding rules for `bind:` (e.g., `bind:value`, `bind:checked`). | docs | Map to state setters/getters. |
| [ ] | Define effect scheduling constraints for embedded targets. | docs | No async side effects; run on state change. |
| [ ] | Generate Rust modules for state wiring and callbacks. | templates | Bind to widget events. |
| [ ] | Add validation errors for unsupported Svelte reactivity patterns. | creator core | Helpful messages. |

---

## 6) Option 5 Hooks: Svelte → WASM → rlvgl Renderer
_User story: As a developer, I can generate the glue code needed to connect Svelte’s runtime to an rlvgl renderer, without creator executing anything._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Define a renderer API surface for Svelte runtime bindings. | docs | Create/update/remove nodes, set props, set styles, dispatch events. |
| [ ] | Generate Rust `wasm-bindgen` shims for renderer entrypoints. | templates | File output only. |
| [ ] | Emit JS glue that forwards DOM ops to renderer bindings. | templates | Svelte runtime adapter. |
| [ ] | Generate build config snippets (`Cargo.toml`, `package.json`) as templates only. | templates | No execution. |
| [ ] | Document supported Svelte features in WASM mode. | docs | Minimal viable subset. |

---

## 7) Dual Build (Option 4) — Planned Later
_User story: As a developer, I can build a web preview and embedded target from the same UI source._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Define a shared IR that can emit both web and rlvgl outputs. | IR | Reuse from section 3. |
| [ ] | Emit web preview output (Svelte + tokens) into a `preview/` bundle. | templates | Static files only. |
| [ ] | Add `svelte preview` command to generate preview bundle. | creator CLI | No dev server. |
| [ ] | Add manifest sections for preview bundles and output paths. | manifest | Track hashes for rebuilds. |

---

## 8) Integration Points in Creator
_User story: As a maintainer, I can integrate Svelte alignment without new crates and keep code modular._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Add `src/bin/creator/svelte.rs` module for CLI entry + orchestration. | creator core | Mirrors other commands. |
| [ ] | Add `src/bin/creator/svelte/` submodules: tokens, ir, compile, wasm, check. | internal | Keep modules small. |
| [ ] | Extend manifest with `svelte` config (tokens path, ui roots, outputs). | manifest | Track hashes for rebuilds. |
| [ ] | Reuse existing manifest and schema emit helpers. | creator core | Consistent patterns. |
| [ ] | Wire Svelte commands into UI menus later (post-CLI parity). | creator_ui | Optional follow-up. |

---

## 9) Validation, Tests, and Docs
_User story: As a maintainer, I can trust the generated outputs and understand the subset clearly._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Add golden snapshots for token outputs (CSS/Tailwind/Rust). | insta | Deterministic formatting. |
| [ ] | Add sample `.svelte` fixtures for compile tests. | tests | Keep minimal subset. |
| [ ] | Document the Svelte subset and mapping table. | docs | Constraints + examples. |
| [ ] | Add creator CLI reference entries for `svelte` subcommands. | docs | Update `docs/CREATOR-CLI.md`. |

---

## 10) Roadmap / Phases
_User story: As a planner, I can stage delivery to land value early and safely._

| Complete | Description | Dependencies | Notes |
|---|---|---|---|
| [ ] | Phase 1 – Token pipeline + schema + `svelte tokens`. | tokens | Immediate B value. |
| [ ] | Phase 2 – Svelte subset + IR + `svelte compile` to rlvgl. | parser | Direction B. |
| [ ] | Phase 3 – Runes mapping and state generation. | rlvgl-ui | Direction B. |
| [ ] | Phase 4 – Option 5 hooks (WASM renderer glue). | templates | File-only output. |
| [ ] | Phase 5 – Dual build preview bundle (Option 4). | preview | Direction A. |
