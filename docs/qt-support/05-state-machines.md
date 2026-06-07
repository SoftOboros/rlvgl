<!--
05-state-machines.md - QT-05: Qt state machines via istate-codegen.
-->

**[← Prev](04f-nested-id-resolution.md) · [Index](README.md) · [Next →](#)**

# Chapter QT-05 — State Machines (istate-codegen Linkage)

QT-05 introduces Qt-side state-machine support to `rlvgl-creator`
without ever generating a state-machine engine inside this repo.
The pipeline is:

```
QML (states / transitions / scripts)
    │
    ▼  rlvgl-creator qt emit-scjson  (QT-05d)
<screen>.scjson  ──►  istate-codegen MCP (out-of-process)  ──►  <sm>_gen/ crate
    │                                                                │
    ▼                                                                │
rlvgl-creator qt emit --target rlvgl  (QT-05b/c/e)                    │
    │                                                                │
    ▼                                                                ▼
screen.rs (build_screen 4-tuple) ─── path-deps ───►  <sm>_gen::{Machine, Event, State, DataModel, Externals, DefaultExternals}
<sm>_externals.rs (impl Externals)
```

QT-05 (this chapter) is the **seed phase**: it freezes the contracts
that QT-05a-e implement. No emitter behaviour ships here. What ships
is the linkage surface, the on-disk vocabulary, the submodule pin,
and the IR shape.

## §0 — Authority Policy

Normative keywords are interpreted per RFC 2119 / 8174. Vocabulary
defers to [QT-00 §3](./00-concepts.md#3--canonical-glossary). The
on-disk scjson element/attribute names defer to the upstream scjson
project at [`vendor/scjson/`](../../vendor/scjson) (BSD-1-Clause,
pinned by submodule), specifically the type definitions in
`vendor/scjson/rust/src/scjson_props.rs` and the meta-template at
`vendor/scjson/py/scjson/templates/scjson_props.rs.jinja2`. The
state-machine engine code shape defers to the istate-codegen Rust
template at
`softoboros/backend/templates/codegen/rust/src/lib.rs.jinja2` (read
into this chapter as "prior knowledge" — the upstream is not pinned
into this repo). Anything *not* covered by those upstreams (the
6-symbol linkage surface, the IR types, file layout, the `// QT-05`
markers) is owned here.

## §1 — Purpose

`rlvgl-creator` must accept Qt projects that contain state machines
and emit Rust glue that drives them, without taking on the burden
of being an SCXML interpreter. The decision is to defer all
state-machine semantics to **istate** (already part of the
softoboros backend, already battle-tested against the W3C SCXML
test vectors) and use **scjson** as the on-disk wire format between
the two repos.

After QT-05 (concepts), QT-05a-e ship the implementation:

- **QT-05a** — read `<screen>.scjson` side-files and merge into IR.
- **QT-05b** — emit `Rc<RefCell<Machine>>` threading + `dispatch`
  glue for QML `on*` handlers.
- **QT-05c** — emit `MachineBinding::TextFromDm` /
  `VisibilityFromState` extending the QT-04e refresh pump.
- **QT-05d** — emit scjson **from** QML `States {}` /
  `transitions:` blocks. (QML → scjson; round-trip parity with
  upstream `scjson` CLI.)
- **QT-05e** — emit `<sm>_externals.rs` stubs for every scjson
  `<script name="…"/>` callout. Closes out the QT-05 family.

## §2 — Problem Statement

SCXML semantics are subtle (entry/exit ordering across compound
states, parallel regions, history, internal-vs-external transitions,
`<finalize>`, microstep / macrostep ordering, eventless
transitions). Re-implementing them inside `rlvgl-creator` would:

- Duplicate work istate already does.
- Risk silent semantic drift between rlvgl's emitter and the
  authoring tool the user actually validates against (Qt Design
  Studio + istate's own SCXML test vectors).
- Bake `std::collections::VecDeque` and `Box<dyn Externals>` into
  `rlvgl-creator`'s output (the istate Rust template's current
  shape) — which is exactly the kind of decision that should live
  *outside* this repo so it can be revisited by istate without a
  rlvgl-creator amendment.

The opposing failure mode — letting Qt state machines through
unchanged and asking the user to wire scjson → istate by hand —
fails the spec-before-code discipline (QT-04 family established
that handler/binding glue is creator's job, not the user's).

QT-05 splits the difference: the **engine** is istate's; the
**glue** is creator's; the **wire format** is scjson, pinned by
submodule.

## §3 — Canonical Glossary (delta only)

QT-05 introduces five IR types and one external linkage envelope.

### `UiStateMachine`

Top-level Qt-side state-machine record. Lives next to the existing
`UiModule.root: UiItem`.

```rust
pub struct UiStateMachine {
    pub id: String,                  // Rust crate name stem; `<id>_gen` is the istate crate.
    pub source: String,              // path to the .scjson file (relative to project root). Stored as `String` on the wire to keep `schemars` derive trivial; ingest treats it as a path. (Amended QT-05 §15 2026-04-29.)
    pub initial: Option<String>,     // initial state ID, mirrors scjson `<scxml initial="…">`.
    pub states: Vec<UiState>,
    pub transitions: Vec<UiTransition>,
    pub datamodel: Vec<UiDmField>,
    pub scripts: Vec<UiScript>,      // discovered <script name="…"/> callouts.
}
```

### `UiState`

```rust
pub struct UiState {
    pub id: String,                  // PascalCased to State::<Id> per istate template rules.
    pub on_entry: Vec<UiAction>,
    pub on_exit: Vec<UiAction>,
}
```

### `UiAction`

Sealed enum over the executable-content elements in the QT-05 §5
subset that contribute to entry / exit / transition action lists.
The variants mirror the scjson elements they encode; istate's
`context.py::_extract_actions` consumes the same shape on the
Python side. Adding a variant is a Specification-Required amendment
to QT-05 §5 and to this glossary.

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAction {
    /// `<assign location="…" expr="…"/>` — write to a `DataModel` field.
    Assign { location: String, expr: Option<String> },
    /// `<raise event="…"/>` — internal-event raise.
    Raise { event: String },
    /// `<script name="…"/>` — callout to an `Externals` method.
    /// `name` references back into `UiStateMachine.scripts[*].name`.
    Script { name: String },
}
```

### `UiTransition`

```rust
pub struct UiTransition {
    pub source: String,
    pub event: Option<String>,       // PascalCased to Event::<Name> per istate template rules.
    pub target: Option<String>,
    pub cond: Option<String>,        // raw scjson `cond` string; parsed by istate, not us.
    pub actions: Vec<UiAction>,
}
```

### `UiDmField`

```rust
pub struct UiDmField {
    pub id: String,                  // snake_case; Rust ident on `DataModel.<id>`.
    pub initial: Option<f64>,        // numeric literals only, per current istate scaffold.
}
```

### `UiScript`

```rust
pub struct UiScript {
    pub name: String,                // method name on the generated Externals trait.
    pub origin: UiScriptOrigin,      // transition vs. onentry vs. onexit (for diagnostics).
}
```

### "Linkage v1"

Shorthand for the QT-05 §6 frozen six-symbol istate Rust surface
(`Machine`, `Event`, `State`, `DataModel`, `Externals`,
`DefaultExternals`). When this chapter says "the linkage", it
means linkage v1 unless explicitly versioned. `ISTATE_LINKAGE_VERSION`
is a numeric constant emitted at the top of every QT-05-touching
generated module so reviewers can grep for it.

### "scjson subset"

The element/attribute set listed in §5. The hand-rolled types live
at `src/bin/creator/qt_scjson.rs`. They are **not** a Cargo dep on
the upstream scjson crate; they are an internal serde subset whose
field naming is wire-compatible by virtue of inspecting the upstream
template and submodule.

### `<sm>_gen` crate

The Rust crate emitted by istate-codegen for a given scjson document.
Default location in the consumer crate is `crates/<sm>_gen/`. The
consumer's `Cargo.toml` gains a path dep
`<sm>_gen = { path = "crates/<sm>_gen" }`. `rlvgl-creator` does
**not** generate this crate — it depends on it.

### `ScreenExternals`

The struct emitted by QT-05e at `src/<sm>_externals.rs` that
implements `<sm>_gen::Externals`. Holds `&mut ScreenState` (and
optionally `&mut Vec<LabelBinding>` if the externals need to refresh
bound widgets). Method bodies start as `// TODO QT-05e:` stubs that
the user fills in.

### `// QT-05` marker family

Mirrors the QT-04 marker scheme:

| Marker prefix                   | Where it appears                                                    |
| ------------------------------- | ------------------------------------------------------------------- |
| `// QT-05a scjson:`             | Top of every emitted module that has a state-machine attached.       |
| `// QT-05b dispatch:`           | Above each `machine.borrow_mut().dispatch(...)` call.                |
| `// QT-05c machine-bound:`      | Above each `MachineBinding` push (analogue of QT-04e bound:).        |
| `// QT-05d emit-scjson:`        | Top of every emitted `.scjson` file (as a JSON `"_comment"` key).    |
| `// QT-05e externals-stub:`     | Inside each `ScreenExternals` method body before the TODO.           |

Reviewers grep on these exact prefixes.

## §4 — Source-of-Truth Map

| Concept                                        | Owner                                                                     |
| ---------------------------------------------- | ------------------------------------------------------------------------- |
| SCXML execution semantics                      | upstream W3C SCXML; istate's interpreter mirrors them.                    |
| scjson on-disk schema                          | `vendor/scjson/scjson.schema.json` (BSD-1-Clause; submodule pin).         |
| scjson Rust type names + serde rename rules    | `vendor/scjson/py/scjson/templates/scjson_props.rs.jinja2`.               |
| State-machine engine Rust code shape           | `softoboros/backend/templates/codegen/rust/src/lib.rs.jinja2` (read as prior knowledge; not pinned). |
| `<sm>_gen/Cargo.toml` shape                    | `softoboros/backend/templates/codegen/rust/Cargo.toml.jinja2`.            |
| 6-symbol linkage surface                       | this chapter (§6).                                                        |
| scjson element subset                          | this chapter (§5).                                                        |
| IR types (`UiStateMachine`, …)                 | this chapter (§3); JSON Schema export owned by QT-02 once amended.        |
| Side-file discovery rules (`.qml` ↔ `.scjson`) | QT-05a.                                                                   |
| Handler `dispatch` glue                        | QT-05b.                                                                   |
| `MachineBinding` shape + refresh integration   | QT-05c.                                                                   |
| QML `States {}` → scjson emit                  | QT-05d.                                                                   |
| `ScreenExternals` stub emission                | QT-05e.                                                                   |

## §5 — Frozen Decision: scjson Element Subset

Registration policy: **Specification Required**. Adding an element
requires an amendment in the chapter that consumes it (typically
QT-05a for ingest, QT-05d for emit).

| Element       | scjson key      | Status   | Owner chapter |
| ------------- | --------------- | -------- | ------------- |
| `<scxml>`     | top-level       | required | this chapter  |
| `<state>`     | `state[]`       | required | this chapter  |
| `<transition>`| `transition[]`  | required | this chapter  |
| `<datamodel>` | `datamodel`     | required | this chapter  |
| `<data>`      | `datamodel.data[]` | required | this chapter |
| `<onentry>`   | `onentry[]`     | required | this chapter  |
| `<onexit>`    | `onexit[]`      | required | this chapter  |
| `<assign>`    | `assign[]`      | required | this chapter  |
| `<raise>`     | `raise[]`       | required | this chapter  |
| `<script>`    | `script[]`      | required | this chapter  |
| `<parallel>`  | `parallel[]`    | deferred | future QT-05x |
| `<final>`     | `final[]`       | deferred | future QT-05x |
| `<initial>`   | `initial`       | deferred | future QT-05x |
| `<history>`   | `history[]`     | deferred | future QT-05x |
| `<invoke>`    | `invoke[]`      | deferred | future QT-05x |
| `<send>`      | `send[]`        | deferred | future QT-05x |
| `<cancel>`    | `cancel[]`      | deferred | future QT-05x |
| `<log>`       | `log[]`         | deferred | future QT-05x |
| `<if>`/`<elseif>`/`<else>` | `if[]` | deferred | future QT-05x |
| `<foreach>`   | `foreach[]`     | deferred | future QT-05x |
| `<param>`     | `param[]`       | deferred | future QT-05x |
| `<finalize>`  | `finalize`      | deferred | future QT-05x |
| `<donedata>`  | `donedata`      | deferred | future QT-05x |
| `<content>`   | `content`       | deferred | future QT-05x |

Deferred elements **MAY** still appear in scjson files we ingest
(istate accepts them) — we just don't lower them into the IR.
Ingest **MUST** preserve them as opaque text in the side-file so
the round-trip through istate-codegen is faithful; emit-scjson
(QT-05d) **MUST NOT** synthesize them.

## §6 — Frozen Decision: istate Linkage Surface (v1)

Registration policy: **Standards Action**. Adding, removing, or
changing the shape of any of these symbols requires (a) an
amendment to this chapter, (b) a bump of `ISTATE_LINKAGE_VERSION`,
and (c) a corresponding change-log entry recording the istate
template git hash that introduced the change.

| Symbol                                 | Shape                                                                                              | How rlvgl-emitted code uses it                       |
| -------------------------------------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `<sm>_gen::Machine`                    | `pub struct Machine { state, dm, queue, internal_events, log_to_stderr, externals }`               | Owned via `Rc<RefCell<Machine>>` next to `ScreenState`. |
| `<sm>_gen::Machine::new()` / `with_options(internal: bool, log: bool)` | `pub fn`                                                          | Construct in `build_screen`. `with_options` used when QT-05e plugs custom externals. |
| `<sm>_gen::Machine::dispatch(&mut self, ev: Event) -> bool` | `pub fn`                                                                       | Only mutation entry point we call (QT-05b).         |
| `<sm>_gen::Event`                      | `pub enum Event { … }` — variants PascalCased per istate template `to_rust_ident \| capitalize`. | Constructed at the call site; never matched exhaustively. |
| `<sm>_gen::State`                      | `pub enum State { … }` — same naming rule.                                                         | Read via `machine.borrow().state == State::<Id>`; never matched exhaustively. |
| `<sm>_gen::DataModel`                  | `pub struct DataModel { pub <var>: f64, … }` — all `f64` per current istate scaffold.              | Read-only `machine.borrow().dm.<var>` (QT-05c).     |
| `<sm>_gen::Externals` (trait) + `DefaultExternals` (struct) | `pub trait Externals { fn <callout>(&mut self, m: &mut Machine); … }` + default no-op impl | QT-05e emits a sibling `ScreenExternals` impl and installs it via `Machine::with_options`. |

**Linkage v1 profile**: the istate Rust template currently uses
`std::collections::VecDeque` for the internal event queue and
`Box<dyn Externals>` for callout dispatch. v1 therefore requires
`std` on the consumer crate. Embedded targets that need `no_std`
SM linkage are blocked on an upstream istate `no_std` SM profile
(comparable to istate's existing `streamz_rust_sync` profile, but
for state machines). When that lands, this chapter is amended and
`ISTATE_LINKAGE_VERSION` becomes 2; rlvgl-emitted code does not
need to change for `std` consumers.

`ISTATE_LINKAGE_VERSION = 1` **MUST** be emitted as a `pub const`
at the top of every QT-05-touching generated module so reviewers
can confirm the version their code is built against.

**Out of scope for v1**: `Machine.queue` (private field —
`raise_name` is an istate-internal helper, not part of the linkage
surface), `event_from_name` (free function, internal), `effect_t<i>`
/ `guard_t<i>` / `on_entry_<id>` / `on_exit_<id>` (free functions,
internal). rlvgl-emitted code **MUST NOT** call these directly.

## §7 — Frozen Decision: File Layout

| Concern                                  | Path (relative to consumer crate root)              |
| ---------------------------------------- | --------------------------------------------------- |
| scjson side-file authored by user        | `<screen>.scjson` (next to `<screen>.qml`)          |
| scjson emitted by QT-05d                 | same path; QT-05d overwrites; user-edited blocks preserved via `_comment` round-trip |
| `<sm>_gen/` crate (istate-codegen output) | `crates/<sm>_gen/` (default; overridable per QT-05a flag) |
| Consumer `Cargo.toml` path dep           | `<sm>_gen = { path = "crates/<sm>_gen" }`           |
| `ScreenExternals` impl (QT-05e)          | `src/<screen>_externals.rs`                         |
| Submodule reference in this repo         | `vendor/scjson/` (Cargo workspace `exclude`-listed) |

The directory `crates/<sm>_gen/` is **not** itself written by
`rlvgl-creator`. QT-05a documents two paths to populate it:

1. **Manual**: user runs istate-codegen via the softoboros MCP
   (e.g. `mcp__softoboros__istate_codegen_create`), downloads the
   zip via `mcp__softoboros__istate_codegen_download`, and unzips
   into `crates/<sm>_gen/`.
2. **Automated** (future, QT-05a §X amendment): `rlvgl-creator qt
   link-sm --with-codegen` invokes the MCP from the host, downloads,
   unzips. Default off; opt-in.

The vendored hand-checked-in `<sm>_gen/` used by tests is path #1
applied once at fixture-creation time.

## §8 — Versioning

| Constant                       | Before QT-05 | After QT-05 (this chapter) | Owns next bump |
| ------------------------------ | ------------ | -------------------------- | -------------- |
| `QT_IR_VERSION`                | 1            | 2 (adds `state_machine` field on `UiModule`) | QT-05a (adds nested IR), QT-02 (next freeze) |
| `QT_EMIT_VERSION_RLVGL`        | 11           | unchanged (concepts only — emit changes happen in QT-05b/c/e) | QT-05b → 12 |
| `QT_EMIT_VERSION_DATA`         | 1            | unchanged                  | future amendment |
| `ISTATE_LINKAGE_VERSION`       | (new)        | 1                          | upstream istate template change |

`QT_IR_VERSION` bumps once and only once at QT-05 — the IR shape
gains `state_machine: Option<UiStateMachine>` as a single additive
field. Subsequent QT-05a-e chapters extend the *content* of that
field (populated by ingest, consumed by emit) but do not bump
`QT_IR_VERSION` again because no new top-level shape lands.

## §9 — Non-Goals

- **No on-device istate runtime**, just like there is no on-device
  PySide6. The state-machine engine is a host-codegen output —
  device-side it is plain Rust.
- **No SCXML semantics enforced at the rlvgl-creator layer.**
  Guards, parallel regions, history, microstep ordering — all
  istate's job. We pass scjson through unchanged.
- **No Cargo dep on the `scjson` crate.** Wire-compat is
  cross-checked via a dev-only tool invocation against the upstream
  CLI built from the submodule. Generated `<sm>_gen/` is the only
  Rust artifact in our build graph that touches the SM world.
- **No bundled istate Python.** Codegen runs out-of-process via the
  softoboros MCP. Users without softoboros can hand-author
  `<sm>_gen/` by hand or skip QT-05 features entirely.
- **No vendoring of `scjson_props.rs`.** A 10-element hand-rolled
  subset suffices for QT-05a's ingest and QT-05d's emit. Adding an
  element is cheaper than maintaining the full upstream surface.
- **No `<sm>_gen/` regeneration on every `cargo build`.** That
  crate is checked into the consumer repo. Regeneration is a
  user-driven step, optionally automated by QT-05a's
  `--with-codegen` flag (deferred sub-amendment).
- **No type widening for `DataModel`.** v1 mirrors istate's
  current `f64`-only scaffold. Bool/string/enum support unblocks
  when istate's template grows them; tracked via
  `ISTATE_LINKAGE_VERSION`.

## §10 — Reconciliation with Adjacent Phases

| Phase    | Concern                                                   | Resolution                                                                                                          |
| -------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| QT-00    | Vocabulary, phase enumeration.                            | QT-05 added under Standards Action. Letters `a`-`e` reserved per QT-00 §5 lettered-suffix rule.                   |
| QT-02    | IR schema freeze (JSON Schema export).                    | `state_machine` field added to `UiModule`; QT-02's `qt-ir.schema.json` regenerated. Schema versioning policy unchanged. |
| QT-03 family | Widget tree, anchor resolver.                         | Independent. State machines do not produce widgets; they drive widgets that QT-03 already builds.                   |
| QT-04    | Signal handlers (`onClicked`).                            | QT-05b extends handler lowering: when a state-machine is linked, an `onClicked` body containing a bare event name (`go`, `start`, …) lowers to `machine.borrow_mut().dispatch(<sm>_gen::Event::Go)`. |
| QT-04b   | `build_screen` 2-tuple → 3-tuple (added `Vec<LabelBinding>`). | QT-05b extends to a 4-tuple: `(WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<Binding>)`. The `LabelBinding` type stays; `Vec<Binding>` becomes a sealed enum (`Binding::Label(LabelBinding)` / `Binding::Machine(MachineBinding)`) at QT-05c. Recorded in QT-04b §15 once QT-05b lands. |
| QT-04c   | Initial-value text bindings (build-time read).            | QT-05c MachineBinding reads from `machine.borrow().dm.<field>` at refresh time; build-time read shape unchanged.   |
| QT-04e   | Reactive refresh (`refresh_bindings`).                    | QT-05c plumbs `MachineBinding` into the same refresh pump. `refresh_bindings(&state, &machine, &bindings)` — gains a `&Rc<RefCell<Machine>>` parameter when at least one MachineBinding exists. Recorded in QT-04e §15 once QT-05c lands. |
| QT-04f   | Nested ID resolution.                                     | Independent. State machine IDs and widget IDs share no namespace.                                                  |
| QT-08    | Directory-mode CLI.                                       | QT-05a's side-file discovery slots into the existing directory walker without new flags; `<x>.qml` → `<x>.scjson`. |

## §11 — Acceptance Checklist

QT-05 (concepts only) is **ratified** when:

- [x] `vendor/scjson/` git submodule exists and points at
      `https://github.com/SoftOboros/scjson.git`.
- [x] `Cargo.toml` workspace excludes `vendor/scjson`.
- [x] §3 names every IR type and every `// QT-05` marker.
- [x] §5 freezes the scjson element subset.
- [x] §6 freezes the 6-symbol istate linkage surface and pins
      `ISTATE_LINKAGE_VERSION = 1` to the std-profile istate
      template.
- [x] §7 freezes the file layout.
- [x] §8 records the IR version bump (`1 → 2`) and the linkage
      version introduction.
- [x] `UiStateMachine`, `UiState`, `UiTransition`, `UiDmField`,
      `UiScript`, `UiScriptOrigin` exist as public IR types in
      `src/bin/creator/qt.rs` (or a dedicated submodule).
- [x] `qt-ir.schema.json` regenerated to include the new
      `state_machine` field.
- [x] `QT_IR_VERSION` updated to `2` in the emitter source and
      reflected in goldens (no fixture has a state machine yet, so
      goldens regenerate idempotently — no behavioural change).
- [x] A serde round-trip smoke test for `UiStateMachine` lands.
- [x] §15 carries a dated initial change-log entry.
- [x] README.md and 00-concepts.md amended with QT-05 status and
      change-log entries.

QT-05 implementation gates (QT-05a-e) carry their own checklists.

## §12 — Files Cited

- [`CLAUDE.md`](../../CLAUDE.md) — spec-before-code planning discipline.
- [`docs/qt-support/00-concepts.md`](./00-concepts.md) — vocabulary authority.
- [`docs/qt-support/04b-properties-bindings.md`](./04b-properties-bindings.md) — 3-tuple return amended by QT-05b.
- [`docs/qt-support/04e-reactive-bindings.md`](./04e-reactive-bindings.md) — refresh pump extended by QT-05c.
- [`docs/qt-support/08-multi-file-cli.md`](./08-multi-file-cli.md) — directory-mode discovery shared with QT-05a.
- [`vendor/scjson/`](../../vendor/scjson) — upstream scjson submodule (BSD-1-Clause).
- [`vendor/scjson/scjson.schema.json`](../../vendor/scjson/scjson.schema.json) — on-disk wire format.
- [`vendor/scjson/rust/src/scjson_props.rs`](../../vendor/scjson/rust/src/scjson_props.rs) — Rust type names + serde rename rules.
- [`vendor/scjson/py/scjson/templates/scjson_props.rs.jinja2`](../../vendor/scjson/py/scjson/templates/scjson_props.rs.jinja2) — meta-template; field naming source of truth.
- `softoboros/backend/templates/codegen/rust/src/lib.rs.jinja2` — istate Rust template (read as prior knowledge for §6).
- `softoboros/backend/templates/codegen/rust/Cargo.toml.jinja2` — `<sm>_gen` Cargo shape.
- [`src/bin/creator/qt.rs`](../../src/bin/creator/qt.rs) — emitter implementation site.
- [`src/bin/creator/qt_scjson.rs`](../../src/bin/creator/qt_scjson.rs) — hand-rolled scjson subset (added by this chapter).
- [`schemas/qt-ir.schema.json`](../../schemas/qt-ir.schema.json) — regenerated for the IR bump.
- [`tests/fixtures/qt/`](../../tests/fixtures/qt/) — canonical fixtures; QT-05a-e add `stopwatch.qml` etc.

## §13 — Unblocks

Ratifying QT-05 unblocks:

- **QT-05a** (scjson side-file ingest): the IR types, file layout,
  and submodule are all in place.
- **QT-05b** (handler dispatch glue): the linkage surface is
  pinned, so emit can target it without ambiguity.
- **QT-05c** (DM/State bindings): the QT-04e refresh pump's
  extension contract is named in §10.
- **QT-05d** (QML → scjson emit): the scjson element subset is
  frozen, so the QML parser knows which forms to surface.
- **QT-05e** (Externals stub emission): the linkage surface names
  the `Externals` trait and `DefaultExternals` struct as the slots
  it plugs into.

## §14 — Files Cited

(see [§12](#12--files-cited))

## §15 — Change Log

| Date       | Change                                                                          |
| ---------- | ------------------------------------------------------------------------------- |
| 2026-04-29 | §3 amendment (same-day): `UiAction` enum definition added (was referenced by `UiState.on_entry` / `UiState.on_exit` / `UiTransition.actions` but never defined inline). Variants: `Assign`/`Raise`/`Script`. Promoted from implicit to explicit; no implementation impact since QT-05a-e are unimplemented. `UiStateMachine.source` typed as `String` on the wire (was `PathBuf` in the §3 sketch) to keep `schemars::JsonSchema` derive trivial. |
| 2026-04-29 | QT-05 ratified (concepts only). `vendor/scjson/` submodule added (`https://github.com/SoftOboros/scjson.git`, BSD-1-Clause, reference-only — not a Cargo dep). 6-symbol istate linkage surface frozen under Standards Action: `Machine`, `Machine::new`/`with_options`, `Machine::dispatch`, `Event`, `State`, `DataModel`, `Externals`+`DefaultExternals`. `ISTATE_LINKAGE_VERSION = 1` introduced and pinned to istate's std-profile Rust template (`backend/templates/codegen/rust/src/lib.rs.jinja2`); `no_std` linkage reserved for v2 once an upstream `no_std` SM profile lands. scjson element subset frozen under Specification Required: `<scxml>`, `<state>`, `<transition>`, `<datamodel>`, `<data>`, `<onentry>`, `<onexit>`, `<assign>`, `<raise>`, `<script>`. IR types added: `UiStateMachine`, `UiState`, `UiTransition`, `UiDmField`, `UiScript`, `UiScriptOrigin`. `UiModule` gains `state_machine: Option<UiStateMachine>` as an additive field. `QT_IR_VERSION` bumped `1 → 2`. `QT_EMIT_VERSION_RLVGL` unchanged (concepts only — emit bumps to 12 land in QT-05b). `QT_EMIT_VERSION_DATA` unchanged. `// QT-05a/b/c/d/e` marker prefixes reserved. File layout frozen: `<screen>.scjson` side-files, `crates/<sm>_gen/` istate output, `src/<screen>_externals.rs` for QT-05e stubs. Hand-rolled scjson subset lives at `src/bin/creator/qt_scjson.rs`. QT-05a-e remain to be ratified in their own chapters; their dependency on this chapter is recorded in their §10 reconciliation tables when they land. |

---

MIT-licensed: MIT.
