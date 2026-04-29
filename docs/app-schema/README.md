<!--
README.md - rlvgl Application Schema, initiative index.
Status: All five chapters RATIFIED 2026-04-27 / 2026-04-29.
-->

# rlvgl Application Schema

> **Status:** All five chapters RATIFIED (owner: Ira Abbott; 00 and
> 01 on 2026-04-27, 03 and 02 and 04 on 2026-04-29). `APP-NN[a-z]`
> execution PRs MAY cite any chapter as a frozen authority.
>
> **Implementation status as of 2026-04-29:** chapter 02 §7 sub-
> generators are ALL real except SM-gen-via-external-tool — APP-02e
> (BSP-gen), APP-02f (i18n), APP-02g (theme), APP-02h (parallel
> stage 3 dispatch), APP-04c (vendored-crate SM consumption +
> CV-1 cross-validate) all shipped. 53/53 creator app-schema
> integration tests pass across seven suites (validator, emit,
> bsp-gen, sm-vendored, i18n, theme, parallel).
>
> **v0.5 work still open:** APP-04b (first SM-bearing round-trip
> target — gated on external `mcp-statechart` tool reachability;
> the orchestrator's vendored-crate consumption path is in place
> and tested, but no committed round-trip target carries
> `state_machine:` yet); APP-05+ (Cargo `[features]` graph
> expansion — chapter 02 §8 preamble; v1 deferred); figma/uml
> layout formats (chapter 01 §5.5 v1 work). These don't reopen
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
