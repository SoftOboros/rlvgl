<!--
MPY-01-INTROSPECTION-BASELINE.md - MPY introspection baseline and first actor set.
-->

# MPY-01 — Introspection Baseline

**Status:** Draft 2026-08-09. Not ratified. No MPY parity claim or behavior
implementation is authorized by this draft.

Parent initiative: [MPY-00-CONCEPTS.md](MPY-00-CONCEPTS.md), ratified
2026-08-09.

## 0. Authority Policy

| Concern | Owner | MPY-01 relationship |
|---|---|---|
| MPY ownership, vocabulary, invariants, and phase order | MPY-00 | Used without modification. |
| LVGL source commit, version macros, config defaults, and parity naming | LPAR-01 | MPY-01 inherits the pin and MUST NOT advance it independently. |
| rlvgl object/event/layout/property semantics | LPAR-02, LPAR-04, LPAR-07, LPAR-10, LPAR-15 | Source evidence for current coverage. MPY-01 classifies; it does not change behavior. |
| Current actor inventory | `widgets/src/lib.rs`, `ui/src/lib.rs` | Repository source is canonical for what exists. |
| Scripting-facing coverage matrix and first representative actor set | This document after ratification | MPY-01 owns status vocabulary, matrix rows, and the Wave 1 proof set. |

The exact inherited source target is LVGL 9.4.0-dev at
`5a89ce8a27505389a0e74814fba79db69718512c`, as defined by LPAR-01 §2.
The `library.json` package label does not override the version macros or the
ratified LPAR baseline.

## 1. Purpose

Establish a bounded, reviewable introspection target before protocol or runtime
code lands. MPY-01 inventories the pinned LVGL reference, maps current rlvgl
coverage to MPY-I0 through MPY-I4, chooses the first representative actor set,
and defines what evidence later phases must attach to a parity claim.

## 2. Problem Statement

LPAR-01 records property/introspection as missing at its original baseline, but
later LPAR phases landed adjacent pieces: `ObjectNode`, object events, layout
state, effective bounds, `Queryable`, `PropertyValue`, and `Subject<T>`. The
MicroPython proof remains a fixed `Rect`/`Text` stack. There is no current
matrix distinguishing:

- native behavior that already exists but is not discoverable;
- behavior that is discoverable only through directly held Rust values;
- missing stable identity, schemas, generic construction, and transport;
- optional upstream LVGL property support from required MPY capabilities; or
- an unsupported feature from a feature that is merely not implemented yet.

Without that distinction, later phases could claim parity by wrapping a few
methods while leaving creation, enumeration, liveness, or event schemas absent.

## 3. Canonical Glossary

| Term | Meaning | Relationship |
|---|---|---|
| **MPY Coverage Row** | One versioned claim about a named introspection surface, its source reference, rlvgl implementation status, target profiles, owning phase, and required evidence. | Owned by MPY-01. |
| **Current Coverage** | Required behavior exists and has evidence, but it may still need adapter exposure in a later phase. | MPY-01 status. |
| **Partial Coverage** | Some semantic substrate exists, but one or more required MPY behaviors or evidence surfaces are absent. | MPY-01 status. |
| **Missing Coverage** | Required behavior has no adequate rlvgl implementation. | MPY-01 status. |
| **Unsupported Coverage** | The feature is deliberately unavailable for a named target/profile and the capability response reports that fact. | MPY-01 status; not a synonym for missing. |
| **Deferred Coverage** | The row is valid scope but assigned beyond the current closure target with a named reopen trigger. | MPY-01 status. |
| **Representative Actor Set** | The minimum cross-section of native actors used to prove generic descriptors, creation, properties/actions, events, layout, and composition. | Selected by MPY-01; implemented by MPY-03 onward. |

The coverage-status set is frozen by Specification Required. New statuses
require an MPY-01 amendment so reports do not invent incompatible meanings.

## 4. Source-of-Truth Map

| Concept | Canonical artifact |
|---|---|
| LVGL pin and config assumptions | LPAR-01 §2 |
| MPY-I levels and claim discipline | MPY-00 §7 |
| LVGL class creation/identity | `lvgl/src/core/lv_obj_class.h`, `lvgl/src/core/lv_obj.h` |
| LVGL tree traversal | `lvgl/src/core/lv_obj_tree.h` |
| LVGL event registration/inspection | `lvgl/src/core/lv_obj_event.h` |
| LVGL generic properties and name lookup | `lvgl/src/core/lv_obj_property.h`, `lvgl/lv_conf_template.h` |
| Current object and event substrate | `core/src/object.rs` |
| Current layout substrate | `core/src/layout.rs`, `core/src/widget.rs` |
| Current property substrate | `core/src/property.rs` |
| Native actor inventory | `widgets/src/lib.rs`, `ui/src/lib.rs` |
| Shared API and MicroPython proof | `api/src/lib.rs`, `micropython/src/lib.rs`, `micropython/mp_module.c` |
| Scripting introspection matrix | This document until MPY-09 publishes the closure matrix |

## 5. Frozen Decisions — Baseline and Claim Unit

MPY inherits one source pin from LPAR. An LVGL submodule advance or project
`lv_conf.h` addition requires LPAR-01 amendment before MPY adds claims against
the new source.

The claim unit is one MPY Coverage Row, never a crate-wide or release-wide
statement. Each row contains:

1. stable row ID and MPY-I level;
2. actor type or runtime surface;
3. reference LVGL symbols or an explicit rlvgl-adjacent label;
4. required semantic behavior;
5. current status and target-profile qualifiers;
6. owning MPY phase;
7. deterministic evidence; and
8. intentional differences or unsupported markers.

## 6. Frozen Decisions — Preliminary Runtime Matrix

This draft matrix is the starting classification. Ratification freezes row
scope and status; later phases update status only with cited evidence.

| Row | Level | Surface | Reference | Draft status | Owner |
|---|---|---|---|---|---|
| MPY-BL-001 | MPY-I0 | Enumerate actor types | LVGL class inventory; rlvgl actor catalog | Missing Coverage | MPY-03 |
| MPY-BL-002 | MPY-I0 | Describe type, capabilities, constructors, properties, actions, events, and child policy | LVGL class/property metadata plus rlvgl extensions | Missing Coverage | MPY-03 |
| MPY-BL-003 | MPY-I1 | Generic actor creation | `lv_obj_class_create_obj`, `lv_obj_class_init_obj` | Partial Coverage: fixed `Rect`/`Text` specs exist but runtime calls are placeholders | MPY-03 |
| MPY-BL-004 | MPY-I1 | Stable liveness and stale-handle rejection | `lv_obj_is_valid` | Partial Coverage: detached state exists; no handle registry exists | MPY-02/03 |
| MPY-BL-005 | MPY-I1 | Parent, children, count, index, reorder, and lifecycle | LVGL tree APIs | Partial Coverage: `ObjectNode` semantics exist without stable lookup | MPY-03/04 |
| MPY-BL-006 | MPY-I2 | Named typed property get/set | LVGL property APIs | Partial Coverage: four `PropertyValue` variants and direct `Queryable` access exist without enumeration | MPY-03/04 |
| MPY-BL-007 | MPY-I2 | Discoverable actor actions and typed results | rlvgl-adjacent scripting requirement | Missing Coverage | MPY-03/04 |
| MPY-BL-008 | MPY-I2 | Flags, states, style, and applicability | LVGL object/style state | Partial Coverage: native state exists without generic descriptors | MPY-04 |
| MPY-BL-009 | MPY-I2 | Requested flex/grid/item layout | LVGL layout APIs; LPAR-10 | Partial Coverage: native setters exist without scripting values | MPY-04 |
| MPY-BL-010 | MPY-I2 | Read-only computed geometry | LVGL coordinates; LPAR-10 `effective_bounds` | Partial Coverage | MPY-04 |
| MPY-BL-011 | MPY-I3 | Event catalog and payload schemas | LVGL event vocabulary; `ObjectEvent` | Partial Coverage: event enum exists without descriptors | MPY-05 |
| MPY-BL-012 | MPY-I3 | Add/remove/enumerate subscriptions | LVGL event descriptor APIs | Missing Coverage for scripting tokens and removal | MPY-05 |
| MPY-BL-013 | MPY-I3 | VM-safe callback cues | rlvgl-adjacent dual-core requirement | Missing Coverage | MPY-05/06 |
| MPY-BL-014 | MPY-I4 | Deterministic stage snapshot | rlvgl-adjacent diagnostic requirement | Missing Coverage | MPY-04/07 |
| MPY-BL-015 | MPY-I4 | Capacity, queue, and capability statistics | rlvgl-adjacent embedded requirement | Missing Coverage; current `stats()` is a placeholder | MPY-02/05/08 |

The upstream `LV_USE_OBJ_PROPERTY` default being disabled does not make MPY-I2
optional. MPY requires its own descriptor/property contract whenever the MPY
feature is enabled; it uses upstream as semantic reference, not as a compile
switch inherited into Rust.

## 7. Frozen Decisions — Representative Actor Set

Wave 1 uses five existing native actors:

| Actor | Why selected | Capabilities exercised |
|---|---|---|
| `Container` | Minimal child-owning visual/layout carrier | Creation, child policy, style, requested layout, computed geometry |
| `Label` | Text-bearing leaf with measurement effects | Owned text value, intrinsic measurement, property invalidation |
| `Button` | Basic interactive control | State, click event, callback cue, native default behavior |
| `Slider` | Mutable numeric control | Range/value properties, input-driven change event, typed payload |
| `List` | Composite collection widget | Child composition, actor actions, selection event, snapshot depth |

These are proof actors, not the complete v1 catalog. MPY-03 MAY add actors when
required to validate a capability, but it MUST NOT remove one without an
MPY-01 amendment. Existing `Rect` and `Text` `NodeSpec` variants remain
compatibility evidence; they do not substitute for descriptors on `Container`
and `Label`.

## 8. Frozen Decisions — Evidence and Target Profiles

Every row promoted to Current Coverage MUST cite at least one deterministic
test and the relevant public documentation. Actor/render rows additionally
cite a simulator snapshot or geometry fixture when visible behavior is part of
the claim.

Evidence is recorded separately for:

- language-neutral in-process runtime;
- MicroPython binding on a host-testable MicroPython build;
- simulator presentation when rendering is relevant; and
- CM7/CM4 board transport when MPY-08 claims the row.

A row may be Current Coverage for in-process and Partial Coverage for dual-core.
Reports MUST retain the qualifier instead of collapsing to the best result.

## 9. Frozen Decisions — Phase Invariants

| Invariant | Normative statement | Verification surface |
|---|---|---|
| **INV-MPY-01-1** | MPY parity work MUST use the exact LPAR-01 source pin and config assumptions until LPAR-01 is amended. | Matrix metadata equality test against LPAR-01 §2. |
| **INV-MPY-01-2** | Every MPY coverage claim MUST be one versioned row with level, surface, target qualifier, owner, status, and evidence. | MPY matrix schema validator. |
| **INV-MPY-01-3** | Missing Coverage and Unsupported Coverage MUST remain distinct, and unsupported rows MUST name the rejecting capability/profile. | Matrix status and capability-response tests. |
| **INV-MPY-01-4** | The first generic actor proof MUST include `Container`, `Label`, `Button`, `Slider`, and `List`. | MPY-03 catalog and construction conformance fixture. |
| **INV-MPY-01-5** | A release MUST NOT claim generic full LVGL or introspection parity; it MUST cite closed MPY Coverage Rows. | MPY-09 documentation and release-claim audit. |

## 10. Reconciliation Decisions

| Existing statement | MPY-01 decision |
|---|---|
| LPAR-01 says property/introspection was missing | Historical baseline remains correct. MPY-01 records later partial substrate without rewriting LPAR history. |
| `library.json` says 9.3.0 while version macros say 9.4.0-dev | LPAR-01's ratified macro-based label governs. Package metadata is recorded only as a source discrepancy. |
| `NodeSpec` supports rectangle/text creation | Classify generic creation Partial Coverage, not Current Coverage, because calls are placeholders and the set is fixed. |
| `Queryable` reads/writes named values | Classify named property access Partial Coverage because no enumeration, descriptors, stable actor identity, or detailed errors exist. |
| `ObjectNode` has rich events/layout | Classify those rows Partial Coverage until stable lookup and scripting-safe schemas exist. |
| Optional LVGL property feature | MPY descriptors are required by the MPY feature regardless of upstream template default. |

## 11. Non-Goals and Open Decisions

1. **No behavior implementation.** MPY-01 produces a baseline and tests for its
   schema; it does not add handles, registries, actors, or bindings.
2. **No complete widget catalog.** The five proof actors exercise capability
   classes; remaining actors are admitted incrementally through matrix rows.
3. **No new LVGL pin.** Updating the submodule or config belongs to LPAR-01.
4. **No C ABI mimicry.** Reference symbols scope behavior; they do not dictate
   Rust or Python signatures.

- **PCDN-MPY-01-001:** Should the machine-readable matrix live as committed
  JSON generated from descriptors, Markdown source projected to JSON, or a
  small typed Rust inventory exported by tests? Recommendation: a committed
  neutral schema validated from the same descriptor source MPY-03 selects.
- **PCDN-MPY-01-002:** Should `List` use the low-level `widgets::list::List` or
  the higher-level `ui::List` in the first proof? Recommendation: use the
  low-level actor as canonical and exercise high-level composition separately.

## 12. Acceptance Checklist

- [ ] `INV-MPY-01-1` pins all matrix rows to the inherited LPAR baseline.
- [ ] `INV-MPY-01-2` matrix fields and target qualifiers are accepted.
- [ ] `INV-MPY-01-3` missing-versus-unsupported semantics are accepted.
- [ ] `INV-MPY-01-4` freezes the five representative actors.
- [ ] `INV-MPY-01-5` claim wording and audit ownership are accepted.
- [ ] PCDN-MPY-01-001 and PCDN-MPY-01-002 are resolved without weakening `INV-MPY-3` or `INV-MPY-01-4`.

## 13. Files Cited

- `docs/concepts/MPY-00-CONCEPTS.md`
- `docs/concepts/LPAR-01-BASELINE.md`
- `docs/concepts/LPAR-02-OBJECT-SUBSTRATE.md`
- `docs/concepts/LPAR-04-EVENT-FOCUS-INPUT.md`
- `docs/concepts/LPAR-10-LAYOUT.md`
- `docs/concepts/LPAR-15-CANVAS-MEDIA-PROPERTY-OBSERVER.md`
- `api/src/lib.rs`
- `micropython/src/lib.rs`
- `micropython/mp_module.c`
- `core/src/object.rs`
- `core/src/layout.rs`
- `core/src/property.rs`
- `widgets/src/lib.rs`
- `ui/src/lib.rs`
- `lvgl/src/core/lv_obj_class.h`
- `lvgl/src/core/lv_obj.h`
- `lvgl/src/core/lv_obj_tree.h`
- `lvgl/src/core/lv_obj_event.h`
- `lvgl/src/core/lv_obj_property.h`
- `lvgl/lv_conf_template.h`

## 14. Unblocks

After owner ratification and a committed machine-readable baseline, MPY-01
unblocks MPY-02 protocol implementation and permits later phase documents to
claim only the rows and target profiles they actually close.

## 15. Change Log

### 0.1.0 — 2026-08-09 — Drafted

**Author:** OpenAI Codex with owner direction

**Change kind:** semantic

**Touches:** INV-MPY-01-1, INV-MPY-01-2, INV-MPY-01-3, INV-MPY-01-4, INV-MPY-01-5, INV-MPY-3, INV-MPY-9, PCDN-MPY-005, §0–§14

**Commits:** pending

**Summary:** Drafts the introspection coverage matrix, claim schema, target
qualifiers, and five-actor proof set for the first MPY implementation waves.

#### Rationale

The initiative needs a bounded unit of parity and a cross-section of actors
before it can select IDs, descriptors, or bindings. Separating Current,
Partial, Missing, Unsupported, and Deferred coverage prevents existing native
substrate from being mistaken for a complete scripting surface.
