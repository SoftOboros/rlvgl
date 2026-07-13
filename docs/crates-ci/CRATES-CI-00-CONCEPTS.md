<!--
docs/crates-ci/CRATES-CI-00-CONCEPTS.md — Crates-Built Headed-Surface CI.
Initiative concepts doc per the Spec-Before-Code Planning Discipline
(CLAUDE.md). Commit-subject prefix: CRATES-CI-NN[a-z]:.
Status: RATIFIED 2026-06-10. See §15.
-->

# CRATES-CI-00 — Concepts: CI for Crates-Built Headed Surfaces

## §0 Authority policy

| Vocabulary domain | Authoritative source |
|---|---|
| Cargo packaging, registries, source replacement, `[workspace]` detachment | The Cargo Book (`cargo package`, `cargo vendor`, source-replacement chapters) |
| egui/eframe application model, accesskit tree, kittest harness | egui 0.33 / eframe 0.33 / `egui_kittest` 0.33 upstream docs |
| Playit wire protocol verbs and framing | `playit/README.md` + `playit/` crate source — owned by the playit crate, NOT restated here |
| Publishable crate set and topological order | `scripts/publish_changed.sh` (`ordered_crates`, printable via `--print-order`) locked by `tests/publish_changed_matrix.rs` |
| Downstream consumer recipe (custom simulator) | `docs/CUSTOM-SIMULATOR.md` |
| Existing test-tier registry | `docs/TEST-STRATEGY.md` |

Where this doc and an authority conflict, the authority wins and this doc
gains a §15 amendment.

## §1 Purpose

Close the CI gap that produced the v0.2.1/v0.2.2 release-repair divergence:
no CI job consumes rlvgl **as packaged crates** (the contractee distribution
story), and no CI job exercises either **headed surface** — the
`creator_ui` GUI wrapper or the windowed/wgpu simulator path. This
initiative defines the harnesses and gates so that both surfaces are
tested *as a downstream user would build them from crates.io*, headlessly,
before a release tag exists.

## §2 Problem statement

Branch `v0.2.2` (untagged; last published tag `v0.2.0`) is composed
entirely of release-repair commits — failures discoverable only at
publish/consume time:

| Commit | Repair | Failure class |
|---|---|---|
| `58bb737` | disco-assets metadata (name/version/license) | **P-META** packaging metadata |
| `bc69338` | publish-order dev-dependencies reshuffle | **P-ORDER** publish topology |
| `4d9255a` | 91-file v0.2.1 sweep; include-set tightening (`5e38de4`); publish order (`8dc2876`) | **P-META / P-ORDER / P-INCLUDE** |
| `9bee2f9` | fontdue reorder in `core/Cargo.toml` for feature resolution | **P-RESOLVE** feature resolution outside workspace |
| `0f038c7`, `28bcb91`, `dbce7fa`… | publish-continue workflow repairs | **P-ORDER** |

Why CI missed all of them:

1. `ci.yml` builds the **workspace in-tree** — path dependencies, unified
   feature resolution, no `include` enforcement. None of P-META, P-INCLUDE,
   P-RESOLVE, P-ORDER can manifest.
2. No workflow builds the `creator_ui` feature at all (`OPTIONS.md` calls it
   "heaviest feature; best off for CI"), so the eframe-0.33 GUI wrapper
   (`src/bin/creator_ui/`) bitrots silently — failure class **H-GUI**.
3. No workflow runs the windowed simulator path
   (`platform/src/simulator.rs`, winit + wgpu 0.19 + eframe 0.27/glow).
   Headless/`--automation-headless` paths are exercised (Phase 4.5);
   the headed path is not — failure class **H-SIM**.
4. The recursive-sim problem: the deliverable *is* a simulator, so "run the
   sim" is not a usable oracle; the oracle must be offscreen rendering +
   wire-protocol introspection, not a window.

## §3 Glossary

- **Headed Surface** — a build artifact whose primary mode opens a window:
  the GUI Wrapper and the windowed simulator. Testing a Headed Surface
  headlessly means driving its application logic and render output without
  a display server.
- **GUI Wrapper** — the desktop UI of `rlvgl-creator`: module
  `src/bin/creator_ui/` behind the root-crate `creator_ui` feature, entered
  by `rlvgl-creator` with no CLI args (`src/bin/rlvgl_creator/main.rs`).
  As defined in code; used without modification.
- **User Sim** — a downstream-authored simulator binary built **only from
  registry crates**, following `docs/CUSTOM-SIMULATOR.md`
  (`WgpuDisplay::new(w,h).run(...)`). Owned by this initiative as a
  canonical Consumer Project; does not exist in repo yet.
- **Consumer Project** — a throwaway, workspace-detached (empty
  `[workspace]` stanza) cargo project that depends on rlvgl crates by
  **registry source only** — no `path =`, no `[patch]` back into the
  workspace. Same detachment precedent as the CHIPS-\*-06 example crates.
- **Staged Registry** — a cargo source built from the output of
  `cargo package` across the publish list, substituted for crates.io via
  source replacement, so Consumer Projects build against the *packaged*
  artifacts before any tag exists.
- **Gate P (pre-publish)** — CI gate: all Consumer Projects build and pass
  their tests against the Staged Registry. Blocks tagging/publishing.
- **Gate R (post-publish / registry-truth)** — the same Consumer Projects
  built against real crates.io, run after publish and on a schedule.
- **Playit Wire Protocol** — the newline-delimited command protocol
  (`T`, `PD/PM/PU`, `KD/KU`, `QB/QE/QC`, `T@tag`, `D`, `?`, …). As defined
  in `playit/README.md`; used without modification at the verb level.
  Adapted: a new **transport/executor** binds the verbs to a kittest
  harness instead of an rlvgl widget tree (§7).
- **Kittest Engine** — an `egui_kittest` 0.33 `Harness` hosting
  `CreatorApp` in-process: drives `update()`, queries the accesskit tree,
  injects pointer/key events, renders snapshots without a window.
- **Snapshot Oracle** — deterministic offscreen render compared against a
  committed golden PNG with an explicit per-test threshold (never
  bit-exact across GPU/driver variation).

## §4 Source-of-truth map

| Concept | Owner |
|---|---|
| Wire protocol verbs/framing | `playit/` crate (`playit/README.md`) |
| Node automation client | `playit/node/src/index.js` (`launchDiscoSim` pattern) |
| Headless offscreen render | `platform/src/simulator.rs` (`WgpuDisplay::headless`, `headless_with_color_format`) |
| GUI Wrapper app state & update loop | `src/bin/creator_ui/app.rs` (`CreatorApp`) |
| Publish list & order | `scripts/publish_changed.sh` + `tests/publish_changed_matrix.rs` |
| Consumer recipe | `docs/CUSTOM-SIMULATOR.md` |
| Throwaway-project materialization pattern | `tests/bsp_esp32c3_compile.rs` family (compile-verify) |
| Playit-over-kittest executor | **owned by CRATES-CI-03/04** (this initiative); repo mirrors once landed |

One owner per concept; Consumer Projects cite these, never restate them.

## §5 Frozen decision — gate topology (Standards Action)

1. Two gates, **both** required for a conforming release: Gate P blocks the
   tag; Gate R verifies registry truth after publish and on a 3×-daily
   schedule (mirroring `publish-continue.yml` cadence).
2. Gate P consumes the full ordered publish list (`ordered_crates` in
   `scripts/publish_changed.sh`; 26 crates as of the 2026-07-12 SCTD-04
   amendment) — packaging
   every crate is part of the gate even when a Consumer Project only
   depends on a subset, because P-META/P-INCLUDE failures are per-crate.
   The list includes the upstream-shaped `ratatui-rlvgl` crate in the
   initialized `vendor/ratatui` submodule. Because that crate is intentionally
   excluded from the rlvgl workspace, Gate P packages it by manifest path
   immediately after `rlvgl-core`, its sole rlvgl dependency.
3. Consumer Projects MUST NOT use `path` dependencies or `[patch]` entries
   pointing into the workspace. Violation = nonconforming run.
4. The publish workflow (`publish.yml`) MUST require Gate P green on the
   release ref before `scripts/publish_changed.sh` executes.

## §6 Frozen decision — Staged Registry mechanism (Specification Required)

*(Amended 2026-06-10 during CRATES-CI-01 validation, per the original
§6.4 escape hatch. The initially drafted pure directory-source
replacement is unimplementable for Gate P: a replaced crates.io source
must satisfy ALL dependencies, and the not-yet-published rlvgl versions
cannot be resolved through it, while third-party deps would all need
vendoring. `[patch.crates-io]` injects candidates that do not exist
upstream — the standard pre-publish mechanism.)*

1. Gate P runs `cargo package --no-verify -p <crate>` in publish order
   (`scripts/crates_ci_stage.sh`), **progressively**: each packaged
   archive is extracted to `$STAGE_DIR/staged/<name>-<version>/` and
   appended to `$STAGE_DIR/patch-config.toml` (a `[patch.crates-io]`
   table) BEFORE the next crate packages, with the partial table fed to
   cargo via `--config`. This is required — `cargo package` re-resolves
   the sanitized manifest, and unpublished sibling versions are
   unresolvable until injected (publish time solves the same problem by
   actually publishing in order). Side effect: a publish-order topology
   error fails Gate P at the same crate that would have failed
   `cargo publish` (P-ORDER caught pre-tag). Consumer Projects consume
   the finished table as their `.cargo/config.toml`; third-party
   dependencies continue to resolve from crates.io.
   The excluded `ratatui-rlvgl` submodule crate uses the equivalent
   `cargo package --manifest-path vendor/ratatui/ratatui-rlvgl/Cargo.toml`;
   its version and archive location are discovered from that manifest's
   workspace rather than the rlvgl root workspace. Its staged consumer patch
   entry is appended after all root-workspace packages complete. The bridge is
   first packaged against extracted `rlvgl-core`; Gate P then defers both
   extracted patch entries while later root packages run because the live root
   already consumes the bridge from the submodule path and patches its own
   core. Exposing both live and extracted paths creates an ambiguous duplicate
   `rlvgl-core` source in Cargo's lockfile. No later publish crate depends on
   `ratatui-rlvgl`, and consumer gates receive both extracted entries after the
   package loop, so the deferral does not weaken publish-order or packaged-core
   verification. Gate P restores the submodule `Cargo.lock` after packaging so
   validation leaves the pinned gitlink clean.
   After installing the completed patch table, every Gate P Consumer Project
   MUST discard any `Cargo.lock` embedded by `cargo package` and resolve a fresh
   lock before building. Otherwise Cargo can retain an older registry version
   that still satisfies a caret requirement and mark the newer staged patch
   unused. Gate R retains registry-native lock behavior because it has no staged
   patch table.
2. The extracted package directories are NOT the workspace: they contain
   the normalized Cargo.toml and the packaged file set, so P-INCLUDE,
   P-META, and P-RESOLVE failures manifest exactly as they would for a
   crates.io download. INV-C2 forbids workspace paths, not staged
   package paths.
3. Known residual: patching does not exercise index/yank semantics or
   crates.io's server-side validation — that is Gate R's job; the split
   is intentional.
4. Asset-bearing crates mirror publish-time prep inside the staging
   script (`stm32_afdb_pipeline.sh` + force-add for rlvgl-chips-stm,
   `gen_ioc_bsps.sh` for rlvgl-bsps-stm), so the staged package contents
   match what `cargo publish` would upload.

## §7 Frozen decision — GUI Wrapper harness: playit over kittest (Standards Action)

1. The Kittest Engine is the **only** in-CI executor for the GUI Wrapper.
   No xvfb, no display server, on any CI job (INV-C5).
2. Two access layers share the one engine:
   - **Layer K (kittest-native)**: Rust `#[test]`s constructing `CreatorApp`
     directly — widget queries by accesskit label, wizard flows, snapshot
     tests. Lives with the root crate behind `creator_ui` (+ a
     test-only harness feature).
   - **Layer W (wire)**: `rlvgl-creator --automation-headless
     --playit-port=<n>` starts a TCP playit server; each verb executes
     against the Kittest Engine. Prints `PLAYIT_READY tcp://127.0.0.1:<port>`
     on stdout exactly like `rlvgl-disco-sim`, so `playit/node`'s client
     drives the GUI Wrapper unmodified.
3. Verb mapping (verb set frozen; mapping Specification Required):
   `QB:/QE:` → accesskit node lookup by label → bounds/existence;
   `T`/`PD/PM/PU` → kittest pointer injection; `KD:/KU:` → key injection;
   `D…` → snapshot pixel region dump in the existing hex framing;
   `?` → update-loop tick/frame counters. Verbs with no egui analogue
   (e.g. `C` star-crawl) return `ERR:unsupported` rather than being
   repurposed.
4. Widget addressing: playit tags ARE the egui/accesskit labels. Stable
   IDs needed for testing are added as labels in `creator_ui` code, not
   via a parallel tag registry.
5. `CreatorApp::new(manifest, path)` is already constructible outside
   `eframe::run_native` (`src/bin/creator_ui/mod.rs:160`). Automation mode
   MUST bypass the rfd `MessageDialog`/`FileDialog` pre-flight
   (`mod.rs:100-150`) — a missing manifest in automation mode is a startup
   error, never a dialog. All other rfd call sites must be unreachable or
   stubbed under automation (validated in CRATES-CI-03).
6. The eframe version skew is acknowledged and tolerated: GUI Wrapper on
   eframe 0.33.3 (root `Cargo.toml:249`), platform simulator on eframe
   0.27 (`platform/Cargo.toml:24`). `egui_kittest` is pinned to the GUI
   Wrapper's egui (0.33). Unifying the versions is out of scope (§11).

## §8 Frozen decision — User Sim Consumer Project contract (Standards Action)

1. A canonical Consumer Project `consumers/user-sim/` (workspace-detached)
   implements `docs/CUSTOM-SIMULATOR.md` verbatim shape: own resolution,
   own widget tree, registry deps only.
2. It MUST expose the same two headless modes as `rlvgl-disco-sim`:
   `--headless=<path>` frame dump and `--automation-headless
   --playit-port=<n>` with `PLAYIT_READY` handshake — proving the playit
   automation surface is reachable **from published crates**, not only
   from in-tree examples.
3. Its test suite: (a) builds against Staged Registry (Gate P) or crates.io
   (Gate R); (b) runs a `playit/node` script exercising tap → state-change
   → `QB`/`D` verification; (c) renders one golden PNG via
   `WgpuDisplay::headless` compared under threshold.
4. A second, minimal Consumer Project `consumers/creator-cli/` installs/
   builds `rlvgl` with `creator` and `creator,creator_ui` feature sets from
   the registry source and runs one CLI scan→convert round-trip plus the
   Layer-W handshake — this is the "GUI wrapper as built from crates" gate.

## §9 Invariant set

- **INV-C1** — No release tag without Gate P green on the same ref.
- **INV-C2** — Consumer Projects contain zero `path`/`[patch]` routes into
  the workspace (scanner-enforced in the Gate P workflow).
- **INV-C3** — Playit verb set is frozen vocabulary (Standards Action per
  `playit/README.md` ownership); the kittest executor maps or rejects,
  never extends, verbs without a playit-crate amendment first.
- **INV-C4** — Snapshot comparisons always carry an explicit threshold;
  bit-exact assertions are forbidden (GPU/driver variance).
- **INV-C5** — No CI job for this initiative requires a display server or
  xvfb. Software rasterization (lavapipe/llvmpipe) is permitted.
- **INV-C6** — `cargo package` artifacts consumed by Gate P are built from
  the exact release ref, never from a dirty tree (CI enforces clean
  checkout).
- **INV-C7** — Layer W's stdout handshake string `PLAYIT_READY tcp://…` is
  byte-compatible with `playit/node/src/index.js` expectations.

## §10 Reconciliation vs adjacent repo primitives

| Adjacent primitive | Relationship |
|---|---|
| `playit` crate / `playit/node` | Reused as-is (client + verbs). New executor backend only; `launchDiscoSim` generalizes to `launch(binPath, opts)` or gains a sibling — decided in CRATES-CI-04 with a playit-crate-side note, since `playit/node` is owned there. |
| `rlvgl-disco-sim --automation-headless` | Pattern source for handshake + flags; untouched. Remains the in-tree (non-crates) automation gate (Phase 4.5). |
| compile-verify tests (`tests/bsp_*_compile.rs`) | Materialize-throwaway-project pattern reused for Gate P harness code; Gate P differs in substituting the registry source instead of pointing at generated source files. |
| `scripts/publish_changed.sh` | Stays the publish authority. Gate P imports its ordered list (single source — read or generated, not copied). |
| `creator-e2e.yml` | Unchanged; covers in-tree CLI. Gate P's `consumers/creator-cli/` is the crates-built complement, not a replacement. |
| `docs/TEST-STRATEGY.md` | Gains tier entries for Gate P / Gate R / Layer K / Layer W once CRATES-CI-01..04 land. |

## §11 Non-goals

- Porting the GUI Wrapper to rlvgl's own widget tree.
- Unifying the eframe 0.27 (platform) / 0.33 (creator_ui) version skew.
- Headed (real-window) testing on macOS/Windows runners.
- Pixel-perfect cross-driver golden images.
- Replacing or wrapping crates.io with a self-hosted registry server.
- Testing `cargo install rlvgl` end-user ergonomics beyond what the
  Consumer Projects compile (may become a later phase by amendment).

## §12 Acceptance checklist (per phase)

- **CRATES-CI-00** (this doc): ratified §15 entry; `CLAUDE.md`
  applicability list and commit-prefix table amended.
- **CRATES-CI-01 — Gate P harness**: workflow `crates-ci.yml` packages all
  publishable crates from a clean ref, materializes the Staged Registry, and builds
  `consumers/creator-cli/` (CLI feature set only at this phase). Red on
  any P-META/P-INCLUDE/P-RESOLVE regression replayed from `9bee2f9`'s
  fontdue case as a fixture test.
- **CRATES-CI-02 — User Sim consumer**: `consumers/user-sim/` builds from
  Staged Registry; node script passes tap→verify; golden PNG within
  threshold; `--automation-headless` handshake verified by `playit/node`
  client unmodified (INV-C7).
- **CRATES-CI-03 — Kittest Layer K**: `CreatorApp` constructible under
  harness; rfd pre-flight bypassed in automation mode; ≥1 wizard flow +
  ≥1 snapshot test green in CI without display server (INV-C5).
- **CRATES-CI-04 — Playit Layer W**: `--automation-headless --playit-port`
  on `rlvgl-creator`; verb mapping per §7.3; driven by `playit/node`
  client; added to `crates-ci.yml` via `consumers/creator-cli/` with
  `creator_ui` features.
- **CRATES-CI-05 — Gate R**: post-publish + scheduled workflow building
  the same consumers against real crates.io; failure pages the repo via
  workflow failure (no silent skip).
- **CRATES-CI-06 — Retrospective**: `docs/crates-ci/CRATES-CI-RETROSPECTIVE.md`
  per the retrospective discipline.

## §13 Files cited

- `src/bin/rlvgl_creator/main.rs` (mode dispatch)
- `src/bin/creator_ui/mod.rs` (`run()`, rfd pre-flight, `CreatorApp::new`)
- `src/bin/creator_ui/app.rs`
- `platform/src/simulator.rs` (`WgpuDisplay::{new,run,headless,headless_with_color_format}`)
- `platform/Cargo.toml:21-24,59` (simulator feature, eframe 0.27)
- `Cargo.toml:249,401-434` (eframe 0.33.3, `creator_ui` feature)
- `scripts/publish_changed.sh:160-195`; `tests/publish_changed_matrix.rs`
- `playit/README.md`; `playit/node/src/index.js`; `playit/node/test/disco-sim.test.js`
- `docs/CUSTOM-SIMULATOR.md`; `docs/TEST-STRATEGY.md`; `OPTIONS.md`
- `.github/workflows/{ci,creator-e2e,publish,publish-continue}.yml`
- Evidence commits: `58bb737`, `bc69338`, `4d9255a`, `9bee2f9`, `5e38de4`, `8dc2876`

## §14 Unblocks

- Tagging v0.2.2+ with pre-verified packaging (ends the
  tag→publish→break→fix-commit loop of §2).
- The contractee crates-first distribution story: a bare Linux box with
  only crates.io access provably builds the creator CLI, the GUI wrapper,
  and a user-authored simulator.
- Future GUI Wrapper feature work (`docs/creator/UI-DESIGN.md` checklist)
  gains a regression harness before, not after, the features land.

## §15 Change log

- **2026-06-10** — Initial draft (CRATES-CI-00). Gate topology (§5),
  Staged Registry mechanism (§6), playit-over-kittest harness (§7),
  Consumer Project contract (§8) frozen pending ratification. Decisions
  taken with owner input: both gates; playit-over-kittest + kittest-native
  layers; new `docs/crates-ci/` family. **Status: DRAFT — awaiting
  ratification.**
- **2026-06-10** — **RATIFIED** by owner. CLAUDE.md applicability list and
  commit-prefix table amended in the ratification commit. Execution begins
  with CRATES-CI-01.
- **2026-06-10** — §6 amended during CRATES-CI-01 validation (per §6.4):
  pure directory-source replacement is unimplementable pre-publish
  (unpublished versions unresolvable; all third-party deps would need
  vendoring); mechanism is progressive `cargo package` + extract +
  `[patch.crates-io]` via `--config`. Findings during bring-up:
  (a) `rlvgl-audio-meters-core` was `publish = true`, depended on by
  `rlvgl-widgets`, but absent from the publish order and the matrix
  test — added to both; matrix test gained a cargo-metadata-derived
  completeness check so the omission class cannot recur. (b)
  `platform/src/blit.rs` ELF `link_section` broke all Mach-O host
  builds of rlvgl-platform (H-SIM-adjacent, crates-first story);
  gated to `target_os = "none"`.
- **2026-06-10** — CRATES-CI-02 landed: `consumers/user-sim/` (custom
  simulator per `docs/CUSTOM-SIMULATOR.md` from packaged crates; playit
  TCP handshake verified byte-compatible with the unmodified
  `playit/node` client per INV-C7; in-binary golden-PNG check at
  threshold 3/255 per INV-C4; node tap→pixel-diff test). Finding
  (P-INCLUDE, the gate's purpose): `rlvgl --features simulator` was
  unbuildable from packaged crates — `rlvgl-app-disco-demo` referenced
  10 `.rle` icons OUTSIDE its crate root via `include_bytes!`, so no
  packaged build of the umbrella simulator feature (including the
  published 0.2.1) could ever compile. Fixed as CRATES-CI-02a: icons
  vendored into `examples/apps/disco-demo/assets/`, umbrella
  `--features simulator` build added to the creator-cli smoke. Known
  residual: `disco-assets`'s `embed` feature has the same
  out-of-crate-root pattern (`$CARGO_MANIFEST_DIR/../media/...`) —
  feature-gated, unused by the simulator path; needs its own repair
  before any consumer enables `disco-assets/embed` from the registry.
  Deviation from §8.1 recorded: user-sim consumes
  `rlvgl-core`/`rlvgl-widgets`/`rlvgl-platform(simulator,fontdue)`/`rlvgl-playit(std)`
  directly rather than the umbrella `rlvgl` crate (more user-realistic;
  the umbrella path is covered by creator-cli per CRATES-CI-02a; the
  root crate also does not re-export `rlvgl-playit`, and platform's
  `draw_text` is a stub without `fontdue`).
- **2026-06-10** — CRATES-CI-03 landed: Layer K via
  `egui_kittest 0.33.3` (dev-dep; `eframe`+`snapshot`+`wgpu` features,
  `Harness::build_eframe` drives the unmodified `eframe::App` impl —
  the §7.5 "update→ui refactor" proved unnecessary; only `pub(crate)`
  visibility changes in `src/bin/creator_ui/mod.rs`).
  `tests/creator_ui_kittest.rs` (gated `required-features =
  ["creator", "creator_ui"]`): boot/menu-tree assertions, Build→Fonts
  Pack wizard flow, and `creator_ui_boot` wgpu snapshot at explicit
  threshold 0.8 (INV-C4), all headless (INV-C5). §7.5's rfd pre-flight
  bypass confirmed already satisfied: `CreatorApp::new` is rfd-free;
  the dialogs live only in `run()`. Layer W (TCP playit server) remains
  CRATES-CI-04.
- **2026-06-10** — CRATES-CI-04 landed: Layer W. New root-crate feature
  `creator_ui_automation = ["creator_ui", "dep:egui_kittest",
  "dep:rlvgl-playit", "rlvgl-playit/std"]`; `rlvgl-creator
  --automation-headless --playit-port=<n>` (detected before CLI
  routing; missing manifest is a startup error per §7.5) hosts
  CreatorApp in a kittest harness and serves the playit verb set over
  loopback TCP with the `PLAYIT_READY` handshake (INV-C7).
  INV-C3 satisfied by construction: the wire codec is reused from
  `rlvgl_playit::protocol` (`parse_command`/`format_response`), the
  executor only maps or rejects — `QB:/QE:` → accesskit label lookup
  (§7.4 tags-are-labels), `T`/`T@`/`PD/PM/PU` → kittest pointer,
  `KD:/KU:` → egui keys, `?` → step counters, `D` → wgpu region dump
  in playit hex framing with documented `ERR: render-unavailable`
  degradation when no GPU adapter exists (CI containers; accesskit
  verbs unaffected), `MT`/`RS/RE/RD`/`C` → `ERR: unsupported`.
  creator-cli smoke now builds
  `creator,creator_ui,creator_ui_automation` from the staged package —
  the §12 "creator_ui from packaged crates" proof — and runs
  `consumers/creator-cli/test/creator-ui.test.js` through the
  unmodified `playit/node` client.
- **2026-06-10** — CRATES-CI-05 landed: Gate R.
  `.github/workflows/gate-r.yml` (workflow_run after "Publish" success
  + daily 16:30 UTC schedule offset from publish-continue + dispatch)
  runs `GATE=r` lib-smoke and user-sim plus
  `consumers/creator-cli/gate_r.sh` — the literal end-user path
  (`cargo install rlvgl --features creator` from crates.io, then the
  smoke round-trip). No `continue-on-error` anywhere (§12 no-silent-
  skip). KNOWN RED until v0.2.2 publishes: "0.2" resolves to the
  broken 0.2.1 simulator path (see CRATES-CI-02a entry); the red is
  the truthful registry state and turns green at the next publish.
- **2026-06-10** — CRATES-CI-01a: two Gate P blind spots closed after
  the first real publish attempt. (a) **P-META hard gate**:
  `cargo package` only WARNS on missing `description`/`license` but
  `cargo publish` hard-fails — disco-assets reached the publish step
  with no `license` field. The staging script now promotes that
  warning subset to errors. (b) **Version-drift gate**: a crate whose
  version already exists on crates.io with DIFFERENT content is
  silently skipped by the publish script's already-published check,
  shipping stale code. Staging now compares each packaged archive
  against the published one (member-content compare, ignoring
  `.cargo_vcs_info.json` / normalized `Cargo.toml` / `Cargo.lock`)
  and fails listing every drifted crate. First audit found real drift
  in `rlvgl-micropython` 0.2.0 (bumped to 0.2.2) and disco-assets
  0.2.0 (metadata fixed + bumped to 0.2.2). `CRATES_CI_OFFLINE=1`
  skips drift checks for airgapped iteration.
- **2026-06-10** — CRATES-CI-01a amendment: the two asset-bearing
  crates (`rlvgl-chips-stm`, `rlvgl-bsps-stm`) are PERMANENTLY exempt
  from the drift gate (loud log line, never silent): their packaged
  content embeds pipeline-regenerated artifacts that are not
  byte-reproducible run-to-run — verified by a full-prep repro where
  ONLY `assets/chipdb.bin.zst` differed on an otherwise untouched
  crate. The first CI execution of the drift gate (publish
  workflow_dispatch runs from main) failed on exactly this false
  positive; local validation had masked it because `SKIP_ASSET_PREP=1`
  skipped those crates' checks. Their source-change → publish
  discipline rides the path mappings in `publish_changed.sh`.
- **2026-06-10** — CRATES-CI-06: initiative retrospective drafted at
  completion (`CRATES-CI-RETROSPECTIVE.md`). Acceptance event: v0.2.2
  published through Gate P and verified green by Gate R against live
  crates.io the same day. The retrospective's §6 forward constraints
  (FC-1…FC-6) are normative for future initiatives. Initiative CLOSED;
  further work proceeds by §15 amendment + sub-letter phases.
- **2026-06-10** — Post-close amendment (owner request): pre-publish
  gauntlet (CLAUDE.md, the maintained phase list — no separate
  /pre-publish skill file exists) gains **Phase 6.5**: cheap
  `cargo check -p rlvgl-platform --features linux_fbdev` on
  `aarch64-unknown-linux-gnu` and `armv7-unknown-linux-gnueabihf`
  (BBB prong). Same defect class as the Mach-O `link_section` break
  (§15 CRATES-CI-01 entry, retrospective divergence 3): host/arch
  coverage gaps in published crates are invisible to x86-Linux-only
  CI. Prerequisite documented (`rustup target add …`; check-only, no
  cross-linker). Validated green on both targets before recording
  (FC-1). Note: requested feature spelling `linux_evdev` does not
  exist — `linux_fbdev` gates both the fbdev and evdev modules
  (`platform/src/lib.rs:76-79`).
- **2026-07-12** — SCTD-04 release amendment: Gate P and the publish chain
  now include the first `ratatui-rlvgl` release. CI initializes the Ratatui
  submodule; the excluded crate is versioned, packaged, staged, drift-checked,
  and published by manifest path immediately after `rlvgl-core`. The ordered
  release set is now 26 crates. The later PR to the Ratatui project is a
  separate upstream action and is not a prerequisite for publishing the
  bridge from the pinned SoftOboros contribution branch.
- **2026-07-12** — SCTD-04 Gate P validation amendment: packaging the bridge
  against staged `rlvgl-core` is retained at its dependency-ordered position,
  then both its and extracted `rlvgl-core`'s `[patch.crates-io]` consumer entries
  are deferred until the root package loop completes. This prevents the live
  submodule/core paths and their extracted counterparts from producing an
  ambiguous duplicate-core lockfile source. Consumer gates still receive both
  extracted entries. The submodule lockfile is backed up and restored around
  packaging.
- **2026-07-12** — Gate P host-target amendment: the creator-cli and user-sim
  harnesses locate debug binaries in either Cargo's default `target/debug`
  layout or `target/<rustc-host>/debug` when a repository/user Cargo config
  sets `build.target`. This makes local macOS release validation exercise the
  same staged consumers as Linux CI without changing CI's artifact path.
- **2026-07-12** — Gate P packaged-lock amendment: every Gate P consumer now
  discards its pre-patch lockfile before resolution. The creator consumer had
  retained registry `rlvgl-core 0.2.4` from the root package lock even after
  staged `rlvgl-core 0.2.5` was installed in `[patch.crates-io]`; Cargo therefore
  compiled the registry crate and correctly failed the provenance assertion.
