<!--
05e-externals-stubs.md - QT-05e: Externals stub emission. Closes QT-05.
-->

**[← Prev](05d-emit-scjson.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05e — Externals Stub Emission (closes QT-05)

QT-05a-d wired up the **observable** half of the QT-05 pipeline:
ingest scjson, dispatch from QML handlers, refresh DM-driven Label
text, emit scjson from inline QML. QT-05e wires up the **callout**
half: for every `<script name="…"/>` element discovered in the
attached scjson, emit a sibling `<basename>_externals.rs` file
containing a hand-fillable `ScreenExternals` struct that
implements the istate-codegen `Externals` trait.

The user fills in the method bodies with side-effect code (timer
start/stop, IO, audio, …) and installs the impl on `Machine` via
`Machine::with_options`-equivalent factory. QT-05e is the close-out
phase of the QT-05 family.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary),
[QT-05 §3](./05-state-machines.md#3--canonical-glossary-delta-only),
and [QT-05a §3](./05a-scjson-ingest.md#3--canonical-glossary-delta-only).
The `ScreenExternals` shape, the sibling-file convention, and the
`qt emit-externals` CLI subcommand are owned here.

## §1 — Purpose

After QT-05a-d, a QT-05c-bound Label updates from `dm.elapsed`
when the consumer mutates that field. But **who** mutates
`dm.elapsed`? In the scjson model, side-effects live in
`<script name="…"/>` callouts — invoked from `<onentry>`,
`<onexit>`, and transition action lists. istate-codegen's Rust
template emits these as default-no-op methods on a public
`Externals` trait; the application provides a concrete impl.

Without QT-05e the user reads QT-05's `Externals` description and
hand-writes the impl from scratch. QT-05e shortens that to:

```bash
$ rlvgl-creator qt emit-externals stopwatch.qml
$ # writes stopwatch_externals.rs with ScreenExternals stubs.
```

The user fills in the method bodies and links the file via
`mod stopwatch_externals;`. The `ScreenExternals` struct can be
installed on the `Machine` via the public field
`machine.borrow_mut().externals = Box::new(ScreenExternals::new(…))`
(linkage v1's `Machine.externals: Box<dyn Externals>` is public per
QT-05 §6).

## §2 — Problem Statement

Three concrete gaps:

- **Discovery**. Without QT-05e, a user updating their `.scjson`
  with new `<script>` callouts has to manually update an
  `Externals` impl in another file. Drift between the two is a
  silent failure (default no-op stub remains in place).
- **Naming**. istate's template synthesises script names from
  position when the SCXML doesn't provide one (e.g.
  `script_trans_idle_running_0`). QT-05e mirrors that synthesis
  exactly so the user's hand-edited impl method names match.
- **Boilerplate**. Even a five-callout state machine is ~30 lines
  of `impl Externals` boilerplate. Generating that frees the user
  to write only the body of each method.

## §3 — Canonical Glossary (delta only)

QT-05e introduces no new IR types. One new emitted Rust struct,
one CLI subcommand, two new helpers.

### `ScreenExternals`

```rust
pub struct ScreenExternals;

impl ScreenExternals {
    pub fn new() -> Self {
        Self
    }
}

impl <sm>_gen::Externals for ScreenExternals {
    fn <callout>(&mut self, m: &mut <sm>_gen::Machine) {
        // QT-05e externals-stub: TODO — fill in side-effect code.
        let _ = m; // silence unused-arg warning
    }
    // … one method per discovered <script name="…"/> …
}
```

Owned here. `pub struct ScreenExternals` is **stateless** in the
v1 emit shape — the user ports state in by editing the struct
fields by hand. (A future amendment may emit the
`pub state: Rc<RefCell<ScreenState>>` field automatically; this
keeps the v1 emit minimal.)

### `qt emit-externals <input> [<out>]`

CLI subcommand. File mode: writes
`<input_dir>/<basename>_externals.rs`. Directory mode: walks
`*.qml` and emits one externals file per QML that has an attached
state machine with at least one script.

**Idempotent** for a fixed input pair: re-running produces
byte-identical output. **Not safe to re-run after the user has
edited the file** — QT-05e v1 overwrites unconditionally. (A
future amendment may add a `--diff-only` mode or merge logic.)

### `// QT-05e externals-stub:` marker

Emitted as the first line inside each generated impl method body
(directly above the user-fillable TODO line). Reviewers grep on
this exact prefix.

### `<basename>_externals.rs` file naming

For input `screens/stopwatch.qml` whose IR's
`state_machine.scripts` is non-empty, the externals file is
`screens/stopwatch_externals.rs`. The user `mod`'s it via
`mod stopwatch_externals;` from a parent module.

## §4 — Source-of-Truth Map

| Concept                                       | Owner                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| `state_machine.scripts: Vec<UiScript>`         | QT-05 §3.                                                               |
| Script-name synthesis when scjson omits `name` | QT-05a §6 (mirrors istate's `context.py::_extract_actions`).            |
| `Externals` trait shape                        | upstream istate template (linkage v1; QT-05 §6).                        |
| `ScreenExternals` struct shape                 | this chapter (§3).                                                       |
| Sibling-file naming                            | this chapter (§3).                                                       |
| `qt emit-externals` CLI                        | this chapter (§5).                                                       |
| `// QT-05e externals-stub:` marker             | this chapter (§3).                                                       |
| `Machine.externals` install path               | QT-05 §6 (the public `pub externals: Box<dyn Externals>` field).         |
| State-aware externals (carrying `ScreenState`) | **deferred** — v1 emits a stateless `ScreenExternals`.                  |
| Merge / diff with user-edited externals files  | **deferred** — v1 overwrites.                                           |

## §5 — Frozen Decision: CLI Surface

```text
USAGE:
    rlvgl-creator qt emit-externals <INPUT> [<OUT>]

ARGS:
    <INPUT>    Path to a `.qml` file or a directory containing `.qml` files.
    <OUT>      Output path (file or directory). Defaults to the input directory.
```

| Mode | `<INPUT>` | `<OUT>` (provided) | `<OUT>` (default) | Behaviour |
| ---- | --------- | ------------------ | ----------------- | --------- |
| File | `path/to/foo.qml` | `dir/`             | (parent of input) | Writes `<OUT>/foo_externals.rs`. |
| File | `path/to/foo.qml` | `dir/bar.rs`        | n/a               | Writes the named file. |
| Dir  | `path/to/screens/` | `dir/`            | (input itself)    | For every `*.qml` with a state machine in the dir, writes `<OUT>/<basename>_externals.rs`. |

A `.qml` without an attached state machine, or with an attached
SM but with empty `scripts`, **does not** produce an externals
file (silent skip — caller-driven contract).

## §6 — Frozen Decision: Emit Order

For a `UiModule` whose `state_machine.scripts` is non-empty:

1. Standard SPDX header + provenance comment naming
   `<basename>.qml` and the regen command.
2. `#![allow(dead_code)]` and `#![allow(unused_variables)]`
   blanket allows.
3. `extern crate alloc;` (matching the QT-04b emit shape).
4. `use <sm>_gen::{Externals, Machine};`.
5. Per-script comment block (one block per discovered script,
   declaration order):
   ```rust
   // QT-05e externals-stub: <script_name> from <origin>
   ```
   where `<origin>` is the `UiScriptOrigin` rendered as
   `OnEntry { state: …}` etc.
6. `pub struct ScreenExternals;` plus
   `impl ScreenExternals { pub fn new() -> Self { Self } }`.
7. `impl Externals for ScreenExternals { … }` containing one
   method per discovered script:
   ```rust
   fn <name>(&mut self, m: &mut Machine) {
       // QT-05e externals-stub: TODO — fill in side-effect code.
       let _ = m;
   }
   ```

The method order matches `state_machine.scripts` declaration
order (which is itself stable per QT-05a §6 — onentry-then-
onexit-then-transition).

## §7 — Frozen Decision: Install Path

The user installs `ScreenExternals` on a `Machine` by replacing
the public `externals` field:

```rust
let (node, state, machine, bindings) = build_screen(bounds);
machine.borrow_mut().externals = Box::new(ScreenExternals::new());
```

This is the linkage v1 contract from QT-05 §6: `Machine.externals`
is a public `Box<dyn Externals>` field, so direct assignment is
the canonical install path. `Machine::with_options(internal_events,
log_to_stderr)` constructs a Machine but does not provide a
constructor that takes externals at construction time — the v1
istate template assigns `Box::new(DefaultExternals)` in `new()`
unconditionally. A consumer who needs externals installed before
the first dispatch can:

```rust
let machine = Rc::new(RefCell::new(Machine::with_options(false, true)));
machine.borrow_mut().externals = Box::new(ScreenExternals::new());
// ... pass `machine` into a custom `build_screen` that doesn't call Machine::new()
```

QT-05e does **not** emit a custom build_screen variant for this
flow — it's caller responsibility.

## §8 — Versioning

| Constant                       | Before QT-05e | After QT-05e |
| ------------------------------ | ------------- | ------------ |
| `QT_EMIT_VERSION_RLVGL`        | 13            | unchanged    |
| `QT_IR_VERSION`                | 2             | unchanged    |
| `QT_EMIT_VERSION_DATA`         | 1             | unchanged    |
| `ISTATE_LINKAGE_VERSION`       | 1             | unchanged    |

QT-05e's artifact is a separate `<basename>_externals.rs` file,
not part of the versioned `build_screen` emit-shape. No bumps to
the existing version constants. The `<basename>_externals.rs`
file embeds its own version constant for traceability:

```rust
pub const QT_EXTERNALS_VERSION: u32 = 1;
```

This bumps when the externals emit shape changes (e.g. when a
future amendment emits `pub state` field for stateful
externals).

## §9 — Non-Goals

- **No automatic install.** The user wires `machine.borrow_mut().externals = …`
  by hand. QT-05e doesn't modify `build_screen`.
- **No state injection.** `ScreenExternals` v1 is stateless. The
  user adds fields by editing the file; future regenerations
  preserve neither the fields nor the bodies (the file
  overwrites). Plan accordingly: keep complex external state in
  a separate module that `ScreenExternals` references.
- **No diff-merge.** Re-running emits-externals overwrites. The
  user is expected to commit and resolve diffs via git.
- **No automatic `Externals` invocation from QML handlers.**
  Externals are invoked by istate-codegen's generated `dispatch`
  body when a transition's `<script>` action fires. QT-05b's
  `dispatch` lowering already routes through `Machine::dispatch`,
  so no QT-05e-specific glue is needed.
- **No multi-screen externals merging.** Each QML that has its
  own state machine gets its own `_externals.rs`. Sharing a
  single externals impl across multiple SMs is a v2 concern.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                | Resolution                                                                                                                        |
| -------- | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| QT-05    | 6-symbol linkage surface, `Externals` trait pin.       | QT-05e is the first chapter to **consume** the `Externals` symbol from the linkage surface. v1 stays within the pin.             |
| QT-05a   | scjson side-file ingest; populates `scripts`.          | QT-05e reads the populated `scripts` table verbatim.                                                                              |
| QT-05b   | Dispatch glue.                                         | Independent. Externals are invoked by istate-codegen's emitted `Machine::dispatch` body, not by QT-05b's QML handler closures. |
| QT-05c   | DM/State bindings.                                     | Independent. An externals method can mutate `m.dm.<field>` directly; QT-05c's `refresh_bindings` picks up the change on next call. |
| QT-05d   | QML → scjson emission.                                 | Independent. QT-05e walks whatever scripts QT-05d's `.scjson` carries (currently zero — QT-05d v1 doesn't emit `<script>`).      |
| QT-08    | Directory-mode CLI.                                    | QT-05e's directory mode shares QT-08's `qt08_collect_qml_files` walker.                                                          |

## §11 — Acceptance Checklist

QT-05e is **ratified and shipped** when:

- [x] §3 names `ScreenExternals`, the sibling-file naming, and
      the marker.
- [x] §5 fixes the CLI surface.
- [x] §6 fixes the per-screen emit order.
- [x] §7 fixes the install path.
- [x] `qt::emit_externals(input, out)` lands; CLI subcommand
      `qt emit-externals` wired.
- [x] `tests/fixtures/qt/stopwatch_externals.rs` is the emitted
      golden for the existing stopwatch fixture (tick_start +
      tick_stop scripts).
- [x] Drift gate: `qt emit-externals` against the stopwatch
      fixture is byte-equal to the golden.
- [x] `QT-05` family closeout recorded in `00-concepts.md` §15.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/05-state-machines.md`](./05-state-machines.md) — `Externals` linkage surface.
- [`docs/qt-support/05a-scjson-ingest.md`](./05a-scjson-ingest.md) — populates `state_machine.scripts`.
- [`docs/qt-support/05d-emit-scjson.md`](./05d-emit-scjson.md) — sibling QML→scjson authoring path.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emit + CLI wiring.
- [`tests/fixtures/qt/stopwatch.qml`](../../tests/fixtures/qt/stopwatch.qml) — canonical fixture.
- [`tests/fixtures/qt/stopwatch.scjson`](../../tests/fixtures/qt/stopwatch.scjson) — has tick_start / tick_stop scripts.
- [`tests/fixtures/qt/stopwatch_externals.rs`](../../tests/fixtures/qt/stopwatch_externals.rs) — emitted golden.
- [`tests/creator_qt_ingest.rs`](../../tests/creator_qt_ingest.rs) — drift gate.

## §13 — Unblocks

Ratifying QT-05e closes the QT-05 family. Unblocks:

- Real-project bring-up: full QML-to-runnable round-trip with
  QT-05a-d-e.
- QT-06 (theme-token reconciliation), QT-07 (asset handoff),
  QT-08b (`.qmldir`), QT-08c (`.qrc`) all become natural next
  slices.
- A future QT-05f (deferred): stateful externals, merge-on-regen,
  multi-screen externals consolidation.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | QT-05e ratified and shipped. **Closes the QT-05 family.** New CLI subcommand `qt emit-externals <input> [<out>]` (file mode + directory mode per QT-08) walks `module.state_machine.scripts` and writes a sibling `<basename>_externals.rs` containing a `pub struct ScreenExternals` with `impl Externals for ScreenExternals` covering one method per discovered script. New entry point `qt::emit_externals(input, out)`; new helpers `resolve_externals_out_for(qml, out_dir)` and `emit_externals_one(input, out_path)` mirror the QT-05d emit-scjson pattern. Method bodies are TODO stubs with `// QT-05e externals-stub: <name> from <origin>` markers; users fill in side-effect code by hand. Method names match QT-05a's synthesis convention (matching istate's `context.py::_extract_actions`) so user-edited bodies survive scjson updates that don't rename scripts. New per-file emit-shape constant `QT_EXTERNALS_VERSION = 1` for traceability. Install path documented per §7: `machine.borrow_mut().externals = Box::new(ScreenExternals::new())` against the public `Machine.externals: Box<dyn Externals>` field from linkage v1. New fixture `tests/fixtures/qt/stopwatch_externals.rs` (emitted golden — stopwatch.scjson has `tick_start` / `tick_stop` scripts) + 1 byte-equality drift gate. No bumps to `QT_IR_VERSION`, `QT_EMIT_VERSION_RLVGL`, `QT_EMIT_VERSION_DATA`, or `ISTATE_LINKAGE_VERSION` — QT-05e's artifact is a separate file. Stateful externals (struct fields), merge-on-regen, and multi-screen consolidation remain deferred under a hypothetical future QT-05f. With QT-05e the QT-05 family is feature-complete; subsequent Qt work moves to QT-06 (theme tokens), QT-07 (asset handoff), or QT-08b/c (qmldir / qrc). |

---

MIT-licensed: MIT.
