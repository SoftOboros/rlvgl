# APP-05-A — Cargo `[features]` graph + `[dependencies]` emission

**Status:** **Resolved 2026-05-04 — APP-05 family complete (a/b/c/d/e + f shipped same day).**
Folded into chapter 02 §15 via the 2026-05-04 IMPLEMENTATION entry.
This file is preserved as historical analysis only; behaviour PRs
reference chapter 02 §8 preamble (frozen rule) and the
`feature_graphs` module directly. Sub-letter analysis for the
APP-05 family named in [chapter 02 §15](02-generator-pipeline.md#§15-change-log)
(2026-04-29 RATIFIED entry: "full Cargo `[features]` graph
expansion ... tracked under APP-05+"). This doc proposes the
v0 shape of the per-prong feature graph + dependency emission,
and sequences the implementation across `APP-05a`–`APP-05f`.

The §8 preamble already pins the rule
([02 §8](02-generator-pipeline.md#§8-per-prong-main-glue-normative-templates)):

> The manifest names *what the user wants enabled*; the per-prong
> template knows *what enabling that means in the emitted crate*.

This sub-letter does not reopen that rule. It commits to a
table shape, an ownership location, and a per-phase sequencing
to ship it.

## §1 Purpose

Replace the current placeholder Cargo.toml emission in
`src/bin/creator/app.rs::emit_cargo_toml` (line 2158, APP-02c) —
where every manifest feature emits as `feat = []` and the
`[dependencies]` table is a single
`# TODO(template-tuning):` comment — with real per-prong/per-
generator output that round-trips against the five committed
reference Cargo.toml files in `examples/`.

## §2 Problem statement

### 2a. Current orchestrator emit (APP-02c, today)

Today's `emit_cargo_toml` (lines 2201–2265) produces, for the
H747 freertos manifest's `target.features: [cm7, freertos,
adapted_cmd, dma2d, splash, desktop]`:

```toml
[features]
default = ["cm7", "freertos", "adapted_cmd", "dma2d", "splash", "desktop"]
cm7 = []
freertos = []
adapted_cmd = []
dma2d = []
splash = []
desktop = []

[dependencies]
# Controller library (chapter 01 §5.10 / chapter 02 §7.8).
rlvgl-app-disco-demo = { path = "../apps/disco-demo" }

# TODO(template-tuning): rlvgl runtime + chipdb + per-generator
# HAL deps. Manifest target.generator=hand_written target.prong=freertos
```

### 2b. Reference Cargo.toml (the same H747 freertos intent)

`examples/stm32h747i-disco/Cargo.toml` actually carries:

```toml
[features]
default = []
cm7 = ["rlvgl-platform/stm32h747i_disco", "stm32h7/stm32h747cm7",
       "dep:stm32h7xx-hal", "dep:embedded-hal", "dep:embedded-hal-02",
       "dep:embedded-sdmmc"]
freertos = ["rlvgl-platform/freertos"]
splash = ["rlvgl-platform/splash"]
dma2d = ["rlvgl-platform/dma2d"]
audio = ["rlvgl-platform/audio"]
sd_storage = ["rlvgl-platform/sd_storage"]
adapted_cmd = []
desktop = []
zephyr = []

[dependencies]
rlvgl-core = { path = "../../core", default-features = false }
rlvgl-platform = { path = "../../platform", default-features = false }
rlvgl-widgets = { path = "../../widgets", default-features = false }
rlvgl-ui = { path = "../../ui", default-features = false }
rlvgl-i18n = { path = "../../i18n" }
rlvgl-app-disco-demo = { path = "../apps/disco-demo", ... }
rlvgl-decomp = { path = "../../rlvgl-decomp" }
rlvgl-playit = { path = "../../playit", default-features = false }
cortex-m-rt = "0.7"
cortex-m = { version = "0.7", features = ["critical-section-single-core"] }
embedded-alloc = "=0.5.1"
panic-halt = "1"
stm32h7 = { version = "0.15.1", features = ["rt"] }
critical-section = "1.1.2"

[target.'cfg(any(target_arch = "arm", target_os = "none"))'.dependencies]
stm32h7xx-hal = { version = "0.16", optional = true, features = [...] }
embedded-hal = { version = "1", optional = true }
...
```

The two diverge along three axes:

1. **Feature graph**: `freertos = []` vs
   `freertos = ["rlvgl-platform/freertos"]`. Same for `splash`,
   `dma2d`, `audio`, `sd_storage`. The `cm7` leaf in particular
   carries both crate-feature activations and `dep:` activations
   for optional HAL crates.
2. **`default`**: orchestrator emits `default = target.features`;
   reference emits `default = []`. The reference relies on
   `[[bin]] required-features = ["cm7"]` instead.
3. **`[dependencies]` + cross-compile-only deps**: orchestrator
   emits only the controller crate; reference emits the rlvgl
   workspace runtime crates (`core` / `platform` / `widgets` /
   `ui` / `i18n` / `decomp` / `playit`), per-prong runtime
   primitives (`cortex-m` family for bare-metal/freertos,
   `libc` + `heapless` for linux), and per-vendor PAC/HAL crates
   (`stm32h7`, `stm32h7xx-hal` under a target-cfg block).

The same divergence exists in shape (not detail) for all four
prongs and all three generators (`hand_written`, `hosted`,
`creator-bsp-pac`).

## §3 Option set — where does the feature-graph table live?

### Option 1. In-binary Rust tables (RECOMMENDED for v0)

A new module `src/bin/creator/app/feature_graphs.rs` exporting
per-prong/per-generator/per-vendor data:

```rust
pub struct ProngTemplate {
    pub base_deps: &'static [Dep],
    pub target_cfg_deps: &'static [(&'static str, &'static [Dep])],
    pub feature_expansions: &'static [(&'static str, &'static [&'static str])],
    pub default_features: DefaultPolicy,
    pub bin_required_features: Option<&'static [&'static str]>,
}
pub fn lookup(prong: &str, generator: &str, vendor: &str, board: &str)
    -> Option<&'static ProngTemplate>;
```

Pros:
- Zero new public surface (no manifest grammar change, no chipdb
  YAML accessor).
- Trivially unit-testable (round-trip against the five reference
  Cargo.tomls as fixtures).
- Naming convention already exists (`feature_graphs.rs` next to
  the orchestrator).

Cons:
- Adding a new round-trip target (e.g. a sixth `app.yaml`)
  requires a creator code change.
- External boards not on the round-trip allow-list still emit
  the placeholder.

### Option 2. YAML alongside chipdb

A new file family
`chipdb/rlvgl-chips-<vendor>/db/feature_graphs/<board>.yaml`,
loaded via the existing chipdb accessor convention.

Pros:
- Mirrors the `bsp from-yaml` data flow.
- Third-party board crates can ship their own table.

Cons:
- New public surface to freeze under chapter 01 §5 — touches a
  ratified chapter.
- Cross-prong features (`splash`, `dma2d`, `desktop`, `playit`)
  are not naturally per-board; they are runtime workspace
  features. The natural partition is by *prong* (linux,
  bare_metal, freertos, zephyr) plus by *vendor/board* only for
  the chip-specific HAL/PAC slice.
- Splits the per-prong template's "ownership" across two files
  (one in-tree, one in chipdb), which the §8 preamble's "the
  template owns the graph" wording does not anticipate.

### Option 3. Hybrid — base in-binary, extension via manifest

Per-prong base deps in-binary; the manifest gains a
`target.feature_graph: <path>` field pointing at a project-
specific YAML overlay merged at emit time.

Pros: extensible without a creator code change.

Cons: new manifest grammar field — touches frozen chapter 01 §5,
needs a §15 amendment and v0 → v1 migration discussion. Out of
scope for the v0 round-trip parity goal.

## §4 Recommendation

Adopt **Option 1**. v0's job is round-trip parity against the
five committed reference targets, all of which are in-tree.
External boards are a v1 concern (§10).

If a future initiative needs external-board extensibility,
Option 3 (manifest extension hook) is the natural escalation —
Option 2's split ownership conflicts with the §8 preamble.

## §5 Proposed table shape

`src/bin/creator/app/feature_graphs.rs`:

```rust
pub struct Dep {
    pub name: &'static str,
    pub source: DepSource,
    pub default_features: bool,
    pub features: &'static [&'static str],
    pub optional: bool,
}

pub enum DepSource {
    Path(&'static str),                     // workspace-relative
    Version(&'static str),                  // crates.io
    PackageRename { package: &'static str, version: &'static str },
}

pub enum DefaultPolicy {
    Empty,                                  // default = []
    AllManifestFeatures,                    // default = target.features
    Explicit(&'static [&'static str]),      // default = [<list>]
}

pub struct ProngTemplate {
    pub base_deps: &'static [Dep],
    pub target_cfg_deps: &'static [(&'static str, &'static [Dep])],
    pub feature_expansions: &'static [(&'static str, &'static [&'static str])],
    pub default_features: DefaultPolicy,
    pub bin_required_features: Option<&'static [&'static str]>,
    pub extra_bins: &'static [ExtraBin],
}

pub struct ExtraBin {
    pub name: &'static str,
    pub path: &'static str,
    pub required_features: &'static [&'static str],
}
```

Lookup is by composite key: the orchestrator builds a
`(prong, generator, vendor, board)` tuple, falls back through
`(prong, generator, vendor, "*")` and
`(prong, generator, "*", "*")`, and errors cleanly if no
template matches (the user gets the current placeholder
behaviour with a `// TODO(APP-05): no template for ...` comment
in the emit, not a hard failure — preserves backwards-compat
for any out-of-allow-list manifest a user might author locally).

Feature-expansion lookup composes: a manifest feature `splash`
under the `freertos` + `hand_written` + `stm` + `stm32h747i_disco`
template expands to whatever the matched template's
`feature_expansions` table says. Manifest features absent from
the table emit as `feat = []` (current behaviour) so authors
can introduce ad-hoc app-level features without an orchestrator
change.

## §6 Coverage matrix

The five committed round-trip manifests (chapter 03) span six
`(prong, generator, vendor, board)` triples (the H747 freertos
manifest and the H747 sm-bearing manifest share a triple):

| `app.yaml`                                         | prong       | generator        | vendor | board                          | reference Cargo.toml                                |
| -------------------------------------------------- | ----------- | ---------------- | ------ | ------------------------------ | --------------------------------------------------- |
| `examples/beaglebone-black/app.yaml`               | `linux`     | `hand_written`   | `ti`   | `beaglebone_black_nhd_cape`    | `examples/beaglebone-black/Cargo.toml`              |
| `examples/beetle-esp32c3/app.yaml`                 | `bare_metal`| `hosted`         | `esp`  | `beetle_esp32c3`               | `examples/beetle-esp32c3/Cargo.toml` (esp-hal half) |
| `examples/beetle-esp32c3/app-bsp-pac.yaml`         | `bare_metal`| `creator-bsp-pac`| `esp`  | `beetle_esp32c3`               | `examples/beetle-esp32c3/Cargo.toml` (bsp-pac half) |
| `examples/stm32h747i-disco/app.yaml`               | `freertos`  | `hand_written`   | `stm`  | `stm32h747i_disco`             | `examples/stm32h747i-disco/Cargo.toml`              |
| `examples/stm32h747i-disco/app-with-sm.yaml`       | `freertos`  | `hand_written`   | `stm`  | `stm32h747i_disco`             | (same)                                              |
| `examples/stm32h747i-disco/app-zephyr.yaml`        | `zephyr`    | `hand_written`   | `stm`  | `stm32h747i_disco`             | `examples/stm32h747i-disco/Cargo.toml`              |

**Same Cargo.toml, two prongs.** The H747 reference Cargo.toml
serves both the freertos and zephyr manifests — they differ only
in the manifest's feature set and in the `[lib]
crate-type = ["staticlib"]` block (already emitted today by
APP-02c when `target.prong == "zephyr"`). The feature-graph
template can therefore be looked up by `(prong, generator,
vendor, board)` and the two H747 lookups land on different
templates whose deps overlap heavily. Open question for §7
sequencing: factor that overlap into a shared sub-table
(`H747_BASE_DEPS`) so APP-05d and APP-05e share data, or
duplicate.

**Two manifests, one Cargo.toml.** The beetle Cargo.toml has
two `[[bin]]` entries gated by `required-features = ["esp_hal"]`
and `["bsp_pac"]`. APP-05b and APP-05c emit *the same*
Cargo.toml from two different manifests — the second emit is a
no-op when the first has already landed in `<out>`. The §9.4
inventory + `--check` semantics already handle this: both
manifests emit the same union of `[features]` and
`[dependencies]`, just driven by different `target.features`
leaf sets.

This is the first round-trip target where two manifests legally
emit a byte-equal Cargo.toml. The orchestrator MAY treat this
as a discovery rule (compose union) or MAY require the user to
emit the two manifests in sequence (each adds the other's
features as `feat = []` placeholder, then the second emit fills
them in). The simpler rule is **per-manifest emit; idempotent
under union**: APP-05b/c each emit their own complete Cargo.toml
including the *other* manifest's leaves as un-expanded
placeholders. This requires the table to know the full beetle
feature set up front, not just the slice the current manifest
selects.

## §7 Per-phase sequencing

| Phase    | Scope                                              | Round-trip target(s)                                                                  |
| -------- | -------------------------------------------------- | ------------------------------------------------------------------------------------- |
| APP-05a  | `linux` + `hand_written` + `ti` + BBB              | `examples/beaglebone-black/app.yaml`                                                  |
| APP-05b  | `bare_metal` + `hosted` + `esp` + beetle           | `examples/beetle-esp32c3/app.yaml`                                                    |
| APP-05c  | `bare_metal` + `creator-bsp-pac` + `esp` + beetle  | `examples/beetle-esp32c3/app-bsp-pac.yaml`                                            |
| APP-05d  | `freertos` + `hand_written` + `stm` + H747         | `examples/stm32h747i-disco/app.yaml` + `app-with-sm.yaml`                             |
| APP-05e  | `zephyr` + `hand_written` + `stm` + H747           | `examples/stm32h747i-disco/app-zephyr.yaml`                                           |
| APP-05f  | Discipline scanner — strip `# TODO(template-tuning):` placeholder; assert no generated Cargo.toml carries that marker | meta-test gating future regressions |

Phases are independent (each lands its own template entry without
touching the others) but share the table shape from §5. APP-05a
is the simplest (no cross-compile target deps, no PAC HAL gymnastics)
and serves as the shape exemplar.

## §8 Acceptance gates

Per-phase, each implementation PR MUST satisfy:

- A new fixture `tests/creator_app_feature_graph.rs` (or extension
  of `tests/creator_app_emit.rs`) that:
  - emits the round-trip target's Cargo.toml under `<temp>/`;
  - parses both the emitted file and the reference file as toml
    (`cargo_toml = "0.20"` is already in the workspace dep tree
    for the chipdb crates);
  - asserts byte-equal `[features]` table modulo key ordering;
  - asserts the emitted `[dependencies]` set (by name + source
    kind) is a superset of the reference, with diff diagnostics
    listing each missing entry. Subset semantics permit the
    template to legitimately omit dev-only deps (`build-
    dependencies`, dev profiles) at v0.
- The orchestrator's `--check` mode (APP-02d) must pass on the
  emitted output two runs in a row (determinism, §9.1).
- No new manifest grammar fields (touches frozen chapter 01 §5
  → out of scope at v0).

The initiative-wide acceptance gate — APP-05f — is:

- The discipline scanner (probably extending
  `tests/creator_app_emit.rs::no_template_tuning_todo`) asserts
  no emitted `Cargo.toml` under any of the five round-trip
  targets carries the literal string `TODO(template-tuning)`
  and no `feat = []` line where the manifest's
  `target.features` includes `feat`.

## §9 Reconciliation with adjacent invariants

- **Chapter 01 §5.2 grammar** (`target.features`): unchanged.
  This sub-letter MUST NOT add a `target.feature_graph` field
  or any other manifest grammar element. The lookup table is an
  orchestrator implementation detail, not a manifest contract.
- **Chapter 01 §5.6 hand_written allow-list** (`stm32h747i_disco`,
  `beaglebone_black_nhd_cape`): unchanged. The two `hand_written`
  templates (BBB + H747) live in `feature_graphs.rs` because
  those are the only two boards on the allow-list — adding a
  third would require both the §5.6 amendment and an
  APP-05g-style template addition.
- **Chapter 02 §7.x sub-generators**: orthogonal. BSP-gen
  (§7.2), asset-pipeline (§7.3), SM-gen (§7.4), i18n (§7.5),
  theme (§7.6) all complete before crate scaffold (stage 6) per
  the §5.1 stage graph. APP-05 is the stage-6 Cargo.toml
  emitter; the upstream sub-generators don't see this code.
- **Chapter 02 §9.1 byte-determinism**: lookup tables are
  `&'static`; expansion is iter+sort. Determinism preserved by
  construction.
- **Chapter 02 §9.4 inventory**: emitted `Cargo.toml` is already
  inventoried with `stage = "scaffold"` (line 2267); APP-05
  doesn't change inventory shape.
- **Discipline scanner** (`platform/tests/discipline.rs`):
  unrelated. APP-05's output crates depend on `rlvgl-platform`
  but don't change platform-level register-mashing rules.

## §10 Non-goals (v0)

- External board feature-graph extension (Option 2/3). v1
  concern; gate on a real external-board use case landing.
- Compile-verification of the emitted Cargo.toml. Chapter 02 §11
  already classifies compile-verify as v1.
- Auto-derivation of `build.rs` / `build-dependencies`. Reference
  Cargo.tomls carry these (BBB has `cc = "1"` for fbdev; beetle
  has memory-x emit); v0 emits a `// TODO(APP-05+): build.rs`
  comment in `Cargo.toml` if the manifest target needs one. A
  follow-up `APP-05g` may automate.
- Cargo profile customization (`[profile.release]` opt-level
  knobs). Not present in the manifest grammar; out of scope.
- Workspace integration (`[workspace]` membership). The five
  reference Cargo.tomls all live under the rlvgl workspace
  root; the emitted crates inherit this implicitly. v1 may
  formalize.

## §11 Files cited

- [`docs/app-schema/02-generator-pipeline.md`](02-generator-pipeline.md)
  §8 preamble (frozen rule), §15 (RATIFIED entry naming APP-05+
  as the family).
- [`docs/app-schema/03-round-trip.md`](03-round-trip.md) §6.8
  (CLOSED disposition).
- [`src/bin/creator/app.rs`](../../src/bin/creator/app.rs)
  `emit_cargo_toml` (line 2158) — the function this initiative
  replaces.
- `examples/stm32h747i-disco/Cargo.toml` — reference for
  APP-05d/e.
- `examples/beaglebone-black/Cargo.toml` — reference for
  APP-05a.
- `examples/beetle-esp32c3/Cargo.toml` — reference for
  APP-05b/c.
- `examples/stm32h747i-disco/app-with-sm.yaml` — second
  manifest covered by the APP-05d template.

## §12 Change log

| Date       | Status | Note                                                                                                     |
| ---------- | ------ | -------------------------------------------------------------------------------------------------------- |
| 2026-05-04 | DRAFT  | Initial sub-letter analysis. Option 1 (in-binary tables) recommended; per-phase sequencing APP-05a–f.    |
| 2026-05-04 | RESOLVED | APP-05 family complete same day. APP-05a–e shipped with five `ProngTemplate` entries (BBB / beetle esp_hal / beetle bsp_pac / H747 freertos / H747 zephyr); H747 base + cross-compile dep tables factored to shared statics for APP-05d/e to share. APP-05f discipline scanner asserts every committed manifest's `(prong, generator, vendor, board)` tuple resolves to a template and every manifest feature appears in that template's `feature_expansions`. 19/19 parity tests pass. Closes the §8 preamble's feature-graph rule at v0. Open question from §6 ("two manifests, one Cargo.toml" union for the beetle esp_hal/bsp_pac pair) deferred to v1+: the orchestrator emits one Cargo.toml per manifest into its own `<out>` directory; composing both intents into a shared output tree was not needed at v0. |
