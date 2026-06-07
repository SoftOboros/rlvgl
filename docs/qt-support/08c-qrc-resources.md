<!--
08c-qrc-resources.md - QT-08c: .qrc resource manifest parser.
-->

**[← Prev](08b-qmldir-resolution.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-08c — `.qrc` Resource Manifests

QT-08c adds a parser for Qt's `.qrc` (Qt Resource Compiler) manifest
file and a `qt list-qrc` CLI subcommand that emits a stable YAML
inventory of the bundled file paths organised by their qrc prefix.
This is the **third** slice of the QT-08 family (after QT-08's
directory-mode walker and QT-08b's qmldir parser).

`.qrc` files are XML; QT-08c uses a **hand-rolled minimal XML
walker** (no new Cargo dep) that recognises the canonical
`<RCC><qresource prefix="..."><file>...</file></qresource></RCC>`
shape.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
canonical `.qrc` schema is owned by Qt's documentation
(`qtbase/doc/src/development/resources.qdoc`); QT-08c parses the
structurally-stable subset described in §5. Output YAML shape and
CLI surface are owned here.

## §1 — Purpose

`.qrc` files map filesystem paths into Qt's resource bundle, where
they're addressed at runtime via `qrc:/<prefix>/<file>` URIs. A
canonical example:

```xml
<!DOCTYPE RCC>
<RCC version="1.0">
    <qresource prefix="/icons">
        <file>play.png</file>
        <file>stop.png</file>
        <file alias="reset">reset_button_v3.png</file>
    </qresource>
    <qresource prefix="/fonts">
        <file>FiraSans-Bold.ttf</file>
    </qresource>
</RCC>
```

Without QT-08c, `rlvgl-creator` cannot:

- Know which on-disk files a Qt project's `qrc:/icons/play.png`
  reference resolves to.
- Cross-reference `.qrc` entries against the QT-07 asset
  inventory.
- Validate that a QML file's `Image { source: "qrc:/…" }` points
  at a file actually declared in the `.qrc`.

After QT-08c:

```bash
$ rlvgl-creator qt list-qrc resources.qrc
$ # writes resources.qrc.yaml — the parsed manifest.
```

## §2 — Problem Statement

Three concrete gaps:

- **No XML parser.** The QT-01a structural parser only handles
  QML; `.qrc` is XML and needs its own (small) walker.
- **No prefix awareness.** A QT-07 inventory lists
  `icons/play.png` (qrc-prefix-stripped); a `.qrc` lists `play.png`
  under `prefix="/icons"`. Connecting the two requires the
  `.qrc` parser to surface the prefix structure.
- **No alias surfacing.** `.qrc` `<file alias="…">` entries can
  rename a file at lookup time. QT-08c records both the alias
  (the qrc-side name) and the on-disk filename.

## §3 — Canonical Glossary (delta only)

QT-08c introduces no new IR types. One parser, one inventory
struct, one CLI subcommand.

### `parse_qrc(content) -> QrcManifest`

Pure function. Parses the `<RCC>` XML subset described in §5
using a hand-rolled walker. Returns a `QrcManifest` populated
with the recognised `qresource` blocks and their `file` entries.

### `QrcManifest`

```rust
pub struct QrcManifest {
    pub version: Option<String>,           // RCC version="…" attribute
    pub resources: Vec<QrcResource>,
}

pub struct QrcResource {
    pub prefix: Option<String>,            // qresource prefix="…"
    pub lang: Option<String>,              // qresource lang="…"
    pub files: Vec<QrcFile>,
}

pub struct QrcFile {
    pub path: String,                      // on-disk path (text content of <file>)
    pub alias: Option<String>,             // <file alias="…"> rename
}
```

Resources, files, and attribute values appear in source order.

### `qt list-qrc <input> [<out>]`

CLI subcommand. File mode: `<input>` is a `.qrc` file. Writes
`<basename>.qrc.yaml`.

## §4 — Source-of-Truth Map

| Concept                                       | Owner                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| Canonical `.qrc` XML schema                   | Qt's `resources.qdoc` documentation.                                    |
| Recognised XML element / attribute set        | this chapter (§5).                                                      |
| Inventory YAML schema                         | this chapter (§6).                                                      |
| `qt list-qrc` CLI                             | this chapter (§7).                                                      |
| Cross-reference against QT-07 asset inventory | **deferred** — out of scope at v1; user-driven.                         |
| QML `Image { source: "qrc:/…" }` validation   | **deferred** — out of scope at v1.                                     |
| `.qrc` runtime extraction at build time       | **deferred** — out of scope; user runs `rcc` if needed.                |

## §5 — Frozen Decision: Recognised XML Subset

Registration policy: **Specification Required**.

| XML form                                            | Parsed as                                                                       |
| --------------------------------------------------- | ------------------------------------------------------------------------------- |
| `<RCC version="1.0">…</RCC>`                        | `manifest.version = Some("1.0")`                                                 |
| `<RCC>…</RCC>` (no version)                         | `manifest.version = None`                                                        |
| `<qresource prefix="/foo">…</qresource>`            | new `QrcResource { prefix: Some("/foo"), … }`                                    |
| `<qresource lang="en">…</qresource>`                | `lang: Some("en")`                                                               |
| `<qresource prefix="/foo" lang="en">…</qresource>`  | both attributes captured                                                         |
| `<file>play.png</file>`                             | `QrcFile { path: "play.png", alias: None }`                                      |
| `<file alias="reset">reset_v3.png</file>`           | `QrcFile { path: "reset_v3.png", alias: Some("reset") }`                         |
| `<!-- comment -->`                                  | dropped                                                                          |
| `<!DOCTYPE …>`                                       | dropped                                                                          |
| `<?xml … ?>`                                        | dropped                                                                          |
| Whitespace between elements                         | dropped                                                                          |
| Any other element                                   | parsing error — emit-time failure                                                |
| Element attributes other than the recognised set    | silently ignored                                                                 |

The parser is intentionally strict on element names: at v1, an
unrecognised element under `<RCC>` is an error, not a passthrough,
because `.qrc` is a tightly-defined schema and unrecognised
elements usually indicate corruption.

Deferred at v1:

- `<file compress="…">` / `<file threshold="…">` compression
  attributes (recorded as ignored).
- Nested CDATA in `<file>` content.
- XML namespaces (the canonical `.qrc` is namespace-free).

## §6 — Frozen Decision: Inventory YAML Shape

```yaml
# QT-08c qrc: <source path>
version: 1
rcc_version: "1.0"
resources:
  - prefix: "/icons"
    lang: null
    files:
      - { path: play.png, alias: null }
      - { path: stop.png, alias: null }
      - { path: reset_button_v3.png, alias: reset }
  - prefix: "/fonts"
    lang: null
    files:
      - { path: FiraSans-Bold.ttf, alias: null }
```

| Field         | Required | Notes                                                     |
| ------------- | -------- | --------------------------------------------------------- |
| `version`     | yes      | Inventory schema version. Always `1` at v1.                |
| `rcc_version` | yes      | The `<RCC version="…">` attribute. `null` if not declared.|
| `resources`   | yes      | Empty list if none.                                        |

Order preserved (XML is order-sensitive, `.qrc` consumers depend
on declaration order for prefix collisions).

## §7 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt list-qrc <INPUT> [<OUT>]
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/foo.qrc` | `dir/`             | (parent of input) | Writes `<OUT>/foo.qrc.yaml`. |
| File | `path/foo.qrc` | `dir/x.yaml`        | n/a               | Writes the named file. |
| Dir  | `path/`         | `dir/`            | (input itself)    | For every `*.qrc` in the dir, writes `<OUT>/<basename>.qrc.yaml`. |

Missing input file: hard error (non-silent).
Malformed XML: hard error with line/column attribution.

## §8 — Versioning

| Constant                       | Before QT-08c | After QT-08c |
| ------------------------------ | ------------- | ------------ |
| All existing emit-shape consts | unchanged     | unchanged    |
| Inventory YAML `version:`      | (new)         | 1            |

QT-08c's artifact is a separate `.qrc.yaml` file. No bumps to
versioned emit-shapes.

## §9 — Non-Goals

- **No cross-validation** with QT-07 asset inventories or QML
  `Image { source: "qrc:…" }` references at v1. A future amendment
  can correlate.
- **No file-existence check** — paths are recorded as declared,
  even if the on-disk file is missing.
- **No actual `rcc` invocation** — QT-08c is parse-only; we don't
  run Qt's resource compiler.
- **No XML round-trip.** YAML inventory only; we don't write back
  to `.qrc`.
- **No compression metadata.** `compress`/`threshold` attributes
  silently ignored at v1.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                       | Resolution                                                                                                           |
| -------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| QT-07    | Asset inventory.                              | Independent. A future amendment may cross-reference qrc-declared files against the asset inventory.                  |
| QT-08    | Directory-mode dispatch.                      | QT-08c is a sibling slice of QT-08 / QT-08b. Each parses a different file type; YAML inventory shape is the convention. |
| QT-08b   | qmldir parser.                                | Independent. A directory may have both qmldir and `.qrc`; running both subcommands produces both inventories.        |

## §11 — Acceptance Checklist

QT-08c is **ratified and shipped** when:

- [x] §5 freezes the recognised XML subset.
- [x] §6 freezes the inventory YAML shape.
- [x] §7 fixes the CLI surface.
- [x] `qt::parse_qrc(content) -> Result<QrcManifest>` and
      `qt::list_qrc(input, out)` land; CLI subcommand wired.
- [x] `tests/fixtures/qt/resources.qrc` exists with at least
      two `<qresource>` blocks, multiple files, and a `<file alias>`.
- [x] `tests/fixtures/qt/resources.qrc.yaml` is the emitted
      golden; emit + re-emit is byte-identical.
- [x] Drift gate asserts byte equality with the golden.
- [x] Missing-input + malformed-XML error cases asserted.
- [x] No bumps to existing version constants.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/08-multi-file-cli.md`](./08-multi-file-cli.md) — directory walker.
- [`docs/qt-support/08b-qmldir-resolution.md`](./08b-qmldir-resolution.md) — sibling slice.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — parser + emit + CLI wiring.
- [`tests/fixtures/qt/resources.qrc`](../../tests/fixtures/qt/resources.qrc) — canonical fixture.
- [`tests/fixtures/qt/resources.qrc.yaml`](../../tests/fixtures/qt/resources.qrc.yaml) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.

## §13 — Unblocks

Ratifying QT-08c unblocks:

- Real-project bring-up where `.qrc` is the only authoritative
  source of "what files this Qt project ships".
- A future cross-validation amendment that reconciles QT-07
  asset inventory ↔ QT-08c qrc inventory ↔ QT-01a `Image source:`
  references.
- A path toward QT-09 / QT-10 where the desktop-UI integration
  needs to display the project's resource manifest.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-08c ratified and shipped. New CLI subcommand `qt list-qrc <input> [<out>]` parses the canonical `.qrc` XML subset (`<RCC><qresource prefix="…" lang="…"><file alias="…">…</file></qresource></RCC>` plus comments / DOCTYPE / XML decl) via a hand-rolled minimal walker — **no new Cargo deps**. New types `QrcManifest` / `QrcResource` / `QrcFile`. Output `<basename>.qrc.yaml` preserves declaration order. Unrecognised element under `<RCC>` is a hard error (not a passthrough), matching the strictness of `.qrc`'s schema. Missing input file and malformed XML are non-silent errors. New fixture `tests/fixtures/qt/resources.qrc` (2 `<qresource>` blocks, 4 `<file>` entries with one `alias`, RCC version 1.0) + emitted golden `tests/fixtures/qt/resources.qrc.yaml` + 2 drift gates (byte-equality + missing-input hard error). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-08c's artifact is a separate file). Cross-validation between qrc declarations and QT-07 asset inventory + QT-01a `Image { source: "qrc:…" }` references, file-existence checks, `<file compress=…>` compression metadata, XML namespaces, and CDATA file content all remain deferred under future Specification-Required §5 amendments. |

---

MIT-licensed: MIT.
