<!--
04-state-machine-boundary.md - rlvgl Application Schema, Chapter 4: State-Machine Boundary.
Status: DRAFT — full Option A treatment.
-->

**[← Prev](03-round-trip.md) · [Index](README.md) · Next → (TBD)**

# Chapter 4 — State-Machine Boundary

> **Status:** DRAFT 2026-04-29 (see §15). Depends on
> [Chapter 0](00-concepts.md), [Chapter 1](01-manifest-schema.md),
> [Chapter 2](02-generator-pipeline.md), [Chapter 3](03-round-trip.md).
> This chapter consolidates the Option A decision from
> [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)
> and expands the SM-gen sub-generator contract from
> [02 §7.4](02-generator-pipeline.md#74-sm-gen-contract-external-mcp)
> with the verification-vector consumption story, the
> screen↔state cross-validation rule, and the v1 promotion
> criteria for Option B.

## §0 Authority policy

This chapter is normative for:

- the screen↔state cross-validation rule (§6),
- the verification-vector consumption surface (§7),
- the vendored-crate offline contract (§5.3),
- the Option-B promotion criteria (§10).

It is *informative* for:

- the SM-gen CLI surface itself (owned by
  [02 §7.4](02-generator-pipeline.md#74-sm-gen-contract-external-mcp)),
- the manifest grammar for `state_machine:`
  (owned by [01 §5.3](01-manifest-schema.md#53-state_machine-optional)),
- the Option-A vs. Option-B framing (owned by
  [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question);
  the recap in §5.1 here cites that section without modification).

RFC 2119 keywords carry their RFC meanings.

## §1 Purpose

Define how an rlvgl application's state machine — the SCXML or UML
model that drives screen transitions and event routing — is
authored, generated, and consumed by an orchestrator-emitted crate.

The chapter answers four questions:

1. **Where does the SM source of truth live**, given Option A's
   "external file by reference" decision (§5.1)?
2. **How does the orchestrator validate the SM against the
   manifest** before the rest of the pipeline runs (§6)?
3. **How do verification vectors flow from the SCXML through to
   `cargo test`** (§7)?
4. **Under what conditions would v1 promote to Option B** (inline
   SM definition in the manifest)?

## §2 Problem statement

The state-chart MCP tool (the existing, external SCXML/UML →
Rust generator referenced throughout this initiative) is owned
outside the rlvgl repo. Its release cycle, schema, and CLI
surface are not under the rlvgl-creator team's control. Two
failure modes follow if the schema couples tightly:

- **Manifest version pinned to SM-gen version.** Bumping the SM
  generator's output format would force an
  `rlvgl-app/v0` → `rlvgl-app/v1` cycle even when no manifest
  semantics changed.
- **Orchestrator network dependency.** If the SM-gen reaches
  network during `cargo build` (or even during
  `rlvgl-creator app from-yaml`), CI loses offline reproducibility
  — counter to [02 §11](02-generator-pipeline.md#§11-non-goals-this-chapter).

Option A — *reference-by-path* — addresses both: the manifest
points at an external `.scxml`/`.uml` plus a generator name; the
SM-gen runs in its own cycle; the orchestrator consumes the
already-generated Rust crate offline.

## §3 Glossary additions

Only terms not defined in
[00 §3](00-concepts.md#§3-canonical-glossary) /
[01 §3](01-manifest-schema.md#§3-glossary-additions) /
[02 §3](02-generator-pipeline.md#§3-glossary-additions):

- **SM source** — *Owned by this chapter.* The `.scxml` or `.uml`
  file the manifest's
  [01 §5.3](01-manifest-schema.md#53-state_machine-optional)
  `state_machine.source` resolves to. Authoring lives outside
  rlvgl; the rlvgl repo treats it as opaque text.
- **SM crate** — *Owned by this chapter.* The Rust output of the
  SM-gen run: a single Cargo crate (or sibling module tree)
  carrying `states.rs` (the enum + transition table), an optional
  `vectors.rs` (the verification-vector test family), and an
  optional `mod.rs` index per
  [02 §5.4](02-generator-pipeline.md#54-emitted-crate-layout).
- **State id** — *Owned by this chapter.* The string identifier
  attached to a state both in the SCXML (`state.id` attribute)
  and in the manifest (`screens[].state`). Cross-validation
  (§6) asserts these agree.
- **Verification vector** — *Used as defined in
  [00 §8](00-concepts.md#§8-frozen-decisions--verification-vectors);
  this chapter binds it to a `#[test]` family in the SM crate.*
  A serialised input/expected-output pair the SCXML author
  sketches alongside the model; SM-gen materialises it as a
  Rust test that exercises the generated transition function.
- **Vendored SM crate** — *Owned by this chapter.* The pattern
  where the SM-gen output is committed to the rlvgl repo (or to
  the round-trip target's tree) and treated by the orchestrator
  as a path-dependency, not a build-time generator output. Same
  shape as the chipdb-rendered BSP committed under
  `examples/beetle-esp32c3/src/bsp_generated/` per
  [03 §5.5](03-round-trip.md#55-examplesbeetle-esp32c3--bsp_pac-binary-bare-metal).

## §4 Source-of-truth additions

| Concept                                          | Owner (canonical)                       | Mirrored / consumed by                                                                  |
| ------------------------------------------------ | --------------------------------------- | --------------------------------------------------------------------------------------- |
| `state_machine.source` path resolution           | [01 §5.3](01-manifest-schema.md#53-state_machine-optional) | this chapter §6 cross-validate                                                          |
| SM-gen CLI surface                               | [02 §7.4](02-generator-pipeline.md#74-sm-gen-contract-external-mcp) | this chapter §5.3 vendored-crate model                                                  |
| State-id set                                     | the SM source                           | manifest `screens[].state` references; orchestrator §6 step 4                           |
| Verification-vector consumption surface          | this chapter §7                         | the emitted crate's `cargo test` family                                                 |
| Option-B promotion gate                          | this chapter §10                        | a future amendment to [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question) |

## §5 Frozen decisions — Option A treatment

### 5.1 Option A — confirmed (recap)

[00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)
ratified Option A on 2026-04-27: the manifest cites a path to
the SM source plus a generator name; the SM-gen runs in its own
cycle; the manifest does NOT define states inline.

This chapter does NOT re-open the decision. §10 below names the
v1 promotion criteria for Option B; until those are met,
inline-state-machine grammar is a §11 non-goal.

### 5.2 SM-gen tool — `mcp-statechart`

[01 §5.3](01-manifest-schema.md#53-state_machine-optional) freezes
`state_machine.generator` as the string `"mcp-statechart"` in v0.
This chapter binds that string to the external state-chart MCP
tool (the istate-family codegen). Adding generator values is a
**Standards Action** registration per
[00 §5](00-concepts.md#§5-frozen-decisions--enums--registration-policy):
amend [01 §5.3](01-manifest-schema.md#53-state_machine-optional)
and add a §15 entry in chapter 01 *first*; behaviour PR follows
in a separate change.

### 5.3 Vendored-crate offline model — frozen

The orchestrator MUST NOT invoke the SM-gen as part of
`rlvgl-creator app from-yaml`. Instead, the SM crate is
**vendored**: pre-generated by a separate `mcp-statechart` run,
committed under the round-trip target's `src/state_machine/`
(or sibling crate path), and consumed by the orchestrator
exactly the way it consumes a chipdb-rendered BSP in
[02 §5.4](02-generator-pipeline.md#54-emitted-crate-layout).

Rationale:

- **Offline reproducibility.** The orchestrator never reaches
  network; CI runs without external service availability.
- **Schema decoupling.** SM-gen schema bumps don't force
  `rlvgl-app/v0` revs; only the vendored output's shape matters.
- **Reviewability.** The generated state enum + transitions are
  visible in PR diffs, same as the BSP.
- **Determinism.** [02 §9.1](02-generator-pipeline.md#91-determinism)
  applies to the orchestrator. The SM-gen has its own
  determinism guarantee, but a network call in the middle of
  `app from-yaml` would defeat both.

A future v1 escape hatch — having the orchestrator regenerate the
SM crate when stale — is named in §10 as a promotion criterion,
not a current capability.

### 5.4 Vendored SM-crate layout — frozen

When `state_machine:` is present, the SM crate MUST conform to:

```
<sm-out>/
├── Cargo.toml                  # (only if SM is a sibling crate;
│                               #  inline-module form omits it)
├── .mcp-statechart-manifest.json # SM-gen self-manifest per [02 §7.1]
└── src/
    ├── lib.rs                  # crate root (sibling-crate form)
    │   OR
    ├── mod.rs                  # `pub mod states; pub mod vectors;`
    │                           #   (inline-module form,
    │                           #    [02 §5.4] `src/state_machine/`)
    ├── states.rs               # state enum + transition fn
    └── vectors.rs              # `#[cfg(test)]` test family
                                #   (only if verification_vectors: true)
```

Two acceptable wrapper shapes:

- **Sibling crate.** SM lives at a sibling path
  (e.g. `examples/disco-demo-states/`); the manifest's
  `state_machine.source` is the SCXML/UML; the round-trip target
  declares the SM crate as a path dependency in its `Cargo.toml`.
  This matches the controller-crate pattern in
  [02 §7.8](02-generator-pipeline.md#78-controller-wiring-contract).
- **Inline module.** SM lives inside the round-trip target's
  `src/state_machine/`; the orchestrator emits a child-module
  `mod.rs` index per [02 §5.4](02-generator-pipeline.md#54-emitted-crate-layout).
  This matches the BSP pattern in [02 §7.2](02-generator-pipeline.md#72-bsp-gen-contract).

The choice is a per-target authoring decision; the manifest does
NOT carry a discriminator at v0. The orchestrator detects which
form is in use by looking at where the vendored output lives.

### 5.5 SM-gen self-manifest — required

Unlike the [02 §7.2.1](02-generator-pipeline.md#721-in-process-invocation-and-71-self-manifest-waiver-v0)
BSP-gen waiver, the SM-gen MUST emit the
`<sm-out>/.mcp-statechart-manifest.json` self-manifest specified
in [02 §7.1](02-generator-pipeline.md#71-common-contract-all-sub-generators)
plus a top-level `state_set: [...]` field listing the emitted
state ids:

```json
{
  "tool":      "mcp-statechart",
  "version":   "<semver>",
  "source":    "states/main.scxml",
  "files": [
    { "path": "src/states.rs",  "hash": "blake3:..." },
    { "path": "src/vectors.rs", "hash": "blake3:..." }
  ],
  "state_set": ["idle", "menu", "settings", "playing"]
}
```

The orchestrator reads `state_set` for the §6 cross-validate
step. Unlike BSP-gen (whose output file list is enumerable from
[02 §7.2](02-generator-pipeline.md#72-bsp-gen-contract)), the SM
crate's state-id set is *not* derivable from anywhere except the
SCXML — making the self-manifest load-bearing rather than
synthesizable. The §7.1 waiver does not apply to SM-gen.

## §6 Cross-validate (normative)

[02 §6](02-generator-pipeline.md#§6-pipeline-flow-normative-ordering)
step 4 is "post-SM cross-validate." This chapter spells out the
rule:

> **CV-1.** If `state_machine:` is present, the orchestrator MUST
> read the SM-gen self-manifest's `state_set` array. For every
> `screens[].state` in the manifest, the value MUST appear in
> `state_set`. The first mismatch fails the run with the screen
> id, the unknown state name, and the resolved `state_set`.

> **CV-2.** If `state_machine:` is absent, every `screens[].state`
> field MUST be absent. A non-empty `screens[].state` without an
> SM is rejected by [01 §6 rule 6](01-manifest-schema.md#§6-validation-rule-set-normative).

> **CV-3.** The orchestrator MUST NOT consult the SCXML / UML
> directly. Authority for the state-id set is the self-manifest's
> `state_set` field. (Re-parsing SCXML in two places creates
> drift; one parser is enough.)

CV-1 fires *after* the parallel sub-generator stage in
[02 §6](02-generator-pipeline.md#§6-pipeline-flow-normative-ordering)
step 3 finishes — that is, the orchestrator already has the
self-manifest. If SM-gen has not been run (no vendored output
present), CV-1 fails with the missing-output error before
reaching the screen-state check.

The screen↔state cross-validate is the single normative rule
this chapter adds to the orchestrator pipeline.

## §7 Verification-vector consumption (normative)

[00 §8](00-concepts.md#§8-frozen-decisions--verification-vectors)
left "how the rlvgl test harness consumes vectors" unfrozen. This
chapter freezes it.

### 7.1 Vector format — owned by SM-gen

The vector serialisation format is owned by the
`mcp-statechart` tool. The rlvgl repo treats `vectors.rs` as
opaque generated text. The orchestrator MAY validate that
`vectors.rs` parses as a `#[cfg(test)] mod vectors { ... }` block
but does NOT inspect individual vector contents.

### 7.2 Test family shape — frozen

Each verification vector MUST emit a single `#[test] fn` whose
name follows the pattern:

```text
vector_<scxml_id_snake_case>
```

The function body steps the generated transition function
through the vector's input sequence and asserts the resulting
state and outputs. Vectors that share a state-coverage class MAY
group under a `#[test]` umbrella using the same pattern, with
`<scxml_id>` an authoring-tool-assigned cluster id.

The naming pattern is normative so a CI grep can attribute test
failures to specific SCXML state ids without parsing rustc
output.

### 7.3 Test discovery — vendored, not generated at build time

Per the §5.3 vendored-crate model, `vectors.rs` is committed.
`cargo test` discovers the test family via the standard `[lib]`
or `[[test]]` mechanism in the SM crate's `Cargo.toml`
(sibling-crate form) or via the round-trip target's normal
`cargo test --lib` invocation (inline-module form, with
`#[cfg(test)] mod vectors;` declared in the parent module).

The pre-publish phase 2 invocation
(`cargo test --workspace`, see
[`CLAUDE.md`](../../CLAUDE.md#pre-publish-validation))
runs the vector test family unmodified — no new make target
or test runner is introduced.

### 7.4 Opt-out — `verification_vectors: false`

[01 §5.3](01-manifest-schema.md#53-state_machine-optional)
allows `verification_vectors: false`. When set, SM-gen MUST NOT
emit `vectors.rs`; the orchestrator MUST NOT require it in the
self-manifest. This is the documented escape hatch for SCXML
sources that have not yet had vectors authored.

The orchestrator MUST NOT silently downgrade — i.e., emitting
`vectors.rs` when the manifest says `verification_vectors:
false` is a self-manifest-mismatch error, not a no-op.

## §8 Round-trip with `state_machine:` present

[Chapter 3 §6.13](03-round-trip.md#613--open--bsp_pac-stretches-the-screen-abstraction)
flagged the headless-schema-fit issue for the bsp_pac binary.
The same shape applies to round-trip when SM is in play:

> **RT-SM-1.** A round-trip target with `state_machine:` present
> MUST commit the vendored SM crate (sibling or inline) to the
> repo. The orchestrator's `--check` mode (chapter 02 §5.2) then
> verifies the SM crate has not drifted from the SCXML — but
> ONLY if the SM-gen has been re-run on the SCXML in the same CI
> step that runs `--check`.

> **RT-SM-2.** v0 does NOT enforce SM-gen re-execution in CI.
> SCXML edits without a follow-up SM-gen run produce a manifest
> that round-trips successfully (orchestrator reads the stale
> vendored crate's stale self-manifest) but encodes drifted
> state-ids relative to the SCXML's current content. This is
> the documented v0 limitation that motivates §10 promotion
> criterion (a).

> **RT-SM-3.** None of the [03 §5](03-round-trip.md#§5-round-trip-targets)
> v0 round-trip targets carry `state_machine:` — by design.
> Adding a SM-bearing target is APP-04+ work and gates on this
> chapter's ratification.

## §9 Reconciliation with adjacent primitives

### 9.1 Hand-written platform crates with internal state machines

Some hand-written platform crates (e.g.
`rlvgl-app-disco-demo` per [02 §7.8](02-generator-pipeline.md#78-controller-wiring-contract))
carry their own internal state via simple Rust enums and
`match`-driven transitions. These are NOT SM crates in this
chapter's sense — they are **controller logic**, not a generated
state machine, and the manifest's `state_machine:` block does
NOT cover them.

The distinction:

- **SM crate** — generated from external SCXML/UML; vendored;
  state ids are SCXML-authored; verification vectors are
  SCXML-authored.
- **Controller logic** — hand-written; lives inside the
  controller crate; not generated; MUST NOT appear in the
  manifest's `state_machine:` block.

If a hand-written controller's internal state ever needs to be
expressed in the manifest, it is a candidate for an SCXML port —
not a candidate for an inline `states:` field at v0.

### 9.2 The `playit` test harness

[`playit/`](../../playit/) provides a serial command protocol for
runtime state interrogation (`?`, `QB:<tag>`, `QE:<tag>`, etc.).
Verification vectors per §7 run inside `cargo test`; `playit`
runs against a live target. The two surfaces are complementary:

- **Vectors** verify the generated SM matches the SCXML's
  authored expectations on the host.
- **Playit** verifies the live target's externally-observable
  behaviour matches the generated SM under hardware constraints.

This chapter does NOT define a playit-driven vector replay path.
v1 may add one as a Specification Required addition to playit's
own protocol.

### 9.3 Existing creator subcommands

`rlvgl-creator` carries no SM-gen subcommand at v0 — the
`mcp-statechart` tool is external. A `rlvgl-creator app sm-regen`
convenience wrapper is a v1 candidate (§10 (a)). At v0 the
authoring workflow is:

```bash
# author / edit
$EDITOR examples/<target>/states/main.scxml

# regenerate vendored SM crate (out of band, external tool)
mcp-statechart --in states/main.scxml --out src/state_machine/ --vectors

# orchestrator picks up the vendored output
rlvgl-creator app from-yaml --out . examples/<target>/app.yaml
```

The manual middle step is the documented v0 cost of decoupling
SM-gen schema from the rlvgl repo.

## §10 Promotion criteria — Option B (v1)

[00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)
left Option B (inline state-machine definition in the manifest)
as a v1 candidate, conditional on Option A proving painful. This
chapter freezes the promotion gate.

Promote to Option B when **at least two** of the following are
demonstrated in landed work:

- **(a)** A `rlvgl-creator app sm-regen` (or equivalent) wrapper
  emerges that effectively re-executes the SM-gen step from
  inside the orchestrator. If the orchestrator already runs the
  SM-gen, the network/offline argument for Option A weakens.

- **(b)** Three or more round-trip targets carry
  `state_machine:` AND the same SCXML is duplicated across
  trees (i.e., the file-by-reference contract becomes a
  copy-paste contract in practice). This is the
  [03 §6.7](03-round-trip.md#67--closed--sibling-manifests-duplicate-by-copy-at-v0)
  pattern recurring; it argues for moving the source of truth
  *into* the manifest.

- **(c)** The verification-vector authoring tooling stabilises
  to the point where vectors can be expressed compactly enough
  to live in the manifest itself without overwhelming it
  (informally: median vector length under ~20 lines, with no
  more than ~20 vectors per state machine).

- **(d)** A reviewer-survey rate of "I had to read four files
  to understand this state's intent" rises above ~40% on
  SM-touching PRs across two consecutive months. This is the
  qualitative-cost signal — the file count Option A imposes
  becomes the dominant complaint.

When any two are met, a §15 amendment to chapter 00 §10.2
re-opens the boundary decision; behaviour PRs do NOT ride on an
unamended invariant.

## §11 Non-goals (this chapter)

- **Inline state-machine grammar at v0.** Promotion criteria in
  §10; until met, `state_machine:` is path-only.

- **A built-in SM-gen subcommand.** v0 keeps `mcp-statechart`
  external. v1 candidate per §10 (a).

- **Playit-driven vector replay.** §9.2 names this as out of
  scope here; future Specification Required for playit.

- **State-coverage gating in CI.** v0 does not assert vector
  coverage of the SCXML's transition graph. SM-gen MAY emit a
  coverage report; this chapter does not require the
  orchestrator to consume it.

- **Cross-target state-id agreement.** Two round-trip targets
  CAN ship different SCXMLs with overlapping state ids. The
  manifest scopes state-id to its own SM crate; the orchestrator
  does NOT diff state sets across manifests.

## §12 Acceptance checklist

This chapter is ratified (§15 entry dated) when:

- [ ] §5.3 vendored-crate offline model is consistent with the
      committed orchestrator behaviour (no SM-gen subprocess
      invocation in `app from-yaml`).
- [ ] §5.5 self-manifest format is documented in a citable form
      (this chapter, plus an example committed under
      `chipdb/` or a future `docs/state-machines/` directory).
- [ ] §6 cross-validate rule (CV-1, CV-2, CV-3) implemented in
      `src/bin/creator/app.rs` Orchestrator step 4 OR explicitly
      noted as gated on a SM-bearing round-trip target landing
      first (in which case §15 records the deferral).
- [ ] §7 vector test-family naming pattern documented in the
      external `mcp-statechart` README (cross-repo cite) OR a
      shim in this chapter accepts the existing tool's actual
      naming and §7.2 amends to that convention.
- [ ] §10 promotion criteria reviewed by the initiative owner
      (Ira Abbott) — non-binding feedback OK; the criteria are
      meant to be argued.
- [ ] §15 has a dated ratification entry signed off by the
      initiative owner.

The implementation work for items 1–4 lives in PR sequence
**APP-04a+**: `APP-04a` — chapter 04 ratified; `APP-04b` — first
SM-bearing round-trip target lands (likely the FreeRTOS H747
demo, since `rlvgl-app-disco-demo` already has internal-state
shape); `APP-04c` — orchestrator CV-1 cross-validate
implemented; `APP-04d` — first SCXML + vendored SM crate
committed to the repo.

## §13 Files cited

- [`docs/app-schema/00-concepts.md`](00-concepts.md) — chapter 0,
  §10.2 Option A authority.
- [`docs/app-schema/01-manifest-schema.md`](01-manifest-schema.md)
  — chapter 1, §5.3 `state_machine:` grammar.
- [`docs/app-schema/02-generator-pipeline.md`](02-generator-pipeline.md)
  — chapter 2, §6 stage graph (step 4 cross-validate),
  §7.1 sub-generator contract, §7.4 SM-gen contract,
  §7.2.1 BSP-gen self-manifest waiver (cited as a non-precedent
  in §5.5 here).
- [`docs/app-schema/03-round-trip.md`](03-round-trip.md) —
  chapter 3, §6.7 (sibling manifest duplication, cited in §10
  (b) here), §6.13 (headless schema fit).
- [`CLAUDE.md`](../../CLAUDE.md) — pre-publish phase 2
  (`cargo test --workspace`), cited in §7.3.
- [`playit/`](../../playit/) — runtime test harness, cited in
  §9.2.

## §14 Unblocks

Ratifying this chapter unblocks:

- `APP-04a+` execution PRs that introduce the first SM-bearing
  round-trip target. Concrete next: lift
  `examples/apps/disco-demo/`'s internal controller state to an
  SCXML model and vendor the resulting SM crate.
- A future `rlvgl-creator app sm-regen` convenience subcommand
  (Standards Action via §10 promotion criteria), if §10 (a)
  triggers.
- An amendment to [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question)
  re-opening Option A vs. Option B if §10 promotion criteria
  fire.

## §15 Change log

| Date       | Status | Note                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 2026-04-29 | DRAFT  | Initial Option A treatment. Recaps [00 §10.2](00-concepts.md#102-state-machine-boundary-the-biggest-open-question) and [02 §7.4](02-generator-pipeline.md#74-sm-gen-contract-external-mcp); adds §5.3 vendored-crate offline model, §5.5 self-manifest is required (no §7.2.1 waiver for SM-gen), §6 CV-1/CV-2/CV-3 screen↔state cross-validate rules, §7 verification-vector test-family shape (`vector_<scxml_id_snake_case>` naming pattern), §10 four-criterion promotion gate for Option B (any two trigger a chapter 00 amendment), §11 non-goals. §12 acceptance items 1–4 are implementation gates; the chapter ratifies on its own substance (items 5–6) and the implementation rides on `APP-04a+`. Chapter remains DRAFT pending owner review of §10 criteria and §7 naming pattern. |
