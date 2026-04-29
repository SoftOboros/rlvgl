<!--
00-concepts.md - rlvgl Application Schema, Chapter 0: Concepts & Vocabulary.
Status: DRAFT — not yet ratified. See §15 change log.
-->

**[Index](README.md) · Next → (TBD)**

# Chapter 0 — rlvgl Application Schema: Concepts & Vocabulary

> **Status:** RATIFIED 2026-04-27 (see §15). `APP-NN` execution PRs
> MAY cite this chapter as a frozen authority. Amendments require a
> new dated §15 entry and the same review depth as the original
> ratification pass.

## §0 Authority policy

This chapter follows the spec-before-code planning discipline declared in
[`CLAUDE.md`](../../CLAUDE.md) §"Spec-Before-Code Planning Discipline".
RFC 2119 / RFC 8174 normative keywords (**MUST**, **SHOULD**, **MAY**,
etc.) carry their RFC meanings when capitalised; lowercase use is
narrative.

For every concept this chapter names, the authority is one of:

| Domain                      | Authoritative source                                            |
| --------------------------- | --------------------------------------------------------------- |
| Widget tree, event loop     | `core/`, `widgets/` crates                                      |
| Platform / BSP trait shape  | `platform/` crate (`Display`, `Blitter`, input traits)          |
| Chip + board inventory      | `chipdb/rlvgl-chips-*/db/{chips,boards}/*.yaml`                 |
| BSP code generation         | `rlvgl-creator bsp from-yaml`, jinja templates under `examples/.../templates/` |
| Asset conversion            | `rlvgl-creator` (image / RLE / palette pipelines)               |
| State machines              | **External** MCP state-chart generator (peeked, see §10)        |
| Localization                | `i18n/` crate                                                   |
| Layout / theme source-of-record | Figma / UML export (format TBD; see §6)                     |
| Audio meter widgets         | `audio-meters-core/`, `audio-meters-widgets/`                   |

Where a concept also exists in code, this chapter MUST cite the file and
mark the relationship per the *Definitions — reference vs. restatement*
rule in `CLAUDE.md`. Silent restatement is a defect.

## §1 Purpose

This initiative defines a **stable underschema** — a single declarative
manifest describing an rlvgl application — that the existing generators
(creator/jinja BSP gen, asset converter, external state-chart MCP, i18n)
can all consume to emit a buildable example crate.

The schema's job is not to be a new language; it is to be the *contract*
beneath which the runtime crates (`core/`, `widgets/`, `platform/`),
the BSP layer (`chipdb` + creator), the asset pipeline, and the state
machine generator stop changing relative to each other. Above that line,
generators and authoring tools may evolve freely.

A conforming rlvgl application MUST be expressible as:

1. an `app.yaml` manifest (this schema), plus
2. asset source files referenced by relative path, plus
3. an optional state-chart file (e.g. `.scxml`), plus
4. an optional layout source export (Figma JSON / UML), plus
5. an optional i18n bundle.

Given those inputs, `rlvgl-creator app from-yaml` SHALL emit a
buildable Cargo crate equivalent to one of the existing examples in
`examples/`.

## §2 Problem statement

Evidence that the schema is missing today, pinned to code paths:

- **BSP generation already works in isolation.** `chipdb/` ships
  vendor crates with chip + board YAML, and
  `rlvgl-creator bsp from-yaml` emits svd2rust-style PAC bring-up code
  (see [`docs/disco-platform-guide/11-generated-bsps.md`](../disco-platform-guide/11-generated-bsps.md)
  and the `bsp_esp32c3_render` / `bsp_esp32c3_compile` test families).
  But the generated BSP is a fragment, not an application — the consumer
  crate is hand-written (`examples/beetle-esp32c3/src/bsp_pac_main.rs`).
- **Asset conversion lives in creator** but is invoked ad-hoc per
  example. There is no manifest entry that says "this app uses these
  assets, in this pixel format, against this palette."
- **State machines are hand-rolled.** `examples/apps/disco-demo/`
  encodes its UI states as Rust match arms; the external MCP state-chart
  generator (separate repo, not pulled in) is unused here even though
  it produces verification-vector-backed Rust state machines.
- **The four-prong port pattern keeps re-deriving the same wiring.**
  `examples/stm32h747i-disco/` (bare-metal + FreeRTOS), the Zephyr port,
  and `examples/beaglebone-black/` (Linux + bare-metal, with FreeRTOS
  and Zephyr planned per
  [`docs/beaglebone-black/README.md`](../beaglebone-black/README.md))
  all express the same `DiscoController` over different runtimes. Today
  each prong restates the wiring; with a manifest, the wiring is data.
- **Audio meters intentionally cross targets.** `audio-meters-core/`
  + `audio-meters-widgets/` were designed to be reused both inside
  rlvgl and from a TypeScript host (per the project memory note).
  That cross-target intent has no manifest expression today.

The cost of *not* having the schema is that every new example, port,
or product variant re-derives the wiring by copy/paste, and the
existing generators stay fragmentary.

## §3 Canonical glossary

Definitions; cite-vs-restate marker on each term. Terms are
**Standards Action** unless noted (see §5 registration policy).

- **Application manifest** (`app.yaml`) — *Owned by this chapter; does
  not exist in repo yet.* The single declarative file naming a target,
  state machine, assets, screens, and i18n bundle.
- **Target** — *As defined in `chipdb/rlvgl-chips-*/db/boards/*.yaml`;
  used without modification.* A `(vendor, board_id)` pair that selects
  a chipdb board YAML and the prong (Linux / bare-metal / FreeRTOS /
  Zephyr) it runs on.
- **Prong** — *As defined in `docs/beaglebone-black/README.md`; adapted:
  generalised from BBB-specific to any board.* The runtime flavour:
  `linux`, `bare_metal`, `freertos`, or `zephyr`. Frozen set per §5.
- **Screen** — *Owned by this chapter; partially derivable from
  `widgets/`.* A named root widget tree paired with a state name. The
  manifest references screens by id; widget trees are emitted from
  layout source (§6).
- **State** — *As defined by the external MCP state-chart generator's
  output; used without modification.* A node in the state machine. The
  manifest references states by id; the state machine itself is
  authored externally (§10).
- **Asset class** — *Owned by this chapter.* The kind-tag on an asset
  (`image_rgb565`, `image_rle_a8`, `palette`, `font`, `audio_pcm`,
  `audio_lufs_capture`, `icon`). Frozen set per §5.
- **Theme** — *Partially overlaps with `chakra/` design tokens; adapted:
  manifest theme is the runtime-side projection.* Named bundle of
  colours / spacing / fonts referenced by widgets; sourced from Figma /
  Chakra tokens.
- **Wiring** — *Owned by this chapter.* The glue layer the generator
  emits: input → state-machine event mapping, state → screen mapping,
  asset → widget mapping, BSP init → app `main()`.
- **Generator** — *Owned by this chapter.* Any tool that consumes the
  manifest (or a fragment of it) and emits Rust/asset/data output.
  The BSP generator and the external state-chart MCP are both
  generators in this sense.

## §4 Source-of-truth map

One row per concept; one owner per concept. If two trees claim
authority over the same row, the schema has a defect — file an
amendment in §15 before writing code that depends on the conflict.

| Concept                  | Owner (canonical)                        | Mirrored / consumed by                                  |
| ------------------------ | ---------------------------------------- | ------------------------------------------------------- |
| Chip inventory           | `chipdb/rlvgl-chips-<vendor>/db/chips/`  | creator BSP gen, app manifest `target.chip` ref         |
| Board inventory          | `chipdb/rlvgl-chips-<vendor>/db/boards/` | creator BSP gen, app manifest `target.board` ref        |
| Prong set                | This chapter §5                          | per-target conditional sections in manifest             |
| Pixel format set         | `core/` (`PixelFmt`)                     | manifest asset entries cite by name                     |
| Widget palette           | `widgets/` crate                         | layout source export, manifest screen refs              |
| State machine definition | External MCP state-chart repo            | manifest references `.scxml` path; generator emits crate |
| Verification vectors     | External MCP state-chart repo            | emitted alongside SM crate; rlvgl test harness consumes |
| Asset binary format      | `rlvgl-creator` asset pipeline           | manifest asset class names match pipeline output        |
| Theme tokens             | `chakra/` (design tokens, TS-side)       | manifest theme bundle is the consumed projection        |
| i18n strings             | `i18n/` crate                            | manifest i18n bundle path                               |
| BSP code                 | creator BSP gen (jinja templates)        | per-prong glue in generated app crate                   |
| Audio meter widgets      | `audio-meters-{core,widgets}/`           | manifest screen refs                                    |
| Initiative prefix        | This chapter §5                          | execution PR commit subjects (`APP-NN[a-z]:`)           |

## §5 Frozen decisions — enums & registration policy

Each frozen enum names its registration policy per the *Frozen
enumerations — registration policy* rule in CLAUDE.md.

### 5.1 Prong set — Standards Action

```text
{ linux, bare_metal, freertos, zephyr }
```

Adding a prong requires a §15 amendment in this chapter and
chipdb-side board-YAML schema review. Removing a prong requires a
deprecation cycle that lands in the same amendment.

### 5.2 Vendor set — Standards Action (delegates to chipdb)

This schema does NOT redefine the chipdb vendor set. It cites the
chipdb's own enum (currently `{esp, stm, ti, nxp, nrf, renesas,
silabs, rp2040, microchip}`) by reference. Adding a vendor is a
chipdb-side amendment, not a manifest-side one.

### 5.3 Asset class set — Specification Required

```text
{ image_rgb565, image_rle_a8, palette, font, audio_pcm,
  audio_lufs_capture, icon }
```

Adding a class requires a per-chapter walkthrough update once §6 (asset
schema) lands; no §0 amendment. Class names MUST match the
`rlvgl-creator` asset pipeline's emitted artifact tags.

### 5.4 Initiative prefix — Standards Action

`APP-NN[a-z]:` for execution PRs scoped to this initiative. Matches
the `DISCO-`, `BBB-`, `CREATOR-`, `CHIPS-<VENDOR>-` convention in
CLAUDE.md.

### 5.5 State-machine boundary — Standards Action — **TBD, see §10**

The schema's most contentious frozen decision: does the manifest
*include* state-machine definition inline, or does it *reference* an
external `.scxml` / UML file by relative path? §10 names this as the
single biggest reconciliation question; this section MUST be filled
before any `APP-` PR.

## §6 Frozen decisions — manifest structure (sketch)

> *Not yet frozen.* This is a strawman to argue against. Concrete
> field names, types, and required-vs-optional are the work of the
> next chapter (`01-manifest-schema.md`).

```yaml
# app.yaml (strawman)
schema: rlvgl-app/v0
name: my-app
target:
  vendor: esp           # cites chipdb vendor set
  board: beetle_esp32c3 # cites chipdb board id
  prong: bare_metal     # cites §5.1

state_machine:
  source: states/main.scxml   # external authoring
  generator: mcp-statechart   # named external generator (§10)

assets:
  - id: splash
    class: image_rgb565
    source: assets/splash.png
  - id: ui-font
    class: font
    source: assets/inter-16.ttf

screens:
  - id: home
    state: idle
    layout: layouts/home.figma.json
  - id: settings
    state: settings
    layout: layouts/settings.figma.json

theme:
  source: themes/dark.tokens.json

i18n:
  bundle: i18n/strings.toml
  default_locale: en-US
```

## §7 Frozen decisions — wiring contract (sketch)

> *Not yet frozen.* The wiring contract names what the *generated app
> crate* MUST contain so that any prong can run it. Strawman:
>
> - A `App::new(bsp)` constructor that consumes the BSP handle and
>   returns an opaque app value.
> - An `App::tick(now, inputs) -> Outputs` step that is prong-agnostic.
> - Per-prong `main` glue (one of the four prong runtimes) that calls
>   `App::tick` from the appropriate scheduling primitive.

The wiring contract MUST be expressible without referencing any
chip-specific register names — those live behind the BSP trait surface
in `platform/`.

## §8 Frozen decisions — verification vectors

> *Not yet frozen.* The external state-chart MCP emits verification
> vectors alongside its generated SM crate. This section MUST name how
> the rlvgl test harness consumes them: as a `#[test]` family, as a
> `playit` script, or both. See §10.

## §9 Frozen decisions — round-trip property

> *Proposed for freeze:* Reverse-engineering an existing example crate
> into a manifest, then emitting from that manifest, MUST produce a
> crate that builds and passes its own pre-publish phases. The
> round-trip property is the schema's correctness oracle.
>
> Initial round-trip targets (one per prong):
>
> - `examples/apps/disco-demo/` (FreeRTOS prong)
> - `examples/beetle-esp32c3/` (bare-metal, esp_hal feature)
> - `examples/beaglebone-black/` (Linux prong)
> - `examples/stm32h747i-disco/` Zephyr build (Zephyr prong)

## §10 Reconciliation with adjacent repo primitives

The non-trivial coupling questions. Each item names the conflict and
the proposed resolution; resolution becomes binding only when listed
in §15 with a ratification date.

### 10.1 BSP generator vs. hand-written H747 platform

`platform/src/stm32h747i_disco.rs` is hand-written; the
chipdb-driven creator BSP gen produces a different code shape. The
manifest MUST NOT pretend the H747 BSP was generated. **Proposed:**
the schema's `target` for `stm32h747i_disco` cites the hand-written
platform module and SHOULD NOT trigger BSP-gen output. Generator
output is opt-in per board.

### 10.2 State-machine boundary (the biggest open question)

Two options, named so we can argue them:

- **Option A — Reference by path.** The manifest cites a path to an
  external `.scxml` / UML file. The state-chart MCP generator is
  invoked separately (or as a build-rs hook) and emits a sibling
  Rust crate; the manifest's `state_machine.source` is just a path
  and a generator name. Decouples release cycles. Cheaper to iterate.
- **Option B — Inline fragment.** The manifest contains the SM
  definition inline (as a YAML subtree). The generator consumes the
  whole manifest and emits both BSP and SM. Tighter, but binds the
  manifest's schema version to the SM generator's schema version.

**Recommendation:** Option A for v0. Promote to B only if path-based
references prove painful in practice across ≥2 round-trip targets.

### 10.3 Asset pipeline ownership

`rlvgl-creator` already converts assets. The manifest does NOT
re-implement conversion — it names assets by id, class, and source
path; the generator pipeline owns format, palette negotiation, and
output layout. **Proposed:** the manifest's asset class names are a
view of the pipeline's tag enum, not a parallel one.

### 10.4 Theme / chakra / Figma overlap

`chakra/` (Next.js / TS) holds design tokens; Figma holds visual
layout; rlvgl renders the result. The chakra-tokens-to-rlvgl path
is **not aspirational** — the softoboros site's chakra theme has
already been exported to the STM32H747I-DISCO via `rlvgl-creator`,
which is the working precedent `chakra_tokens_v1` formalizes (see
[01 §5.7](01-manifest-schema.md#57-theme-optional) and [01 §10.4](01-manifest-schema.md#104-themeformat-chakra_tokens_v1-vs-live-chakra-tokens)).
A Svelte-side token export also exists as an alternate
source-of-record but is *not* canonical at v0 — chakra is more
internally consistent across the authoring stack and is the only
theme source recognised in the v0 grammar.

**Resolved:** the manifest's `theme.source` references a chakra
tokens JSON export (output of `extendTheme(...)` serialized as
JSON, the same shape a `ChakraProvider` would consume). The
manifest's `screens[].layout` references a Figma export (format
TBD in §6). The runtime does NOT consume Figma or live chakra
runtimes directly — the generator translates layout source and
tokens into widget-tree code at build time.

### 10.5 Audio meters cross-target

`audio-meters-{core,widgets}/` are designed to be reused from a TS
host. **Proposed:** widget references in the manifest are
prong-agnostic; the cross-target story is a §11 non-goal for v0
(the manifest emits Rust; TS consumption is a separate generator).

### 10.6 i18n vs. layout

Strings can live in two places: the i18n bundle (per-locale) or the
layout (hard-coded). **Proposed:** the manifest's
`screens[].layout` source MAY contain string ids; the generator
resolves ids against the i18n bundle at build time. Hard-coded
strings in layout are a generator warning, not an error, in v0.

## §11 Non-goals

Explicit out-of-scope for v0 of this schema:

- **Hot reload.** The manifest is a build-time artifact, not a runtime
  one. No dynamic re-layout from manifest changes without rebuild.
- **TypeScript host emission.** The generator emits Rust. Cross-target
  TS emission for shared widgets (audio meters) is a separate
  initiative.
- **Multi-app projects.** One manifest, one app. Workspaces of related
  apps are a v1 concern.
- **Configuration-style runtime tunables.** The manifest is structural
  (what the app *is*), not behavioural (how it's tuned at runtime).
  Runtime knobs stay in code or runtime config files.
- **Replacing `chipdb/`.** This schema cites chipdb; it does not
  duplicate or wrap it.
- **Replacing the external state-chart MCP.** §10.2 names the
  boundary; the schema does not re-implement state-machine codegen.

## §12 Acceptance checklist

This concepts chapter is ratified (§15 entry dated) when:

- [x] §0 authority table reviewed against `CLAUDE.md` source-of-truth
      claims; no silent restatement defects.
- [x] §3 glossary terms each carry a cite-vs-restate marker.
- [x] §4 source-of-truth map has exactly one owner per row.
- [x] §5.1 (prong set) and §5.2 (vendor delegation) confirmed.
- [x] §5.5 / §10.2 (state-machine boundary) decided — **Option A**
      ratified 2026-04-27.
- [x] §9 round-trip targets each have a reverse-engineered manifest
      candidate — satisfied by [chapter 03](03-round-trip.md) §5.1–§5.4.
- [x] §11 non-goals reviewed; nothing in scope is silently excluded.
- [x] §15 has a dated ratification entry signed off by the initiative
      owner.

The next chapter (`01-manifest-schema.md`, *not yet written*) is
unblocked by ratification of §3, §4, §5, §10. §6, §7, §8 are sketches
here and become normative in their own chapters.

## §13 Files cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline,
  enum registration policy, initiative-prefix convention.
- [`docs/beaglebone-black/README.md`](../beaglebone-black/README.md) —
  prong-set vocabulary, source for the four-prong pattern.
- [`docs/disco-platform-guide/11-generated-bsps.md`](../disco-platform-guide/11-generated-bsps.md)
  — BSP generator status, cited by §2.
- [`chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml`](../../chipdb/rlvgl-chips-esp/db/boards/beetle_esp32c3.yaml)
  — example board YAML cited by §4 and §5.2.
- [`examples/apps/disco-demo/`](../../examples/apps/disco-demo/) —
  round-trip target cited by §9.
- [`examples/beetle-esp32c3/`](../../examples/beetle-esp32c3/) —
  round-trip target cited by §9.
- [`platform/src/stm32h747i_disco.rs`](../../platform/src/stm32h747i_disco.rs)
  — hand-written BSP cited by §10.1.
- [`audio-meters-core/`](../../audio-meters-core/),
  [`audio-meters-widgets/`](../../audio-meters-widgets/) — cross-target
  precedent cited by §10.5.
- [`chakra/`](../../chakra/) — design-token source cited by §10.4.
- [`i18n/`](../../i18n/) — localization owner cited by §4.

## §14 Unblocks

Ratifying this chapter unblocks:

- `01-manifest-schema.md` — concrete YAML grammar, types, required vs.
  optional fields, validation rules.
- `02-generator-pipeline.md` — how `rlvgl-creator app from-yaml`
  composes the existing BSP / asset / SM generators.
- `03-round-trip.md` — reverse-engineering the §9 targets into
  manifests.
- `04-state-machine-boundary.md` — full Option A / B treatment with
  prototype code from both sides.

## §15 Change log

| Date       | Status | Note                                                                                  |
| ---------- | ------ | ------------------------------------------------------------------------------------- |
| 2026-04-27 | DRAFT  | Initial skeleton. Not ratified. Argument target — review §10 first, then §4, then §5. |
| 2026-04-27 | DRAFT  | §10.2 resolved: Option A (state machine referenced by external `.scxml`/`.uml` path). §10.4 corrected: chakra → 747i-disco theme export already shipped via `rlvgl-creator`; svelte alternate noted but not canonical at v0. §12 checklist still incomplete; chapter remains DRAFT. |
| 2026-04-27 | RATIFIED | Owner: Ira Abbott. All §12 items closed via the convergence-pass amendments propagated through chapters 01/02/03. `APP-NN` execution PRs may now cite this chapter as a frozen authority. Future amendments require a new dated entry and matching review depth. |
