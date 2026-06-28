<!-- README.md - Qt/QML ingestion initiative for rlvgl-creator. -->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Qt Support

> **Status: phase QT-01a shipped — `rlvgl-creator qt ingest`.** Later
> phases (QT-02 onwards) are still aspirational. Per the
> **Spec-Before-Code Planning Discipline** in [`CLAUDE.md`](../../CLAUDE.md),
> this README is **informative**; the per-phase concepts docs starting
> with [`00-concepts.md`](./00-concepts.md) are the normative artifacts.

## Chapters

| Phase    | Doc                                                    | Status                     |
| -------- | ------------------------------------------------------ | -------------------------- |
| `QT-00`  | [`00-concepts.md`](./00-concepts.md)                   | normative — ratified       |
| `QT-01a` | structural ingest MVP — see [`../creator/QT-INGEST.md`](../creator/QT-INGEST.md) | shipped                    |
| `QT-01b` | type-introspection ingest (PySide6 / `qmlplugindump`)  | not started                |
| `QT-02`  | [`02-ir-schema.md`](./02-ir-schema.md)                 | normative — ratified       |
| `QT-03`  | [`03-rlvgl-emitter-widgets.md`](./03-rlvgl-emitter-widgets.md) — data-only Rust emit | normative — ratified       |
| `QT-03b` | [`03b-rlvgl-widget-mapping.md`](./03b-rlvgl-widget-mapping.md) — widget API mapping (QML → `rlvgl-widgets`) | normative — ratified; initial implementation shipped |
| `QT-03c` | [`03c-anchor-resolver.md`](./03c-anchor-resolver.md) — anchor resolver beyond fill/margins | normative — ratified; `anchors.centerIn` + single edges + corner combinations shipped; axial fills (`left+right`, `top+bottom`) and `*Center` / sibling-relative under §5 amendments |
| `QT-04`  | [`04-signal-handlers.md`](./04-signal-handlers.md) — signal handlers (`onClicked` on `Button`)   | normative — ratified; initial implementation shipped |
| `QT-04b` | [`04b-properties-bindings.md`](./04b-properties-bindings.md) — properties + bindings | normative — ratified; implementation shipped |
| `QT-04c` | [`04c-initial-value-bindings.md`](./04c-initial-value-bindings.md) — initial-value text bindings | normative — ratified; implementation shipped |
| `QT-04d` | [`04d-mousearea.md`](./04d-mousearea.md) — MouseArea support via new `ClickArea` widget | normative — ratified; implementation shipped (onClicked; hover deferred) |
| `QT-04e` | [`04e-reactive-bindings.md`](./04e-reactive-bindings.md) — reactive Label-text bindings (closes out QT-04 family) | normative — ratified; implementation shipped |
| `QT-04f` | [`04f-nested-id-resolution.md`](./04f-nested-id-resolution.md) — nested ID resolution | normative — ratified; implementation shipped |
| `QT-05`  | [`05-state-machines.md`](./05-state-machines.md) — istate-codegen linkage (concepts) | normative — ratified; implementation pending in QT-05a-e |
| `QT-05a` | [`05a-scjson-ingest.md`](./05a-scjson-ingest.md) — scjson side-file ingest | normative — ratified; implementation shipped |
| `QT-05b` | [`05b-handler-dispatch.md`](./05b-handler-dispatch.md) — rlvgl emit: handler dispatch glue | normative — ratified; implementation shipped |
| `QT-05c` | [`05c-machine-bindings.md`](./05c-machine-bindings.md) — rlvgl emit: DataModel-driven Label bindings | normative — ratified; implementation shipped |
| `QT-05d` | [`05d-emit-scjson.md`](./05d-emit-scjson.md) — QML `states:` / `transitions:` → scjson emission | normative — ratified; implementation shipped |
| `QT-05e` | [`05e-externals-stubs.md`](./05e-externals-stubs.md) — Externals stub emission (closes QT-05) | normative — ratified; implementation shipped |
| `QT-05g` | [`05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md) — rlvgl emit: state-predicate → Image-source bindings (`is_active`); istate linkage v2 | normative — ratified (concepts); implementation in progress |
| `QT-06`  | [`06-theme-tokens.md`](./06-theme-tokens.md) — Qt theme → tokens.yaml | normative — ratified; implementation shipped |
| `QT-07`  | [`07-asset-handoff.md`](./07-asset-handoff.md) — Qt-referenced asset inventory | normative — ratified; implementation shipped |
| `QT-08`  | [`08-multi-file-cli.md`](./08-multi-file-cli.md) — directory-mode CLI | normative — ratified; directory-mode shipped |
| `QT-08b` | [`08b-qmldir-resolution.md`](./08b-qmldir-resolution.md) — qmldir manifest parser | normative — ratified; implementation shipped |
| `QT-08c` | [`08c-qrc-resources.md`](./08c-qrc-resources.md) — .qrc XML parser | normative — ratified; implementation shipped |
| `QT-09`  | [`09-desktop-ui.md`](./09-desktop-ui.md) — desktop UI Qt menu group | normative — ratified; UI wiring shipped; both creator and creator_ui feature builds verified |
| `QT-10`  | [`10-release.md`](./10-release.md) — strict-mode acceptance + release tag (auto-mode half) | normative — ratified; strict-mode-1 shipped (release tag is user-driven) |

---

## §1 — Purpose

Add **Qt / QML as a first-class authoring source** to `rlvgl-creator`,
joining the existing Chakra and Svelte ingestion paths. The output target
for this initiative is the existing **rlvgl (Rust)** code-generation
surface; a sibling C++ runtime (lvglpp) consuming the same IR is tracked
in a separate thread and is **out of scope here**.

Concretely: take a Qt/QML project (designer-authored UI types, properties,
signals, optional state machines), normalise it into a versioned neutral
IR inside `rlvgl-creator`, and emit `no_std + alloc`-friendly Rust that
plugs into the existing rlvgl widget tree, theme tokens, and asset crate
scaffolding.

---

## §2 — Problem Statement

`rlvgl-creator` already ingests several authoring formats and emits
`no_std`-friendly artifacts:

| Source                  | Driver in creator                                   | Output                                 |
| ----------------------- | --------------------------------------------------- | -------------------------------------- |
| Chakra TS theme         | [`src/bin/creator/chakra.rs`](../../src/bin/creator/chakra.rs) | `tokens.yaml`                  |
| Svelte token YAML       | [`src/bin/creator/svelte.rs`](../../src/bin/creator/svelte.rs) | `tokens.json`/`.css`/`.cjs` + rlvgl glue |
| Raw RGBA / fonts / Lottie | [`docs/creator/ASSET-PIPELINE.md`](../creator/ASSET-PIPELINE.md) | Dual-mode assets crate |
| Vendor BSP YAML         | [`src/bin/creator/bsp/`](../../src/bin/creator/bsp/) | Per-board PAC / BSP modules            |
| ST Cube `.ioc`          | [`docs/bsp/IOC-IR-ALIGNMENT.md`](../bsp/IOC-IR-ALIGNMENT.md) | STM32 BSP via shared IR         |

What is **missing** is an authoring path for full UI screens — types,
properties, signals, optional state machines — driven by a designer
tool. Qt Designer / Qt Design Studio is the most widely deployed such
tool. The closest existing primitive is the Chakra/Svelte token pipeline,
but tokens encode *theme*, not *structure*. There is no creator pass
today that produces a `widget_tree.rs` analogous to how
`svelte tokens` produces a `tokens.css`.

A Qt ingestion path closes this gap and reuses three pieces creator
already owns: the asset pipeline, the token pipeline, and the
`no_std + alloc`-aware emitter discipline (see
[`docs/creator/TEMPLATES.md`](../creator/TEMPLATES.md)).

---

## §3 — Architecture (informative)

```
Qt / QML  (authoring truth — Qt Design Studio, .qml, qmldir)
    │
    ▼  qmlplugindump · QMetaObject · PySide6 introspection
qt-ir   (versioned, neutral, serde-serializable — internal to creator)
    │
    ▼  rlvgl-creator emitter (MiniJinja templates, like BSP gen)
rlvgl Rust crate output
    ├─ widget tree (uses existing rlvgl-ui Stack/Box/etc.)
    ├─ theme tokens (composed with the chakra/svelte token output)
    └─ assets handle (consumes the existing assets-crate scaffolding)
```

Three properties make this fit the creator's existing shape:

1. **Qt is an ingest plugin, not a runtime.** Nothing Qt-specific runs
   on-device. The whole Qt dependency tree (PySide6, qmlplugindump,
   etc.) lives in the creator's `std` host environment, exactly like
   the Chakra TS parser does today.
2. **`qt-ir` is internal.** It is a creator-private serialisation
   format, not a publishable wire protocol. Versioning it lets us
   change designer-side and emitter-side independently, but it is
   not an API surface.
3. **Emission is template-driven.** The MiniJinja template discipline
   used by the BSP generator
   ([`src/bin/creator/bsp/templates/`](../../src/bin/creator/bsp/)) applies
   verbatim — `peripherals.rs.jinja` / `clocks.rs.jinja` etc. become
   `widget_tree.rs.jinja` / `screens.rs.jinja` for Qt output.

---

## §4 — IR Schema Seed (informative)

The shape below is a starting sketch; the normative IR is owned by
`QT-02` once that chapter is ratified. Property and signal sets are
expected to be **frozen enumerations** under the **Specification
Required** policy (see *Frozen enumerations* in `CLAUDE.md`).

```rust
// crate-private inside rlvgl-creator; serde, no_std-clean for emit-side use.
#[derive(Serialize, Deserialize)]
pub struct UiModule<'a> {
    pub version: u32,
    pub types:   &'a [UiType<'a>],
}

#[derive(Serialize, Deserialize)]
pub struct UiType<'a> {
    pub name:          &'a str,
    pub base:          Option<&'a str>,
    pub properties:    &'a [UiProperty<'a>],
    pub signals:       &'a [UiSignal<'a>],
    pub state_machine: Option<UiStateMachine<'a>>,
}

#[derive(Serialize, Deserialize)]
pub struct UiProperty<'a> {
    pub name:     &'a str,
    pub ty:       IrType,        // Bool | I32 | F32 | Str | Enum(&'a str) | …
    pub notify:   bool,
    pub writable: bool,
}
```

Open questions for `QT-02`:

- How to encode QML bindings (`property real foo: bar * 2`) without
  pulling a JS engine into the emitter. Candidates: pre-evaluate on the
  host, emit `const fn` where the binding is pure, fall back to a
  sealed expression enum.
- Whether Qt state machines lower to SCXML (already standardised) or
  to an enum + transition table. SCXML is more general; a transition
  table compiles smaller.
- How to map QML-side `Connections { }` blocks to rlvgl's existing
  signal/handler conventions (see [`docs/UI-COMPONENT-ARCHITECTURE.md`](../UI-COMPONENT-ARCHITECTURE.md)).

---

## §5 — Phase Plan

Suggested chapter sequence. Each phase produces a per-chapter normative
concepts doc under `docs/qt-support/NN-…md` following the §0–§15 doc
shape from `CLAUDE.md`. Commit-message prefix for this initiative is
**`QT-NN[a-z]:`** (e.g. `QT-02a:`, `QT-04b:`).

| Phase     | Title                                          | Owns vocabulary                                         | Acceptance gate (preview)                                                              |
| --------- | ---------------------------------------------- | ------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `QT-00`   | Concepts, scope, authority policy              | initiative-wide RFC 2119 keywords; chapter index        | Concepts doc ratified; this README updated to cite it.                                 |
| `QT-01`   | Qt ingestion surface                           | `qmlplugindump` invocation; PySide6 `QMetaObject` walk; supported Qt/QML version range | Smoke test ingests the Qt example `gallery.qml` and dumps a JSON IR.                   |
| `QT-02`   | `qt-ir` schema (frozen)                        | `UiModule`, `UiType`, `UiProperty`, `UiSignal`, `IrType`, `UiStateMachine` | Schema versioned at `v1`; round-trip serde test on representative module.              |
| `QT-03`   | rlvgl emitter (widgets)                        | template names; mapping from `UiType` to `rlvgl-ui` widget; theme-token interop | Generated crate renders a single QML screen in the rlvgl simulator.                    |
| `QT-04`   | rlvgl emitter (signals + bindings)             | binding lowering rules; signal handler glue            | Designer-authored counter / list-binding demo round-trips through ingest+emit.         |
| `QT-05`   | State machines (istate-codegen linkage)        | `UiStateMachine` IR shape; 6-symbol istate Rust linkage surface; scjson on-disk subset; `ISTATE_LINKAGE_VERSION` | A `.scjson` side-file linked to a QML screen generates a `<sm>_gen/` crate via istate-codegen MCP; rlvgl-emitted glue calls it through the QT-05 §6 frozen linkage surface; W3C-vector parity remains owned by istate's own suite. |
| `QT-06`   | Theme-token reconciliation                     | precedence rules between Qt theme, chakra tokens, svelte tokens | Single `tokens.yaml` derived from a Qt project matches the chakra/svelte pipeline output. |
| `QT-07`   | Asset-crate handoff                            | how Qt-referenced images/fonts flow into the existing dual-mode assets crate | Qt project's images and fonts vendor cleanly via `rlvgl-creator vendor`.               |
| `QT-08`   | CLI surface (`creator qt …`)                   | command names, flags, exit codes                       | `--help` examples in [`docs/creator/CLI.md`](../creator/CLI.md); end-to-end recipe documented. |
| `QT-09`   | Desktop-UI integration                         | menu placement (Assets/Build/Deploy), wizard flow      | UI parity with CLI per [`docs/creator/UI-DESIGN.md`](../creator/UI-DESIGN.md).         |
| `QT-10`   | Strict-mode acceptance + release tag           | conformance targets become MUST                        | All earlier phases' SHOULD gates promoted to MUST; v0.x.0 tag.                         |

Phase numbering is stable across chapters once `QT-00` ratifies — adding
a phase later uses a lettered suffix (`QT-04b`) per `CLAUDE.md`.

---

## §6 — Conformance Targets (informative until ratified by `QT-00`)

A conforming Qt-support implementation:

- **MUST** satisfy the acceptance gates of `QT-01` through `QT-08`
  (ingest, IR, widget emit, signal/binding emit, state-machine emit,
  token reconciliation, asset handoff, CLI surface).
- **SHOULD** satisfy `QT-09` (desktop UI parity).
- **MAY** additionally satisfy `QT-10` (strict mode + tag).

Each per-chapter doc names its own normative `Acceptance` checklist;
the items above are placeholders until those checklists are ratified.

---

## §7 — Reconciliation with Adjacent Creator Primitives

| Adjacent primitive                              | Relationship to Qt support                                                                  | Reconciliation owner |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------- | --- |
| Chakra TS → `tokens.yaml`                       | Both produce a token tree. Qt theme MUST funnel through the same `tokens.yaml` to avoid forking precedence rules. | `QT-06`            |
| Svelte token → multi-target output              | Same. Qt is a *new source* for `tokens.yaml`, not a parallel emitter.                       | `QT-06`            |
| Asset pipeline (raw RGBA / fonts / Lottie)      | Qt-referenced assets flow into the existing dual-mode assets crate; no new asset crate type. | `QT-07`            |
| BSP YAML / chipdb generator                     | Orthogonal. Qt produces UI; BSP produces hardware glue. They meet only at the workspace scaffold. | n/a (no overlap)   |
| `rlvgl-ui` (Chakra-inspired components)         | Qt emitter MUST target `rlvgl-ui` widgets, not raw `rlvgl-core` primitives.                 | `QT-03`            |
| MicroPython integration ([`MICROPYTHON-INTEGRATION.md`](MICROPYTHON-INTEGRATION.md)) | Orthogonal. MicroPython is a runtime API surface; Qt is a compile-time authoring surface. | n/a (no overlap)   |

---

## §8 — Non-goals

- **No on-device Qt runtime.** Qt, QML, PySide6, and `qmlplugindump`
  are creator-host-only dependencies.
- **No lvglpp emission.** A C++ companion runtime consuming the same
  `qt-ir` is tracked in a **separate thread** and is out of scope for
  this roadmap.
- **No live designer round-trip.** This roadmap is one-way: Qt → IR →
  rlvgl. Edits to generated Rust are not propagated back into the
  Qt project.
- **No QML JavaScript engine on-device.** Bindings are lowered at
  emit time; anything that cannot be lowered is rejected with a
  fix-it message.
- **No Qt licence redistribution.** Whatever Qt the user has
  installed is what the creator drives; the project does not vendor
  Qt itself.

---

## §9 — Open Questions / Unblocks

| Question                                                               | Resolution lives in     |
| ---------------------------------------------------------------------- | ----------------------- |
| Minimum Qt version (Qt 5.15 LTS vs Qt 6.x)?                            | `QT-01`                |
| Use `qmlplugindump` (binary, deprecated in Qt 6.4+) or PySide6?        | `QT-01`                |
| ~~SCXML vs transition table for state machines?~~ Resolved by QT-05: scjson + istate-codegen Rust template (std-profile). | `QT-05` (resolved 2026-04-29) |
| QML binding subset that lowers cleanly to `const fn` / pure expressions? | `QT-04`              |
| Token-precedence order when Qt theme + chakra + svelte all present?    | `QT-06`                |
| Should the desktop UI launch Qt Design Studio externally or embed a viewer? | `QT-09`           |

---

## §10 — Change Log

| Date       | Change                                                          |
| ---------- | --------------------------------------------------------------- |
| 2026-04-28 | Initial roadmap. Lvglpp emission explicitly out of scope; tracked separately. |
| 2026-04-30 | **QT-10 strict-mode-1 ratified** ([`10-release.md`](./10-release.md)). Closes the Qt-support roadmap's enforcement story (auto-mode half). New `pub const QT_FAMILY_STRICT_VERSION: u32 = 1;` in `src/bin/creator/qt.rs`. New meta-test `tests/creator_qt_strict_mode.rs` (5 assertions) enforces the chapter file set (24 entries — every `docs/qt-support/*.md` excluding README must appear and every named chapter must exist), every chapter carries the canonical `## §15 — Change Log` marker, the version-constant snapshot (`QT_IR_VERSION = 2`, `QT_EMIT_VERSION_RLVGL = 13`, `QT_EMIT_VERSION_DATA = 1`, `QT_EXTERNALS_VERSION = 1`, `QT_FAMILY_STRICT_VERSION = 1`), and the CLI subcommand surface (10 entries reachable via `qt --help`). Release-tag execution (cutting `v0.x.0`, `cargo publish`, GitHub Release notes) is documented in QT-10 §8 but **not performed** under auto mode per the safety protocol — the user runs it when ready. The Qt-support roadmap is feature-complete: 24 chapters ratified, 10 CLI subcommands shipped, full QML→scjson→Rust→glue+Externals+theme+assets+qmldir+qrc pipeline, desktop UI parity, strict-mode invariants under cargo-test enforcement. |
| 2026-04-30 | QT-09 §10 amendment #2 (same-day): silabs / microchip / ti chipdb crates migrated to the YAML accessor convention. Each crate gets `db/chips/*.yaml` + `db/boards/*.yaml` seeds (silabs `EFM32GG11`; microchip `ATSAMD51J19A`; ti `MSP432P401R` + `beaglebone_black_nhd_cape`/`AM335x`), nrf-style build.rs, accessor-surface lib.rs, per-crate smoke tests (4+4+5=13 tests). `Cargo.toml` adds `default = ["std"]` feature to silence cfg-warning. `boards.rs::load_ir` now takes the YAML path for all eight chipdb vendors; legacy raw_db error branch removed. Both feature gates still build clean; 13 new chipdb tests green. |
| 2026-06-27 | **QT-05g ratified (concepts) + QT-05 §6 linkage-v2 amendment.** Opens the scxml→istate→rlvgl reactive-binding integration. QT-05 §6 gains **linkage v2** (Standards Action): the istate **M1P6 dynamic-string machine surface** (`step(&str, Value)` / `is_active(&str)` / `get_var(&str)` / `current_state` / `active_states` / `Value` enum) the SCTD demo actually runs, which v1's `dispatch(Event)`/`state == State::X`/`dm.<f64>` surface could not describe. `ISTATE_LINKAGE_VERSION = 2` on v2-attached modules. QT-05g ([`05g-state-predicate-bindings.md`](./05g-state-predicate-bindings.md)) is its first consumer: a new `Binding::Predicate(PredicateBinding)` variant lowers `source: <ctx>.<state> ? "A" : "B"` (e.g. `scxmlBolero.mediaPlaying ? Pause : Play`) into a reactive `Image` artwork swap driven by `machine.is_active("<state>")`, via a new `--scxml-context <ctx>=<crate>` linkage flag for externally-injected SCXML context objects. Ratified decisions: PCDN-05g-1 (every UI predicate is a real `is_active` state — the SCXML re-models mute/shuffle/repeat as real parallel regions without cross-region `In()` guards) and PCDN-05g-2 (Play/Pause ships first; `mediaPlaying` is already a real state, so the first slice needs no remodel). `QT_EMIT_VERSION_RLVGL` 17→18 and the emit/`Image::set_pixels`/pixel-gate/skin-wiring changes land in QT-05g's implementation commits. Visibility (`visible: <ctx>.<state>`), colour, and external-media text deferred to QT-05h / later letters. Note: `QT-05f` stays reserved (stateful-externals consolidation per the 2026-04-29 QT-05e entry); the predicate-binding work takes the next free letter, `QT-05g`. |
| 2026-04-30 | QT-09 §10 caveat resolved (same-day amendment). The pre-existing `creator_ui` build break in `boards.rs::load_ir` (5 `raw_db` lookup errors against chip vendor crates that had migrated to `board_yaml` / `chip_yaml` per-spec YAML accessors) is fixed by rewriting `load_ir` to use the new accessor convention for nrf / esp / nxp / renesas / rp2040 and returning a typed "not yet supported" error for silabs / microchip / ti (whose source data is unwired). Dead zstd `parse_raw_db` helper + `zstd` / `std::io::Read` / `HashMap` imports removed. `cargo check --features creator,creator_ui` now builds clean. 263/263 tests still green. |
| 2026-04-30 | QT-09 ratified ([`09-desktop-ui.md`](./09-desktop-ui.md)); UI wiring shipped. Desktop UI exposes all 10 `qt …` CLI subcommands via a new "Qt" menu group with 10 entries (Qt Ingest / Qt Check / Qt Schema / Qt Emit / Qt Emit Scjson / Qt Emit Externals / Qt Emit Tokens / Qt List Assets / Qt List Qmldir / Qt List Qrc), each following the existing `handle_scan` file-picker → CLI fn → toast pattern. `creator_ui/mod.rs` adds `mod qt;` + `mod qt_scjson;` via `#[path]` from `creator/`. UI defaults to `qt emit --target rlvgl`; CLI remains canonical for advanced flags. 263/263 creator-feature tests still green. |
| 2026-04-30 | QT-08c ratified + shipped ([`08c-qrc-resources.md`](./08c-qrc-resources.md)). New CLI subcommand `qt list-qrc <input> [<out>]` parses the `.qrc` XML manifest subset (`<RCC version="…">`, `<qresource prefix="…" lang="…">`, `<file alias="…">…</file>` plus comments/DOCTYPE/XML decl) via a hand-rolled minimal XML walker — **no new Cargo deps**. New types `QrcManifest` / `QrcResource` / `QrcFile`. Strict at v1: unrecognised elements under `<RCC>` or `<qresource>` are emit-time errors. New fixture `tests/fixtures/qt/resources.qrc` (RCC version 1.0 + 2 qresource blocks + 4 files including 1 alias) + emitted golden `tests/fixtures/qt/resources.qrc.yaml` + 2 drift gates. No bumps to versioned emit-shapes. Cross-validation against QT-07 / QT-01a, file-existence checks, compression metadata, namespaces, and CDATA file content remain deferred. |
| 2026-04-30 | QT-08b ratified + shipped ([`08b-qmldir-resolution.md`](./08b-qmldir-resolution.md)). New CLI subcommand `qt list-qmldir <input> [<out>]` parses Qt module-system `qmldir` manifests and emits a stable `<dirname>.qmldir.yaml` inventory. New public types `QmldirManifest` / `QmldirType` / `QmldirImport` / `QmldirPlugin`. New parser `parse_qmldir(content) -> QmldirManifest` recognises `module`, ordinary type registrations (`<Name> <version> <file>.qml`), `singleton` and `internal` modifiers, `import` / `depends`, and `plugin` directives; comments and blank lines silently dropped; unrecognised non-empty lines preserved verbatim in `manifest.other`. Multiple `module` lines: last-one-wins per Qt's own behaviour. Output YAML preserves declaration order. Missing input file is a hard error (non-silent). New fixture `tests/fixtures/qt/sample_module/qmldir` (1 module + 2 types + 1 singleton + 1 internal + 2 imports/depends + 1 unrecognised typeinfo line) + emitted golden `tests/fixtures/qt/sample_module.qmldir.yaml` + 2 drift gates (byte-equality + missing-input hard error). No bumps to versioned emit-shapes (QT-08b's artifact is a separate file). Cross-import resolution at QT-01a, qmltypes parsing, classname/prefer/optional/designersupported directives, recursive bundle expansion, and singleton-driven theme auto-discovery for QT-06 remain deferred. |
| 2026-04-30 | QT-07 ratified + shipped ([`07-asset-handoff.md`](./07-asset-handoff.md)). New CLI subcommand `qt list-assets <input> [<out>]` (file mode + directory mode per QT-08) walks every `UiItem` and extracts asset references: `Image { source: "qrc:[/]…" or "<path>" }` (qrc:/ and qrc:/// prefix-stripped), `Text { font.family: "<name>" }`, dotted `<*>.font.family: "<name>"`, and standalone `Font { family: "<name>" }` blocks. Outputs a `<basename>.assets.yaml` with stable `version: 1` + `images: […]` + `fonts: […]` lists; uses `BTreeSet` for dedup + lexical ordering, YAML scalars quoted on whitespace/metachars (e.g. `"FiraSans Bold"`). Silent skip on QML with no recognised refs. Inventory is intentionally **not** a manifest — manifest merge stays user-driven at v1; a sibling `manifest merge-qt` subcommand is reserved for future amendments. New fixture `tests/fixtures/qt/image_refs.qml` (4 Image declarations: 3 distinct paths after dedup with mixed qrc/relative forms; 3 Text declarations: 2 distinct font families after dedup) + emitted golden `tests/fixtures/qt/image_refs.assets.yaml` + 2 drift gates (byte-equality + silent-skip-on-non-asset-QML exercising counter.qml). No bumps to versioned emit-shapes (QT-07's artifact is a separate file). State-bound `source:` expressions, `font.weight`/`font.pointSize` derivation, qrc bundle resolution (QT-08c), `.qmldir` external assets (QT-08b), `AnimatedImage` frame folders, and localised variants remain deferred under future Specification-Required §5 amendments. |
| 2026-04-30 | QT-06 ratified + shipped ([`06-theme-tokens.md`](./06-theme-tokens.md)). New CLI subcommand `qt emit-tokens <input> [<out>]` walks root-level `property color/int/string` declarations and emits a `<basename>.tokens.yaml` matching the existing chakra/svelte schema (`version: 1` + `colors:` + `spacing:` + `radii:` + `fonts:` + optional `modes.dark.colors:`). Name-to-category rules (§6): `color` → `colors.<name>`; `color` with `_dark` suffix → `modes.dark.colors.<name>`; `int spacing_<key>` → `spacing.<key>`; `int radius_<key>` → `radii.<key>`; `string font_<key>` → `fonts.<key>`. Hex-only at v1 (`^#[0-9a-fA-F]{3,8}$`); rgba/hsl/named-colors and Material/Universal style-system parsing deferred. Lexical key ordering for byte-stable output. Silent skip for QML with no recognised theme properties. Multi-source precedence stays a user concern at v1 (user picks one canonical filename); a `qt merge-tokens` overlay subcommand is reserved for v2. New fixture `tests/fixtures/qt/Theme.qml` (4 colors + 5 spacing + 5 radii + 3 fonts + 2 dark-mode colors) + emitted golden `tests/fixtures/qt/Theme.tokens.yaml` + 2 drift gates (byte-equality + silent-skip). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` (QT-06's artifact is a separate file). |
| 2026-04-29 | **QT-05 family closed out.** QT-05e ratified + shipped ([`05e-externals-stubs.md`](./05e-externals-stubs.md)). New CLI subcommand `qt emit-externals <input> [<out>]` walks `module.state_machine.scripts` and writes a sibling `<basename>_externals.rs` containing a `pub struct ScreenExternals` with `impl Externals for ScreenExternals` covering one method per discovered script. Method bodies are TODO stubs with `// QT-05e externals-stub: <name> from <origin>` markers; users fill in side-effect code by hand. Per-file emit-shape constant `QT_EXTERNALS_VERSION = 1`. Install path documented per §7: assign to the public `Machine.externals: Box<dyn Externals>` field from linkage v1. New emitted golden `tests/fixtures/qt/stopwatch_externals.rs` (stopwatch.scjson has `tick_start` / `tick_stop` scripts) + 1 byte-equality drift gate + 1 install-path compile-as-mod test. No version bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `ISTATE_LINKAGE_VERSION` (QT-05e's artifact is a separate file). Stateful externals, merge-on-regen, and multi-screen consolidation deferred to a hypothetical future QT-05f. With QT-05e the QT-05 family is feature-complete; subsequent Qt work moves to QT-06 (theme tokens), QT-07 (asset handoff), or QT-08b/c (`.qmldir` / `.qrc`). |
| 2026-04-29 | QT-05d ratified + shipped ([`05d-emit-scjson.md`](./05d-emit-scjson.md)). New CLI subcommand `qt emit-scjson <input> [<out>]` (file mode + directory mode per QT-08) walks inline `states: [State { name; initial }]` and `transitions: [Transition { from; to; event }]` blocks at the QML root and writes a sibling `.scjson` document. New pure walker `walk_qml_state_machine(item, source) -> Result<Option<Scxml>>` lifts the QML idiom into `qt_scjson::Scxml`. Provenance tagged via `_comment: "QT-05d emit-scjson: <path>"` in `Scxml.other_attributes`. Round-trip parity contract: emit-scjson → write `.scjson` → QT-05a re-ingest produces a `UiStateMachine` with `states`/`transitions`/`initial` shape-equal to the QML's declarations. Multiple `initial: true` → emit-time error; missing `name`/`from`/`to` → error; transition referencing unknown state → error. Animation-flavoured `Transition` properties (`signal`, `PathAnimation`), `PropertyChanges`, wildcard `from: "*"`, and the `QtQml.StateMachine` framework form silently dropped or deferred per §5. New fixture `tests/fixtures/qt/inline_states.qml` + emitted golden `tests/fixtures/qt/inline_states.scjson` + 2 round-trip drift gates. No `QT_IR_VERSION` or `QT_EMIT_VERSION_RLVGL` bump (QT-05d's artifact is separate from the versioned emit-shapes). |
| 2026-04-29 | QT-05c ratified + shipped ([`05c-machine-bindings.md`](./05c-machine-bindings.md)). New emitted `pub enum Binding { Label(LabelBinding), Machine(MachineBinding) }` sealed enum and new `pub struct MachineBinding { label, accessor: fn(&DataModel) -> String }`. `build_screen`'s 4-tuple slot retyped from `Vec<LabelBinding>` to `Vec<Binding>` on SM-attached modules. Helper signatures' `&mut Vec<LabelBinding>` becomes `&mut Vec<Binding>` on SM-attached modules. `refresh_bindings` signature widened with `&Rc<RefCell<Machine>>` between `state` and `bindings` on SM-attached modules; QT-04e shape preserved on non-SM. New `text: sm.dm.<field>` Label-text grammar lowers under a `// QT-05c machine-bound:` marker. Per-binding-site `format_dm_<field>` free functions emitted (`f64::to_string` representation). Unknown DM field → emit-time error. `<sm>_gen::DataModel` joins the import set on SM-attached modules. `QT_EMIT_VERSION_RLVGL` bumped `12 → 13`. `tests/fixtures/qt/stopwatch.qml` extended with a `counter` Label whose `text: sm.dm.elapsed` exercises the grammar. New compile-as-mod gate `generated_stopwatch_module_lowers_dm_text_binding` mutates `machine.borrow_mut().dm.elapsed = 12.5`, calls `refresh_bindings`, asserts the bound Label's text became `"12.5"`. All 9 existing rlvgl-target compile-gate version assertions bumped `12 → 13`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). |
| 2026-04-29 | QT-05b ratified + shipped ([`05b-handler-dispatch.md`](./05b-handler-dispatch.md)). `qt::render_rlvgl` now reactive to `module.state_machine`: when populated, `build_screen` returns a 4-tuple `(WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<<sm>_gen::Machine>>, Vec<LabelBinding>)` and every helper threads `Rc<RefCell<Machine>>` between `state` and `label_bindings`. New `dispatch("<event>")` handler grammar lowers to `machine.borrow_mut().dispatch(<sm>_gen::Event::<Pascal>)` under a `// QT-05b dispatch:` marker; PascalCase normalisation matches istate's `to_rust_ident \| capitalize` rule. New emit constants `ISTATE_LINKAGE_VERSION = 1` and `QT_SM_NAME = "<sm>"` appear at the top of every SM-attached module. `QT_EMIT_VERSION_RLVGL` bumped `11 → 12`. New mock istate crate `tests/fixtures/qt/stopwatch_gen/` (matches QT-05 §6 6-symbol linkage surface) wired as dev-dep of rlvgl. New compile-as-mod gate `tests/creator_qt_emit_stopwatch_compile.rs` destructures the 4-tuple, fires synthetic clicks on Start/Stop/Reset, asserts `machine.borrow().state` flips Idle → Running → Idle. All existing rlvgl-target compile-gate version assertions bumped `11 → 12`. All existing rlvgl-target goldens regenerated for the version bump (otherwise byte-equal). When `state_machine` is `None`, the QT-04e 3-tuple shape is preserved verbatim — full backwards-compat. |
| 2026-04-29 | QT-05a ratified + shipped ([`05a-scjson-ingest.md`](./05a-scjson-ingest.md)). `qt::ingest` (file mode + dir mode) and `qt::emit` and `qt::check` now probe `<basename>.scjson` sibling files and walk them via `qt_scjson::Scxml` → `UiStateMachine` per QT-05a §6. Side-file discovery rule, walk algorithm, error contract (silent fall-through on missing, hard error on malformed), and `<sm>` ID derivation (snake_cased QML stem; `<scxml name>` overrides) frozen. New fixture: `tests/fixtures/qt/stopwatch.qml` + `stopwatch.scjson` + 3 drift gates pinning the populated `state_machine` field shape. Plus 2 contract gates: malformed scjson is a hard error; missing scjson is silent fall-through. All 9 pre-QT-05 fixture goldens unchanged (silent fall-through enforced by their existing drift gates). Schema gate extended to verify `$defs/UiStateMachine`/`UiState`/`UiTransition`/`UiAction`/`UiDmField`/`UiScript`/`UiScriptOrigin` are present. No `QT_IR_VERSION` bump (the additive field already shipped at QT-05). No `QT_EMIT_VERSION_RLVGL` bump (emit shape unchanged — QT-05b ships the first emit change). |
| 2026-04-29 | QT-05 ratified ([`05-state-machines.md`](./05-state-machines.md)). Concepts only — no emit changes ship in QT-05. Replaces the QT-05 acceptance gate from "PySide6 parity" with "scjson + istate-codegen pipeline; W3C parity owned by istate". `vendor/scjson/` submodule added (BSD-1-Clause, reference-only — never a Cargo dep). 6-symbol istate Rust linkage surface frozen under Standards Action; `ISTATE_LINKAGE_VERSION = 1` pinned to istate's std-profile template. scjson element subset (10 elements) frozen under Specification Required. IR types `UiStateMachine`/`UiState`/`UiTransition`/`UiDmField`/`UiScript` added; `UiModule` gains `state_machine: Option<UiStateMachine>`. `QT_IR_VERSION` bumped `1 → 2`. §9 SCXML-vs-table question resolved (scjson + istate-codegen). QT-05a-e remain to ship the implementation. |

---

MIT-licensed: MIT.
