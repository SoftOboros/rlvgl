<!--
07-asset-handoff.md - QT-07: asset-crate handoff.
-->

**[← Prev](06-theme-tokens.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-07 — Asset-Crate Handoff

QT-07 introduces a one-way **discovery** path between Qt-authored
QML and the existing `rlvgl-creator` asset pipeline (icons / fonts
/ media folders + `manifest.yml`). The goal: enumerate every asset
a given `.qml` file references so the user can vendor them
through the existing `rlvgl-creator scan` / `rlvgl-creator vendor`
flow without manual auditing.

QT-07 introduces no new IR types, no new emit-shape constants, and
no parallel asset-crate type. It introduces a new walker, a new
CLI subcommand, and a YAML inventory shape that downstream tools
can ingest.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
existing asset-pipeline policy (folder roots `icons/` / `fonts/`
/ `media/`, `manifest.yml` shape, naming rules) is owned by
[`docs/creator/ASSET-PIPELINE.md`](../creator/ASSET-PIPELINE.md).
QT-07 produces an inventory document that does **not** modify the
manifest; merging is a user-driven step (or a future amendment).

The QT-07 inventory schema, the QML reference forms recognised,
the classification rules, and the `qt list-assets` CLI are owned
here.

## §1 — Purpose

A real Qt screen carries `Image { source: "icons/play.png" }` and
`Text { font.family: "Roboto" }` references that the asset
pipeline doesn't see until the user manually mirrors them in
`manifest.yml`. After QT-07:

```bash
$ rlvgl-creator qt list-assets screens/Player.qml
$ # writes screens/Player.assets.yaml — the canonical inventory.
```

The user reviews the inventory, drops the missing assets into the
pipeline's folder roots, and runs `rlvgl-creator scan` /
`rlvgl-creator vendor` per the existing flow. The Qt project is
the source of truth for **which** assets are needed; the asset
pipeline is the source of truth for **how** they're named, packed,
and consumed.

## §2 — Problem Statement

Three concrete gaps:

- **No discovery.** Today, the only way to know a Qt screen needs
  `icons/play.png` is to read the QML by hand. For non-trivial
  projects (10+ screens, 50+ assets) this is painful and
  error-prone.
- **No drift detection.** A QML edit that adds an asset reference
  silently breaks the build; one that removes a reference leaves
  an orphaned asset in `manifest.yml` that bloats the vendor
  output.
- **No type discipline at the boundary.** A user asking "what
  fonts does this screen use?" needs to grep through QML for both
  `font.family` and `Font { family: ... }` and `font: Qt.font(...)`
  forms. QT-07 normalises them.

## §3 — Canonical Glossary (delta only)

QT-07 introduces no new IR types. One new walker, one CLI
subcommand, one inventory shape, one comment marker.

### `walk_asset_refs(item) -> AssetInventory`

Pure function. Recursively visits `item` and every descendant
`UiItem`, extracting:

- `Image { source: "<path>" }` and `Image { source: "qrc:/<path>" }`
  → `images.<path>` (qrc:/ prefix stripped).
- `Text { font.family: "<name>" }` → `fonts.<name>`.
- `font.family: "<name>"` direct assignment on any item →
  `fonts.<name>` (same).
- Standalone `Font { family: "<name>" }` blocks → `fonts.<name>`.

Deduplicates: identical entries (same path or family) appear once
in the output. Lexically ordered for byte-stable emit.

### `AssetInventory`

```rust
pub struct AssetInventory {
    pub images: BTreeSet<String>,
    pub fonts: BTreeSet<String>,
}
```

Private intermediate. Serialised by `render_assets_yaml`.

### `qt list-assets <input> [<out>]`

CLI subcommand. File mode: writes `<input_dir>/<basename>.assets.yaml`
or to the explicit `<out>` path. Directory mode: walks `*.qml` per
QT-08 and emits one `<basename>.assets.yaml` per QML that has any
asset references.

Silent skip for QML with no asset references.

### `// QT-07 assets:` marker

Emitted as a YAML comment at the top of the produced inventory
naming the source QML path. Reviewers grep on this exact prefix.

## §4 — Source-of-Truth Map

| Concept                                 | Owner                                                                  |
| --------------------------------------- | ---------------------------------------------------------------------- |
| Folder roots, manifest, naming policy   | `docs/creator/ASSET-PIPELINE.md`.                                       |
| QML structural parse (`UiItem`)         | QT-01a.                                                                 |
| Asset reference forms recognised        | this chapter (§5).                                                      |
| Inventory YAML schema                   | this chapter (§6).                                                      |
| Classification rules                    | this chapter (§5 / §6).                                                 |
| `qt list-assets` CLI                    | this chapter (§7).                                                      |
| Deduplication + lexical ordering        | this chapter (§3 / §6).                                                 |
| Manifest merge / round-trip             | **deferred** — user-driven at v1.                                       |
| Asset path resolution against `qrc:`/relative paths | **deferred** — paths recorded verbatim minus `qrc:` prefix. |

## §5 — Frozen Decision: Recognised Reference Forms

Registration policy: **Specification Required**.

| QML form                                          | Inventory bucket  | Stored as                           |
| ------------------------------------------------- | ----------------- | ----------------------------------- |
| `Image { source: "<path>" }`                      | `images`          | `<path>` verbatim                   |
| `Image { source: "qrc:/<path>" }`                 | `images`          | `<path>` (qrc:/ prefix stripped)    |
| `Image { source: "qrc:///<path>" }`               | `images`          | `<path>` (triple-slash also stripped) |
| `Text { font.family: "<name>" }`                  | `fonts`           | `<name>` verbatim                   |
| `<any>.font.family: "<name>"` (dotted target)     | `fonts`           | `<name>`                            |
| `Font { family: "<name>" }` standalone object     | `fonts`           | `<name>`                            |
| `Image { source: someBinding }` (non-literal)     | silently dropped  | n/a                                 |
| `font: Qt.font({family: ...})` JS-call form       | silently dropped  | n/a                                 |
| `Image { source: "https://…" }`                   | `images`          | recorded verbatim (downstream policy decides) |
| Image / font assigned via state-bound expression  | silently dropped  | n/a                                 |

The `qrc:/` and `qrc:///` prefix-stripping rules are deliberately
narrow: at v1 we don't resolve qrc bundles — `.qrc` resolution
lands in QT-08c. The intent of stripping is purely cosmetic so
the inventory matches the on-disk path the user types into the
`icons/` / `fonts/` / `media/` folders.

Deferred at v1:

- `.qrc` resource manifest reading (matches QT-08c scope).
- `.qmldir` modules with externally-declared assets.
- Image fallback chains (`source` set in onCompleted handlers).
- Font weight/style derivation (`font.weight: Font.Bold`).
- Localised asset variants.
- Animation-frame folders (`AnimatedImage { source: "media/explosion/" }`).

## §6 — Frozen Decision: Inventory YAML Shape

```yaml
# QT-07 assets: <source.qml>
version: 1
images:
  - icons/play.png
  - icons/stop.png
fonts:
  - "FiraSans Bold"
  - Roboto
```

| Field      | Required | Notes                                           |
| ---------- | -------- | ----------------------------------------------- |
| `version`  | yes      | Always `1` at QT-07 v1.                         |
| `images`   | yes      | Empty list if none — both keys always present so the schema is stable. |
| `fonts`    | yes      | Empty list if none.                             |

Lexical ordering applies to both lists.

The output is **not** a `manifest.yml` — it deliberately uses a
different schema (top-level `images:` / `fonts:` lists instead of
the manifest's nested entry map) so a user wiring it into the
manifest by hand cannot accidentally clobber existing entries.

## §7 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt list-assets <INPUT> [<OUT>]
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/screen.qml` | `dir/`             | (parent of input) | Writes `<OUT>/screen.assets.yaml`. |
| File | `path/screen.qml` | `dir/x.yaml`        | n/a               | Writes the named file. |
| Dir  | `path/screens/`   | `dir/`            | (input itself)    | For every `*.qml` with asset refs, writes `<OUT>/<basename>.assets.yaml`. |

Silent skip on QML with no recognised asset references.

## §8 — Versioning

| Constant                       | Before QT-07 | After QT-07 |
| ------------------------------ | ------------ | ----------- |
| `QT_IR_VERSION`                | 2            | unchanged   |
| `QT_EMIT_VERSION_RLVGL`        | 13           | unchanged   |
| `QT_EMIT_VERSION_DATA`         | 1            | unchanged   |
| `ISTATE_LINKAGE_VERSION`       | 1            | unchanged   |
| Inventory YAML `version:`      | (new)        | 1           |

QT-07's artifact is a separate `<basename>.assets.yaml` file. No
bumps to the versioned emit-shapes.

## §9 — Non-Goals

- **No manifest modification.** QT-07 produces an inventory; the
  user merges it into `manifest.yml` (or doesn't).
- **No asset-file copy.** QT-07 doesn't copy files from the QML
  project into `icons/` / `fonts/` / `media/`. The user does that
  before running `rlvgl-creator scan`.
- **No qrc bundle resolution.** QT-08c.
- **No hash-based dedup.** Two different `<path>` strings that
  resolve to the same file on disk show up twice in the inventory
  unless they have textually identical paths.
- **No image-format detection.** The inventory records the path
  verbatim; the asset pipeline owns format normalisation.
- **No widget-tree backreference.** Knowing that `icons/play.png`
  is referenced by `Image { id: playBtn }` is not preserved at v1
  — only the path/family is enumerated.
- **No state-binding tracking.** `Image { source: ui.icon }` is
  silently dropped because the value is a binding, not a literal.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                        | Resolution                                                                                          |
| -------- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| QT-01a   | Structural parse.                              | QT-07 reads existing `UiItem` shape. No parser changes.                                             |
| QT-04 family | Widget mapping.                            | Independent. Asset references are extracted from the IR pre-emit; no emit-side change.               |
| QT-05 family | State machines.                            | Independent.                                                                                         |
| QT-06    | Theme tokens.                                  | Independent. Theme exposes named tokens; assets are referenced by path/family.                       |
| QT-08    | Directory-mode CLI.                            | QT-07 reuses `qt08_collect_qml_files` for directory mode.                                            |
| QT-08c   | `.qrc` resource bundles (deferred).            | QT-07 strips `qrc:/` prefix only; full resolution waits for QT-08c.                                  |
| `docs/creator/ASSET-PIPELINE.md` | manifest schema + naming.   | QT-07 produces a discovery inventory, not a manifest. Merge stays user-driven at v1.                |

## §11 — Acceptance Checklist

QT-07 is **ratified and shipped** when:

- [x] §5 freezes the recognised reference forms.
- [x] §6 freezes the inventory YAML shape.
- [x] §7 fixes the CLI surface.
- [x] `qt::list_assets(input, out)` lands; CLI subcommand
      `qt list-assets` wired.
- [x] `tests/fixtures/qt/image_refs.qml` exists with the §5
      idiom covering Image and font references.
- [x] `tests/fixtures/qt/image_refs.assets.yaml` is the emitted
      golden; emit + re-emit is byte-identical.
- [x] A drift gate asserts byte equality with the golden.
- [x] Silent-skip-for-non-asset-QML test exercises a fixture
      with no asset references.
- [x] No bumps to existing version constants.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/creator/ASSET-PIPELINE.md`](../creator/ASSET-PIPELINE.md) — asset pipeline policy.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emit + CLI wiring.
- [`tests/fixtures/qt/image_refs.qml`](../../tests/fixtures/qt/image_refs.qml) — canonical fixture.
- [`tests/fixtures/qt/image_refs.assets.yaml`](../../tests/fixtures/qt/image_refs.assets.yaml) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.

## §13 — Unblocks

Ratifying QT-07 unblocks:

- Real-project bring-up: a `qt list-assets` run gives the
  authoring user a clear "what's missing in `manifest.yml`"
  signal.
- A future QT-07 amendment (or a sibling `manifest merge-qt`
  subcommand) can automate the inventory → manifest merge
  round-trip.
- QT-08b (`.qmldir`) and QT-08c (`.qrc`) inherit a stable
  inventory format to extend.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-07 ratified and shipped. New CLI subcommand `qt list-assets <input> [<out>]` (file mode + directory mode per QT-08) walks every `UiItem` in the parsed module, extracts `Image { source: "…" }` (with `qrc:/` and `qrc:///` prefix-stripping) and font references (`Text.font.family`, dotted `<*>.font.family`, standalone `Font { family: … }` blocks), deduplicates lexically, and emits a `<basename>.assets.yaml` with stable `version: 1` + `images: […]` + `fonts: […]` lists. New entry point `qt::list_assets(input, out)`; new walker `walk_asset_refs(item) -> AssetInventory` (pure, recursive); new helper `extract_image_path` strips `qrc:/` / `qrc:///` prefixes from on-disk literals. Inventory uses `BTreeSet<String>` for both buckets so the YAML is deterministic and dedup-stable. Silent skip on QML with no recognised refs (no `.assets.yaml` produced). Empty `images:` / `fonts:` lists are still emitted when the QML has only one of the two. New fixture `tests/fixtures/qt/image_refs.qml` (3 distinct image references in mixed `qrc:` / relative-path form + 2 font families across `Text.font.family` and standalone `Font {}` block) + emitted golden `tests/fixtures/qt/image_refs.assets.yaml` + 2 drift gates (byte-equality + silent-skip-on-non-asset-QML). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-07's artifact is a separate file). Manifest-merge round-trip stays a user concern at v1. State-bound `source:` expressions, `font.weight` / `font.pointSize` derivation, qrc bundle resolution, `.qmldir` external-asset declarations, `AnimatedImage` frame folders, and localised variants remain deferred under future Specification-Required §5 amendments. |
| 2026-06-26 | §5 amendment: **state-bound `source:` harvest** promoted from "deferred". The walker now extracts every quoted image-path literal from a non-literal `source:` expression (e.g. the branches of a `cond ? "a.png" : "b.png"` ternary) via `extract_asset_literals`, keeping literals with a known image extension (`.png/.jpg/.jpeg/.gif/.svg/.bmp/.webp`) or a `qrc:` prefix. `BorderImage` / `AnimatedImage` join `Image` as harvested source types. This closes the gap where designer-authored conditional artwork (repeat/shuffle/source glyphs in the scjson tutorial media player) produced an empty inventory. The QT-07 artifact shape is unchanged (still `version: 1` + `images:` + `fonts:`); no version constant bump. The same literal-picking feeds the QT-03b Image-emit `qt_assets` symbol resolution. |

---

MIT-licensed: MIT.
