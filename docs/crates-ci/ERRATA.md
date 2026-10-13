# ERRATA — CRATES-CI

**Family:** CRATES-CI
**Phase docs:** `docs/crates-ci/`
**Governed by:** the current parent SBC-00 concepts and Addendum B, as adopted
in [CLAUDE.md](../../CLAUDE.md) and [the compatibility map](../spec-index/SBC-COMPATIBILITY.md).

## Status legend

| Icon | Meaning |
| --- | --- |
| 🟢 | resolved — fix landed and verification evidence recorded |
| 🟡 | diagnosed — root cause known and fix prescription clear |
| 🔴 | open — undiagnosed or unresolved |
| ⚪ | deviation-pending-ratification — awaiting a §15 amendment |

## Questions and dispositions

| EOQ id | Errata | Ask |
| --- | --- | --- |
| `EOQ-002-ERRATA-002` | `ERRATA-002` | Resolved by owner 2026-10-06: frozen tagged source, same-ref Gate P and manual-only continuation. Provider/full Gate P verification pending. |
| `EOQ-001-ERRATA-001` | `ERRATA-001` | Mechanism accepted 2026-10-10 in CRATES-CI-00 §15 0.1.4: bounded real source graph admission, extracted-only verification/consumers. Implementation/full proof pending. |

## Index

| Id | Title | Status | Phase | First seen |
| --- | --- | --- | --- | --- |
| `ERRATA-002` | Continuation can publish workflow-HEAD sources without same-ref Gate P | 🟡 | CRATES-CI-00 §5 | 2026-10-05 |
| `ERRATA-001` | Core/bridge deferral breaks unpublished core resolution | 🟡 | CRATES-CI-01b | 2026-10-05 |

## ERRATA-001 — Core and bridge deferral breaks unpublished core resolution

**Status:** 🟡 diagnosed — bounded mechanism accepted; implementation/full proof pending
**First seen:** 2026-10-05
**Owning phase:** CRATES-CI-01b, [draft isolation correction](CRATES-CI-01B-STAGING-ISOLATION.md)

### Symptom

The v0.2.7 candidate staging diagnostic packages core and the Ratatui bridge,
then cannot resolve platform's unpublished core minimum. The captured failure
first used `rlvgl-core ^0.2.6`. The October 5 online diagnostic subsequently
packages API 0.2.1, core 0.2.7 and bridge 0.1.1, then reproduces the failure on
platform's `^0.2.7` requirement. Restoring the extracted core patch previously
produced duplicate live/extracted API source identities.
[Diagnostic receipt](../releases/v0.2.7-validation.md).

### Root cause

Before the October 10 correction, [CRATES-CI-00 §6](CRATES-CI-00-CONCEPTS.md)
and `scripts/crates_ci_stage.sh::stage_crate` deferred extracted core and bridge
patches while later packages resolved in the live root workspace. That
exception conflicted with progressive resolution of unpublished dependencies.

### Fix

Accepted: use the effective CRATES-CI-00 §6 bounded graph-admission boundary,
preserving original manifests and Cargo-owned normalization. Extracted-only
verification, predecessor routing and consumers remain required. The owning
amendment precedes implementation; acceptance does not establish Gate P.

### Verification

The draft phase requires missing/staged predecessor negative and positive
controls, archive/manifest equivalence, clean-source refusal, metadata/drift
guards, nested lock restoration, complete staging and all three packaged
consumers. The root candidate's existing dirty-source refusal and five publisher
matrix checks are rerun independently; they do not close this defect. A
synthetic detached-package fixture packages a predecessor, fails without its
patch (exit 101), and succeeds with its extracted patch (exit 0). It proves
only that control, not real-crate inventory equivalence or the full Gate P.

### Tracking

[October 10 execution receipt](../releases/v0.2.7-staging-20261010.md): the
bounded helper has ten passing guard tests and actual API/core positive,
missing-API, dirty-source and absent-lock controls. The mixed full-set diagnostic
produced 18 archives then stopped at Ratatui's unsupported real ancestor
descriptor/inheritance. That new owning decision frontier and the remaining
full/consumer/CI gates stay open; this entry is not defect closure.

On 2026-10-06 the owner accepted decision 2's direction: original manifests,
Cargo-owned normalization and extracted-only dependencies, with lock equivalence
resolved before final ratification. CRATES-CI-00 §15 0.1.3 and CRATES-CI-01b
§15 0.1.1 record that acceptance and remaining condition. Effective §6 and the
governing stager remain unchanged; the full mechanism is not yet ratified.
On 2026-10-10 the owner accepted the exact bounded source admission exception.
CRATES-CI-00 §15 0.1.4 now amends effective §6. The historical Draft dispositions
above are preserved as history; 01B remains a Draft execution/verification plan.
Implementation identity, complete staging, consumer and CI proof remain open.
On 2026-10-10 the owner also accepted the enumerated real root and nested
Ratatui descriptor/member contexts. CRATES-CI-00 §15 0.1.5 amends effective
§6 before implementation; 01B §15 0.1.4 remains drafted. The
[new diagnostic receipt](../releases/v0.2.7-staging-descriptors-20261010.md)
separates the current completed-MPY snapshot from the earlier mixed archives,
records actual Cargo inventories and nested Git provenance, and retains clean
source, full consumer and Linux CI acceptance as distinct gates. Unexpected
workspace or additional source requirements remain an owning-decision stop.
On resolution record the implementation commit and actual verification proof.

## ERRATA-002 — Continuation source is not frozen to its selected release

**Status:** 🟡 diagnosed — owner disposition ratified; local implementation prepared, provider/full Gate P verification pending
**First seen:** 2026-10-05
**Owning phase:** CRATES-CI-00 §5, INV-C1 and INV-C6

### Symptom and root cause

The pre-correction `publish-continue.yml` records the latest tag but publishes from the workflow
checkout without a Gate P job dependency. A newer workflow revision can change
package sources after qualification of the selected release. Its explicit
workflow-revision policy and §5.4's naming of only `publish.yml` require owning
disposition; this record does not silently extend the effective contract.

### Accepted fix and verification

Freeze a tagged commit for both Gate P and publication, retain all three existing
consumers, and distinguish the preserved helper revision from package sources.
The [review packet](../releases/v0.2.7-continuation-wave3.md) contains an external
YAML/patch and a faithful local resolver reproduction. Proposed source controls
reject the later fixture HEAD (1) and accept the tagged commit (0); static YAML
and dependency-graph checks pass. No provider execution or actual publication
has been performed. Complete Gate P remains independently blocked by ERRATA-001.

### Tracking

EOQ-002-ERRATA-002 was resolved by the owner on 2026-10-06. CRATES-CI-00 §15
0.1.2 ratifies the frozen-source/same-ref gate correction and manual-only
continuation. The local workflow implements it; the primary Gate P checkout also
explicitly pins `github.sha`. See the [decision receipt](../releases/v0.2.7-continuation-decision.md).
This entry remains diagnosed pending committed implementation identity and
provider/full packaged-consumer evidence. ERRATA-001 still blocks staging.

## Maintenance

Keep entries permanently. New deviations need their affected §15 record and
stable question handle. Resolve with the actual commit and verification
evidence; never delete the historical entry or silently change its authority.
