<!--
LPAR-16-CONFORMANCE-EXAMPLES-DOCS-RELEASE.md — LVGL parity conformance
fixtures, examples, documentation, no-std gates, and release tracking.
-->

# LPAR-16 — Conformance, Examples, Docs, and Release

**Status:** §5 (fixture contract) and §6 (per-phase fixture ledger) ratified
2026-06-14; §10 rule 1 (version policy) decided 2026-06-13. §7–§9 remain
drafted policy. Normative for the LPAR conformance-fixture contract and the
per-phase fixture obligations; fixture execution is unblocked.

Parent initiative: [LPAR-00-CONCEPTS.md](LPAR-00-CONCEPTS.md). Baseline:
[LPAR-01-BASELINE.md](LPAR-01-BASELINE.md). Every implementation phase
(LPAR-02 through LPAR-15) feeds conformance obligations into this phase; the
load-bearing cross-references are the LPAR-08 software-reference tolerance
table ([LPAR-08-TEXT-DRAW-IMAGE-MASK.md](LPAR-08-TEXT-DRAW-IMAGE-MASK.md) §5.H)
and the per-phase determinism invariants enumerated in §6.

## 0. Authority Policy

| Concern | Owner | LPAR-16 relationship |
|---|---|---|
| LVGL reference behavior and the pinned parity baseline | `lvgl/src` submodule @ the LPAR-01 §2 pin (LVGL 9.4.0-dev @ 5a89ce8a) | All parity *claims* a fixture asserts are measured against the pinned commit. LPAR-16 does NOT re-pin or re-classify; it consumes LPAR-01's matrix. A C-reference vector, where used, is derived from this exact commit. |
| Per-phase determinism invariants | The owning phase doc (LPAR-05 §8.1, LPAR-06 §7.2, LPAR-08 §5.F/§5.H, LPAR-09 §5.D, LPAR-10 §4) | Each phase owns the invariant that makes its fixtures reproducible. LPAR-16 enumerates them (§6) and builds fixtures on top; it MUST NOT weaken or restate them. Tightening one requires that phase's §15 amendment first. |
| Pixel tolerance for software-vs-hardware paths | `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` §5.H (jointly owned by LPAR-08 and LPAR-16) | The §5.H tolerance table (exact match for integer-aligned fills/blits; ≤1 px positional, ≤4 value-delta per channel, ≤1% mismatched pixels for AA/scaled/blurred hardware paths) is the conformance oracle. LPAR-16 references it; it does not define a second tolerance. |
| Software-reference oracle | `core/src/draw.rs`, `core/src/renderer.rs` default implementations | The defaulted `Renderer` capability methods (`fill_gradient`, `draw_shadow`, `blend_rect`, `blend_row`, `fill_masked`, `draw_text_shaped`, `blit_image`, `stroke_line_aa`) are the deterministic oracle. Fixtures render through a software `Renderer` test double, not hardware. |
| Existing golden/conformance tests | `core/tests/`, `widgets/tests/`, `examples/.../vectors/` | Repo is canonical for tests already landed (§11). LPAR-16 extends the set per the §6 ledger; it does not rewrite or relocate landed fixtures without naming the move. |
| Public crate release policy | Workspace manifests, `docs/CHANGELOG.md`, `scripts/publish_changed.sh`, the CLAUDE.md Pre-Publish Validation phases | Repo is canonical for the publish gate. LPAR-16 owns the *decision* that the LPAR surface is release-ready and the version/changelog bookkeeping that records it; it does not invent a new publish mechanism. |
| Existing CI conformance discipline | `docs/concepts/CRATES-CI-00`, the CLAUDE.md Pre-Publish Validation section | CRATES-CI owns packaged-crate / headed-surface gates. LPAR-16 conformance fixtures run inside the existing `cargo test --workspace` phase; LPAR-16 MUST NOT fork a parallel CI harness. |

If LPAR-16 changes a frozen decision in §5–§10, §15 MUST be amended first in a
separate docs change. A fixture that would require relaxing a phase's
determinism invariant (§6) is blocked until that phase's §15 is amended. If a
conflict cannot be resolved locally, create `LPAR-16-X.md` per LPAR-00 §0.

## 1. Purpose

LPAR-16 is Wave 6: the conformance, examples, documentation, and release
phase that every prior phase feeds. Its job is not to add widget behavior but
to *prove* the behavior already claimed, package it for users, and make the
LPAR surface releasable. Concretely:

- **Conformance fixtures.** Define one fixture-contract shape and complete the
  per-phase fixture ledger (§6): the deterministic tests, geometry assertions,
  and pixel/behavioral goldens that turn each phase's "parity" claim into
  evidence. Some already landed (LPAR-08); most were explicitly deferred here.
- **Examples.** Ship runnable simulator examples that exercise the new parity
  widgets so users have a working reference, not just unit tests.
- **Docs.** Ensure every new public parity surface carries crate-doc coverage
  and that the LPAR family is discoverable from `docs/concepts/README.md` and
  the changelog.
- **No-std / feature gates.** Lock in compile-gate coverage so the advertised
  `no_std + alloc` contracts (and the optional `std`/feature-gated surfaces)
  cannot silently regress.
- **Changelog / version tracking / release readiness.** Decide the SemVer
  bump for the LPAR-touched crates, write the `docs/CHANGELOG.md` entry, and
  drive the publish dry-run to green.

Per LPAR-00 §6, Wave 6 "runs continuously, but each phase is incomplete until
its LPAR-16 evidence lands." This document is the place that contract is made
explicit and tracked to closure. It does NOT retroactively block already-landed
phases from being called *implemented*; it tracks the *conformance-complete*
level (§5.A) separately.

## 2. Problem Statement

Evidence in the current tree:

### 2.1 The fixture shape was referenced but never defined

Eleven phases cite "LPAR-16 fixtures" or an "LPAR-16-binding" determinism
invariant (enumerated in §6), and LPAR-15 §0 names its conformance-fixtures
dependency as "[LPAR-16 (not yet drafted)]". No document defines what a
conformance fixture *is* structurally — where it lives, what it asserts, what
oracle it renders against, or what counts as "at least one fixture per widget".
The result is a contract every phase defers into and none can close.

### 2.2 Fixtures are landed unevenly

- **LPAR-08 shipped its fixtures** (`docs/concepts/LPAR-08-...md` §15,
  2026-06-13): `core/tests/lpar16_conformance.rs` (gradient + shadow
  determinism), `widgets/tests/scroll_view.rs` (shaped-text clip in a
  `ScrollView` viewport), and `core/tests/image_blit.rs` (recolor alpha
  sweep). These are the reference shape for everything else.
- **A widget golden suite already exists** under `widgets/tests/golden_*.rs`
  (button, checkbox, container, image, label, list, progress, slider) from the
  pre-LPAR widget work — capture-renderer pixel goldens that predate this phase
  but match its intent.
- **Behavioral trace goldens** exist for the disco demo state machine under
  `examples/stm32h747i-disco/disco-demo-states/vectors/` (event/state
  sequences, not pixels).
- **Everything else is deferred here.** LPAR-05 (scroll trajectory),
  LPAR-06 (timer/anim determinism), LPAR-09 (asset/cache fixtures), LPAR-10
  (layout geometry), LPAR-11/12/13/14 (widget pixel goldens), and LPAR-15
  (Canvas/AnimImage/ArcLabel tick-count fixtures) all carry explicit "deferred
  to LPAR-16" notes. The full ledger is §6.

### 2.3 Release bookkeeping has not caught up to the surface

Crate versions are `core` 0.2.3, `widgets` 0.2.3, `ui` 0.2.2, `platform`
0.2.3, `playit` 0.2.4. LPAR-11 through LPAR-15 added a large additive public
surface (≈30 new widget modules, `core::property`, `core::observer`,
`core::edit::EditCore` promotion) with no corresponding version bump or
`docs/CHANGELOG.md` entry. The most recent changelog entry is `v0.2.2`
(CRATES-CI). The release ledger does not yet reflect the LPAR widget waves.

### 2.4 No-std contracts are asserted in prose, not gates

Each phase declares its `no_std`/`alloc`/`std` level (LPAR-00 §5.6), and the
CLAUDE.md Pre-Publish Validation includes embedded-target builds, but there is
no LPAR-specific compile gate asserting that `core` + `widgets` (with the LPAR
additions) still build for `thumbv7em-none-eabihf` without `std`, nor that the
feature-gated surfaces (`lpar_arclabel`, GIF/APNG `std` decode, `png`) are
correctly gated. A `std` leak into the base widget surface would currently
pass host CI.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **Conformance fixture** | A deterministic, checked-in test proving one parity claim. One of four kinds (§5.B): determinism fixture, geometry assertion, pixel golden, behavioral/trace golden. | LPAR-16 |
| **Determinism fixture** | A test that runs the same operation twice (or across a synthetic tick stream) and asserts bit-identical output, plus at least one concrete expected-value assertion so the test fails on a *wrong-but-stable* result, not only on non-determinism. | LPAR-16 (shape); owning phase (invariant) |
| **Geometry assertion** | A test asserting exact integer-computed positions/bounds (glyph placement angle, layout box, snap offset) given fixed input geometry. No rendering required. | LPAR-16 (shape); owning phase (math) |
| **Pixel golden** | A render through a software `Renderer` test double whose output pixel buffer is asserted against an in-code expected buffer (or a checked-in reference), within the LPAR-08 §5.H tolerance. | LPAR-16 + LPAR-08 |
| **Behavioral / trace golden** | A checked-in expected sequence of events/states (not pixels) asserted against a recorded run. The disco-demo `*.golden.trace.txt` vectors are the precedent. | LPAR-16 |
| **Software-reference oracle** | The defaulted `Renderer` capability methods in `core` (deterministic, no RNG, no platform float-order dependency). The canonical output a hardware path must match within tolerance. | LPAR-08 (defn) / LPAR-16 (use) |
| **Tolerance** | The §5.H table: exact for integer-aligned fills/blits; ≤1 px / ≤4 value-delta / ≤1% pixels for AA, scaled, or blurred hardware paths. | LPAR-08 + LPAR-16 |
| **C-reference vector** | A parity value or pixel set captured from the pinned upstream LVGL build, used where Rust output must match C semantics exactly (not appearance). Optional per fixture; used only where it adds signal over an in-code expected value. | LPAR-16 |
| **No-std gate** | A compile-only CI step asserting an LPAR crate/surface builds for an embedded target without `std`, and that feature-gated surfaces are correctly gated. | LPAR-16 |
| **Conformance-complete** | A phase whose §6 fixture obligations have all landed and pass. Distinct from *implemented* (behavior shipped). | LPAR-16 |
| **Release readiness** | The LPAR surface has a ratified SemVer bump, a `docs/CHANGELOG.md` entry, green no-std gates, and a green `DRY_RUN=1` publish dry-run. | LPAR-16 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Fixture-contract shape (the four kinds, location, oracle) | This document, §5 |
| Per-phase fixture obligation + status | This document, §6 (mirrors each phase doc's §12/§15; the phase doc remains canonical for its own invariant) |
| Determinism invariant per phase | The owning phase doc (§6 cites the section) |
| Pixel tolerance | `docs/concepts/LPAR-08-...md` §5.H |
| Software-reference oracle | `core/src/renderer.rs`, `core/src/draw.rs` defaulted methods |
| Landed conformance tests | `core/tests/lpar16_conformance.rs`, `core/tests/image_blit.rs`, `widgets/tests/scroll_view.rs`, `widgets/tests/golden_*.rs` |
| Behavioral trace goldens | `examples/stm32h747i-disco/disco-demo-states/vectors/` |
| No-std / feature contracts | Crate manifests; this document §7 |
| Examples | `examples/` simulator crates; this document §8 |
| Release / version / changelog | Workspace manifests, `docs/CHANGELOG.md`, `scripts/publish_changed.sh`; this document §10 |
| CI execution context | CLAUDE.md Pre-Publish Validation phases; `docs/concepts/CRATES-CI-00` |

## 5. Frozen Decisions — Fixture Contract

### 5.A Two conformance levels

Per the CLAUDE.md "Conformance targets" discipline, LPAR distinguishes:

- **Implemented** — a phase's behavior has shipped and its own colocated unit
  tests pass. This is what the LPAR-11..15 §15 "implementation landed" entries
  assert. It does NOT require LPAR-16 fixtures.
- **Conformance-complete** — a phase's §6 fixture obligations have all landed
  and pass. A phase MAY be implemented without being conformance-complete; the
  two ship on independent timelines (LPAR-00 §6).

A conforming LPAR release (§10) MUST have every phase at *implemented* and
SHOULD have every phase at *conformance-complete*. Phases that remain merely
implemented at release MUST be listed explicitly in the §10 changelog entry as
conformance-deferred, with the §6 row that remains open — no silent gaps.

### 5.B The four fixture kinds

Every LPAR-16 fixture is exactly one of:

1. **Determinism fixture.** Run twice (or over a synthetic tick stream);
   assert `first == second`; AND assert at least one concrete expected value.
   The expected-value assertion is mandatory — a pure self-equality test
   passes on a stably-wrong implementation. `core/tests/lpar16_conformance.rs`
   is the reference (it asserts both `first == second` and the exact 4-pixel
   gradient row).
2. **Geometry assertion.** Assert exact integer positions/bounds from fixed
   input. No renderer. Example: ArcLabel per-glyph angle `Δθ = advance/radius`
   at a known radius/font; a flex/grid computed box; a snap offset.
3. **Pixel golden.** Render through a software `Renderer` test double; assert
   the output buffer against an in-code expected buffer within §5.H tolerance.
   `widgets/tests/golden_*.rs` are the reference shape.
4. **Behavioral / trace golden.** Assert an event/state sequence against a
   checked-in `*.golden.trace.txt`. The disco-demo vectors are the precedent.

### 5.C Location and naming

- Cross-widget / core determinism + image fixtures live in `core/tests/`
  (`lpar16_conformance.rs` and siblings).
- Per-widget pixel goldens and geometry assertions live in `widgets/tests/`
  (`golden_<widget>.rs` for pixel goldens; `<widget>_geometry.rs` or a
  colocated `#[test]` for geometry).
- Behavioral trace goldens live next to the example that produces them, under
  a `vectors/` directory, as the disco demo already does.
- A fixture file header MUST name the phase it discharges (e.g.
  `//! LPAR-09 asset-source conformance fixtures`).

### 5.D The oracle is the software reference

Pixel goldens render through a software `Renderer` test double built on the
defaulted capability methods, NEVER through a hardware path. Hardware/DMA2D
paths are validated separately by asserting they match the software reference
within §5.H tolerance; that is an LPAR-08/platform concern, not a precondition
for an LPAR-16 widget fixture. This keeps fixtures host-runnable and
target-independent.

### 5.E Determinism is inherited, not redefined

Each determinism fixture relies on its phase's invariant (§6) and MUST cite it
in a comment. LPAR-16 adds no new determinism guarantee and weakens none. A
fixture that cannot be made deterministic under the existing invariant is a
signal the invariant is wrong — fix it via the phase's §15, do not add a
tolerance to paper over it (the §5.H tolerance is only for software-vs-hardware
pixel divergence, never for run-to-run instability).

### 5.F C-reference vectors are optional per fixture

A C-reference vector (a value/pixel set captured from the pinned LVGL build) is
used ONLY where matching C *semantics* (not appearance) adds signal an in-code
expected value would not — e.g. a wrapping or rounding edge case where the
intended answer is "whatever LVGL does". Most fixtures assert an in-code
expected value derived from the phase's own spec; that is sufficient and
preferred (it is self-documenting and needs no submodule build to run). When a
C-reference vector IS used, the fixture comment MUST record the capture method
and the pinned commit, and the captured data is checked in (the test MUST NOT
build LVGL at test time).

## 6. Frozen Decisions — Per-Phase Fixture Ledger

Each row is the conformance obligation a phase declared into LPAR-16. "Kind"
is the §5.B classification. "Invariant" cites the owning determinism rule.
Status: **Landed** (fixtures exist and pass), **Open** (deferred here, not yet
written). This ledger is the §12 checklist's backing data; the phase doc
remains canonical for its own invariant.

| Phase | Obligation | Kind | Invariant (owning §) | Status |
|---|---|---|---|---|
| LPAR-01 | Parity claims measured against the pinned LVGL commit | n/a (policy) | LPAR-01 §2 pin | Landed (pin exists) |
| LPAR-05 | Scroll throw/momentum trajectory reproducible for golden-dependent widgets | Determinism | LPAR-05 §8.1 (tick-only px/tick; Tween deceleration) | **Landed** (`core/tests/lpar16_scroll_throw.rs`) |
| LPAR-06 | Timer fire sequence + object-animation value samples bit-identical over a synthetic tick stream | Determinism | LPAR-06 §7.2 (LPAR-16-binding) | **Landed** (`core/tests/lpar16_timer_anim.rs`) |
| LPAR-08 | Shaped-text clip in ScrollView; gradient determinism; shadow-blur determinism; image recolor alpha sweep | Determinism + Pixel | LPAR-08 §5.F oracle, §5.H tolerance | **Landed** (`core/tests/lpar16_conformance.rs`, `widgets/tests/scroll_view.rs`, `core/tests/image_blit.rs`) |
| LPAR-09 | Embedded source lookup (hit+miss); `SlotCache<N>` LRU eviction sequence; `ImageData::Asset` round-trip via `resolve_image` + `MemoryAssetSource` | Determinism + Behavioral | LPAR-09 §5.D (monotonic-u32 LRU; fresh cache per fixture) | **Landed** (`core/tests/lpar16_asset_cache.rs`, `--features fs`). FATFS-over-`SimBlockDevice` prong stays Open (needs `FatfsAssetSource` + std-only `rlvgl-fs-sim`; see §14). |
| LPAR-10 | Identical input geometry + dirty-flag state → identical computed bounds (flex + grid) | Geometry | LPAR-10 §4 (integer-only, no wall clock) | **Landed** (`core/tests/lpar16_layout_geometry.rs`) |
| LPAR-11 | Pixel goldens for Arc, Bar, LED, Line, Spinner, Scale | Pixel + Geometry | LPAR-08 §5.H tolerance | **Landed** (`widgets/tests/lpar16_lpar11_goldens.rs`) |
| LPAR-12 | Pixel goldens / event-dispatch fixtures for ButtonMatrix, ImageButton, Spinbox | Pixel + Behavioral | LPAR-08 §5.H | **Landed** (`widgets/tests/lpar16_lpar12_goldens.rs`) |
| LPAR-13 | Pixel/geometry goldens for Dropdown, Keyboard, Menu, Roller, Tabview, Tileview, Window (Roller snap geometry; Tileview tile positions) | Pixel + Geometry | LPAR-05 snap (`snap_offset_to_points`); LPAR-08 §5.H | **Landed** (`widgets/tests/lpar16_lpar13_goldens.rs`) |
| LPAR-14 | Fixtures for Calendar, Chart, Span, Table, Textarea v2, MessageBox | Pixel + Geometry + Behavioral | LPAR-08 text metrics; §5.H | **Landed** (`widgets/tests/lpar16_lpar14_goldens.rs`) |
| LPAR-15 | ≥1 deterministic tick-count fixture each for CanvasWidget, AnimImage, ArcLabel (ArcLabel = geometry) | Determinism + Geometry | LPAR-06 tick model; ArcLabel `Δθ=advance/radius` | **Landed** (`widgets/tests/lpar16_canvas_anim.rs`, `widgets/tests/lpar16_arc_label_geometry.rs`) |

**Closure rule.** A phase row moves to Landed only when its fixtures (a) exist
under §5.C locations, (b) are exactly one §5.B kind each, (c) cite their §6
invariant, (d) include the mandatory concrete-value assertion for determinism
kinds, and (e) pass under `cargo test`. Marking a row Landed without (a)–(e) is
a discipline violation.

## 7. Frozen Decisions — No-std and Feature Gates

1. **Base surface stays `no_std + alloc`.** `core` and `widgets` (default
   features) MUST build for `thumbv7em-none-eabihf` without `std`. LPAR-16
   adds an explicit compile gate (per CLAUDE.md Pre-Publish Phase 6 lineage):
   `RUSTFLAGS="" cargo build --target thumbv7em-none-eabihf -p rlvgl-core
   -p rlvgl-widgets`.
2. **Feature-gated surfaces are gated both ways.** For each optional surface
   the gate MUST be exercised on AND off: `lpar_arclabel` (widget present iff
   enabled), GIF/APNG `std` decode, `png` canvas export, and any Lottie/3D
   gate if those LPAR-Optional surfaces land. `cargo test -p rlvgl-widgets
   --no-default-features` is the floor; the enabled-feature pass is the
   `--features lpar_arclabel` (and siblings) run.
3. **No `std` creep.** A new `use std::` in `core`/`widgets` default paths is a
   conformance failure even if host CI is green; the embedded compile gate
   (rule 1) is the enforcement.
4. **Gates run in the existing workspace test phase.** No new CI runner; these
   are added commands in the CLAUDE.md Pre-Publish Validation block (§10).

## 8. Frozen Decisions — Examples

1. **At least one simulator example exercises the new parity widgets.** Users
   need a runnable reference, not only `#[test]` capture renderers. The example
   builds under `rlvgl-example-sim` (host) — no hardware required.
2. **Examples are demonstrative, not exhaustive.** One coherent screen
   composing a representative subset (e.g. a settings-style screen using
   `Dropdown`, `Roller`, `Spinbox`, `Tabview`, `Chart`) is sufficient; a
   per-widget gallery is OPTIONAL and MAY be deferred.
3. **Examples MUST compile in the existing example test phase** (CLAUDE.md
   Phase 4) so they cannot bit-rot silently.
4. **Examples are not conformance fixtures.** They demonstrate; they do not
   assert goldens. A widget's evidence is its §6 fixture, not its appearance in
   an example.

## 9. Frozen Decisions — Documentation

1. **Every new public parity item has a doc comment.** Already enforced
   per-crate by `#![deny(missing_docs)]` on `widgets`; LPAR-16 makes the
   `cargo doc --workspace --no-deps` pass (CLAUDE.md Phase 5) a named release
   gate for the LPAR surface.
2. **The LPAR family is discoverable.** `docs/concepts/README.md` lists every
   LPAR phase with its status; LPAR-16 is added there on ratification, and the
   index is updated to *conformance-complete* status as §6 rows close.
3. **Migration/difference notes ship with the widget.** Per LPAR-00 §5.5, every
   intentional Rust-vs-LVGL API difference is documented in the phase doc and
   the shipped item docs. LPAR-16 does not re-document these; it verifies they
   exist for the release-noted surface.
4. **No per-widget tutorial requirement.** Long-form guides (disco-tutorial
   style) are OUT of LPAR-16 scope; crate docs + one example + the concepts
   docs are the documentation deliverable.

## 10. Frozen Decisions — Changelog, Version, Release

1. **SemVer bump reflects additive surface; stay on the `0.2.x` line.**
   LPAR-11 through LPAR-15 added a large *additive* public API (new modules,
   `core::property`, `core::observer`, `core::edit::EditCore`) with no removals
   or breaking changes to landed APIs. **Decision (owner, 2026-06-13): the LPAR
   release continues the `0.2.x` line targeting `0.2.4`; no `0.3.0`.** Pre-1.0,
   additive surface rides a patch-level bump. Concretely: the crates that
   gained LPAR surface — `core` (0.2.3 → 0.2.4), `widgets` (0.2.3 → 0.2.4), and
   `ui` (0.2.2 → 0.2.4) — bump to `0.2.4`. `playit` is already `0.2.4`.
   `platform` (0.2.3) bumps to `0.2.4` only if its surface changed in the LPAR
   waves; otherwise it stays. "Minor semver bumps where needed" — only the
   crates whose public surface actually moved get bumped; untouched crates are
   left alone.
2. **One `docs/CHANGELOG.md` entry for the LPAR widget waves.** The entry
   enumerates the new widget modules and core surfaces by phase, names any
   phase that ships *implemented but not conformance-complete* (§5.A) with its
   open §6 row, and records the version bump from rule 1.
3. **Release readiness = all gates green.** The LPAR surface is release-ready
   when: every §6 row is Landed OR explicitly changelog-noted as deferred; the
   §7 no-std gates pass; `cargo doc --workspace --no-deps` passes; and
   `DRY_RUN=1 scripts/publish_changed.sh HEAD~1` is green. These are the
   existing CLAUDE.md Pre-Publish phases; LPAR-16 names them as the release
   gate, it does not add a publish mechanism.
4. **No publish rides on an open invariant.** Per LPAR-00 §5.7 and the
   Spec-Before-Code execution discipline, a release that leaves a §6 row open
   MUST say so in the changelog; it MUST NOT silently claim conformance.

## 11. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| `core/tests/lpar16_conformance.rs` | The reference determinism fixture (LPAR-08). LPAR-16 extends this file (or adds siblings) for other core determinism rows; it is not rewritten. |
| `widgets/tests/golden_*.rs` (button, checkbox, container, image, label, list, progress, slider) | Pre-LPAR capture-renderer pixel goldens that already match the §5.B kind 3 shape. New widget goldens follow the same file pattern; existing ones are not relocated. |
| `widgets/tests/scroll_view.rs`, `core/tests/image_blit.rs` | LPAR-08 driving-case fixtures. Canonical examples of the shaped-text-clip and recolor patterns; reused as templates. |
| `examples/.../disco-demo-states/vectors/*.golden.trace.txt` | The behavioral/trace-golden precedent (§5.B kind 4). New trace goldens follow this layout. |
| `docs/concepts/CRATES-CI-00` + CLAUDE.md Pre-Publish Validation | The CI execution context. LPAR-16 fixtures and gates run inside these existing phases; no parallel harness is created. |
| `docs/CHANGELOG.md` | The single workspace changelog. The LPAR release entry is added here, continuing the existing format (most recent: `v0.2.2`). |
| ANIM-00 / REND-00 determinism guarantees | Inherited. LPAR-05/06 fixtures rely on the ANIM tick model and REND clip contract; LPAR-16 does not re-assert those initiatives' own goldens. |

## 12. Acceptance Checklist

LPAR-16 is **conformance-complete** when:

### 12.A Fixture contract (this document)

- [x] §5 (fixture contract) and §6 (ledger) ratified with a dated §15 entry
      (2026-06-14). §7–§9 remain drafted policy; §10 rule 1 decided.
- [x] The four fixture kinds (§5.B), the location/naming convention (§5.C),
      the software-reference oracle rule (§5.D), and the inherited-determinism
      rule (§5.E) are stable and referenced by at least the LPAR-08 landed
      fixtures.

### 12.B Per-phase fixture ledger (§6 closure)

- [x] LPAR-08 — shaped-text clip, gradient/shadow determinism, recolor sweep
      (already Landed; recorded for completeness).
- [x] LPAR-05 — scroll trajectory determinism fixture. Landed 2026-06-14.
- [x] LPAR-06 — timer + object-animation determinism over a synthetic tick
      stream. Landed 2026-06-14.
- [~] LPAR-09 — embedded lookup, SlotCache LRU sequence, `ImageData::Asset`
      round-trip Landed 2026-06-14 (`--features fs`). FATFS-over-`SimBlockDevice`
      prong still Open (needs `FatfsAssetSource` + std-only `rlvgl-fs-sim`; §14).
- [x] LPAR-10 — flex + grid computed-bounds geometry assertions.
      Landed 2026-06-14.
- [x] LPAR-11 — Arc, Bar, LED, Line, Spinner, Scale goldens. Landed 2026-06-14.
- [x] LPAR-12 — ButtonMatrix, ImageButton, Spinbox fixtures. Landed 2026-06-14.
- [x] LPAR-13 — Dropdown, Keyboard, Menu, Roller (snap geometry), Tabview,
      Tileview (tile positions), Window fixtures. Landed 2026-06-14.
- [x] LPAR-14 — Calendar, Chart, Span, Table, Textarea v2, MessageBox
      fixtures. Landed 2026-06-14.
- [x] LPAR-15 — CanvasWidget (draw-sequence determinism), AnimImage
      (tick-stream determinism), ArcLabel (exact arc geometry, gated
      `lpar_arclabel`). Landed 2026-06-14.

### 12.C No-std / feature gates (§7)

- [x] `core` + `widgets` (default features) build for
      `thumbv7em-none-eabihf` without `std`.
- [x] `cargo test -p rlvgl-widgets --no-default-features` passes.
- [x] Each feature-gated surface is exercised both on and off as applicable:
      default/off gates plus `rlvgl-core --all-features` for PNG/JPEG/GIF/APNG/
      canvas/fatfs/fs/nes/fontdue/qrcode and `rlvgl-widgets
      --features lpar_arclabel` for ArcLabel. Lottie runtime backends and 3D
      media remain deferred-Optional per §14.

### 12.D Examples (§8)

- [x] At least one host-simulator example composes a representative subset of
      the new parity widgets and builds in the example test phase.

### 12.E Docs (§9)

- [x] `cargo doc --workspace --no-deps` passes for the LPAR surface.
- [x] `docs/concepts/README.md` lists LPAR-16 and reflects per-phase
      conformance status.

### 12.F Release (§10)

- [x] SemVer bump applied: `core`/`widgets`/`ui` → `0.2.4` (stay on `0.2.x`
      per §10 rule 1); `platform` → `0.2.4` only if its surface moved.
- [x] `docs/CHANGELOG.md` entry written, naming any conformance-deferred phase
      with its open §6 row.
- [x] `DRY_RUN=1 scripts/publish_changed.sh HEAD~1` is green for the committed
      cleanup. Reran 2026-06-14 against commit `e5b8956`: the commit-diff
      script resolved the full changed-crate set in publish order — `core`,
      `playit`, `platform`, `widgets`, `ui`, `fs-sim`, `app-demo`,
      `app-disco-demo`, `rlvgl` — and exited 0. (The `DRY_RUN=1` path resolves
      diff + ordering and skips actual `cargo package`/`publish`; a real
      release run does the packaging, and `playit` packaging is independently
      covered by Pre-Publish Phase 3.)

### 12.G Initiative retrospective

- [x] `docs/concepts/LPAR-RETROSPECTIVE.md` is written per the CLAUDE.md
      Initiative-Retrospective discipline (§1–§8 shape) at initiative
      completion.

## 13. Files Cited

- `docs/concepts/LPAR-00-CONCEPTS.md` — initiative shape; Wave 6 contract (§6),
  conflict gates for visual-golden determinism and hardware tolerance (§9).
- `docs/concepts/LPAR-01-BASELINE.md` — pinned LVGL commit/version (§2);
  parity matrix.
- `docs/concepts/LPAR-05-SCROLL-RUNTIME.md` §8.1 — tick-only scroll
  determinism invariant.
- `docs/concepts/LPAR-06-TIMERS-OBJECT-ANIM.md` §7.2 — timer/animation
  determinism invariant (LPAR-16-binding).
- `docs/concepts/LPAR-08-TEXT-DRAW-IMAGE-MASK.md` §5.F, §5.H — software-
  reference oracle and tolerance table.
- `docs/concepts/LPAR-09-ASSET-FILESYSTEM.md` §5.D — LRU eviction determinism.
- `docs/concepts/LPAR-10-LAYOUT.md` §4 — layout geometry determinism.
- `docs/concepts/LPAR-11-PRIMITIVE-WIDGETS.md` §9, LPAR-12/13/14/15 §12/§15 —
  per-widget fixture deferrals.
- `core/tests/lpar16_conformance.rs` — landed gradient/shadow determinism.
- `core/tests/image_blit.rs` — landed recolor alpha sweep.
- `widgets/tests/scroll_view.rs` — landed shaped-text-clip driving case.
- `widgets/tests/golden_*.rs` — landed widget pixel goldens (precedent shape).
- `examples/stm32h747i-disco/disco-demo-states/vectors/` — behavioral trace
  golden precedent.
- `core/src/renderer.rs`, `core/src/draw.rs` — defaulted capability methods
  (the software-reference oracle).
- `docs/CHANGELOG.md`, `scripts/publish_changed.sh` — release bookkeeping.
- CLAUDE.md Pre-Publish Validation — the CI phases LPAR-16 gates run inside.

## 14. Unblocks / Deferred

- **Unblocks now:** writing conformance fixtures for any phase in §6 against a
  defined contract; closing the "[LPAR-16 (not yet drafted)]" dependency in
  LPAR-15 §0; planning the LPAR release entry and version bump.
- **Deferred — Safe:** per-widget galleries beyond the one required example
  (§8.2); long-form tutorials (§9.4). Orthogonal to conformance.
- **Deferred — Coupled:** C-reference vector capture tooling (§5.F) — only
  needed for the subset of fixtures where C-semantics matching adds signal;
  coupled to a reproducible LVGL build step. Most fixtures use in-code expected
  values and do not need it.
- **Deferred — Optional:** pixel goldens for the LPAR-Optional media widgets
  (`Lottie`, `DashLottie`, `Texture3d`) — coupled to those surfaces landing
  (LPAR-15 §12.B), which is itself deferred-Optional.

## 15. Change Log

- **2026-06-14** — LPAR-16 closeout. The validation cleanup slice was committed
  (`e5b8956`) and `DRY_RUN=1 scripts/publish_changed.sh HEAD~1` reran green
  against it (full changed-crate set resolved in publish order, exit 0),
  closing the last open §12.F box. Every §12 acceptance box is now checked
  except the deferred-Coupled LPAR-09 FATFS-over-`SimBlockDevice` prong
  (§12.B `[~]`, §14). LPAR-16 is **conformance-complete** at the §5.A bar:
  every phase is *implemented*, and every phase is *conformance-complete*
  except LPAR-09's explicitly-deferred FATFS prong. The LPAR initiative is
  closed; the retrospective is `LPAR-RETROSPECTIVE.md`.
- **2026-06-14** — Validation cleanup slice. Fixed release-gate regressions
  found by `/pre-publish`: `core::asset` clippy simplifications, host-vs-
  embedded cfg gating in the STM32H747I-DISCO crate, Playit drag validation
  using simulator stderr status instead of stale footer pixels, and the
  generic simulator QR toggle baseline. Added
  `rlvgl_app_demo::build_lpar_parity_demo` plus a simulator test so §8 is a
  concrete LPAR widget composition rather than a legacy-demo build. Aligned
  publishable LPAR crates and demo app dependencies to `0.2.4`
  (`core`/`platform`/`widgets`/`ui`/`fs-sim`/`app-demo`/`app-disco-demo`;
  `playit` dependency constraint updated). Gates run: fmt, clippy workspace,
  workspace tests, Playit std/no-std/package checks, simulator build/tests,
  creator feature tests, core/widget feature gates, docs, `make build-disco`,
  and `DRY_RUN=1 scripts/publish_changed.sh HEAD~1` for the committed
  LPAR head (`d3d0c4d`, changed crate `rlvgl-widgets`). Residual drift: docs
  pass but rustdoc emits broken/private intra-doc-link warnings; embedded build
  passes but remains warning-heavy; the publish dry-run must be rerun after the
  validation cleanup is committed so the commit-diff script sees the cleanup
  itself.
- **2026-06-13** — LPAR-16 drafted. Defines the conformance-fixture contract
  (§5: four fixture kinds, location/naming, software-reference oracle,
  inherited-determinism rule, optional C-reference vectors), the per-phase
  fixture ledger (§6, harvested from every LPAR-01..15 doc's LPAR-16
  references — LPAR-08 Landed, all others Open), the no-std/feature gate set
  (§7), the example (§8) and documentation (§9) requirements, and the
  changelog/version/release-readiness policy (§10: minor bump for
  core/widgets/ui, concrete number deferred to ratification). Reconciles
  against the existing `golden_*` widget tests, the disco-demo trace goldens,
  and the CRATES-CI / Pre-Publish CI context (no parallel harness). Frozen
  decisions: §5.A two conformance levels (implemented vs conformance-complete),
  §5.B fixture taxonomy, §5.D oracle-is-software-reference, §6 closure rule,
  §7 no-std enforcement via embedded compile gate, §10 SemVer policy. Not
  ratified; fixture/release execution is blocked until owner ratification is
  recorded here.
- **2026-06-14** — Widget pixel-golden wave: LPAR-11/12/13/14 rows Landed
  (exhaustive — one §5.B kind-3 golden per widget, 22 widgets total).
  `widgets/tests/lpar16_lpar11_goldens.rs` (Arc, Bar, LED, Line, Spinner,
  Scale), `…lpar12_goldens.rs` (ButtonMatrix, ImageButton, Spinbox),
  `…lpar13_goldens.rs` (Dropdown, Keyboard, Menu, Roller, Tabview, Tileview,
  Window), `…lpar14_goldens.rs` (Calendar, Chart, Span, Table, Textarea,
  MessageBox). Each renders through a `BufferDisplay`-backed software renderer
  (the §5.D oracle; capability methods — `fill_arc_aa`, `stroke_line_aa`,
  `blit_image`, `blend_rect` — routed into the buffer where a widget's visible
  output flows through them), asserts render-twice determinism, a concrete
  expected pixel/color anchor, and non-triviality. Built via four parallel
  workers (one per phase, disjoint files). Gates: fmt clean; clippy
  `-p rlvgl-widgets --all-targets` clean; full widgets suite green (0
  failures across all test binaries). §6 ledger + §12.B updated. With this
  wave every §6 row is Landed except the deferred-Coupled LPAR-09
  FATFS-over-`SimBlockDevice` prong (§14).
- **2026-06-14** — Substrate fixture wave: LPAR-05, LPAR-06, LPAR-10 rows
  Landed and LPAR-09 partially Landed. `core/tests/lpar16_timer_anim.rs`
  (timer fire sequence `[31,50,100,150]`; frozen linear/easeout Tween tables;
  object-anim applied sequence `[10,20,…,100]` — all replayed for
  determinism, LPAR-06 §7.2). `core/tests/lpar16_scroll_throw.rs` (a fixed
  drag-then-fling script replays bit-identical; monotonic EaseOut momentum
  settling exactly at the `1800` max-scroll clamp, LPAR-05 §8.1).
  `core/tests/lpar16_layout_geometry.rs` (exact flex row-gap-grow
  `[80|gap10|210]` and grid `1fr/2fr → [100|200]` rects, re-run for
  determinism, LPAR-10 §5.F integer-only). `core/tests/lpar16_asset_cache.rs`
  (gated `#![cfg(feature = "fs")]`: embedded hit+miss, `SlotCache<2>` LRU
  eviction, `MemoryAssetSource` round-trip — LPAR-09 §5.D). The
  FATFS-over-`SimBlockDevice` prong stays Open (deferred-Coupled on an
  unimplemented `FatfsAssetSource` + the std-only `rlvgl-fs-sim` crate; §14).
  Each fixture is exactly one §5.B kind and cites its §6 invariant.
  **Release-gate note (feeds §7/§10):** the `fs`-gated fixture needs a
  `cargo test -p rlvgl-core --features fs` run in addition to the default
  `cargo test -p rlvgl-core`; default-feature runs compile it empty. Gates:
  fmt clean; clippy `-p rlvgl-core --all-targets` clean both default and
  `--features fs`; full core suite green both feature sets. §6 ledger +
  §12.B updated.
- **2026-06-14** — LPAR-15 fixture row Landed (first slice under the ratified
  contract). `widgets/tests/lpar16_canvas_anim.rs` (CanvasWidget
  draw-sequence determinism + concrete pixel values; AnimImage 12-tick-stream
  determinism + concrete frame sequence `[1,2,3,0]×3`) and
  `widgets/tests/lpar16_arc_label_geometry.rs` (gated `lpar_arclabel`; exact
  per-glyph arc origins `[(250,150),(248,165),(244,181)]` for radius 100 /
  16 px advance / 0° CW — libm `cosf`/`sinf` truncations are host-stable).
  Each fixture is exactly one §5.B kind and cites its §6 invariant. §6 ledger
  + §12.B updated; LPAR-15 §12.C closed. Gates: fmt clean; clippy
  `-p rlvgl-widgets --all-targets` clean both with and without
  `lpar_arclabel`; widgets suite green both feature sets.
- **2026-06-14** — §5 (fixture contract) and §6 (per-phase fixture ledger)
  ratified by owner instruction ("5&6 ratified"). Fixture execution is
  unblocked. The four fixture kinds, location/naming convention, software-
  reference oracle, inherited-determinism rule, and the per-phase ledger
  (LPAR-08 Landed; the rest Open with their owning invariants cited) are now
  normative. §7–§9 (no-std gates, examples, docs) remain drafted policy and
  §10 awaits its remaining rows; these do not block fixture work. First slice:
  LPAR-15 Canvas/AnimImage/ArcLabel tick-count/geometry fixtures.
- **2026-06-13** — §10 version decision recorded (owner): the LPAR release
  continues the `0.2.x` line targeting `0.2.4`; no `0.3.0`. `core`/`widgets`
  (0.2.3) and `ui` (0.2.2) bump to `0.2.4`; `playit` already `0.2.4`;
  `platform` bumps only if its surface moved. "Minor semver bumps where
  needed" — untouched crates are left alone. This settles the §10 rule 1
  ratification decision; the rest of the document remains drafted-not-ratified
  (the fixture contract §5/§6 still awaits an owner ratification entry before
  fixture execution begins).
