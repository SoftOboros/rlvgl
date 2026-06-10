<!--
docs/crates-ci/CRATES-CI-RETROSPECTIVE.md — Initiative retrospective per the
Spec-Before-Code Planning Discipline (CLAUDE.md §Initiative retrospective).
Historical artifact; behaviour PRs cite CRATES-CI-00-CONCEPTS.md + its §15,
never this file. Audience: future agents running a structurally similar
initiative. Tone: mechanistic, neutral.
-->

# CRATES-CI Retrospective

Initiative: crates-built headed-surface CI (`docs/crates-ci/`,
CRATES-CI-00…05 + sub-letter amendments). Planned, ratified, implemented,
and proven against a live release (v0.2.2 published through the gates)
in a single day, 2026-06-10. Completion event: all §12 phases shipped;
Gate R green against real crates.io.

## §1 Outcome snapshot

**Shipped architecture**
- **Gate P** (`scripts/crates_ci_stage.sh` + `.github/workflows/crates-ci.yml`
  + embedded `gate-p` job in `publish.yml`, `needs:`-gating publish per
  INV-C1): packages all publishable crates in publish order with a
  progressive extract-and-`[patch.crates-io]` bootstrap, then builds three
  workspace-detached Consumer Projects against the packaged set
  (`consumers/lib-smoke/`, `consumers/creator-cli/`, `consumers/user-sim/`).
  Staging additionally enforces P-META (publish-fatal manifest warnings →
  errors) and version-drift (same-version-different-content vs crates.io →
  error) gates.
- **Gate R** (`.github/workflows/gate-r.yml`): the same consumers against
  real crates.io after every successful Publish and daily; creator path is
  the literal `cargo install rlvgl --features creator`.
- **Layer K**: `egui_kittest 0.33.3` in-process harness for `CreatorApp`
  (`tests/creator_ui_kittest.rs`), accesskit queries + wgpu snapshot at
  explicit threshold.
- **Layer W**: `rlvgl-creator --automation-headless --playit-port=<n>`
  (feature `creator_ui_automation`) serving the frozen playit verb set over
  TCP against the kittest engine, byte-compatible with the unmodified
  `playit/node` client.
- Publish-order hardening: `--print-order` single-sourcing; matrix test
  completeness derived from `cargo metadata`.

**Production evidence**: v0.2.2 tag → gate-p green → 23 crates published
→ Gate R green (user-sim flipped from expected-red to green the moment
0.2.2 indexed). Stragglers (`disco-assets`, `rlvgl-micropython` 0.2.2)
published via Publish Continue after their drift-gate-driven bumps.

**Defects the gates caught before/at first contact** (all fixed in-tree):
publish-order omission (`rlvgl-audio-meters-core`), unbuildable packaged
`rlvgl/simulator` (P-INCLUDE), Mach-O host-build break in
`rlvgl-platform`, missing publish metadata in `disco-assets` (P-META),
silent stale-version publishing (`rlvgl-micropython`, `disco-assets`),
consumer gate passing without consuming the staged set (gate honesty),
and the drift gate's own chipdb false positive.

**Deferred items**: enumerated in §5. **Residual risks**: the drift-gate
exemption for the two asset-bearing crates rides on
`publish_changed.sh`'s path mappings staying correct (§5, Coupled);
Gate P requires crates.io sparse-index reachability (CRATES_CI_OFFLINE=1
escape documented); Layer K/W are pinned to the egui 0.33 line.

## §2 Divergence log

Format: **Assumption → Symptom → Root cause → Detection gap.**

1. **Staged Registry as directory source (§6 as drafted).**
   Assumption: extracted `cargo package` output could replace crates.io as
   a directory source. Symptom: design unimplementable — and separately,
   `cargo package` for `rlvgl-playit` failed resolving `rlvgl-core ^0.2.2`.
   Root cause: a replaced registry must satisfy ALL dependencies, and
   unpublished versions cannot exist in any replaced index; additionally
   `cargo package` re-resolves the sanitized manifest, so unpublished
   sibling versions are unresolvable until injected. Detection gap: none —
   caught at first execution, which is what §6.4's amendment escape hatch
   anticipated. Resolution: progressive extract + `[patch.crates-io]` via
   `--config`, which also converts publish-topology errors into Gate P
   failures.

2. **Publish-order completeness.** Assumption: the matrix test guaranteed
   every publishable crate was in `ordered_crates`. Symptom: lib-smoke's
   offline resolution mentioned `rlvgl-audio-meters-core` — a
   `publish = true` workspace crate, depended on by `rlvgl-widgets`,
   absent from the order. Root cause: both the script list and the test's
   crate inventory were hand-maintained twins; omissions were invisible to
   a string-contains test. Detection gap: no derivation from
   `cargo metadata`. Fixed: completeness test derives the publishable set.

3. **Mach-O host builds.** Assumption: `rlvgl-platform` host-compiles on
   any dev machine. Symptom: LLVM fatal ("mach-o section specifier
   requires a segment") on every macOS build. Root cause: ELF-style
   `link_section` on the blit scratch static without a target cfg.
   Detection gap: CI is Linux-only and nothing consumed the crate on
   another host OS; the crates-first story was silently Linux-only.

4. **Packaged ≈ workspace builds (P-INCLUDE).** Assumption: a crate that
   builds in the workspace builds when packaged. Symptom: staged
   `rlvgl --features simulator` failed; `rlvgl-app-disco-demo`
   `include_bytes!`ed 10 icons via `../../../stm32h747i-disco/...` —
   outside its crate root, unpackageable by construction; published 0.2.1
   was broken the same way. Detection gap: nothing built from packaged
   artifacts anywhere in CI. This is the initiative's thesis defect.

5. **Gate honesty / stale lockfile.** Assumption: `[patch.crates-io]`
   entries always win resolution. Symptom: lib-smoke PASSED while
   compiling registry `0.2.0` crates; every patch entry sat in
   `[[patch.unused]]`. Root cause: cargo honors an existing `Cargo.lock`
   over patch candidates; a lockfile left by an earlier `cargo metadata`
   pinned registry sources. Detection gap: a passing gate and a vacuous
   gate were observationally identical — no assertion tied the resolved
   graph to the staged sources. Fixed: fresh-lock + staged-source lock
   scan in every consumer runner.

6. **Warning/error asymmetry (P-META).** Assumption: if `cargo package`
   succeeds, `cargo publish` will too (given `--no-verify` parity).
   Symptom: `disco-assets` reached the publish step with no `license`
   field ("no manifest [metadata]" failure). Root cause: cargo only WARNS
   at package time for conditions that are hard errors at publish time.
   Detection gap: staging tolerated all warnings. Fixed: publish-fatal
   warning subset promoted to staging errors.

7. **Version-bump discipline.** Assumption: changed crates get version
   bumps. Symptom: `rlvgl-micropython` and `disco-assets` content changed
   across the v0.2.1 cycle at unchanged versions; publish printed
   "Skipping (already published)" and shipped nothing — stale code path.
   Root cause: the skip checks version EXISTENCE, not content identity
   (and the skip is load-bearing for publish re-runs, so it cannot simply
   error). Detection gap: nothing compared packaged vs published content.
   Fixed: drift gate (member-content compare ignoring
   `.cargo_vcs_info.json` / normalized `Cargo.toml` / `Cargo.lock`).

8. **Validation path ≠ enforcement path (drift-gate false positive).**
   Assumption: local `SKIP_ASSET_PREP=1` staging validated the drift gate.
   Symptom: first CI executions (publish workflow_dispatch) failed gate-p
   on `rlvgl-chips-stm` "drift". Root cause: `stm32_afdb_pipeline.sh`'s
   `chipdb.bin.zst` is not byte-reproducible run-to-run, so full-prep
   packaging never matches the published archive; the local shortcut had
   skipped exactly the crates whose CI behavior differed. Detection gap:
   the only code path that could fail in CI was the one local validation
   exempted. Fixed: permanent loud exemption for the two asset-bearing
   crates (verified by full-prep repro: only `assets/chipdb.bin.zst`
   differed).

9. **Workspace enclosure of staged packages.** Assumption: an extracted
   package dir builds standalone anywhere. Symptom: "current package
   believes it's in a workspace when it's not" building inside
   `target/crates-ci/staged/`. Root cause: packaged manifests carry no
   `[workspace]` table; cargo walks up to the repo manifest. Detection
   gap: none — first in-place build caught it. Fixed: `target` added to
   `workspace.exclude`.

10. **Toolchain-coupled compile-fail fixtures.** Assumption (implicit):
    `discipline_compile` fixtures are host-independent. Symptom: once
    divergence 3 was fixed and platform compiled on macOS at all, the
    `mmio_dsi_offset` fixture mismatched on diagnostic caret spans only.
    Root cause: rustc diagnostic rendering varies across versions; the
    fixture was blessed by the CI container's rustc. Detection gap:
    masked for the entire period the crate could not compile on macOS.
    Disposition: not blessed locally; CI rustc remains the fixture
    authority.

## §3 Refactor points

Format: **Trigger → Alternatives → Selection rationale → Cost of switch.**

1. **Staged Registry mechanism.** Trigger: divergence 1. Alternatives:
   (a) pure directory source, (b) self-hosted registry server
   (margo/kellnr), (c) `[patch.crates-io]` over extracted packages.
   Selected (c): zero infrastructure, injects nonexistent-upstream
   versions (the defining pre-publish requirement), third-party deps keep
   resolving from crates.io. Cost: one §6 amendment + staging-script
   rewrite from two-pass to progressive single-pass (which bought
   topology validation for free).

2. **GUI harness shape.** Trigger: AskUserQuestion at spec time; owner
   selected "playit over kittest + kittest" over plain kittest, xvfb, or
   a hand-rolled egui playit port. Rationale: one wire protocol across
   all three headed surfaces; kittest supplies the egui-native engine so
   the executor only maps verbs. Cost: `egui_kittest` becomes an optional
   production dependency behind `creator_ui_automation`; accepted.

3. **Gate R creator path.** Trigger: designing CRATES-CI-05; the Gate P
   build-inside-staged-package shape has no registry analogue.
   Alternatives: GATE=r variant of smoke.sh vs literal `cargo install`.
   Selected `cargo install`: it is the actual end-user command and
   exercises crates.io resolution + the install profile. Cost: a second
   entry point (`gate_r.sh`) per the consumer, duplicated round-trip
   logic.

4. **user-sim dependency shape.** Trigger: umbrella
   `rlvgl/simulator` was unbuildable from packages (divergence 4) while
   the consumer was being built in a parallel lane. Alternatives: block
   on the fix vs consume `rlvgl-core/-widgets/-platform/-playit`
   directly. Selected direct deps (recorded §8.1 deviation): more
   user-realistic (the root crate doesn't re-export `rlvgl-playit`), and
   the umbrella path gained permanent coverage in creator-cli's smoke
   instead (CRATES-CI-02a). Cost: §15 deviation record; two shapes to
   keep in mind when reading consumers.

5. **Drift-gate scope.** Trigger: divergence 8. Alternatives: per-crate
   generated-path ignore lists vs permanent loud exemption for the two
   asset-prep crates vs making the chipdb pipeline reproducible.
   Selected exemption: ignore lists gut the check anyway for those
   crates; pipeline reproducibility is a separate initiative-sized fix.
   Cost: those two crates' stale-version risk is carried by
   `publish_changed.sh` path mappings alone (named residual, §1/§5).

## §4 Mitigation patterns

- **Gate-honesty assertion.** When a harness injects sources (patches,
  overlays, mocks), the gate MUST assert the output graph actually used
  them (here: lockfile scan rejecting registry-sourced `rlvgl-*`). A gate
  that can pass vacuously will, eventually, and silently.
- **Validation path = enforcement path.** Any local-validation shortcut
  that disables a check for subset X leaves X's CI behavior unvalidated.
  Enumerate the skipped subset explicitly and exercise it once for real
  before shipping the gate (divergence 8 cost one broken publish cycle).
- **Promote publish-fatal warnings.** Wherever a toolchain downgrades a
  later-stage hard error to an early-stage warning (`cargo package` vs
  `cargo publish` metadata), the earliest gate re-promotes it.
- **Pair every "exists → skip" with content identity.** Resume-friendly
  skips (publish re-runs) are correct only when skipped content is
  provably identical; otherwise they ship stale artifacts.
- **In-crate-root assets only.** `include_bytes!`/`include_str!` paths
  reaching outside the crate root are unpackageable by construction and
  invisible to workspace builds. Greppable; worth a future lint.
- **Loud exemptions, accumulated failures.** Every skipped check prints
  why; audit-style gates collect all findings before failing (the drift
  gate lists every drifted crate, not the first).
- **Progressive bootstrap for interdependent packaging.** Stage → inject
  → package-next in topological order; order errors then surface at the
  exact crate that would have failed the real publish.

## §5 Deferred work reclassification

**Safe** (orthogonal; no invariant impact):
- `push: branches: [main]` trigger for `crates-ci.yml` (visibility gap
  observed when direct-to-main commits ran nothing).
- `concurrency:` group to dedupe push+PR double runs of Gate P.
- playit/node launcher naming (`launchDiscoSim` is generic in behavior;
  rename/sibling is a playit-crate decision per §10).
- actions/checkout Node 24 deprecation warning in workflows.

**Coupled** (revisit with named assumption):
- `disco-assets` `embed` feature repair — same out-of-crate-root pattern
  as divergence 4. Assumption it rides on: no consumer enables
  `disco-assets/embed` from the registry. Revisit before advertising the
  feature.
- egui version skew (platform eframe 0.27/glow vs creator 0.33).
  Assumption: the two stacks never link into one binary. Touching either
  side moves Layer K/W (kittest pin) and the simulator together.
- `D` dump verb in CI — currently allowed to degrade to
  `ERR: render-unavailable`. Assumption: accesskit verbs suffice for CI
  assertions on the creator UI. Revisit if pixel oracles are needed in
  CI (would require software Vulkan in the container, INV-C5-compatible).
- `GATE_R_FEATURES` growth to `creator_ui,creator_ui_automation`.
  Assumption: latest published 0.2.x builds those features (true from
  0.2.2 on; flip when convenient).
- Drift-gate exemption for `rlvgl-chips-stm`/`rlvgl-bsps-stm`.
  Assumption: `publish_changed.sh` path mappings stay the sole and
  sufficient change-detector for them. A reproducible-chipdb effort
  would dissolve the exemption.
- `MT` multi-touch verb in Layer W — rejected as unsupported; needs an
  egui multi-touch mapping if creator UI ever wants gesture tests.

**Abandoned** (with resurrection prevention):
- Pure directory-source Staged Registry. Do not re-derive: a replaced
  crates.io source can never resolve unpublished versions, and full
  third-party vendoring would be required besides. `[patch.crates-io]`
  is the mechanism (§6 as amended).
- xvfb/display-server harnesses. INV-C5 forbids them; kittest + wgpu
  covers the need with deterministic in-process control.

## §6 Forward constraints (normative)

- **FC-1** — A new CI gate MUST NOT ship while any local-validation
  shortcut leaves part of its CI-only behavior unexercised; the skipped
  subset MUST be run once for real (or in CI on a branch) first.
- **FC-2** — Release tooling MUST NOT contain an "already exists → skip"
  branch without either a content-identity check or a documented, loud
  exemption naming the alternative change-detector.
- **FC-3** — A new publishable crate MUST carry full publish metadata
  (description, license) and a version distinct from any published
  content at birth; the metadata-derived completeness test and the drift
  gate are the enforcing mechanisms and MUST NOT be weakened to admit a
  crate.
- **FC-4** — A new headed surface (windowed binary, GUI feature) MUST
  land in the same phase as its headless harness; "tested manually on the
  dev box" does not satisfy any acceptance checklist.
- **FC-5** — Asset references in publishable crates MUST resolve inside
  the crate root (vendor or generate-into; never `../`).
- **FC-6** — Harnesses that inject sources MUST assert provenance of the
  resolved artifact (FC-1's runtime twin; see §4 gate-honesty).

## §7 Provenance hooks

- Concepts doc + amendments: `docs/crates-ci/CRATES-CI-00-CONCEPTS.md`
  §15 (six dated 2026-06-10 entries).
- Phase commits: `131cb42` (00 ratify), `70c0268` (01), `fa5e195` (02),
  `6acf2ff` (02a), `d6a4b22` (03), `d428fb5` (04), `c665311` (05),
  `2f748c6` (01a gates + bumps), `3276fef` (01a exemption),
  `b23ce8a` (chipdb blob untrack).
- Evidence base (pre-initiative release repairs): `58bb737`, `bc69338`,
  `4d9255a`, `9bee2f9`; last pre-gate tag `v0.2.0` (`bfa5499`).
- Production runs: Publish tag run 27284101609 (success, gate-p
  embedded); failed workflow_dispatch runs 27286329983 / 27287089496 /
  27288556670 (divergence 8); first Gate R run green post-publish;
  Gate P container-init infra flake 27283480148 (not code).
- Divergence 4/7 registry truth: crates.io index entries for
  `rlvgl` 0.2.1 (broken simulator), `disco-assets` 0.2.0 / 0.2.2,
  `rlvgl-micropython` 0.2.0 / 0.2.2.

## §8 Change log

- **2026-06-10** — Retrospective drafted at initiative completion
  (CRATES-CI-06). All §12 phases shipped; v0.2.2 published through the
  gates; Gate R green against live crates.io.
