<!--
docs/spec-index/README.md - Ownership, scope, and commands for the rlvgl documentation-object index.
-->

# rlvgl Documentation Object Index

`rlvgl` owns a deterministic index of its documentation corpus. The parent
Softoboros index intentionally excludes submodules; a standalone `rlvgl`
checkout therefore generates and verifies this projection locally.

The scanner reads Markdown under `docs/` and `chipdb/`, excluding generated,
vendored, archived, and nested-upstream trees. Generated JSON lives in
`docs/spec-index/index/` and must not be edited by hand.

The current corpus is a baseline. Structural findings from
`spec-index-report` are diagnostic and do not claim that legacy documents
conform to a newer document grammar. Existing documents are not rewritten as
part of indexing work.

rlvgl has adopted the current Softoboros SBC form for new and materially
amended multi-phase documentation. The local migration boundary and explicit
legacy bridges are in [SBC-COMPATIBILITY.md](SBC-COMPATIBILITY.md); named
family-prefix ownership is in [PREFIX-REGISTRY.md](PREFIX-REGISTRY.md). Those
operational records keep the committed local projection useful without
silently promoting legacy documentation to a newer status.

Commands:

- `make spec-index` regenerates the committed JSON projection.
- `make spec-index-check` fails when regeneration would change that projection.
- `make spec-index-report` prints structural findings without changing files.
- `make spec-suspect` derives dependency suspicion from the index and Git history.
- `make spec-test` runs parser, determinism, and local non-vacuity tests.

Changes to indexed documentation should regenerate the local index and run
`make spec-test spec-index-check`. Parent-repository dashboard ingestion and
cross-repository conformance policy are owned outside this subrepo.
