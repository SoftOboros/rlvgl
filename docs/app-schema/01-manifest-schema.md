<!--
01-manifest-schema.md - rlvgl Application Schema, Chapter 1: Manifest Grammar.
Status: DRAFT — not yet ratified. See §15 change log.
-->

**[← Prev](00-concepts.md) · [Index](README.md) · Next → (TBD)**

# Chapter 1 — Manifest Grammar (`app.yaml`, schema `rlvgl-app/v0`)

> **Status:** RATIFIED 2026-04-27 (see §15). Depends on
> [Chapter 0](00-concepts.md). `APP-01` execution PRs MAY cite this
> chapter as a frozen authority for the v0 manifest grammar.
>
> **Authority on undefined terms:** see [Chapter 0 §3
> glossary](00-concepts.md#§3-canonical-glossary). This chapter does
> not restate definitions.

## §0 Authority policy

This chapter is normative for the YAML grammar of `app.yaml` at schema
version `rlvgl-app/v0`. Field names, types, required-vs-optional, and
validation rules in §5 / §6 are binding on parsers and generators
claiming v0 conformance. RFC 2119 keywords carry their RFC meanings.

The grammar reconciles against, but does not own, these adjacent
schemas:

| Adjacent schema                            | Owner                                             |
| ------------------------------------------ | ------------------------------------------------- |
| Chip / board YAML                          | `chipdb/rlvgl-chips-<vendor>/db/{chips,boards}/`  |
| Asset class enum                           | `rlvgl-creator` asset pipeline                    |
| State-chart `.scxml` / verification vectors| External MCP state-chart generator (see [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)) |
| i18n bundle JSON                           | `i18n/locales/*.json`                             |
| Theme tokens                               | `chakra/` design-token export                     |

When this chapter and an adjacent schema disagree, the adjacent
schema wins for fields it owns; this chapter's job is to *cite by
reference*, not to redefine.

## §1 Purpose

Define the concrete YAML grammar of `app.yaml` so that:

1. A parser can validate any `app.yaml` deterministically against this
   chapter alone (with chipdb / pipeline cross-references resolved
   at validation time).
2. The reverse round-trip ([00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property))
   has a target shape — extracting a manifest from
   `examples/beetle-esp32c3/` produces a YAML that this grammar
   accepts.
3. Subsequent chapters (`02-generator-pipeline.md`, etc.) can name
   manifest fields by exact path without ambiguity.

## §3 Glossary additions

Only terms *not* defined in [00 §3](00-concepts.md#§3-canonical-glossary):

- **Manifest path** — *Owned by this chapter.* A relative POSIX-style
  path resolved from the directory containing `app.yaml`. Absolute
  paths and `..` traversals outside the manifest's parent are
  rejected.
- **Schema tag** — *Owned by this chapter.* The string value of the
  top-level `schema:` key, of shape `rlvgl-app/v<N>`. v0 is unstable
  and breaking-change-allowed; v1 will be the first stable tag.
- **Reference id** — *Owned by this chapter.* A string matching
  `^[a-z][a-z0-9-]*$` used for `name`, `assets[].id`, `screens[].id`.
  Maximum length 63 (matches cargo crate-name limit).
- **Board id** — *As defined in `chipdb/rlvgl-chips-<vendor>/db/boards/`;
  used without modification.* The basename (without `.yaml`) of a
  board file. snake_case, not kebab-case — matches the chipdb file
  naming convention.

## §4 Source-of-truth additions

Rows added by this chapter on top of [00 §4](00-concepts.md#§4-source-of-truth-map):

| Concept                      | Owner (canonical)              | Mirrored / consumed by                    |
| ---------------------------- | ------------------------------ | ----------------------------------------- |
| Manifest grammar             | This chapter §5                | parser, generator, validator              |
| Schema-version negotiation   | This chapter §5.1              | parser rejects unknown major versions     |
| Reference id format          | This chapter §3                | every `id:` field in the manifest         |
| Validation rule set          | This chapter §6                | validator                                 |

## §5 Frozen decisions — grammar

> *Proposed for freeze. Not ratified — see §15.*

### 5.1 Top-level shape

```yaml
schema:         <string>      # required, MUST equal "rlvgl-app/v0"
name:           <ref-id>      # required, kebab-case, becomes cargo crate name
target:         <Target>      # required, see §5.2
controller:     <Controller>  # optional, see §5.10
state_machine:  <StateMachine># optional, see §5.3
assets:         [<Asset>]     # optional, default []
screens:        [<Screen>]    # optional, default []
theme:          <Theme>       # optional
i18n:           <I18n>        # optional
metadata:       <Metadata>    # optional, free-form authoring info
```

**Unknown top-level keys** at schema `rlvgl-app/v0` MUST be rejected
as errors. v1 MAY introduce reserved-key forward-compatibility; v0
does not.

### 5.2 `target` (required)

```yaml
target:
  vendor:   <string>      # required, MUST be in chipdb vendor enum
  board:    <board-id>    # required, MUST exist as
                          #   chipdb/rlvgl-chips-<vendor>/db/boards/<board>.yaml
  prong:    <string>      # required, one of:
                          #   linux | bare_metal | freertos | zephyr
  chip:     <string>      # optional, override; default = board's declared chip
  features: [<string>]    # optional, cargo features to enable on generated crate
  generator: <string>     # optional, one of:
                          #   creator-bsp-pac (default) — chipdb-driven BSP gen,
                          #     emits src/bsp_generated/ from board YAML.
                          #   hosted — upstream HAL crate provides the BSP
                          #     (esp-hal, embassy-stm32, ...). target.features
                          #     selects which HAL. Pipeline emits no BSP code.
                          #   hand_written — rlvgl platform crate provides the
                          #     BSP. Allow-list in §5.6.
```

**Validation:**

- `vendor` MUST appear in the chipdb crate set
  ([00 §5.2](00-concepts.md#52-vendor-set--standards-action-delegates-to-chipdb)).
- `board` MUST be a file basename in
  `chipdb/rlvgl-chips-<vendor>/db/boards/`. **This applies regardless
  of `generator`** — even `hosted` and `hand_written` boards MUST
  have a chipdb board entry. Minimal entry requirements: `chip`,
  `console: { peripheral, baud }`. Pin assignments and detailed
  peripheral config MAY be omitted for non-`creator-bsp-pac`
  generators (the manifest pipeline does not consume them, though
  other tools in the workspace may).
- `prong` MUST be in the §5.1 frozen set in chapter 0.
- If `chip` is provided, it MUST equal the chip declared in the
  board YAML (override is for *future* support of multi-chip boards;
  v0 enforces equality).
- `generator: hand_written` requires the board id to appear in the
  allow-list in §5.6.
- `generator: hosted` does NOT require allow-listing; the manifest
  validator MUST verify that `target.features` includes at least
  one feature flag whose name matches a known upstream HAL pattern
  (e.g. `esp_hal`, `embassy_*`). Specific patterns are pipeline-side
  knowledge, not manifest-side.

### 5.3 `state_machine` (optional)

```yaml
state_machine:
  source:               <manifest-path>  # required if state_machine: present
  generator:            <string>         # required, "mcp-statechart" only in v0
  verification_vectors: <bool>           # optional, default true
```

**Validation:**

- `source` MUST resolve to a regular file with extension `.scxml` or
  `.uml` (case-insensitive).
- `generator` MUST be `"mcp-statechart"` in v0. Adding generators is
  a Standards Action ([00 §5](00-concepts.md#§5-frozen-decisions--enums--registration-policy)).
- The manifest does NOT define states inline. This is the resolved
  decision from
  [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)
  — Option A. Re-opening this requires a §15 amendment in chapter 0.

### 5.4 `assets[]` (optional)

```yaml
assets:
  - id:          <ref-id>          # required, unique within the assets list
    class:       <asset-class>     # required, one of [00 §5.3] enum values
    source:      <manifest-path>   # required
    palette_ref: <ref-id>          # optional, MUST reference another asset of class "palette"
    options:     <map>             # optional, free-form per-class options
                                   #   passed verbatim to creator pipeline
```

**Validation:**

- `id` matches `^[a-z][a-z0-9-]*$`, unique among `assets[].id`.
- `class` MUST be in the [00 §5.3](00-concepts.md#53-asset-class-set--specification-required) enum.
- `source` MUST resolve to a regular file under the manifest's
  parent directory.
- `palette_ref`, if present, MUST resolve to an asset whose
  `class` is `palette`. Cycles (palette referencing itself) are
  errors.
- `options:` is opaque to the manifest validator — the pipeline
  validates per-class. Manifest validator MUST preserve key order
  for reproducibility.

#### 5.4.1 Common `options:` keys (informative, non-normative)

The following keys are *commonly* understood by the asset pipeline.
Per-class authoritative documentation lives in the pipeline's own
docs, not here. This table exists so manifest authors have a
starting point.

| Asset class               | Common `options:` keys                                                  |
| ------------------------- | ----------------------------------------------------------------------- |
| `image_rgb565`            | `orientation` (`none` / `rot90_cw` / `rot90_ccw` / `rot180`), `dither` |
| `image_rle_a8`            | `orientation`, `palette_quantize` (count), `dither`                     |
| `palette`                 | `format` (`act` / `gpl` / `json`), `count`                              |
| `font`                    | `size_px`, `glyph_set` (subset spec), `bpp`                             |
| `audio_pcm`               | `sample_rate`, `bit_depth`, `channels`                                  |
| `audio_lufs_capture`      | `target_lufs`, `gating` (`relative_bs1770` / `strict` / `none`)         |
| `icon`                    | `size_px`, `palette_ref` style override                                 |

Adding or modifying a key here is **Specification Required**
(per [00 §5](00-concepts.md#§5-frozen-decisions--enums--registration-policy))
in the asset pipeline's docs, not in this chapter. This table is
informative — manifest validators MUST NOT reject unknown
`options:` keys.

### 5.5 `screens[]` (optional)

```yaml
screens:
  - id:            <ref-id>          # required, unique within screens
    state:         <string>          # optional, MUST match a state id
                                     #   in the state-machine if state_machine: present
    layout:        <manifest-path>   # required
    layout_format: <string>          # required, one of [figma_export_v1, uml_widget_v1, rust_inline_v1]
    default:       <bool>            # optional, default false
```

**Validation:**

- Exactly one of:
    - `state_machine` is present AND every screen with a `state` field
      references a state id known to the SM (validated post-SM-gen), OR
    - `state_machine` is absent AND exactly one screen has
      `default: true`.
- If `state_machine` is present, screens MAY omit `state` to be
  *modeless* (e.g. an always-available overlay); the generator emits
  these as siblings of the state-routed root.
- `layout_format` of `rust_inline_v1` means `layout:` points at a
  `.rs` source fragment; the generator includes it directly. Reserved
  for round-trip targets where a layout authoring tool isn't yet
  in scope. **Removal in v1 is conditional** on a real layout
  authoring pipeline (e.g. `figma_export_v1` or `uml_widget_v1`)
  shipping first — round-trip evidence
  ([03 §6.10](03-round-trip.md#610--rust_inline_v1-is-doing-more-work-than-expected))
  shows it is the primary path today, not the backdoor it was
  framed as. Don't remove the backdoor before there's a real front
  door.

### 5.6 `target.generator: hand_written` allow-list

The set of boards for which the generated app crate calls into a
hand-written `platform/` module instead of creator-emitted BSP code.
Standards Action; current set:

- `stm32h747i_disco` ([00 §10.1](00-concepts.md#101-bsp-generator-vs-hand-written-h747-platform))
- `beaglebone_black_nhd_cape` (Linux prong; "BSP" is the kernel's
  fbdev + evdev surface accessed via `rlvgl-platform/linux_fbdev` —
  no chipdb-driven bring-up applies)

Adding a board to this list requires a §15 amendment in this
chapter and a corresponding hand-written platform module that
implements the BSP trait surface from `platform/`.

`target.generator: hosted` is **not** allow-listed — any board MAY
be hosted by an upstream HAL crate if such a HAL exists for its
chip family. The manifest names the HAL via `target.features`; the
pipeline maps the feature flag to the corresponding HAL crate
dependency.

### 5.7 `theme` (optional)

```yaml
theme:
  source: <manifest-path>  # required if theme: present
  format: <string>         # required, one of [chakra_tokens_v1, raw_palette_v1]
```

`chakra_tokens_v1` consumes the JSON output of a chakra
`extendTheme(...)` call — the same object shape a `ChakraProvider`
loads at runtime (`colors`, `space`, `fontSizes`, `radii`,
`shadows`, etc.). This is a working path: the softoboros site's
chakra theme has already been exported to the STM32H747I-DISCO
via `rlvgl-creator`, which is the reference implementation behind
this format tag (see [00 §10.4](00-concepts.md#104-theme--chakra--figma-overlap)).

`raw_palette_v1` is a flat `{ color_name: "#rrggbb" }` map for
minimal apps without a chakra-side authoring path.

A Svelte-side token export exists in the workspace as an alternate
source-of-record but is **not** in the v0 `format` enum. Adding
`svelte_tokens_v1` is a future Standards Action; chakra is the
v0 canonical theme source.

### 5.8 `i18n` (optional)

```yaml
i18n:
  bundle_dir:     <manifest-path>  # required if i18n: present
                                   # MUST be a directory of <locale>.json files
  default_locale: <string>         # required, e.g. "en", "en-US", "fr"
  locales:        [<string>]       # optional, additional locales to compile in
                                   # default: all *.json files in bundle_dir
  format:         <string>         # required, "rlvgl_i18n_v1" only in v0
                                   # matches i18n/locales/*.json shape
```

**Validation:**

- `bundle_dir` MUST be a directory.
- `<locale>.json` files in `bundle_dir` MUST have keys conforming
  to the rlvgl `i18n` crate's key format (dotted lowercase, e.g.
  `demo.title`, see `i18n/locales/en.json`).
- `default_locale.json` MUST exist in `bundle_dir`.
- Locale codes are accepted in either short form (`en`) or BCP-47
  (`en-US`); the format is the union of what `i18n/` accepts.

### 5.9 `metadata` (optional, free-form)

```yaml
metadata:
  version:     <string>     # optional, semver — emitted as cargo package version
  authors:     [<string>]   # optional
  license:     <string>     # optional, SPDX expression
  description: <string>     # optional, short
  # Other keys are accepted and passed through to cargo manifest emission.
```

The validator MUST accept any keys here. Generator behaviour for
unknown keys: pass through to `Cargo.toml` `[package.metadata]` if
the value is a string, scalar, or simple list; reject nested
structures (those would need a per-key schema, out of v0 scope).

### 5.10 `controller` (optional)

```yaml
controller:
  crate:        <string>           # required if controller: present, cargo crate name
  path:         <manifest-path>    # optional, path-dep override (sibling crate in workspace)
  version:      <string>           # optional, registry version requirement (semver)
  capabilities: <string>           # optional, named preset interpreted by the controller
                                   #   crate itself (e.g. "stm32h747i_disco", "simulator")
  features:     [<string>]         # optional, cargo features to enable on the controller
```

A **controller library** is a hand-written rlvgl crate that
implements the [00 §7](00-concepts.md#§7-frozen-decisions--wiring-contract-sketch)
wiring contract's `App::tick` body. It is *not* generated from a
manifest — round-trip evidence
([03 §6.2](03-round-trip.md#62--controller-libraries-need-first-class-manifest-support))
shows that real applications are typically thin wiring shims around
a shared controller crate (canonical example:
`rlvgl-app-disco-demo`, consumed by every H747, BBB, and
simulator binary).

**Validation:**

- If `controller:` is present, `crate` is required.
- `path` and `version` are mutually exclusive. Specifying neither
  means "resolve from the workspace's default cargo registry."
- `path` MUST be a `<manifest-path>` (relative, no escape) that
  resolves to a directory containing a `Cargo.toml`.
- `version`, if present, MUST be a valid semver requirement
  (`"0.2.0"`, `"^0.2"`, `">=0.1, <0.3"`).
- `capabilities` is a free-form string. The manifest validator
  does NOT validate it against the controller crate's actual
  capability set — the crate may not be on disk during validation,
  and capability presets are owned by the controller crate's API.
  An unknown `capabilities` value surfaces as a build-time error
  in the generated `app.rs`, not a manifest-time error.
- `features` are passed through verbatim to the emitted
  `Cargo.toml` `[dependencies]` line for the controller crate.

**Pipeline behaviour:** the [chapter 2](02-generator-pipeline.md)
orchestrator emits a `[dependencies]` entry for the named crate,
and the scaffold's `app.rs` emits an `App::new` body that
constructs the controller with the given `capabilities` preset. See
[02 §7.8](02-generator-pipeline.md#78-controller-wiring-contract).

**Reference example** (snippet from the H747 FreeRTOS round-trip
manifest, [03 §5.3.2](03-round-trip.md#532-candidate-manifest--freertos-intent)):

```yaml
controller:
  crate: rlvgl-app-disco-demo
  path: ../apps/disco-demo        # workspace path-dep
  capabilities: stm32h747i_disco  # selects DiscoCapabilities::stm32h747i_disco()
```

## §6 Validation rule set (normative)

A conforming v0 validator MUST enforce, in this order:

1. **Schema tag** — `schema: rlvgl-app/v0`. Unknown tag → reject.
2. **Required top-level keys present** — `name`, `target`.
3. **Reference id format** — every `id:` field matches the §3 regex.
4. **Path safety** — every `<manifest-path>` resolves under the
   manifest's parent directory; absolute paths and parent-directory
   escapes are rejected.
5. **Cross-references resolve**:
   - `target.vendor` is a known chipdb vendor.
   - `target.board` exists in chipdb (see §5.2 for minimal-entry
     rule when `target.generator` is `hosted` or `hand_written`).
   - `target.generator` is in the §5.2 enum.
   - `target.prong` is in the frozen prong set.
   - `assets[].class` is in the frozen asset class set.
   - `assets[].palette_ref` resolves to a palette asset.
   - `screens[].layout_format` is in the supported set.
   - `controller`, if present, has `crate` set and at most one of
     `path` / `version`. `path`, if present, resolves under the
     manifest's parent.
6. **Default-screen invariant** — exactly one default screen iff
   no `state_machine`.
7. **Unknown top-level keys** — reject.

Fields validated post-generation (NOT by the manifest validator):

- `screens[].state` against the SM's emitted state set.
- `assets[].source` content (the pipeline validates format).
- `theme.source` content.
- `i18n` key-set agreement across locales (the i18n crate validates
  this with its existing `extract_keys.py` workflow).

## §7 Reference example — minimal

```yaml
schema: rlvgl-app/v0
name: blinky-led

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  features: [esp_hal]

screens:
  - id: only-screen
    layout: layouts/only.rs
    layout_format: rust_inline_v1
    default: true
```

Validates without a chipdb, asset pipeline, SM generator, or i18n.
Round-trips against `examples/beetle-esp32c3/` (esp_hal feature) at
the wiring level.

## §8 Reference example — full

```yaml
schema: rlvgl-app/v0
name: disco-demo

metadata:
  version: 0.2.0
  license: MIT OR Apache-2.0
  description: "rlvgl reference demo on STM32H747I-DISCO."

target:
  vendor: stm
  board: stm32h747i_disco
  prong: freertos
  generator: hand_written     # see §5.6
  features: [cm7, freertos, adapted_cmd, dma2d, splash, desktop]

state_machine:
  source: states/disco.scxml
  generator: mcp-statechart
  verification_vectors: true

assets:
  - id: ui-palette
    class: palette
    source: assets/ui.act
  - id: splash
    class: image_rle_a8
    source: assets/splash.png
    palette_ref: ui-palette
  - id: inter-16
    class: font
    source: assets/inter-16.ttf
  - id: cursor-tap
    class: audio_pcm
    source: assets/cursor.wav

screens:
  - id: home
    state: idle
    layout: layouts/home.figma.json
    layout_format: figma_export_v1
  - id: settings
    state: settings
    layout: layouts/settings.figma.json
    layout_format: figma_export_v1
  - id: toast-overlay
    layout: layouts/toast.figma.json
    layout_format: figma_export_v1

theme:
  source: themes/disco.tokens.json
  format: chakra_tokens_v1

i18n:
  bundle_dir: i18n/
  default_locale: en
  format: rlvgl_i18n_v1
```

## §9 Counter-examples (rejected)

Each MUST be rejected by a v0 validator; the column shows which §6
rule fires.

| Snippet                                                    | Rejected by §6 rule         |
| ---------------------------------------------------------- | --------------------------- |
| `schema: rlvgl-app/v1`                                     | 1 (unknown schema tag)      |
| `name: My_App`                                             | 3 (id format)               |
| `assets: [{ id: a, class: foo, source: x }]`               | 5 (unknown asset class)     |
| `assets: [{ id: a, class: image_rgb565, source: ../x }]`   | 4 (path escape)             |
| `target: { vendor: esp, board: nonexistent, prong: linux }`| 5 (board not in chipdb)     |
| `target: { vendor: esp, board: beetle_esp32c3, prong: vxworks }` | 5 (prong not in set)   |
| Two screens with `default: true`, no `state_machine`       | 6 (default-screen invariant)|
| Top-level key `runtime: { ... }`                           | 7 (unknown top-level key)   |

## §10 Reconciliation with adjacent schemas

Resolved here; resolutions become binding when §15 records ratification.

### 10.1 `target.board` vs. chipdb board YAML

The manifest cites the board *id*; chipdb owns every field beneath
it (pin map, console, flash size, etc.). v0 does NOT support
overriding chipdb fields from the manifest. If a board needs a
variant (e.g. different I2C frequency), the variant is a new
chipdb board id, not a manifest field.

### 10.2 `assets[].class` vs. creator pipeline tag enum

The class names in [00 §5.3](00-concepts.md#53-asset-class-set--specification-required)
MUST match `rlvgl-creator`'s emitted artifact tags exactly. When
the pipeline adds a tag, this manifest grammar MUST be amended in
the same PR (Specification Required).

### 10.3 `i18n.bundle_dir` vs. existing `i18n/locales/`

The existing repo i18n shape uses `<locale>.json` files in a flat
directory (e.g. `i18n/locales/en.json`). v0 adopts that shape
verbatim; `format: rlvgl_i18n_v1` is the formal name for it. The
manifest does NOT introduce a new on-disk layout.

### 10.4 `theme.format: chakra_tokens_v1` vs. live chakra tokens

The chakra-to-rlvgl path is already in production: the softoboros
site's chakra theme has been exported to the STM32H747I-DISCO via
`rlvgl-creator`. `chakra_tokens_v1` formalizes that working path —
the consumed artifact is a JSON file at `theme.source`, equivalent
to the result of serialising `extendTheme(...)` to JSON.

The manifest does NOT pull from a live chakra runtime — there is
no Next.js dev server in the build path of an embedded app. A
build-time live consumer (e.g. a chakra exporter that pulls from
the running site) is a v1 concern; v0 consumes the static JSON
artifact only.

The Svelte-side conversion that also exists in the workspace is
recognised but not canonical: chakra is more internally consistent
across the authoring stack, so the v0 `format` enum admits only
`chakra_tokens_v1` and `raw_palette_v1`. `svelte_tokens_v1` MAY
be added in a future Standards Action if the Svelte path becomes
load-bearing for a downstream app.

### 10.5 `state_machine.source` vs. external MCP repo

The MCP state-chart generator lives outside this repo. The
manifest references the *input* (the `.scxml`/`.uml` file, which
lives in *this* repo's tree); the *generator* runs as an external
tool that produces a sibling Rust crate. The manifest does NOT
specify how the generator is invoked — that's a build-time concern
covered in `02-generator-pipeline.md`.

## §11 Non-goals (this chapter)

- **JSON Schema emission.** A `.schema.json` may be derived from
  this chapter as a tool-aid, but the canonical grammar is this
  chapter's prose. Disagreements between a derived JSON Schema and
  this chapter are bugs in the schema, not in the manifest.
- **Schema migrations.** v0 → v1 migration tooling is out of scope.
  v0 manifests do not declare upgrade compatibility.
- **Multi-target manifests.** A single `app.yaml` names exactly one
  `target`. Multi-target builds are expressed as multiple manifests.
  Real-world cargo crates often host multiple intents (multiple
  binaries, mutually-exclusive features, libraries beside binaries);
  the manifest's job is the *intent*, not the cargo crate. See
  [03 §6.1](03-round-trip.md#61--the-one-appyaml-per-intent-rule-holds-but-reviewers-must-understand-it)
  for round-trip evidence.
- **Conditional sections.** No `cfg`-style conditionals on top-level
  keys in v0. If you need them, write multiple manifests.
- **Manifest inheritance (`extends:`).** Sibling manifests sharing
  most of their content (same controller, same assets, different
  prong) duplicate by copy in v0. Inheritance/include is rejected
  for v0 to keep override-vs-merge semantics out of scope. Revisit
  at v1 if duplication friction becomes load-bearing in practice
  ([03 §6.7](03-round-trip.md#67--sibling-manifests-with-shared-content)).

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [x] §5.1 top-level shape reviewed against
      [00 §6](00-concepts.md#§6-frozen-decisions--manifest-structure-sketch)
      strawman; deltas accepted (added `controller:` per
      [03 §6.2 closure](03-round-trip.md#62--closed--controller-libraries-get-a-first-class-manifest-slot)).
- [x] §5.3 confirms Option A from
      [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question);
      00's §15 records the same resolution.
- [x] §5.6 `hand_written` allow-list reviewed; H747 board + BBB
      Linux entry at v0 ratification (BBB added during the
      convergence pass).
- [ ] §6 validation rule set has a corresponding test fixture in
      a future `creator/` validator implementation (not blocking
      ratification — APP-02a tracking).
- [ ] §7 minimal example accepted by the validator (proof: round-trip
      target `examples/beetle-esp32c3/` per [00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property))
      — manifest landed via APP-03a; validator acceptance pending APP-02a.
- [x] §15 has a dated ratification entry signed off by the
      initiative owner.

## §13 Files cited

- [`docs/app-schema/00-concepts.md`](00-concepts.md) — chapter 0,
  authority for all undefined terms.
- [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml`](../../chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml)
  — example board YAML, cited by §5.2 and §10.1.
- [`i18n/locales/en.json`](../../i18n/locales/en.json) — i18n bundle
  format reference, cited by §5.8 and §10.3.
- [`platform/src/stm32h747i_disco.rs`](../../platform/src/stm32h747i_disco.rs)
  — the hand-written BSP, cited by §5.6.
- [`examples/beetle-esp32c3/`](../../examples/beetle-esp32c3/) —
  minimal-example round-trip target, cited by §7.
- [`examples/apps/disco-demo/`](../../examples/apps/disco-demo/) —
  full-example round-trip target, cited by §8.

## §14 Unblocks

Ratifying this chapter unblocks:

- `02-generator-pipeline.md` — how `rlvgl-creator app from-yaml`
  composes BSP / asset / SM generators against this grammar.
- `03-round-trip.md` — extracting an `app.yaml` from each [00
  §9](00-concepts.md#§9-frozen-decisions--round-trip-property)
  target.
- A v0 validator implementation under `rlvgl-creator` (not blocking,
  but the first concrete consumer).

## §15 Change log

| Date       | Status | Note                                                                                                |
| ---------- | ------ | --------------------------------------------------------------------------------------------------- |
| 2026-04-27 | DRAFT  | Initial grammar. Adopts Option A for state-machine boundary. v0 schema tag. Argument target — §5.5 / §5.8. |
| 2026-04-27 | DRAFT  | §5.3 / §5.7 resolved. Option A confirmed (matches 00 §15 same-day entry). `chakra_tokens_v1` reframed: working path, not aspirational — softoboros chakra theme already exported to 747i-disco via `rlvgl-creator`. Svelte alternate noted, not in v0 enum. §5.5 `rust_inline_v1` v0-only backdoor confirmed for removal in v1. §12 checklist still incomplete; chapter remains DRAFT. |
| 2026-04-27 | DRAFT  | Round-trip convergence pass closed. Grammar additions: §5.10 `controller:` field (closes [03 §6.2](03-round-trip.md#62--closed--controller-libraries-get-a-first-class-manifest-slot)); §5.2 `target.generator: hosted` value + chipdb minimal-entry rule (closes [03 §6.3](03-round-trip.md#63--closed--targetgenerator-gets-a-third-value-hosted) and [03 §6.4](03-round-trip.md#64--closed--chipdb-minimal-entry-rule-for-non-creator-bsp-pac-boards)); §5.4.1 informative `options:` keys table (closes [03 §6.6](03-round-trip.md#66--closed--common-options-keys-catalogued-informative)); §5.5 explicit v1 gating note for `rust_inline_v1` (closes [03 §6.10](03-round-trip.md#610--closed--rust_inline_v1-v1-removal-is-conditional)); §5.6 expanded with `beaglebone_black_nhd_cape` allow-list entry. §6 validation rule 5 extended for `controller`. §11 amended: explicit "intent vs. crate" language ([03 §6.1](03-round-trip.md#61--closed--the-one-appyaml-per-intent-rule-holds)) and explicit no-`extends:` decision ([03 §6.7](03-round-trip.md#67--closed--sibling-manifests-duplicate-by-copy-at-v0)). §12 checklist still incomplete; chapter remains DRAFT pending validator implementation and round-trip target check-in. |
| 2026-04-27 | RATIFIED | Owner: Ira Abbott. v0 manifest grammar (`rlvgl-app/v0`) frozen. §6 validator-test-fixture and §7 minimal-example items remain unchecked but are explicitly non-blocking per §12 (tracked under APP-02a / APP-03a). `APP-NN` execution PRs may now cite this chapter as a frozen authority for grammar, validation rules, and field semantics. Future grammar amendments require a new dated entry and matching review depth. |
