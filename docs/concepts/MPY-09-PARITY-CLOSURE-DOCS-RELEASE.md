<!--
MPY-09-PARITY-CLOSURE-DOCS-RELEASE.md - MPY evidence, documentation, and release closure contract.
-->

# MPY-09 — Parity Closure, Docs, and Release

**Status:** Draft 2026-08-09. Not ratified. Exact release version, performance
budgets, compatibility window, and artifact-retention policy remain open until
earlier phase evidence exists.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md). Dependency:
MPY-01 through MPY-08 must provide their ratified decisions and implementation
evidence before initiative closure.

## 0. Authority Policy

| Concern | Owner | MPY-09 relationship |
|---|---|---|
| MPY ownership model and initiative invariants | MPY-00 | Audited, not redefined. |
| LVGL pin, MPY-I row schema, and priority actor set | MPY-01 and inherited LPAR documents | Claim baseline. |
| Protocol/runtime/binding/transport semantics | MPY-02 through MPY-08 | Must close against their own acceptance evidence. |
| Package versions, compatibility policy, and release notes | Package manifests, repository `docs/CHANGELOG.md`, and ratified release decision | Updated only after evidence review. |
| rlvgl-local documentation object index | `docs/spec-index/` and local index tooling | MPY-09 keeps the subrepo index current. |
| Parent-repository submodule inclusion, conformance marking/tests, and dashboard | Parent repository's active work | Explicitly external to MPY-09; this phase supplies evidence but does not edit those surfaces. |
| Closure ledger, evidence bundle, documentation set, and retrospective | This document after ratification | MPY-09 is canonical. |

## 1. Purpose

Turn completed implementation into narrow, auditable claims: what Python can
discover and direct, which actor types and target profiles support it, which
LVGL baseline each claim compares against, and where deterministic evidence is
retained. Publish the API and operational guidance, make compatibility and
SemVer decisions, and close or explicitly defer every admitted MPY scope item.

## 2. Problem Statement

Feature demos alone do not establish introspection parity. A button created
from Python may work while its actions are undiscoverable, its layout snapshot
differs on the board, its callback overflows without notice, or its binding is
absent from a `no_std` profile. Conversely, “LVGL parity” is too broad to be a
useful release statement when the inherited baseline includes many types and
optional capabilities outside the first actor set.

Closure therefore operates per MPY-I row, actor/capability surface, and target
profile. Missing work cannot disappear behind an aggregate percentage, and an
unsupported or deferred row needs a named disposition and reopen condition.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **Claim Ledger** | Machine-readable MPY-01 matrix at closure, with one disposition and evidence set per row/profile. | Owned by MPY-01/09. |
| **Closure Disposition** | `Proven`, `Partial`, `Unsupported`, or `Deferred`, each with the metadata required below. | Owned by MPY-09. |
| **Release Profile** | Named feature/target combination whose capacities, actor catalog, protocol/binding versions, and evidence are published together. | Extends MPY-01/02 target profiles. |
| **Evidence Bundle** | Checksummed manifest connecting commits, builds, scenarios, traces, snapshots, frames, board records, capacity/footprint/performance data, and claims. | Owned by MPY-09. |
| **Compatibility Decision** | Reviewed disposition for each pre-MPY API: preserve, adapt, deprecate with window, or remove through an authorized breaking release. | Owned by MPY-09. |
| **Closure Record** | Owner declaration that all admitted rows and phase gates are proven or explicitly dispositioned and that remaining work has named reopen triggers. | Final MPY-09 artifact. |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Inherited LVGL version/commit and parity vocabulary | LPAR-01 plus MPY-01 |
| Per-row current/partial/missing inventory | MPY-01 Claim Ledger |
| Protocol and descriptor schema versions | MPY-02/03 generated/versioned artifacts |
| Same-core traces, snapshots, and frames | MPY-07 evidence |
| Board/transport record | MPY-08 evidence |
| Public Python names and exceptions | MPY-06 plus generated API reference |
| Crate/package versions | Relevant `Cargo.toml` files and API version source |
| Release history | `docs/CHANGELOG.md` and release notes |
| Local documentation objects | `docs/spec-index/index/_manifest.json` and generated family indexes |
| Final claims, deferrals, and retrospective | This document's implementation record and MPY retrospective |

## 5. Frozen Decisions — Claim Ledger

Every admitted MPY-I row is evaluated separately for each claimed Release
Profile and actor/capability surface. The row records:

- stable row ID and MPY-I level;
- inherited LVGL version and commit;
- rlvgl actor/type/capability and descriptor/schema revision;
- Release Profile and feature requirements;
- Closure Disposition;
- direct-runtime, actual-MicroPython, simulator, and board evidence qualifiers;
- known behavioral differences or capacity limits;
- evidence IDs/checksums and source commit; and
- owner, review date, and reopen condition where applicable.

The dispositions mean:

| Disposition | Closure meaning |
|---|---|
| **Proven** | The named surface meets the claimed MPY-I level on the named profile and all required evidence is green. |
| **Partial** | A precisely listed subset works; the unmet subset remains visible and cannot be advertised as the full row. |
| **Unsupported** | The profile intentionally does not offer the capability and discovery reports that absence consistently. |
| **Deferred** | Work is admitted but postponed with reason, owner/initiative destination, and objective reopen trigger. |

`Missing` is an inventory state, not a closure disposition. No advertised row
may remain Missing, and Partial/Unsupported/Deferred rows do not contribute to
a Proven claim. Aggregate counts may summarize the ledger but never replace
row-level evidence.

## 6. Frozen Decisions — Required Evidence Gates

MPY initiative closure requires all of the following:

### 6.1 Architecture and implementation

- MPY-01 through MPY-08 are ratified and their admitted implementation work is
  complete or has an owner-approved Closure Disposition.
- The five proof actors—Container, Label, Button, Slider, and List—close their
  admitted creation, descriptor, property/action/event, layout, lifecycle, and
  snapshot rows.
- Dependency-direction and `no_std + alloc` builds prove that MicroPython/VM
  types remain outside the language-neutral core.
- Protocol, descriptor, Python API, and transport versions are mutually
  compatible and discoverable.

### 6.2 Conformance

- MPY-02 golden protocol vectors and all phase invariant tests are green.
- Every required MPY-07 scenario passes through Direct Driver and actual
  MicroPython Driver with equivalent logical traces.
- Required simulator frame/geometry evidence is green.
- The MPY-08 board profile replays the applicable scenarios and passes input,
  callback, saturation, wraparound, stall, reset, mismatch, and recovery gates.
- Repeat runs produce identical deterministic artifacts or an approved Golden
  Update Record explains every change.

### 6.3 Resource characterization

Each Release Profile publishes measured:

- code and static-data footprint by affected image/crate;
- heap peak and allocation count for catalog open, proof-stage construction,
  snapshot, callback drain, and teardown;
- actor, subscription, command/return queue, text, frame, fragment, snapshot,
  and transaction capacities;
- create/set/get/invoke/subscribe and callback-drain latency distributions;
- full stage commit/layout/draw/present time; and
- 50-, 250-, and 1,000-actor lookup/snapshot behavior.

MPY-07/08 measurements establish numerical release budgets before final
ratification. A profile exceeding a budget either fails closure or records an
owner-approved narrower profile; MPY-09 does not retroactively loosen a budget
to make a run pass.

## 7. Frozen Decisions — Documentation and Examples

The release documentation set contains:

1. an architecture overview explaining Director, Stage, Actor, Direction, Cue,
   Safe Turn, requested/computed layout, and same-core/dual-core profiles;
2. a generated or descriptor-checked Python API reference for `Stage`, `Actor`,
   `Subscription`, `Transaction`, value conversion, and exceptions;
3. an actor catalog/reference with target availability, constructor fields,
   properties, actions, events, layout capabilities, defaults, and limits;
4. a protocol/version/capacity reference suitable for another adapter;
5. a tutorial that creates the proof stage, sets layout, registers callbacks,
   orchestrates a UI update, inspects geometry, and tears down cleanly;
6. callback and Safe Turn guidance, including callback-time restrictions,
   coalescing, overflow, and exception handling;
7. same-core simulator build/test/debug instructions;
8. a board guide covering paired images, boot/flash order, REPL, transport
   diagnostics, reset/recovery, and evidence capture; and
9. migration guidance for the placeholder `NodeSpec`/z-index stack C ABI and
   `mp_rlvgl` module surface.

Examples are tested inputs, not pasted illustrations. The tutorial script must
be one of the canonical scenarios or invoke the same checked library fixture so
documentation cannot drift from the binding.

## 8. Frozen Decisions — Compatibility, SemVer, and Publication

Before release, maintainers inventory the existing `rlvgl-api` `NodeSpec`,
`ZIndex`, `InputEvent`, `API_VERSION`, stack functions, C symbols, and
`mp_rlvgl` module names. Each receives a Compatibility Decision with known
in-tree/external consumers and one of these outcomes:

- preserve unchanged beside the MPY surface;
- adapt to the new protocol while preserving observable compatibility;
- deprecate with warnings, replacement mapping, and a published removal
  version/window; or
- remove only in a separately approved SemVer-breaking release.

No draft phase preauthorizes removal. Package versions in affected manifests,
the shared API/protocol version, `docs/CHANGELOG.md`, release notes, compatibility
matrix, and examples must agree. The branch name is development organization,
not evidence that a particular release number or compatibility decision has
already been ratified.

The Evidence Bundle manifest records source commit, dirty-state prohibition,
toolchain/MicroPython/LVGL pins, feature sets, hardware identity, checksums, and
result locations. Large artifacts may live in CI retention rather than Git,
but their immutable locator, checksum, retention period, and regeneration
command remain in the repository.

## 9. Frozen Decisions — Invariants and Evidence

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-09-1** | Every admitted Claim Ledger row MUST end in exactly one auditable Closure Disposition per claimed Release Profile, and no advertised row may remain Missing. | Ledger schema/lint and release-claim audit. |
| **INV-MPY-09-2** | A Proven MPY-I claim MUST name the inherited LVGL pin, actor/capability scope, target profile, source commit, and deterministic direct/MicroPython/simulator/board evidence required by that row. | Evidence-manifest referential-integrity check. |
| **INV-MPY-09-3** | The public Python/actor/protocol documentation and tutorial MUST derive from or be checked against the same descriptors, versions, capacities, exceptions, and canonical scenarios used by the implementation. | Generated-doc diff and executable documentation tests. |
| **INV-MPY-09-4** | Every affected compatibility surface and package/protocol version MUST receive an explicit SemVer review; legacy APIs MUST NOT disappear through an undocumented MPY implementation change. | Public API diff, consumer inventory, and release checklist. |
| **INV-MPY-09-5** | Every Release Profile MUST publish reproducible footprint, capacity, performance, no-std, simulator, and applicable board results against ratified budgets. | Clean-build evidence bundle and budget gate. |
| **INV-MPY-09-6** | Initiative closure MUST include explicit deferrals/reopen triggers, a retrospective, a clean rlvgl-local documentation index, and owner declaration without claiming completion of parent-repository dashboard work. | Closure-record audit and local index checks. |

## 10. Reconciliation Decisions

| Existing surface | MPY-09 decision |
|---|---|
| Broad “LVGL parity” wording | Replaced by row-, level-, actor-, pin-, and profile-scoped claims. |
| `rlvgl-api` fixed `NodeSpec`/z-index stack API | Retained until Compatibility Decision and SemVer review; migration target is the generic Stage/Actor protocol. |
| `rlvgl-micropython` placeholder status/`ValueError` surface | Documented as pre-MPY behavior and migrated under MPY-06; not silently presented as feature-complete. |
| `mp_rlvgl` module name | Compatibility alias decision remains explicit even if `rlvgl` is the primary MPY-06 import. |
| CPython/PyO3 host layer | Optional additional profile; cannot satisfy actual-MicroPython evidence. |
| Root/parent documentation dashboard | Receives evidence from this submodule through separate parent work; not edited or declared complete here. |
| Existing legacy documentation | Updated only where MPY release behavior makes it inaccurate; unrelated conformance modernization remains out of scope. |

## 11. Non-Goals and Open Decisions

1. **No claim of all-widget LVGL parity.** Closure covers only ledger rows and
   profiles carrying the required evidence.
2. **No implementation in this draft.** Budgets and release decisions depend on
   earlier phase measurements.
3. **No parent-repository integration.** Submodule pinning, parent conformance
   labels/tests, and dashboard updates remain owned by the separate parent task.
4. **No broad legacy-doc rewrite.** MPY must not introduce new local index
   issues, but existing unrelated findings are not this initiative's scope.

- **PCDN-MPY-09-001:** Which package and repository release number carries the
  first MPY profile, and which packages require coordinated version bumps?
- **PCDN-MPY-09-002:** What compatibility window and removal version apply to
  `NodeSpec`, stack functions/C symbols, `mp_rlvgl`, and other superseded
  surfaces?
- **PCDN-MPY-09-003:** Which numerical footprint/latency budgets are ratified
  from MPY-07/08 measurements for host and board Release Profiles?
- **PCDN-MPY-09-004:** What Git-versus-CI artifact threshold and retention
  period satisfy reproducibility?
- **PCDN-MPY-09-005:** Is STM32H747I-DISCO board success mandatory for the first
  public MPY release, or may a clearly named non-closing same-core preview ship
  before full initiative closure?

## 12. Acceptance Checklist

- [ ] `INV-MPY-09-1` row/profile dispositions and no-Missing release rule are accepted.
- [ ] `INV-MPY-09-2` evidence-qualified parity claims are accepted.
- [ ] `INV-MPY-09-3` descriptor/scenario-checked docs and examples are accepted.
- [ ] `INV-MPY-09-4` compatibility inventory and SemVer gate are accepted.
- [ ] `INV-MPY-09-5` resource/build/conformance publication is accepted with measured budgets.
- [ ] `INV-MPY-09-6` local-only index ownership and closure artifacts are accepted.
- [ ] PCDN-MPY-09-001 through PCDN-MPY-09-005 are resolved from completed phase evidence.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/MPY-01-INTROSPECTION-BASELINE.md`
- `docs/concepts/MPY-02-IDENTITY-VALUES-PROTOCOL.md`
- `docs/concepts/MPY-03-RUNTIME-REGISTRY-ACTOR-CREATION.md`
- `docs/concepts/MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md`
- `docs/concepts/MPY-05-CUES-SAFE-SCHEDULING.md`
- `docs/concepts/MPY-06-MICROPYTHON-DIRECTOR-BINDING.md`
- `docs/concepts/MPY-07-SAME-CORE-SIMULATOR-CONFORMANCE.md`
- `docs/concepts/MPY-08-CM7-CM4-TRANSPORT-BOARD-PROOF.md`
- `docs/concepts/LPAR-01-BASELINE.md`
- `docs/spec-index/README.md`
- `docs/spec-index/index/_manifest.json`
- `api/src/lib.rs`
- `micropython/src/lib.rs`
- `micropython/mp_module.c`
- `docs/CHANGELOG.md`

## 14. Unblocks

After every gate is satisfied and the owner signs the Closure Record, MPY-09
unblocks publication of the named MPY Release Profile and closes this
initiative. Any Partial, Unsupported, or Deferred work continues only through
its recorded owner and reopen trigger; closure does not silently convert it to
parity.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-09-1, INV-MPY-09-2, INV-MPY-09-3, INV-MPY-09-4, INV-MPY-09-5, INV-MPY-09-6, INV-MPY-9, PCDN-MPY-09-001, PCDN-MPY-09-002, PCDN-MPY-09-003, PCDN-MPY-09-004, PCDN-MPY-09-005, §0–§14

**Commits:** pending

**Summary:** Drafts the row/profile closure ledger, deterministic evidence
bundle, resource and release gates, documentation set, legacy compatibility and
SemVer review, local index responsibility, and owner closure record.

#### Rationale

The initiative can only claim introspection parity when its advertised rows
connect to exact baselines and repeatable behavior on the relevant paths. A
single closure ledger prevents demos, optional host shims, broad wording, or
untracked deferrals from overstating what MicroPython can direct in a released
profile.
