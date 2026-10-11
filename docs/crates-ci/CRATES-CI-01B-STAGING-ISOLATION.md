# CRATES CI 01b staging isolation correction

**Status:** Draft execution and verification plan. On 2026-10-11 the owner
accepted the one-target addition through effective CRATES-CI-00 §6 / §15 0.1.6.
Governing implementation is authorized by that amended ratified concepts doc.
This plan's full-set, consumer and CI acceptance gates remain open.

## 0 Authority policy

Cargo owns package inventory and manifest normalization. CRATES-CI-00 owns
release gates and extracted-package consumer isolation. The current SBC owns
amendment/ratification and errata rules. This Draft plan tracks implementation and verification of the effective
CRATES-CI-00 §6 boundary for [ERRATA-001](ERRATA.md).

## 1 Purpose and diagnosis

Eliminate the live-workspace/extracted-package identity collision without
temporarily hiding an unpublished predecessor. See
[the recorded proposal](../releases/v0.2.7-gate-p-proposal.md) and the defect's
original diagnostics. The core 0.2.7 selection does not remove the mechanism
defect.

## 3 Terms and source map

Use Consumer Project, Staged Registry, Gate P and Gate R as defined in
CRATES-CI-00. No term, version policy or invariant is redefined here.

| Surface | Owner |
| --- | --- |
| Cargo file inventory and normalized manifest | Cargo package output and Cargo Book |
| Publishable set/order | `scripts/publish_changed.sh` and its matrix tests |
| Effective staging implementation | `scripts/crates_ci_stage.sh` |
| Effective admission boundary | CRATES-CI-00 §6 / §15 0.1.6 |
| Library/creator/user-sim gate proof | Existing consumer harnesses |

## 5 Execution boundary

Use [CRATES-CI-00 §6](CRATES-CI-00-CONCEPTS.md#6-frozen-decision--staged-registry-mechanism-specification-required)
as the single normative definition. The execution plan must inventory real
package inputs, preserve manifests, route predecessors to extracted archives,
record the bounded dev graph admission separately, and keep it unavailable to
normalized verification and consumers. Complete generated-member/provenance
accounting is separate from payload comparisons. A fresh mismatch must stop
with its actual failure rather than broadening admission or bypassing Cargo.

The additional archive-preparation contexts are exactly the real root and nested
Ratatui descriptors, with 47 / 43 actual metadata members. Their finite manifest
paths, original hashes, actual targets and inherited shared-file context are
enumerated before execution in `scripts/crates_ci_workspace_contexts.json` and
the fresh snapshot receipt. Effective concepts §15 0.1.6 additionally admits
only `tests/bsp_clock_ownership.rs` in the root `rlvgl` target context, with its
exact concepts-recorded fingerprint. The actual 47/43 member sets and descriptor
bytes remain unchanged. Preserve nested ED Git provenance separately from
outer AA provenance. Any unexpected descriptor/member/target/additional source
requirement is an owning-decision stop. The full set and all three consumers
must use the immutable completed-MPY source snapshot; the earlier mixed 18
archives remain diagnostic history and cannot supply its identity.

## 10 Reconciliation and non-goals

This changes CRATES-CI-00 §6's documented core/bridge deferral, preserving
`INV-C1`, `INV-C2`, `INV-C6` and the existing failure classes. It does not relax
Gate P, enable dirty-source CI, select registry credentials, redefine package
versions, or count source-workspace checks as packaged-consumer proof.

## 12 Acceptance checklist

- [x] Owner accepts the exact §6 bounded mechanism (CRATES-CI-00 §15 0.1.4, 2026-10-10); phase execution gates below remain open.
- [ ] Inventory equals Cargo's normal package file set, including generated assets.
- [ ] Normalized manifests/dependencies equal Cargo's packaged manifests.
- [ ] An unpublished missing predecessor fails at the affected package.
- [ ] The same package succeeds with its extracted predecessor available.
- [ ] Normalized package verification and all final consumers have no source admission or live sibling route; graph admission follows effective §6.
- [ ] No live/extracted package identity collision remains with core 0.2.7 and bridge 0.1.1.
- [ ] Clean-source refusal, metadata failure and existing-version content drift remain enforced.
- [ ] Nested Cargo.lock restoration and checkout preservation pass on success and failure.
- [ ] Enumerated descriptor/member/target contexts preserve exact bytes, nested Git provenance and checkout-lock hashes/absence; unlisted context fails closed.
- [ ] Full publishable-set staging passes with normal asset preparation.
- [ ] Library, creator CLI and user-sim consumers pass against extracted packages.
- [ ] Linux CI proof and effective source identity are attached before release tagging.

## 14 Unblocks and stops

CRATES-CI-00 §15 0.1.6 supplies effective authority for bounded governing-stager
implementation. This Draft plan's open proof checklist remains required before
claiming full Gate P. A new unresolved PCDN or mechanism discrepancy stops its
dependent work; independent existing gates may continue. Missing external CI,
registry or consumer proof remains a recorded gate.

## 15 Change Log

### 0.1.0 — 2026-10-05 — drafted

**Author:** Codex
**Change kind:** semantic
**Touches:** CRATES-CI-00 §6, INV-C1, INV-C2, INV-C6
**Summary:** Proposes detached packaging against progressive extracted dependencies to replace the core/bridge deferral.

#### Rationale

The recorded deferral removes an unpublished dependency needed by later
packages. Restoring it in the live root exposes duplicate source identities.
Considered and rejected: lowering the declared dependency minimum, falling
back to live workspace patches, or treating a partial packaging run as Gate P.
Those hide the published-content failure instead of proving its correction.
Gate requirements, source preservation, metadata/drift checks and publication
authority deliberately remain unchanged.

### 0.1.1 — 2026-10-06 — drafted

**Author:** Codex, recording owner direction
**Change kind:** semantic
**Touches:** §5, §12, CRATES-CI-00 §6
**Summary:** Owner accepts original manifests, Cargo-owned normalization and extracted-only dependencies; resolve lock equivalence before final mechanism ratification.

#### Rationale

The owner accepted decision 2's recommendation, which explicitly preserves
these constraints and resolves the core lock case before final ratification.
This records acceptance of the direction without treating an unresolved input
mechanism as implemented authority. The
[follow-up receipt](../releases/v0.2.7-staging-decision.md) retains the bounded
lock experiment and its exact result. The whole-workspace lock currently forces
an absent path-only widgets dependency; a successful fresh-lock subset does not
prove retained-source lock equivalence. Acceptance boxes remain open and the
governed stager remains unchanged.

### 0.1.2 — 2026-10-07 — drafted

**Author:** Codex
**Change kind:** semantic
**Touches:** §5, §12, §14, CRATES-CI-00 §6
**Summary:** Proposes real dev-graph admission before Cargo normalization while retaining extracted-only package verification and consumer resolution.

#### Rationale

The retained-lock core trial now packages successfully with its unchanged
manifest after Cargo can load the actual path-only dev-dependency graph. Every
member of its 84-member archive, including original/normalized manifests, lock
and VCS metadata, equals the retained actual package. Extracted-core metadata
then resolves only extracted core/API plus registry packages. This distinguishes
source graph loading from the normalized package graph instead of deleting a
lock, fabricating a sibling manifest or weakening a declared version.

The proposed exception and its guards are in the
[mechanism packet](../releases/v0.2.7-staging-mechanism.md). The current draft's
blanket extracted-only sibling-alias clause would exclude this real dev-graph
input; owner ratification must explicitly dispose of that difference. The
proposal is not effective authority. Existing acceptance boxes, governing
stager, source/asset/drift guards, complete-set packaging and consumer gates
remain unchanged.

### 0.1.3 — 2026-10-10 — drafted

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §5, §12, §14
**Summary:** Aligns this Draft execution plan with the effective owner-accepted CRATES-CI-00 §6 admission amendment; full execution/consumer/CI acceptance remains open.

#### Rationale

CRATES-CI-00 §15 0.1.4 amends the already ratified owning concepts after the
owner's acceptance. It supplies governing implementation authority for the
bounded admission exception. This Draft plan records that authority without
misusing `ratified` (which closes the document's gates under SBC Addendum D §3)
or `amended` (which changes an already ratified document). Its acceptance items
remain a plan for fresh proof. Considered and rejected: calling the entire phase
ratified on historical one-package diagnostics, or marking unresolved release
gates complete. No invariant, release set, consumer, clean-source rule, asset
preparation or registry requirement changes. Stop on a new discrepancy or PCDN;
owner acceptance does not authorize recursively broadening the boundary.

### 0.1.4 — 2026-10-10 — drafted

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §5, §12, §14
**Summary:** Plans the effective concepts 0.1.5 real descriptor/member preparation exception and fresh current-MPY diagnostics; execution, consumers and clean Linux CI gates remain open.

#### Rationale

The owner accepted the bounded additional descriptor disposition after the
Ratatui stop. CRATES-CI-00 §15 0.1.5 amends the ratified owning concepts before
code. This plan enumerates both real workspace contexts and requires fresh
positive, refusal, nested-provenance and checkout-lock controls on an immutable
completed-MPY source snapshot. Prior mixed archives cannot supply that identity.
Considered and rejected: ratifying this entire execution phase on the accepted
design, reusing mixed artifacts, or quietly loading another workspace/context.
The plan remains Draft; unchanged full staging and three-consumer proof is
distinct from source tests, local dirty diagnostics and clean Linux CI Gate P.

### 0.1.5 — 2026-10-11 — drafted

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §5, §14
**Summary:** Aligns this Draft plan with the effective concepts 0.1.6 one-target admission amendment; full execution and release proof remain open.

#### Rationale

The owner accepted exactly the existing BSP clock-ownership regression target
for root archive preparation. CRATES-CI-00 §15 0.1.6 records the amendment
before governing implementation. This plan retains the unchanged member sets,
real descriptor bytes, target fingerprints and all existing acceptance gates.
Considered and rejected: inferring general target admission, hiding the test,
or ratifying the execution plan from that acceptance. All other unexpected
contexts remain an owning-decision stop; Linux, clean-ref, full-set staging,
three extracted consumers and publication proof remain separate.
