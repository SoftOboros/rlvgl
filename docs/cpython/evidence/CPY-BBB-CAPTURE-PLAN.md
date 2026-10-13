<!--
CPY-BBB-CAPTURE-PLAN.md - Informative prerequisites for paired native-service BBB evidence.
-->

# CPY representative BBB capture plan

Prepared 2026-10-05; not executed on hardware. This plan prepares the existing
native-only diagnostic probe under [CPY-01](../CPY-01-BASELINE-TARGET-PROFILES.md)
and [CPY-02](../CPY-02-UNIFY-PARTITION-CRATES.md). It does not ratify
[CPY-03](../CPY-03-NATIVE-RUNTIME-SERVICE.md), select runtime capacities, or
authorize CPython/device implementation from later Draft phases.

## Evidence target

Run the committed `runtime-std/examples/cpy_capacity_probe.rs` workload on the
physical BeagleBone Black selected by CPY-01. The v3 probe exercises Stage/
Slider mutations and completions, pointer input/Cues, fixed native cadence,
private 320×240 RGBA rendering, readiness, bounded queues and close accounting.
It exports no Frame Lease and presents no physical display. It does not open
fbdev/evdev, prove physical touch, or import a Python binding. The Python
interpreter here runs the measurement tool; it is not the future CPython
Director qualification variant.

Expected retained result: four explicit ingress/egress/turn tuples, three
scenarios, one warmup and five retained iterations per row: 60 retained runs
and 12 summaries. Board results carry `qualification: physical-board-candidate`
and `normative_decision: false`; those labels alone do not qualify a tuple or
close a phase.

## Source and environment prerequisites

1. Verify the physical BBB identity and the CPY-01 selected Debian 13
   (`trixie`) armhf board/rootfs envelope. Retain actual rootfs provenance,
   kernel, CPU/RAM, toolchain and installed interpreter facts externally.
   No rootfs installation, flashing, permission change or service activation
   is part of this plan. A mismatching or unidentified board/rootfs is a stop.
2. Use an isolated, pristine checkout and exact nested source pins. Do not
   reset or clean the shared development checkout. The current release
   candidate has uncommitted package changes and cannot be captured yet.
3. Pair host and BBB at the same immutable source and retained resolver
   snapshot. The latest retained host pair candidate is source
   `2bb9a9116b3899355cb026ff1f1f30e383455a65`, with
   [`CPY-CAPACITY-CARGO-LOCK-2bb9a911.lock`](_generated/CPY-CAPACITY-CARGO-LOCK-2bb9a911.lock).
   A measurement for the final v0.2.7 candidate instead needs a new clean
   checkpoint and new host matrix before comparison. Selecting that checkpoint
   does not select capacities or a CPY release version.
4. Place a byte-identical copy of the paired resolver snapshot at the isolated
   checkout's ignored root `Cargo.lock`. Ensure required Cargo sources and
   native build prerequisites are available; use the existing installed
   Python ≥3.11 for `tomllib`. Resolve absent dependencies separately rather
   than modifying the workload or substituting host ABI facts.
5. Confirm no evidence output or generated lock with the chosen short source
   SHA already exists in this checkout. The helper uses exclusive creation
   for both files. A new output filename alone cannot bypass an existing
   `CPY-CAPACITY-CARGO-LOCK-<short-sha>.lock`. Preserve existing artifacts;
   use the pristine measured-source checkout, not a later checkout containing
   that source's already-retained output. Historical source `2bb9a911` predates
   the commit that retained its host bundle.

## Preflight on the board

Run these from the isolated checkout. Retain the outputs outside it so that
untracked preflight files do not invalidate the required clean source state.
An expected source SHA and resolver digest come from the paired manifest.

```bash
git rev-parse HEAD
git status --porcelain --untracked-files=all
git submodule status --recursive
sha256sum Cargo.lock
uname -a
cat /proc/device-tree/model
cat /etc/os-release
getconf GNU_LIBC_VERSION
python3 --version
rustc -vV
cargo --version
```

An empty Git status, exact source/lock agreement, physical-board identity and
adequate storage/memory are prerequisites, not inferred from a hardware label.
The capture builds with `--locked`; it does not freeze a newly resolved graph.
The runner records the executing native host triple, so a host-side cross-build
or emulation cannot substitute for executing on the physical BBB.

## Capture with the existing helper

Replace the date below with the actual capture date and confirm that both
output and generated-lock destinations are absent. The label records observed
hardware; it does not assert that the cape's touch sensor has been qualified.

```bash
python3 scripts/cpy_capacity_probe.py capture \
  --profile embedded-linux-direct \
  --physical-board \
  --hardware-label "BeagleBone Black + NHD-7.0CTP-CAPE-P" \
  --candidate 8:16:4 \
  --candidate 16:32:8 \
  --candidate 32:64:16 \
  --candidate 64:128:32 \
  --warmups 1 \
  --iterations 5 \
  --messages 1024 \
  --ingress-payload-bytes 256 \
  --egress-payload-bytes 128 \
  --observer-stall-us 50000 \
  --retry-backoff-us 50 \
  --sampling-hold-us 20000 \
  --frame-period-us 16667 \
  --output docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-BBB-YYYY-MM-DD.json

python3 scripts/cpy_capacity_probe.py validate \
  docs/cpython/evidence/CPY-CAPACITY-REPRESENTATIVE-BBB-YYYY-MM-DD.json
```

The helper runs cold-burst, sustained and observer-stall scenarios. Explicit
candidate values reproduce the retained host matrix; they are not public
defaults. Preserve complete logs, the JSON bundle and its generated resolver
snapshot. An interruption or failed validation yields no qualified row; retain
the diagnostic failure without overwriting a previous successful capture.

## Review and stopping point

Verify the manifest's actual Linux/ARM environment, `physical_board: true`,
source/lock hashes, sample counts, bounds, exact accepted-to-terminal and
Stage/input/Cue/frame accounting, ordered close and envelope release. Review
cadence, RSS, latency and observer-stall behavior against the paired host
capture under the accepted CPY-03 capacity policy; do not invent a budget or
pick the most convenient tuple automatically.

Prepare the evidence and review packet, then stop for the owner to assess
qualification and record any CPY-03 ratification. Binding, Frame Lease,
physical presentation, PyPI, release budgets and CPY artifact version remain
their own later gates. Existing BBB display observation and driver-only touch
status do not close those gates. No board run occurred in preparing this plan.
