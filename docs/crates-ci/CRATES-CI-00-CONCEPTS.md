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
   tag; Gate R verifies registry truth after publish and on
   the established daily verification schedule. Publishing has no timed trigger.
2. Gate P consumes the full ordered publish list (`ordered_crates` in
   `scripts/publish_changed.sh`; 26 crates as of the 2026-07-12 SCTD-04
   amendment) — packaging
   every crate is part of the gate even when a Consumer Project only
   depends on a subset, because P-META/P-INCLUDE failures are per-crate.
   The current list contains all 31 existing publishable crates. It includes
   the RLVGL-owned `ratatui-rlvgl` root workspace member at
   `crates/ratatui-rlvgl`. Gate P packages it immediately after `rlvgl-core`,
   its sole RLVGL dependency. The official Ratatui 0.30.2 source remains
   unmodified; its native workspace supplies only the finite preparation
   context described in §6 and §15 0.1.7.
3. Consumer Projects MUST NOT use `path` dependencies or `[patch]` entries
   pointing into the workspace. Violation = nonconforming run.
4. Both publishing workflows (`publish.yml` and `publish-continue.yml`) MUST
   require Gate P green on the exact frozen release commit before the publisher
   executes. Continuation is manual-only: resolve the latest release tag and
   previous tag to immutable commit SHAs, then use the same release SHA for
   packaging/consumers and publication. Publication MUST depend on successful
   Gate P, verify exact HEAD and a clean source checkout before authentication,
   and retain all three existing packaged consumers. A newer maintenance helper
   MAY be preserved from the workflow revision only when its identity is recorded
   separately and it executes against the frozen release sources. Changes to crate
   sources require a newly qualified candidate. The continuation workflow MUST
   NOT have a scheduled publishing trigger; registry throttling requires explicit
   operator retry after the reported reset time. Gate R's daily registry-truth
   verification remains independent of publication.
5. Crates.io publication MUST authenticate through Trusted Publishing, not a
   repository-stored long-lived registry token. Every publishing job MUST use
   the GitHub `release` Environment, grant only `contents: read` and
   `id-token: write`, exchange its OIDC identity through
   `rust-lang/crates-io-auth-action`, and pass that step's short-lived token to
   Cargo. Each owned crate MUST trust both `publish.yml` and
   `publish-continue.yml`, because either workflow can execute the same
   dependency-ordered publisher. A brand-new crate MUST receive its first
   publish manually before these configurations can be created; after that
   bootstrap it MUST join both Trusted Publisher configurations before the
   automated chain resumes.

6. Owner-accepted PCDN-15 supersedes the permanent dated qualification hold
   in 0.1.13 and the pending-policy clause in 0.1.14. Main SHALL build daily
   against the newest official nightly to detect drift. Every release SHALL
   pass all existing source/pre-publish and exact-ref Gate P guards against
   the most recent nightly. Resolve the official latest channel manifest once
   into an immutable dated cohort per workflow, and record its full-body hash,
   date and actual compiler/Cargo/component/target identities. Require all
   existing components and targets; missing inputs fail rather than silently
   falling back to an older nightly. Reconfirm newest identity at release
   preflight; an advanced cohort invalidates compiler qualification and requires
   requalification. Daily registry verification is separate from daily main
   compilation. Keep actual failures visible and every strict guard intact;
   no scheduled publication is admitted. See §15 amendment 0.1.15.

## §6 Frozen decision — Staged Registry mechanism (Specification Required)

*(Amended 2026-06-10 during CRATES-CI-01 validation, per the original
§6.4 escape hatch. The initially drafted pure directory-source
replacement is unimplementable for Gate P: a replaced crates.io source
must satisfy ALL dependencies, and the not-yet-published rlvgl versions
cannot be resolved through it, while third-party deps would all need
vendoring. `[patch.crates-io]` injects candidates that do not exist
upstream — the standard pre-publish mechanism.)*

1. Gate P packages in publish order (`scripts/crates_ci_stage.sh`), using
   workspace-detached temporary inputs that preserve each original manifest
   and every actual Cargo-selected payload byte. Cargo owns inventory,
   normalization and archive generation. Each archive is immediately extracted
   to `$STAGE_DIR/staged/<name>-<version>/` and its entry is appended to the
   progressive `$STAGE_DIR/patch-config.toml` BEFORE the next package runs.
   Every previously packaged dependency, including core and the owned
   Ratatui bridge, MUST resolve through its extracted package; no deferral,
   source fallback, fabricated manifest, dependency-minimum change, or manual
   archive/lock editing is permitted. Missing extracted predecessors fail.

   Cargo MAY load a separately recorded source graph-admission input for genuine
   path-only dev dependencies removed by Cargo normalization and their necessary
   real path closure. Every admitted file MUST come from its actual package's
   Cargo source inventory at the same frozen candidate, including original
   manifests and actual targets. Missing inventory files fail. An actual graph
   back-edge MAY point to the current package's input; previously packaged
   predecessors MUST still route to extracted archives. This permission only
   admits Cargo's pre-normalization graph; it does not admit source paths into
   normalized package verification or final consumers.

   Solely for Cargo archive preparation, the real RLVGL `Cargo.toml` workspace
   and nested `vendor/ratatui/Cargo.toml` workspace MAY additionally supply their
   actual ancestor descriptors and finite member context. Before execution,
   enumerate and fingerprint their original descriptor/member manifests,
   Cargo-selected payload inventories, actual targets and any inherited real
   shared readme/license paths. Preserve those bytes; Cargo owns interpretation
   of inherited values, inventory, normalization and generated members. The
   current bounded context is the root's 48 metadata members and Ratatui's 42
   metadata members, recorded by exact manifest path in
   `scripts/crates_ci_workspace_contexts.json`, superseding the previous
   47/43 inventories under the accepted migration in §15 0.1.7. Only the
   already-enumerated platform manifest fingerprint additionally changes for
   its accepted version allocation under §15 0.1.8. Under §15
   0.1.6, the root
   `rlvgl` member additionally admits only the integration-test target
   `tests/bsp_clock_ownership.rs`, fingerprinted by SHA256
   `02e67e0539fa9f86d41d2d4f349deca56c6bc0a1513950c9759a24a0925d153e`.
   Other workspace descriptors, new members/targets or additional source
   requirements MUST stop for an owning decision rather than recursively
   broadening admission. Previously
   packaged predecessors MUST still resolve through their extracted archives.
   Keep actual nested Git identity and dirty-overlay provenance; the outer
   repository identity MUST NOT replace nested Ratatui provenance. Checkout
   lock preservation includes retaining absence when no source lock exists.
   All additional admission inputs MUST become unavailable before normalized
   package verification and final consumers.

   Solely when preparing the owned `ratatui-rlvgl` package at
   `crates/ratatui-rlvgl/Cargo.toml`, its existing normal dependency
   `ratatui-core` with exact requirement `=0.1.2`, `default-features = false`
   and resolved original manifest `vendor/ratatui/ratatui-core/Cargo.toml`
   MAY activate and record the already-admitted finite native workspace context
   before adding that graph prerequisite. This permission applies only to that
   exact edge under §15 0.1.14; it adds no descriptor, member, target, source,
   policy fingerprint or publish-list entry. Preserve all existing descriptor,
   member, target and shared-file fingerprints, original Cargo inventories,
   actual nested Git provenance and checkout locks or their absence. Native
   admission inputs MUST become unavailable before normalized verification and
   consumers. Cargo-generated normalized metadata MUST resolve native
   `ratatui-core =0.1.2` from the registry, with no native path or additional
   native patch. Extracted owned predecessors remain mandatory. Every other
   unadmitted normal path edge MUST still fail; no generic activation or
   automatic context widening is granted.

   The extracted package's resolved metadata MUST contain local paths only for
   itself and its extracted staged predecessors; other packages MUST have registry
   sources. Any source admission/original-checkout route in that graph fails.
   Verification MUST still resolve after admission inputs become unavailable.
   Preserve and compare the source inventory and original manifest, record
   Cargo's normalized manifest, packaged lock and VCS provenance, and retain
   complete generated-member accounting separately from payload equality.
   Temporary input-graph locks may be changed only by Cargo. Checkout locks,
   including nested locks, MUST remain unchanged on success and failure.

   The owned `ratatui-rlvgl` is discovered exactly once by root workspace
   metadata and packaged at its dependency-ordered position against extracted core. Both
   extracted entries remain available throughout later packaging. The completed
   patch table points only into extracted archives and is consumed by all
   existing Consumer Projects. An unpublished predecessor/topology error remains
   a P-ORDER/P-RESOLVE failure; a subset diagnostic is not full Gate P.
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
- **2026-07-13** — Trusted Publishing amendment (owner direction): the 403 on
  `rlvgl-chips-silabs 0.2.1` showed that the repository token path was no
  longer an accepted release authority. §5 now requires GitHub OIDC exchange
  through `rust-lang/crates-io-auth-action`, the `release` Environment, and
  per-crate trust for both publisher workflow filenames. The first
  `ratatui-rlvgl` release remains a one-time manual bootstrap because crates.io
  cannot configure a Trusted Publisher before a crate exists. The same change
  quotes the workflow-supplied base revision so an empty/manual `*` value
  cannot expand to a repository filename.

### 0.1.0 — 2026-10-05 — drafted

**Author:** Codex
**Change kind:** semantic
**Touches:** §6, INV-C1, INV-C2, INV-C6
**Summary:** Records ERRATA-001 and proposes CRATES-CI-01b isolated packaging; §6 remains unchanged pending exact mechanism disposition.

#### Rationale

The core/bridge deferral hides an unpublished predecessor needed by later
packages, while restoring it in the live root produces duplicate source
identities. [The drafted correction](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/CRATES-CI-01B-STAGING-ISOLATION.md)
specifies archive/manifest equivalence, predecessor negative controls and
complete consumer proof. [ERRATA-001](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/ERRATA.md) remains pending ratification.
Considered and rejected: reducing dependency minima, live-source fallback,
or counting a partial run as Gate P. The effective mechanism, clean-source
rule, metadata/drift checks, release gate and owner decision remain unchanged.

### 0.1.1 — 2026-10-05 — drafted

**Author:** Codex
**Change kind:** semantic
**Touches:** §5, INV-C1, INV-C6
**Summary:** Records ERRATA-002 and proposes frozen-source, same-ref Gate P for continuation; effective workflow and §5 remain unchanged.

#### Rationale

The continuation resolves a release tag but executes its publisher against the
workflow revision without a Gate P dependency. The
[concrete review packet](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-continuation-wave3.md) separates helper
maintenance from package-source qualification and retains all consumers/OIDC
requirements. [ERRATA-002](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/ERRATA.md) awaits owning disposition. A static/local
fixture pass does not replace full Gate P or provider execution. Updating only
the helper while packaging unqualified later crate sources was rejected as a
solution to the owner's one-candidate requirement.

### 0.1.2 — 2026-10-06 — ratified

**Author:** Codex, recording owner direction
**Change kind:** semantic
**Touches:** §5, INV-C1, INV-C6
**Summary:** Owner accepts frozen-source same-ref Gate P for continuation and removes its timed publishing trigger.

#### Rationale

The owner agreed to decision 1 and directed that timed publishing runs are no
longer needed. This ratifies the source/gate correction proposed in 0.1.1,
adds manual-only continuation, and preserves separate helper identity. The
publisher has no local five-crate cutoff; this amendment does not assume removal
of registry-side rate limits. Existing HTTP 429 diagnostics and indexing waits
remain. Gate R's existing daily verification schedule remains independent, and its
post-publish trigger follows successful primary and continuation runs.
[The decision receipt](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-continuation-decision.md) records the
implementation and bounded checks. ERRATA-002's owner question is settled;
provider execution and full Gate P remain verification gates. ERRATA-001 and
its staging-mechanism decision are unchanged.

### 0.1.3 — 2026-10-06 — drafted

**Author:** Codex, recording owner direction
**Change kind:** semantic
**Touches:** §6, INV-C1, INV-C2, INV-C6
**Summary:** Owner accepts detached-packaging direction with original manifests and Cargo-owned normalization; exact lock mechanism awaits equivalence and final ratification.

#### Rationale

Decision 2 was accepted with its stated remaining lock-resolution condition.
[CRATES-CI-01b](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/CRATES-CI-01B-STAGING-ISOLATION.md) records those accepted
constraints. [The follow-up receipt](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-staging-decision.md)
records the bounded experiment. No dependency minimum, live-source fallback,
archive member or failed lock comparison is waived. Effective §6 remains
unchanged until the detailed mechanism is ready for final ratification.

### 0.1.4 — 2026-10-10 — amended

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §6
**Summary:** Owner makes bounded real source graph admission effective before detached packaging; extracted-only verification, predecessor routing and all release gates remain required.

#### Rationale

The owner accepted recommendations 1–6 on 2026-10-10 and authorized parallel
implementation. This adopts the exact admission boundary proposed in
[the October 7 mechanism packet](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-staging-mechanism.md) for
[ERRATA-001](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/ERRATA.md). The historical core trial separated Cargo's loading of
real path-only dev dependencies from its normalized package graph; its all-member
comparison and missing extracted-API control are recorded in
[the historical receipt](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-packaging-wave4.md). Those transcript-
recovered observations are historical evidence, not fresh qualification of the
restored candidate or recovered raw archives.

Considered and rejected: suppressing extracted core/bridge entries, replacing
real dependency manifests, lowering version minima, removing a source lock to
make a trial succeed, and allowing consumer paths into the source admission
graph. Each hides the original resolution or distribution defect. Cargo retains
ownership of inventory, normalization, archive generation and external input-
graph lock updates. Original manifests and checkout locks remain preserved.
This changes §6's deferral mechanism, not INV-C1, INV-C2 or INV-C6, the release
set/order, selective versions, asset preparation, metadata/content-drift checks,
clean-source refusal, the three consumers, provider CI or publication authority.
Implementation is authorized against this ratified concepts amendment. The
[01B execution plan](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/crates-ci/CRATES-CI-01B-STAGING-ISOLATION.md) remains Draft with its
remaining acceptance gates open; this entry is not its ratification or Gate P.

### 0.1.5 — 2026-10-10 — amended

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §6
**Summary:** Owner authorizes the enumerated real root and nested Ratatui descriptor/member contexts solely for Cargo archive preparation, preserving extracted predecessors and admission-free verification/consumers.

#### Rationale

The owner accepted both latest recommendations on October 10 local time after the bounded
execution stopped on Ratatui's inherited descriptor. The exact frontier and two
dispositions are recorded in [that receipt](https://github.com/SoftOboros/rlvgl/blob/a2b48e108ae9e0623d2bb6217cef73f0113e8708/docs/releases/v0.2.7-staging-20261010.md).
This amendment is recorded before governing implementation. Root and nested
Ratatui are the only additional workspace contexts authorized: their actual
descriptor bytes and finite actual member/target contexts must be enumerated
before preparation. Cargo, rather than fabricated ancestors or flattened
inheritance, interprets those inputs and generates normalized archives. Any
unexpected workspace or additional source requirement stops for a new owning
decision; this is not recursive workspace admission.

Considered and rejected: continuing the previous fail-closed diagnostic as if
full staging passed, omitting bridge/root from the publishable set, inventing an
ancestor, rewriting inherited values, bypassing a predecessor through original
source or registry, and assigning the outer AA Git identity to nested ED source.
The selected disposition preserves real manifests, payload inventories/targets,
actual nested Git provenance including dirty overlays, and every checkout lock's
bytes or absence. Source admission is solely for archive preparation, must become
unavailable before normalized verification and consumers, and does not waive
clean-source, asset, metadata/drift, full-set, three-consumer or Linux CI gates.
The 01B plan remains Draft. A frozen hashed uncommitted candidate permits local
diagnostics; it is not a clean release ref or authorization to create a source
commit before the remaining CLAUDE prepublish gates pass.

### 0.1.6 — 2026-10-11 — amended

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §6
**Summary:** Owner admits only the already approved BSP clock-ownership regression target into the finite root archive-preparation context.

#### Rationale

The owner accepted the one-target staging recommendation on October 11 local
time after the unchanged workspace guard refused the new Cargo-discovered
integration test. This amendment is recorded before governing implementation.
Only `tests/bsp_clock_ownership.rs` in the root `rlvgl` member is additionally
admitted, with SHA256
`02e67e0539fa9f86d41d2d4f349deca56c6bc0a1513950c9759a24a0925d153e`.
It supplies the separately approved BSP null-owner regression coverage.
The actual root and nested Ratatui descriptor bytes, original manifest/shared
file fingerprints, and 47/43 member sets remain unchanged. Governing finite
context implementation SHALL enumerate only this additional target. Every
other unexpected descriptor, member, target or source requirement remains an
owning-decision stop.

Considered and rejected: removing or hiding the approved regression test,
automatically admitting future Cargo-discovered targets, or disabling the
unchanged fail-closed target guard. Those bypass the finite admission boundary.
Original Cargo inventories and normalization, progressive extracted
predecessors, withdrawal of admission inputs before normalized verification
and all consumers, genuine nested Git/overlay provenance, checkout lock bytes
or absence, clean release-ref, metadata/drift, normal asset preparation,
full ordered publish set, all three consumers and Linux same-ref/provider
gates remain required. Root/core allocation remains 0.2.7; this amendment does
not ratify the Draft 01B execution plan or establish Gate P/publication readiness.

### 0.1.7 — 2026-10-12 — amended

**Author:** Codex, recording I. Abbott's acceptance
**Change kind:** semantic
**Touches:** §5, §6
**Summary:** Records the accepted RLVGL-owned bridge migration and its exact
finite archive-preparation contexts; all release guards remain required.

#### Rationale

The owner accepted PCDN-07 and the migration plan, then authorized the owned
bridge/fonts, compatibility and Stage/publisher integration slices. This entry
records that accepted scope before adoption through protected PRs. It supersedes
the current nested-bridge discovery and 47/43 context allocation; earlier dated
entries retain their historical scope.

`ratatui-rlvgl` is the regular root workspace member at
`crates/ratatui-rlvgl/Cargo.toml`, discovered exactly once through root metadata.
Its archive includes the four existing packed font variants and their notices.
The official Ratatui 0.30.2 source is pinned, unmodified, to commit
`e665c36cb14752a61cd777fbd06dbef8474f2add`, tree
`cc2e7266ebf5e17f9250ef2d7bab1222dc62150f`. Its native core retains its genuine
nested workspace and Git provenance; that context does not add another bridge
or native crate to the RLVGL publish list.

Actual locked Cargo metadata enumerates exactly 48 root members and 42 native
members. `scripts/crates_ci_workspace_contexts.json` fingerprints their actual
descriptors, member manifests, targets and inherited shared files; its accepted
inventory SHA256 is
`c757a5affa3761214abbddcc1cf9aa14d01a10c9c1232808cf39585a5a9a9f1c`.
The root descriptor SHA256 is
`ddb1ee0b32907519e0bf804b158e2494090e1b14b634868b0efb4f63776a2c3e`;
the native descriptor SHA256 is
`a5007fe4cb0d9901302c5eb201941dcec82755370917144e312d2bfe9e5a6567`.
These are finite inventories for this candidate, not permission to discover
and admit future descriptors, members, targets or sources automatically.

The existing 31-crate order is unchanged, with core immediately before the
bridge. Selective candidate versions are bridge 0.1.2 and root 0.2.8; core
remains 0.2.7 and the other published packages retain their existing versions.
Released A2/v0.2.7 and original parent pins remain unchanged. The future release
identity requires separate admission.

Progressive extracted predecessors, absence of source fallback, withdrawal of
preparation inputs before normalized verification and consumers, actual nested
Git/dirty-overlay provenance, checkout lock bytes or absence, clean-source
refusal, metadata/content drift, normal asset preparation, the full ordered set,
all three existing consumers, full source validation and Linux same-ref/provider
gates remain required. Existing failed results remain open; this amendment does
not ratify Draft 01B, waive a guard, qualify Gate P or authorize publication.
The governing amendment follows the protected amendment-first PR route before
behavior adoption. Any unexpected requirement stops for an owning decision.

### 0.1.8 — 2026-10-12 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-08
**Change kind:** semantic
**Touches:** §6 finite context and selective-version boundary
**Summary:** Owner admits the existing platform member's version fingerprint
for the exact scanout diagnostic correction, without changing context membership.

#### Rationale

The owner accepted the named snapshot correction in SCTD-04 §15 0.1.1 and its
mandatory `rlvgl-platform` 0.2.6 to 0.2.7 allocation. The snapshot is shipped in
the immutable published 0.2.6 archive; changing it at that version would violate
P-DIFF. Only `platform/Cargo.toml`'s already-enumerated member fingerprint may
change in `scripts/crates_ci_workspace_contexts.json`, computed from the actual
candidate bytes and verified against locked Cargo metadata before execution.
Record and freeze the resulting policy identity. Root/native descriptor bytes,
48/42 member sets, targets and inherited shared files remain unchanged.
Prior dated fingerprints and failed results retain their historical scope.

The actual platform manifest SHA256 for this allocation is
`27ac01c83570036e437abd7f9b60eb74f1bba0358c568f06a634a4918ff07e61`.
The resulting finite-context policy SHA256 is
`460242ed77e45a8115dac0474f6a6e6d83170257843f23ccd5dbfbeb81aa8a1c`;
locked root/native metadata verifies the unchanged 48/42 inventories.

Cargo may regenerate only the necessary platform-version entries in the root
and IDF candidate locks before they are frozen for validation; no unrelated
dependency-resolution churn or manual lock editing is admitted. All other six
active locks, including official Ratatui's, remain exact. Bridge 0.1.2 and root
0.2.8 retain their accepted allocation; core stays 0.2.7 and other published
versions and dependency minima remain unchanged.

No additional descriptor/member/target, source fallback, diagnostic normalization,
fixture refresh or guard waiver is granted. The full 31-crate order, extracted
predecessors, unavailable preparation inputs before verification/consumers,
actual nested provenance, runtime source-lock preservation, clean-source,
metadata/drift, full source validation, same-ref/protected PR, Linux Gate P,
all consumers and publication/Gate R gates remain required. This amendment is
recorded before the correction; adoption uses the established protected
amendment-first route. It does not establish a passing gate or authorize a
release identity, publication or parent-pin change. Stop for any new PCDN.

### 0.1.9 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-09
**Change kind:** semantic
**Touches:** §5 source-adoption prerequisites and owning Markdown-link instruction
**Summary:** Owner admits a finite changed-document link result for this owned
Ratatui maintenance candidate, retaining inherited whole-tree failures as open.

#### Rationale

The owner accepted PCDN-09 after the unchanged `scripts/check-links.sh` returned
exit 1, without a timeout: 9,535 checks, 6,429 unique checks, 9,270 successful
checks and 255 errors across 105 inputs. All failed input bodies are byte exact
their genuine current owner HEAD: 24 unchanged A2 root inputs, one chips-child
input, 68 vendor inputs and 12 tracked Ruby-bundle inputs. The actual failed
receipt, error inventory, source identities and earlier negative results SHALL
remain retained as an OPEN documentation follow-up. The separate four-document
diagnostic passed all 23 checks; it did not qualify the failed whole-tree guard.

This admission applies only to the current RLVGL-owned Ratatui migration and
PCDN-08 candidate, anchored to its prior 44-path source freeze SHA256
`65daac9de90e4549692372fb89cd197c7b868150c13f2c7950638b6e29df6169`.
Before documentation adoption through protected PRs, freshly run the unchanged
`lychee --no-progress` executable on every document in this exact finite set:

- `AGENTS.md`, including its owning candidate-specific crossreference;
- `docs/concepts/RATATUI-00-CONCEPTS.md`;
- `docs/concepts/SCTD-04-RATATUI-RLVGL-INTEGRATION.md`;
- `docs/crates-ci/CRATES-CI-00-CONCEPTS.md`, including this amendment;
- `crates/ratatui-rlvgl/README.md`.

All five documents SHALL pass actual checks with no added filters, exclusions,
fabricated responses or ignored failures. Freeze their actual hashes and modes
and retain the actual command and receipt. Any additional Markdown edit or new
error is an owning-decision stop. Regenerate affected spec-index projections
with the unchanged canonical generator; do not edit generated files by hand.
The existing checker script remains unchanged, and its inherited whole-tree
result remains FAIL rather than an all-green receipt. Outside this exact
candidate-specific admission, the normal whole-tree instruction still applies.

Official Ratatui 0.30.2 remains unmodified at `e665c36cb14752a61cd777fbd06dbef8474f2add`.
Released A2/v0.2.7 and original parent pins remain unchanged. Selective versions
remain bridge 0.1.2, platform 0.2.7 and root 0.2.8; core remains 0.2.7, with no
other package allocation, dependency, source, fixture, API or compiler change.
The full source and remaining BSP/C3 checks, clean-source refusal, P-VCS,
P-META, P-SOURCE-LOCK, P-DIFF, the full 31-crate order, extracted predecessors,
withdrawal of preparation inputs before verification and consumers, genuine
nested provenance, all three consumers and the owned bridge boundary remain
required. No source fallback or automatic descriptor/member/target admission
is granted. Protected amendment-first source PRs, same-ref clean Linux Gate P,
separately admitted release identity/publication/Gate R and parent adoption
remain distinct gates. This amendment does not qualify those gates or close
the inherited link failures. Stop for any new PCDN.

### 0.1.10 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-10
**Change kind:** semantic
**Touches:** §5 protected amendment-first adoption and owning pre-commit instruction
**Summary:** Owner admits one exact documentation-only protected amendment PR
with the inherited main snapshot failure retained as OPEN; source gates remain.

#### Rationale

The owner accepted PCDN-10 to preserve two separate protected PRs. The governing
invariant amendments SHALL be adopted first; a fully qualified implementation
PR follows from the exact adopted main commit. The admission is confined to
one documentation-only amendment PR based on genuine main
`b64ec7ec7dda87f2d30f3160bf2fb8094fd3fee2`, refreshed before any ref mutation.
An unexpected base, authored input, source requirement or failure is an owning
decision stop, not permission to rebase or widen this boundary automatically.

The retained actual `scripts/pre-commit.sh` run failed with exit 101 after
300.248 seconds: 13 of 14 compile-fail cases passed, and `scanout_front_write`
differed only in the diagnostic `FrontBuffer` lifetime spelling. That run used
main `77ac0bf3876f5c732946ed478c2e0d85d3f94e35` plus the accepted Kria manifest
postimage that became b64. Its entire platform tree and compiled source are
unchanged on b64; fixture program, expected diagnostic, harness and typed API
hashes match, as do the compiler binaries and host runtime library bytes.
This is retained actual-source causal proof, not a fresh passing main run.
The failed receipt and source identities SHALL remain OPEN and immutable.

For this one amendment PR only, the owner admits that exact inherited failed
pre-commit result in place of the otherwise required successful pre-commit
prerequisite. Its authored diff SHALL contain exactly the five Markdown paths
enumerated in amendment 0.1.9, including this disposition and the matching
AGENTS crossreference. Refreeze all five actual postimage hashes and modes,
freshly run `lychee --no-progress` on all five without added filters or ignored
errors, and prove that all compiled source, fixtures, manifests, versions,
lock bytes or absence, nested pins and release/parent refs remain unchanged
outside those five documentation paths. Review complete main-to-postimage
document differences; do not apply A2 patches blindly. The README may stage
the future carrier's documentation, but no bridge manifest, code, fonts or
other implementation input belongs in this PR.

Genuine b64 has no spec-index source or projections. No partial or transplanted
index, scanner or extra tool source is admitted in the documentation PR. Keep
the actual complete candidate's canonical index proof tied to that candidate.
Adopt and regenerate the already-approved complete A2 index and generator with
the later coherent source PR. Normal protected PR/squash rules remain required;
no admin bypass, force update or retagging is authorized. The docs-only
exception expires at adoption or any additional source/profile/error
requirement, and never applies to the following implementation PR.

The implementation PR SHALL preserve the adopted five Markdown postimages and
compose coherent A2 plus the approved owned bridge and PCDN-08 correction. All
normal source checks, remaining C3 checks, full pre-commit/prepublish, genuine
source history, metadata, checkout locks, source/content drift, P-DIFF, full
31-crate order, extracted predecessors, withdrawal of preparation inputs,
all three consumers and the owned bridge/font boundary remain required before
source adoption and clean-ref Linux Gate P qualification. No original-source
fallback or automatic descriptor/member/target admission is granted.

Official Ratatui 0.30.2 remains unmodified; released A2 and original parent pins
remain unchanged. Bridge 0.1.2, platform 0.2.7 and root 0.2.8 retain their
selective allocation, core remains 0.2.7, and other versions stay unchanged.
PCDN-09's inherited whole-tree link failures remain OPEN. This admission makes
no compiled/provider-green, Gate P, registry, release, hardware or parent
qualification claim. New release identity, publication/Gate R and parent
adoption remain separate decisions. Stop for any new PCDN.

### 0.1.11 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-11
**Change kind:** semantic
**Touches:** §5 protected amendment-first documentation postimages and §15 history references
**Summary:** Keep the exact five-Markdown governing PR and correct only its
15 missing historical hyperlink destinations to immutable released-A2 sources;
a fresh passing five-document link check remains required.

#### Rationale

The owner accepted the bounded historical-link correction and requested a
release-documentation review for new features and necessary decisions before
release. The fresh unfiltered five-document check on genuine main
`b64ec7ec7dda87f2d30f3160bf2fb8094fd3fee2` failed: 26 checks, 18 unique,
11 passed and 15 missing-file occurrences named ten absent Markdown targets.
The retained failed result SHALL remain OPEN and immutable. A read-only local
prerequisite inventory reached 30 additional released-A2 Markdown documents,
a required 31st, six unvisited bodies and a host-local HTML prerequisite. That
inventory is not a closed import set and SHALL NOT be transplanted as one.

For this one protected governing PR, retain exactly the five Markdown paths
admitted by 0.1.9 and 0.1.10. Only the following 15 historical href occurrences
SHALL use the GitHub blob view in `SoftOboros/rlvgl`, pinned to the full
released-A2 commit `a2b48e108ae9e0623d2bb6217cef73f0113e8708` and their exact
tracked target paths. Preserve each link label and its surrounding decision:

- `docs/concepts/RATATUI-00-CONCEPTS.md`: one link to
  `docs/concepts/RATATUI-RETROSPECTIVE.md`.
- `docs/concepts/SCTD-04-RATATUI-RLVGL-INTEGRATION.md`: one link each to
  `docs/concepts/RATATUI-RETROSPECTIVE.md` and
  `docs/releases/v0.2.7-demo-evidence.md`.
- This document: three links each to
  `docs/crates-ci/CRATES-CI-01B-STAGING-ISOLATION.md` and
  `docs/crates-ci/ERRATA.md`; one link each to
  `docs/releases/v0.2.7-continuation-decision.md`,
  `docs/releases/v0.2.7-continuation-wave3.md`,
  `docs/releases/v0.2.7-packaging-wave4.md`,
  `docs/releases/v0.2.7-staging-20261010.md`,
  `docs/releases/v0.2.7-staging-decision.md` and
  `docs/releases/v0.2.7-staging-mechanism.md`.

Those URLs identify historical A2 documents, not adopted main implementation
or repaired historical document internals. Do not rewrite labels, delete
history, import the unclosed prerequisite chain or amend released A2. Record
this disposition and the matching AGENTS crossreference before adopting the
new five postimages; refreeze their hashes and modes and review the complete
main-to-postimage differences. No other Markdown path, scanner, index,
implementation input, manifest, font, lock, version or gitlink belongs in the
governing PR. The complete canonical candidate index remains candidate proof
and belongs with the later coherent implementation PR.

A fresh actual `lychee --no-progress` run on all five SHALL pass, without added
filters, exclusions or ignored errors. HTTP availability diagnostics do not
replace that check; a failed destination requires a stop, not a missing-link
or HTTP-status waiver. Retain every prior failed receipt. Run the unchanged
literal formatting commands and prove compiled source, fixtures, manifests,
versions, modes, locks or absence, nested pins and release/parent refs unchanged
outside the five. Unexpected base drift, input, source/profile requirement,
new error or new normative feature decision SHALL stop for an owning PCDN.

Retain 0.1.10's exact inherited main pre-commit failure as OPEN for this one
governing PR only. Keep the normal protected amendment-first PR/squash route,
with no admin bypass, force update or retagging. The exception expires at
adoption or any additional unapproved requirement and never transfers to the
following implementation PR. Review release documentation against accepted
feature decisions and actual source; do not silently ratify Draft phases,
close missing evidence or assert hardware, consumer or publication readiness.

Official Ratatui 0.30.2 remains unmodified, released A2 and original parent pins
remain unchanged, and selective versions remain bridge 0.1.2, platform 0.2.7,
root 0.2.8 and core 0.2.7 with other packages unchanged. All normal following
source, complete index, full pre-commit/prepublish, P-VCS, P-META,
P-SOURCE-LOCK, P-DIFF, 31-crate order, extracted-predecessor withdrawal,
three-consumer and bridge/font boundaries remain required. Clean-ref Linux
Gate P, separately admitted release identity/publication/Gate R and parent
adoption remain distinct gates. PCDN-09 inherited whole-tree link failures and
other retained failures remain OPEN. Stop for any new PCDN.

### 0.1.12 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-12
**Change kind:** semantic
**Touches:** §5 documentation postimage timing and §6 shared-file admission receipts
**Summary:** Admit the exact eleven-file factual release-documentation refresh,
fifteen-document passing link scope and two reviewed README fingerprint values;
retain the exact-five governing PR and all source and release guards.

#### Rationale

The owner accepted PCDN-12 after the bounded public-feature and family review.
It found stale fork dependencies/imports/paths, missing owned compatibility and
portable-atomic guidance, mismatched font metrics in examples, dated version
navigation and imprecise native-versus-interpreter snapshot status. The review
selected factual documentation corrections, not new feature behavior, phase
ratification, a release identity or a registry-availability claim.

The exact authored cleanup set SHALL be:

- `crates/ratatui-rlvgl/README.md`;
- `docs/ratatui-tutorial/README.md`;
- `docs/ratatui-tutorial/01-backend-and-surface.md`;
- `docs/ratatui-tutorial/02-rendering-cells.md`;
- `docs/ratatui-tutorial/04-live-state.md`;
- `README.md`;
- `docs/CHANGELOG.md`;
- `docs/releases/owned-bridge-maintenance.md`;
- `docs/releases/v0.2.7-successor-family-state-20261013.md`;
- `docs/releases/v0.2.7.md`; and
- `docs/concepts/MPY-04-STAGE-DIRECTIONS-INTROSPECTION.md`.

Keep the first governing PR at exactly the five Markdown paths enumerated in
0.1.9. Finalize the owned bridge README's self-contained future-carrier
migration documentation, including its corrected font-metrics example, before
freezing that governing PR. Record this amendment and matching AGENTS authority
first. Its main carrier SHALL NOT link to later documentation absent from main.
Refreeze and review all five postimages and freshly pass their unchanged,
unfiltered actual link command, literal formatting and source preservation.
Keep 0.1.10's exact inherited main pre-commit failure OPEN for this governing
PR only; no other failed result is admitted there.

The other ten authored cleanup paths belong only in the later coherent
implementation PR with complete accepted A2 source/docs/index and the owned
bridge implementation. That PR SHALL preserve all five adopted governing
postimages. Explain official unmodified Ratatui 0.30.2 separately from native
core 0.1.2, native imports, the owned compatibility subset and target portability
limits. Do not add widget behavior, hardware assumptions, font repertoire or
rendering defaults merely to retain obsolete examples. Use Unreleased for the
maintenance guide and distinguish prepared package allocations from registry
availability. Add dated successor navigation; retain historical observations,
source identities, tables, measurements and failed receipts.

For this exact candidate, extend 0.1.9's changed-document admission to exactly
fifteen distinct Markdown paths: its original five union the eleven paths above,
with the bridge README overlapping. Every touched postimage SHALL freshly pass
unchanged `lychee --no-progress`, without added filters, exclusions, fabricated
responses or ignored failures. Retain the actual inherited whole-tree failure
as FAIL and OPEN; keep the checker unchanged. Freeze actual hashes and modes,
regenerate only affected projections with the unchanged canonical generator,
and extend the finite freeze to the authored inputs and changed projections.
Do not transplant index/generator source into the governing PR. Any additional
path, unexpected source/profile requirement, base drift or new error SHALL stop
for an owning PCDN; this is not general documentation or dependency admission.

The staging receipt already fingerprints both edited READMEs. Admit only the
reviewed, frozen postimage SHA-256 values at these two existing JSON pointers
in `scripts/crates_ci_workspace_contexts.json`, in the later source PR:

- `/workspaces/0/members/0/shared_files/0/sha256`, for `README.md`, replacing
  `009a30632167019925ca7c38889eac1d9301ebf3f40ddc4e3885aa66ee8bda2a`;
- `/workspaces/0/members/14/shared_files/0/sha256`, for the owned bridge README,
  replacing `7491acc1c1b72ab08e5d147cb62149f847ebd1241a12d28376a5a1c7d59d9710`.

Every other receipt value SHALL be structurally exact: same workspace identities,
descriptor hashes, paths, members, manifest hashes, target contexts, other shared
hashes and finite root48/native42 contexts. Keep staging/package checker code
unchanged. No automatic receipt refresh, wildcard, profile widening, new
workspace/descriptor/member/target, original-source fallback or baseline ratchet
is authorized. The README postimage admission does not itself qualify staging:
fresh full package/source-content/P-DIFF/predecessor/withdrawal/three-consumer
and same-ref Linux Gate P proof remains mandatory at its applicable gate.

Do not edit unchanged published core README/docstrings or other unchanged
package payloads; no code, manifest, font, lock, version, gitlink or accepted
feature contract change belongs in this refresh. The two documentation-derived
receipt hash values are the sole additional non-Markdown authored input change.
Native snapshot documentation does not qualify wire/interpreter/cross-driver
or board surfaces. MPY-06/07 remain unratified; MPY-08/09 and staging01B remain
Draft. Preserve producer production0.4.2 pins, unknown absent HSE frequency,
CCPS-specific validation and all independent hardware/provider obligations.

Official Ratatui 0.30.2, released A2 and original parent pins remain unchanged.
Keep bridge0.1.2, platform0.2.7, root0.2.8 and core0.2.7 selective allocations,
with other versions unchanged. All normal source, formatting, full pre-commit
and prepublish, P-VCS, P-META, P-SOURCE-LOCK, content/P-DIFF, full31 order,
extracted predecessors, withdrawal, three consumers and protected amendment-first
PR/squash guards remain required. New release/tag identity, publication/Gate R
and parent adoption remain separately admitted decisions after their gates.
No admin bypass, force update, retagging, failure relabeling or hardware readiness
claim is granted. Stop for any new PCDN.

### 0.1.13 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-13
**Change kind:** semantic
**Touches:** §5 one governing documentation adoption and later qualification profile
**Summary:** Admit the exact retained hosted diagnostic spelling failures for
one five-document PR; admit two dated qualification inputs only in the later
coherent source PR. All source and release gates remain required.

#### Rationale and bounded documentation disposition

The owner accepted PCDN-13 after PR223's hosted CI run 38425309332 failed its
workspace test step: 11 of 14 compile-fail cases passed. Its setup script
updated nightly from Rust 1.91 to 1.101.0-nightly `e9bbadf68` (2026-10-12).
Only `dca_dbuf_use_after_lend`, `dca_ltdc_use_after_stop` and
`dca_use_after_lend` differed. Each retained E0382, identical moved values,
move/borrow positions and receiver ownership prohibition; only the explanatory
note added `rlvgl_platform::hwcore::dca::` to its type name. Retain the actual
11/14 failed result and complete diagnostic evidence as OPEN. Do not bless,
normalize, filter or relabel the failed snapshots as passing.

For the one five-Markdown governing PR223 based on genuine main
`b64ec7ec7dda87f2d30f3160bf2fb8094fd3fee2` only, the owner admits that exact
hosted result alongside 0.1.10's retained local 13/14 `scanout_front_write`
result. Hosted Gate P run 38425309292 reproduced the already OPEN PCDN-04
root rlvgl 0.2.5 `Cargo.toml.orig` payload drift; its consumers were skipped.
That remains a failed Gate P, not a source or release qualification.

Amend only this owning disposition and its AGENTS mirror, then refreeze and
review the complete five postimages enumerated in 0.1.9. Freshly pass their
unchanged unfiltered link command and literal formatting, and prove unchanged
compiled source, fixtures, manifests, versions, locks, nested pins and release
and parent refs outside the five paths. Keep the protected PR/squash route;
no administrative bypass, force update or status substitution is admitted.
This explicitly admits that documentation adoption boundary despite the
retained exact failures; all other prerequisites remain. The exception expires
at adoption. Any additional failing case, source/profile/input requirement or
base drift is a new owning decision stop. It never qualifies later source.

#### Exact later qualification-profile admission

After adoption and verification of the actual resulting main ref, the later
coherent implementation PR MAY additionally change exactly
`scripts/setup-ci-env.sh` and `.github/workflows/ci.yml` for the qualification
profile. Select `nightly-2025-07-31`: rustc 1.90.0-nightly `3048886e5` and
Cargo 1.90.0-nightly `9b296973b`. Replace floating nightly update/selection
with genuine dated installation, effective profile selection for all existing
setup callers and subsequent steps, identity logging and fail-closed checks.
Make CI's docs command use that same dated profile instead of `cargo +nightly`.
Retain recursive submodule setup, rustfmt, clippy, rust-src,
llvm-tools-preview, required targets and every existing command/failure return.
A caller's unexpected profile or unavailable component/target is a stop;
no compiler/date fallback or simulated installation is permitted.

Record actual Linux compiler, Cargo, component and target identities. The
preserved macOS profile is provenance, not proof of Linux qualification.
Pinning b64 alone is not expected to make its inherited snapshot green.
The subsequent genuine integrated source SHALL pass all 14 compile-fail
cases, complete pre-commit/pre-publish, publisher/workflow contracts and full
clean same-ref Linux/hosted Gate P, P-DIFF, archive order, extracted predecessor
withdrawal and all three consumers. Retain accepted source/provenance checks,
including the separately admitted scanout correction. Neither documentation
failure disposition transfers to this source PR. Any further mismatch or
unadmitted source, fixture, lock or guard requirement stops for a new PCDN.

The shared helper's dated selection reaches its existing container callers;
no other workflow YAML, publisher stable selection, OIDC/authentication,
trigger, container/image or qualification guard is admitted for alteration.
Keep all five adopted governing postimages and the ten later cleanup docs,
0.1.12's exact two README receipt SHA leaves and every other receipt value.
Preserve selective bridge 0.1.2, platform 0.2.7, root 0.2.8 and core 0.2.7,
unmodified official Ratatui 0.30.2, released A2/v0.2.7 and parent pins.
No new release identity, registry publication, hardware flash, phase
ratification or general CI waiver follows. Stop for any new PCDN.

### 0.1.14 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's acceptance of PCDN-14
**Change kind:** semantic
**Touches:** §6 finite preparation activation and the protected amendment boundary
**Summary:** Activate only the existing owned-bridge to native-core preparation
edge; preserve all source, publication and verification guards.

#### Rationale and exact behavior admission

The owner authorized patching the PCDN-14 staging blocker. Genuine source
`0326c3a522d4937d310aeebbd59b5d75dd1cf116`, tree
`66ca38fd5af133a085fe6d08866b7f9d80135bca`, failed hosted Gate P run
38446207092 and clean-reference Linux Gate P at package 19, the owned bridge,
after the first 18 packages completed. Retain both actual P-ORDER failures:
`ratatui-rlvgl has nonadmitted path dependency ratatui-core`. Hosted consumers
were skipped and local consumers were not run; the full 31-package gate is
FAIL, not partially qualified.

Native `ratatui-core` is already in the 42-member admitted native context.
The explicit bridge manifest has neither a workspace section nor inherited
fields, so archive preparation does not activate that context before its normal
path dependency check. Admit only activation and receipt recording of the exact
edge specified in §6. Keep the original bridge manifest, genuine native
descriptor/member/target/shared-file identities and actual source inventories.
The native descriptor fingerprint remains
`a5007fe4cb0d9901302c5eb201941dcec82755370917144e312d2bfe9e5a6567`;
native-core member manifest fingerprint remains
`6f8222cb1d4a15490641faec42314bc8975b7e8e265f8ab2e0a3e4925ff068ca`.
The current 48 root / 42 native member policy stays unchanged. This permits no
generic admission, new descriptor/member/target or native publish-list entry.

The later bounded implementation MAY change only the existing package helper
and meaningful finite boundary tests, followed by canonical index generation.
Record the exact edge and activated context. Require positive coverage for that
edge and refusal controls for another consumer, name/path/version/dependency
kind/default-feature tuple, changed fingerprints, added membership or source
leaks. Preserve Cargo ownership of manifests, inventories and normalization,
progressive extracted predecessors, source lock bytes or absence on success
and failure, genuine nested provenance, P-VCS/P-META/P-INCLUDE/content/P-DIFF,
clean-source refusal, full package order and all three consumers. Withdraw
native/original admission inputs before normalized verification and consumers;
native-core must then resolve solely from the registry at its exact version.
An unavailable registry version or further source/lock/guard requirement stops
for another owning decision.

#### One protected two-document amendment disposition

Adopt this owning amendment and then its AGENTS mirror in a separate protected
PR on unchanged main `16a6631aae0ae0cb9917f2332414f917a69b5dca` before behavior
adoption. That PR contains exactly these two governing Markdown paths, with no
index, helper, test, workflow, source, fixture, manifest, lock, version or pin
change. The owner admits only this one documentation adoption boundary despite
independently classified inherited main failures: the retained dated local
13/14 scanout diagnostic result, floating hosted 11/14 with the exact three
DCA spelling differences recorded in 0.1.13, and OPEN PCDN-04 root 0.2.5
`Cargo.toml.orig` payload drift. Require full two-document review, fresh
applicable unfiltered link and literal-format checks, and exact unchanged
baseline custody. Preserve failed statuses and complete negative evidence as
OPEN. Any added failing case, source/profile/input requirement or base drift
stops for another owning decision. No administrative bypass, status
substitution, fixture blessing or force update is admitted.

PR223's prior exception has expired. This new two-document disposition expires
at its own adoption and never transfers to PR224 or corrected source. After
authority adoption, qualify a genuine corrected source ref with all 14
snapshots, full pre-commit/pre-publish, complete clean Linux and hosted
31-package Gate P, predecessor withdrawal and all three extracted consumers.
Preserve the failed predecessor source and its evidence.

Unmodified official Ratatui 0.30.2, eight source locks, fonts/notices, selective
bridge 0.1.2 / platform 0.2.7 / root 0.2.8 / core 0.2.7 allocations, released
A2/v0.2.7, parent pins and all other guards remain preserved. PCDN-13's genuine
dated qualification profile remains unchanged; tracking later nightly changes
requires a separately settled compiler policy. No new release identity,
registry publication, hardware qualification, phase ratification or parent
adoption follows. Stop for any new PCDN.

### 0.1.15 — 2026-10-13 — amended

**Author:** Codex, recording I. Abbott's latest-nightly policy and PCDN-15/16 acceptance
**Change kind:** semantic
**Touches:** §5 qualification/compiler freshness and three exact DCA diagnostics
**Summary:** Build main daily on newest nightly, qualify every release on most
recent nightly, and admit only three semantically reviewed diagnostic note updates.

#### Owner policy and scope

The owner specified daily main builds to catch nightly drift and clean releases
on the most recent nightly, then accepted PCDN-15. This supersedes the ongoing
nightly-2025-07-31 hold and the later-policy-pending clause. Retain all prior
amendments, dated successes and floating failures as historical evidence;
none qualifies a future release as current-latest.

Owner-accepted PCDN-16 admits exactly this owning Markdown, its AGENTS mirror,
and docs/spec-index/index/crates-ci.json plus docs/spec-index/index/_manifest.json
mechanically generated by the unchanged canonical generator in a separate
protected authority PR before implementation. No helper, workflow, fixture,
manifest, version, source lock, pin, scanner or unrelated prefix output belongs
in that PR. Its unchanged implementation uses the existing dated setup until
authority adoption; every applicable source, document and current-index guard
must pass, with no waiver or status substitution. This boundary is not latest
release proof. PCDN-16's original 19/20 index failure remains OPEN historical
evidence; regenerated current projections and passing guards provide new proof.

After authority adoption the bounded source PR MAY change the shared setup and
meaningful latest-manifest/cohort tooling/tests, add daily main CI and manual
verification while preserving PR checks, and propagate the cohort through
existing qualification callers and release freshness preflight. Resolve newest
once per workflow, install that exact date, verify actual host/compiler/Cargo,
components and required targets, and retain immutable manifest provenance.
Do not change the global/user or previously preserved default profile.
A missing component/target, older fallback, mismatched cohort, new source/profile
requirement or unadmitted failure is an owning decision stop. Reconfirm newest
at release preflight; a new cohort requires compiler-dependent requalification.

#### Exact diagnostic admission

The official 2026-10-13 manifest advertises rustc e9bbadf68, the compiler from
retained PR225 CI38450233045's actual 11/14 failure. Only the following existing
stderr postimages MAY change, after fresh same-ref newest-nightly execution and
semantic review:

- platform/tests/ui/dca_dbuf_use_after_lend.stderr
- platform/tests/ui/dca_ltdc_use_after_stop.stderr
- platform/tests/ui/dca_use_after_lend.stderr

The admitted delta is only the explanatory receiver-type path prefix
`rlvgl_platform::hwcore::dca::`. Preserve E0382, moved values, source locations,
receiver self ownership and every prohibition. Obtain exact postimages from
fresh actual diagnostics; do not substitute old logs for current-source proof.
No generic TRYBUILD overwrite, filtering, harness change, Rust-body edit or
other fixture delta is admitted. Additional mismatch or semantic drift stops
for a new PCDN. Retain the original failed result as OPEN and separately prove
all fourteen strict cases pass after the bounded correction.

After the owner's explicit newest-nightly policy clarification, generate only
the two named authority projections in the authority PR. Later bounded source
changes require their corresponding canonical projections. Require full
pre-commit/pre-publish, full clean same-ref Linux/hosted Gate P all31 packages
and all three extracted consumers, exact source/payload/P-DIFF/lock/withdrawal
provenance, publisher/workflow checks and protected adoption. No narrowed
result replaces any required guard.

Keep stable publisher selection, Trusted Publishing/OIDC, release environment,
frozen release-ref authentication prerequisites and manual continuation. Registry
Gate R remains separate; neither daily main CI nor this amendment publishes.
Preserve official Ratatui0.30.2, native-core0.1.2, original eight locks, fonts,
fixed48/42 policy, released A2/v0.2.7, parent pins and selective bridge0.1.2 /
platform0.2.7 / root0.2.8 / core0.2.7. No new tag, registry publication, hardware
qualification, phase ratification or parent adoption follows. Stop for new PCDNs.

### 0.1.16 — 2026-10-14 — amended

**Author:** Codex, recording I. Abbott's PCDN-17 acceptance
**Change kind:** semantic
**Touches:** §5 exact scanout diagnostic admission and protected source integration
**Summary:** Admit one lifetime spelling correction in the existing read-only
front-buffer diagnostic, preserving the strict prohibition and all release guards.

The owner accepted PCDN-17 after the actual clean-source newest-nightly run
passed ten of fourteen strict cases. Retain that complete failure as OPEN
historical evidence. Its three DCA receiver-path differences are already bounded
by 0.1.15; only platform/tests/ui/scanout_front_write.stderr is additionally
admitted here. Change only the headline type spelling from `FrontBuffer` to
`FrontBuffer<'a>`. Preserve E0599, the absent `cpu_slice_mut` method, source
location 33:35, the final inferred `FrontBuffer<'_>` and the read-only API.
The existing lifetime was already declared by the source; this correction adds
no lifetime behavior, mutable access, method, API or fixture Rust change.

Apply only the four captured actual postimages within these exact admissions.
Freshly run every one of the unchanged fourteen cases on the corrected genuine
source and the immutable most-recent official nightly cohort. The captured
diagnostics explain the bounded edit; they do not replace fresh execution if
the latest manifest advances. No generic TRYBUILD overwrite, case selection,
test filtering, harness edit, Rust-body change or additional diagnostic delta
is admitted. A further mismatch, semantic drift or source/profile requirement
stops for another owning PCDN.

Record this owning amendment and its AGENTS mirror, with only
docs/spec-index/index/crates-ci.json and docs/spec-index/index/_manifest.json
mechanically regenerated by the unchanged canonical generator, in the planned
normal protected source integration after this owner acceptance. There is no
second dated-result substitution, authority waiver or transferred exception.
The source integration remains bounded to the accepted shared cohort helper,
meaningful controls, six existing qualification workflows, these four stderr
postimages and their governing projections. Require all fourteen snapshots,
full source/pre-publish, complete clean same-ref Linux and hosted Gate P with
all31 packages and all three extracted consumers, publisher/cohort controls,
current canonical index, exact payload/lock/withdrawal provenance and normal
protected adoption. Keep every failed receipt OPEN; a narrowed result replaces
no required guard.

Keep daily latest-nightly main builds and most-recent-nightly release freshness,
stable publisher selection, frozen-ref authentication prerequisites, Trusted
Publishing/OIDC, release environment, manual continuation and separate Gate R.
Preserve released A2/v0.2.7, official unmodified Ratatui0.30.2/native-core0.1.2,
parent pins, original eight locks, fonts/notices, fixed48/42 policy and selective
bridge0.1.2 / platform0.2.7 / root0.2.8 / core0.2.7 allocations. No new tag,
registry publication, hardware qualification, phase ratification or parent
adoption follows. Stop for any new PCDN.

### 0.1.17 — 2026-10-14 — amended

**Author:** Codex, recording I. Abbott's PCDN-18 acceptance
**Change kind:** semantic
**Touches:** §5 newest-nightly public iterator compatibility and selective API allocation
**Summary:** Preserve the exposed wire iterator with one exact lint expectation,
allocate API 0.2.2 and admit only its necessary fingerprint and lock-node updates.

The owner accepted PCDN-18 after genuine source
13dfd3b143864ab74c6da62e078ecddc610e6cfe passed newest-nightly formatting,
all fourteen strict snapshots and strict discipline, then failed workspace
Clippy with -D warnings at api/src/protocol.rs:1442:65. The actual compiler
cohort was official nightly-2026-10-14. Retain this complete exit-101 failure
as OPEN historical evidence; it is not a passing release or substituted status.

#### Exact compatibility correction

Admit one function-scoped #[expect(clippy::chunks_exact_to_as_chunks)] with a
documented reason at OpcodeList::iter. Preserve the production method body and
public OpcodeIter::Wire(core::slice::ChunksExact<'a, u8>) payload. Replacing it
with an array-slice iterator would change the exposed public type. Preserve
native/wire ordering, little-endian decoding, length, size_hint, remainder
behavior and ExactSizeIterator. This admits no API redesign or data reinterpretation.
Global -D warnings and unfulfilled-expectation detection remain mandatory;
an absent or changed diagnostic requires review, not a permanent blanket allow.

API 0.2.1 is already published. Allocate only rlvgl-api 0.2.2 in api/Cargo.toml
and align API_VERSION.patch from 1 to 2 in api/src/lib.rs. Preserve MPY_V1 and
wire framing. Existing core, runtime-std dev and MicroPython requirements of
0.2.1 already admit this patch release; keep their manifests and published core
payload unchanged. Add meaningful public Wire construction/matching, native/wire
sequence, advancing length/size_hint, empty/remainder and advertised-version
controls only within existing api/tests/mpy_v1_golden.rs. Keep existing
assertions and golden bytes; add no Cargo target.

#### Twelve-path amendment and source boundary

Record this owning amendment and AGENTS mirror before implementation, then
mechanically regenerate docs/spec-index/index/crates-ci.json and
docs/spec-index/index/_manifest.json with the unchanged canonical generator.
Use the established protected integration route. No additional authority PR,
dated-result substitution, transferred waiver or direct main update is admitted.

The remaining eight paths are api/src/protocol.rs, api/Cargo.toml,
api/src/lib.rs, api/tests/mpy_v1_golden.rs,
scripts/crates_ci_workspace_contexts.json, Cargo.lock,
micropython/staticlib/Cargo.lock and
examples/beetle-esp32p4-idf/components/rlvgl_app/rust/Cargo.lock.
Only /workspaces/0/members/1/manifest_sha256 in the finite policy may change to
the corrected API manifest fingerprint. Admit only the local rlvgl-api node's
version from 0.2.1 to 0.2.2 in those three locks. Every other parsed lock value,
checksum and the other five governed lock bodies stays exact. Root/native
memberships remain 48/42, with no descriptor, member, target or shared-file
expansion. No other dependency minimum, package allocation or payload edit follows.

Freshly resolve the most recent official nightly cohort and qualify the genuine
corrected ref. Require all fourteen strict snapshots, the unchanged publisher
and package-boundary suites, meaningful cohort/API controls, full source and
pre-publish phases, complete clean same-ref Linux and hosted Gate P all31
packages and all three extracted consumers, current canonical projections,
source/payload/P-DIFF/lock/withdrawal/native-context custody and normal protected
adoption. Retain every failed receipt; no narrowed or older result replaces a guard.

Preserve original frozen source 13dfd3b1 and its actual failed results, released
A2/v0.2.7, official unmodified Ratatui0.30.2/native-core0.1.2, fonts/notices,
parent pins and selective bridge0.1.2 / platform0.2.7 / root0.2.8 / core0.2.7.
Keep daily newest-nightly main builds, most-recent release freshness, stable
publisher selection, Trusted Publishing/OIDC, release environment, frozen-ref
authentication prerequisites, manual continuation and separate Gate R. No
generic lint allowance, further nightly cleanup, new release identity, registry
publication, parent adoption, hardware qualification or phase ratification is
granted. Any additional failure, source/profile requirement or scope drift stops
for another owning PCDN.

### 0.1.18 — 2026-10-14 — amended

**Author:** Codex, recording I. Abbott's PCDN-19 acceptance
**Change kind:** semantic
**Touches:** §5 newest-nightly const dispatch compatibility and selective core allocation
**Summary:** Use a const-safe slice pattern in the target view, preserve its public
contract and allocate only the corrected future core payload as 0.2.8.

The owner accepted PCDN-19 after genuine source
62357e8e99b6369be97b8d6b20346d7248a9352f passed latest-nightly read-only
formatting, all three full locked metadata graphs, all 67 API golden tests,
all fourteen strict snapshots and strict discipline, then failed workspace
Clippy with -D warnings at core/src/object.rs:935:19. The actual official
cohort was nightly-2026-10-14. Retain the complete exit-101 failure as OPEN
historical evidence; it is not full source or release qualification.

#### Exact const-safe correction

Admit only the target_view production expression in core/src/object.rs.
Keep public const fn target_view(&self) -> NativeNodeView and the private
route representation unchanged. Match self.route.as_slice() directly:
the nonempty [.., target] pattern returns target.view; the empty pattern
panics with the exact existing "resolved routes contain a target" message.
Preserve last-node selection, pre-dispatch cached view semantics and the
nonempty resolved-route invariant. Change no other production method or type.

The selected genuine standard-library source makes Vec::as_slice const-stable,
while Vec const Deref remains gated by const_convert and there is no inherent
Vec::last. Merely deleting as_slice is not admitted as a const-compatible
correction. Introduce no language feature, new compiler minimum, lint
allowance or additional expectation. Global -D warnings and the existing
API's named expectation and unfulfilled-expectation guard remain unchanged.

Add meaningful controls only within the existing inline object test module:
a compiler-checked const caller, actual selected-root and deep-route final
cached views, and the exact empty-route invariant panic payload. Preserve every
original test, assertion and helper; add no Cargo target. A const caller
establishes callability, not nonempty allocated Vec CTFE evaluation.

#### Selective allocation and nine-path boundary

Core 0.2.7 is already published and remains immutable with released A2.
Allocate only the corrected future rlvgl-core payload as 0.2.8 in
core/Cargo.toml. The 27 existing dependent records use compatible 0.2, 0.2.4
or 0.2.7 minima or path-only edges; keep every dependent requirement unchanged.
Retain API 0.2.2 and its exact PCDN-18 implementation, bridge 0.1.2,
platform 0.2.7 and root 0.2.8. No blanket family increment follows.

Record this owning amendment and its AGENTS mirror before implementation,
then mechanically regenerate only docs/spec-index/index/crates-ci.json and
docs/spec-index/index/_manifest.json with the unchanged canonical generator.
Use the established protected source integration route; no additional
authority PR, dated-result substitution, transferred waiver or direct main
update is admitted.

The remaining five paths are core/src/object.rs, core/Cargo.toml,
scripts/crates_ci_workspace_contexts.json, Cargo.lock and
examples/beetle-esp32p4-idf/components/rlvgl_app/rust/Cargo.lock.
Only /workspaces/0/members/13/manifest_sha256 in the finite policy may change
to the new core manifest fingerprint. In exactly those two locks, admit only
the local rlvgl-core node version from 0.2.7 to 0.2.8. Preserve every other
parsed value/checksum and the other six governed lock bodies. Keep root/native
memberships 48/42, descriptors, targets and shared-file receipts unchanged.

Freshly resolve the most recent official nightly and qualify the corrected
genuine ref. Require the const/route controls, all 67 API goldens, all fourteen
strict cases, strict discipline and workspace Clippy, unchanged publisher,
package-boundary and cohort suites, full source/pre-publish phases and current
canonical projections. Require complete clean same-ref Linux and hosted Gate P
with all 31 packages and all three extracted consumers, source/payload/P-DIFF/
lock/withdrawal/native-context custody, freshness and normal protected adoption.
Automatic PR merge-ref integration evidence and exact head-ref Gate P evidence
retain their actual distinct provenance; relabel no source or passing result.
Retain every failed receipt; a narrower or older result replaces no guard.

Preserve frozen 62357e8e and 13dfd3b1 and their failed results, released
A2/v0.2.7 and published core 0.2.7, official unmodified Ratatui 0.30.2 and
native-core 0.1.2, fonts/notices, parent pins and every other package allocation.
Keep daily newest-nightly main builds, most-recent release freshness, stable
publisher selection, Trusted Publishing/OIDC, release environment, frozen-ref
authentication prerequisites, manual continuation and separate Gate R.
No new tag, registry publication, parent adoption, hardware qualification or
phase ratification follows. Any additional failure, source/profile requirement
or scope drift stops for another owning PCDN.

### 0.1.19 — 2026-10-14 — amended

**Author:** Codex, recording I. Abbott's PCDN-20 acceptance
**Change kind:** semantic
**Touches:** §5 newest-nightly private runtime atomic compatibility
**Summary:** Replace exactly two deprecated atomic method names while preserving
service admission and epoch behavior, with no package allocation or lock change.

The owner accepted PCDN-20 after genuine source
5d75506225e21f04a2910ba7a429a4327f47c20d passed latest-nightly read-only
formatting, three full locked metadata graphs, all 272 core library tests,
all 67 API golden tests, all fourteen strict compile cases and strict
discipline, then failed workspace Clippy with -D warnings. Exactly two fatal
deprecation diagnostics identify runtime-std/src/service.rs:892:14 in
NativeService::try_submit and :1729:10 in next_epoch. The actual official
cohort was nightly-2026-10-14. Retain its full exit-101 result, nonfatal
warnings, provenance and terminal custody as OPEN historical evidence.
These partial results do not qualify full pre-commit, Gate P or release.

#### Exact atomic spelling correction and compiler consequence

In runtime-std/src/service.rs replace only the two fetch_update method names
with try_update. Genuine selected standard-library source proves the integer
atomic fetch_update alias delegates directly to
try_update(set_order, fetch_order, f). Preserve Relaxed/Relaxed, checked_add(1),
closure retry behavior, returned previous value, NonZeroU64 conversion,
request ownership on failure, existing counter consumption, exhaustion,
epoch isolation, ticket validation, lifecycle and backpressure. Change no
other production body, helper, public API, type, test or assertion.
Introduce no lint allowance, expectation, feature, fallback or dependency.

try_update is stable since Rust 1.95. Accept this API-imposed lower bound for
the private runtime source; it is not a verified whole-crate MSRV or a new
compiler floor for public packages. Claim no historical Rust 1.90 compatibility.
Effective 0.1.15 supersedes the ongoing dated compiler hold: daily main builds
track newest nightly and every release is freshly qualified on most recent
nightly. Retain all older successes and failures as historical evidence.

The private package rlvgl-runtime-std remains 0.0.0 with publish=false and no
declared rust-version. No Cargo manifest, package version, dependency minimum,
policy leaf, target membership, shared-file fingerprint or lock update is
admitted. service.rs is neither an enumerated policy fingerprint nor an
admitted target leaf; its module remains reached through the existing library
target. Preserve core 0.2.8, API 0.2.2, bridge 0.1.2, platform 0.2.7, root 0.2.8
and every other existing allocation, all eight governed locks and 48/42 policy.

#### Exact five-path protected integration boundary

Record this owning amendment and its AGENTS mirror before implementation.
Mechanically regenerate only docs/spec-index/index/crates-ci.json and
docs/spec-index/index/_manifest.json using the unchanged canonical generator.
Those four governing paths and runtime-std/src/service.rs are the complete
five-path boundary. Use fresh owned source derived from frozen 5d755062 and
genuine normal history through the established protected source route.
No additional authority PR, dated-result substitution, transferred waiver or
direct main update is admitted.

Run unchanged complete runtime unit/integration/doc tests, including all
thirteen native-service controls and headless-owner coverage. Existing controls
cover terminal ticket accounting, Full/Closing returned ownership, FIFO and
backpressure, restart/stale epoch/ticket/record rejection, fault fencing,
lifecycle and owner destruction. Direct u64::MAX request/epoch exhaustion
coverage remains unqualified; the unchanged checked_add and selected alias
source establish arithmetic/exhaustion equivalence. Add no tests mirroring
the standard library and do not mutate the global epoch for a test.

Freshly resolve the most recent official nightly in a new private profile.
Require complete source/index/document/publisher/cohort/context/lock/payload
guards, all core and API controls, all fourteen strict compile cases, strict
discipline, workspace Clippy and full scripts/pre-commit.sh before opening a
PR. Require literal CLAUDE pre-publish phases, all-features documentation,
complete clean same-ref Linux and hosted Gate P all 31 packages and all three
extracted consumers, P-DIFF/withdrawal/native-context custody, freshness and
normal protected adoption. Automatic PR merge-ref integration and exact
head-ref Gate P keep their distinct provenance. Missing manual CI dispatch
on current main waives no guard. Preserve inherited whole-tree link failures
OPEN under the accepted dispositions and freshly pass owning changed documents
without added exclusions. An older or narrower passing result replaces no guard.

Preserve frozen 5d755062, 62357e8e and 13dfd3b1, their private profiles and all
failed receipts, released A2/v0.2.7, published core 0.2.7, unmodified official
Ratatui 0.30.2/native-core 0.1.2, fonts/notices, parent pins and all other guards.
Keep stable publisher selection, Trusted Publishing/OIDC, release environment,
frozen-ref authentication prerequisites, manual continuation and separate Gate R.
No new tag, registry publication, parent adoption, hardware qualification or
phase ratification follows. Any additional failure, input expansion, source or
profile requirement or new normative choice stops for another owning PCDN.

### 0.1.20 — 2026-10-14 — amended

**Author:** Codex, recording I. Abbott's PCDN-21 one-retry authorization
**Change kind:** semantic
**Touches:** §5 owning-document link-check retry disposition
**Summary:** Admit one identical unfiltered retry after four HTTP503 responses;
retain failure evidence and require a complete passing result.

The owner authorized one retry after the PCDN-20 five-path WIP, derived from
genuine 5d75506225e21f04a2910ba7a429a4327f47c20d, passed canonical index/diff
and eighteen cohort, six publisher and all thirty-four package-boundary
controls, then failed the required unfiltered owning-two-document link check.
Twenty checks passed; four unchanged immutable released-A2 GitHub URLs returned
HTTP503 Service Unavailable. Retain that full exit-2 command/log/duration and
the complete frozen PCDN-20 postimages and seals as OPEN historical evidence.
No link status pass, runtime qualification or publisher fault was established.

#### One exact retry, with no status or source waiver

Admit exactly one further execution of the identical command:

    /Users/iraabbott/.cargo/bin/lychee --no-progress AGENTS.md docs/crates-ci/CRATES-CI-00-CONCEPTS.md

Use the same network access and default status policy. Check the complete two
owning documents after this expressly admitted amendment; identical command
does not mean retaining the old governing postimage bytes. Freeze the new
governing postimages and prove all four failed href strings and occurrences
remain unchanged. Their released-A2 files are ERRATA.md,
v0.2.7-continuation-decision.md, v0.2.7-staging-mechanism.md and
v0.2.7-staging-20261010.md, originally at concepts lines607/646/678/710.

The retry MUST produce an actual complete PASS before any following release
guard or genuine-history operation. Accept no HTTP503, filter, ignored error,
cache substitution, URL rewrite or narrowed diagnostic in place of the whole
check. If the retry fails, stop and retain both outcomes, without another
automatic retry or recursive remediation. This disposition expires after
that retry outcome; it grants no failing source or release claim.

#### Four-path recording and preservation

Record this owning amendment and its AGENTS mirror before the retry. Regenerate
only docs/spec-index/index/crates-ci.json and docs/spec-index/index/_manifest.json
with the unchanged canonical generator. These exact four governing paths are
the complete new admission in a fresh derived checkout. Preserve the accepted
PCDN-20 runtime postimage byte-for-byte and the complete old five-path WIP/seal
as historical evidence. No Rust, test, manifest, version, dependency, lock,
policy, workflow, target, source pin or publisher-tool change is admitted.
A possible publisher cause is read-only investigation, not implementation
authority; the failed inputs here are unchanged GitHub documentation hrefs.

A passing retry permits the existing genuine-history and fresh most-recent
same-ref qualification sequence to resume. Require complete unchanged runtime
tests, core/API controls, all fourteen strict compile cases, strict discipline,
Clippy, full scripts/pre-commit.sh before opening a PR, literal CLAUDE pre-publish
phases, all-features documentation, complete clean Linux and hosted Gate P
all31 packages/all3 extracted consumers, exact context/payload/lock/P-DIFF/
withdrawal/source/profile custody, latest freshness and normal protected
adoption. Keep automatic PR merge-ref and exact head-ref evidence distinct.
Preserve inherited whole-tree link failures OPEN under their accepted
dispositions; this one retry waives no other guard or following fresh check.

Preserve genuine5d/623/13df and all old postimages/private profiles/failed
receipts, released A2/v0.2.7 and published core0.2.7, official unmodified
Ratatui0.30.2/native-core0.1.2, fonts/notices, all current allocations and
dependency minima, every governed lock, finite48/42 policy and parent pins.
Keep daily newest-nightly main, most-recent release qualification, stable
publisher selection, Trusted Publishing/OIDC, release environment, frozen-ref
authentication, manual continuation and separate Gate R. No tag, registry
publication, parent adoption, hardware/live qualification or phase closure
follows. Any additional source, URL, status-policy, profile, input or normative
requirement stops for owner disposition; any subsequent guard failure stops
for another owning PCDN.

### 0.1.21 — 2026-10-15 — amended

**Author:** Codex, recording I. Abbott's accepted PCDN-34
**Change kind:** semantic
**Touches:** §5 strict diagnostic snapshot maintenance admission
**Summary:** Admit three reviewed diagnostic-context updates in a successor
candidate; retain strict negative tests and every release prerequisite.

Permit only the added enclosing impl/method context captured by the PCDN-33
latest-nightly failure in platform/tests/ui/dca_dbuf_use_after_lend.stderr,
platform/tests/ui/dca_ltdc_use_after_stop.stderr and
platform/tests/ui/dca_use_after_lend.stderr. Preserve every existing diagnostic
line in order, E0382, moved-value/consuming-self rejection and fixture positions.
Do not change fixtures, harness, API, production code, versions or locks, and
do not use broad snapshot overwrite or diagnostic normalization.

Record this amendment and its AGENTS mirror before the correction; regenerate
only docs/spec-index/index/crates-ci.json and docs/spec-index/index/_manifest.json
with the unchanged canonical generator. These four governing paths plus the
three snapshots are the complete seven-path source boundary. Preserve frozen
67b4a3565dfa977e7057908ea46c70474d4398c5 and form a genuine successor through
the existing protected route. Bind its exact ref/tree/bodies and actual fresh
source/profile/helper receipts; do not transfer prior qualification to it.

After fresh admission on the most-recent nightly, run all fourteen unchanged
negative cases once. On success, run one complete scripts/pre-commit.sh from
phase zero with process-local RUSTC_WRAPPER empty under the accepted PCDN-33
mode. Preserve all environment, warning, timeout, capacity, native metadata,
source/profile/default and old-state guards. The focused suite replaces no full
gate. Retain failed results and keep sccache repair open; stop for any new PCDN.
All existing pre-publish, clean Linux/hosted Gate P and protected adoption
prerequisites remain mandatory. Preserve released A2, unmodified Ratatui
0.30.2, parent pins and all other guards. No release, publication or pin
adoption follows from this admission.

Attempt-specific commands, timings, compiler identities and raw failures remain
in retained release evidence. This amendment records only the accepted scope
and prerequisites; it grants no standing maintenance or retry authority.

### 0.1.22 — 2026-10-16 — amended

**Author:** Codex, recording I. Abbott's accepted PCDN-40
**Change kind:** semantic
**Touches:** §5 bounded normal BSP asset preparation correction
**Summary:** Align generated optional C-FFI declarations and calls with concrete
board-local bindings; retain normal asset preparation and release guards.

Permit the owned BSP generator to declare and reference an optional C-FFI module
only when that board's output has the concrete supported module. Preserve the
existing STM32H747I-DISCO bindings and C-HAL calls. Boards without those bindings
retain native Rust initialization, including when the crate-wide c_hal feature
is selected. Keep module declaration and all generated C-call branches
consistent; do not infer bindings from an ancestor directory, synthesize empty
bindings, or broaden C-HAL support. Preserve one-file output behavior.

Bound implementation to src/bin/creator/bsp_gen.rs and its existing inline
tests, src/bin/creator/bsp/templates/board_mod.rs.jinja and
src/bin/creator/bsp/templates/pac.rs.jinja. Document the template capability
boundary in the existing README.md helper section. Require focused checks for
present and absent bindings, retained native and C branches, unchanged concrete
H747 bindings, and generated formatter closure. Do not waive or narrow formatting.

Record this owning amendment and its AGENTS mirror before adopting the
implementation, then regenerate docs/spec-index/index/crates-ci.json and
docs/spec-index/index/_manifest.json with the unchanged canonical generator.
These four governing paths plus the four implementation/documentation paths
are the bounded source change. Preserve frozen candidate98bf768b and its failed
packaging checkout; form a genuine successor and qualify its exact source anew.

Admit the existing requirements.txt prerequisite tomli-w==1.2.0 into a bounded
private packaging Python context. Preserve original interpreter/compiler
profiles, defaults and all earlier setup results; no complete requirements or
setup replay is authorized. Capture actual selected interpreter and dependency
identities. Keep attempt-specific environment and evidence details outside
the normative source amendment.

Retain the failed Stage31 result and all partial evidence. The separate MCU
lookup failure and five explicit H7 PLL-source refusals remain reported/open;
do not invent clock sources or frequencies, change board-failure handling,
edit vendor inputs, or add board skips. Preserve all versions, dependencies,
locks, finite48/42 policy, assets policy, released A2, official Ratatui0.30.2,
vendor and parent pins. No generated asset postimage is adopted by this source
correction.

Require fresh same-ref most-recent-nightly source/profile admission, focused
regressions, all existing full pre-commit and pre-publish checks, clean Linux
and hosted Stage31 with all three consumers, and normal protected adoption.
Preserve process-local empty RUSTC_WRAPPER, the open sccache repair, strict
warnings and diagnostics, timeouts, capacity floor, native/source/profile/
default/old-state custody and every other guard. Previous candidate results
do not qualify this successor. Stop for any new PCDN. No standing retry,
publication, tag, parent adoption, hardware or phase-closure authority follows.

### 0.1.23 — 2026-10-16 — amended

**Author:** Codex, recording I. Abbott's accepted PCDN-41
**Change kind:** semantic
**Touches:** §5 existing shared README receipt admission
**Summary:** Admit only the existing root README checksum for the reviewed
PCDN-40 helper documentation; preserve every other receipt value and guard.

At /workspaces/0/members/0/shared_files/0/sha256 in
scripts/crates_ci_workspace_contexts.json, replace only
c6028e99000b07f20bce08269a27bc539b840dbb51947dc657c2321e707d3960 with
8fa6ca9b1cd6b4077c255ecba1339effeea85e2eac2171bf333d23be6842e836.
This is the reviewed README.md postimage accompanying PCDN-40. Verify that
every other receipt byte and structural value is unchanged. Keep the checker,
workspace identities, descriptor/member/manifest/target definitions, other
shared hashes and finite48/42 contexts exact. No automatic receipt refresh or
standing checksum-maintenance exception is authorized.

Record this amendment and AGENTS mirror before adopting the receipt change;
regenerate the same two canonical outputs with the unchanged generator. The
existing receipt file is the sole additional source path beyond 0.1.22, making
nine paths. Resume the accepted correction and bounded Python prerequisite
only with all existing fresh source/profile, focused, full pre-commit,
pre-publish, clean Linux/hosted Stage31, three-consumer and protected adoption
requirements intact. The checksum admission does not itself qualify packaging.

Preserve all locks, versions, allocations, frozen and failed candidates,
profiles, evidence, A2, official Ratatui0.30.2 and parent/vendor pins. Keep
existing MCU and H7 refusals open and retain every other guard. Stop for any
new PCDN; no publication, tag, parent adoption or phase closure follows.
