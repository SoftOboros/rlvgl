<!--
README.md - rlvgl Application Schema, initiative index.
Status: All five chapters RATIFIED 2026-04-27 / 2026-04-29.
-->

# rlvgl Application Schema

> **Status:** All five chapters RATIFIED (owner: Ira Abbott; 00 and
> 01 on 2026-04-27, 03 and 02 and 04 on 2026-04-29). `APP-NN[a-z]`
> execution PRs MAY cite any chapter as a frozen authority.
>
> **Implementation status as of 2026-04-30:** chapter 02 §7 sub-
> generators are ALL real — APP-02e (BSP-gen), APP-02f (i18n),
> APP-02g (theme), APP-02h (parallel stage 3 dispatch), APP-04c
> (vendored-crate SM consumption + CV-1 cross-validate) all
> shipped. APP-04b (first SM-bearing round-trip target —
> `examples/stm32h747i-disco/app-with-sm.yaml` paired with the
> vendored `disco-demo-states/` SM crate) shipped 2026-04-30; the
> external `mcp-statechart` codegen produces a clean self-manifest
> with `state_set` and 6/6 vectors passing host-side via
> `make verify-all`. Orchestrator now honors the §5.4 wrapper-shape
> discriminator (`Cargo.toml` present → sibling, no inline copy).
>
> **APP-05 family shipped 2026-04 → 2026-05-04.** Per-prong /
> per-generator / per-vendor / per-board feature-graph templates
> wired into `emit_cargo_toml`; sub-letter analysis at
> [`APP-05-A.md`](APP-05-A.md). All six committed round-trip
> manifests now emit Cargo.toml's with feature expansions
> set-equal to the reference and `[dependencies]` subsets of the
> reference. APP-05f discipline scanner enforces template
> registration as a closed-list invariant.
>
> **Remaining v0.5 work:** figma/uml layout formats (chapter 01
> §5.5 v1 work — `rust_inline_v1` is the de facto path until a
> real authoring pipeline ships first). These don't reopen
> ratified chapters. See [`CLAUDE.md`](../../CLAUDE.md)
> §"Spec-Before-Code Planning Discipline" for what ratification
> means.

This initiative defines a **stable underschema** — a single
declarative manifest (`app.yaml`) describing an rlvgl application —
that the existing generators (`rlvgl-creator` BSP gen + asset
pipeline, the external MCP state-chart generator, the `i18n` crate)
can all consume to emit a buildable example crate.

The schema's job is to be the *contract* beneath which the runtime
crates (`core/`, `widgets/`, `platform/`), the BSP layer (`chipdb`
+ creator), the asset pipeline, and the state-machine generator
stop changing relative to each other. Above that line, generators
and authoring tools may evolve freely.

## Chapters

| #  | Title                                                              | Status  |
| -- | ------------------------------------------------------------------ | ------- |
| 00 | [Concepts & Vocabulary](00-concepts.md)                            | **RATIFIED** |
| 01 | [Manifest Grammar (`app.yaml`, `rlvgl-app/v0`)](01-manifest-schema.md) | **RATIFIED** |
| 02 | [Generator Pipeline (`rlvgl-creator app from-yaml`)](02-generator-pipeline.md) | **RATIFIED** |
| 03 | [Round-Trip Targets](03-round-trip.md)                             | **RATIFIED** |
| 04 | [State-Machine Boundary (full Option A treatment)](04-state-machine-boundary.md) | **RATIFIED** |

## Conformance

A conforming rlvgl application MUST be expressible as:

1. an `app.yaml` manifest conforming to chapter 01, plus
2. asset source files referenced by relative path, plus
3. an optional state-chart file (`.scxml` / `.uml`), plus
4. an optional layout source export, plus
5. an optional i18n bundle conforming to `format: rlvgl_i18n_v1`.

Given those inputs, `rlvgl-creator app from-yaml` (implemented per
chapter 02; APP-02a–h shipped 2026-04-29) SHALL emit a buildable
Cargo crate equivalent to one of the existing examples in
`examples/`. The orchestrator supports `--validate-only`,
`--check`, `--force`, and `--jobs N` parallel dispatch.

## Initiative prefix

Execution PRs scoped to this initiative use the commit-subject
prefix `APP-NN[a-z]:` (e.g. `APP-01a:`, `APP-02b:`), matching the
`DISCO-`, `BBB-`, `CREATOR-`, `CHIPS-<VENDOR>-` convention in
`CLAUDE.md`. Frozen by [00 §5.4](00-concepts.md#54-initiative-prefix--standards-action).

## Relationship to other initiatives

- **`rlvgl-creator` + chipdb** (`docs/creator/`, `docs/bsp/`,
  `chipdb/rlvgl-chips-*`) — this initiative is a *consumer* of the
  BSP generator and asset pipeline. It does not duplicate or wrap
  them; it cites them by reference.
- **STM32H747I-DISCO bring-up** (`docs/disco-platform-guide/`,
  `docs/disco-tutorial/`, `docs/disco-freertos-guide/`,
  `docs/disco-zephyr-guide/`, `docs/disco-test-and-debug/`) — the
  H747 hand-written platform module is one of the round-trip
  targets ([00 §9](00-concepts.md#§9-frozen-decisions--round-trip-property))
  and the only currently-allowed `target.generator: hand_written`
  board ([01 §5.6](01-manifest-schema.md#56-targetgenerator-hand_written-allow-list)).
- **BeagleBone Black + NHD cape** (`docs/beaglebone-black/`) —
  contributes the prong vocabulary ([00 §5.1](00-concepts.md#51-prong-set--standards-action))
  and one round-trip target (Linux prong).
- **Audio meters** (`audio-meters-{core,widgets}/`) — cross-target
  precedent. v0 emits Rust only; TS host emission is a §11
  non-goal in chapter 00.
