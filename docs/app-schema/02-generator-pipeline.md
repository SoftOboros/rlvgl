<!--
02-generator-pipeline.md - rlvgl Application Schema, Chapter 2: Generator Pipeline.
Status: DRAFT — not yet ratified. See §15 change log.
-->

**[← Prev](01-manifest-schema.md) · [Index](README.md) · Next → (TBD)**

# Chapter 2 — Generator Pipeline (`rlvgl-creator app from-yaml`)

> **Status:** DRAFT, unratified. Depends on
> [Chapter 0](00-concepts.md) and [Chapter 1](01-manifest-schema.md).
> Until §15 records a ratified entry, no `APP-02` PR may cite this
> doc as a frozen authority.

## §0 Authority policy

This chapter is normative for:

- the pipeline stages (§5),
- the sub-generator contracts (§7),
- the emitted crate layout (§5.4),
- the determinism and rebuild policy (§9).

It is *not* normative for the internal implementation of the
existing sub-generators (BSP gen, asset pipeline, MCP state-chart,
i18n) — those are owned by their respective trees and only have to
satisfy the contracts in §7.

RFC 2119 keywords carry their RFC meanings.

## §1 Purpose

Define how a single command —

```bash
rlvgl-creator app from-yaml --manifest path/to/app.yaml --out path/to/crate/
```

— consumes a [chapter 01](01-manifest-schema.md) `app.yaml` and emits
a buildable Cargo crate equivalent to one of the existing examples
under `examples/`.

The chapter answers:

1. **What stages run, in what order** (§5, §6).
2. **What contract each sub-generator obeys** so the orchestrator
   can compose them without owning their internals (§7).
3. **What the emitted crate looks like on disk** so reviewers and
   round-trip extractors have a target shape (§5.4).
4. **How regeneration interacts with hand-edited code** (§9).
5. **How the four prongs differ in the emitted glue** (§8).

## §3 Glossary additions

Only terms not defined in
[00 §3](00-concepts.md#§3-canonical-glossary) or
[01 §3](01-manifest-schema.md#§3-glossary-additions):

- **Sub-generator** — *Owned by this chapter.* A tool the
  orchestrator invokes to produce one slice of the output: the BSP
  gen, the asset pipeline, the MCP state-chart, the i18n consumer,
  the theme translator, the layout translator. Each obeys the §7
  contract.
- **Orchestrator** — *Owned by this chapter.* The
  `rlvgl-creator app from-yaml` driver. Owns parse + validate,
  sub-generator invocation order, output-tree assembly, the
  determinism guarantee.
- **Stage** — *Owned by this chapter.* A node in the pipeline
  dependency graph (§5.1). Stages with no dependency edge between
  them MAY run in parallel.
- **Crate scaffold** — *Owned by this chapter.* The set of files
  the orchestrator emits *itself* (not via a sub-generator):
  `Cargo.toml`, top-level `src/main.rs`, `src/app.rs`, the per-prong
  glue, and the index `mod.rs` files that wire sub-generator output
  in.
- **Wiring contract** — *As defined in [00 §7](00-concepts.md#§7-frozen-decisions--wiring-contract-sketch);
  used without modification.* The `App::new` / `App::tick` shape
  the scaffold emits.

## §4 Source-of-truth additions

| Concept                              | Owner (canonical)               | Mirrored / consumed by                       |
| ------------------------------------ | ------------------------------- | -------------------------------------------- |
| Pipeline stage graph                 | This chapter §5.1               | orchestrator                                 |
| Sub-generator contract               | This chapter §7                 | every sub-generator's CLI                    |
| Emitted crate file layout            | This chapter §5.4               | orchestrator output, round-trip extractor    |
| Per-prong main-glue templates        | This chapter §8                 | orchestrator's scaffold step                 |
| Regeneration / overwrite policy      | This chapter §9                 | orchestrator, CI divergence check            |

## §5 Frozen decisions — pipeline shape

> *Proposed for freeze. Not ratified — see §15.*

### 5.1 Stage graph

```
                     parse + validate (manifest)
                              │
            ┌─────────────────┼─────────────────┬────────────────┐
            ▼                 ▼                 ▼                ▼
       BSP-gen          asset-pipeline      sm-gen           i18n + theme
      (chipdb +        (creator asset       (external MCP    (existing
       creator)         pipeline)            state-chart)     translators)
            │                 │                 │                │
            └─────────────────┴────────┬────────┴────────────────┘
                                       ▼
                              cross-reference resolve
                              (screens.state vs SM,
                               screens.layout asset refs)
                                       ▼
                              layout-translator
                              (per screens[] entry)
                                       ▼
                              crate scaffold emit
                              (Cargo.toml, main.rs,
                               app.rs, mod.rs index)
                                       ▼
                              fmt + post-emit checks
```

Stages with no dependency edge between them — BSP-gen,
asset-pipeline, sm-gen, i18n, theme — MAY run in parallel.
The orchestrator MUST treat them as parallelisable and SHOULD
parallelise when invoked with `--jobs > 1`.

### 5.2 CLI surface

```bash
rlvgl-creator app from-yaml \
    --manifest <path>      # required, path to app.yaml
    --out <dir>            # required, output crate directory
    [--validate-only]      # parse + validate; do not emit
    [--check]              # emit to a temp dir, compare against --out, exit 1 on diff
    [--jobs <N>]           # parallel sub-generator invocations, default 1
    [--silent]             # suppress progress output
    [--force]              # overwrite --out without confirmation
```

`--validate-only` runs stage 1 only and exits with the validator's
status code.

`--check` is the CI mode: it asserts the emitted output matches a
committed `--out` directory byte-for-byte. A non-empty diff fails
the run. This is the determinism gate from §9.

`--force` is required when `--out` is non-empty and contains files
not produced by a previous run (the orchestrator records its own
file inventory in `<out>/.rlvgl-app-manifest.json`; files not in
that inventory are treated as user-owned and trigger an error
without `--force`).

### 5.3 Output is committed, not build-time

Generated output **MUST** be committed to the consuming repo, not
regenerated at `cargo build` time. This matches the chipdb
precedent (the existing
`bsp from-yaml` generator's output is copied into
`src/bsp_generated/` — see
[`docs/disco-platform-guide/11-generated-bsps.md`](../disco-platform-guide/11-generated-bsps.md)).

Rationale:

- Reviewers can read the generated code in PRs.
- `cargo build` stays fast and offline.
- A divergence between `app.yaml` and committed output is caught
  by CI's `--check` run, not deferred to build-time errors.

A `build.rs` MAY be emitted for build-time tasks that genuinely
need cargo's environment (e.g. picking up `OUT_DIR`, target
detection), but MUST NOT re-invoke sub-generators.

### 5.4 Emitted crate layout

```
<out>/
├── Cargo.toml                       # scaffold-emitted
├── build.rs                         # scaffold-emitted, optional
├── README.md                        # scaffold-emitted, references the manifest
├── .rlvgl-app-manifest.json         # orchestrator inventory (§5.2 --force)
├── assets/                          # asset-pipeline-emitted (binaries)
│   ├── splash.bin
│   └── inter-16.bin
├── states/                          # sm-gen-emitted, copy of source .scxml
│   └── main.scxml
└── src/
    ├── main.rs                      # scaffold-emitted, per-prong (§8)
    ├── app.rs                       # scaffold-emitted, prong-agnostic
    ├── bsp_generated/               # BSP-gen-emitted (only when
    │   ├── mod.rs                   # target.generator = creator-bsp-pac)
    │   ├── pac.rs
    │   ├── clocks.rs
    │   ├── io_mux.rs
    │   ├── peripherals.rs
    │   └── board.rs
    ├── state_machine/               # sm-gen-emitted (only when
    │   ├── mod.rs                   # state_machine: present)
    │   ├── states.rs
    │   └── vectors.rs               # only when verification_vectors: true
    ├── screens/                     # layout-translator-emitted
    │   ├── mod.rs
    │   ├── home.rs
    │   └── settings.rs
    ├── theme.rs                     # theme-translator-emitted
    ├── i18n_generated.rs            # i18n-emitted
    └── assets_generated.rs          # asset-pipeline-emitted (Rust indices)
```

Files marked *scaffold-emitted* are the orchestrator's own work.
Files marked *<sub-gen>-emitted* are produced by the named
sub-generator and copied/moved into place.

For `target.generator: hand_written`
([01 §5.6](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list)),
`src/bsp_generated/` is omitted; `Cargo.toml` instead names the
hand-written platform crate as a path dependency.

For `target.generator: hosted`, `src/bsp_generated/` is also
omitted; the upstream HAL crate (e.g. `esp-hal`, `embassy-stm32`)
is added to `[dependencies]` and `src/main.rs` consumes it
directly.

### 5.4.1 Zephyr prong: nested west project

When `target.prong = zephyr`, the emitted layout adds a nested
west project beside the Cargo crate, and the Cargo crate becomes
a `staticlib` rather than a `bin`:

```
<out>/
├── Cargo.toml                       # crate-type = ["staticlib"]
├── src/
│   ├── lib.rs                       # entry point: rlvgl_init() etc.
│   ├── app.rs
│   └── ... (other src as in §5.4)
└── zephyr/                          # nested west project
    ├── CMakeLists.txt               # links librlvgl_<name>.a
    ├── prj.conf                     # Zephyr Kconfig
    ├── app.overlay                  # devicetree overlay
    └── src/
        └── main.c                   # Zephyr main(), calls rlvgl_init()
```

Rationale: Zephyr's build system is CMake/west, not cargo. The
manifest pipeline cannot emit a buildable Zephyr app as a cargo
crate alone — the Zephyr side needs its own project root with
Kconfig and devicetree fragments. The nested layout matches the
existing reference at `examples/stm32h747i-disco/zephyr/`.

The Zephyr template (§8.4) owns the `prj.conf` Kconfig set, the
`app.overlay` overlay structure, and the `main.c` shape that calls
into the Rust staticlib's exported `rlvgl_init()` entry point.

The emitted `Cargo.toml` for the zephyr prong:

```toml
[lib]
crate-type = ["staticlib"]
```

… with no `[[bin]]` section. The reference in
[`examples/stm32h747i-disco/Cargo.toml`](../../examples/stm32h747i-disco/Cargo.toml)
keeps the staticlib alongside `[[bin]]` entries for other prongs in
the same crate; the manifest pipeline does NOT do that — one
manifest, one prong, one output.

## §6 Pipeline flow (normative ordering)

The orchestrator MUST execute the following stages in order. Stages
on the same level of indentation MAY run in parallel.

1. **Parse & validate.** Apply [chapter 01 §6](01-manifest-schema.md#§6-validation-rule-set-normative)
   in order. On any failure, exit non-zero with the rule number and
   field path.

2. **Resolve cross-references.** chipdb board YAML loaded; asset
   class enum loaded; manifest's `target` resolves a real chip;
   `assets[].palette_ref` graph topologically sorted.

3. **Independent sub-generators (parallel):**
   - BSP-gen — only if `target.generator` is `creator-bsp-pac`.
   - asset-pipeline — for each `assets[]` entry.
   - sm-gen — only if `state_machine:` present.
   - i18n — only if `i18n:` present.
   - theme-translator — only if `theme:` present.

4. **Post-SM cross-validate.** Every `screens[].state` MUST resolve
   against the SM's emitted state set. Fail with the screen `id`
   and the unknown state name.

5. **Layout-translator.** For each `screens[]` entry, translate the
   `layout:` source into `src/screens/<id>.rs`. Layout translator
   needs:
   - asset id index (from asset-pipeline output),
   - theme token table (from theme-translator output),
   - i18n key set (from i18n output, for compile-time string
     resolution).

6. **Crate scaffold.** Emit `Cargo.toml`, `src/main.rs` (per-prong,
   §8), `src/app.rs`, `src/screens/mod.rs`, `README.md`,
   `.rlvgl-app-manifest.json` inventory.

7. **Post-emit checks:**
   - `cargo fmt -- --check` on the emitted tree (deterministic
     formatting, §9).
   - For `target.prong = bare_metal | freertos | zephyr`: a
     `cargo check --target <triple>` against the emitted crate is
     RECOMMENDED but not blocking for v0; see §11.

## §7 Sub-generator contracts (normative)

Every sub-generator the orchestrator invokes MUST satisfy this
contract.

### 7.1 Common contract (all sub-generators)

A sub-generator:

- MUST be invokable as a CLI command with all inputs passed as
  flags, files at known paths, and/or stdin. No environment-variable
  inputs other than `RUSTFLAGS` and standard cargo env.
- MUST be **deterministic**: same inputs (including version pins)
  → byte-identical output.
- MUST write all output beneath a single `--out` directory.
- MUST emit a self-manifest at `<out>/.<gen-name>-manifest.json`
  listing every file it produced, with a content hash for each.
- MUST exit non-zero on any input or processing error, with a
  human-readable message on stderr.
- MUST NOT modify files outside `--out`.

The orchestrator validates the self-manifest, then merges
sub-generator output into the application crate at the §5.4
locations.

### 7.2 BSP-gen contract

Already implemented as `rlvgl-creator bsp from-yaml`. v0 adopts
the existing CLI verbatim:

```bash
rlvgl-creator bsp from-yaml \
    --vendor <vendor> --board <board> \
    --out <dir> --emit-pac
```

Output files (per
[`docs/disco-platform-guide/11-generated-bsps.md`](../disco-platform-guide/11-generated-bsps.md)):
`mod.rs`, `pac.rs`, `clocks.rs`, `io_mux.rs`, `peripherals.rs`,
`board.rs`. The orchestrator invokes this as the BSP-gen stage and
copies output into `<app-out>/src/bsp_generated/`.

The output's `mod.rs` is crate-root-shaped; the orchestrator
substitutes a child-module-shaped `mod.rs` that uses `super::`
references. This matches the existing
beetle-esp32c3 hand-written pattern.

### 7.3 Asset-pipeline contract

Existing `rlvgl-creator` asset converters. v0 wraps each `assets[]`
entry as a single converter invocation. Outputs go to
`<app-out>/assets/<id>.bin` and a Rust index module
`<app-out>/src/assets_generated.rs` exposing each asset by its
manifest `id`:

```rust
// src/assets_generated.rs (excerpt, generator-emitted)
pub static SPLASH: &[u8] = include_bytes!("../assets/splash.bin");
pub static INTER_16: &[u8] = include_bytes!("../assets/inter-16.bin");

pub mod meta {
    pub const SPLASH_CLASS: &str = "image_rle_a8";
    pub const SPLASH_PALETTE_REF: Option<&str> = Some("ui-palette");
    // ...
}
```

Asset class names ([00 §5.3](00-concepts.md#53-asset-class-set--specification-required))
MUST appear verbatim in the meta block; mismatches between manifest
and pipeline output are an orchestrator-level error.

### 7.4 SM-gen contract (external MCP)

The state-machine generator runs *outside* this repo. v0 invokes it
as a CLI tool the orchestrator does not own. The contract:

- **Input:** `state_machine.source` (the `.scxml`/`.uml` file from
  the manifest).
- **Output (under `<sm-out>/`):**
    - `mod.rs` — `pub mod states; pub mod vectors;` index.
    - `states.rs` — Rust enum + transition function. The state
      enum's variant names MUST match the SM `state` ids (validated
      in pipeline step 4, §6).
    - `vectors.rs` (only if `verification_vectors: true`) —
      `#[cfg(test)]` test vectors derived from the SCXML.
- **Self-manifest:** as in §7.1, plus a top-level `state_set`
  field listing the emitted state ids for the orchestrator's
  cross-reference step.

If the external generator is unreachable (offline build, missing
binary), the orchestrator fails at stage 3 with a clear error
naming the missing tool. v0 does NOT fall back to a stub SM.

### 7.5 i18n contract

Adopts the existing `i18n/` crate's bundle shape verbatim
([01 §5.8](01-manifest-schema.md#58-i18n-optional)). The "generator"
here is a thin emitter that produces `src/i18n_generated.rs`:

```rust
// src/i18n_generated.rs (excerpt)
pub fn t(key: &str, locale: &str) -> &'static str {
    match (locale, key) {
        ("en", "demo.title") => "rlvgl Demo v{version}",
        ("fr", "demo.title") => "Démo rlvgl v{version}",
        // ...
        _ => key,  // fall back to the key
    }
}
```

The translator MUST read from `i18n.bundle_dir` and emit one match
arm per (locale, key). Missing-key validation (a key present in
some locales but not others) is reported as a warning at v0; v1
may promote it to an error.

### 7.6 Theme-translator contract

`format: chakra_tokens_v1` consumes a JSON file shaped like the
output of `extendTheme(...)` and emits `src/theme.rs`:

```rust
// src/theme.rs (excerpt for chakra_tokens_v1)
pub mod colors {
    pub const PRIMARY_500: u32 = 0x3182CE;
    pub const GRAY_50:     u32 = 0xF7FAFC;
    // ...
}
pub mod space   { pub const SP_4: u16 = 16; /* ... */ }
pub mod radii   { pub const MD: u16 = 6;  /* ... */ }
```

The chakra → rlvgl mapping for non-trivial token types
(typography scale, shadows) is owned by the existing creator
chakra exporter — the orchestrator delegates to it without
wrapping the mapping rules in the manifest.

`format: raw_palette_v1` consumes a flat `{ name: "#rrggbb" }` map
and emits the `colors` module only.

### 7.7 Layout-translator contract

Consumes `screens[].layout` (any of the v0 layout formats per
[01 §5.5](01-manifest-schema.md#55-screens-optional)) plus the
output of asset-pipeline, theme-translator, and i18n. Emits
`src/screens/<id>.rs` per screen entry, plus `src/screens/mod.rs`
listing them.

For `layout_format: rust_inline_v1`, the translator copies the
referenced `.rs` file verbatim into `src/screens/<id>.rs`. This is
the v0 round-trip backdoor noted at
[01 §5.5](01-manifest-schema.md#55-screens-optional); v1 will
remove it once a real layout authoring pipeline ships.

### 7.8 Controller wiring contract

When the manifest declares a [01 §5.10](01-manifest-schema.md#510-controller-optional)
`controller:` block, the orchestrator does NOT generate the
controller — it is a hand-written rlvgl crate. The orchestrator's
responsibilities are:

1. Emit a `[dependencies]` entry in `Cargo.toml`. For a `path:`
   override:

   ```toml
   [dependencies]
   rlvgl-app-disco-demo = { path = "../apps/disco-demo", features = [...] }
   ```

   For a `version:` requirement:

   ```toml
   [dependencies]
   rlvgl-app-disco-demo = { version = "0.2.0", features = [...] }
   ```

   For neither (default registry resolution):

   ```toml
   [dependencies]
   rlvgl-app-disco-demo = { features = [...] }
   ```

2. Emit `src/app.rs` with an `App::new(bsp)` body that constructs
   the controller. The `capabilities:` field, if present, is
   passed verbatim as a constructor selector — the controller
   crate itself owns the `capabilities` → `pub const fn xxx() -> Self`
   mapping. Example:

   ```rust
   // src/app.rs (scaffold-emitted, controller: rlvgl-app-disco-demo,
   // capabilities: stm32h747i_disco)
   use rlvgl_app_disco_demo::{DiscoCapabilities, DiscoController};

   pub struct App {
       controller: DiscoController,
   }

   impl App {
       pub fn new(bsp: Bsp) -> Self {
           let caps = DiscoCapabilities::stm32h747i_disco();
           Self { controller: DiscoController::new(bsp, caps) }
       }

       pub fn tick(&mut self, now: Cycles, inputs: Inputs) -> Outputs {
           self.controller.tick(now, inputs)
       }
   }
   ```

3. Validate at build time (not at manifest-validate time) that the
   named `capabilities` constructor exists on the controller crate.
   An unknown preset surfaces as a `cargo build` error pointing at
   `src/app.rs`, not as a manifest error.

When `controller:` is absent, the scaffold emits a stub `App` whose
`tick` body is empty and whose author is expected to fill it in by
hand. This is the path used by minimal examples like the
beetle-esp32c3 esp_hal binary
([03 §5.1](03-round-trip.md#51-examplesbeetle-esp32c3--esp_hal-binary-bare-metal)).

## §8 Per-prong main glue (normative templates)

> **Feature graph ownership.** The manifest's `target.features`
> is a flat list of leaf features the user wants enabled. Real
> Cargo `[features]` tables have graph structure (e.g.
> `freertos = ["rlvgl-platform/freertos"]`). **The per-prong
> template owns the graph.** When the orchestrator emits
> `Cargo.toml`, it expands `target.features` against the prong
> template's known feature dependencies. The manifest names *what
> the user wants enabled*; the template knows *what enabling that
> means in the emitted crate*.
>
> Round-trip evidence: [03 §6.8](03-round-trip.md#68--cargo-features-graph-is-one-level-deep-at-v0).

The crate scaffold emits `src/main.rs` per `target.prong`. The
prong-agnostic `App::new` / `App::tick` lives in `src/app.rs`; the
glue is the only file that knows which runtime owns the loop.

### 8.1 `linux` prong

```rust
// src/main.rs (linux prong, scaffold-emitted)
fn main() -> std::io::Result<()> {
    let bsp = bsp::init()?;                    // BSP-gen or hand-written
    let mut app = app::App::new(bsp);
    let frame = std::time::Duration::from_millis(16);
    let mut next = std::time::Instant::now();
    loop {
        let inputs = bsp.poll_inputs();
        app.tick(std::time::Instant::now(), inputs);
        next += frame;
        std::thread::sleep(next.saturating_duration_since(std::time::Instant::now()));
    }
}
```

### 8.2 `bare_metal` prong

```rust
// src/main.rs (bare_metal prong, scaffold-emitted)
#![no_std]
#![no_main]
use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    let bsp = bsp::init();
    let mut app = app::App::new(bsp);
    loop {
        let inputs = bsp.poll_inputs();
        app.tick(now_cycles(), inputs);
        bsp.present();
        bsp.wait_for_frame();   // ERIF gate, SysTick, etc.
    }
}
```

The DSI ERIF holdoff for the H747 path lives behind
`bsp.wait_for_frame()` — this is why the H747 board is on the
hand-written allow-list ([01 §5.6](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list));
its `wait_for_frame` is non-trivial (see
[disco-platform-guide chapter 5](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)).

### 8.3 `freertos` prong

The scaffold emits one task per stage matching the FreeRTOS
pattern in
[`CLAUDE.md`](../../CLAUDE.md#freertos-build): `present_task`,
`render_task`, `input_task`, plus `playit_task` if
[`playit/`](../../playit/) integration is enabled. Tasks
communicate via FreeRTOS queues; `App::tick` runs in `render_task`.

### 8.4 `zephyr` prong

Per [§5.4.1](#541-zephyr-prong-nested-west-project), the Zephyr
prong emits **two** trees: a Cargo staticlib and a nested west
project under `zephyr/`.

**Rust side (`src/lib.rs`, scaffold-emitted):**

```rust
// src/lib.rs (zephyr prong, scaffold-emitted)
#![no_std]

mod app;
mod bsp;     // hand_written or hosted; no bsp_generated for H747

#[unsafe(no_mangle)]
pub extern "C" fn rlvgl_init() -> i32 {
    let bsp = bsp::init();
    let mut app = app::App::new(bsp);
    loop {
        let inputs = bsp.poll_inputs();
        app.tick(now_cycles(), inputs);
        bsp.present();
        bsp.wait_for_frame();
    }
}
```

**Zephyr `zephyr/src/main.c` (scaffold-emitted):**

```c
#include <zephyr/kernel.h>

extern int rlvgl_init(void);

int main(void) {
    return rlvgl_init();
}
```

**Zephyr `zephyr/CMakeLists.txt` (scaffold-emitted):**

```cmake
cmake_minimum_required(VERSION 3.20.0)
find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(<manifest-name> C)

target_sources(app PRIVATE src/main.c)

set(RLVGL_RUST_LIB
    ${CMAKE_CURRENT_SOURCE_DIR}/../target/<triple>/release/lib<crate-name>.a)

if(EXISTS ${RLVGL_RUST_LIB})
    target_link_libraries(app PUBLIC ${RLVGL_RUST_LIB})
else()
    message(FATAL_ERROR "Rust staticlib not found at ${RLVGL_RUST_LIB}")
endif()
```

**Zephyr `zephyr/prj.conf` (scaffold-emitted):** the template
emits a baseline Kconfig set that the manifest's
`target.features` extends. The baseline includes
`CONFIG_DISPLAY=y`, `CONFIG_INPUT=y`, `CONFIG_LOG=y`, plus
target-board-specific defaults derived from chipdb. Per-feature
extensions are owned by the prong template's feature-graph map
(see §8 preamble).

**Zephyr `zephyr/app.overlay` (scaffold-emitted):** baseline
devicetree overlay enabling display, touch, and console nodes for
the chipdb-declared peripherals. The H747-specific
`adapted_cmd.overlay` (DSI adapted command mode) is emitted only
when `target.features` includes the corresponding flag.

For both `freertos` and `zephyr` prongs, the per-prong template
expansions are extensive; v0's scaffold MAY emit minimal viable
templates with `// TODO(APP-NN)` markers for the runtime-specific
plumbing the existing repo's hand-written ports already solve. The
[03-round-trip](03-round-trip.md) chapter names which existing
examples cover the fully-fleshed templates — currently
`examples/stm32h747i-disco/` (FreeRTOS via `freertos` feature,
Zephyr via the sibling `zephyr/` directory) and
`examples/beaglebone-black/freertos/` + `zephyr/` (planned).

## §9 Determinism, regeneration, hand edits

### 9.1 Determinism

The pipeline MUST be byte-identical-deterministic for a given
combination of:

- manifest content,
- chipdb content (board + chip YAML),
- asset source files,
- sub-generator versions (each sub-generator pins its version in
  its self-manifest, §7.1),
- the orchestrator version.

CI MUST run `--check` on the committed manifest and committed
output; a non-empty diff fails the build. This is how
"manifest-says-X / committed-says-Y" drift is caught.

### 9.2 `cargo fmt`

The orchestrator runs `cargo fmt` on the emitted tree as the final
step. Determinism therefore depends on the rustfmt version the
orchestrator pins. v0 pins to the workspace's `rust-toolchain.toml`
(or repo-root `rustfmt.toml` if present).

### 9.3 Hand edits

v0 policy: regeneration overwrites everything in `<out>/`. There is
no "keep my edits" mode.

Hand edits live outside the generated tree:

- Add `src/lib_extra.rs` to the *manifest's* sibling files list
  (TBD field — flagged as v0 limitation; v1 work).
- Or fork the generated crate, accept the divergence, and let CI
  `--check` flag the divergence as an explicit choice.

This is intentionally restrictive at v0. The cost of allowing
mixed hand-edits + regen is exactly the class of bug that motivated
the schema in the first place — silent forks between authoring
intent and committed code.

### 9.4 Inventory tracking

`<out>/.rlvgl-app-manifest.json` records every file the
orchestrator wrote on its last run. On subsequent runs:

- Files present in `<out>` AND in the inventory: overwritten.
- Files present in `<out>` but NOT in the inventory: error
  (without `--force`).
- Files in the inventory but no longer in the *new* output:
  deleted.

This catches the common failure where an asset is removed from the
manifest but its old binary lingers in `<out>/assets/`.

## §10 Reconciliation with existing creator subcommands

### 10.1 `bsp from-yaml` keeps its CLI

The orchestrator does not wrap, re-implement, or wrap-and-rename
the existing `rlvgl-creator bsp from-yaml`. It invokes it with the
exact existing flags. This means the existing chipdb test families
(`bsp_esp32c3_render`, `bsp_esp32c3_compile`, etc.) keep their
contracts untouched.

### 10.2 Asset converters keep their CLIs

Same principle: the orchestrator wraps each existing converter
invocation per `assets[]` entry. New asset classes added to
[00 §5.3](00-concepts.md#53-asset-class-set--specification-required)
get a new converter invocation in the orchestrator, not a rewrite
of the existing pipeline.

### 10.3 i18n's `extract_keys.py` workflow

The existing `i18n/extract_keys.py` extracts string keys from
source code. The orchestrator runs in the *opposite* direction —
emitting code that *uses* keys. The two flows are not in conflict;
v0 simply assumes the i18n bundle exists and is up-to-date when
the orchestrator runs. CI may run `extract_keys.py` separately.

### 10.4 Hand-written platform crates

For `target.generator: hand_written`, the orchestrator does not
invoke BSP-gen. Instead, the emitted `Cargo.toml` declares a path
dependency on the hand-written platform crate (e.g.
`platform = { path = "../platform" }`). The wiring contract
([00 §7](00-concepts.md#§7-frozen-decisions--wiring-contract-sketch))
is satisfied by the platform crate's existing trait surface.

## §11 Non-goals (this chapter)

- **Compile-verification of the emitted crate.** The orchestrator
  does not run `cargo check` or `cargo build` on its own output in
  v0. CI runs those separately. Adding compile-verify is a v1
  concern (analogous to chipdb's existing `compile-verify` feature).
- **Live regeneration on file change.** No watch mode. Run the
  command explicitly.
- **Mixing manual and generated source files in the same module.**
  See §9.3 — v0 forbids this. v1 may relax.
- **Network calls during generation.** All sub-generators MUST be
  offline-capable. The MCP state-chart generator, if it has a
  network dependency, MUST satisfy it before the orchestrator
  invokes it.
- **Diff-aware partial regeneration.** v0 always regenerates the
  full output. v1 may support partial.

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [ ] §5.1 stage graph matches the eventual orchestrator
      implementation.
- [ ] §5.2 CLI flags reviewed; no missing required flag.
- [ ] §5.4 emitted-crate layout reproduced by reverse extraction
      from at least one round-trip target ([00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property)).
- [ ] §7 contracts each have at least one existing or
      stubbed-in-rlvgl-creator sub-generator that satisfies them.
- [ ] §8 per-prong templates each match an existing example's
      `main.rs` shape closely enough that the round-trip emitter
      could plausibly produce it.
- [ ] §9.4 inventory file format is settled (JSON Schema or prose).
- [ ] §15 has a dated ratification entry signed off by the
      initiative owner.

## §13 Files cited

- [`docs/app-schema/00-concepts.md`](00-concepts.md) — chapter 0,
  authority for vocabulary and source-of-truth map.
- [`docs/app-schema/01-manifest-schema.md`](01-manifest-schema.md) —
  chapter 1, the input format this chapter consumes.
- [`docs/disco-platform-guide/11-generated-bsps.md`](../disco-platform-guide/11-generated-bsps.md)
  — BSP generator existing CLI, cited by §7.2.
- [`docs/disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md`](../disco-platform-guide/05-ltdc-dsi-and-axi-holdoff.md)
  — H747 ERIF gating, cited by §8.2 (why the H747 is on the
  hand-written allow-list).
- [`CLAUDE.md`](../../CLAUDE.md) — FreeRTOS build profile, cited
  by §8.3.
- [`platform/`](../../platform/) — hand-written platform crate,
  cited by §10.4.
- [`i18n/extract_keys.py`](../../i18n/extract_keys.py) — existing
  i18n workflow, cited by §10.3.

## §14 Unblocks

Ratifying this chapter unblocks:

- `03-round-trip.md` — extracting a manifest *and* verifying
  the orchestrator produces the original example crate from it,
  for each [00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property)
  target.
- `04-state-machine-boundary.md` — full Option A treatment now
  that the SM-gen contract (§7.4) is concrete.
- A `rlvgl-creator app from-yaml` subcommand implementation
  (initial PR sequence: `APP-02a` orchestrator skeleton +
  validator wrapper, `APP-02b` parallel sub-generator dispatch,
  `APP-02c` scaffold emitter + per-prong templates, `APP-02d`
  `--check` mode + CI integration).

## §15 Change log

| Date       | Status | Note                                                                                                                                                                                                |
| ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-04-27 | DRAFT  | Initial pipeline definition. Stage graph, CLI surface, sub-generator contracts, per-prong main glue templates. Argument target — §5.3 (committed-vs-buildtime), §9.3 (no hand-edit mode), §8.3/§8.4 (FreeRTOS / Zephyr template completeness). |
| 2026-04-27 | DRAFT  | Round-trip convergence pass closed. Pipeline additions: §5.4.1 Zephyr nested west project layout + §8.4 fleshed Zephyr templates with `lib.rs`/`main.c`/`CMakeLists.txt`/`prj.conf`/`app.overlay` shapes (closes [03 §6.9](03-round-trip.md#69--closed--zephyr-prong-emits-a-nested-west-project)); §5.4 amended for `target.generator: hosted` (no `bsp_generated/`, HAL crate as dependency); §7.8 controller-wiring contract — `App::new` body construction with `capabilities` preset, `[dependencies]` emission for path / version / default cases (closes [03 §6.2](03-round-trip.md#62--closed--controller-libraries-get-a-first-class-manifest-slot)); §8 preamble — explicit feature-graph ownership rule (per-prong template owns `Cargo.toml` `[features]` graph expansion; manifest names leaves) (closes [03 §6.8](03-round-trip.md#68--closed--per-prong-templates-own-the-cargo-feature-graph)). §12 checklist still incomplete; chapter remains DRAFT pending implementation. |
| 2026-04-29 | IMPLEMENTATION | APP-02b: orchestrator emission landed in `src/bin/creator/app.rs` `Orchestrator` + `tests/creator_app_emit.rs`. Implements stage 3 (sub-generator dispatch, sequential — parallel optimization deferred), stage 5 (layout-translator full impl for `rust_inline_v1` per §7.7), and §9.4 inventory tracking (blake3-hashed entries written to `<out>/.rlvgl-app-manifest.json`). Asset-pipeline at v0 is file-copy + `include_bytes!` index per §7.3. SM-gen, theme, i18n, BSP-gen for `creator-bsp-pac`, and full crate scaffold (Cargo.toml `[dependencies]`, src/main.rs, src/app.rs) emit clearly-marked `// TODO(APP-02c)` stubs. All five committed manifests emit successfully under `--out <DIR>`; 7/7 emit-integration tests pass (BBB linux cross-tree splash, beetle bsp_pac BSP-gen stub, beetle esp_hal hosted-no-bsp, H747 freertos + zephyr, inventory invariants). §12 stage-3/5/§9.4 items now satisfied; emission stages 6-7 (full scaffold, `--check`) remain for APP-02c/d. |
| 2026-04-29 | IMPLEMENTATION | APP-02c: stage 6 crate scaffold + per-prong main glue templates. `Cargo.toml` and `README.md` flip from APP-02b stubs to real emission — `Cargo.toml` carries `[package]` + `[[bin]]` (or `[lib] crate-type = ["staticlib"]` for zephyr) + `[features]` from `target.features` + `[dependencies]` for the `controller:` crate. `src/app.rs` emits the §7.8 wiring shim (when `controller:` present, calls `DiscoCapabilities::<preset>()` and constructs `DiscoController`). `src/main.rs` per prong: §8.1 linux loop with `std::thread::sleep`, §8.2 bare_metal `#![no_std]` template with panic handler, §8.3 freertos task-shape comments + render_task body, §8.4 zephyr → `src/lib.rs` `extern "C" fn rlvgl_init()` + nested west project at `zephyr/{CMakeLists.txt, prj.conf, app.overlay, src/main.c}` per §5.4.1. Emit tests updated to match (7/7 still pass alongside 17/17 validator tests; 24/24 total). §12 stage-6 satisfied; only `--check` mode + post-emit `cargo fmt` (APP-02d) remain. |
