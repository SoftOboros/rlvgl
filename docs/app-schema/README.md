<!--
README.md - rlvgl Application Schema, initiative index.
Status: DRAFT — chapters not yet ratified.
-->

# rlvgl Application Schema

> **Status:** Chapters 00, 01, and 03 RATIFIED (owner: Ira
> Abbott; 00 and 01 ratified 2026-04-27, 03 ratified 2026-04-29
> after the APP-02a validator landing closed §12). Chapter 02
> remains DRAFT — emission-gated, awaiting APP-02b/c/d.
> `APP-NN[a-z]` execution PRs MAY cite ratified chapters as
> frozen authorities; cite DRAFT chapters only as
> references-in-progress. See [`CLAUDE.md`](../../CLAUDE.md)
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
| 02 | [Generator Pipeline (`rlvgl-creator app from-yaml`)](02-generator-pipeline.md) | DRAFT (emission-gated) |
| 03 | [Round-Trip Targets](03-round-trip.md)                             | **RATIFIED** |
| 04 | State-Machine Boundary (full Option A / B treatment)               | TBD     |

## Conformance

A conforming rlvgl application MUST be expressible as:

1. an `app.yaml` manifest conforming to chapter 01, plus
2. asset source files referenced by relative path, plus
3. an optional state-chart file (`.scxml` / `.uml`), plus
4. an optional layout source export, plus
5. an optional i18n bundle conforming to `format: rlvgl_i18n_v1`.

Given those inputs, `rlvgl-creator app from-yaml` (TBD, chapter 02)
SHALL emit a buildable Cargo crate equivalent to one of the
existing examples in `examples/`.

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
