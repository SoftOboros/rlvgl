<!--
08b-qmldir-resolution.md - QT-08b: qmldir manifest parser.
-->

**[← Prev](08-multi-file-cli.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-08b — `qmldir` Resolution

QT-08b adds a parser for the Qt module-system's `qmldir` manifest
file and a `qt list-qmldir` CLI subcommand that emits a stable YAML
inventory of the manifest's directives. This is the **second**
slice of the QT-08 family (after QT-08's directory-mode walker);
`.qrc` resource manifests are QT-08c's job and remain deferred.

QT-08b introduces no new IR types. It introduces a new public
parser, a new CLI subcommand, and a fixture pinning the output.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
canonical `qmldir` syntax is owned by Qt's documentation
(`qtdeclarative`); QT-08b parses a structurally-stable subset and
records the rest as opaque `other` directives. The output YAML
shape and the CLI surface are owned here.

## §1 — Purpose

`qmldir` files declare which `.qml` files in a directory form a
named QML module. A canonical example:

```text
module MyModule
MyButton 1.0 MyButton.qml
MyLabel 1.0 MyLabel.qml
singleton Theme 1.0 Theme.qml
internal _Helper _Helper.qml
import QtQuick 2.15
depends QtQuick.Controls 2.15
```

Without QT-08b, `rlvgl-creator` cannot:

- Tell which `.qml` in a project directory are exposed as types.
- Resolve a future `import "MyModule"` against a directory.
- Surface QML singletons (theme objects, controllers) for special
  handling.

After QT-08b:

```bash
$ rlvgl-creator qt list-qmldir screens/
$ # writes screens/qmldir.yaml — the parsed manifest.
```

The `qmldir.yaml` is a discovery artifact, not a binding contract:
it tells QT-08c (`.qrc`) and future amendments how to look up
QML modules without re-implementing the parser per phase.

## §2 — Problem Statement

Three concrete gaps:

- **No parse.** The `qmldir` line-grammar isn't recognised by the
  QT-01a structural parser, which only handles `.qml` content.
- **No inventory.** Unlike QT-06 / QT-07 inventories, qmldir
  declarations are line-oriented, not nested. A YAML inventory
  preserves the structure for downstream review.
- **No singleton awareness.** Some QML projects rely on a
  `singleton Theme 1.0 Theme.qml` declaration to make `Theme.qml`
  the canonical theme source for QT-06; today, QT-06's walker
  ignores that designation and would treat any `QtObject` root
  as theme. A future QT-06 amendment can use the `qmldir`
  inventory to disambiguate.

## §3 — Canonical Glossary (delta only)

QT-08b introduces no new IR types. One parser, one inventory
struct, one CLI subcommand.

### `parse_qmldir(content) -> QmldirManifest`

Pure function. Tokenises `content` line-by-line per Qt's qmldir
grammar (whitespace-separated tokens, `#`-prefix comments,
blank-line-tolerant). Returns a `QmldirManifest` struct populated
with the recognised directive sets.

### `QmldirManifest`

```rust
pub struct QmldirManifest {
    pub module: Option<String>,
    pub types: Vec<QmldirType>,         // ordinary types
    pub singletons: Vec<QmldirType>,    // marked `singleton`
    pub internals: Vec<QmldirType>,     // marked `internal`
    pub imports: Vec<QmldirImport>,
    pub depends: Vec<QmldirImport>,
    pub plugins: Vec<QmldirPlugin>,
    pub other: Vec<String>,             // unrecognised lines, raw
}
```

`QmldirType` carries `name`, `version`, `file`. `QmldirImport`
carries `module`, optional `version`. `QmldirPlugin` carries
`name`, optional `path`.

### `qt list-qmldir <input> [<out>]`

CLI subcommand. File mode: `<input>` is a `qmldir` file or a
directory containing one. Writes `<basename_or_dirname>.qmldir.yaml`.
Directory walk resolves to `<dir>/qmldir`.

## §4 — Source-of-Truth Map

| Concept                                     | Owner                                                                  |
| ------------------------------------------- | ---------------------------------------------------------------------- |
| Canonical `qmldir` line grammar             | Qt's `qtdeclarative` documentation.                                     |
| Parser line tokenisation                    | this chapter (§5).                                                      |
| Recognised directive set                    | this chapter (§5 / §6).                                                 |
| Inventory YAML schema                       | this chapter (§6).                                                      |
| `qt list-qmldir` CLI                        | this chapter (§7).                                                      |
| `import "Module"` cross-resolution at QT-01a | **deferred** — out of scope at v1.                                     |
| `.qrc` resource manifests                   | **QT-08c** (deferred).                                                  |
| QML singleton special-casing in QT-06       | **deferred** — future QT-06 amendment can read this inventory.         |

## §5 — Frozen Decision: Recognised Directives

Registration policy: **Specification Required**.

| Line form                                          | Parsed as                                                                     |
| -------------------------------------------------- | ----------------------------------------------------------------------------- |
| `module <name>`                                    | `manifest.module = Some(name)`                                                 |
| `<TypeName> <version> <file>.qml`                  | `manifest.types.push({ name, version, file })`                                |
| `singleton <TypeName> <version> <file>.qml`        | `manifest.singletons.push({ name, version, file })`                           |
| `internal <TypeName> <file>.qml`                   | `manifest.internals.push({ name, version: None, file })`                      |
| `import <ModuleName>` or `import <ModuleName> <version>` | `manifest.imports.push({ module, version })`                            |
| `depends <ModuleName>` or `depends <ModuleName> <version>` | `manifest.depends.push({ module, version })`                          |
| `plugin <name>` or `plugin <name> <path>`          | `manifest.plugins.push({ name, path })`                                       |
| `# <comment>`                                      | dropped silently                                                              |
| blank line                                         | dropped silently                                                              |
| anything else                                      | recorded verbatim in `manifest.other`                                          |

Multiple `module` lines: the **last one wins** (matches Qt's
own behaviour) and a debug-mode warning may be issued by a
future amendment.

Deferred at v1:

- `typeinfo <file>.qmltypes` (the qmltypes registration line).
- `classname <Name>` (C++ plugin classname binding).
- `prefer <path>` (alternate module location).
- `optional` modifier on imports.
- `designersupported` flag.

These remain in `other` until promoted to a §5 amendment.

## §6 — Frozen Decision: Inventory YAML Shape

```yaml
# QT-08b qmldir: <source path>
version: 1
module: MyModule
types:
  - { name: MyButton, version: "1.0", file: MyButton.qml }
  - { name: MyLabel,  version: "1.0", file: MyLabel.qml }
singletons:
  - { name: Theme, version: "1.0", file: Theme.qml }
internals:
  - { name: _Helper, version: null, file: _Helper.qml }
imports:
  - { module: QtQuick, version: "2.15" }
depends:
  - { module: QtQuick.Controls, version: "2.15" }
plugins: []
other: []
```

| Field        | Required | Notes                                                                           |
| ------------ | -------- | ------------------------------------------------------------------------------- |
| `version`    | yes      | Always `1`.                                                                     |
| `module`     | yes      | `null` if not declared.                                                          |
| `types`      | yes      | Empty list if none.                                                              |
| `singletons` | yes      | Empty list if none.                                                              |
| `internals`  | yes      | Empty list if none.                                                              |
| `imports`    | yes      | Empty list if none.                                                              |
| `depends`    | yes      | Empty list if none.                                                              |
| `plugins`    | yes      | Empty list if none.                                                              |
| `other`      | yes      | Empty list if none. Each entry is the raw unrecognised line (whitespace-trimmed). |

Lists preserve the order they appeared in the source `qmldir`.
This is **not** lexical — `qmldir` is order-sensitive at parse
time (later lines can override earlier ones), so preserving
declaration order keeps round-trip review correct.

## §7 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt list-qmldir <INPUT> [<OUT>]
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/qmldir`     | `dir/`             | (parent of input) | Writes `<OUT>/<dirname-of-input>.qmldir.yaml`. |
| File | `path/qmldir`     | `dir/x.yaml`        | n/a               | Writes the named file.                       |
| Dir  | `path/screens/`   | `dir/`            | (input itself)    | Reads `path/screens/qmldir`; writes `<OUT>/<dirname>.qmldir.yaml`. |
| Dir  | `path/screens/`   | (none)            | (parent of input) | Same; output filename is `<dirname>.qmldir.yaml`. |

Missing `qmldir` file: the command exits non-zero with the
expected path printed. (No silent skip — at QT-08b the user
explicitly invoked the subcommand, so a missing input is an
error.)

## §8 — Versioning

| Constant                       | Before QT-08b | After QT-08b |
| ------------------------------ | ------------- | ------------ |
| All existing emit-shape consts | unchanged     | unchanged    |
| Inventory YAML `version:`      | (new)         | 1            |

QT-08b's artifact is a separate `.qmldir.yaml` file. No bumps to
the versioned emit-shapes.

## §9 — Non-Goals

- **No cross-import resolution.** `import "MyModule"` in a
  `.qml` does not resolve through this manifest at QT-08b. A
  future QT-01a amendment may do so.
- **No qmltypes / classname parsing.** Recorded in `other`.
- **No bundle expansion.** A `qmldir` referencing files in
  subdirectories doesn't trigger a recursive walk.
- **No write-back.** QT-08b is one-way: qmldir → YAML.
- **No version negotiation.** All versions are recorded
  verbatim; QT-08b doesn't pick "the best" when multiple
  registrations match.
- **No singleton hookup.** QT-06 doesn't read this inventory at
  v1; that wiring is a future QT-06 amendment.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                       | Resolution                                                                                                                |
| -------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| QT-01a   | QML structural parse.                         | Independent. QT-08b parses the `qmldir` line grammar; the QML body parser is untouched.                                  |
| QT-08    | Directory-mode dispatch.                      | QT-08b is a sibling slice. The QT-08 directory walker doesn't read `qmldir`; QT-08b is invoked by name.                  |
| QT-08c   | `.qrc` resource manifests.                    | QT-08c can consume QT-08b's inventory (same directory walk, different artifact) when it lands.                            |
| QT-06    | Theme tokens.                                 | A future amendment can use `singletons` to identify the canonical theme source instead of probing every QML root.        |
| QT-07    | Asset inventory.                              | Independent; the two artifacts coexist.                                                                                   |

## §11 — Acceptance Checklist

QT-08b is **ratified and shipped** when:

- [x] §5 freezes the recognised directive set.
- [x] §6 freezes the inventory YAML shape.
- [x] §7 fixes the CLI surface.
- [x] `qt::parse_qmldir(content) -> QmldirManifest` and
      `qt::list_qmldir(input, out)` land; CLI subcommand wired.
- [x] `tests/fixtures/qt/sample_module/qmldir` exists with the
      §5 directive set.
- [x] `tests/fixtures/qt/sample_module.qmldir.yaml` is the
      emitted golden; emit + re-emit is byte-identical.
- [x] A drift gate asserts byte equality with the golden.
- [x] Missing-qmldir error case asserted (non-zero exit).
- [x] No bumps to existing version constants.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/08-multi-file-cli.md`](./08-multi-file-cli.md) — directory-mode walker (related slice).
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — parser + emit + CLI wiring.
- [`tests/fixtures/qt/sample_module/qmldir`](../../tests/fixtures/qt/sample_module/qmldir) — canonical fixture.
- [`tests/fixtures/qt/sample_module.qmldir.yaml`](../../tests/fixtures/qt/sample_module.qmldir.yaml) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.

## §13 — Unblocks

Ratifying QT-08b unblocks:

- **QT-08c** (`.qrc` resource manifests) — same author can land
  QRC parsing using the same emit pattern.
- A future QT-06 amendment that prefers a `qmldir`-declared
  singleton as the canonical theme source.
- A future QT-01a amendment that resolves `import "MyModule"`
  against a registered qmldir.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-08b ratified and shipped. New CLI subcommand `qt list-qmldir <input> [<out>]` (file mode + directory mode) parses the Qt `qmldir` line grammar and emits a stable `<basename>.qmldir.yaml` inventory. New types `QmldirManifest` / `QmldirType` / `QmldirImport` / `QmldirPlugin`. New parser `parse_qmldir(content) -> QmldirManifest` recognises `module`, ordinary type registrations (`<Name> <version> <file>.qml`), `singleton` and `internal` modifiers, `import` / `depends`, and `plugin` directives; comments (`#` prefix) and blank lines silently dropped; unrecognised non-empty lines preserved verbatim in `manifest.other`. Multiple `module` lines: last-one-wins. Output YAML preserves declaration order (qmldir is order-sensitive). Missing input file is a hard error (non-silent). New fixture `tests/fixtures/qt/sample_module/qmldir` (1 module + 2 types + 1 singleton + 1 internal + 2 imports + 1 depends + 1 unrecognised "typeinfo" directive captured in `other`) + emitted golden `tests/fixtures/qt/sample_module.qmldir.yaml` + 2 drift gates (byte-equality + missing-file error). No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` / `ISTATE_LINKAGE_VERSION` (QT-08b's artifact is a separate file). `import "Module"` cross-resolution at QT-01a, qmltypes parsing, classname / prefer / optional / designersupported directives, recursive bundle expansion, and singleton-driven theme auto-discovery for QT-06 remain deferred under future Specification-Required §5 amendments. |

---

MIT-licensed: MIT.
