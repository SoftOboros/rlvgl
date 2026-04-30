<!-- QT-INGEST.md - Qt/QML ingestion for rlvgl-creator. -->
<p align="center">
  <img src="../../rlvgl-logo.png" alt="rlvgl" />
</p>

# Qt / QML Ingestion

Status: **MVP shipped (phase QT-01a)** — see [`docs/qt-support/`](../qt-support/)
for the full multi-phase plan and conformance targets.

This page is the practical setup guide. It covers:

1. Which external tools (Homebrew, pip, etc.) the current Qt
   ingestion path requires (**none** in the MVP).
2. What the MVP can and cannot extract from a `.qml` file.
3. The opt-in install steps for the richer ingestion tiers planned
   in later phases.

> Cargo dependencies are deliberately omitted here — they live in
> [`Cargo.toml`](../../Cargo.toml) under the `creator` feature and are
> considered self-documenting. This document tracks **only** the
> non-Cargo dependencies (Homebrew formulae, pip packages, system
> binaries) that the creator product touches when you run `qt ingest`.

---

## TL;DR

The MVP ingestion is **pure Rust**. No Homebrew install, no
`pip install`, no Qt runtime, no PySide6. The parser is a small
recursive-descent QML parser inside the `rlvgl-creator` binary —
running `cargo build --features creator` gives you everything.

Subcommand surface:

| Command                              | What it does                                                       |
| ------------------------------------ | ------------------------------------------------------------------ |
| `rlvgl-creator qt ingest <in> <out>` | Parse `<in>.qml` (or every `*.qml` under `<in>/` in dir mode, per QT-08) and write `<out>/qt-ir.json` (file mode) or `<out>/<basename>.qt-ir.json` per file (dir mode). |
| `rlvgl-creator qt check <in>`        | Parse-only; non-zero exit on parse error. No IR emitted. File-only at QT-08. |
| `rlvgl-creator qt schema [--out P]`  | Emit JSON Schema for `qt-ir.json` (the `UiModule` type) to stdout or `P`. File-only at QT-08. |
| `rlvgl-creator qt emit <in> <out> [--target {data,rlvgl}]` | Lower `<in>.qml` (or every `*.qml` under `<in>/` in dir mode) to one Rust module per file. `--target rlvgl` (default) emits a runnable `<basename>.rlvgl.rs` with `build_screen(bounds) -> (WidgetNode, Rc<RefCell<ScreenState>>)` (depends on `rlvgl-core` + `rlvgl-widgets`). `--target data` emits a self-contained `<basename>.rs` static-data module (no external crate dependency). |

```bash
# Smoke test (also runs from the workspace root via cargo test):
cargo run --features creator --bin rlvgl-creator -- \
    qt ingest tests/fixtures/qt/hello.qml /tmp/qt-out
cat /tmp/qt-out/qt-ir.json

# Validate a designer-authored QML without committing IR to disk:
cargo run --features creator --bin rlvgl-creator -- \
    qt check path/to/Screen.qml

# Hand the IR's JSON Schema to your editor or a CI validator:
cargo run --features creator --bin rlvgl-creator -- \
    qt schema --out /tmp/qt-ir.schema.json
```

---

## What the MVP Extracts

The MVP parses the **structural subset** of QML. Every `.qml` produces a
`qt-ir.json` shaped like:

```jsonc
{
  "version": 1,
  "source": "tests/fixtures/qt/hello.qml",
  "imports":  [{ "module": "QtQuick", "version": "2.15", "alias": null }, ...],
  "pragmas":  [],
  "root": {
    "type_name": "Item",
    "id": "root",
    "properties":  [{ "name": "title", "ty": "string", "default_value": "\"Hello\"", ... }],
    "assignments": [{ "target": "anchors.fill", "value": { "kind": "expression", "text": "parent" } }],
    "signals":     [{ "name": "pressed", "params": [{ "name": "x", "ty": "int" }, ...] }],
    "handlers":    [{ "signal": "onClicked", "body": "root.count += 1" }],
    "children":    [{ "type_name": "Rectangle", ... }, ...]
  }
}
```

| Captured                                                 | Captured how           |
| -------------------------------------------------------- | ---------------------- |
| `import QtQuick 2.15 [as X]`, `import "."`               | Structured             |
| `pragma <directive>`                                     | Verbatim string        |
| Type instances: `Item { ... }`, `Rectangle { ... }`      | Recursive `UiItem`     |
| `id: <ident>`                                            | Structured             |
| `[default] [readonly] property <ty> <name>[: <expr>]`    | Structured + opaque RHS |
| `signal <name>(<ty> <name>, ...)`                        | Structured             |
| `onSignal: <expr>` and `onSignal: { ...JS... }`          | Signal name + opaque body |
| Plain assignments: `width: 800`, `text: root.title`      | Target + opaque expression |
| Dotted targets: `anchors.fill: parent`                   | Target preserved as `anchors.fill` |
| Grouped properties: `font { pixelSize: 32 }`             | `Object` value (lowercase typename heuristic) |
| Object-valued assignments: `transitions: Transition { ... }` | `Object` value (uppercase typename) |
| Child items: `Rectangle { ... }` inside an item body     | Pushed onto `children` |
| `function name(args) { ... }`                             | Captured as a `function:<name>` handler with verbatim body |
| `// line` and `/* block */` comments                     | Stripped               |

## What the MVP Does *Not* Capture

These are deliberate scope cuts for `QT-01a`. They land in later phases.

- **Type introspection.** No knowledge of which properties a `Rectangle`
  exposes, what its base class is, or whether `MouseArea.onClicked` is
  spelled correctly. The IR records what was *written*, not what the
  Qt type system says is *valid*. (Owned by `QT-02`+.)
- **Binding evaluation.** RHS expressions are stored as opaque text.
  No constant-folding, no JS engine. (Owned by `QT-04`.)
- **Multi-line expressions without surrounding `()` / `{}`.** A `:`
  binding terminates at the first newline at top depth. Wrap split
  expressions in parens.
- **`Behavior on <prop> { ... }`** and other QML-syntax-special forms
  with attached-property modifiers between the type name and the
  block. (Owned by `QT-04`.)
- **State machines.** `StateGroup` / `State` / `Transition` parse as
  child items, but their semantic lowering is a separate phase.
  (Owned by `QT-05`.)
- **`.qmldir` / `.pri` / `.qrc` resource manifests.** Single `.qml`
  in, single IR out. (Owned by `QT-08` CLI surface.)
- **Type generation.** Nothing is emitted as Rust yet — only the IR.
  (Owned by `QT-03`.)

## Diagnostics

Parse errors carry line and column information:

```text
Error: parsing tests/fixtures/qt/broken.qml

Caused by:
    expected `:` or `{` after `Behavior` at line 12, column 26
```

If you hit a "this should work but doesn't" parse failure on a real
QML file, please file an issue with the fixture. The MVP grammar is
intentionally narrow but should accept any QML written in the style
shown above.

---

## External Tool Requirements

### MVP path (current)

| Dependency               | Required? | Install command          | Notes                                                |
| ------------------------ | --------- | ------------------------ | ---------------------------------------------------- |
| Rust toolchain           | yes       | `rustup` / your usual    | Same toolchain as the rest of `rlvgl`.               |
| Homebrew formulae        | **none**  | —                        | The MVP ingest path has no system dependencies.      |
| `pip` packages           | **none**  | —                        | No Python runtime needed.                             |
| Qt installation          | **none**  | —                        | The parser does not link against Qt or QtQuick.       |
| `qmlplugindump`          | **none**  | —                        | Reserved for the future richer-ingest tier.           |
| `pyside6` Python module  | **none**  | —                        | Reserved for the future richer-ingest tier.           |

In other words: nothing extra. `cargo build --features creator` is
sufficient.

### Future tier — type-introspection ingest (planned, not shipped)

The MVP parses what is *written* in a single `.qml` file. To know what
*could* be written — i.e. what properties `Rectangle` exposes, which
classes derive from `Item`, what types a Qt Design Studio project
makes available — we need either Qt's own `qmlplugindump` binary or a
Python-side walk over `QMetaObject`. Both are opt-in installs, and
both are tracked under `QT-01b`.

> **Not a current dependency.** Do not install these unless you are
> working on the `QT-01b` phase. They are listed here so the
> dependency story for creator is fully written down in one place.

#### Option A: Qt 6 via Homebrew

```bash
brew install qt
# Tools land under $(brew --prefix qt)/share/qt/libexec/. Common ones:
$(brew --prefix qt)/share/qt/libexec/qmltyperegistrar --version
# qmlplugindump is deprecated in Qt 6.4+ but still ships:
$(brew --prefix qt)/share/qt/libexec/qmlplugindump --help 2>&1 | head -3
```

Disk cost: ~600 MB. Provides the canonical Qt-supplied introspection
binaries. Only install if you are actively working on the type-
introspection tier.

#### Option B: PySide6 via pip

```bash
python3 -m venv ~/.venvs/rlvgl-qt
source ~/.venvs/rlvgl-qt/bin/activate
pip install --upgrade pip
pip install PySide6
python3 -c "import PySide6; print(PySide6.__version__)"
```

Disk cost: ~250 MB inside the venv. Lets us walk `QMetaObject`
programmatically without invoking Qt's own command-line tools. The
creator binary will detect the active venv via `python3` on `PATH` —
keep the venv active in the shell that runs `qt ingest --type-info`
(future flag, not yet wired).

`PySide6` is the only Python dep planned for this tier; no `requirements.txt`
will be vendored into the rlvgl repo until `QT-01b` actually ships.

---

## Other External Setup the Creator Touches

These are unrelated to Qt but listed so this page is the single
"external setup" reference for `rlvgl-creator`:

| Subcommand                      | External tool needed | Optional? | Install                          |
| ------------------------------- | -------------------- | --------- | -------------------------------- |
| `rlvgl-creator new <name>`      | `git`                | optional  | `brew install git`               |
| `rlvgl-creator scaffold …`      | `cargo`              | required  | already installed via rustup     |
| `rlvgl-creator run`             | `cargo`              | required  | already installed via rustup     |
| `rlvgl-creator lottie cli …`    | `lottie-cli` binary  | optional  | per the Lottie project's docs    |
| `rlvgl-creator qt ingest`       | none                 | n/a       | n/a                               |

If creator ever grows a feature that requires another external tool,
add the row here in the same PR — that is the rule the doc exists
to enforce.

---

## Canonical Artifacts (frozen by QT-02)

The IR contract is locked in two checked-in files:

| Artifact                                                              | Purpose                                                              |
| --------------------------------------------------------------------- | -------------------------------------------------------------------- |
| [`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json)        | Canonical JSON Schema for `qt-ir.json`. Editor / CI integration target. Frozen by QT-02. |
| [`tests/fixtures/qt/hello.qt-ir.json`](../../tests/fixtures/qt/hello.qt-ir.json) | Canonical golden ingest of `hello.qml`. Pinned by the QT-02 drift gate. |
| [`tests/fixtures/qt/hello.rs`](../../tests/fixtures/qt/hello.rs)      | Canonical data-target Rust emit of `hello.qml`. Pinned by the QT-03 drift + compile-as-mod gates. |
| [`tests/fixtures/qt/hello.rlvgl.rs`](../../tests/fixtures/qt/hello.rlvgl.rs) | Canonical rlvgl-target Rust emit. Pinned by the QT-03b drift + compile-as-mod gates (the latter compiles the emitted module against `rlvgl-core` + `rlvgl-widgets`). |
| [`tests/fixtures/qt/clickable.qml`](../../tests/fixtures/qt/clickable.qml) | QT-04 fixture exercising `Button` + `onClicked`. |
| [`tests/fixtures/qt/clickable.qt-ir.json`](../../tests/fixtures/qt/clickable.qt-ir.json) | Canonical IR for the clickable fixture. |
| [`tests/fixtures/qt/clickable.rs`](../../tests/fixtures/qt/clickable.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/clickable.rlvgl.rs`](../../tests/fixtures/qt/clickable.rlvgl.rs) | Canonical rlvgl-target emit with lowered `set_on_click` closure. Pinned by the QT-04 drift + compile-as-mod gates. |
| [`tests/fixtures/qt/counter.qml`](../../tests/fixtures/qt/counter.qml) | QT-04b fixture exercising `property int count: 0` + `onClicked: count += 1` (state-mutating handler). |
| [`tests/fixtures/qt/counter.qt-ir.json`](../../tests/fixtures/qt/counter.qt-ir.json) | Canonical IR for the counter fixture. |
| [`tests/fixtures/qt/counter.rs`](../../tests/fixtures/qt/counter.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/counter.rlvgl.rs`](../../tests/fixtures/qt/counter.rlvgl.rs) | Canonical rlvgl-target emit with `pub struct ScreenState { pub count: i32 }` and a state-mutating closure. Pinned by the QT-04b drift + synthetic-click compile-as-mod gates. |
| [`tests/fixtures/qt/bound_text.qml`](../../tests/fixtures/qt/bound_text.qml) | QT-04c fixture exercising `text:` bound to a root-scope `string` property. |
| [`tests/fixtures/qt/bound_text.qt-ir.json`](../../tests/fixtures/qt/bound_text.qt-ir.json) | Canonical IR for the bound_text fixture. |
| [`tests/fixtures/qt/bound_text.rs`](../../tests/fixtures/qt/bound_text.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/bound_text.rlvgl.rs`](../../tests/fixtures/qt/bound_text.rlvgl.rs) | Canonical rlvgl-target emit with `Label::new(state.borrow().title.clone(), bounds)` + a `LabelBinding` push (QT-04e). Pinned by the QT-04c+QT-04e drift + reactive compile-as-mod gate (mutate state → call `refresh_bindings` → label text updates). |
| [`tests/fixtures/qt/centered.qml`](../../tests/fixtures/qt/centered.qml) | QT-03c fixture exercising `anchors.centerIn: parent` with literal child `width`/`height`. |
| [`tests/fixtures/qt/centered.qt-ir.json`](../../tests/fixtures/qt/centered.qt-ir.json) | Canonical IR for the centered fixture. |
| [`tests/fixtures/qt/centered.rs`](../../tests/fixtures/qt/centered.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/centered.rlvgl.rs`](../../tests/fixtures/qt/centered.rlvgl.rs) | Canonical rlvgl-target emit with the centered-bounds arithmetic + `// QT-03c centered:` marker. Pinned by the QT-03c drift + bounds-assertion compile-as-mod gates. |
| [`tests/fixtures/qt/multi/`](../../tests/fixtures/qt/multi/) | QT-08 multi-file fixture (`a.qml`, `b.qml`). Pinned by the QT-08 dir-mode drift gates that verify the walker emits one output per `*.qml` child. |
| [`tests/fixtures/qt/nested.qml`](../../tests/fixtures/qt/nested.qml) | QT-04f fixture exercising a non-root id'd Rectangle's `property int alpha: 100` referenced from a sibling Button's `onClicked: bg.alpha -= 10`. |
| [`tests/fixtures/qt/nested.qt-ir.json`](../../tests/fixtures/qt/nested.qt-ir.json) | Canonical IR for the nested fixture. |
| [`tests/fixtures/qt/nested.rs`](../../tests/fixtures/qt/nested.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/nested.rlvgl.rs`](../../tests/fixtures/qt/nested.rlvgl.rs) | Canonical rlvgl-target emit with `pub struct ScreenState { pub bg_alpha: i32 }` and a closure that mutates the namespaced field. Pinned by the QT-04f drift + synthetic-click compile-as-mod gates. |
| [`tests/fixtures/qt/edges.qml`](../../tests/fixtures/qt/edges.qml) | QT-03c §5 amendment fixture exercising each of `anchors.left`/`right`/`top`/`bottom` in isolation. |
| [`tests/fixtures/qt/edges.qt-ir.json`](../../tests/fixtures/qt/edges.qt-ir.json) | Canonical IR for the edges fixture. |
| [`tests/fixtures/qt/edges.rs`](../../tests/fixtures/qt/edges.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/edges.rlvgl.rs`](../../tests/fixtures/qt/edges.rlvgl.rs) | Canonical rlvgl-target emit with `// QT-03c edge:` markers and edge-positioned `Rect` arithmetic. Pinned by the QT-03c drift + bounds-assertion compile-as-mod gates. |
| [`tests/fixtures/qt/corners.qml`](../../tests/fixtures/qt/corners.qml) | QT-03c §5 amendment #2 fixture exercising the four corner combinations (`left+top`, `right+top`, `left+bottom`, `right+bottom`). |
| [`tests/fixtures/qt/corners.qt-ir.json`](../../tests/fixtures/qt/corners.qt-ir.json) | Canonical IR for the corners fixture. |
| [`tests/fixtures/qt/corners.rs`](../../tests/fixtures/qt/corners.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/corners.rlvgl.rs`](../../tests/fixtures/qt/corners.rlvgl.rs) | Canonical rlvgl-target emit with `// QT-03c corner:` markers and corner-positioned `Rect` arithmetic. Pinned by the QT-03c drift + bounds-assertion compile-as-mod gates. |
| [`tests/fixtures/qt/mousearea.qml`](../../tests/fixtures/qt/mousearea.qml) | QT-04d fixture exercising `MouseArea` + `onClicked: taps += 1`. |
| [`tests/fixtures/qt/mousearea.qt-ir.json`](../../tests/fixtures/qt/mousearea.qt-ir.json) | Canonical IR for the mousearea fixture. |
| [`tests/fixtures/qt/mousearea.rs`](../../tests/fixtures/qt/mousearea.rs) | Canonical data-target emit. |
| [`tests/fixtures/qt/mousearea.rlvgl.rs`](../../tests/fixtures/qt/mousearea.rlvgl.rs) | Canonical rlvgl-target emit lowering MouseArea to `rlvgl_widgets::click_area::ClickArea` with a state-mutating `set_on_click` closure. Pinned by the QT-04d drift + synthetic-click compile-as-mod gates. |

Both are regenerated via the commands embedded in the failure
messages of the schema-drift / golden-file tests. The frozen
regeneration commands are owned by
[QT-02 §5](../qt-support/02-ir-schema.md#5--frozen-decision-regeneration-commands);
when in doubt, copy them straight from the test panic output.

---

## See Also

- [`docs/qt-support/`](../qt-support/) — multi-phase plan and conformance targets.
- [`docs/creator/CLI.md`](./CLI.md) — full creator command-line reference.
- [`docs/creator/ASSET-PIPELINE.md`](./ASSET-PIPELINE.md) — how Qt-referenced
  images and fonts will eventually flow into the existing dual-mode
  assets crate (owned by `QT-07`).

---

MIT-licensed: MIT.
