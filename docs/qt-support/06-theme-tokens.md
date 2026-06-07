<!--
06-theme-tokens.md - QT-06: Qt theme → tokens.yaml.
-->

**[← Prev](05e-externals-stubs.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-06 — Theme Tokens

QT-06 closes the Qt-side of `tokens.yaml` authoring. A Qt project's
theme — colors, spacing, radii, font sizes — gets parsed from a
canonical `Theme.qml` (`QtObject` root carrying `property` declarations)
and lowered to the same `tokens.yaml` shape that the Chakra TS and
Svelte token pipelines already produce. After QT-06, `tokens.yaml`
has three first-class authoring sources (Chakra, Svelte, Qt) and one
canonical schema.

QT-06 introduces no new IR types and no new emit-shape constants.
It introduces a new walker, a new CLI subcommand, a precedence rule
for resolving multi-source tokens, and a fixture pinning the
output.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
on-disk `tokens.yaml` schema is owned by the existing Chakra /
Svelte pipeline (see
[`src/bin/creator/chakra.rs`](../../src/bin/creator/chakra.rs) and
[`src/bin/creator/svelte.rs`](../../src/bin/creator/svelte.rs)) —
QT-06 produces the **same** schema, not a parallel one. The QML
authoring convention, name-to-category mapping rules, precedence
ordering, and `qt emit-tokens` CLI are owned here.

## §1 — Purpose

Today, a Qt project bringing up an rlvgl screen has two
unwanted-fork failure modes for theme:

- The author writes `property color primary: "#3366ff"` in QML
  *and* a `tokens.yaml` by hand. The two drift silently when one
  changes and the other does not.
- The author skips the QML side and authors theme only in
  `tokens.yaml`. QML widgets then hard-code colors literal-string,
  which the rlvgl emit pipeline cannot reconcile against
  `tokens.yaml`.

After QT-06:

```bash
$ rlvgl-creator qt emit-tokens Theme.qml
$ # writes Theme.tokens.yaml (single source of truth for both
$ # the Qt screens and the rlvgl-creator chakra/svelte pipeline).
```

The QML stays canonical for the Qt side; the generated `tokens.yaml`
flows into rlvgl's existing pipeline without drift.

## §2 — Problem Statement

The roadmap's §7 reconciliation table promised: "Qt theme MUST
funnel through the same `tokens.yaml` to avoid forking precedence
rules." Three concrete issues:

- **No parser.** The QT-01a structural parser captures
  `property color primary: "#3366ff"` as a `UiProperty` with a
  literal `default_value`, but no chapter promotes it to
  tokens.
- **No category convention.** A `property color brand_500:
  "#..."` and a `property int spacing_md: 8` need to land in
  different sections of `tokens.yaml`. Without a frozen rule,
  reviewers cannot predict the output.
- **No precedence story.** A user could in principle have a Qt
  Theme.qml *and* a Chakra `theme.ts` and a Svelte `tokens.yaml`
  all in one project. QT-06 freezes the merge order so silent
  overrides are impossible.

## §3 — Canonical Glossary (delta only)

QT-06 introduces no new IR types. One new walker, one CLI
subcommand, one comment marker.

### `walk_theme_module(module) -> Option<TokenSet>`

Pure function. Given a `UiModule` whose root is a `QtObject` (or
any container with property declarations — the type name is not
checked), returns `Some(token_set)` carrying the categorised
tokens, or `None` if no recognised theme properties were found.

`TokenSet` is a private intermediate (not part of any IR); it
serialises to `tokens.yaml` via `render_tokens_yaml`.

### `qt emit-tokens <input> [<out>]`

CLI subcommand. File mode: writes `<input_dir>/<basename>.tokens.yaml`
or to the explicit `<out>` path. Directory mode: walks every
`*.qml` per QT-08 and emits one `<basename>.tokens.yaml` per QML
that has any recognised theme properties. Silent skip for QML
files with no theme properties.

### `// QT-06 theme:` marker

Emitted as the first content line of the produced `tokens.yaml`
(after the `version: 1` line) as a YAML comment naming the source
QML path. Provenance for reviewers — same pattern as QT-05d's
`_comment` field.

## §4 — Source-of-Truth Map

| Concept                                       | Owner                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| `tokens.yaml` schema                          | Chakra / Svelte pipeline (existing).                                   |
| Required keys (`version: 1`, `colors:`, `spacing:`, `radii:`, `fonts:`) | Chakra / Svelte pipeline.                                              |
| Optional `modes: dark: colors:`               | Chakra / Svelte pipeline (existing).                                   |
| QML `property` parse (UiProperty)             | QT-01a.                                                                 |
| QML theme authoring convention                | this chapter (§5).                                                      |
| Name-to-category mapping                      | this chapter (§6).                                                      |
| `walk_theme_module` algorithm                 | this chapter (§3 / §6).                                                 |
| `qt emit-tokens` CLI                          | this chapter (§7).                                                      |
| Multi-source precedence rules                 | this chapter (§8).                                                      |
| Dark-mode authoring (`property color name_dark: …`) | this chapter (§5).                                                |

## §5 — Frozen Decision: Authoring Convention

Registration policy: **Specification Required**.

A canonical `Theme.qml` (the basename is by convention; any
`*.qml` works) declares a `QtObject` root containing `property`
declarations. **The QtObject type-name is not enforced** —
QT-06's walker accepts any root. Authoring shape:

```qml
import QtQuick 2.15

QtObject {
    // Color tokens (any property whose declared type is `color`).
    // Hex literals only at v1; rgba()/hsl()/named-color forms
    // are deferred.
    property color primary:    "#3366ff"
    property color background: "#ffffff"
    property color text:       "#111111"
    property color accent:     "#ff8800"

    // Spacing tokens (`property int spacing_<size>: <int-literal>`).
    // The suffix becomes the YAML key. Recommended set:
    // xs / sm / md / lg / xl, but any name is accepted.
    property int spacing_xs: 2
    property int spacing_sm: 4
    property int spacing_md: 8
    property int spacing_lg: 16
    property int spacing_xl: 24

    // Radius tokens (`property int radius_<size>: <int-literal>`).
    property int radius_none: 0
    property int radius_sm:   2
    property int radius_md:   4
    property int radius_lg:   8
    property int radius_full: 255

    // Font tokens (`property string font_<size>: "<name>"`).
    property string font_small:   "tiny"
    property string font_body:    "default"
    property string font_heading: "bold"

    // Optional dark-mode overrides — name suffix `_dark`.
    // Only `color`-typed properties are recognised; integers /
    // strings with `_dark` suffix are ignored at v1.
    property color background_dark: "#171923"
    property color text_dark:       "#f7fafc"
}
```

Recognised forms (per QT-06 §6):

| QML                                                | tokens.yaml location           |
| -------------------------------------------------- | ------------------------------ |
| `property color <name>: "<hex>"`                   | `colors.<name>`                |
| `property color <name>_dark: "<hex>"`              | `modes.dark.colors.<name>`     |
| `property int spacing_<key>: <int>`                | `spacing.<key>`                |
| `property int radius_<key>: <int>`                 | `radii.<key>`                  |
| `property string font_<key>: "<text>"`             | `fonts.<key>`                  |
| Anything else (other types, non-literal defaults)  | Silently dropped. `// QT-06 theme:` comment doesn't list them. |

Deferred at v1:

- `Material.accent` / `Universal.accent` / etc. (Qt Quick
  Controls 2 style-system bindings).
- `palette { … }` block parsing.
- rgba/hsl/named-color parsing.
- Per-state overrides beyond `_dark`.
- Animation / transition tokens.
- Numeric font sizes (we only emit named font keys for v1).

A future amendment may promote any of them.

## §6 — Frozen Decision: Walk Algorithm

For a `UiModule` (or any `UiItem`):

1. **Iterate root properties** (`item.properties`).
2. For each `UiProperty { name, ty, default_value: Some(lit), … }`:
   - If `ty == "color"` and `default_value` parses as a hex
     literal:
     - If `name` ends in `_dark`: push to `dark_colors`
       (key = `name` without the `_dark` suffix).
     - Else: push to `colors` (key = `name`).
   - Else if `ty == "int"` and `default_value` parses as `i64`:
     - If `name` starts with `spacing_`: push to `spacing`
       (key = `name[8..]`).
     - Else if `name` starts with `radius_`: push to `radii`
       (key = `name[7..]`).
     - Else: silently dropped.
   - Else if `ty == "string"` and `default_value` parses as a
     string literal:
     - If `name` starts with `font_`: push to `fonts`
       (key = `name[5..]`).
     - Else: silently dropped.
   - Otherwise: silently dropped.
3. **Determinism**: walks emit each section in **lexical order
   by key**. This is independent of the QML declaration order —
   the canonical fixture's QML happens to be lexical too, but the
   walker doesn't rely on that.
4. Return `None` if all of `colors`, `spacing`, `radii`,
   `fonts`, and `dark_colors` are empty.

Hex literal acceptance: `^#[0-9a-fA-F]{3,8}$` — accepts `#rgb`,
`#rrggbb`, and `#rrggbbaa`. Other forms (`rgba()`, named colors)
are rejected and the property silently dropped.

## §7 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt emit-tokens <INPUT> [<OUT>]

ARGS:
    <INPUT>    Path to a `.qml` file or a directory containing `.qml` files.
    <OUT>      Output path (file or directory). Defaults to the input directory.
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/Theme.qml` | `dir/`             | (parent of input) | Writes `<OUT>/Theme.tokens.yaml`. |
| File | `path/Theme.qml` | `dir/foo.yaml`     | n/a               | Writes the named file. |
| Dir  | `path/themes/`   | `dir/`            | (input itself)    | For every `*.qml` with theme properties, writes `<OUT>/<basename>.tokens.yaml`. |

Output filename convention: `<basename>.tokens.yaml`. The
`tokens.yaml` extension preserves compatibility with the existing
`svelte tokens` consumers; the `<basename>.` prefix prevents
collision when multiple QML files in one directory each produce
their own.

A `.qml` with no recognised theme properties silently skips.

## §8 — Frozen Decision: Multi-Source Precedence

When a project has more than one token source, **the same
`tokens.yaml` filename is the merge point** — there is no
in-creator merge step at QT-06. The user runs:

1. `rlvgl-creator qt emit-tokens Theme.qml` → `Theme.tokens.yaml`.
2. `rlvgl-creator chakra ingest theme.ts` → `tokens.yaml`.
3. `rlvgl-creator svelte align ...` → consumes any of the above.

The user picks **one** source-of-truth filename and points
downstream commands at it. Precedence between sources is a
**user concern**, not a creator concern, until QT-06 v2 grows a
merge subcommand.

Recommended pattern:

| Project type                | Recommended source | Rationale |
| --------------------------- | ------------------ | --------- |
| Pure Qt project             | `Theme.qml`        | Single canonical authoring file. |
| Pure web/Svelte project     | `tokens.yaml`      | Already canonical; QT-06 doesn't apply. |
| Mixed (Qt + Chakra)         | `Theme.qml` is canonical; chakra `theme.ts` is regenerated from it (manually for now). | Avoids the two-source drift. |
| Migration in progress       | Whichever is more complete; the other catches up. | Pragmatic. |

QT-06 v2 (deferred) MAY add `qt merge-tokens <a> <b> [<c>] ...`
that overlays multiple sources with explicit precedence flags.
For v1 the user picks one.

## §9 — Non-Goals

- **No theme application at runtime.** Tokens are
  build-time-only. The rlvgl widget tree consumes them via the
  existing Chakra/Svelte → emit pipeline.
- **No named-color / rgba parsing.** Hex literals only at v1.
- **No automatic dark-mode detection.** The `_dark` suffix is
  the explicit opt-in; no inference from "darker hex value" or
  similar.
- **No per-widget theme overrides.** A property declared on a
  child `Item` is silently ignored — only the root-level
  properties contribute. Per-widget theme is a Chakra-side
  concern (semantic tokens) that QT-06 doesn't replicate.
- **No `Material.accent` / `Universal.accent` parsing.**
  Deferred to a §5 amendment.
- **No multi-file theme composition.** A `Theme.qml` that
  imports other QML files for partial themes is not supported
  at v1; the walker only reads the immediate root.
- **No write-back to QML.** QT-06 is one-way: QML → YAML.
  Editing the YAML doesn't propagate to QML.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                    | Resolution                                                                                                                |
| -------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| QT-01a   | Structural parse (`UiProperty` w/ `default_value`). | QT-06 reads existing fields; no parser changes.                                                                         |
| QT-04b   | `ScreenState` field projection.            | Independent. Theme tokens are build-time; ScreenState is runtime mutable.                                                |
| QT-04e   | Reactive Label-text bindings.              | Independent. A future amendment may add `text: theme.<token>` lowering, but at v1 theme tokens are not reactive.        |
| QT-05    | State machines.                            | Independent.                                                                                                              |
| QT-08    | Directory-mode CLI.                        | QT-06 reuses `qt08_collect_qml_files`. Directory mode emits per-`<basename>.tokens.yaml`.                                |
| Chakra TS / Svelte token | `tokens.yaml` schema.        | QT-06 produces the **same** schema. No precedence merge in-creator at v1; user-driven per §8.                            |

## §11 — Acceptance Checklist

QT-06 is **ratified and shipped** when:

- [x] §5 freezes the QML authoring convention.
- [x] §6 fixes the walk algorithm (deterministic, lexical
      key order).
- [x] §7 fixes the CLI surface.
- [x] §8 fixes the multi-source precedence rule (user-driven
      at v1).
- [x] `qt::emit_tokens(input, out)` lands; CLI subcommand
      `qt emit-tokens` wired.
- [x] `tests/fixtures/qt/Theme.qml` exists with the §5 idiom
      covering colors, spacing, radii, fonts, and dark-mode
      colors.
- [x] `tests/fixtures/qt/Theme.tokens.yaml` is the emitted
      golden; emit + re-emit is byte-identical.
- [x] A drift gate asserts byte equality with the golden.
- [x] No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` /
      `QT_EMIT_VERSION_DATA` (QT-06's artifact is a separate
      file).
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`src/bin/creator/chakra.rs`](../../src/bin/creator/chakra.rs) — `tokens.yaml` schema source-of-truth.
- [`src/bin/creator/svelte.rs`](../../src/bin/creator/svelte.rs) — same.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emit + CLI wiring.
- [`tests/fixtures/svelte/tokens-valid.yaml`](../../tests/fixtures/svelte/tokens-valid.yaml) — schema reference.
- [`tests/fixtures/qt/Theme.qml`](../../tests/fixtures/qt/Theme.qml) — canonical fixture.
- [`tests/fixtures/qt/Theme.tokens.yaml`](../../tests/fixtures/qt/Theme.tokens.yaml) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.

## §13 — Unblocks

Ratifying QT-06 unblocks:

- **QT-07** (asset-crate handoff): with theme tokens flowing
  through `tokens.yaml`, asset-crate scaffolding can pick up
  Qt-authored colors via the existing chakra/svelte pipeline.
- Real-project bring-up where authors prefer single-file QML
  theming during prototyping.
- Future QT-06 amendments — Material/Universal palette parsing,
  rgba/hsl support, multi-file theme composition, automated
  precedence merge — slot in as §5 / §6 / §8 amendments without
  reshaping the file format.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-30 | QT-06 ratified and shipped. New CLI subcommand `qt emit-tokens <input> [<out>]` (file mode + directory mode per QT-08) walks root-level `property color/int/string` declarations on a QML file's root item and emits a `<basename>.tokens.yaml` matching the existing chakra/svelte schema (`version: 1` + `colors:` + `spacing:` + `radii:` + `fonts:` + optional `modes.dark.colors:`). New entry point `qt::emit_tokens(input, out)` + `qt::render_tokens_yaml(theme, qml_source)` + `walk_theme_module(item) -> Option<TokenSet>` walker. Name-to-category rules per §6: `color` → `colors.<name>`, `color` with `_dark` suffix → `modes.dark.colors.<name>`, `int spacing_<key>` → `spacing.<key>`, `int radius_<key>` → `radii.<key>`, `string font_<key>` → `fonts.<key>`. Hex-literal regex `^#[0-9a-fA-F]{3,8}$`; non-conforming color values silently dropped. Lexical key ordering for byte-stable output. Output silently skips QML files with no recognised theme properties. New fixture `tests/fixtures/qt/Theme.qml` (4 colors + 5 spacing + 5 radii + 3 fonts + 2 dark-mode colors) + emitted golden `tests/fixtures/qt/Theme.tokens.yaml` + 1 byte-equality drift gate. No bumps to `QT_IR_VERSION` / `QT_EMIT_VERSION_RLVGL` / `QT_EMIT_VERSION_DATA` (QT-06's artifact is a separate file). Multi-source precedence resolution remains a user concern at v1 per §8 (user picks one canonical filename); a `qt merge-tokens` subcommand for explicit-precedence overlays is reserved as a future v2 amendment. Material/Universal style-system extraction, `palette {}` block parsing, rgba/hsl/named-color parsing, and per-widget overrides remain deferred under future Specification-Required §5 amendments. |

---

MIT-licensed: MIT.
