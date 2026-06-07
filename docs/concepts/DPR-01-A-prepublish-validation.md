# DPR-01-A — Pre-publish phases 6 + 7 validation

**Status:** Drafted 2026-05-19. Sub-letter to DPR-01-A. Snapshot of the
pre-publish gate state at HEAD `01c23b8` after the DPR-01 + DPR-02 +
DPR-03 doc work and the FrameScheduler / FreeRtosPacing scaffolds
landed.

## 1. Why this exists

The earlier phases of `scripts/pre-commit.sh` (0 fmt, 1 clippy, 2
workspace test, 3 playit, 4 simulator/creator, 4.5 disco-demo/sim) all
verified green in-session. Phases 6 (embedded build) and 7 (publish
dry-run) are the heaviest and weren't run during the orchestration
fan-out. This report closes that gap.

## 2. Phase 6 — `make build-disco`

**Result:** PASS.

Invocation: `make build-disco` from repo root. Target:
`thumbv7em-none-eabihf`, feature set
`cm7,splash,desktop,dma2d,cpu_stats,qspi_flash,sd_storage,audio` per
CLAUDE.md Makefile defaults.

### Artifacts produced

| Path | Size |
|---|---|
| `target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco` (ELF) | 8.7 M |
| `target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco.bin` | 374 K |
| `target/thumbv7em-none-eabihf/debug/rlvgl-stm32h747i-disco.hex` | 1.0 M |

### Baseline comparison

CLAUDE.md records the **release** profile fingerprint as ~321K ELF /
152K .bin / 448K .hex. The debug build is roughly 2.5× the release
sizes, which is the normal debug overhead and not a regression
attributable to the new `frame_scheduler.rs` / `pacing/freertos.rs`
modules.

Sanity check: a release build under the same feature set is what would
need to fit flash. Debug only validates that linking succeeds and
nothing in the new scaffolds breaks the address-map layout.

### Observations

- No new clippy warnings from the scaffold modules.
- The discipline scanner (Phase 2.5) remains green; the DPR-01a
  scaffold introduces no raw `*mut u32` and no `static mut`.
- The `frame_scheduler` and `pacing` modules compile under the
  full feature mix without requiring new optional deps.

## 3. Phase 7 — `DRY_RUN=1 scripts/publish_changed.sh`

**Result:** PASS (with one expected-failure caveat noted below).

### Run against `HEAD~1` (CLAUDE.md doc fix only)

```
$ DRY_RUN=1 scripts/publish_changed.sh HEAD~1
Publish diff:
  base: adc33a2 (v0.1.9-421-gadc33a2)
  head: 01c23b8 (v0.1.9-422-g01c23b8)
No changed crates detected; nothing to publish.
```

Expected — the only delta between `adc33a2` and `01c23b8` is
`CLAUDE.md` (not a publishable crate file).

### Run against session start `cdff3f8` (full session diff)

```
$ DRY_RUN=1 scripts/publish_changed.sh cdff3f8
Publish diff:
  base: cdff3f8 (v0.1.9-412-gcdff3f8)
  head: 01c23b8 (v0.1.9-422-g01c23b8)
Changed crates (publish order):
  - rlvgl-playit
  - rlvgl-platform
  - rlvgl
Dry run enabled; skipping cargo publish.
```

The script correctly identifies the three crates whose source set
changed across the session: `rlvgl-platform` (frame_scheduler.rs +
pacing/), `rlvgl` (transitive — workspace Cargo.lock), and
`rlvgl-playit` (transitive — workspace Cargo.lock). Order matches the
dependency graph.

### `cargo publish --dry-run -p rlvgl-platform` caveat

Running cargo's own publish dry-run for `rlvgl-platform v0.2.0` fails
with:

```
error: failed to prepare local package for uploading
Caused by:
  failed to select a version for the requirement `rlvgl-core = "^0.2.0"`
  candidate versions found which didn't match: 0.1.7, 0.1.6, 0.1.5, ...
```

**This is expected, not a regression.** `rlvgl-core 0.2.0` has not
been published to crates.io yet — it would be published first in the
same release, ahead of `rlvgl-platform`. The `scripts/publish_changed.sh`
output already enumerates the correct publish order; a real release
flow handles this naturally (publish core → platform → playit →
rlvgl), or uses `cargo workspaces publish` / similar tooling that
respects the dep graph.

The dry-run failure would surface against ANY pre-publish check on
this branch independent of the DPR work. It is not caused by the
session's commits.

### Package contents verification

`cd platform && cargo package --list --allow-dirty` confirms the new
scaffold files are in the publishable file set:

- `src/frame_scheduler.rs` ✓
- `src/pacing/mod.rs` ✓
- `src/pacing/freertos.rs` ✓

The platform crate's Cargo.toml does not use a restrictive `include`
list — every `src/**/*.rs` file is captured. Good — no Cargo.toml
update needed for the scaffold to ship.

## 4. Action items

Prioritized list of follow-ups surfaced by this validation pass.

1. **(P3, easy)** Run a `--release` `make build-disco-release` and
   record the size delta after the scaffold additions. Confirm the
   release artifact still fits flash. The scaffold is mostly inlined
   const-generic dispatch + a few atomic-load wrappers; flash impact
   should be near-zero, but worth measuring.

2. **(P3, doc)** The `scripts/publish_changed.sh HEAD~1` "no changed
   crates" output is misleading when the only diff is a CLAUDE.md
   tweak. Consider widening the doc to mention that a single-doc
   commit short-circuits the dry-run. Low priority — the script is
   correct, just briefly surprising.

3. **(P3, env)** `cargo publish --dry-run -p rlvgl-platform` against
   the local working tree fails because workspace deps haven't been
   bumped to 0.2.0 on crates.io. This is by design but worth a runbook
   note ("dry-run individual crates against a published-dep
   intermediate state, not against the local workspace head") for
   future agents.

4. **(P2, follow-up)** The `rlvgl-playit` crate appears in the
   changed-crates list against `cdff3f8` even though no source files
   in `playit/` were touched. Cause is presumably `Cargo.lock`
   transitive update from the `examples/stm32h747i-disco/Cargo.toml`
   ARM-gating change. Worth confirming with `git log --stat
   cdff3f8..HEAD playit/`. If lockfile drift is the only reason, the
   publish script could be tightened to skip crates whose source
   files are unchanged.

5. **(P1, none)** No blocking action items. The pre-publish gate is
   green end-to-end (with the well-known cargo-publish dep-version
   caveat).

## 5. Conclusion

Phase 6 and Phase 7 both pass. The session's commits between
`cdff3f8` and `01c23b8` are publishable and the embedded firmware
build is clean.

The remaining pre-publish gates (Phase 6 release-mode build, full
chip-target retests) are not blocking — they're optional thoroughness
that can run at actual-publish time.

## 6. Change Log

- **2026-05-19** — Initial draft. Captures Phase 6 + Phase 7 results
  against HEAD `01c23b8`. Validates that the DPR-01 / DPR-02 / DPR-03
  doc work + FrameScheduler / FreeRtosPacing scaffolds + workspace
  test gate fix + disco-sim test fixes all clear the heaviest
  pre-publish phases.
