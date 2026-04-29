<!--
03-round-trip.md - rlvgl Application Schema, Chapter 3: Round-Trip Targets.
Status: DRAFT — not yet ratified. See §15 change log.
-->

**[← Prev](02-generator-pipeline.md) · [Index](README.md) · Next → (TBD)**

# Chapter 3 — Round-Trip Targets

> **Status:** DRAFT, unratified. Depends on
> [Chapter 0](00-concepts.md), [Chapter 1](01-manifest-schema.md),
> [Chapter 2](02-generator-pipeline.md). Until §15 records a
> ratified entry, no `APP-03` PR may cite this doc as a frozen
> authority.
>
> **Purpose:** verify the v0 schema and pipeline against reality
> before they are frozen. The round-trip property declared in
> [00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property) —
> "reverse-engineering an existing example into a manifest, then
> emitting from that manifest, MUST produce a crate that builds and
> passes its own pre-publish phases" — is unverified until this
> chapter exists.

## §0 Authority policy

This chapter is normative for:

- the **finding-level conclusions** in §6 (gaps and amendments
  needed in 00/01/02),
- the **per-target manifests** in §5 *until* they are checked into
  the round-trip target trees themselves (after which the
  in-tree `app.yaml` is canonical and this chapter cites it).

This chapter is *not* normative for:

- changes to the chapter 1 grammar — proposed amendments here flow
  back into 01 §15 entries.
- changes to the chapter 2 pipeline — same flow.

A finding that the v0 schema cannot express something the example
needs is a *valid* outcome of this chapter. The whole point of
running the round-trip *before* freeze is to surface such gaps.

## §1 Purpose

For each [00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property)
target:

1. Inventory what the existing crate actually contains.
2. Write a candidate `app.yaml` against the
   [chapter 1 grammar](01-manifest-schema.md).
3. Identify the gaps — places where the v0 grammar cannot represent
   the example, or where the [chapter 2 pipeline](02-generator-pipeline.md)
   would need to do something it does not currently describe.
4. File the gaps as proposed amendments to the upstream chapters.

The chapter's value is the gap list (§6), not the manifests
themselves. The manifests are scaffolding for surfacing the gaps.

## §3 Glossary additions

- **Round-trip extraction** — *Owned by this chapter.* The act of
  reading an existing example and producing a candidate `app.yaml`
  that the chapter 2 pipeline could plausibly emit it from.
- **Intent-level round-trip** — *Owned by this chapter.* Round-trip
  at the granularity of one `(target, prong)` pair, not one cargo
  crate. The existing examples often host multiple intents in a
  single `Cargo.toml`; the manifest grammar deliberately does not
  ([01 §11](01-manifest-schema.md#§11-non-goals-this-chapter)).
- **Controller library** — *Owned by this chapter.* A hand-written
  rlrgl crate that implements the wiring contract's `App::tick`
  body but is *not* generated from a manifest. `rlvgl-app-disco-demo`
  is the canonical example. Manifests reference controller libraries
  via `Cargo.toml` deps; the manifest does not generate them.

## §4 Source-of-truth additions

| Concept                                | Owner (canonical)                                        |
| -------------------------------------- | -------------------------------------------------------- |
| Candidate manifest, beetle-esp32c3     | This chapter §5.1 (until in-tree `app.yaml` lands)       |
| Candidate manifest, BBB Linux          | This chapter §5.2 (until in-tree `app.yaml` lands)       |
| Candidate manifest, H747 FreeRTOS      | This chapter §5.3 (until in-tree `app.yaml` lands)       |
| Candidate manifest, H747 Zephyr        | This chapter §5.4 (until in-tree `app.yaml` lands)       |
| Cross-target gap list                  | This chapter §6                                          |
| Controller-library boundary            | This chapter §3, §6.2                                    |

## §5 Per-target round-trips

Each subsection has the same shape: **Inventory** of the existing
crate, **Candidate manifest**, **Findings** (gaps and notes).

### 5.1 `examples/beetle-esp32c3/` — esp_hal binary (bare-metal)

#### 5.1.1 Inventory

- Cargo crate: `rlvgl-example-beetle-esp32c3`.
- **Two binaries** in one `Cargo.toml`, mutually exclusive features:
    - `rlvgl-beetle-esp32c3` (`src/esp_hal_main.rs`, requires
      `esp_hal` feature) — the working SSD1306 demo.
    - `rlvgl-beetle-esp32c3-bsp-pac` (`src/bsp_pac_main.rs`,
      requires `bsp_pac` feature) — LED blink proving the BSP-gen
      pipeline.
- BSP source: hand-wired esp-hal calls (esp_hal binary) OR
  `src/bsp_generated/` files copied from
  `rlvgl-creator bsp from-yaml --vendor esp --board beetle_esp32c3`
  (bsp_pac binary).
- No state machine, no theme, no i18n, no assets.
- Display: SSD1306 128×64 mono OLED over I2C0 at 400 kHz.

The two binaries are **two intents** sharing a `Cargo.toml` for
convenience. The manifest grammar deliberately separates them
([01 §11](01-manifest-schema.md#§11-non-goals-this-chapter)) — see
finding §6.1.

#### 5.1.2 Candidate manifest — `esp_hal` intent

```yaml
# beetle-esp32c3-eh.app.yaml
schema: rlvgl-app/v0
name: rlvgl-beetle-esp32c3-eh

metadata:
  description: "rlvgl SSD1306 demo on DFR0868 Beetle ESP32-C3 (esp_hal path)."

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: hand_written          # see finding §6.3 — no chipdb hand_written
                                    # entry exists for ESP32-C3 today; would
                                    # need an addition to [01 §5.6] allow-list,
                                    # OR a new generator value "esp_hal_hosted".
  features: [esp_hal]

screens:
  - id: only-screen
    layout: layouts/only.rs
    layout_format: rust_inline_v1
    default: true
```

#### 5.1.3 Candidate manifest — `bsp_pac` intent

```yaml
# beetle-esp32c3-pac.app.yaml
schema: rlvgl-app/v0
name: rlvgl-beetle-esp32c3-pac

metadata:
  description: "Raw-PAC LED blink proving chipdb→generator→boot pipeline."

target:
  vendor: esp
  board: beetle_esp32c3
  prong: bare_metal
  generator: creator-bsp-pac       # default; matches existing src/bsp_generated/
  features: [bsp_pac]

screens:
  - id: led-blink
    layout: layouts/led_blink.rs
    layout_format: rust_inline_v1
    default: true
```

#### 5.1.4 Findings

- ✅ Both intents express in v0 grammar **modulo** the
  `target.generator: hand_written` allow-list issue (§6.3).
- ⚠️ Round-trip is *intent-level*, not *crate-level*. The existing
  `Cargo.toml` hosts both binaries; round-tripping produces two
  manifests that the pipeline emits into two separate output
  directories. Re-merging them into a single `Cargo.toml` is *not*
  a v0 pipeline responsibility.
- ⚠️ `layout_format: rust_inline_v1` is doing real work here —
  these binaries have no Figma/UML layout source, just hand-rolled
  Rust calls into `rlvgl_core` / `esp_hal`. v1's plan to remove
  this format ([01 §5.5](01-manifest-schema.md#55-screens-optional))
  needs to land alongside a real layout authoring path or it
  blocks ESP32-class examples.

### 5.2 `examples/beaglebone-black/` — Linux binary

#### 5.2.1 Inventory

- Cargo crate: `rlvgl-example-bbb`.
- **Multiple intents** in one `Cargo.toml`:
    - `rlvgl-bbb` binary (Linux, default features).
    - `rlvgl-bbb-bare` binary (bare-metal, `bare_metal` feature).
    - FreeRTOS and Zephyr paths gated by features
      (`freertos = ["bare_metal"]`, `zephyr = ["bare_metal"]`).
- Default features: `[linux, splash, desktop, playit, star_crawl]`.
- Controller: pulled from `rlvgl-app-disco-demo` (the shared
  controller library, see §6.2).
- Asset: splash image (decoded by `rlvgl-decomp`).
- Display: kernel `tilcdc` fbdev `/dev/fb0`, ARGB8888 800×480 with
  90° CW rotation from a 480×800 portrait splash.
- Touch: `edt-ft5x06` driver via `/dev/input/eventN` (currently
  RMA'd; `playit` feature provides a TCP loopback fallback per
  `RMA-newhaven-2026-04-22.md`).

#### 5.2.2 Candidate manifest — Linux intent

```yaml
# bbb-linux.app.yaml
schema: rlvgl-app/v0
name: rlvgl-bbb-linux

metadata:
  description: "rlvgl on BeagleBone Black + NHD-7.0CTP-CAPE-P (Linux fbdev)."

target:
  vendor: ti
  board: beaglebone_black_nhd_cape  # see finding §6.4 — chipdb board file MAY not exist yet
  prong: linux
  generator: hand_written           # BBB Linux uses platform/linux_fbdev,
                                     # no chipdb-driven BSP gen for Linux prong
  features: [linux, splash, desktop, playit, star_crawl]

assets:
  - id: splash
    class: image_rle_a8              # see finding §6.5 — actual format may differ;
                                     # rlvgl-decomp consumes whatever creator emits
    source: assets/splash.bin
    options:
      orientation: rot90_ccw         # see finding §6.6

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1    # uses DiscoController, no Figma source
    default: true

# CANNOT EXPRESS in v0:
#   - Dependency on the rlvgl-app-disco-demo controller library
#     (see finding §6.2). The manifest needs a `dependencies:` or
#     `controller:` section.
#   - The `playit` TCP-loopback fallback as a runtime feature flag
#     mapped to a manifest concept. Currently expressed only as a
#     cargo feature in `target.features`.
```

#### 5.2.3 Findings (BBB-specific)

- ⚠️ `target.generator: hand_written` for the **Linux prong**
  pushes the [01 §5.6 allow-list](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list)
  question hard. Linux prong has no chipdb-driven BSP at all — the
  "BSP" is the kernel's fbdev + evdev surface, accessed through
  `rlvgl-platform`'s `linux_fbdev` feature. The allow-list needs
  to grow or get a new category (§6.4).
- ⚠️ The bare-metal / FreeRTOS / Zephyr intents in the same
  `Cargo.toml` would be **three more manifests**, all referencing
  the same `rlvgl-app-disco-demo` controller library. Common
  content (assets, screens) duplicates across them. v0 has no
  include / inherit mechanism — see finding §6.7.

### 5.3 `examples/stm32h747i-disco/` — FreeRTOS binary

#### 5.3.1 Inventory

- Cargo crate: `rlvgl-example-disco`.
- **Three** primary build artifacts in one `Cargo.toml`:
    - `rlvgl-stm32h747i-disco` binary (CM7 core, requires `cm7`
      feature — covers both bare-metal and FreeRTOS paths via the
      `freertos` feature).
    - `rlvgl-stm32h747i-disco-cm4` binary (CM4 core, requires
      `cm4` feature; effectively idle).
    - `librlvgl_example_disco.a` staticlib (Zephyr path, requires
      `zephyr` feature; consumed by the C/CMake project at
      `examples/stm32h747i-disco/zephyr/`).
- Hand-written platform: `platform/src/stm32h747i_disco.rs`
  (already on the [01 §5.6 allow-list](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list)).
- Controller: `rlvgl-app-disco-demo`.
- Many features: `cm7, dma2d, splash, desktop, audio, qspi_flash,
  sd_storage, freertos, adapted_cmd, semihosting, ...`.

The FreeRTOS intent uses the same `cm7` binary as bare-metal,
selected by adding the `freertos` feature.

#### 5.3.2 Candidate manifest — FreeRTOS intent

```yaml
# disco-freertos.app.yaml
schema: rlvgl-app/v0
name: rlvgl-stm32h747i-disco-freertos

metadata:
  description: "rlvgl reference demo on STM32H747I-DISCO (FreeRTOS prong)."

target:
  vendor: stm
  board: stm32h747i_disco           # see finding §6.4 — chipdb stm board entry
                                     # required; may need to be added.
  prong: freertos
  generator: hand_written            # already allow-listed [01 §5.6]
  features:
    - cm7
    - freertos
    - adapted_cmd
    - dma2d
    - splash
    - desktop

assets:
  - id: splash
    class: image_rle_a8
    source: assets/splash.bin

screens:
  - id: home
    state: idle                      # disco-demo has implicit states
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true

# CANNOT EXPRESS in v0:
#   - The state set is implicit in DiscoController; no .scxml exists.
#     Either an SM is reverse-engineered (significant work) or this
#     manifest omits state_machine: entirely (also OK at v0 because
#     state_machine: is optional).
#   - The CM4 idle binary as a sibling intent — out of scope for
#     a single manifest.
#   - playit integration via the rlvgl-playit crate dep.
```

#### 5.3.3 Findings (H747 FreeRTOS-specific)

- ✅ The `target.generator: hand_written` lookup already exists
  for this board. This is the round-trip target the v0 allow-list
  was designed for.
- ⚠️ The implicit-state-machine question: `DiscoController` has
  states (Idle, Wing-open, etc.) baked into Rust match arms. v0
  schema treats `state_machine:` as optional, so the manifest
  legitimately omits it. **Finding:** the round-trip is honest —
  the target *has no .scxml today*, so the manifest correctly
  reflects that. Adding an SCXML would be a separate `APP-` PR.
- ⚠️ The feature list is large (~6 items). The
  pipeline emits `Cargo.toml` `[features]` from `target.features`
  — does it emit the *feature graph* or just the leaf set? Today
  the existing crate has features like `freertos = ["rlvgl-platform/freertos"]`
  (an enable-other-feature graph). Manifest-driven emission cannot
  reproduce that without a new field. See finding §6.8.

### 5.4 `examples/stm32h747i-disco/` — Zephyr binary (hybrid)

#### 5.4.1 Inventory

The Zephyr path is **structurally different** from the other three
targets:

- The Rust side is built as a `staticlib` (`crate-type =
  ["staticlib"]`, `required-features = ["zephyr"]`), producing
  `librlvgl_example_disco.a`.
- A Zephyr west project at `examples/stm32h747i-disco/zephyr/`
  contains:
    - `CMakeLists.txt` — links the Rust `.a` into the Zephyr app.
    - `prj.conf` — Zephyr Kconfig (display, MIPI-DSI, FT5336,
      memc, serial, log, SD).
    - `app.overlay` + `adapted_cmd.overlay` — devicetree overlays.
    - `src/main.c` — Zephyr `main()` that calls into the Rust
      static lib.
- Build flow: `cargo build --features cm7,zephyr,...`, then
  `west build` against the Zephyr app, which links the `.a`.

#### 5.4.2 Candidate manifest — Zephyr intent

```yaml
# disco-zephyr.app.yaml
schema: rlvgl-app/v0
name: rlvgl-stm32h747i-disco-zephyr

metadata:
  description: "rlvgl on STM32H747I-DISCO via Zephyr (hybrid Rust staticlib + west)."

target:
  vendor: stm
  board: stm32h747i_disco
  prong: zephyr
  generator: hand_written
  features:
    - cm7
    - zephyr
    - splash
    - desktop
    - dma2d

assets:
  - id: splash
    class: image_rle_a8
    source: assets/splash.bin

screens:
  - id: home
    layout: layouts/home.rs
    layout_format: rust_inline_v1
    default: true

# CANNOT EXPRESS in v0:
#   - The Zephyr west sibling project (CMakeLists.txt, prj.conf,
#     app.overlay, src/main.c). The pipeline emits a Rust crate;
#     the Zephyr prong needs an *additional* output tree. See
#     finding §6.9 — the most significant Zephyr finding.
#   - The crate-type = ["staticlib"] requirement vs. binary.
```

#### 5.4.3 Findings (Zephyr-specific)

- 🔴 **Major gap:** the [chapter 2 pipeline](02-generator-pipeline.md)
  emits one Rust Cargo crate. The Zephyr prong needs:
    1. A Rust staticlib crate.
    2. A sibling west project (CMakeLists.txt, prj.conf, overlays,
       a thin `main.c`).
  The §8.4 sketch in chapter 2 acknowledges this loosely ("plus the
  `prj.conf` and `CMakeLists.txt` Zephyr expects") but does not
  pin down where those files live in the emitted output. See
  finding §6.9.
- ⚠️ `crate-type = ["staticlib"]` is a per-prong cargo manifest
  setting that v0 does not expose. The pipeline needs to know that
  the Zephyr prong implies staticlib. Could be hardcoded in the
  per-prong template, but worth naming explicitly.

## §6 Cross-target findings

The §5 round-trips surfaced these gaps. Each is proposed as an
amendment back to the cited upstream chapter. All are **DRAFT**;
none are ratified.

### 6.1 ✅ CLOSED — The "one app.yaml per intent" rule holds

[01 §11](01-manifest-schema.md#§11-non-goals-this-chapter) deliberately
forbids multi-target manifests. Round-trip evidence supports this
choice — the cargo crates host multiple intents, but the intents
are independently buildable and reviewable. The manifest pinning
the *intent* (one prong, one binary) is the right granularity.

**Disposition: ACCEPT** as drafted. 01 §11 amended with explicit
"intent vs. crate" language.

### 6.2 🔴 CLOSED — Controller libraries get a first-class manifest slot

`rlvgl-app-disco-demo` is consumed by **at least four** of the
round-trip targets (BBB linux/bare/freertos/zephyr) plus the H747
freertos and zephyr binaries plus the simulator. It is hand-written
and will stay hand-written — it implements the wiring contract's
`App::tick` body in a way the manifest cannot generate.

**Disposition: ACCEPT.** Chapter 1 amended with a new top-level
[§5.10 `controller:` field](01-manifest-schema.md#510-controller-optional)
and chapter 2 with a [§7.8 controller wiring contract](02-generator-pipeline.md#78-controller-wiring-contract).
The pipeline emits a `[dependencies]` entry for the named crate
and an `App::new` body that constructs the controller with the
manifest-named `capabilities` preset.

### 6.3 🟡 CLOSED — `target.generator` gets a third value: `hosted`

The candidate manifests need three values:

- `creator-bsp-pac` (default; chipdb-driven, beetle bsp_pac case).
- `hand_written` (the [01 §5.6 allow-list](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list);
  H747 case).
- `hosted` — upstream HAL crate provides the BSP (esp-hal,
  embassy-stm32, ...).

beetle-esp32c3's `esp_hal` intent is `hosted` — it consumes
upstream `esp-hal` directly with no hand-written platform module.

**Disposition: ACCEPT.** Chapter 1 [§5.2](01-manifest-schema.md#52-target-required)
adds `hosted` as a third value with no allow-list (any board MAY
be hosted by an upstream HAL). The manifest's `target.features`
selects the HAL; the pipeline maps the feature flag to the HAL
crate dependency. Chapter 2 [§5.4](02-generator-pipeline.md#54-emitted-crate-layout)
specifies that `hosted` omits `src/bsp_generated/` and adds the
HAL to `[dependencies]`.

The "rename hand_written → external" alternative was rejected —
the distinction (rlvgl-curated platform module vs. upstream HAL
crate) matters to reviewers and to the §5.6 allow-list.

### 6.4 🟡 CLOSED — chipdb minimal-entry rule for non-`creator-bsp-pac` boards

The candidate manifests cite:

- `vendor: ti, board: beaglebone_black_nhd_cape` — does not exist
  in `chipdb/rlvgl-chips-ti/db/boards/`.
- `vendor: stm, board: stm32h747i_disco` — does not exist in
  `chipdb/rlvgl-chips-stm/db/boards/`.

Both are "BSP is hand-written" or "hosted" cases. The chipdb still
needs an entry to validate `target.board`.

**Disposition: ACCEPT.** Initial closure (2026-04-27) wrote the
rule against esp-shape-only assumptions; **superseded by the
2026-04-29 amendment to 01 §5.2** ([01 §15](01-manifest-schema.md#§15-change-log))
which replaces "file basename in `db/boards/`" with "resolves via
the vendor crate's `find()` API." The per-vendor backing-storage
map (esp YAML, stm zstd archive, ti `BOARDS` const, …) now lives
in 01 §5.2 itself.

**Follow-up work — landed 2026-04-29 (APP-04a):**

Both stm and ti vendor crates already drive `find()` from a
hardcoded `BOARDS: &[BoardInfo]` constant in `src/lib.rs`; the
zstd archive in stm is decorative for `find()` purposes (it
exposes raw board definitions to *other* tools via `raw_db()`,
not to the validator). Adding boards is therefore a one-line
const append per vendor:

- **ti** — `chipdb/rlvgl-chips-ti/src/lib.rs` `BOARDS` const adds
  `BoardInfo { board: "beaglebone_black_nhd_cape", chip: "AM335x" }`.
  Unblocks BBB linux/bare-metal/freertos/zephyr round-trip
  manifests.
- **stm** — `chipdb/rlvgl-chips-stm/src/lib.rs` `BOARDS` const
  adds `BoardInfo { board: "stm32h747i_disco", chip: "STM32H747XIH6" }`.
  Unblocks H747 freertos/zephyr round-trip manifests.

A future PR may regenerate `assets/chipdb.bin.zst` from
`chips/stm/STM32_open_pin_data/` so `raw_db()` reflects the same
board set, but that does not block manifest validation.

### 6.5 🟢 CLOSED — Asset class enum survives round-trip

All four targets use only assets in
[00 §5.3](00-concepts.md#53-asset-class-set--specification-required)
(splash → `image_rle_a8`). No new asset classes surfaced.

**Disposition: ACCEPT** — enum holds at v0, no amendment needed.

### 6.6 🟡 CLOSED — Common `options:` keys catalogued (informative)

The BBB Linux candidate has `options.orientation: rot90_ccw`. The
grammar ([01 §5.4](01-manifest-schema.md#54-assets-optional))
declares a free-form `options:` map per asset, but did not pin
down the keys.

**Disposition: ACCEPT.** Chapter 1 [§5.4.1](01-manifest-schema.md#541-common-options-keys-informative-non-normative)
added: a non-normative table of common `options:` keys per asset
class. Authoritative per-class documentation lives in the asset
pipeline's own docs (Specification Required); the table is a
starting point for manifest authors. Manifest validators MUST
NOT reject unknown `options:` keys.

### 6.7 🟡 CLOSED — Sibling manifests duplicate by copy at v0

BBB hosts four prongs; H747 hosts two prongs (freertos + zephyr)
sharing assets, screens, and controller. With §6.2 (controller
amendment) closed, the duplication shrinks — assets and screens
still duplicate, but the controller does not.

**Disposition: ACCEPT Option A** (no inheritance at v0). Chapter
1 [§11](01-manifest-schema.md#§11-non-goals-this-chapter) amended:
manifest inheritance (`extends:`) is explicitly out for v0.
Revisit at v1 if the duplication friction becomes load-bearing in
practice.

Rationale: inheritance grammar (override-vs-merge semantics, list
merging, key precedence) carries non-trivial design cost; v0
should not pay it speculatively when the duplication is bounded
(assets list + screens list per prong).

### 6.8 🟡 CLOSED — Per-prong templates own the Cargo feature graph

Real `Cargo.toml` features have graph structure:
`freertos = ["rlvgl-platform/freertos"]`. The manifest's
`target.features` is a flat list of leaf features.

**Disposition: ACCEPT.** Chapter 2 [§8 preamble](02-generator-pipeline.md#§8-per-prong-main-glue-normative-templates)
amended with the explicit ownership rule: the manifest names *what
the user wants enabled*; the per-prong template knows *what
enabling that means in the emitted Cargo.toml*. Feature-graph
expansion happens at scaffold-emit time. This matches chapter 2's
general principle that prong-specific knowledge lives in
templates.

### 6.9 🔴 CLOSED — Zephyr prong emits a nested west project

Chapter 2 emits one Cargo crate; the Zephyr prong needs both a
Rust staticlib and a sibling/nested west project.

**Disposition: ACCEPT** (nested layout). Chapter 2 amended in
two places:

- [§5.4.1](02-generator-pipeline.md#541-zephyr-prong-nested-west-project)
  declares the layout: a `zephyr/` subdirectory containing
  `CMakeLists.txt`, `prj.conf`, `app.overlay`, and `src/main.c`,
  alongside the Cargo crate (which becomes a `staticlib`).
- [§8.4](02-generator-pipeline.md#84-zephyr-prong) fleshes out
  the templates: `src/lib.rs` exports `rlvgl_init()`,
  `zephyr/src/main.c` calls it, `zephyr/CMakeLists.txt` links the
  staticlib, `zephyr/prj.conf` baseline + feature-graph extensions,
  `zephyr/app.overlay` baseline + conditional inclusions like
  `adapted_cmd.overlay`.

The nested layout matches the existing reference at
`examples/stm32h747i-disco/zephyr/`. The "sibling at `<out>-zephyr/`"
alternative was rejected — nested keeps the two trees co-located
in version control and matches existing repo precedent.

### 6.10 🟢 CLOSED — `rust_inline_v1` v1 removal is conditional

Three of four candidate manifests use `rust_inline_v1` for at
least one screen. The intent of the format
([01 §5.5](01-manifest-schema.md#55-screens-optional)) was as a
v0 backdoor to be removed in v1. Round-trip evidence shows it is
the *primary* path today — there is no Figma authoring pipeline
yet, so most layout source is hand-rolled Rust.

**Disposition: ACCEPT** with explicit gating. Chapter 1
[§5.5](01-manifest-schema.md#55-screens-optional) amended:
"removal in v1 is **conditional** on a real layout authoring
pipeline (e.g. `figma_export_v1` or `uml_widget_v1`) shipping
first. Don't remove the backdoor before there's a real front door."

### 6.12 🟡 OPEN — Zephyr `prj.conf` has hand-tuned values the template cannot reproduce

**Surfaced 2026-04-29 by APP-03d** (H747 Zephyr round-trip). The
existing `examples/stm32h747i-disco/zephyr/prj.conf` contains
hand-tuned values that derive neither from chipdb nor from
`target.features`:

- `CONFIG_MAIN_STACK_SIZE=16384` — tuned because the default 4 KB
  overflowed during star-crawl render (per the in-tree comment).
- `CONFIG_HEAP_MEM_POOL_SIZE=65536` — same family of memory tuning.
- `CONFIG_INPUT_FT5336_INTERRUPT=n`, `CONFIG_INPUT_FT5336_PERIOD=10`,
  `CONFIG_INPUT_MODE_SYNCHRONOUS=y` — touch-driver behavioural
  tunables resolved during bring-up (per the existing `feedback_acm_ar_zephyr`
  / `project_zephyr_ft5336_no_touches` memory entries).
- `CONFIG_LOG_MODE_IMMEDIATE=y`, `CONFIG_INPUT_LOG_LEVEL_DBG=y` —
  diagnostic-level overrides.

Chapter 02 §8.4 says the Zephyr template emits a baseline `prj.conf`
the manifest's `target.features` extends. There is no mechanism to
override individual Kconfig values from the manifest.

**Disposition: DEFER.** v0 ships without per-Kconfig override.
Pipeline behaviour (APP-02d): emit the template-baselined `prj.conf`,
then if the destination directory already contains a `prj.conf` with
hand-edits, `--check` mode flags the divergence (per chapter 02 §9.4
inventory tracking) and CI surfaces it as an explicit choice. The
*intent* of the existing tuning gets carried by the in-tree comments
in `prj.conf`, not by the manifest.

A v1 amendment MAY add a `target.zephyr_kconfig:` map for explicit
overrides, but only if APP-03d implementation evidence shows the
deferred approach causes recurring CI noise. Premature now.

### 6.11 🟡 CLOSED — Path safety scoped to workspace root, not manifest parent

**Surfaced 2026-04-29 by APP-03b** (BBB Linux round-trip). The
existing example uses two cross-tree paths the original
[01 §3](01-manifest-schema.md#§3-glossary-additions) /
[§6 rule 4](01-manifest-schema.md#§6-validation-rule-set-normative)
incorrectly rejected:

- `controller.path: ../apps/disco-demo` — workspace path-dep on
  the shared `rlvgl-app-disco-demo` crate. The H747 freertos and
  zephyr round-trip manifests will hit the same.
- `assets[].source: ../stm32h747i-disco/assets/media/splash.rle`
  — splash binary shared between the BBB and H747 desktops; the
  existing BBB main.rs uses
  `include_bytes!("../../stm32h747i-disco/assets/media/splash.rle")`
  which Cargo allows but our schema's path-safety rule did not.

Both are legitimate monorepo patterns. The original "outside the
manifest's parent" wording was over-tight — the right scope for
manifest paths is the **cargo workspace root**, not the manifest
file's directory. Beyond the workspace root, absolute paths and
upward traversals are still rejected (the security intent is
preserved; only the scope is corrected).

**Disposition: ACCEPT** as ratified amendment to chapter 01
([§15 entry 2026-04-29](01-manifest-schema.md#§15-change-log)):
§3 Manifest path glossary, §6 rule 4 Path safety, and §5.10
`controller.path` validation all updated to scope at workspace
root.

## §10 Reconciliation with adjacent initiatives

### 10.1 vs. the BBB four-prong initiative

[`docs/beaglebone-black/`](../beaglebone-black/) defines the four-prong
pattern this chapter inherits from. The BBB initiative's status —
linux working, bare-metal pixels working, freertos/zephyr planned —
constrains §5.2's round-trip: only the **linux** intent has a
fully-functional reference today. The bare-metal manifest could
be written but cannot be *validated by build* until the BBB
bare-metal demo lands its display pipeline.

### 10.2 vs. the disco-platform-guide initiative

[`docs/disco-platform-guide/`](../disco-platform-guide/) owns the
H747 hand-written platform module's authority. §5.3 / §5.4
manifests cite that initiative; this chapter does not redefine
anything on the H747 side.

### 10.3 vs. the chipdb initiative

§6.4 names a chipdb-side amendment — minimal board entries for
hand-written cases. That work is filed against the chipdb
initiative, not against this chapter. Cross-reference here is for
readers, not authority.

## §11 Non-goals (this chapter)

- **Implementing the manifests for real.** v0 ratification of this
  chapter does NOT require checking `app.yaml` files into the
  example trees. That is a separate `APP-03[a-d]` PR sequence.
- **Round-tripping the simulator** (`examples/sim/`,
  `examples/disco-sim/`). The simulator is host-only and uses
  different runtime primitives; treat it as a v1 concern.
- **Round-tripping `examples/uefi-disco/`.** UEFI is a fifth prong
  not in the [00 §5.1](00-concepts.md#51-prong-set--standards-action)
  set. Adding it is a Standards Action; out of scope here.
- **Validating the manifests against an implemented pipeline.**
  The pipeline does not exist yet. This chapter validates the
  *grammar's expressive sufficiency*, not the *pipeline's
  correctness*. The latter is verified once `APP-02` PRs land.

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [ ] §5.1–§5.4 manifests reviewed for accuracy against current
      `examples/` content. Inventory drift is OK to record (not
      block ratification) as long as it is recorded in §15.
- [ ] §6 finding list reviewed; each finding has a disposition
      (ACCEPT / REJECT / DEFER) recorded in §15.
- [ ] §6.2 controller-library amendment decided — if ACCEPTED, a
      corresponding 01 §15 amendment opens before this chapter
      ratifies.
- [ ] §6.9 Zephyr sibling-project amendment decided — if
      ACCEPTED, a corresponding 02 §15 amendment opens before
      this chapter ratifies.
- [ ] §6.3 `target.generator` enum decision recorded.
- [ ] §15 has a dated ratification entry signed off by the
      initiative owner.

## §13 Files cited

- [`docs/app-schema/00-concepts.md`](00-concepts.md), [01](01-manifest-schema.md),
  [02](02-generator-pipeline.md) — chapters this round-trip evaluates.
- [`examples/beetle-esp32c3/Cargo.toml`](../../examples/beetle-esp32c3/Cargo.toml)
  — round-trip target §5.1.
- [`examples/beetle-esp32c3/src/esp_hal_main.rs`](../../examples/beetle-esp32c3/src/esp_hal_main.rs),
  [`bsp_pac_main.rs`](../../examples/beetle-esp32c3/src/bsp_pac_main.rs).
- [`examples/beaglebone-black/Cargo.toml`](../../examples/beaglebone-black/Cargo.toml),
  [`src/main.rs`](../../examples/beaglebone-black/src/main.rs)
  — round-trip target §5.2.
- [`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml),
  [`zephyr/CMakeLists.txt`](../../examples/stm32h747i-disco/zephyr/CMakeLists.txt),
  [`zephyr/prj.conf`](../../examples/stm32h747i-disco/zephyr/prj.conf)
  — round-trip targets §5.3, §5.4.
- [`examples/apps/disco-demo/src/lib.rs`](../../examples/apps/disco-demo/src/lib.rs)
  — controller library reference, finding §6.2.
- [`docs/beaglebone-black/README.md`](../beaglebone-black/README.md)
  — adjacent initiative, §10.1.
- [`docs/disco-platform-guide/README.md`](../disco-platform-guide/README.md)
  — adjacent initiative, §10.2.

## §14 Unblocks

Ratifying this chapter (with §6 dispositions recorded) unblocks:

- Chapter 1 amendments per §6.2 (controller field), §6.3 (generator
  enum), §6.6 (asset options), §6.10 (rust_inline_v1 v1 plan).
- Chapter 2 amendments per §6.8 (feature-graph in templates),
  §6.9 (Zephyr sibling-project emission).
- The `APP-03[a-d]` PR sequence: check real `app.yaml` files into
  each round-trip target and add a CI step that runs the (not yet
  implemented) `rlvgl-creator app from-yaml --check` against each.
- A decision on `extends:` (§6.7) at v1 planning time.

## §15 Change log

| Date       | Status | Note                                                                                                                                                                                                                                                                                                                                                                                              |
| ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-04-27 | DRAFT  | Initial round-trip pass against all four [00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property) targets. Significant findings: §6.2 (controller-library slot, 🔴), §6.9 (Zephyr sibling-project emission, 🔴), §6.3 (third `target.generator` value, 🟡), §6.4 (chipdb coverage extension, 🟡), §6.10 (`rust_inline_v1` is the primary path today, not the v0 backdoor it was framed as). |
| 2026-04-27 | DRAFT  | Convergence pass: all §6 findings closed and amended into upstream chapters. **Dispositions:** §6.1 ACCEPT (intent-vs-crate); §6.2 ACCEPT (`controller:` field added to 01 §5.10 + 02 §7.8); §6.3 ACCEPT (`hosted` added to 01 §5.2 generator enum); §6.4 ACCEPT (chipdb minimal-entry rule in 01 §5.2; follow-up chipdb board YAMLs filed); §6.5 ACCEPT (asset class enum holds); §6.6 ACCEPT (informative `options:` keys table at 01 §5.4.1); §6.7 ACCEPT Option A (no `extends:` at v0; recorded in 01 §11); §6.8 ACCEPT (feature-graph in 02 §8 preamble); §6.9 ACCEPT nested layout (02 §5.4.1 + §8.4); §6.10 ACCEPT with v1 gating (01 §5.5). §12 checklist still has the in-tree `app.yaml` check-in items — chapter remains DRAFT pending real manifests landing in round-trip target trees. |
| 2026-04-27 | DRAFT  | APP-03a partial landing: `examples/beetle-esp32c3/app.yaml` + `layouts/main_screen.rs` checked in as the first round-trip artifact (chapters 00 + 01 RATIFIED same day). Cites `target.generator: hosted`, `layout_format: rust_inline_v1`. Validator `--check` gating awaits APP-02a. Remaining round-trip artifacts (BBB linux, H747 freertos, H747 zephyr) blocked on chipdb minimal entries (01 §5.2 / [03 §6.4 follow-up](#64--closed--chipdb-minimal-entry-rule-for-non-creator-bsp-pac-boards)) and corresponding chip families being present in the chipdb tree. |
| 2026-04-29 | DRAFT  | §6.4 closure superseded after [01 §5.2 amendment](01-manifest-schema.md#§15-change-log) discovered the original wording assumed esp-shape YAML files universally. New rule: board id resolves via vendor-crate `find()` API; backing storage is per-vendor. Follow-up actions rewritten — ti gets a hardcoded `BOARDS` const entry, stm gets a build-script source addition + archive rebuild, no YAML files dropped into trees that don't use them. |
| 2026-04-29 | DRAFT  | APP-04a landed: ti and stm vendor crates each gained one `BoardInfo` const entry (`beaglebone_black_nhd_cape` → AM335x, `stm32h747i_disco` → STM32H747XIH6). Investigation showed both vendors drive `find()` from a hardcoded `BOARDS` const, so the simpler-than-expected mechanism made the addition trivial. BBB and H747 round-trip manifests are now unblocked at the `target.board` validation level. |
| 2026-04-29 | DRAFT  | APP-03b landed: `examples/beaglebone-black/app.yaml` + `layouts/home.rs` checked in as the second round-trip artifact. Cites `target.generator: hand_written` (BBB Linux on §5.6 allow-list), the new `controller:` field at `../apps/disco-demo`, and a cross-tree splash asset at `../stm32h747i-disco/assets/media/splash.rle`. Discovery of the cross-tree paths surfaced [§6.11](#611--closed--path-safety-scoped-to-workspace-root-not-manifest-parent) — path-safety rule was over-tight; landed alongside as a 01 §15 ratified amendment scoping path safety to the workspace root rather than the manifest's parent. |
| 2026-04-29 | DRAFT  | APP-03c landed: `examples/stm32h747i-disco/app.yaml` + `layouts/home.rs` checked in as the third round-trip artifact (FreeRTOS intent, build profile `cm7,freertos,adapted_cmd,dma2d,splash,desktop` per CLAUDE.md). No new spec gaps — the path-safety amendment from APP-03b carried it. Cites the new `controller:` field at `../apps/disco-demo`, capabilities preset `stm32h747i_disco`. Splash asset is local (`assets/media/splash.rle`); same blob is what the BBB manifest cross-references. Three of four round-trip targets now landed (beetle esp_hal, BBB linux, H747 freertos); H747 zephyr (APP-03d) remains. |
| 2026-04-29 | DRAFT  | APP-03d landed: `examples/stm32h747i-disco/app-zephyr.yaml` checked in as the fourth and final round-trip artifact (Zephyr intent, build profile `cm7,zephyr,splash,desktop,dma2d`). Reuses `layouts/home.rs` with the FreeRTOS manifest — the controller-driven render call is prong-agnostic; cross-prong layout reuse is exactly the schema's value proposition. Filename is `app-zephyr.yaml` (not `app.yaml`) since the same Cargo crate hosts the FreeRTOS intent at the canonical name. Surfaced finding [§6.12](#612--open--zephyr-prjconf-has-hand-tuned-values-the-template-cannot-reproduce) — existing `zephyr/prj.conf` has hand-tuned values (`CONFIG_MAIN_STACK_SIZE=16384`, FT5336 touch tunables, log levels) that neither chipdb nor `target.features` express; **disposition DEFER** with `--check`-flag surfacing in APP-02d, no v0 grammar change. **All four round-trip targets now landed**; remaining §12 acceptance work for chapter 03 is the validator-acceptance proof under APP-02a. |
