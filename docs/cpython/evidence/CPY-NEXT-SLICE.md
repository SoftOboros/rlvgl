# CPY next-phase and decision preparation

Prepared 2026-10-05. Informative continuation of the
[readiness receipt](CPY-V027-READINESS.md) and
[BBB capture plan](CPY-BBB-CAPTURE-PLAN.md). The coordinator's wave-two baseline
identifies the source plus uncommitted core-0.2.7 overlay. No new capture,
hardware access, PyO3/runtime/frame behavior, manifest, generated evidence,
phase status or PCDN disposition is introduced.

## Immediate executable frontier

[CPY-00](../CPY-00-CONCEPTS.md),
[CPY-01](../CPY-01-BASELINE-TARGET-PROFILES.md) and
[CPY-02](../CPY-02-UNIFY-PARTITION-CRATES.md) remain ratified; CPY-03 through
CPY-09 remain Draft. The read-only wave-two prerequisite audit checked all
seven Draft headers and shared-source/handoff facts: 21 checks passed, zero
failed. Its receipt is external `wave2/cpy-logs/next-slice-prerequisite-audit.json`.
These checks are not new implementation conformance or target qualification.

The nearest CPY gate is
[CPY-03 §§12/14](../CPY-03-NATIVE-RUNTIME-SERVICE.md#12-acceptance-checklist):
the same committed representative native workload must qualify at least one
explicit development tuple on the physical CPY-01 BBB, followed by owner phase
ratification. All five CPY-03 policy PCDNs are already resolved. Do not reopen
them or treat that fact as completed board evidence.

The capture plan supplies the exact existing command and 60-run/12-summary
matrix. Two source choices must remain distinct:

- Pair BBB against the historical August 23 host source `2bb9a911` and its
  retained exact resolver in a pristine checkout. This tests that historical
  native service; it does not qualify the current release overlay.
- Select a new clean checkpoint for the final candidate, retain its resolver
  and run a new host matrix before pairing a new BBB matrix. No new measurement
  is allowed from the present dirty candidate.

Neither choice changes shared code or selects capacities. Source checkpoint
selection and access to a known BBB/rootfs are prerequisites; this preparation
does not commit source, install a rootfs or access the board.

## Phase review packets

Each packet is preparation under the existing owning document, not authority to
implement a Draft phase. Keep phase ratification and physical/publication
acceptance separate.

| Phase | Concrete review packet | Exact stop |
| --- | --- | --- |
| [CPY-03](../CPY-03-NATIVE-RUNTIME-SERVICE.md) | Clean source/lock and host/BBB identity pair; full matrix accounting; explicit qualifying development tuple; cadence/RSS/latency/stall observations; existing lifecycle/readiness/epoch/close proof; owner ratification entry. | No physical BBB result exists in retained CPY inventory. Python export/display is not required for this native-only gate. |
| [CPY-04](../CPY-04-CPYTHON-DIRECTOR-BINDING.md) | A method-to-neutral-operation table with owning MPY phase/implemented path; generated/generic parity cases; non-Send native ownership, Python-detach, callback-token/exception and finalization test designs; copied runtime/backend config ownership. | All seven policy PCDNs resolved, but phase remains Draft. CPY-03, consumed MPY authority and completed binding proof are prerequisites. No leaf PyO3 implementation is admitted here. |
| [CPY-05](../CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md) | Slot-mode measurement matrix and exact layout vector inventory; retain/export/release and close/finalization test design; overflow/loss labels; independent native-cadence observations. | `PCDN-CPY-05-002` plus CPY-02/03 and lifetime/layout proof. Private probe buffers are not exported Frame Leases. |
| [CPY-06](../CPY-06-EMBEDDED-LINUX-RUNTIME.md) | Exact BBB/rootfs/kernel/Python facts, configured stable device identity/capability manifest, privilege/fd audit design, startup/device-loss/shutdown traces, import/input/render/present and held-lease cadence procedure. | Draft phase and CPY-01..05 dependencies. Direct profile forbids root/capabilities/`/dev/mem`; historical BBB display evidence is not a CPython import/touch/presentation proof. |
| [CPY-07](../CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md) | Per-host/backend main-thread lifecycle plan; hermetic canonical-frame versus screenshot labels; shared poll/async drain order; positive-count/reschedule, input and finalization cases. | CPY-03/04/05, per-row topology and drain-count PCDNs, remaining evidence and ratification. A native host smoke does not prove ordinary extension import/window topology. |
| [CPY-08](../CPY-08-PACKAGING-CROSS-DEPLOYMENT.md) | Per-target artifact/ABI/import rows, exact rootfs inputs, dependency/license/SBOM mapping, two clean reproducibility builds, offline install/upgrade and TestPyPI procedure, normalized PyPI project-control evidence. | Draft and selected binding/deployment profiles. Resolved packaging policy is not permission to create/upload an unqualified distribution. |
| [CPY-09](../CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md) | Claim-to-source/target/artifact/evidence joins, profile-specific measured budgets, independent version/compatibility inventory, durable-retention selection and owner Closure Record. | Three evidence/release PCDNs remain open. Rust repository/core v0.2.7 does not select CPY artifact SemVer. |

## Open decision packets

The six existing open PCDNs below remain open. Supply the listed inputs to the
owner; do not insert paper defaults while implementation evidence is absent.

| Existing decision | Packet inputs | Decision to be recorded by owner |
| --- | --- | --- |
| `PCDN-CPY-05-002` | Per-mode slot candidates; retained memory, pin-duration distribution, native cadence, conversion bandwidth, observer loss and required-consumer latency on host and BBB; saturation/lifetime/finalization cases. | Default/minimum/maximum counts and configurable bounds in CPY-09. Triple buffering remains a candidate. |
| `PCDN-CPY-07-002` | Separate OS/backend rows proving ordinary extension import, main-thread create/run/close and Binding Turns; exact failures and any proposed launcher trace. | Whether a Launcher-Owned Process is necessary for each row. No row inherits another platform's outcome. |
| `PCDN-CPY-07-004` | Positive-count immediate-reschedule policy plus callback latency, loop starvation, wakeups, queue depth, close latency and throughput, joined to CPY-03 capacity inputs. | Exact default/max drain count per turn with CPY-03/09 budgets. |
| `PCDN-CPY-09-001` | Comparable source/profile methods and startup/RSS/frame latency/jitter/throughput/artifact-size measurements from applicable CPY-03/05/06 evidence. | Profile-specific numeric envelopes and rejection criteria. Historical host values are not board defaults. |
| `PCDN-CPY-09-004` | Implemented interface compatibility, independently versioned extension/runtime/daemon/protocol/distribution, artifact/import/upgrade evidence and intended release claims. | Truthful first CPY artifact SemVer line. Preserve core0.2.7 while this remains unresolved. |
| `PCDN-CPY-09-005` | Expected large-bundle size, named durable artifact store, access/retrieval controls, duration and integrity/retrieval proof; small repository manifests and cryptographic locators. | Retention location and duration. External temporary session logs are not a durable retention policy. |

## Available software preparation and deferred verification

The [MPY snapshot packet](../../releases/v0.2.7-mpy-next-slice.md) supplies exact
source ownership candidates, revision/metadata questions and a new test design.
It establishes no shared handoff. CPY-02's historical baseline checker pins its
original path grant and must remain immutable rather than be repurposed as a
later migration record.

For a future checkpoint or changed runtime, the existing bounded suites below
are named verification consumers. They were not rerun in this slice because
their implementation is unchanged and wave-one results already exist:

```bash
cargo test -p rlvgl-runtime-std --test headless_owner --test native_service
python3 scripts/test_cpy_evidence.py
python3 scripts/test_cpy_capacity_probe.py
```

Keep native_service close/restart/accounting tests distinct from physical
capacity measurements and Frame Lease/finalization tests. Run changed-surface
tests when the corresponding authorized implementation lands; generate no new
index or immutable evidence here. Any new capture needs a clean exact source,
resolver, unused output/lock destinations and actual executing-target facts.

The next executable CPY action is a clean-source/known-board native capture
after its prerequisites are met. Draft binding, lease, Linux, asyncio and
package implementation stop at their own authorities. This packet leaves all
historical host bundles, source history, PCDNs and release gates intact.
