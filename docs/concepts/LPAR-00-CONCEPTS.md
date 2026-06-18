<!--
LPAR-00-CONCEPTS.md - LVGL parity initiative plan and dependency analysis.
-->

# LPAR-00 — LVGL Parity Initiative Concepts

**Status:** Ratified 2026-06-12. Normative for the LPAR initiative
(LVGL parity waves, phase order, dependency gates, and conflict
policy).

Requesting ticket: "promote outstanding LVGL parity TODO list into a
spec-before-code initiative broken down into phases and waves and
analyzed for dependencies and conflict" (2026-06-12).

## 0. Authority Policy

| Concern | Owner | LPAR relationship |
|---|---|---|
| LVGL reference behavior | `lvgl/src` submodule | Source reference for parity vocabulary, widget family names, and behavior classes. LPAR-01 MUST pin the exact submodule commit and LVGL version target before implementation phases claim parity. |
| Existing Rust object and widget traits | `core/src/widget.rs`, `core/src/lib.rs`, `widgets/src/lib.rs` | Repo is canonical for current API. LPAR phases MAY extend these surfaces only through explicit phase gates and SemVer review. |
| Renderer and raster contracts | `core/src/renderer.rs`, `core/src/draw.rs`, `core/src/raster.rs`, `platform/src/blit.rs` | Repo is canonical for backend behavior. LPAR draw/display phases reconcile LVGL semantics with existing renderer adapters and hardware paths. |
| Existing concepts initiatives | `docs/concepts/ANIM-00-CONCEPTS.md`, `REND-00-CONCEPTS.md`, `INPUT-00-CONCEPTS.md`, `WID-00-CONCEPTS.md` | These are already ratified and landed. LPAR MUST treat them as inherited constraints unless their §15 logs are amended first. |
| Public crate release policy | Workspace manifests, `docs/CHANGELOG.md`, publish scripts | LPAR phases that touch publishable crates MUST include version/changelog/release tracking work before publish. |

If an LPAR implementation phase changes a frozen decision in this
document, the §15 change log MUST be amended first in a separate docs
change. If a phase conflict cannot be resolved locally, create a
sub-letter analysis doc named `LPAR-NN-X.md` and fold the resolution
back into this document before code lands.

## 1. Purpose

Turn the outstanding LVGL parity backlog into a multi-wave initiative
that can be implemented without silently forking the runtime, style,
layout, draw, widget, and verification contracts. The goal is not to
clone LVGL's C internals; it is to make rlvgl's Rust APIs cover the same
user-visible widget and runtime behavior where that behavior makes sense
for `no_std`, allocator-backed embedded targets and simulator backends.

LPAR is deliberately broad. It exists to sequence work so the widget
surface does not outrun the object/event/style/draw substrate needed to
make those widgets behave consistently.

## 2. Problem Statement

Evidence in the current tree:

- `widgets/src/lib.rs` exports a focused first-party set: button,
  checkbox, click area, clock, container, image, label, list, meters,
  motion, progress, radio, scroll view, slider, and switch.
- `ui/src/lib.rs` provides higher-level wrappers and application UI
  elements, including editable `Input`/`Textarea` from WID-01, but it is
  not an LVGL-wide widget compatibility layer.
- `lvgl/src/widgets` contains many upstream widget families not yet
  represented in rlvgl: arc, bar, buttonmatrix, calendar, canvas,
  chart, dropdown, imagebutton, keyboard, led, line, menu, msgbox,
  roller, scale, span, spinbox, spinner, table, tabview, tileview,
  window, and media-specialized widgets.
- Existing concepts initiatives already solved pieces of the substrate:
  ANIM provides deterministic tick-driven animation, REND provides
  rectangular clipping and `ScrollView`, INPUT provides drag
  recognition, and WID provides editable text. LPAR needs to integrate
  these rather than replace them.
- The original TODO list was ordered but did not express phase gates,
  parallelizable waves, conflict points, or sub-letter triggers.

## 3. Glossary

| Term | Meaning | Owner |
|---|---|---|
| **LPAR** | The LVGL Parity initiative family. Owns this sequencing document and future LPAR phase docs. | LPAR |
| **Parity baseline** | A pinned LVGL submodule commit, target LVGL version, config assumptions, and current/partial/missing matrix. Does not exist yet; LPAR-01 owns it. | LPAR |
| **Object substrate** | Core object-tree behavior equivalent to LVGL `lv_obj`: parent/child ownership, flags, hit testing, invalidation, focus, events, and lifecycle. Current rlvgl `WidgetNode`/`Widget` are related but narrower. | LPAR-02/03/04 |
| **Runtime substrate** | Cross-widget services: invalidation, event propagation, focus groups, input devices, timers, and animation binding. | LPAR-02 through LPAR-06 |
| **Style substrate** | Selector/state/part style resolution, inherited properties, transitions, themes, and widget default styles. | LPAR-07 |
| **Draw substrate** | Renderer primitives, masks, image descriptors, display buffer semantics, and optional accelerator hooks. | LPAR-08/09 |
| **Layout substrate** | Sizing primitives plus LVGL-like flex and grid behavior. | LPAR-10 |
| **Parity widget** | A widget whose documented behavior maps to an LVGL widget family, even if the Rust API differs. | LPAR-11 through LPAR-15 |
| **Wrapper collision** | A conflict between an existing rlvgl name/API and an LVGL parity name/API, such as `Progress` vs `Bar` or `Modal`/`Alert` vs `MessageBox`. | LPAR |
| **Conformance fixture** | A deterministic test, golden image, geometry assertion, or C-reference vector proving a parity claim. | LPAR-16 |

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| Initiative phase order, dependency rules, conflict policy | This document |
| Original backlog | `docs/todo/TODO-LVGL-PARITY.md` |
| LVGL reference inventory | LPAR-01 output, derived from `lvgl/src` |
| Current Rust public widget inventory | `widgets/src/lib.rs`, `ui/src/lib.rs` |
| Existing landed constraints | ANIM-00, REND-00, INPUT-00, WID-00 |
| Phase acceptance evidence | Per-phase implementation PRs and LPAR-16 conformance fixtures |
| Release visibility | `docs/CHANGELOG.md`, crate manifests, publish dry-run output |

## 5. Frozen Decisions — Initiative Shape

1. **LPAR is a multi-wave initiative, not a single ticket.** No
   implementation PR may claim "LVGL parity" generically. It MUST cite a
   specific phase code and the subset of the baseline matrix it closes.
2. **Baseline before behavior.** LPAR-01 MUST land before any new
   implementation phase claims parity with an LVGL widget or runtime
   subsystem. Exploratory code may exist, but it does not count as LPAR
   conformance until the baseline is pinned.
3. **Substrate before broad widgets.** Widget phases that need object,
   style, layout, text, draw, or focus behavior MUST wait for the owning
   substrate phase or explicitly document a narrower v1 contract.
4. **Existing ratified initiatives are inherited.** ANIM, REND, INPUT,
   and WID remain authoritative for their surfaces. LPAR phases that
   need to alter their invariants MUST amend those docs before code.
5. **Rust API differences are allowed only when documented.** Parity is
   user-visible behavior plus migration documentation, not C API
   mimicry. Every intentional difference goes in the phase doc and the
   shipped widget docs.
6. **No hidden `std` creep.** Each new parity surface MUST declare
   whether it is `no_std`, `alloc`, or `std` only. Publishable crates
   MUST keep their advertised feature contracts.
7. **Conflict-first execution.** If a phase has a named conflict in
   §8/§9, the first PR in that phase is either a small docs resolution
   or a code change that stays inside the already-resolved option. No
   large implementation PR may decide the conflict implicitly.

## 6. Frozen Decisions — Waves

| Wave | Phase range | Goal | Parallelism rule |
|---|---:|---|---|
| **Wave 0 — Baseline and policy** | LPAR-01 | Pin reference LVGL target, naming rules, conformance levels, and inventory matrix. | Serial; all later waves depend on it. |
| **Wave 1 — Runtime substrate** | LPAR-02 through LPAR-06 | Object semantics, invalidation, events, focus, input devices, gestures, scroll, timers. | LPAR-02 is first. LPAR-03/04/05 may run in parallel after LPAR-02 interfaces are fixed. |
| **Wave 2 — Style, draw, layout substrate** | LPAR-07 through LPAR-10 | Style cascade, text/font, draw/mask/image/display, sizing/flex/grid. | LPAR-07/08/10 can run in parallel after LPAR-02 and LPAR-03; text work inside LPAR-08 gates text-heavy widgets. |
| **Wave 3 — Foundational widget parity** | LPAR-11 through LPAR-12 | Low-composition and existing-neighbor widgets: Arc, Bar, LED, Line, Spinner, Scale, ImageButton, ButtonMatrix, Spinbox. | Parallel by widget if each consumes settled style/draw/event contracts and owns separate files. |
| **Wave 4 — Composite and data widgets** | LPAR-13 through LPAR-14 | Calendar, Chart, Dropdown, Keyboard, Menu, MessageBox, Roller, Span, Table, Tabview, Textarea v2, Tileview, Window. | Parallel only after the shared dependencies each widget declares are landed. |
| **Wave 5 — Canvas, media, property, observer** | LPAR-15 | Canvas, AnimImage/Lottie/3DTexture/ArcLabel scope, property/introspection, observer/data-binding. | May split into sub-waves after LPAR-01 classifies scope. |
| **Wave 6 — Conformance, examples, docs, release** | LPAR-16 | C-reference vectors, visual goldens, examples, no-std gates, docs, changelog and release tracking. | Runs continuously, but each phase is incomplete until its LPAR-16 evidence lands. |

## 7. Frozen Decisions — Phase Plan

| Phase | Wave | Scope | Depends on | Conflict gates |
|---|---:|---|---|---|
| **LPAR-01 Baseline Matrix** | 0 | Pin LVGL commit/version/config; record current/partial/missing matrix; define Rust-vs-LVGL naming policy; define conformance levels. | None | Naming collisions for `Bar`/`Progress`, `Textarea`, `MessageBox`, layout names. |
| **LPAR-02 Object Substrate** | 1 | Parent/child ownership, screen roots, sibling order, flags, hit testing, hidden/disabled/clickable state, deletion lifecycle. | LPAR-01 | `WidgetNode` compatibility; public trait break risk. |
| **LPAR-03 Invalidation and Display Runtime** | 1 | Dirty area propagation, invalid rectangle merge, display buffer/flush semantics, rotation/full/partial refresh. | LPAR-02 | Existing blitter planners, REND dirty contract, platform display APIs. |
| **LPAR-04 Event, Focus, and Input Runtime** | 1 | Event codes, bubbling/trickling, focus groups, pointer/keypad/encoder/button devices, long press/repeat/gesture events. | LPAR-02 | INPUT/WID key routing; app-level key handlers; event enum growth policy. |
| **LPAR-05 Scroll Runtime** | 1 | Scroll flags, scrollbars, nested scroll, snapping, chaining, throw/momentum, scroll begin/end. | LPAR-03, LPAR-04, REND-00 | Existing `ScrollView` v1 contract and drag recognizer suppression. |
| **LPAR-06 Timers and Object Animations** | 1 | LVGL-like timers, pause/resume/repeat, object-bound animations, transition timer integration. | LPAR-02, ANIM-00 | Existing `core::anim` vs legacy `animation.rs` naming and semantics. |
| **LPAR-07 Style and Theme Substrate** | 2 | Parts/states/selectors, local/shared style lists, inherited properties, property reset/removal, transitions, default theme chaining. | LPAR-02, LPAR-06 for transitions | Existing `core::style` and `ui::style` overlap; theme API compatibility. |
| **LPAR-08 Text, Draw, Image, and Mask Substrate** | 2 | Glyph metrics, wrapping, bidi/RTL policy, draw primitives, masks, gradients, shadows, image descriptors/cache/recolor/transform. | LPAR-03, LPAR-07 for style-driven draw | Renderer trait expansion risk; REND text clipping limitation; accelerator paths. |
| **LPAR-09 Asset and Filesystem Sources** | 2 | Embedded/FATFS/simulator/memory asset lookup, LVGL-like image/file source conventions, cache policy. | LPAR-08 | Existing asset pipeline and plugin source conventions. |
| **LPAR-10 Layout Substrate** | 2 | Percent/content sizing, min/max constraints, align helpers, size-change events, flex layout, grid layout. | LPAR-02, LPAR-07 | Existing `ui::layout` helpers and creator Qt emit assumptions. |
| **LPAR-11 Primitive Widget Wave** | 3 | `Arc`, `Bar`, `LED`, `Line`, `Spinner`, `Scale`. | LPAR-07, LPAR-08, LPAR-10 as needed | `Progress` vs `Bar`; meter/scale overlap. |
| **LPAR-12 Control Widget Wave** | 3 | `ButtonMatrix`, `ImageButton`, `Spinbox`, keyboard navigation for controls. | LPAR-04, LPAR-07, LPAR-08 | Existing `Button`, `Checkbox`, `Radio`, `Switch`, WID input routing. |
| **LPAR-13 Selection and Navigation Widgets** | 4 | `Dropdown`, `Keyboard`, `Menu`, `Roller`, `Tabview`, `Tileview`, `Window`. | LPAR-04, LPAR-05, LPAR-07, LPAR-10 | Focus/input conflicts; existing `Drawer`, `Modal`, app navigation. |
| **LPAR-14 Data and Rich Content Widgets** | 4 | `Calendar`, `Chart`, `Span`, `Table`, `Textarea` v2, `MessageBox`. | LPAR-04, LPAR-07, LPAR-08, LPAR-10 | `Modal`/`Alert` vs `MessageBox`; WID `Textarea` compatibility; text metrics. |
| **LPAR-15 Canvas, Media, Property, Observer** | 5 | `Canvas`, `AnimImage`, `Lottie`, `3DTexture`, `ArcLabel`, typed property layer, introspection, observer/data-binding. | LPAR-08, LPAR-09, LPAR-16 fixture shape | Plugin feature boundaries; creator/playit introspection ownership. |
| **LPAR-16 Conformance, Examples, Docs, Release** | 6 | C-reference vectors, visual goldens, examples, no-std gates, per-widget docs, changelog/version tracking. | LPAR-01; each phase feeds it | Test runtime cost; visual determinism; release ordering. |

### 7.1 Original Backlog Mapping

| TODO item(s) | Owning phase | Notes |
|---|---|---|
| 1-2 | LPAR-01 | Baseline, LVGL target, naming policy, conformance levels. |
| 3 | LPAR-02 | Object semantics and lifecycle. |
| 4, 18 | LPAR-03 | Invalidation plus display buffer/flush semantics. |
| 5-9 | LPAR-04 | Event expansion, propagation, focus groups, input devices, pointer gestures. |
| 10 | LPAR-05 | Scroll container parity beyond REND-00. |
| 11-14 | LPAR-07 | Style cascade, properties, transitions, themes. |
| 15-17, 19-20 | LPAR-08 | Text/font, draw primitives, masks, image descriptors, accelerator hooks. |
| 21 | LPAR-09 | Filesystem and asset source parity. |
| 22 | LPAR-06 | Timers and object-bound animations. |
| 23-25 | LPAR-10 | Sizing, flex, and grid layout parity. |
| 26-27 | LPAR-15 | Property/introspection and observer/data-binding scope. |
| 28-29, 36-37, 41, 44 | LPAR-11 | Primitive visual widgets: Arc, Bar, LED, Line, Scale, Spinner. |
| 30, 34, 43 | LPAR-12 | Control widgets: ButtonMatrix, ImageButton, Spinbox. |
| 33, 35, 38, 40, 46, 48-49 | LPAR-13 | Selection/navigation widgets: Dropdown, Keyboard, Menu, Roller, Tabview, Tileview, Window. |
| 31-32, 39, 42, 45, 47 | LPAR-14 | Data/rich-content widgets: Calendar, Chart, MessageBox, Span, Table, Textarea v2. |
| 50-51 | LPAR-15 | Canvas and media-specialized widgets. |
| 52-57 | LPAR-16 | Examples, C-reference tests, visual goldens, feature gates, docs, release tracking. |

## 8. Dependency Analysis

| Dependency | Why it matters | Blocks |
|---|---|---|
| LPAR-01 before all parity claims | Without a pinned LVGL target, "parity" is unbounded and review arguments become subjective. | All implementation phases |
| Object substrate before event/style/layout/widgets | Flags, parts, states, lifecycle, and parentage are the common vocabulary for almost every LVGL widget. | LPAR-03 through LPAR-15 |
| Invalidation before display and scroll | Scroll, animation, and display flush behavior need a common dirty-region model. | LPAR-03, LPAR-05, LPAR-06, visual tests |
| Event/focus/input before control widgets | Dropdown, roller, keyboard, spinbox, tabview, and textarea v2 all depend on focus and key/encoder routing. | LPAR-12 through LPAR-14 |
| Style cascade before widget parts | LVGL widgets expose part/state styling; implementing widgets first would hard-code appearance and require rewrites. | LPAR-11 through LPAR-15 |
| Text metrics before text-heavy widgets | Label wrapping, textarea cursor placement, span, table cells, calendar labels, and chart ticks need consistent metrics. | LPAR-08, LPAR-13, LPAR-14 |
| Draw primitives before arc/scale/spinner/chart | These widgets need arcs, lines, masks, gradients, and anti-aliased primitives. | LPAR-11, LPAR-14 |
| Layout substrate before composite widgets | Menus, tabs, windows, table, keyboard, calendar, and dropdown need stable sizing and placement. | LPAR-13, LPAR-14 |
| Asset/image substrate before media widgets | ImageButton, Canvas, AnimImage, Lottie, and 3DTexture need common source/cache/format rules. | LPAR-12, LPAR-15 |
| Conformance fixtures throughout | Every phase needs evidence before it can be called done; LPAR-16 is not a last-minute cleanup. | All phases |

## 9. Conflict Analysis

| Conflict | Risk | Resolution policy |
|---|---|---|
| `Widget`/`WidgetNode` public shape vs object parity | Adding object semantics directly to existing traits can break consumers that construct nodes by literal or implement `Widget`. | LPAR-02 MUST prefer additive adapters or extension traits first. Any breaking trait change requires explicit SemVer and release planning. |
| `Renderer` trait stability vs draw parity | Masks, transforms, gradients, and text metrics may require new renderer capabilities. | LPAR-08 MUST separate required core methods from optional fast paths and keep default fallbacks deterministic. |
| REND `ClipRenderer` text limitation vs LVGL text clipping | REND deliberately drops partially visible backend text lines because glyph extents are unavailable. | LPAR-08 owns glyph metrics and text clipping. Do not bypass REND by widget-local hacks. |
| WID explicit activation vs LVGL focus groups | WID v1 uses `set_active(bool)` without framework focus. LVGL-style widgets expect focus groups. | LPAR-04 MUST preserve WID APIs while adding focus-group routing. WID behavior changes require WID-00 amendment. |
| INPUT drag suppression vs LVGL scroll/gesture events | Existing recognizers suppress clicks after drag; LVGL scroll widgets need scroll begin/end/throw semantics. | LPAR-05 MUST compose with `DragRecognizer` and define cancellation/event ordering. |
| `Progress` vs LVGL `Bar` | Existing Rust widget may overlap but not match LVGL Bar modes and parts. | LPAR-01 naming policy decides alias, wrapper, or new `bar` module before LPAR-11 code. |
| `Modal`/`Alert` vs LVGL `MessageBox` | Existing UI wrappers may satisfy part of message-box behavior but have different APIs. | LPAR-14 decides whether `MessageBox` wraps existing types or becomes a separate parity widget. |
| `ui::layout` helpers vs LVGL flex/grid | Existing helpers are static placement utilities; LVGL layouts are object-managed. | LPAR-10 MUST preserve helper APIs while adding or isolating LVGL-compatible layout engines. |
| Theme/style duplication across `core` and `ui` | Current style types exist in multiple crates; LVGL part/state cascade may force shared vocabulary. | LPAR-07 decides ownership and re-export policy before broad widget styling. |
| Custom rlvgl widgets vs parity widgets | Audio meters, motion widgets, drawers, badges, tags, and toasts are useful but not LVGL parity targets. | LPAR docs MUST label them as adjacent/custom. They do not block parity unless they share substrate contracts. |
| `no_std` footprint vs LVGL feature breadth | Some LVGL widgets naturally need allocation, file IO, or heavy media support. | Every phase declares feature level. Heavy widgets MAY be optional and gated; core crates MUST keep intended `no_std` contracts. |
| Creator/playit introspection ownership | Property and observer systems may serve runtime, tests, and generated UI. | LPAR-15 MUST assign ownership before adding cross-crate property APIs. |
| Visual golden determinism vs animation/transitions | LVGL-style transitions and spinners are time-dependent. | LPAR-06/16 MUST use deterministic ticks and frozen frame points for tests. |
| Hardware acceleration vs software reference behavior | DMA2D or future GPU paths may produce subtly different pixels. | LPAR-08/16 MUST define acceptable tolerance or require software-reference fallbacks for parity tests. |

## 10. Reconciliation vs Adjacent Repo Primitives

| Primitive | Relationship |
|---|---|
| ANIM-00 `Tween`/`Animations` | LPAR-06 consumes this as the deterministic animation substrate. LPAR does not reintroduce wall-clock animation. |
| REND-00 `ClipRenderer`/`ScrollView` | LPAR-05 expands scroll semantics; LPAR-08 expands text/mask clipping. Existing REND v1 guarantees stay intact until amended. |
| INPUT-00 `DragRecognizer` | LPAR-04/05 build higher-level input-device and scroll/gesture events above it. |
| WID-00 `Input`/`Textarea` | LPAR-04 adds focus routing around WID; LPAR-14 owns textarea v2 features. |
| `widgets/src/meters` | Adjacent non-LVGL widgets. Scale/chart work may reuse draw primitives but MUST NOT force meter API churn without a separate reason. |
| `ui/src/modal.rs` / `alert.rs` / `drawer.rs` | Application UI wrappers. MessageBox/Window/Menu parity may wrap or coexist with them after naming policy resolution. |
| `src/bin/rlvgl_creator` and Qt emit docs | Layout/property/introspection phases may affect generated code. Creator-facing changes MUST be listed in phase acceptance. |

## 11. Non-Goals

- No promise of C ABI compatibility with LVGL.
- No automatic clone of all upstream LVGL internals or private data
  structures.
- No guarantee every LVGL widget ships in the same crate or feature set.
- No breaking change to existing public Rust APIs without an explicit
  phase conflict resolution and release plan.
- No requirement that custom rlvgl widgets become LVGL-compatible.
- No hardware-accelerated backend requirement for parity. Software
  reference behavior is sufficient unless a phase says otherwise.

## 12. Acceptance Checklist

LPAR-00 is accepted when:

- [ ] The TODO backlog is linked to this initiative and no longer stands
      as the only source of phase ordering.
- [ ] LPAR-01 through LPAR-16 are listed with dependencies and conflict
      gates.
- [ ] Each original TODO item is mapped to at least one phase.
- [ ] Known conflicts with ANIM, REND, INPUT, WID, existing widgets,
      style/layout APIs, and renderer backends are named.
- [ ] `docs/concepts/README.md` lists LPAR as an active draft
      initiative.

Individual implementation phases are accepted only when their phase
docs define:

- [ ] Scope, non-goals, crate ownership, and public API compatibility.
- [ ] Dependency prerequisites and conflict resolutions.
- [ ] Tests or conformance fixtures, including `no_std`/feature gates
      where relevant.
- [ ] Documentation and release-version updates for publishable crates.

## 13. Files Cited

- `docs/todo/TODO-LVGL-PARITY.md` — original ordered backlog
- `lvgl/src/widgets` — LVGL widget family inventory
- `widgets/src/lib.rs` — current first-party widget exports
- `ui/src/lib.rs` — current high-level UI exports
- `core/src/widget.rs` — current widget trait and tree primitives
- `core/src/renderer.rs` — renderer trait and text/draw constraints
- `docs/concepts/ANIM-00-CONCEPTS.md` — animation substrate
- `docs/concepts/REND-00-CONCEPTS.md` — clipping/scroll v1 substrate
- `docs/concepts/INPUT-00-CONCEPTS.md` — drag recognizer substrate
- `docs/concepts/WID-00-CONCEPTS.md` — editable text substrate

## 14. Unblocks / Deferred

- **Unblocks now:** turning parity requests into phase-scoped tickets;
  assigning parallel waves without mixing substrate and widget work;
  using `LPAR-NN` prefixes for future spec-before-code PRs.
- **Deferred — Safe:** exact widget-by-widget API designs. Those live
  in the owning phase docs after LPAR-01 pins naming policy.
- **Deferred — Coupled:** renderer trait expansion, focus manager
  design, style ownership, and property/observer ownership. These are
  explicitly named conflict gates and must not be decided incidentally.
- **Deferred — Optional:** media-heavy widgets (`3DTexture`, full
  Lottie parity, animated image parity) may become optional conformance
  levels if LPAR-01 determines they are outside the embedded baseline.

## 15. Change Log

- **2026-06-12** — LPAR-00 drafted from
  `docs/todo/TODO-LVGL-PARITY.md`. Defines waves 0-6, phases
  LPAR-01 through LPAR-16, dependency analysis, conflict analysis,
  sub-letter trigger policy, and acceptance requirements. Not ratified.
- **2026-06-12** — LPAR-00 ratified by owner instruction
  ("LPAR-00 RATIFIED"). Wave 0 / LPAR-01 unblocked.
