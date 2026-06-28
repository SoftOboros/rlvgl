<!--
00-concepts.md - QT-00: initiative concepts, vocabulary, and registration policies.
-->

**[Index](README.md) · [Next →](#)** *(QT-01b not yet authored)*

# Chapter QT-00 — Concepts

This chapter is the initiative's **normative** entry point. Per the
spec-before-code planning discipline in [`CLAUDE.md`](../../CLAUDE.md),
all later QT-NN chapters and behaviour PRs cite this document for
vocabulary, frozen enumerations, and registration policies. The
sibling [`README.md`](./README.md) is informative — it links here for
authority.

## §0 — Authority Policy

Normative keywords (**MUST**, **MUST NOT**, **SHALL**, **SHOULD**,
**SHOULD NOT**, **MAY**, **RECOMMENDED**) are interpreted per RFC 2119
and RFC 8174. Capitalisation matters: lowercase use is ordinary
English.

External authority for terms used in this initiative:

| Term family                                   | Authoritative source                          | Relationship |
| --------------------------------------------- | --------------------------------------------- | ------------ |
| QML grammar (imports, `Item { }`, properties, signals, handlers, grouped/attached properties) | Qt 6.x [QML Reference](https://doc.qt.io/qt-6/qmlreference.html) | reference — used without modification |
| `QMetaObject` introspection surface           | Qt 6.x QObject docs                           | reference — used without modification |
| RFC 2119 / 8174 keywords                      | IETF                                          | reference — used without modification |
| `qt-ir` IR types and serialisation            | this chapter and its successors               | owned here   |
| Initiative-and-phase commit prefix `QT-NN[a-z]:` | this chapter (§8)                          | owned here   |
| Registration policy for IR enums              | this chapter (§5–§7)                          | owned here   |

External documents (Qt manuals, Qt Quick Reference, Design Studio
docs) are **not** vendored into this repo. The IR is the only
contract; whatever upstream Qt accepts at the QML layer is what users
author, and the ingest path's job is to lower a sensible subset of
that into `qt-ir.json`.

## §1 — Purpose

QT-00 ratifies the vocabulary that all later phases use. It does not
introduce behaviour by itself. After ratification, behaviour PRs:

- **MUST** use the glossary terms in §3 with the meanings frozen here.
- **MUST** cite the registration policy in §5–§7 before adding values
  to a frozen enumeration.
- **MUST** use the commit prefix in §8.

## §2 — Problem Statement

Without a ratified vocabulary, the four ingest paths in
`rlvgl-creator` (Chakra, Svelte, BSP YAML, Qt) drift apart. Concrete
prior incidents:

- The Chakra TS parser at [`src/bin/creator/chakra.rs`](../../src/bin/creator/chakra.rs)
  emits `tokens.yaml`. The Svelte token pipeline at
  [`src/bin/creator/svelte.rs`](../../src/bin/creator/svelte.rs) consumes
  it. Neither names its tokens "design tokens" canonically; the term
  appears in [`docs/SVELTE-DESIGN-TOKEN-ALIGNMENT.md`](../SVELTE-DESIGN-TOKEN-ALIGNMENT.md)
  but not in the source comments. A Qt ingest path that introduces
  yet another synonym ("Qt theme tokens", "QML resources") would fork
  the vocabulary across three sources for a single `tokens.yaml`.
- The BSP family rejected this exact failure mode by ratifying the
  chipdb vocabulary up front (see
  [`docs/bsp/CHIP-SUPPORT.md`](../bsp/CHIP-SUPPORT.md)). QT-00 mirrors
  that pattern.

The MVP at [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)
already declares concrete IR types. QT-00 freezes the names so the
forthcoming emit-side phases (`QT-03`, `QT-04`) cannot silently rename
them.

## §3 — Canonical Glossary

Each term is annotated per the discipline's reference / restatement /
chapter-owned convention.

### `qt-ir`

The serde-serializable JSON IR produced by `rlvgl-creator qt ingest`.
**Owned by this chapter; lives in
[`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs); used without
modification.** Schema version pinned at `1`. The version is bumped
under the Specification-Required policy (§7).

### `UiModule`

The top-level structure of one parsed `.qml` file. As defined in
[`src/bin/creator/qt.rs:34`](../../src/bin/creator/qt.rs); used
without modification. Fields: `version: u32`, `source: String`,
`imports: Vec<UiImport>`, `pragmas: Vec<String>`, `root: UiItem`.

### `UiImport`

`import <module> [<version>] [as <alias>]`. As defined in
[`src/bin/creator/qt.rs:43`](../../src/bin/creator/qt.rs); used
without modification.

### `UiItem`

A QML type instance — `Item { ... }`, `Rectangle { ... }`. As defined
in [`src/bin/creator/qt.rs:51`](../../src/bin/creator/qt.rs); used
without modification. The `children: Vec<UiItem>` field carries
nested items; sibling collections (`properties`, `assignments`,
`signals`, `handlers`) carry the per-item member lists.

### `UiProperty`

A `[default] [readonly] property <ty> <name>[: <expr>]` declaration.
As defined in [`src/bin/creator/qt.rs:65`](../../src/bin/creator/qt.rs);
used without modification. Default values are stored as opaque
expression text, not evaluated.

### `UiAssignment` and `UiAssignmentValue`

A `target: <value>` binding inside an item body. The target is the
literal text of the LHS (e.g. `width`, `anchors.fill`,
`font.pixelSize`); dotted targets are preserved verbatim. As defined
in [`src/bin/creator/qt.rs:76`](../../src/bin/creator/qt.rs); used
without modification. The value side is the frozen enumeration in §6.

### `UiSignal`, `UiSignalParam`

A `signal name(<ty> <name>, ...)` declaration. As defined in
[`src/bin/creator/qt.rs:95`](../../src/bin/creator/qt.rs); used without
modification.

### `UiHandler`

An `onSignal: ...` binding. As defined in
[`src/bin/creator/qt.rs:109`](../../src/bin/creator/qt.rs); used
without modification. Bodies are stored as opaque text. Function
declarations (`function name(args) { ... }`) are recorded with a
`signal` field of `function:<name>` — adapted: this is a creator-side
convention not present in QML; the alternative was a separate
`UiFunction` type, deferred to QT-04.

### Authoring source vs. emit target

- **Authoring source**: a designer-facing input that creator ingests
  into IR. The four current sources are Chakra TS theme files, Svelte
  token YAML, BSP `.ioc` / chipdb YAML, and (with QT-01a) `.qml`
  files.
- **Emit target**: a creator-output crate or codegen surface. The
  current targets are the rlvgl Rust runtime, the dual-mode assets
  crate, per-vendor BSPs, and the chakra/svelte token outputs.

Qt ingestion adds an authoring source; it does **not** add an emit
target. Lvglpp emission is tracked in a separate thread and is not
in scope for this initiative.

### Phase, chapter, gate

- **Phase**: a numbered milestone in the §5 phase set, prefixed
  `QT-NN`. Lettered suffixes (`QT-04b`) extend an existing phase
  without renumbering.
- **Chapter**: a normative concepts doc owning one phase's
  vocabulary and frozen decisions.
- **Gate** (or *acceptance gate*): the pass/fail criterion at the end
  of a chapter's §12 acceptance checklist.

## §4 — Source-of-Truth Map

| Concept                                | Owner (canonical source)                                       |
| -------------------------------------- | -------------------------------------------------------------- |
| `qt-ir` schema types and field names   | [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs)         |
| `qt-ir` schema **version**             | `QT_IR_VERSION` const in the same file; semantic policy §7     |
| QML grammar accepted by the parser     | [`src/bin/creator/qt.rs::Parser`](../../src/bin/creator/qt.rs) |
| Phase set membership                   | §5 of this chapter                                             |
| Registration policy per enum           | §5–§7 of this chapter                                          |
| External-tool dependency catalogue     | [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md)         |
| User-facing `qt …` CLI surface         | [`docs/creator/CLI.md`](../creator/CLI.md)                     |
| Sibling Chakra / Svelte vocabulary     | [`src/bin/creator/{chakra,svelte}.rs`](../../src/bin/creator/) |
| BSP / chipdb vocabulary                | [`chipdb/`](../../chipdb/) and [`docs/bsp/`](../bsp/)         |

When a Qt-related decision touches a row owned outside this chapter
(for example, where a Qt theme funnels into `tokens.yaml`), the
owning concepts doc takes precedence. QT-NN amendments referencing
those rows **MUST** be co-ordinated with the owner; see §8.

## §5 — Frozen Enumeration: Phase Set

Registration policy: **Standards Action**. Adding a phase requires a
change-log amendment to this chapter and explicit agreement from the
initiative owner.

| Phase     | Title                                       | Concepts doc                              |
| --------- | ------------------------------------------- | ----------------------------------------- |
| `QT-00`   | Concepts (this doc)                         | `00-concepts.md`                          |
| `QT-01a`  | Structural QML ingest (MVP)                 | shipped — see [`../creator/QT-INGEST.md`](../creator/QT-INGEST.md) |
| `QT-01b`  | Type-introspection ingest                   | TBA — `01b-type-introspection.md`         |
| `QT-02`   | `qt-ir` schema freeze (JSON Schema export)  | [`02-ir-schema.md`](./02-ir-schema.md) — ratified |
| `QT-03`   | rlvgl emitter — data-only Rust shape        | [`03-rlvgl-emitter-widgets.md`](./03-rlvgl-emitter-widgets.md) — ratified |
| `QT-03b`  | rlvgl emitter — widget API mapping          | [`03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — ratified; implementation pending |
| `QT-03c`  | rlvgl emitter — anchor resolver             | [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) — ratified; initial implementation shipped (`anchors.centerIn`) |
| `QT-03d`  | rlvgl emitter — image asset wiring          | TBA                                         |
| `QT-04`   | rlvgl emitter — signal handlers             | [`04-signal-handlers.md`](./04-signal-handlers.md) — ratified; initial implementation shipped |
| `QT-04b`  | rlvgl emitter — properties + bindings       | [`04b-properties-bindings.md`](./04b-properties-bindings.md) — ratified; implementation shipped |
| `QT-04c`  | rlvgl emitter — initial-value text bindings | [`04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) — ratified; implementation shipped |
| `QT-04d`  | rlvgl emitter — MouseArea support           | [`04d-mousearea.md`](./04d-mousearea.md) — ratified; implementation shipped (onClicked; hover deferred) |
| `QT-04e`  | rlvgl emitter — reactive bindings (Label text) | [`04e-reactive-bindings.md`](./04e-reactive-bindings.md) — ratified; implementation shipped — closes out QT-04 family |
| `QT-04f`  | rlvgl emitter — nested ID resolution        | [`04f-nested-id-resolution.md`](./04f-nested-id-resolution.md) — ratified; implementation shipped |
| `QT-05`   | State machines (istate-codegen linkage)     | [`05-state-machines.md`](./05-state-machines.md) — ratified; concepts only, implementation pending in QT-05a-e |
| `QT-05a`  | scjson side-file ingest                     | [`05a-scjson-ingest.md`](./05a-scjson-ingest.md) — ratified + shipped |
| `QT-05b`  | rlvgl emit — handler dispatch glue          | [`05b-handler-dispatch.md`](./05b-handler-dispatch.md) — ratified + shipped |
| `QT-05c`  | rlvgl emit — DataModel + State bindings     | [`05c-machine-bindings.md`](./05c-machine-bindings.md) — ratified + shipped |
| `QT-05d`  | QML `States {}` → scjson emit               | [`05d-emit-scjson.md`](./05d-emit-scjson.md) — ratified + shipped |
| `QT-05e`  | Externals stub emission (closes QT-05)      | [`05e-externals-stubs.md`](./05e-externals-stubs.md) — ratified + shipped — closes the QT-05 family |
| `QT-05g`  | rlvgl emit — state-predicate → Image bindings; istate linkage v2 | [`05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md) — ratified + shipped (Play/Pause live on ESP32-P4; `QT-05f` reserved for stateful-externals) |
| `QT-05h`  | rlvgl emit — state-predicate → widget visibility; mute region remodel | [`05h-visibility-bindings.md`](./05h-visibility-bindings.md) — ratified + shipped (mute visibility live on ESP32-P4) |
| `QT-05i`  | rlvgl emit — chained state-predicate → Image bindings (repeat icon); shuffle/repeat region remodel | [`05i-chained-predicate-bindings.md`](./05i-chained-predicate-bindings.md) — ratified + shipped (repeat live on ESP32-P4) |
| `QT-05j`  | rlvgl emit — `submitBtnSetupEvent` → tap-target table (shuffle tap wired; consumer owns the QML→machine map) | [`05j-button-event-bindings.md`](./05j-button-event-bindings.md) — ratified + shipped (host-verified) |
| `QT-05k`  | rlvgl emit — `text: <obj>.<prop>` external-object Label text → `Binding::ExternalText` + `apply_external_text` (consumer owns the key→value resolver) | [`05k-external-text-bindings.md`](./05k-external-text-bindings.md) — ratified + shipped (host-verified; P4 caption pending bench round) |
| `QT-06`   | Theme-token reconciliation                  | [`06-theme-tokens.md`](./06-theme-tokens.md) — ratified + shipped |
| `QT-07`   | Asset-crate handoff                         | [`07-asset-handoff.md`](./07-asset-handoff.md) — ratified + shipped |
| `QT-08`   | CLI surface (`creator qt …`)                | [`08-multi-file-cli.md`](./08-multi-file-cli.md) — directory-mode shipped; `.qmldir` (`QT-08b`) and `.qrc` (`QT-08c`) deferred |
| `QT-08b`  | `.qmldir` resolution                        | [`08b-qmldir-resolution.md`](./08b-qmldir-resolution.md) — ratified + shipped |
| `QT-08c`  | `.qrc` resource manifests                   | [`08c-qrc-resources.md`](./08c-qrc-resources.md) — ratified + shipped |
| `QT-09`   | Desktop-UI integration                      | [`09-desktop-ui.md`](./09-desktop-ui.md) — ratified + shipped (creator and creator_ui feature builds both verified) |
| `QT-10`   | Strict-mode acceptance + release tag        | [`10-release.md`](./10-release.md) — ratified; strict-mode-1 shipped (auto-mode half); release tag is user-driven |

Lettered suffixes (`QT-NNa`, `QT-NNb`) **MAY** be added under the
same phase number without re-amendment, provided the new sub-phase
preserves all invariants of the parent. Otherwise file a new phase
number.

## §6 — Frozen Enumeration: `UiAssignmentValue` Kinds

Registration policy: **Specification Required**. Adding a kind
requires an amendment to the chapter that owns the assignment-value
semantics (currently this one; later ownership transfers to QT-02).

| Kind          | JSON tag       | Semantic                                                |
| ------------- | -------------- | ------------------------------------------------------- |
| `Expression`  | `expression`   | Opaque text on the RHS of `target: …`.                  |
| `Object`      | `object`       | A nested `UiItem` value, e.g. `transitions: Transition { … }`. |
| `List`        | `list`         | A `[ … , … ]` value of further `UiAssignmentValue`s.    |

Implementations **MUST NOT** introduce ad-hoc kinds. If a future Qt
construct does not lower onto one of these three, it **MUST** be
captured as `Expression` (opaque) until a kind is added under this
policy.

## §7 — Frozen Decision: IR Schema Version

The `qt-ir` schema version is **`1`** at QT-01a. The constant lives at
[`src/bin/creator/qt.rs:QT_IR_VERSION`](../../src/bin/creator/qt.rs).

Bumping policy: **Specification Required**. The schema version
**MUST** be incremented when:

- A field is removed from a `Ui*` type.
- A field's type changes incompatibly.
- A `UiAssignmentValue` kind is renamed.
- A handler-name convention changes (e.g. the `function:<name>`
  prefix in §3 is replaced).

Adding new optional fields with `#[serde(default)]` / `Option<T>` is
**SHOULD NOT** bump the version; it is a forward-compatible change.

The amendment **MUST** appear in this chapter's §15 change log
together with a migration note. Tools that consume `qt-ir.json`
**MUST** check `version` and **MAY** refuse to operate on a
higher-than-supported version.

## §8 — Frozen Decision: Commit Prefix

Initiative-and-phase commits **MUST** use the prefix **`QT-NN[a-z]:`**
in the subject line, e.g. `QT-01a: …`, `QT-04b: …`. This replaces the
conventional-commit type for changes scoped to a ratified phase.
Cross-cutting plumbing PRs that affect more than one phase **MAY**
use the conventional `feat:` / `fix:` / `docs:` types.

## §9 — Frozen Decision: Out-of-Scope Boundaries

These are not provisional — they are decisions the initiative
**MUST NOT** revisit without a Standards-Action amendment to this
chapter:

1. **No on-device Qt runtime.** The Qt dependency tree (Qt itself,
   PySide6, `qmlplugindump`) lives only in the creator host
   environment.
2. **No QML JavaScript engine on-device.** Bindings are lowered at
   emit time. Anything that cannot be lowered is rejected with a
   fix-it message.
3. **No lvglpp emission.** A C++ companion runtime consuming
   `qt-ir.json` is a separate initiative.
4. **No live designer round-trip.** Qt → IR → rlvgl is one-way.
   Edits to generated Rust are not propagated back.
5. **No vendoring of upstream Qt.** Whatever Qt the user has
   installed (or doesn't, in the MVP) is what the creator drives.

## §10 — Reconciliation with Adjacent Repo Primitives

| Adjacent primitive                              | Relationship                                                                                | Reconciliation owner |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------- | -------------------- |
| Chakra TS → `tokens.yaml` (`chakra.rs`)         | Both produce a token tree. Qt theme **MUST** funnel through the same `tokens.yaml`.         | `QT-06`              |
| Svelte token → multi-target output (`svelte.rs`)| Same. Qt is a *new source* for `tokens.yaml`, not a parallel emitter.                       | `QT-06`              |
| Asset pipeline (`docs/creator/ASSET-PIPELINE.md`) | Qt-referenced assets **MUST** flow into the existing dual-mode assets crate.              | `QT-07`              |
| BSP YAML / chipdb generator                     | Orthogonal. Qt produces UI; BSP produces hardware glue. Meet only at the workspace scaffold. | n/a (no overlap)     |
| `rlvgl-ui` widgets                              | Qt emitter **MUST** target `rlvgl-ui` widgets, not raw `rlvgl-core` primitives.             | `QT-03`              |
| MicroPython integration ([`docs/future/MICROPYTHON-INTEGRATION.md`](../future/MICROPYTHON-INTEGRATION.md)) | Orthogonal. MicroPython is a runtime API surface; Qt is a compile-time authoring surface. | n/a (no overlap)     |

## §11 — Non-Goals

(Restated from §9 for index visibility — §9 holds normative weight.)

- No Qt runtime on-device.
- No JS engine on-device.
- No lvglpp emission in this initiative.
- No live designer round-trip.
- No vendored Qt distribution.

## §12 — Acceptance Checklist (this chapter)

This chapter is **ratified** when:

- [x] §0 cites RFC 2119 / 8174.
- [x] §3 names every `Ui*` IR type currently defined in
      [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) and marks
      each "used without modification" or "adapted: …".
- [x] §5 enumerates the phase set with one row per known phase.
- [x] §6 freezes the assignment-value kind set.
- [x] §7 freezes the schema version policy.
- [x] §8 freezes the commit prefix.
- [x] §10 names a reconciliation owner for every adjacent primitive
      currently in the repo.
- [x] §15 carries a dated initial change-log entry.

Once a behaviour PR cites this chapter (e.g. via a `QT-NN[a-z]:`
commit), the chapter is considered referenced and amendments to §3,
§5–§7 require a separate amendment PR per `CLAUDE.md`'s execution
discipline.

## §13 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — IR types and parser.
- [`src/bin/creator/chakra.rs`](../../src/bin/creator/chakra.rs) — sibling ingest path.
- [`src/bin/creator/svelte.rs`](../../src/bin/creator/svelte.rs) — sibling token pipeline.
- [`docs/creator/QT-INGEST.md`](../creator/QT-INGEST.md) — practical setup doc.
- [`docs/creator/ASSET-PIPELINE.md`](../creator/ASSET-PIPELINE.md) — asset crate handoff target.
- [`docs/SVELTE-DESIGN-TOKEN-ALIGNMENT.md`](../SVELTE-DESIGN-TOKEN-ALIGNMENT.md) — token vocabulary precedent.
- [`docs/bsp/CHIP-SUPPORT.md`](../bsp/CHIP-SUPPORT.md) — sibling vocabulary-freezing precedent.
- [`docs/UI-COMPONENT-ARCHITECTURE.md`](../UI-COMPONENT-ARCHITECTURE.md) — `rlvgl-ui` target surface.
- [`docs/future/MICROPYTHON-INTEGRATION.md`](../future/MICROPYTHON-INTEGRATION.md) — orthogonal initiative.
- [`tests/fixtures/qt/hello.qml`](../../tests/fixtures/qt/hello.qml) — canonical MVP fixture.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — end-to-end test.

## §14 — Unblocks

Ratifying QT-00 unblocks:

- `QT-01b` — type-introspection ingest may begin authoring its own
  concepts doc citing §3 and §5.
- `QT-02` — IR schema may be exported as JSON Schema and locked in
  via roundtrip tests citing §7's bumping policy.
- `QT-03` — rlvgl emitter design may begin, citing §10's
  `rlvgl-ui` target.

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-28 | Initial ratification. Phase set, IR type names, assignment-value kinds, schema version `1`, and commit prefix `QT-NN[a-z]:` frozen. |
| 2026-04-28 | QT-02 ratified ([`02-ir-schema.md`](./02-ir-schema.md)). Schema artifact at `schemas/qt-ir.schema.json`; golden ingest at `tests/fixtures/qt/hello.qt-ir.json`; drift / golden-file gates added. No IR type changes; schema version remains `1`. |
| 2026-04-28 | QT-03 ratified ([`03-rlvgl-emitter-widgets.md`](./03-rlvgl-emitter-widgets.md)). Data-only Rust emit-shape version `1`; golden Rust at `tests/fixtures/qt/hello.rs`; drift + compile-as-mod gates added. Widget API mapping (QML → `rlvgl-ui`) deferred to QT-03b. No IR type changes. |
| 2026-04-28 | QT-03b ratified ([`03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md)). Mapping table (§5), property-lowering rules (§6), trivial bounds resolver (§7), `--target {data,rlvgl}` flag, and `QT_EMIT_VERSION_DATA`/`QT_EMIT_VERSION_RLVGL` rename plan frozen. Implementation deferred to next pass under commit prefix `QT-03b:`. No IR type changes; `QT_EMIT_VERSION_DATA = 1` and (planned) `QT_EMIT_VERSION_RLVGL = 2`. |
| 2026-04-28 | QT-03b implementation landed. `qt::render_rlvgl` + `--target {data,rlvgl}` (default `rlvgl`) emit a runnable `build_screen(bounds) -> WidgetNode` per QT-03b §3 / §6 / §7 / §8. Drift gate + compile-as-mod gate (against `rlvgl-core` + `rlvgl-widgets`) in place. `QT_EMIT_VERSION_DATA = 1`, `QT_EMIT_VERSION_RLVGL = 2`. QT-03b §3 amended: `build_screen` returns `WidgetNode` (was `Box<dyn Widget>`). §5 amendment: most non-Container/Label rows downgraded to Container fallback for the initial implementation pass; full table coverage tracked under further QT-03b amendments. |
| 2026-04-29 | QT-04 ratified ([`04-signal-handlers.md`](./04-signal-handlers.md)). Phase set in §5 expanded: `QT-04` now scopes to signal handlers only; `QT-04b` carved out for properties + bindings; `QT-04c` for MouseArea / hover handlers. Implementation landed in the same pass: `Button` / `QC.Button` lower with `set_on_click` wiring for `onClicked`, body kept as `// QT-04 body:` comment until QT-04b. `QT_EMIT_VERSION_RLVGL` bumped from `2` to `3`. New `clickable.qml` fixture + 3 goldens + 3 drift gates + compile-as-mod gate. QT-03b §5 amended: Button row promoted from Container fallback to typed mapping. |
| 2026-04-29 | QT-04b ratified ([`04b-properties-bindings.md`](./04b-properties-bindings.md)). `ScreenState` struct shape (§3), `build_screen` signature change to return `(WidgetNode, Rc<RefCell<ScreenState>>)` (§3 / §10), supported property types (§5), default-value lowering (§6), handler-body grammar (§7), root-only ID resolution (§8), and `QT_EMIT_VERSION_RLVGL` bump plan to `4` (§11) frozen. Implementation deferred to next pass under commit prefix `QT-04b:`. The amendment reaches into QT-03b §3 and QT-04 §15 (the `// QT-04 body:` marker repurposing) — both will be recorded once implementation lands. No IR type changes. |
| 2026-04-29 | QT-04b implementation landed (same-day). `qt::render_rlvgl` now emits `pub struct ScreenState`, the new tuple `build_screen` signature, threaded `state` parameter on every helper, property literal-default lowering (i32 / f32 / bool / String), and §7 handler-body grammar lowering with `// QT-04b body:` markers. `QT_EMIT_VERSION_RLVGL = 4`. New `counter.qml` fixture + 3 goldens + 3 drift gates + synthetic-click compile-as-mod gate that asserts `state.borrow().count == 1` after firing `Event::PressRelease`. QT-03b §3 / §15 amended for the signature change; QT-04 §15 amended for the marker repurposing. Existing rlvgl-target compile-as-mod gates updated to consume the tuple return. No IR type changes. |
| 2026-04-29 | **QT-04 family closed out.** QT-04e ratified + shipped ([`04e-reactive-bindings.md`](./04e-reactive-bindings.md)). `build_screen` extended from a 2-tuple to a 3-tuple gaining `Vec<LabelBinding>`; new `pub struct LabelBinding`, `impl LabelBinding`, and `pub fn refresh_bindings` are emitted into every rlvgl-target file. Helper signatures gain `&mut Vec<LabelBinding>`. Bound `Label` widgets (per QT-04c §5 resolution) now retain a concrete `Rc<RefCell<Label>>` handle so mutation+refresh updates the rendered text. `QT_EMIT_VERSION_RLVGL` bumped `10 → 11`. `bound_text` compile-as-mod gate now asserts the **reactive** contract end-to-end (mutate state.title → call refresh_bindings → Label text updates from `"Greetings"` to `"Updated"`). All existing rlvgl-target goldens regenerated; compile-gate destructures updated from 2-tuple to 3-tuple. QT-04b §3 / §15 records the `build_screen` extension; QT-04c §15 records the non-reactive-contract supersession. Color bindings, Button text bindings, and handler-body expression expansion remain deferred under future Specification-Required amendments. **No further QT-04 sub-phases planned**; subsequent Qt work moves to QT-05 (state machines), QT-06 (theme tokens), QT-07 (asset handoff), or QT-08b (`.qmldir`). |
| 2026-04-29 | QT-04d ratified + shipped ([`04d-mousearea.md`](./04d-mousearea.md)). New `rlvgl_widgets::click_area::ClickArea` widget added upstream — transparent hit-area with `set_on_click` mirroring Button's pattern. QML `MouseArea` promoted from QT-03b §5 Container fallback to a typed `ClickArea` mapping; QT-04 §5 handler-supported widget set extended from `{Button}` to `{Button, ClickArea}`. `onClicked` lowers per QT-04 §6 + QT-04b §7. `QT_EMIT_VERSION_RLVGL` bumped `9 → 10`. New `mousearea.qml` fixture + 3 goldens + 3 drift gates + synthetic-click compile-as-mod gate that asserts `state.taps == 1` after firing `Event::PressRelease`. `hello.rlvgl.rs` regenerated — its MouseArea now lowers to ClickArea and `onClicked: root.count += 1` lowers under QT-04b's grammar. All existing rlvgl-target goldens regenerated; existing compile-gate version assertions bumped. Hover events, drag, multi-touch, press/release split deferred. |
| 2026-04-29 | QT-03c §5 amendment #2: corner combinations (`left+top`, `right+top`, `left+bottom`, `right+bottom`) promoted from "deferred" to "implemented" via [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) §15. New `// QT-03c corner:` marker. `QT_EMIT_VERSION_RLVGL` bumped `8 → 9`. New `corners.qml` fixture (4 corner-anchored badges) + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate (TL=(0,0), TR=(170,0), BL=(0,180), BR=(170,180)). All existing rlvgl goldens regenerated; existing compile-gate version assertions bumped. Axial-fill combinations (`left+right`, `top+bottom`) and `*Center` / sibling-relative anchors remain deferred. |
| 2026-04-29 | QT-03c §5 amendment: single edge anchors `anchors.left` / `anchors.right` / `anchors.top` / `anchors.bottom` (against `parent.<edge>`) promoted from "deferred" to "implemented" via [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) §15. Each anchor lowers in isolation; combined edge anchors fall through. New `// QT-03c edge:` marker. `QT_EMIT_VERSION_RLVGL` bumped `7 → 8`. New `edges.qml` fixture + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate (verifies all four children land at expected positions). All existing rlvgl-target goldens regenerated; existing compile-gate version assertions bumped. `horizontalCenter` / `verticalCenter` and combined-edge / corner anchors remain deferred. |
| 2026-04-29 | QT-04f ratified + shipped ([`04f-nested-id-resolution.md`](./04f-nested-id-resolution.md)). `ScreenState` now carries `<sanitized_id>_<prop>` fields for non-root id'd items; root-scope fields stay un-namespaced for back-compat. A shared `resolve_state_field_ref` helper backs both QT-04b's handler-body grammar and QT-04c's text-binding resolver, accepting `<ident>`, `root.<ident>`, and `<other_id>.<ident>` forms. Three-or-more-level dotted refs and aliases remain fall-through. `QT_EMIT_VERSION_RLVGL` bumped `6 → 7`. New `nested.qml` fixture + 3 goldens + 3 drift gates + synthetic-click compile-as-mod gate that asserts `state.bg_alpha == 90` after firing `Event::PressRelease` on the sibling Button. All existing rlvgl-target goldens regenerated; compile-gate version assertions bumped. QT-04b §8 amended via this chapter. |
| 2026-04-29 | QT-08 ratified + shipped ([`08-multi-file-cli.md`](./08-multi-file-cli.md)). `qt ingest` and `qt emit` now accept a directory `<input>` and walk every `*.qml` in lexical order, emitting `<basename>.qt-ir.json` / `<basename>.rs` / `<basename>.rlvgl.rs` per file. File-mode behaviour preserved — `qt ingest <file.qml> <out>/` still writes `qt-ir.json` (asymmetry frozen for back-compat per QT-08 §6). No `QT_EMIT_VERSION` bump (per-file shape unchanged). New `tests/fixtures/qt/multi/{a,b}.qml` fixture + 3 dir-mode gates verifying both files emit. `qt schema` / `qt check` remain file-only at QT-08; `.qmldir` and `.qrc` deferred to QT-08b / QT-08c. |
| 2026-04-30 | **QT-10 strict-mode-1 ratified + shipped** ([`10-release.md`](./10-release.md)). Closes the Qt-support roadmap's enforcement story (auto-mode half). New `pub const QT_FAMILY_STRICT_VERSION: u32 = 1;` in `src/bin/creator/qt.rs` pins the strict-mode generation. New integration test `tests/creator_qt_strict_mode.rs` enforces three invariant sets per QT-10 §7: (1) chapter file set — every `docs/qt-support/*.md` (excluding README.md) is in the canonical 24-entry list AND every named entry exists on disk; bidirectional check guards against drive-by chapter additions; (2) every chapter carries the `## §15 — Change Log` section that the spec-before-code discipline requires; (3) the version-constant snapshot is intact: `QT_IR_VERSION = 2`, `QT_EMIT_VERSION_RLVGL = 13`, `QT_EMIT_VERSION_DATA = 1`, `QT_EXTERNALS_VERSION = 1`, `QT_FAMILY_STRICT_VERSION = 1` (5 constants pinned in tests/creator_qt_strict_mode.rs::rlvgl_creator_qt_test_helpers and cross-checked against the source via the `qt10_strict_version_const_pinned_in_source` source-grep test); (4) the CLI subcommand surface from QT-10 §5 — `ingest` / `check` / `schema` / `emit` / `emit-scjson` / `emit-externals` / `emit-tokens` / `list-assets` / `list-qmldir` / `list-qrc` (10 entries) — is reachable via `rlvgl-creator qt --help`. Helper module `rlvgl_creator_qt_test_helpers` mirrors the §6 strict-mode-1 snapshot as hand-pinned values; if the constants drift in source, the source-grep test catches it. New `[[test]]` entry in `Cargo.toml` for `creator_qt_strict_mode`, `required-features = ["creator"]`. The release-tag half — cutting `v0.x.0`, running `cargo fmt --check` / `cargo clippy --workspace` / `cargo test --workspace`, bumping `Cargo.toml` workspace version, updating `CHANGELOG.md` (if maintained) with the QT-* family entries from this §15 log, `git tag v0.<minor>.<patch>` + `git push --tags`, `cargo publish` per crate in dependency order (rlvgl-chips-* → rlvgl-core → rlvgl-widgets → rlvgl), and GitHub Release notes — is documented in QT-10 §8 as the canonical workflow but **NOT performed** under auto mode per the safety protocol's "destructive operations require explicit user authorization" rule. The user runs it when ready. With QT-10 strict-mode-1 the Qt-support roadmap is feature-complete: 24 chapters ratified (00 + 02 + 03/03b/03c + 04/04b/04c/04d/04e/04f + 05/05a/05b/05c/05d/05e + 06 + 07 + 08/08b/08c + 09 + 10), 10 CLI subcommands shipped, end-to-end QML → scjson → istate-codegen → rlvgl-emitted Rust → ScreenState bindings → Externals callouts → theme tokens → asset inventory pipeline, desktop UI parity (creator + creator_ui feature both build clean), 8 chipdb crates on the unified YAML accessor convention (stm intentionally retains its zstd-blob path for the disco-platform-guide ecosystem), and strict-mode invariants under `cargo test` enforcement. 268 creator-feature integration tests + 13 chipdb crate unit tests all green. Future bumps to `QT_FAMILY_STRICT_VERSION` (to 2+) trigger Standards Action chapter amendments per QT-10 §3. |
| 2026-04-30 | QT-09 §10 amendment #2 (same-day): silabs / microchip / ti chipdb crates migrated to the YAML accessor convention, removing the prior "not yet supported by the YAML accessor convention" typed-error fallback from `boards.rs::load_ir`. Per-crate work: silabs adds `db/chips/EFM32GG11.yaml` + `db/boards/EFM32GG11.yaml`; microchip adds `db/chips/ATSAMD51J19A.yaml` + `db/boards/ATSAMD51J19A.yaml`; ti adds `db/chips/MSP432P401R.yaml` + `db/chips/AM335x.yaml` + `db/boards/MSP432P401R.yaml` + `db/boards/beaglebone_black_nhd_cape.yaml`. Each YAML file uses the same shape as the nrf seed data: top-level `name:` / `chip:` / `arch:` / `package:` / `pac_crate:` for chips; top-level `name:` / `chip:` / `flash_mb:` / `console:` / `pins:` for boards. File stems chosen to match the YAML `name:` field so `<vendor>::find(name)` and `<vendor>::board_yaml(name)` resolve to the same entry without a stem→name lookup helper. Each crate's `build.rs` is a verbatim copy of nrf's (vendor-agnostic — scans `db/chips/` + `db/boards/`, emits `generated.rs` with `chip_yaml_impl` / `board_yaml_impl` / `CHIP_NAMES` / `BOARD_NAMES` / `BOARD_INFOS`). Each crate's `src/lib.rs` is rewritten to mirror nrf's accessor surface — the placeholder `BOARDS: &[BoardInfo]` const is removed in favour of the build-time-generated `BOARD_INFOS` slice; the legacy `raw_db()` function is removed entirely (no callers existed). Each crate's `Cargo.toml` gains `[features] default = ["std"]; std = ["dep:serde"]` to satisfy the `#![cfg_attr(not(feature = "std"), no_std)]` attribute used in lib.rs (otherwise rustc warns `unexpected cfg condition value: "std"`). Per-crate smoke tests added (silabs: 4 — chip presence, board presence, find-by-name, missing-name → None; microchip: 4 same; ti: 5 — both chips MSP432P401R and AM335x present, BBB+NHD board present + asserts `chip: AM335x`, find-by-name, missing → None) for a total of 13 new tests. `boards.rs::load_ir` extends to dispatch silabs / microchip / ti through the new path; the legacy raw_db error branch is removed entirely. Both `cargo check --features creator -p rlvgl --bin rlvgl-creator` and `cargo check --features creator,creator_ui -p rlvgl --bin rlvgl-creator` build clean. 263 creator-feature integration tests + 13 chipdb crate unit tests all green. The remaining 8th vendor — stm — still uses its zstd-blob `raw_db()` convention because the rest of the rlvgl tree references it that way (`docs/disco-platform-guide/`, `examples/stm32h747i-disco/`); migrating stm is a separate slice not gated on QT-09. With this amendment all eight chipdb crates either fully use the YAML accessor convention (nrf, esp, nxp, renesas, rp2040, silabs, microchip, ti) or stay on the parallel zstd-blob convention (stm). The `boards.rs::load_ir` function consequently no longer needs vendor-specific fallback logic for the chipdb crates it knows about; an unknown vendor is the only remaining error path. |
| 2026-04-30 | QT-09 §10 caveat resolved (same-day amendment). The pre-existing `creator_ui` build break in `src/bin/creator/boards.rs::load_ir` (5 `cannot find function raw_db` errors against `nrf` / `esp` / `nxp` / `renesas` / `rp2040` chip vendor crates which had migrated to per-spec `board_yaml(name)` / `chip_yaml(name)` accessors) is now fixed. `load_ir` rewritten to consume the new accessor convention: each match arm calls `<vendor>::board_yaml(board)` and `<vendor>::chip_yaml(info.chip)`, parses the YAML strings into `serde_json::Value` via `serde_yaml::from_str` (the YAML and JSON serde representations are interchangeable for `Value`). nrf / esp / nxp / renesas / rp2040 take the new path. silabs / microchip / ti — whose chipdb crates still ship only the legacy `raw_db()` blob with no source data wired (build.rs reads `RLVGL_CHIP_SRC` which is unset, so the blob is empty) — return a typed `"load_ir: vendor 'X' not yet supported by the YAML accessor convention; the chipdb crate ships only the legacy raw_db blob"` error rather than panicking on zstd-decoding an empty blob (the previous behaviour). Dead `parse_raw_db` helper removed along with the `zstd::stream::read::Decoder` / `std::io::Read` / `HashMap` imports it depended on. `test_vendor` (the cfg(test)-only mock vendor inside `boards.rs`) updated to expose `board_yaml(name)` / `chip_yaml(name)` accessors mirroring the new convention; tests that used to feed `raw_db()` now feed YAML literal strings. Type-annotation hint added on the `(Option<&'static str>, Option<&'static str>)` match expression so rustc's E0282 inference is happy in the `cfg(test)` branch. `cargo check --features creator,creator_ui -p rlvgl --bin rlvgl-creator` now builds clean. `cargo check --features creator -p rlvgl --bin rlvgl-creator` continues to build clean. 263/263 creator-feature integration tests still green (no regression). All previously-shipped fixtures unaffected. The `load_ir` / `render_template` functions remain unused by external callers (they were dead code even before this change); the fix is structural correctness only. silabs / microchip / ti chipdb crates may grow `board_yaml` / `chip_yaml` accessors in a future amendment when their source data is wired into `RLVGL_CHIP_SRC` or shipped in `assets/chipdb.bin.zst`. |
| 2026-04-30 | QT-09 ratified ([`09-desktop-ui.md`](./09-desktop-ui.md)); UI wiring shipped. New "Qt" menu group added to `src/bin/creator_ui/menus.rs` with 10 entries — Qt Ingest, Qt Check, Qt Schema, Qt Emit, Qt Emit Scjson, Qt Emit Externals, Qt Emit Tokens, Qt List Assets, Qt List Qmldir, Qt List Qrc — in CLI subcommand declaration order. Module wiring in `src/bin/creator_ui/mod.rs`: `#[path = "../creator/qt.rs"] mod qt;` + `#[path = "../creator/qt_scjson.rs"] mod qt_scjson;` (each `#[allow(dead_code)]` because the qt module re-exports more than the UI consumes). New 10 `handle_qt_*` functions in `src/bin/creator_ui/commands.rs` following the existing `handle_scan` / `handle_vendor` pattern: `pick_file()` / `pick_folder()` for input → optional `pick_folder()` for output → `qt::*` invocation → `show_feedback` toast. New helper `CreatorApp::pick_qml_input()` tries `.qml` file picker first, falls through to folder picker — covers QT-08's file-or-directory input mode uniformly. Per-subcommand specifics: `Qt Schema` uses `save_file()` (cancel writes to stdout via `qt::schema(None)`); `Qt Emit` defaults to `qt::EmitTarget::Rlvgl` (UI does not expose `--target data` at v1); `Qt List Qmldir` prefers folder picker (per QT-08b §7); `Qt List Qrc` tries `.qrc` file picker first then folder fallback. New 10 dispatch arms in `handle_action` keyed by the menu labels. **§10 caveat**: the wider `cargo check --features creator,creator_ui` build is blocked by 5 pre-existing `raw_db` lookup errors in `creator_ui/../creator/boards.rs` against `nrf` / `esp` / `nxp` / `renesas` / `rp2040` chip vendor crates. Verified via `git stash` that the same 5 errors appear with QT-09 reverted, confirming the breakage predates QT-09 and is tracked separately. QT-09 verification falls back to `cargo check --features creator` (without `creator_ui`) compile-clean which confirms the `qt::*` call sites in `commands.rs` are syntactically correct against the public surface. 263/263 creator-feature tests still green (no regression). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (UI wiring only; no emit-shape change). No headless UI tests added — the codebase has no eframe test harness, so visual verification by the user awaits the `boards.rs` breakage being resolved upstream. The advanced flags (`qt emit --target data`, verbose, silent), drag-and-drop input, project-wide "run all qt" macro analogous to "Scan Convert Preview", and qmldir/qrc preview tabs remain deferred under future Specification-Required §5 amendments. |
| 2026-04-30 | QT-08c ratified + shipped ([`08c-qrc-resources.md`](./08c-qrc-resources.md)). New CLI subcommand `qt list-qrc <input> [<out>]` (file mode + directory mode). New entry point `qt::list_qrc(input, out)`. New public Rust types in `qt.rs`: `pub struct QrcManifest { version, resources }`, `pub struct QrcResource { prefix, lang, files }`, `pub struct QrcFile { path, alias }` (all derive `Debug`/`Clone`/`PartialEq` / `Default` where appropriate). New `qt::parse_qrc(content: &str) -> Result<QrcManifest>` parses the `.qrc` XML subset via a hand-rolled minimal walker (`QrcParser` struct holding `src` + `pos`) — **no new Cargo deps**. The walker handles XML prologue (`<?xml … ?>`, `<!DOCTYPE …>`, `<!--…-->` comments), the `<RCC>` root with optional `version` attribute, nested `<qresource>` blocks with `prefix`/`lang` attributes, and `<file>` entries with optional `alias` attribute and trimmed text content. Recognised elements are gated strictly: an unknown element under `<RCC>` or `<qresource>` is an emit-time error rather than a passthrough (matches `.qrc`'s tightly-defined schema). New helper methods on `QrcParser`: `parse_document`, `parse_qresource`, `parse_file`, `skip_ws`, `skip_ws_and_comments`, `skip_comment`, `skip_doctype`, `skip_pi`, `starts_with`, `expect`, `parse_attrs`. Attribute values support both single and double quoting; backslash escapes deferred. New `qt::render_qrc_yaml(manifest, source) -> String` produces stable `# QT-08c qrc: <path>` provenance comment + `version: 1` + scalar `rcc_version:` + `resources:` list with nested `prefix:`/`lang:`/`files:` mappings; flow-style `{ path: …, alias: … }` rows for files, lexically faithful to declaration order (XML is order-sensitive). Smart `<out>` resolution: file path / directory path with synthesised `<basename>.qrc.yaml` filename / default-to-source-parent. Directory mode discovers `*.qrc` files via lexical sort. Missing input file is a hard error (non-silent) listing the expected path. Malformed XML produces `bail!` errors with byte-position attribution. New fixture `tests/fixtures/qt/resources.qrc` declares `<RCC version="1.0">` + 2 `<qresource>` blocks (`prefix="/icons"` with 3 files including 1 `alias="reset"`; `prefix="/fonts"` with 1 file). Emitted golden `tests/fixtures/qt/resources.qrc.yaml` checked in. Two new drift gates in `tests/creator_qt_ingest.rs`: byte-equality (`qt_resources_list_qrc_matches_golden`) and missing-input hard-error (`qt_list_qrc_missing_input_is_hard_error` exercises a non-existent .qrc path and asserts non-zero exit + stderr mentioning `.qrc`/`not found`). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-08c's artifact is a separate `.qrc.yaml` file). All previously-shipped fixtures unaffected. Cross-validation between qrc declarations + QT-07 asset inventory + QT-01a `Image { source: "qrc:…" }` references, file-existence checks, `<file compress="…">` / `<file threshold="…">` compression metadata, XML namespaces, and nested CDATA in `<file>` content all remain deferred under future Specification-Required §5 amendments. |
| 2026-04-30 | QT-08b ratified + shipped ([`08b-qmldir-resolution.md`](./08b-qmldir-resolution.md)). New CLI subcommand `qt list-qmldir <input> [<out>]` (file mode + directory mode). New entry point `qt::list_qmldir(input, out)`. New public Rust types in `qt.rs`: `pub struct QmldirManifest { module, types, singletons, internals, imports, depends, plugins, other }`, `pub struct QmldirType { name, version, file }`, `pub struct QmldirImport { module, version }`, `pub struct QmldirPlugin { name, path }` (all derive `Debug`/`Clone`/`PartialEq`/`Default` where appropriate). New `qt::parse_qmldir(content: &str) -> QmldirManifest` pure parser tokenises lines on whitespace, drops `#`-prefix comments and blank lines, dispatches via slice-pattern match across all QT-08b §5 directives. Bare type registration form (`<Name> <version> <file>.qml`) gated on `.qml` suffix to disambiguate from rare 3-token unrecognised lines. Multiple `module` lines collapse to last-one-wins matching Qt's own qmldir loader. New `qt::render_qmldir_yaml(manifest, source) -> String` produces stable `# QT-08b qmldir: <path>` provenance comment + `version: 1` + scalar `module:` + 7 lists (types/singletons/internals/imports/depends/plugins/other) always emitted even when empty so the schema is invariant. Helpers `render_qmldir_type` / `render_qmldir_import` produce inline-flow `{ name: …, version: …, file: … }` mapping rows; `null` literal used for `Option::None` versions. Output YAML preserves declaration order (qmldir is order-sensitive at parse time). Smart `<out>` resolution per §7: file path / directory path with synthesised `<dirname>.qmldir.yaml` filename / default-to-source-parent. Missing input file → hard error (`bail!`) listing the expected path; non-silent because the user explicitly invoked the subcommand. New fixture `tests/fixtures/qt/sample_module/qmldir` declaring `module SampleModule` + 2 ordinary types (MyButton/MyLabel) + 1 singleton (Theme) + 1 internal (_Helper) + 1 import (QtQuick 2.15) + 1 depends (QtQuick.Controls 2.15) + 1 unrecognised `typeinfo plugin.qmltypes` directive captured in `other`. Emitted golden `tests/fixtures/qt/sample_module.qmldir.yaml` checked in. Two new drift gates in `tests/creator_qt_ingest.rs`: byte-equality (`qt_sample_module_list_qmldir_matches_golden`) and missing-input hard-error (`qt_list_qmldir_missing_input_is_hard_error` exercises an empty workdir and asserts non-zero exit + stderr mentioning `qmldir`/`not found`). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-08b's artifact is a separate `.qmldir.yaml` file). All previously-shipped fixtures unaffected. `import "Module"` cross-resolution at QT-01a (would let `import "MyModule"` resolve to a registered qmldir), `typeinfo <file>.qmltypes` parsing, `classname <Name>` C++ plugin classname binding, `prefer <path>` alternate module location, `optional` import modifier, `designersupported` flag, recursive bundle expansion across nested directories, and singleton-driven theme auto-discovery in QT-06 (where a qmldir-declared singleton becomes the canonical Theme source rather than probing every QtObject root) all remain deferred under future Specification-Required §5 amendments. |
| 2026-04-30 | QT-07 ratified + shipped ([`07-asset-handoff.md`](./07-asset-handoff.md)). New CLI subcommand `qt list-assets <input> [<out>]` (file mode + directory mode per QT-08). New entry point `qt::list_assets(input, out)`, helpers `resolve_assets_out_for(qml, out_dir)` and `list_assets_one(input, out_path)`. New `qt::walk_asset_refs(item) -> AssetInventory` pure recursive walker; new private intermediate `pub struct AssetInventory { images: BTreeSet<String>, fonts: BTreeSet<String> }` for dedup + lexical ordering. New `visit_for_assets(item, &mut inv)` recursive helper that handles four reference forms per QT-07 §5: (1) `Image { source: "<path>" }` and `Image { source: "qrc:[/]<path>" }` with `qrc:/` and `qrc:///` prefix stripped via new `strip_qrc_prefix` helper; (2) standalone `Font { family: "<name>" }` blocks (matched by stripped type-name); (3) dotted `<*>.font.family: "<name>"` direct assignments; (4) nested `font: Font { family: "<name>" }` object values. Type-name matching is suffix-aware (`QC.Image` and `Image` both match) via `rsplit('.').next()`. Non-literal sources / family expressions / state-bound forms silently dropped. New `qt::render_assets_yaml(inv, qml_source) -> String` produces stable `# QT-07 assets: <path>` provenance comment + `version: 1` + `images: […]` + `fonts: […]` lists (always emitted, even when empty, so the schema is invariant). New `quote_yaml_scalar(s)` helper emits double-quoted form when the value contains whitespace, YAML metacharacters (`:`, `#`, `[`, `]`, `{`, `}`, `,`), or quote characters; backslashes and inner double-quotes escaped. Empty inventory triggers silent skip in `list_assets_one`. New fixture `tests/fixtures/qt/image_refs.qml` declaring 4 Image children (mixed `qrc:/` / `qrc:///` / relative-path forms, with one duplicate to verify dedup) and 3 Text children (with one duplicate font family) on an Item root. Emitted golden `tests/fixtures/qt/image_refs.assets.yaml` shows exactly 3 distinct image paths (background.png/play.png/stop.png — qrc-stripped) and 2 distinct fonts (`"FiraSans Bold"` quoted for whitespace, `Roboto` bare). Two new drift gates in `tests/creator_qt_ingest.rs`: `qt_image_refs_list_assets_matches_golden` (byte-equality pinning the qrc:-prefix stripping, BTreeSet-based dedup, lexical ordering, and YAML quoting) and `qt_list_assets_silent_skip_for_non_asset_qml` (exercises `counter.qml` which has only Buttons + onClicked → no Image / no font references → no `.assets.yaml` produced). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-07's artifact is a separate `.assets.yaml` file from the versioned emit-shapes). All previously-shipped fixtures unaffected — QT-07's walker silently skips QML files with no recognised asset references. Inventory is intentionally NOT a `manifest.yml` (different schema) so users hand-merging into the asset pipeline cannot accidentally clobber existing manifest entries. State-bound `source:` expressions, `font.weight: Font.Bold` / `font.pointSize` derivation, qrc-bundle resolution (deferred to QT-08c), `.qmldir` external-asset declarations (deferred to QT-08b), `AnimatedImage` frame folders, localised asset variants, and a `manifest merge-qt` round-trip subcommand all remain deferred under future Specification-Required amendments. |
| 2026-04-30 | QT-06 ratified + shipped ([`06-theme-tokens.md`](./06-theme-tokens.md)). New CLI subcommand `qt emit-tokens <input> [<out>]` (file mode + directory mode per QT-08). New entry point `qt::emit_tokens(input, out)`, helper functions `resolve_tokens_out_for(qml, out_dir)` and `emit_tokens_one(input, out_path)` mirror the QT-05d/05e emission pattern. New `qt::walk_theme_module(item) -> Option<TokenSet>` pure walker. New private intermediate `pub struct TokenSet { colors, spacing, radii, fonts, dark_colors }` using `BTreeMap` everywhere for deterministic lexical key ordering at YAML emission. New `qt::render_tokens_yaml(theme, qml_source) -> String` produces the chakra/svelte-compatible `tokens.yaml` shape: `version: 1` + `colors:` + `spacing:` + `radii:` + `fonts:` + optional `modes.dark.colors:`, with `# QT-06 theme: <path>` provenance comment as the first content line. Name-to-category rules per QT-06 §6: `property color <name>: "#hex"` → `colors.<name>`; `property color <name>_dark: "#hex"` → `modes.dark.colors.<name>` (suffix-stripped key); `property int spacing_<key>: <int>` → `spacing.<key>` (prefix-stripped key); `property int radius_<key>: <int>` → `radii.<key>`; `property string font_<key>: "<name>"` → `fonts.<key>`. New helper `parse_hex_color_lit(expr) -> Option<String>` accepts `#rgb`/`#rrggbb`/`#rrggbbaa` (case-insensitive), rejects non-conforming forms; new `parse_int_literal_i64(expr) -> Option<i64>`. Silent fall-through for properties whose name doesn't match a category prefix and for non-literal `default_value`s. Walker returns `None` if no recognised theme properties were found, triggering silent skip in `emit_tokens_one`. New fixture `tests/fixtures/qt/Theme.qml` declaring 4 base colors (primary/background/text/accent), 5 spacing tokens (xs/sm/md/lg/xl), 5 radius tokens (none/sm/md/lg/full), 3 fonts (small/body/heading), and 2 dark-mode color overrides (background_dark/text_dark) on a `QtObject` root. Emitted golden `tests/fixtures/qt/Theme.tokens.yaml` checked in. Two new drift gates in `tests/creator_qt_ingest.rs`: byte-equality (`qt_theme_emit_tokens_matches_golden`) and silent-skip (`qt_emit_tokens_silent_skip_for_non_theme_qml`, exercises the `hello.qml` widget fixture which has no theme properties → no tokens.yaml produced). Multi-source precedence resolution stays a user concern at QT-06 v1 (per §8); a `qt merge-tokens` subcommand for explicit-precedence overlays across Qt + Chakra + Svelte sources is reserved as a future v2 amendment. Material/Universal style-system extraction (`Material.accent` / `Universal.accent`), `palette {}` block parsing, rgba/hsl/named-color support, per-state overrides beyond `_dark`, animation/transition tokens, and numeric font sizes remain deferred under future Specification-Required §5 amendments. No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-06's artifact is a separate `.tokens.yaml` file from the versioned emit-shapes). All previously-shipped fixtures unaffected — QT-06's walker silently skips QML files with no theme properties, so existing widget fixtures don't produce token files. |
| 2026-04-29 | **QT-05 family closed out.** QT-05e ratified + shipped ([`05e-externals-stubs.md`](./05e-externals-stubs.md)). New CLI subcommand `qt emit-externals <input> [<out>]` (file mode + directory mode per QT-08) and entry point `qt::emit_externals(input, out)`. New helpers `resolve_externals_out_for(qml, out_dir)` and `emit_externals_one(input, out_path)` mirror the QT-05d emit-scjson pattern. New `qt::render_externals(sm_id, sm, qml_stem, qml_source) -> String` walks `state_machine.scripts` and emits a `<basename>_externals.rs` containing a `pub struct ScreenExternals;` (stateless v1) with `impl ScreenExternals { pub fn new() -> Self { Self } }` + `impl Default for ScreenExternals` + `impl <sm>_gen::Externals for ScreenExternals` covering exactly one method per discovered script (in `state_machine.scripts` declaration order). Method bodies are `// QT-05e externals-stub: <name> from <origin>` + `// TODO — fill in side-effect code.` + `let _ = m;`. New helper `render_script_origin(origin)` formats `UiScriptOrigin` for the emitted comment marker. Per-file emit-shape constant `QT_EXTERNALS_VERSION = 1` for traceability; bumps when a future amendment changes the externals emit shape. SPDX header + provenance comment naming source QML and regen command. `#![allow(dead_code)]` + `#![allow(unused_variables)]` blanket allows. `use <sm>_gen::{Externals, Machine};` import. Install path documented per §7: `machine.borrow_mut().externals = Box::new(ScreenExternals::new())` against the public `Machine.externals: Box<dyn Externals>` field from linkage v1; `Machine::with_options` does not provide constructor injection in linkage v1, so post-construction assignment is the canonical install. Silent-skip cases per §5: a `.qml` without an attached state machine, OR with attached SM but empty `scripts`, produces no externals file. New emitted golden `tests/fixtures/qt/stopwatch_externals.rs` (stopwatch.scjson has `tick_start` (OnEntry: running) + `tick_stop` (OnExit: running) scripts). New byte-equality drift gate `qt_stopwatch_externals_emit_matches_golden` in `tests/creator_qt_ingest.rs`. New compile-as-mod gate `generated_stopwatch_externals_installs_on_machine` in `tests/creator_qt_emit_stopwatch_compile.rs` proves: `ScreenExternals::new()` constructs, `machine.externals = Box::new(ScreenExternals::new())` installs against the QT-05 §6 trait, the stub methods compile and accept `&mut Machine`, and the per-file `QT_EXTERNALS_VERSION = 1` constant is reachable. No bumps to `QT_IR_VERSION`, `QT_EMIT_VERSION_RLVGL`, `QT_EMIT_VERSION_DATA`, or `ISTATE_LINKAGE_VERSION` (QT-05e's artifact is a separate file from the versioned `build_screen` emit shape). Stateful externals (auto-injected `pub state: Rc<RefCell<ScreenState>>` field), merge-on-regen with hand-edited bodies, and multi-screen externals consolidation remain deferred under a hypothetical future QT-05f. **No further QT-05 sub-phases planned**; with QT-05a (ingest), QT-05b (dispatch), QT-05c (DM bindings), QT-05d (QML→scjson emit), and QT-05e (externals) the QT-05 family is feature-complete: a Qt screen with inline `States{}`/`transitions:` declarations now round-trips end-to-end through scjson → istate-codegen-compatible Rust → ScreenState bindings → Externals-driven side effects. Subsequent Qt work moves to QT-06 (theme-token reconciliation), QT-07 (asset-crate handoff), or QT-08b/c (`.qmldir` / `.qrc` resolution). |
| 2026-04-29 | QT-05d ratified + shipped ([`05d-emit-scjson.md`](./05d-emit-scjson.md)). New CLI subcommand `qt emit-scjson <input> [<out>]` (file mode + directory mode per QT-08). New pure walker `walk_qml_state_machine(item, source) -> Result<Option<qt_scjson::Scxml>>` recognises the §5 QML idiom — `states: [State { name: "…"; initial: true }]` (where `initial` is optional) and `transitions: [Transition { from: "…"; to: "…"; event: "…" }]` blocks at the QML root — and lifts them into a `qt_scjson::Scxml` document. New helper `iter_object_items(value, expected)` extracts `UiItem`s from `UiAssignmentValue::Object` / `List` based on a target type-name. Walker is pure (re-running on the same `UiItem` produces byte-identical bytes), preserves declaration order for states and transitions, validates referential integrity (unknown source state → error). Multiple `initial: true` → emit-time error; missing `name`/`from`/`to` → emit-time error; transition's `from` not in the states list → emit-time error. Provenance attribute `_comment: "QT-05d emit-scjson: <path>"` written into `Scxml.other_attributes` so reviewers can trace generated `.scjson` back to its source QML. Animation-flavoured `Transition` properties (`signal`, `PathAnimation`, etc.), `PropertyChanges` blocks, wildcard `from: "*"`, and the `import QtQml.StateMachine 1.0` framework form silently dropped or deferred per §5. New `qt::emit_scjson(input, out)` entry point with file mode + directory mode + smart `<out>` resolution (file path, directory path, or default-to-input-parent). New `resolve_scjson_out_for(qml, out_dir)` helper builds `<out_dir>/<basename>.scjson`. Round-trip parity contract from §8: emit-scjson → write `.scjson` next to QML → QT-05a re-ingest produces a `UiStateMachine` with `states`/`transitions`/`initial` shape-equal to direct walking of the QML (allowing for `id`/`source` differences which are populated by QT-05a from filename context). New fixture `tests/fixtures/qt/inline_states.qml` declares idle/running states with start/stop transitions inline; emitted golden `tests/fixtures/qt/inline_states.scjson` checked in. New 2 drift gates in `tests/creator_qt_ingest.rs`: byte-stability (provenance-comment ignored for portability) and end-to-end emit-then-ingest parity (verifies `state_machine.initial == "idle"`, two states with correct IDs, two transitions with correct events). No `QT_IR_VERSION` or `QT_EMIT_VERSION_RLVGL` bump (QT-05d's artifact is a separate `.scjson` file, not part of the versioned emit-shapes). All previously-shipped fixtures unaffected (QT-05d's walker only fires on QML with explicit `states:`/`transitions:` blocks; existing fixtures have neither). The QML `StateMachine{}` framework form (from `QtQml.StateMachine 1.0`), wildcard `from:`, `PropertyChanges`, and inline `<datamodel>` remain deferred under future Specification-Required amendments. |
| 2026-04-29 | QT-05c ratified + shipped ([`05c-machine-bindings.md`](./05c-machine-bindings.md)). New emit-shape primitives in `qt::render_rlvgl`: `pub enum Binding { Label(LabelBinding), Machine(MachineBinding) }` sealed enum and `pub struct MachineBinding { label: Rc<RefCell<Label>>, accessor: fn(&DataModel) -> String }`. New free helpers `emit_machine_binding_struct`, `emit_binding_enum`, `parse_dm_text_ref`. `RlvglEmitCtx` extended with `dm_field_ids: Vec<String>` (snapshot of `state_machine.datamodel`) and `used_dm_fields: Vec<String>` (first-use ordered set of DM fields actually consumed by lowered MachineBindings, used to emit one `format_dm_<field>` free fn per name). `build_screen`'s 4-tuple binding slot retyped `Vec<LabelBinding>` → `Vec<Binding>` on SM-attached modules. Helper signatures' `&mut Vec<LabelBinding>` → `&mut Vec<Binding>` on SM-attached modules. `refresh_bindings` signature widened with `&Rc<RefCell<Machine>>` between `state` and `bindings` on SM-attached modules (matched to a sealed-enum dispatch body that calls `lb.refresh(&s)` or `mb.refresh(&m.dm)`); QT-04e shape preserved verbatim on non-SM. New `text: sm.dm.<field>` Label-text grammar (recognised by `parse_dm_text_ref` matching the literal `sm.dm.…` prefix) lowers to a `Binding::Machine(MachineBinding { label: …, accessor: format_dm_<field> })` push under a `// QT-05c machine-bound:` marker; initial-text is `format_dm_<field>(&machine.borrow().dm)` mirroring QT-04c's build-time read pattern. Unknown DM field reference → emit-time panic naming the offending QML expression and the known field set. Per-field `format_dm_<field>(dm: &DataModel) -> String` free functions emitted at the tail of `render_rlvgl`, exactly once per used field, in first-use order; the body is `dm.<field>.to_string()` matching `f64::to_string`'s deterministic decimal output. `<sm>_gen::DataModel` joins the QT-05b `Event`/`Machine` import set on SM-attached modules; `#![allow(unused_imports)]` covers SM-attached modules with no DM bindings. `QT_EMIT_VERSION_RLVGL` bumped `12 → 13`. `tests/fixtures/qt/stopwatch.qml` extended with a `counter` Label whose `text: sm.dm.elapsed` exercises the grammar; the existing `display` Label's QT-04e `text: root.title` binding now wraps as `Binding::Label(...)` to verify the sealed enum coexistence. New compile-as-mod gate `generated_stopwatch_module_lowers_dm_text_binding` asserts the full reactive contract end-to-end: initial bound text is `"0"` from `machine.dm.elapsed = 0.0`; `dm.elapsed = 12.5` without refresh leaves the Label at `"0"` (QT-04e §9 caller-driven contract); `refresh_bindings(&state, &machine, &bindings)` re-reads and updates text to `"12.5"`; the QT-04e binding still updates from `state.title` mutations independently. All 9 existing rlvgl-target compile-gate version assertions bumped `12 → 13`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). QT-05b §3 / §15 records the 4-tuple slot retype. QT-04e §15 records the `refresh_bindings` signature widening on SM-attached modules. VisibilityFromState (`visible: sm.state == State::…`), color-from-DM, Button-text-from-DM, and multi-field formatters remain deferred under future Specification-Required amendments. |
| 2026-04-29 | QT-05b ratified + shipped ([`05b-handler-dispatch.md`](./05b-handler-dispatch.md)). `qt::render_rlvgl` is now reactive to `module.state_machine`: when populated, `build_screen` returns a 4-tuple `(WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<<sm>_gen::Machine>>, Vec<LabelBinding>)` and every helper gains `machine: Rc<RefCell<Machine>>` between `state` and `label_bindings`. New `RlvglEmitCtx.sm_id` field drives all conditional emit logic. New `dispatch("<event>")` handler grammar (`lower_dispatch_body` + `parse_dispatch_call` + `pascal_case_event`) lowers to `m.dispatch(Event::<Pascal>)` under a `// QT-05b dispatch:` marker. `<Pascal>` normalisation matches istate's `to_rust_ident \| capitalize` rule (snake_case / kebab-case / dotted forms split on word boundaries; numeric leading character → emit error; empty → emit error). New emit constants `ISTATE_LINKAGE_VERSION = 1` and `QT_SM_NAME = "<sm>"` appear at the top of every SM-attached module. New `use <sm>_gen::{Event, Machine};` import joins the existing rlvgl-core / rlvgl-widgets imports. `QT_EMIT_VERSION_RLVGL` bumped `11 → 12`. When `state_machine` is `None`, the QT-04e 3-tuple shape is preserved verbatim — every pre-QT-05 fixture's emitted Rust changes only the `QT_EMIT_VERSION` const. New mock istate crate `tests/fixtures/qt/stopwatch_gen/` (path dev-dep of `rlvgl`) hand-implements the QT-05 §6 6-symbol linkage surface (`Machine`, `Machine::new`/`with_options`/`dispatch`, `Event` (Start/Stop/Reset/Stopped), `State` (Idle/Running), `DataModel` (elapsed/lap), `Externals` + `DefaultExternals`) with semantics matching `stopwatch.scjson`'s transition table. New compile-as-mod gate `tests/creator_qt_emit_stopwatch_compile.rs` destructures the 4-tuple, fires synthetic `Event::PressRelease` on Start/Stop/Reset buttons, asserts `machine.borrow().state` flips Idle → Running → Idle and that emit-shape constants are present. All 9 existing rlvgl-target compile-gate version assertions bumped `11 → 12`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). QT-04b §3 / §15 records the 4-tuple amendment-when-SM-attached. State-gated handlers, `dm` mutation in handlers, and Button-text / DM-text bindings remain deferred to QT-05c. |
| 2026-04-29 | QT-05a ratified + shipped ([`05a-scjson-ingest.md`](./05a-scjson-ingest.md)). `qt::ingest`, `qt::emit`, and `qt::check` now probe `<basename>.scjson` sibling files and walk them via `qt_scjson::Scxml` → `UiStateMachine` per QT-05a §6. New code in `src/bin/creator/qt.rs`: `find_scjson_side_file`, `attach_scjson_side_file`, `derive_state_machine_id`, `walk_scxml_into_ui_state_machine`, `walk_states`, `lower_action_block`, `lower_action_block_exit`, `lower_transition_actions`, `extract_script_name`, `snake_case_for_sm`. Side-file discovery rule (case-sensitive sibling lookup with symlink-following), walk algorithm (depth-first state flatten + scjson element subset surface), error contract (silent fall-through on missing scjson; hard error on empty/malformed/not-actually-scjson), and `<sm>` ID derivation (snake_cased QML stem; `<scxml name>` overrides) frozen. New fixture `tests/fixtures/qt/stopwatch.qml` + `stopwatch.scjson` + 3 byte-equality drift gates with structural assertions on the populated `state_machine` field. Plus 2 contract gates: malformed scjson is a hard error; missing scjson is silent fall-through. Schema gate extended to verify `$defs/UiStateMachine`/`UiState`/`UiTransition`/`UiAction`/`UiDmField`/`UiScript`/`UiScriptOrigin` are present. All 9 pre-QT-05 fixture goldens unchanged (silent fall-through enforced by their existing drift gates). No `QT_IR_VERSION` bump (the additive field shipped at QT-05). No `QT_EMIT_VERSION_RLVGL` bump (emit shape unchanged — QT-05b ships the first emit change). |
| 2026-04-29 | **QT-05 ratified** ([`05-state-machines.md`](./05-state-machines.md)) — concepts only; emit changes ship in QT-05a-e. Phase set in §5 expanded: `QT-05` plus reserved sub-phases `QT-05a/b/c/d/e`. `vendor/scjson/` git submodule added (`https://github.com/SoftOboros/scjson.git`, BSD-1-Clause, reference-only — never a Cargo dep; Cargo workspace excludes it). 6-symbol istate Rust linkage surface frozen under Standards Action: `Machine`, `Machine::new`/`with_options`, `Machine::dispatch`, `Event`, `State`, `DataModel`, `Externals`+`DefaultExternals`. New constant `ISTATE_LINKAGE_VERSION = 1` pinned to istate's std-profile Rust template (`backend/templates/codegen/rust/src/lib.rs.jinja2`); `no_std` linkage reserved for v2. scjson element subset (10 elements: scxml/state/transition/datamodel/data/onentry/onexit/assign/raise/script) frozen under Specification Required. New IR types: `UiStateMachine`, `UiState`, `UiTransition`, `UiDmField`, `UiScript`, `UiScriptOrigin`. `UiModule` gains additive `state_machine: Option<UiStateMachine>`. `QT_IR_VERSION` bumped `1 → 2`. `QT_EMIT_VERSION_RLVGL` unchanged at `11` (concepts only — emit bumps to 12 land in QT-05b). `QT_EMIT_VERSION_DATA` unchanged. `// QT-05a/b/c/d/e` marker prefixes reserved. File layout frozen: `<screen>.scjson` side-files, `crates/<sm>_gen/` istate output, `src/<screen>_externals.rs` for QT-05e stubs. Hand-rolled scjson subset added at `src/bin/creator/qt_scjson.rs`. README §9 SCXML-vs-table question resolved (scjson + istate-codegen). QT-05a-e deferred to follow-on chapters; their dependencies on QT-05 §3/§5/§6/§7 are pinned in QT-05 §13. |
| 2026-04-29 | QT-03c ratified + shipped ([`03c-anchor-resolver.md`](./03c-anchor-resolver.md)). `anchors.centerIn: parent` with literal child `width`/`height` lowers to centered `Rect` arithmetic with a `// QT-03c centered:` marker; unhandled anchors get a `// emitter-skipped (QT-03c+):` comment naming the QML target. `QT_EMIT_VERSION_RLVGL` bumped `5 → 6`. New `centered.qml` fixture (200×200 parent, 50×50 centered child) + 3 goldens + 3 drift gates + bounds-assertion compile-as-mod gate that verifies the runtime widget bounds land at `(75, 75, 50, 50)`. All existing rlvgl-target goldens regenerated for the version bump; compile-as-mod gates' version assertions updated. Remaining anchor variants (`horizontalCenter`, `verticalCenter`, `left`, `right`, `top`, `bottom`, sibling-relative) tracked under §5 amendments. QT-03b §7 amended to point at this chapter's promotion table. |
| 2026-04-29 | QT-04c ratified + shipped ([`04c-initial-value-bindings.md`](./04c-initial-value-bindings.md)). **Phase re-split**: the original "QT-04c — MouseArea / hover handlers" intent (from QT-04 §10) and the broader QT-04b §10 framing of "QT-04c = reactive + MouseArea + nested IDs" are *both superseded*. New phase set: `QT-04c` = initial-value text bindings (this chapter); `QT-04d` = MouseArea / hover; `QT-04e` = reactive bindings; `QT-04f` = nested ID resolution. Implementation: `text:` expressions matching `<ident>` or `<root_id>.<ident>` against a root-scope `string` field lower to `state.borrow().<field>.clone()` at construction with a `// QT-04c bound:` marker; everything else falls through to a renamed `// TODO QT-04e: reactive bind text` comment. `QT_EMIT_VERSION_RLVGL` bumped `4 → 5`. New `bound_text.qml` fixture + 3 goldens + 3 drift gates + non-reactive compile-as-mod gate. `hello.rlvgl.rs` regenerated — its `QC.Label` now reads `state.title`. No IR type changes. |

---

MIT-licensed: MIT.
