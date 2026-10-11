<!--
CPY-V027-READINESS.md - Informative verification receipt and remaining CPY release gates.
-->

# CPY v0.2.7 readiness receipt

Recorded 2026-10-05. This is an informative evidence inventory, not a phase
ratification, capacity selection, or CPY release closure.

## Authority and scope

The primary phase authorities are the ratified [CPY-00](../CPY-00-CONCEPTS.md),
[CPY-01](../CPY-01-BASELINE-TARGET-PROFILES.md), and
[CPY-02](../CPY-02-UNIFY-PARTITION-CRATES.md). CPY-02 §8 permits CPY-only
planning while requiring a new coordinated handoff before another shared-path
migration. SBC authority is the exact parent revision cited by
[`CLAUDE.md`](../../../CLAUDE.md#current-sbc-authority-and-rlvgl-adoption).

This slice validates existing evidence and checker controls, adds informative
inventory pointers, and prepares a later BBB capture. It changes no runtime,
manifest, generated evidence, normative phase section, PCDN, or phase status.

## Source boundary

The release candidate is based on
`aa94329ff9b26e8afeb9e0e37f824a1ae17bea6b` with local package changes,
including `rlvgl-core 0.2.7` and the matching `runtime-std` dependency
requirement. The wave-one baseline receipt outside the repository identifies
that starting tree. It is not a new clean CPY measurement baseline.

Comparing the 23 source records in each representative bundle to the current
candidate files gives:

| Retained capture | Exact measured source | Current-file differences | Qualification |
|---|---|---|---|
| [August 19](CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-19.json) | `8749931931abbd3aa53e97ecf9a95a530439c6bc` | Eleven source records: probe, runtime lib/service/manifest/tests, API protocol, core actor/direction/style cascade, widget container/slider. | `diagnostic-host`, `physical_board: false` |
| [August 23](CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-23.json) | `2bb9a9116b3899355cb026ff1f1f30e383455a65` | `runtime-std/Cargo.toml` only; its core requirement is now 0.2.7. | `diagnostic-host`, `physical_board: false` |

Both bundles remain valid against their original commits and retained resolver
snapshots. Matching later implementation files does not requalify either
capture for this candidate. The August 23 hardware label mentions BBB pairing;
its actual measured environment is macOS, and no BBB capture is retained in
this evidence inventory. Neither capture selects capacities, maxima, budgets,
or defaults; both retain `normative_decision: false`.

## Verification receipt

Interpreter: installed Python 3.12.10. No new Python packages or tools were
installed. Cargo metadata required unpacking cached crates and fetching the
missing `libudev`/`libudev-sys` resolver inputs; no Rust compilation was needed.

| Check | Result |
|---|---|
| Historical baseline and coordination-closeout validation | Pass; baseline `0cf406bb22509f1040af6a772d0476a614c7bd9c` and first-wave closeout preserved. |
| Existing baseline/firewall tests | 6 passed, including rejection of invented authority revisions, relabelled MPY acceptance, and a synthetic PyO3 edge into API. |
| Current workspace dependency firewall | Pass; this is current-graph evidence, distinct from historical baseline verification. |
| Existing capacity-checker tests | 11 passed, including sequence, bound, incomplete-matrix, missing-Cue, and workload-version negative controls. |
| August 19 representative bundle | Pass; 60 retained runs, 12 summaries, exact immutable source hashes and retained lock hash. |
| August 23 representative bundle | Pass; 60 retained runs, 12 summaries, exact immutable source hashes and retained lock hash. |
| Documentation links and whitespace | Offline Lychee: 57 links, 52 valid local links, 5 external links excluded, zero errors; `git diff --check` passed. |

Reproduce the checks with the installed Python ≥3.11 interpreter:

```bash
python3 scripts/cpy_evidence.py check
python3 scripts/test_cpy_evidence.py
python3 scripts/cpy_evidence.py firewall
python3 scripts/test_cpy_capacity_probe.py
python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-19.json
python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-23.json
```

The checks read historical artifacts; they do not capture a new matrix.
Session logs are retained outside the source tree in the wave-one CPY log
directory. Final documentation/index integration belongs to the release
coordinator.

## Remaining gates and stop packet

The implemented resting point remains the interpreter-neutral native service.
CPY-03 through CPY-09 are Draft. As defined in
[CPY-03 §§12/14](../CPY-03-NATIVE-RUNTIME-SERVICE.md), the next gate is the
paired representative workload on the physical CPY-01 BBB, then an owner
ratification record. The [capture plan](CPY-BBB-CAPTURE-PLAN.md) prepares that
step and records no hardware execution.

| Deliverable | Remaining boundary |
|---|---|
| CPY-03 | Physical BBB capacity/cadence qualification at an explicit tuple and owner ratification. Exported frames, Python, and physical display presentation are later-phase evidence. |
| CPY-04 | Binding implementation, neutral-operation parity, thread/callback/finalization proof, and phase authority remain open. |
| CPY-05 | As defined in [CPY-05 §11.3](../CPY-05-FRAME-LEASE-BUFFER-PROTOCOL.md), `PCDN-CPY-05-002` still requires measured slot capacities. Frame export/lifetime evidence remains open. |
| CPY-06 | Exact rootfs/device facts, target import, permission/lifecycle and physical presentation evidence remain open. Existing Rust BBB display evidence is not CPython proof. |
| CPY-07 | As defined in [CPY-07 §11.3](../CPY-07-HOST-HEADLESS-WINDOWED-ASYNCIO.md), `PCDN-CPY-07-002` and `PCDN-CPY-07-004` remain open for per-host topology and measured drain counts. |
| CPY-08 | Python source/wheel artifacts, isolation, target ABI/import, reproducibility and normalized PyPI project control remain open. |
| CPY-09 | As defined in [CPY-09 §11.3](../CPY-09-CONFORMANCE-PERFORMANCE-RELEASE.md), `PCDN-CPY-09-001`, `PCDN-CPY-09-004` and `PCDN-CPY-09-005` remain open for budgets, CPY artifact version and durable retention. Repository/core 0.2.7 does not select the Python artifact version. |

Stop before resolving these decisions, entering an unratified implementation
phase, changing shared ownership, or relabelling host measurements as board
qualification. A new source measurement needs a clean immutable checkpoint,
its exact resolver snapshot, new artifact names, and a separately retained
host/board pair. No package was published and no phase gate was closed here.
