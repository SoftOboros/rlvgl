<!--
docs/crates-ci/README.md — Crates-Built Headed-Surface CI initiative index.
Informative. The normative artifact is CRATES-CI-00-CONCEPTS.md.
-->

# Crates-Built Headed-Surface CI (CRATES-CI)

CI today builds the workspace in-tree and headless. The two things a
downstream user actually receives — published crates, and the two headed
surfaces (the `rlvgl-creator` GUI wrapper and the windowed simulator) —
are never exercised. The v0.2.1/v0.2.2 cycle paid for that gap in
release-repair commits (`58bb737`, `bc69338`, `4d9255a`, `9bee2f9`).

This initiative adds two gates and two harness layers:

- **Gate P** — pre-publish: `cargo package` every publishable crate,
  build throwaway Consumer Projects against the packaged set (Staged
  Registry). Blocks the release tag.
- **Gate R** — post-publish: the same consumers against real crates.io,
  scheduled.
- **Layer K** — `egui_kittest` in-process tests of `CreatorApp`
  (widgets, wizards, snapshots), no display server.
- **Layer W** — the playit wire protocol served over TCP by
  `rlvgl-creator --automation-headless`, executed against the kittest
  engine, driven by the existing `playit/node` client.

## Conformance

- A conforming **release** MUST pass Gate P on the release ref before
  tagging and MUST be verified by Gate R after publishing
  (CRATES-CI-00 §5, §12).
- A conforming **GUI Wrapper change** MUST keep Layer K green and, once
  CRATES-CI-04 lands, Layer W green (CRATES-CI-00 §7).
- Consumer Projects MUST use registry sources only — no `path`/`[patch]`
  into the workspace (INV-C2).

## Documents

| Doc | Status |
|---|---|
| [CRATES-CI-00-CONCEPTS.md](CRATES-CI-00-CONCEPTS.md) | RATIFIED 2026-06-10 (§15 amendments through CRATES-CI-01a) |
| [CRATES-CI-RETROSPECTIVE.md](CRATES-CI-RETROSPECTIVE.md) | Drafted 2026-06-10 at initiative completion |

## Phases — all shipped 2026-06-10

| Phase | Deliverable |
|---|---|
| CRATES-CI-00 | Concepts doc ratified; CLAUDE.md amended |
| CRATES-CI-01 (+01a) | Gate P harness + `crates-ci.yml` + `consumers/creator-cli/`; P-META + version-drift gates |
| CRATES-CI-02 (+02a) | `consumers/user-sim/` — user-built simulator from crates, playit + golden PNG; P-INCLUDE fix |
| CRATES-CI-03 | Layer K — kittest harness for `CreatorApp` |
| CRATES-CI-04 | Layer W — playit-over-kittest TCP server on `rlvgl-creator` |
| CRATES-CI-05 | Gate R post-publish/scheduled workflow |
| CRATES-CI-06 | [Initiative retrospective](CRATES-CI-RETROSPECTIVE.md) |

The initiative completed its acceptance event the same day: v0.2.2 was
published through Gate P (INV-C1) and verified green by Gate R against
live crates.io. §6 forward constraints in the retrospective are binding
on future initiatives.

Commit-subject prefix: `CRATES-CI-NN[a-z]:`.
