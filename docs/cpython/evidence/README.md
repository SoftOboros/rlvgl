<!--
README.md - Provenance and verification guide for CPY baseline and phase evidence.
-->

# CPY Evidence

This directory contains machine-checkable CPY baseline and phase evidence.
Each artifact states its qualification boundary; diagnostic host measurements
do not satisfy embedded-Linux, physical-board, Python, frame, or release gates.

## Evidence set

| Artifact | Role |
|---|---|
| [`CPY-BASELINE-2026-08-18.json`](CPY-BASELINE-2026-08-18.json) | Exact source, authority, tool, interpreter, rootfs, board, and planned-artifact selection. |
| [`CPY-HANDOFF-0cf406b.json`](CPY-HANDOFF-0cf406b.json) | MPY owner's acknowledged Safe Point, verified MPY frontier, concurrent-work boundary, and exact paths authorized for the first shared CPY migration wave. |
| [`_generated/CPY-CARGO-LOCK-0cf406b.lock`](_generated/CPY-CARGO-LOCK-0cf406b.lock) | Detached copy of the resolver snapshot used for the baseline. The workspace intentionally ignores its root `Cargo.lock`; this evidence file preserves the exact selection without changing that policy. |
| [`_generated/CPY-GRAPH-0cf406b.json`](_generated/CPY-GRAPH-0cf406b.json) | Normalized workspace packages, features, local dependency edges, public-path hashes, and governed publish order. |
| [`CPY-CAPACITY-HOST-2026-08-18.json`](CPY-CAPACITY-HOST-2026-08-18.json) | CPY-03 diagnostic host matrix: four ingress/egress/turn candidates, three bounded-channel stress scenarios, and five retained iterations per row. It explicitly makes no capacity decision. |
| [`_generated/CPY-CAPACITY-CARGO-LOCK-9382b050.lock`](_generated/CPY-CAPACITY-CARGO-LOCK-9382b050.lock) | Detached resolver snapshot for the capacity probe, including Crossbeam Channel 0.5.16. |
| [`CPY-CAPACITY-SERVICE-HOST-2026-08-18.json`](CPY-CAPACITY-SERVICE-HOST-2026-08-18.json) | CPY-03 v2 diagnostic host matrix over the production native service, exact terminal records, close lifecycle, and Unix readiness. It explicitly makes no capacity decision. |
| [`_generated/CPY-CAPACITY-CARGO-LOCK-c994f163.lock`](_generated/CPY-CAPACITY-CARGO-LOCK-c994f163.lock) | Detached resolver snapshot for the v2 service probe, including production Crossbeam Channel and Rustix dependencies. |
| [`CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-19.json`](CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-19.json) | CPY-03 v3 diagnostic host matrix over representative Stage/input/Cue/private-frame work. It completes the host half of the paired host/physical-board evidence gate without selecting a capacity. |
| [`_generated/CPY-CAPACITY-CARGO-LOCK-87499319.lock`](_generated/CPY-CAPACITY-CARGO-LOCK-87499319.lock) | Detached resolver snapshot for the v3 representative host matrix from clean combined source `87499319`. |

The immutable source authority is commit
`0cf406bb22509f1040af6a772d0476a614c7bd9c`. The baseline hashes all 53
tracked Cargo manifests at that commit. The detached lock and normalized graph
are evidence artifacts rather than source authority because the root lockfile
is intentionally ignored for this library workspace.

## Qualification-state rule

- `selected` means the exact version, digest, target, or rootfs is frozen for a
  later gate.
- `planned` means the package or artifact does not exist yet.
- `verified` is reserved for retained execution evidence from the owning
  phase.
- The BBB display is `observed-functional` from the existing 800x480 RGB565
  fbdev proof. Touch remains `observed-driver-only`: `edt_ft5x06` binds and
  registers an evdev node, but the cape's physical sensor is under RMA.

CPY-01 ratification therefore freezes a reproducible matrix; it does not
satisfy CPY-06 target import, physical touch, service lifecycle, or CPY-09
release evidence.

## External pins

- CPython patch releases are 3.13.15 and 3.14.7 from the
  [official Python downloads](https://www.python.org/downloads/).
- ARM rootfs rows use per-platform manifests from the
  [official Python container images](https://hub.docker.com/_/python) for
  `3.13.15-slim-trixie` and `3.14.7-slim-trixie`.
- The recorded glibc package version is
  [Debian 13 `libc6` 2.41-12+deb13u3](https://packages.debian.org/trixie/arm64/libc6).
  It was independently confirmed in each pinned OCI image by reading the last
  layer containing `/var/lib/dpkg/status`; the exact layer digest is retained
  in each rootfs row.
- PyO3 0.28.3 uses the checksum recorded by
  [crates.io](https://crates.io/crates/pyo3/0.28.3). Maturin 1.13.0 uses the
  source-distribution SHA-256 published by
  [PyPI](https://pypi.org/project/maturin/1.13.0/).

## Verification

Run the authored baseline check and current-graph firewall:

```bash
python3 scripts/cpy_evidence.py all
python3 scripts/test_cpy_evidence.py
```

The test suite includes a negative control that injects a synthetic PyO3 edge
into `rlvgl-api` and requires the firewall to reject it. The checker uses the
`jsonschema` package when available and otherwise uses its dependency-free
validator for the exact schema features used here.

The generated graph and detached lock are captured only at a clean baseline
boundary:

```bash
python3 scripts/cpy_evidence.py capture \
  --manifest docs/cpython/evidence/CPY-BASELINE-2026-08-18.json
```

Do not edit `_generated/` by hand. A new baseline uses a new manifest and
artifact names; it does not overwrite historical evidence.

## CPY-03 capacity evidence

The retained v1 probe constructs the non-`Send` Endpoint on its owner thread
and uses bounded Crossbeam channels around empty neutral Safe Turns. The v2
probe drives the production `NativeService`, including its typed admission,
turn batching, terminal records, close sequence, and Unix OS readiness. Both
record cold-burst admission, sustained retry pressure, and a 50 ms stalled
observer. Owned-envelope accounting, whole-process peak RSS, completion count,
ordering, queue-depth bounds, readiness/transition counts, and latency
distributions are retained for every run.

The committed host bundle is `diagnostic-host` with
`normative_decision: false`. It cannot close `PCDN-CPY-03-002`: the workload
is the v1 transport-only workload and does not include the implemented OS
readiness boundary or representative actor/render/frame/input work.

The v2 host bundle is also `diagnostic-host` with
`normative_decision: false`. All 60 runs used the production service and
completed with exact terminal accounting, clean ordering, bounded depths,
released tracked envelopes, and a drainable readiness path. It still executes
an empty Endpoint Safe Turn, and it has not run on the CPY-01 BeagleBone Black,
so it also cannot close the PCDN or select defaults/maxima.

The v3 representative host bundle remains `diagnostic-host` with
`normative_decision: false`. Its 60 retained runs cover the same four
candidates and three scenarios while adding real Stage mutation/completion,
pointer input and Cue accounting, fixed native cadence, and private flattened
RGBA rendering. Every one of its 12 summaries reports exact representative
semantics, clean sequence accounting, and full owned-envelope release. It
completes the host half of the accepted paired matrix, but the physical
BeagleBone Black run remains required and no row is a public default, maximum,
or release budget.

Validate the retained bundle:

```bash
python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-HOST-2026-08-18.json
python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-SERVICE-HOST-2026-08-18.json
python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-HOST-2026-08-19.json
python3 scripts/test_cpy_capacity_probe.py
```

Capture a new clean-source host matrix under a new evidence name:

```bash
python3 scripts/cpy_capacity_probe.py capture \
  --profile host-headless \
  --hardware-label <stable-hardware-label> \
  --output docs/cpython/evidence/<new-name>.json
```

An embedded run additionally requires `--profile embedded-linux-direct` and
`--physical-board`. Those flags identify the environment; they do not promote
candidate measurements into defaults or a release budget.

Run the paired physical-board capture from a clean checkout of the committed
representative workload on the CPY-01 reference board:

```bash
python3 scripts/cpy_capacity_probe.py capture \
  --profile embedded-linux-direct \
  --physical-board \
  --hardware-label "BeagleBone Black + NHD-7.0CTP-CAPE-P" \
  --output docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-BBB-<date>.json
```
