// SPDX-License-Identifier: MIT
//! Qt / QML ingestion for rlvgl-creator.
//!
//! Parses the structural subset of `.qml` (imports, types, properties,
//! signal declarations, signal handlers, child items) and emits a
//! versioned `qt-ir.json` IR. JavaScript expressions on the RHS of
//! property assignments and signal handler bodies are captured as
//! opaque strings — the IR is structural, not semantic.
//!
//! MVP scope (phase QT-01a):
//! - One `.qml` file in, one `qt-ir.json` out.
//! - No external tooling required (no Qt install, no PySide6, no
//!   `qmlplugindump`). See `docs/creator/QT-INGEST.md`.
//!
//! Out of scope here: type introspection, binding evaluation, state
//! machines, attached-property semantics, JS function bodies. Those
//! land in later QT-NN chapters.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::qt_scjson;

/// Current `qt-ir` schema version. Bumping is a Specification-Required
/// event under the spec-before-code discipline (see `CLAUDE.md`).
///
/// QT-05 (2026-04-29) bumped this `1 → 2` to accommodate the additive
/// `state_machine: Option<UiStateMachine>` field on `UiModule`. See
/// `docs/qt-support/05-state-machines.md` §8.
pub const QT_IR_VERSION: u32 = 2;

/// Canonical `$id` for the emitted JSON Schema. Owned by phase QT-02
/// (see `docs/qt-support/02-ir-schema.md`).
pub const QT_IR_SCHEMA_ID: &str = "https://rlvgl.dev/schemas/qt-ir.schema.json";

/// Canonical `$comment` written into the emitted schema. Names the
/// regen command so reviewers do not have to hunt for it.
pub const QT_IR_SCHEMA_COMMENT: &str = "schemas/qt-ir.schema.json - rlvgl-creator qt-ir IR. \
     Regenerate with `rlvgl-creator qt schema --out schemas/qt-ir.schema.json`. \
     See docs/qt-support/02-ir-schema.md for the bumping policy.";

// ============================================================================
// IR types
// ============================================================================

/// Top-level QML module — the parsed shape of a single `.qml` file.
///
/// QT-05 amendment: the optional `state_machine` field carries a
/// linked scjson side-file once QT-05a ingest lands. `None` for any
/// QML that does not declare a state machine; never written by
/// QT-05 itself (concepts only).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiModule {
    pub version: u32,
    pub source: String,
    pub imports: Vec<UiImport>,
    pub pragmas: Vec<String>,
    pub root: UiItem,
    /// QT-05: linked scjson state-machine, populated by QT-05a ingest.
    /// Always `None` from the QT-01a structural parser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_machine: Option<UiStateMachine>,
}

/// `import QtQuick 2.15 as Q` style declaration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiImport {
    pub module: String,
    pub version: Option<String>,
    pub alias: Option<String>,
}

/// A QML type instance — `Item { ... }`, `Rectangle { ... }`, etc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct UiItem {
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub properties: Vec<UiProperty>,
    pub assignments: Vec<UiAssignment>,
    pub signals: Vec<UiSignal>,
    pub handlers: Vec<UiHandler>,
    pub children: Vec<UiItem>,
}

/// `[default] [readonly] property <ty> <name>[: <expr>]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiProperty {
    pub name: String,
    pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    pub readonly: bool,
    pub default_kw: bool,
}

/// `target: <expression-or-object-or-list>` and dotted variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiAssignment {
    pub target: String,
    pub value: UiAssignmentValue,
}

/// Value side of an assignment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAssignmentValue {
    /// Plain expression text, opaque (e.g. `parent.width * 0.5`).
    Expression { text: String },
    /// `target: SomeType { ... }` — value is a sub-item.
    Object { item: Box<UiItem> },
    /// `target: [Item {}, Item {}, ...]`.
    List { items: Vec<UiAssignmentValue> },
}

/// `signal pressed(int x, int y)` declaration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiSignal {
    pub name: String,
    pub params: Vec<UiSignalParam>,
}

/// Single `(int x)` style signal parameter.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiSignalParam {
    pub name: String,
    pub ty: String,
}

/// `onClicked: ...` signal-handler binding. Body captured raw.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiHandler {
    pub signal: String,
    pub body: String,
}

// ============================================================================
// QT-05: state-machine IR types
//
// These mirror `docs/qt-support/05-state-machines.md` §3 and are
// **populated by QT-05a-e**, not by the QT-01a structural parser. The
// QT-05 (this chapter) seed adds only the type shapes so subsequent
// chapters have a stable IR target.
// ============================================================================

/// QT-05 §3 — Qt-side state-machine record. Lives next to
/// [`UiModule.root`] and links to a `<screen>.scjson` side-file plus
/// a pre-walked enumeration of states/transitions/dm/scripts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiStateMachine {
    /// Rust crate name stem; the istate-codegen Rust crate is
    /// `<id>_gen` per QT-05 §6.
    pub id: String,
    /// Path to the scjson side-file, relative to the QML project
    /// root. Stored as `String` on the wire (see QT-05 §3 amendment
    /// 2026-04-29) — ingest treats it as a path.
    pub source: String,
    /// Initial state ID; mirrors scjson `<scxml initial="…">`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
    pub states: Vec<UiState>,
    pub transitions: Vec<UiTransition>,
    pub datamodel: Vec<UiDmField>,
    pub scripts: Vec<UiScript>,
}

/// QT-05 §3 — flat state record. Compound nesting and parallel
/// regions are deferred per QT-05 §5 (covered by future QT-05x).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiState {
    /// State ID; PascalCased to `<sm>_gen::State::<Id>` per the
    /// istate template's `to_rust_ident | capitalize` rule.
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_entry: Vec<UiAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_exit: Vec<UiAction>,
}

/// QT-05 §3 — transition record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiTransition {
    pub source: String,
    /// PascalCased to `<sm>_gen::Event::<Name>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Raw scjson `cond` expression. Parsed by istate-codegen, not
    /// by us — we pass it through verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cond: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<UiAction>,
}

/// QT-05 §3 — DataModel field. v1 mirrors istate's `f64`-only
/// scaffold; type widening is reserved for future linkage versions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiDmField {
    /// snake_case Rust ident exposed at `<sm>_gen::DataModel.<id>`.
    pub id: String,
    /// Numeric literal initializer; `None` = default 0.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<f64>,
}

/// QT-05 §3 — discovered `<script name="…"/>` callout. Method name
/// on the generated `<sm>_gen::Externals` trait.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct UiScript {
    pub name: String,
    /// Where the script lived in the SCXML tree — used by QT-05e
    /// when emitting the `// QT-05e externals-stub:` comment for
    /// reviewer context.
    pub origin: UiScriptOrigin,
}

/// QT-05 §3 — provenance of a discovered `<script>` callout.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiScriptOrigin {
    Transition {
        index: u32,
        from: String,
        to: Option<String>,
    },
    OnEntry {
        state: String,
    },
    OnExit {
        state: String,
    },
}

/// QT-05 §3 — sealed enum over the scjson executable-content
/// elements in the QT-05 §5 subset that contribute to entry/exit/
/// transition action lists. Adding a variant requires a
/// Specification-Required amendment to QT-05 §5 + this glossary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAction {
    /// `<assign location="…" expr="…"/>` — write to a `DataModel`
    /// field.
    Assign {
        location: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expr: Option<String>,
    },
    /// `<raise event="…"/>` — internal-event raise.
    Raise { event: String },
    /// `<script name="…"/>` — callout to an `Externals` method.
    /// `name` references back into `UiStateMachine.scripts[*].name`.
    Script { name: String },
}

// ============================================================================
// QT-05d: QML `states:` / `transitions:` → scjson emission
// (`docs/qt-support/05d-emit-scjson.md`).
// ============================================================================

/// QT-05d §6 — pure walker. Consumes a `UiItem` (typically the
/// QML root) and returns `Some(Scxml)` if the item carries inline
/// `states:` / `transitions:` blocks per the §5 idiom, or `None`
/// otherwise. Emit-time errors (missing `name`/`from`/`to`,
/// unknown transition target, multiple `initial: true`) bubble
/// out as `Result::Err`.
fn walk_qml_state_machine(item: &UiItem, source: &str) -> Result<Option<qt_scjson::Scxml>> {
    use serde_json::Value;

    let states_assignment = item.assignments.iter().find(|a| a.target == "states");
    let transitions_assignment = item.assignments.iter().find(|a| a.target == "transitions");

    if states_assignment.is_none() && transitions_assignment.is_none() {
        return Ok(None);
    }

    // Step 2 — collect <state> entries from `states: […]`.
    let mut states: Vec<qt_scjson::State> = Vec::new();
    let mut initial: Option<String> = None;
    if let Some(asn) = states_assignment {
        for inner in iter_object_items(&asn.value, "State") {
            let name = lookup_assignment(inner, "name")
                .and_then(parse_string_literal)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "QT-05d §6: `State` block in {source} has no literal-string `name:` \
                         attribute (per QT-05d §5)"
                    )
                })?;
            let is_initial = lookup_assignment(inner, "initial")
                .map(|s| s.trim() == "true")
                .unwrap_or(false);
            if is_initial {
                if initial.is_some() {
                    bail!(
                        "QT-05d §6 / §5: at most one `State {{ … initial: true }}` permitted; \
                         {source} has multiple"
                    );
                }
                initial = Some(name.clone());
            }
            states.push(qt_scjson::State {
                id: Some(name),
                ..qt_scjson::State::default()
            });
        }
    }

    // Step 4 — distribute Transition entries onto matching states.
    if let Some(asn) = transitions_assignment {
        for inner in iter_object_items(&asn.value, "Transition") {
            let from = lookup_assignment(inner, "from")
                .and_then(parse_string_literal)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "QT-05d §6: `Transition` block in {source} has no literal-string \
                         `from:` (per QT-05d §5)"
                    )
                })?;
            let to = lookup_assignment(inner, "to")
                .and_then(parse_string_literal)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "QT-05d §6: `Transition` block in {source} has no literal-string \
                         `to:` (per QT-05d §5)"
                    )
                })?;
            let event = lookup_assignment(inner, "event").and_then(parse_string_literal);
            let host = states.iter_mut().find(|s| s.id.as_deref() == Some(from.as_str()))
                .ok_or_else(|| anyhow::anyhow!(
                    "QT-05d §6: `Transition` in {source} references unknown source state `{from}` (no matching `State {{ name: \"{from}\" }}`)"
                ))?;
            host.transition.push(qt_scjson::Transition {
                event,
                target: vec![to],
                ..qt_scjson::Transition::default()
            });
        }
    }

    // Step 5 — assemble the Scxml document.
    let mut other_attributes = serde_json::Map::new();
    other_attributes.insert(
        "_comment".to_string(),
        Value::String(format!("QT-05d emit-scjson: {source}")),
    );
    let scxml = qt_scjson::Scxml {
        state: states,
        initial: initial.into_iter().collect(),
        other_attributes,
        ..qt_scjson::Scxml::default()
    };
    Ok(Some(scxml))
}

/// Iterate over the `Object` items under an `UiAssignmentValue`
/// whose nested `type_name` matches `expected`. Skips entries with
/// other shapes (non-Object inside a List, or wrong type_name).
fn iter_object_items<'a>(value: &'a UiAssignmentValue, expected: &str) -> Vec<&'a UiItem> {
    match value {
        UiAssignmentValue::Object { item } if item.type_name == expected => vec![item.as_ref()],
        UiAssignmentValue::List { items } => items
            .iter()
            .filter_map(|v| match v {
                UiAssignmentValue::Object { item } if item.type_name == expected => {
                    Some(item.as_ref())
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// QT-05d §7 — `qt emit-scjson <input> [<out>]` entry point.
/// File mode + directory mode per the QT-08 walker convention.
pub(crate) fn emit_scjson(input: &Path, out: Option<&Path>) -> Result<()> {
    if input.is_dir() {
        let out_dir = out.unwrap_or(input);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output dir {}", out_dir.display()))?;
        for qml in qt08_collect_qml_files(input)? {
            emit_scjson_one(&qml, &resolve_scjson_out_for(&qml, out_dir))?;
        }
        Ok(())
    } else {
        let out_path = match out {
            Some(p) if p.is_dir() => resolve_scjson_out_for(input, p),
            Some(p) => p.to_path_buf(),
            None => resolve_scjson_out_for(input, input.parent().unwrap_or_else(|| Path::new("."))),
        };
        emit_scjson_one(input, &out_path)
    }
}

fn resolve_scjson_out_for(qml: &Path, out_dir: &Path) -> PathBuf {
    let stem = qml
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    out_dir.join(format!("{stem}.scjson"))
}

fn emit_scjson_one(input: &Path, out_path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    let source_label = input.display().to_string();
    let Some(scxml) = walk_qml_state_machine(&module.root, &source_label)? else {
        // No states:/transitions: blocks — silent skip per QT-05d §7.
        return Ok(());
    };
    let json = serde_json::to_string_pretty(&scxml)
        .with_context(|| format!("serialising scjson for {}", input.display()))?;
    let mut json = json;
    json.push('\n');
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(out_path, json).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ============================================================================
// QT-05e: Externals stub emission (`docs/qt-support/05e-externals-stubs.md`).
// ============================================================================

/// QT-05e per-file emit-shape version. Bumps when the
/// `<basename>_externals.rs` shape changes (e.g. a future stateful-
/// externals amendment).
pub const QT_EXTERNALS_VERSION: u32 = 1;

/// QT-05e §5 — `qt emit-externals <input> [<out>]` entry point.
/// File mode + directory mode per QT-08 directory walker.
pub(crate) fn emit_externals(input: &Path, out: Option<&Path>) -> Result<()> {
    if input.is_dir() {
        let out_dir = out.unwrap_or(input);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output dir {}", out_dir.display()))?;
        for qml in qt08_collect_qml_files(input)? {
            emit_externals_one(&qml, &resolve_externals_out_for(&qml, out_dir))?;
        }
        Ok(())
    } else {
        let out_path = match out {
            Some(p) if p.is_dir() => resolve_externals_out_for(input, p),
            Some(p) => p.to_path_buf(),
            None => {
                resolve_externals_out_for(input, input.parent().unwrap_or_else(|| Path::new(".")))
            }
        };
        emit_externals_one(input, &out_path)
    }
}

fn resolve_externals_out_for(qml: &Path, out_dir: &Path) -> PathBuf {
    let stem = qml
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    out_dir.join(format!("{stem}_externals.rs"))
}

fn emit_externals_one(input: &Path, out_path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let mut module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    attach_scjson_side_file(&mut module, input)?;
    let Some(sm) = module.state_machine.as_ref() else {
        // QT-05e §5 silent skip: no SM attached.
        return Ok(());
    };
    if sm.scripts.is_empty() {
        // QT-05e §5 silent skip: SM attached but no scripts.
        return Ok(());
    }
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("input has no usable file stem: {}", input.display()))?;
    let rust = render_externals(&sm.id, sm, stem, &input.display().to_string());
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(out_path, rust).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

/// QT-05e §6 — render the externals impl for one screen.
pub fn render_externals(
    sm_id: &str,
    sm: &UiStateMachine,
    qml_stem: &str,
    qml_source: &str,
) -> String {
    let mut out = String::new();
    out.push_str("// SPDX-License-Identifier: MIT\n");
    out.push_str("//\n");
    out.push_str(&format!(
        "// Generated by `rlvgl-creator qt emit-externals` from `{qml_source}`.\n"
    ));
    out.push_str("// Hand-edit the method bodies below; regeneration\n");
    out.push_str("// overwrites this file. See QT-05e §9.\n");
    out.push_str("//\n");
    out.push_str("// Emit-shape contract: docs/qt-support/05e-externals-stubs.md\n");
    out.push_str("// Install path (QT-05e §7):\n");
    out.push_str(&format!(
        "//   let (_, _, machine, _) = build_screen(bounds);\n//   machine.borrow_mut().externals = Box::new({qml_stem}_externals::ScreenExternals::new());\n",
    ));
    out.push('\n');
    out.push_str("#![allow(dead_code)]\n");
    out.push_str("#![allow(unused_variables)]\n");
    out.push('\n');
    out.push_str(&format!("use {sm_id}_gen::{{Externals, Machine}};\n\n"));
    out.push_str(&format!(
        "/// QT-05e per-file emit-shape version.\npub const QT_EXTERNALS_VERSION: u32 = {QT_EXTERNALS_VERSION};\n\n"
    ));
    out.push_str("/// QT-05e §3 — `pub struct ScreenExternals`. v1 is\n");
    out.push_str("/// stateless; the user adds fields by hand.\n");
    out.push_str("pub struct ScreenExternals;\n\n");
    out.push_str("impl ScreenExternals {\n    pub fn new() -> Self {\n        Self\n    }\n}\n\n");
    out.push_str("impl Default for ScreenExternals {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n\n");
    out.push_str(&format!("impl Externals for ScreenExternals {{\n"));
    let mut first = true;
    for script in &sm.scripts {
        if !first {
            out.push('\n');
        }
        first = false;
        let origin = render_script_origin(&script.origin);
        out.push_str(&format!(
            "    fn {name}(&mut self, m: &mut Machine) {{\n        \
             // QT-05e externals-stub: {name} from {origin}\n        \
             // TODO — fill in side-effect code.\n        \
             let _ = m;\n    \
             }}\n",
            name = script.name,
            origin = origin,
        ));
    }
    out.push_str("}\n");
    format!("{}\n", out.trim_end())
}

fn render_script_origin(origin: &UiScriptOrigin) -> String {
    match origin {
        UiScriptOrigin::Transition { index, from, to } => format!(
            "Transition {{ index: {index}, from: \"{from}\", to: {} }}",
            to.as_deref()
                .map(|s| format!("Some(\"{s}\")"))
                .unwrap_or_else(|| "None".to_string())
        ),
        UiScriptOrigin::OnEntry { state } => format!("OnEntry {{ state: \"{state}\" }}"),
        UiScriptOrigin::OnExit { state } => format!("OnExit {{ state: \"{state}\" }}"),
    }
}

// ============================================================================
// QT-06: theme-token emission (`docs/qt-support/06-theme-tokens.md`).
// ============================================================================

/// QT-06 intermediate. `BTreeMap` for deterministic lexical key
/// order at YAML emission per §6.
#[derive(Debug, Default)]
pub struct TokenSet {
    pub colors: std::collections::BTreeMap<String, String>,
    pub spacing: std::collections::BTreeMap<String, i64>,
    pub radii: std::collections::BTreeMap<String, i64>,
    pub fonts: std::collections::BTreeMap<String, String>,
    pub dark_colors: std::collections::BTreeMap<String, String>,
}

impl TokenSet {
    fn is_empty(&self) -> bool {
        self.colors.is_empty()
            && self.spacing.is_empty()
            && self.radii.is_empty()
            && self.fonts.is_empty()
            && self.dark_colors.is_empty()
    }
}

/// QT-06 §3 / §6 — pure walker. Returns `None` if the item carries
/// no recognised theme properties.
pub fn walk_theme_module(item: &UiItem) -> Option<TokenSet> {
    let mut ts = TokenSet::default();
    for prop in &item.properties {
        let Some(default) = prop.default_value.as_deref() else {
            continue;
        };
        match prop.ty.as_str() {
            "color" => {
                if let Some(hex) = parse_hex_color_lit(default) {
                    if let Some(stem) = prop.name.strip_suffix("_dark") {
                        ts.dark_colors.insert(stem.to_string(), hex);
                    } else {
                        ts.colors.insert(prop.name.clone(), hex);
                    }
                }
            }
            "int" => {
                if let Some(v) = parse_int_literal_i64(default) {
                    if let Some(key) = prop.name.strip_prefix("spacing_") {
                        ts.spacing.insert(key.to_string(), v);
                    } else if let Some(key) = prop.name.strip_prefix("radius_") {
                        ts.radii.insert(key.to_string(), v);
                    }
                }
            }
            "string" => {
                if let Some(s) = parse_string_literal(default) {
                    if let Some(key) = prop.name.strip_prefix("font_") {
                        ts.fonts.insert(key.to_string(), s);
                    }
                }
            }
            _ => {}
        }
    }
    if ts.is_empty() { None } else { Some(ts) }
}

/// QT-06 §6 — accept `#rgb`, `#rrggbb`, `#rrggbbaa` (case-insensitive).
/// All other forms (rgba(), named colors) silently dropped.
fn parse_hex_color_lit(expr: &str) -> Option<String> {
    let s = parse_string_literal(expr)?;
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return None;
    }
    let hex = &bytes[1..];
    if !matches!(hex.len(), 3 | 4 | 6 | 8) {
        return None;
    }
    if !hex.iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(s)
}

fn parse_int_literal_i64(expr: &str) -> Option<i64> {
    expr.trim().parse::<i64>().ok()
}

/// QT-06 §3 / §6 — render a `tokens.yaml` document from a populated
/// `TokenSet`. Output is byte-stable: lexical key order in every
/// section, no trailing whitespace, single trailing newline.
pub fn render_tokens_yaml(theme: &TokenSet, qml_source: &str) -> String {
    let mut out = String::new();
    out.push_str("# Auto-generated from Qt theme by rlvgl-creator (QT-06)\n");
    out.push_str(&format!("# QT-06 theme: {qml_source}\n"));
    out.push_str("version: 1\n");
    out.push_str("colors:\n");
    for (k, v) in &theme.colors {
        out.push_str(&format!("  {k}: \"{v}\"\n"));
    }
    out.push_str("spacing:\n");
    for (k, v) in &theme.spacing {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push_str("radii:\n");
    for (k, v) in &theme.radii {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push_str("fonts:\n");
    for (k, v) in &theme.fonts {
        out.push_str(&format!("  {k}: \"{v}\"\n"));
    }
    if !theme.dark_colors.is_empty() {
        out.push_str("modes:\n  dark:\n    colors:\n");
        for (k, v) in &theme.dark_colors {
            out.push_str(&format!("      {k}: \"{v}\"\n"));
        }
    }
    out
}

/// QT-06 §7 — `qt emit-tokens <input> [<out>]` entry point.
pub(crate) fn emit_tokens(input: &Path, out: Option<&Path>) -> Result<()> {
    if input.is_dir() {
        let out_dir = out.unwrap_or(input);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output dir {}", out_dir.display()))?;
        for qml in qt08_collect_qml_files(input)? {
            emit_tokens_one(&qml, &resolve_tokens_out_for(&qml, out_dir))?;
        }
        Ok(())
    } else {
        let out_path = match out {
            Some(p) if p.is_dir() => resolve_tokens_out_for(input, p),
            Some(p) => p.to_path_buf(),
            None => resolve_tokens_out_for(input, input.parent().unwrap_or_else(|| Path::new("."))),
        };
        emit_tokens_one(input, &out_path)
    }
}

fn resolve_tokens_out_for(qml: &Path, out_dir: &Path) -> PathBuf {
    let stem = qml
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    out_dir.join(format!("{stem}.tokens.yaml"))
}

fn emit_tokens_one(input: &Path, out_path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    let Some(theme) = walk_theme_module(&module.root) else {
        // No recognised theme properties — silent skip per QT-06 §7.
        return Ok(());
    };
    let yaml = render_tokens_yaml(&theme, &input.display().to_string());
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(out_path, yaml).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ============================================================================
// QT-07: asset-crate handoff (`docs/qt-support/07-asset-handoff.md`).
// ============================================================================

/// QT-07 §3 — inventory of asset references discovered across a
/// QML tree. Sets keep entries deduplicated and lexically ordered.
#[derive(Debug, Default)]
pub struct AssetInventory {
    pub images: std::collections::BTreeSet<String>,
    pub fonts: std::collections::BTreeSet<String>,
}

impl AssetInventory {
    fn is_empty(&self) -> bool {
        self.images.is_empty() && self.fonts.is_empty()
    }
}

/// QT-07 §3 / §5 — pure walker. Recurses through `item` and every
/// descendant `UiItem`, extracting Image and font references.
pub fn walk_asset_refs(item: &UiItem) -> AssetInventory {
    let mut inv = AssetInventory::default();
    visit_for_assets(item, &mut inv);
    inv
}

fn visit_for_assets(item: &UiItem, inv: &mut AssetInventory) {
    let stripped_type = item.type_name.rsplit('.').next().unwrap_or(&item.type_name);

    // Images: any item whose stripped type is `Image` and which has
    // a literal `source: "<path>"` assignment.
    if stripped_type == "Image"
        && let Some(raw_source) = lookup_assignment(item, "source")
        && let Some(s) = parse_string_literal(raw_source)
    {
        inv.images.insert(strip_qrc_prefix(&s).to_string());
    }

    // Standalone Font { family: "<name>" } blocks.
    if stripped_type == "Font"
        && let Some(raw) = lookup_assignment(item, "family")
        && let Some(s) = parse_string_literal(raw)
    {
        inv.fonts.insert(s);
    }

    // Dotted form `font.family: "<name>"` on any item.
    if let Some(raw) = lookup_assignment(item, "font.family")
        && let Some(s) = parse_string_literal(raw)
    {
        inv.fonts.insert(s);
    }

    // Nested `font: Font { family: "<name>" }` object value.
    for asn in &item.assignments {
        if asn.target == "font"
            && let UiAssignmentValue::Object { item: nested } = &asn.value
            && nested
                .type_name
                .rsplit('.')
                .next()
                .unwrap_or(&nested.type_name)
                == "Font"
            && let Some(raw) = lookup_assignment(nested, "family")
            && let Some(s) = parse_string_literal(raw)
        {
            inv.fonts.insert(s);
        }
    }

    for child in &item.children {
        visit_for_assets(child, inv);
    }
}

/// QT-07 §5 — strip `qrc:/` and `qrc:///` prefixes verbatim. Any
/// other prefix (including HTTP URLs) passes through unchanged.
fn strip_qrc_prefix(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix("qrc:///") {
        rest
    } else if let Some(rest) = path.strip_prefix("qrc:/") {
        rest
    } else {
        path
    }
}

/// QT-07 §6 — render the inventory YAML. Lists are always emitted
/// (even when empty) so the schema is stable.
pub fn render_assets_yaml(inv: &AssetInventory, qml_source: &str) -> String {
    let mut out = String::new();
    out.push_str("# Auto-generated from Qt project by rlvgl-creator (QT-07)\n");
    out.push_str(&format!("# QT-07 assets: {qml_source}\n"));
    out.push_str("version: 1\n");
    out.push_str("images:\n");
    for path in &inv.images {
        out.push_str(&format!("  - {}\n", quote_yaml_scalar(path)));
    }
    out.push_str("fonts:\n");
    for family in &inv.fonts {
        out.push_str(&format!("  - {}\n", quote_yaml_scalar(family)));
    }
    out
}

/// Quote scalars containing whitespace or YAML metacharacters so
/// the output round-trips through any YAML parser.
fn quote_yaml_scalar(s: &str) -> String {
    let needs_quote = s.is_empty()
        || s.chars().any(|c| {
            matches!(
                c,
                ' ' | '\t' | ':' | '#' | '"' | '\'' | '[' | ']' | '{' | '}' | ','
            )
        });
    if needs_quote {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// QT-07 §7 — `qt list-assets <input> [<out>]` entry point.
pub(crate) fn list_assets(input: &Path, out: Option<&Path>) -> Result<()> {
    if input.is_dir() {
        let out_dir = out.unwrap_or(input);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output dir {}", out_dir.display()))?;
        for qml in qt08_collect_qml_files(input)? {
            list_assets_one(&qml, &resolve_assets_out_for(&qml, out_dir))?;
        }
        Ok(())
    } else {
        let out_path = match out {
            Some(p) if p.is_dir() => resolve_assets_out_for(input, p),
            Some(p) => p.to_path_buf(),
            None => resolve_assets_out_for(input, input.parent().unwrap_or_else(|| Path::new("."))),
        };
        list_assets_one(input, &out_path)
    }
}

fn resolve_assets_out_for(qml: &Path, out_dir: &Path) -> PathBuf {
    let stem = qml
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    out_dir.join(format!("{stem}.assets.yaml"))
}

fn list_assets_one(input: &Path, out_path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    let inv = walk_asset_refs(&module.root);
    if inv.is_empty() {
        // QT-07 §7 silent skip.
        return Ok(());
    }
    let yaml = render_assets_yaml(&inv, &input.display().to_string());
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(out_path, yaml).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ============================================================================
// QT-08b: qmldir manifest parser (`docs/qt-support/08b-qmldir-resolution.md`).
// ============================================================================

/// QT-08b §3 — single type registration in a `qmldir` manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct QmldirType {
    pub name: String,
    pub version: Option<String>,
    pub file: String,
}

/// QT-08b §3 — `import` / `depends` directive entry.
#[derive(Debug, Clone, PartialEq)]
pub struct QmldirImport {
    pub module: String,
    pub version: Option<String>,
}

/// QT-08b §3 — `plugin <name> [<path>]` directive.
#[derive(Debug, Clone, PartialEq)]
pub struct QmldirPlugin {
    pub name: String,
    pub path: Option<String>,
}

/// QT-08b §3 — parsed `qmldir` manifest.
#[derive(Debug, Default, PartialEq)]
pub struct QmldirManifest {
    pub module: Option<String>,
    pub types: Vec<QmldirType>,
    pub singletons: Vec<QmldirType>,
    pub internals: Vec<QmldirType>,
    pub imports: Vec<QmldirImport>,
    pub depends: Vec<QmldirImport>,
    pub plugins: Vec<QmldirPlugin>,
    pub other: Vec<String>,
}

/// QT-08b §5 — pure parser. Tokenises whitespace-separated fields,
/// drops `#`-prefix comments and blank lines.
pub fn parse_qmldir(content: &str) -> QmldirManifest {
    let mut m = QmldirManifest::default();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        match tokens.as_slice() {
            ["module", name] => {
                // Last-one-wins per QT-08b §5.
                m.module = Some(name.to_string());
            }
            ["singleton", name, version, file] => {
                m.singletons.push(QmldirType {
                    name: name.to_string(),
                    version: Some(version.to_string()),
                    file: file.to_string(),
                });
            }
            ["internal", name, file] => {
                m.internals.push(QmldirType {
                    name: name.to_string(),
                    version: None,
                    file: file.to_string(),
                });
            }
            ["import", module] => {
                m.imports.push(QmldirImport {
                    module: module.to_string(),
                    version: None,
                });
            }
            ["import", module, version] => {
                m.imports.push(QmldirImport {
                    module: module.to_string(),
                    version: Some(version.to_string()),
                });
            }
            ["depends", module] => {
                m.depends.push(QmldirImport {
                    module: module.to_string(),
                    version: None,
                });
            }
            ["depends", module, version] => {
                m.depends.push(QmldirImport {
                    module: module.to_string(),
                    version: Some(version.to_string()),
                });
            }
            ["plugin", name] => {
                m.plugins.push(QmldirPlugin {
                    name: name.to_string(),
                    path: None,
                });
            }
            ["plugin", name, path] => {
                m.plugins.push(QmldirPlugin {
                    name: name.to_string(),
                    path: Some(path.to_string()),
                });
            }
            [name, version, file] if file.ends_with(".qml") => {
                m.types.push(QmldirType {
                    name: name.to_string(),
                    version: Some(version.to_string()),
                    file: file.to_string(),
                });
            }
            _ => {
                m.other.push(line.to_string());
            }
        }
    }
    m
}

/// QT-08b §6 — render the qmldir manifest as a stable YAML
/// inventory.
pub fn render_qmldir_yaml(manifest: &QmldirManifest, source: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# QT-08b qmldir: {source}\n"));
    out.push_str("version: 1\n");
    match &manifest.module {
        Some(name) => out.push_str(&format!("module: {name}\n")),
        None => out.push_str("module: null\n"),
    }
    out.push_str("types:\n");
    for t in &manifest.types {
        out.push_str(&format!("  - {}\n", render_qmldir_type(t)));
    }
    out.push_str("singletons:\n");
    for t in &manifest.singletons {
        out.push_str(&format!("  - {}\n", render_qmldir_type(t)));
    }
    out.push_str("internals:\n");
    for t in &manifest.internals {
        out.push_str(&format!("  - {}\n", render_qmldir_type(t)));
    }
    out.push_str("imports:\n");
    for i in &manifest.imports {
        out.push_str(&format!("  - {}\n", render_qmldir_import(i)));
    }
    out.push_str("depends:\n");
    for d in &manifest.depends {
        out.push_str(&format!("  - {}\n", render_qmldir_import(d)));
    }
    out.push_str("plugins:\n");
    for p in &manifest.plugins {
        let path = p
            .path
            .as_deref()
            .map(|s| format!("\"{s}\""))
            .unwrap_or_else(|| "null".to_string());
        out.push_str(&format!("  - {{ name: {}, path: {path} }}\n", p.name));
    }
    out.push_str("other:\n");
    for s in &manifest.other {
        out.push_str(&format!("  - {}\n", quote_yaml_scalar(s)));
    }
    out
}

fn render_qmldir_type(t: &QmldirType) -> String {
    let version = t
        .version
        .as_deref()
        .map(|s| format!("\"{s}\""))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{ name: {}, version: {version}, file: {} }}",
        t.name, t.file
    )
}

fn render_qmldir_import(i: &QmldirImport) -> String {
    let version = i
        .version
        .as_deref()
        .map(|s| format!("\"{s}\""))
        .unwrap_or_else(|| "null".to_string());
    format!("{{ module: {}, version: {version} }}", i.module)
}

/// QT-08b §7 — `qt list-qmldir <input> [<out>]` entry point.
pub(crate) fn list_qmldir(input: &Path, out: Option<&Path>) -> Result<()> {
    let (qmldir_path, dirname) = if input.is_dir() {
        let p = input.join("qmldir");
        let dn = input
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        (p, dn)
    } else {
        let dn = input
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        (input.to_path_buf(), dn)
    };
    if !qmldir_path.exists() {
        bail!(
            "QT-08b §7: expected qmldir file at {} (not found)",
            qmldir_path.display()
        );
    }
    let content = fs::read_to_string(&qmldir_path)
        .with_context(|| format!("reading {}", qmldir_path.display()))?;
    let manifest = parse_qmldir(&content);
    let source_label = qmldir_path.display().to_string();
    let yaml = render_qmldir_yaml(&manifest, &source_label);

    let out_path = match out {
        Some(p) if p.is_dir() => p.join(format!("{dirname}.qmldir.yaml")),
        Some(p) => p.to_path_buf(),
        None => qmldir_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{dirname}.qmldir.yaml")),
    };
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(&out_path, yaml).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ============================================================================
// QT-08c: .qrc resource manifest parser (`docs/qt-support/08c-qrc-resources.md`).
// ============================================================================

/// QT-08c §3 — single bundled file entry inside a `<qresource>`.
#[derive(Debug, Clone, PartialEq)]
pub struct QrcFile {
    pub path: String,
    pub alias: Option<String>,
}

/// QT-08c §3 — `<qresource prefix="…" lang="…">` block.
#[derive(Debug, Clone, PartialEq)]
pub struct QrcResource {
    pub prefix: Option<String>,
    pub lang: Option<String>,
    pub files: Vec<QrcFile>,
}

/// QT-08c §3 — parsed `.qrc` manifest.
#[derive(Debug, Default, PartialEq)]
pub struct QrcManifest {
    pub version: Option<String>,
    pub resources: Vec<QrcResource>,
}

/// QT-08c §5 — pure parser. Hand-rolled minimal XML walker keyed
/// on the recognised element subset; rejects unknown elements
/// under `<RCC>` and inside `<qresource>`. No new Cargo deps.
pub fn parse_qrc(content: &str) -> Result<QrcManifest> {
    let mut parser = QrcParser::new(content);
    parser.parse_document()
}

struct QrcParser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> QrcParser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn parse_document(&mut self) -> Result<QrcManifest> {
        // Strip prologue comments / DOCTYPE / xml-decl / whitespace
        // until we hit the <RCC> opening tag.
        loop {
            self.skip_ws();
            if self.starts_with("<!--") {
                self.skip_comment()?;
                continue;
            }
            if self.starts_with("<!DOCTYPE") {
                self.skip_doctype()?;
                continue;
            }
            if self.starts_with("<?") {
                self.skip_pi()?;
                continue;
            }
            break;
        }
        if !self.starts_with("<RCC") {
            bail!(
                "QT-08c §5: expected `<RCC>` root element at byte {}",
                self.pos
            );
        }
        // Parse <RCC ...>
        self.expect("<RCC")?;
        let attrs = self.parse_attrs()?;
        self.expect(">")?;
        let mut manifest = QrcManifest::default();
        if let Some(v) = attrs.iter().find(|(k, _)| k == "version") {
            manifest.version = Some(v.1.clone());
        }
        loop {
            self.skip_ws_and_comments()?;
            if self.starts_with("</RCC>") {
                self.expect("</RCC>")?;
                self.skip_ws_and_comments()?;
                if self.pos < self.src.len() {
                    bail!(
                        "QT-08c §5: unexpected trailing content after </RCC> at byte {}",
                        self.pos
                    );
                }
                return Ok(manifest);
            }
            if self.starts_with("<qresource") {
                let res = self.parse_qresource()?;
                manifest.resources.push(res);
                continue;
            }
            bail!(
                "QT-08c §5: unrecognised element under <RCC> at byte {} — \
                 only <qresource> is allowed (per §5 strictness rule)",
                self.pos
            );
        }
    }

    fn parse_qresource(&mut self) -> Result<QrcResource> {
        self.expect("<qresource")?;
        let attrs = self.parse_attrs()?;
        self.expect(">")?;
        let mut res = QrcResource {
            prefix: attrs
                .iter()
                .find(|(k, _)| k == "prefix")
                .map(|(_, v)| v.clone()),
            lang: attrs
                .iter()
                .find(|(k, _)| k == "lang")
                .map(|(_, v)| v.clone()),
            files: Vec::new(),
        };
        loop {
            self.skip_ws_and_comments()?;
            if self.starts_with("</qresource>") {
                self.expect("</qresource>")?;
                return Ok(res);
            }
            if self.starts_with("<file") {
                let file = self.parse_file()?;
                res.files.push(file);
                continue;
            }
            bail!(
                "QT-08c §5: unrecognised element under <qresource> at byte {} — \
                 only <file> is allowed",
                self.pos
            );
        }
    }

    fn parse_file(&mut self) -> Result<QrcFile> {
        self.expect("<file")?;
        let attrs = self.parse_attrs()?;
        self.expect(">")?;
        // Read text content until `</file>`. No CDATA / nested
        // elements supported at v1 per §5.
        let start = self.pos;
        let close_idx = self.src[start..]
            .find("</file>")
            .ok_or_else(|| anyhow::anyhow!("QT-08c §5: unterminated <file> at byte {start}"))?;
        let path = self.src[start..start + close_idx].trim().to_string();
        self.pos = start + close_idx;
        self.expect("</file>")?;
        let alias = attrs
            .iter()
            .find(|(k, _)| k == "alias")
            .map(|(_, v)| v.clone());
        Ok(QrcFile { path, alias })
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.src.as_bytes().get(self.pos) {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn skip_ws_and_comments(&mut self) -> Result<()> {
        loop {
            self.skip_ws();
            if self.starts_with("<!--") {
                self.skip_comment()?;
                continue;
            }
            return Ok(());
        }
    }

    fn skip_comment(&mut self) -> Result<()> {
        self.expect("<!--")?;
        let start = self.pos;
        let end = self.src[start..]
            .find("-->")
            .ok_or_else(|| anyhow::anyhow!("QT-08c: unterminated XML comment at byte {start}"))?;
        self.pos = start + end + "-->".len();
        Ok(())
    }

    fn skip_doctype(&mut self) -> Result<()> {
        // Only single-line `<!DOCTYPE …>` supported at v1.
        let start = self.pos;
        let end = self.src[start..]
            .find('>')
            .ok_or_else(|| anyhow::anyhow!("QT-08c: unterminated DOCTYPE at byte {start}"))?;
        self.pos = start + end + 1;
        Ok(())
    }

    fn skip_pi(&mut self) -> Result<()> {
        let start = self.pos;
        let end = self.src[start..]
            .find("?>")
            .ok_or_else(|| anyhow::anyhow!("QT-08c: unterminated XML PI at byte {start}"))?;
        self.pos = start + end + "?>".len();
        Ok(())
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.src[self.pos..].starts_with(prefix)
    }

    fn expect(&mut self, lit: &str) -> Result<()> {
        if self.starts_with(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            bail!(
                "QT-08c §5: expected `{lit}` at byte {} (saw `{}`)",
                self.pos,
                &self.src[self.pos..self.pos.saturating_add(8.min(self.src.len() - self.pos))]
            )
        }
    }

    fn parse_attrs(&mut self) -> Result<Vec<(String, String)>> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            let bytes = self.src.as_bytes();
            let Some(&b) = bytes.get(self.pos) else {
                bail!("QT-08c §5: unexpected end of input in attribute list");
            };
            if b == b'>' || (b == b'/' && bytes.get(self.pos + 1) == Some(&b'>')) {
                return Ok(out);
            }
            let name_start = self.pos;
            while let Some(&c) = self.src.as_bytes().get(self.pos) {
                if matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'/' | b'>') {
                    break;
                }
                self.pos += 1;
            }
            if self.pos == name_start {
                bail!("QT-08c §5: expected attribute name at byte {}", self.pos);
            }
            let name = self.src[name_start..self.pos].to_string();
            self.skip_ws();
            self.expect("=")?;
            self.skip_ws();
            let quote = self
                .src
                .as_bytes()
                .get(self.pos)
                .copied()
                .ok_or_else(|| anyhow::anyhow!("QT-08c: expected attribute quote"))?;
            if quote != b'"' && quote != b'\'' {
                bail!(
                    "QT-08c §5: attribute value must be quoted at byte {}",
                    self.pos
                );
            }
            self.pos += 1;
            let val_start = self.pos;
            let close_byte = quote;
            while let Some(&c) = self.src.as_bytes().get(self.pos) {
                if c == close_byte {
                    break;
                }
                self.pos += 1;
            }
            let value = self.src[val_start..self.pos].to_string();
            self.expect(if quote == b'"' { "\"" } else { "'" })?;
            out.push((name, value));
        }
    }
}

/// QT-08c §6 — render a stable YAML inventory.
pub fn render_qrc_yaml(manifest: &QrcManifest, source: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# QT-08c qrc: {source}\n"));
    out.push_str("version: 1\n");
    match &manifest.version {
        Some(v) => out.push_str(&format!("rcc_version: \"{v}\"\n")),
        None => out.push_str("rcc_version: null\n"),
    }
    out.push_str("resources:\n");
    for r in &manifest.resources {
        let prefix = r
            .prefix
            .as_deref()
            .map(|s| format!("\"{s}\""))
            .unwrap_or_else(|| "null".to_string());
        let lang = r
            .lang
            .as_deref()
            .map(|s| format!("\"{s}\""))
            .unwrap_or_else(|| "null".to_string());
        out.push_str(&format!("  - prefix: {prefix}\n"));
        out.push_str(&format!("    lang: {lang}\n"));
        out.push_str("    files:\n");
        for f in &r.files {
            let alias = f
                .alias
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "null".to_string());
            out.push_str(&format!("      - {{ path: {}, alias: {alias} }}\n", f.path));
        }
    }
    out
}

/// QT-08c §7 — `qt list-qrc <input> [<out>]` entry point.
pub(crate) fn list_qrc(input: &Path, out: Option<&Path>) -> Result<()> {
    if input.is_dir() {
        let out_dir = out.unwrap_or(input);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output dir {}", out_dir.display()))?;
        let read =
            fs::read_dir(input).with_context(|| format!("reading dir {}", input.display()))?;
        let mut qrc_files: Vec<PathBuf> = Vec::new();
        for entry in read {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("qrc") {
                qrc_files.push(path);
            }
        }
        qrc_files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for qrc in qrc_files {
            list_qrc_one(&qrc, &resolve_qrc_out_for(&qrc, out_dir))?;
        }
        Ok(())
    } else {
        if !input.exists() {
            bail!(
                "QT-08c §7: expected .qrc file at {} (not found)",
                input.display()
            );
        }
        let out_path = match out {
            Some(p) if p.is_dir() => resolve_qrc_out_for(input, p),
            Some(p) => p.to_path_buf(),
            None => resolve_qrc_out_for(input, input.parent().unwrap_or_else(|| Path::new("."))),
        };
        list_qrc_one(input, &out_path)
    }
}

fn resolve_qrc_out_for(qrc: &Path, out_dir: &Path) -> PathBuf {
    let stem = qrc
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    out_dir.join(format!("{stem}.qrc.yaml"))
}

fn list_qrc_one(input: &Path, out_path: &Path) -> Result<()> {
    let content =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let manifest = parse_qrc(&content)
        .with_context(|| format!("parsing {} per QT-08c §5", input.display()))?;
    let yaml = render_qrc_yaml(&manifest, &input.display().to_string());
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }
    fs::write(out_path, yaml).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

// ============================================================================
// QT-05a: scjson side-file discovery + walk (`docs/qt-support/05a-scjson-ingest.md`).
// ============================================================================

/// QT-05a §3 — discover the sibling scjson side-file for a given QML
/// path. `/path/to/foo.qml` → `Some(/path/to/foo.scjson)` iff the file
/// exists, else `None`. Symlinks are followed via `fs::metadata`;
/// broken symlinks count as "absent".
fn find_scjson_side_file(qml_path: &Path) -> Option<PathBuf> {
    let stem = qml_path.file_stem()?.to_str()?;
    let parent = qml_path.parent().unwrap_or_else(|| Path::new("."));
    let candidate = parent.join(format!("{stem}.scjson"));
    if fs::metadata(&candidate).is_ok() {
        Some(candidate)
    } else {
        None
    }
}

/// QT-05a §5–§7 — apply side-file discovery to a freshly parsed
/// `UiModule`. Missing scjson is silent fall-through; malformed or
/// "not actually scjson" content is a hard error tagged with the path.
fn attach_scjson_side_file(module: &mut UiModule, qml_path: &Path) -> Result<()> {
    let Some(scjson_path) = find_scjson_side_file(qml_path) else {
        return Ok(());
    };
    let raw = fs::read_to_string(&scjson_path)
        .with_context(|| format!("reading scjson side-file {}", scjson_path.display()))?;
    if raw.trim().is_empty() {
        bail!(
            "scjson side-file is empty: {} (per QT-05a §7)",
            scjson_path.display()
        );
    }
    let scxml: qt_scjson::Scxml = serde_json::from_str(&raw).with_context(|| {
        format!(
            "parsing scjson side-file {} (per QT-05a §7)",
            scjson_path.display()
        )
    })?;
    if scxml.state.is_empty() && scxml.datamodel.is_empty() && scxml.initial.is_empty() {
        bail!(
            "scjson side-file {} has neither <state> nor <datamodel> nor an initial state — \
             it does not look like a state machine (per QT-05a §7 / QT-05 §5)",
            scjson_path.display()
        );
    }
    let id = derive_state_machine_id(&scxml, qml_path)?;
    let source = scjson_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    module.state_machine = Some(walk_scxml_into_ui_state_machine(&scxml, id, source)?);
    Ok(())
}

/// QT-05a §8 — derive the `<sm>` ID. Default = QML basename in
/// snake_case; override via `<scxml name="…">` if present.
fn derive_state_machine_id(scxml: &qt_scjson::Scxml, qml_path: &Path) -> Result<String> {
    if let Some(name) = scxml.name.as_deref()
        && !name.trim().is_empty()
    {
        return Ok(snake_case_for_sm(name));
    }
    let stem = qml_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            anyhow::anyhow!("QML path has no usable file stem: {}", qml_path.display())
        })?;
    Ok(snake_case_for_sm(stem))
}

/// snake_case helper for the `<sm>` ID. Mirrors the convention used
/// by `rlvgl_creator` for board names: `Stopwatch` → `stopwatch`,
/// `MyTimer` → `my_timer`, `traffic-light` → `traffic_light`.
fn snake_case_for_sm(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_lower = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if prev_lower {
                    out.push('_');
                }
                for low in ch.to_lowercase() {
                    out.push(low);
                }
                prev_lower = false;
            } else {
                out.push(ch);
                prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
            }
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
            prev_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// QT-05a §6 — walk a parsed `Scxml` into a `UiStateMachine`. Steps
/// 1–6 are byte-stable: emission order matches the §6 listing so
/// goldens are reproducible.
fn walk_scxml_into_ui_state_machine(
    scxml: &qt_scjson::Scxml,
    id: String,
    source: String,
) -> Result<UiStateMachine> {
    // Step 1 — initial.
    let initial = scxml.initial.iter().find(|s| !s.is_empty()).cloned();

    // Step 2 — flatten states (depth-first; nested states become
    // additional top-level entries).
    let mut states: Vec<UiState> = Vec::new();
    let mut transitions: Vec<UiTransition> = Vec::new();
    let mut scripts: Vec<UiScript> = Vec::new();
    let mut anon_state_counter: u32 = 0;
    let mut transition_index: u32 = 0;

    walk_states(
        &scxml.state,
        &mut states,
        &mut transitions,
        &mut scripts,
        &mut anon_state_counter,
        &mut transition_index,
    )?;

    // Step 4 — datamodel.
    let mut datamodel: Vec<UiDmField> = Vec::new();
    for dm in &scxml.datamodel {
        for d in &dm.data {
            let initial_value = d.expr.as_deref().and_then(|expr| expr.trim().parse().ok());
            datamodel.push(UiDmField {
                id: d.id.clone(),
                initial: initial_value,
            });
        }
    }

    // Step 5 (top-level scripts). Top-level scripts get an
    // OnEntry { state: "_root" } origin for diagnostic clarity.
    for sc in &scxml.script {
        scripts.push(UiScript {
            name: extract_script_name(sc, &mut 0_u32, "root_script"),
            origin: UiScriptOrigin::OnEntry {
                state: "_root".to_string(),
            },
        });
    }

    Ok(UiStateMachine {
        id,
        source,
        initial,
        states,
        transitions,
        datamodel,
        scripts,
    })
}

fn walk_states(
    src: &[qt_scjson::State],
    states: &mut Vec<UiState>,
    transitions: &mut Vec<UiTransition>,
    scripts: &mut Vec<UiScript>,
    anon_state_counter: &mut u32,
    transition_index: &mut u32,
) -> Result<()> {
    for s in src {
        let state_id = s.id.clone().unwrap_or_else(|| {
            let n = *anon_state_counter;
            *anon_state_counter += 1;
            format!("_anon_{n}")
        });

        let on_entry = lower_action_block(
            &s.onentry,
            scripts,
            UiScriptOrigin::OnEntry {
                state: state_id.clone(),
            },
        );
        let on_exit = lower_action_block_exit(
            &s.onexit,
            scripts,
            UiScriptOrigin::OnExit {
                state: state_id.clone(),
            },
        );

        // Push the (flattened) state.
        states.push(UiState {
            id: state_id.clone(),
            on_entry,
            on_exit,
        });

        // Step 3 — transitions for this state.
        for t in &s.transition {
            let idx = *transition_index;
            *transition_index += 1;
            let target = t.target.first().cloned();
            let actions = lower_transition_actions(
                t,
                scripts,
                UiScriptOrigin::Transition {
                    index: idx,
                    from: state_id.clone(),
                    to: target.clone(),
                },
            );
            transitions.push(UiTransition {
                source: state_id.clone(),
                event: t.event.clone(),
                target,
                cond: t.cond.clone(),
                actions,
            });
        }

        // Recurse into nested states (flatten per QT-05a §6 step 2).
        if !s.state.is_empty() {
            walk_states(
                &s.state,
                states,
                transitions,
                scripts,
                anon_state_counter,
                transition_index,
            )?;
        }
    }
    Ok(())
}

fn lower_action_block(
    onentry: &[qt_scjson::Onentry],
    scripts: &mut Vec<UiScript>,
    origin: UiScriptOrigin,
) -> Vec<UiAction> {
    let mut out = Vec::new();
    let mut anon = 0_u32;
    for blk in onentry {
        for a in &blk.assign {
            out.push(UiAction::Assign {
                location: a.location.clone(),
                expr: a.expr.clone(),
            });
        }
        for r in &blk.raise_value {
            out.push(UiAction::Raise {
                event: r.event.clone(),
            });
        }
        for sc in &blk.script {
            let name = extract_script_name(sc, &mut anon, "onentry");
            scripts.push(UiScript {
                name: name.clone(),
                origin: origin.clone(),
            });
            out.push(UiAction::Script { name });
        }
    }
    out
}

fn lower_action_block_exit(
    onexit: &[qt_scjson::Onexit],
    scripts: &mut Vec<UiScript>,
    origin: UiScriptOrigin,
) -> Vec<UiAction> {
    let mut out = Vec::new();
    let mut anon = 0_u32;
    for blk in onexit {
        for a in &blk.assign {
            out.push(UiAction::Assign {
                location: a.location.clone(),
                expr: a.expr.clone(),
            });
        }
        for r in &blk.raise_value {
            out.push(UiAction::Raise {
                event: r.event.clone(),
            });
        }
        for sc in &blk.script {
            let name = extract_script_name(sc, &mut anon, "onexit");
            scripts.push(UiScript {
                name: name.clone(),
                origin: origin.clone(),
            });
            out.push(UiAction::Script { name });
        }
    }
    out
}

fn lower_transition_actions(
    t: &qt_scjson::Transition,
    scripts: &mut Vec<UiScript>,
    origin: UiScriptOrigin,
) -> Vec<UiAction> {
    let mut out = Vec::new();
    let mut anon = 0_u32;
    for a in &t.assign {
        out.push(UiAction::Assign {
            location: a.location.clone(),
            expr: a.expr.clone(),
        });
    }
    for r in &t.raise_value {
        out.push(UiAction::Raise {
            event: r.event.clone(),
        });
    }
    for sc in &t.script {
        let name = extract_script_name(sc, &mut anon, "trans");
        scripts.push(UiScript {
            name: name.clone(),
            origin: origin.clone(),
        });
        out.push(UiAction::Script { name });
    }
    out
}

/// Extract a `<script name="…"/>` from `other_attributes` (the
/// canonical scjson location per `vendor/scjson/py/scjson/pydantic/`).
/// Falls back to a deterministic synthesized name keyed on the
/// caller-provided `tag` when the attribute is missing — mirrors
/// istate's `context.py::_extract_actions` synthesis convention.
fn extract_script_name(sc: &qt_scjson::Script, anon: &mut u32, tag: &str) -> String {
    if let Some(v) = sc.other_attributes.get("name")
        && let Some(s) = v.as_str()
        && !s.is_empty()
    {
        return s.to_string();
    }
    let n = *anon;
    *anon += 1;
    format!("script_{tag}_{n}")
}

// ============================================================================
// Public entry point
// ============================================================================

/// Parse `input` and exit non-zero on any parse error. Does not emit IR.
///
/// QT-05a: also validates any sibling `<basename>.scjson` side-file —
/// a malformed scjson is a hard error here, matching `qt ingest` and
/// `qt emit`.
pub(crate) fn check(input: &Path) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let mut module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    attach_scjson_side_file(&mut module, input)?;
    Ok(())
}

/// Emit a JSON Schema describing [`UiModule`]. Writes to `out` if given,
/// otherwise prints to stdout. The output is decorated with the
/// canonical `$id` and `$comment` (see [`QT_IR_SCHEMA_ID`] /
/// [`QT_IR_SCHEMA_COMMENT`]) so it matches the convention used by other
/// schemas in `schemas/`.
pub(crate) fn schema(out: Option<&Path>) -> Result<()> {
    let json = render_schema()?;
    if let Some(path) = out {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output dir {}", parent.display()))?;
        }
        fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

/// Render the canonical `qt-ir.schema.json` body as a `String`.
///
/// Splits out from [`schema`] so tests can compare in-memory output
/// against the checked-in `schemas/qt-ir.schema.json` without going
/// through a tempdir.
pub fn render_schema() -> Result<String> {
    let raw = schemars::schema_for!(UiModule);
    let raw_value = serde_json::to_value(raw)?;
    let decorated = decorate_schema(raw_value);
    let json = serde_json::to_string_pretty(&decorated)?;
    Ok(json)
}

fn decorate_schema(raw: serde_json::Value) -> serde_json::Value {
    let raw_obj = match raw {
        serde_json::Value::Object(obj) => obj,
        other => return other,
    };
    let mut head = serde_json::Map::new();
    if let Some(s) = raw_obj.get("$schema") {
        head.insert("$schema".to_string(), s.clone());
    }
    head.insert(
        "$id".to_string(),
        serde_json::Value::String(QT_IR_SCHEMA_ID.to_string()),
    );
    head.insert(
        "$comment".to_string(),
        serde_json::Value::String(QT_IR_SCHEMA_COMMENT.to_string()),
    );
    for (k, v) in raw_obj.into_iter() {
        if k == "$schema" {
            continue;
        }
        head.insert(k, v);
    }
    serde_json::Value::Object(head)
}

/// `qt emit --target data` shape version. Owned by phase QT-03
/// (see `docs/qt-support/03-rlvgl-emitter-widgets.md` §7).
pub const QT_EMIT_VERSION_DATA: u32 = 1;

/// `qt emit --target rlvgl` shape version. Owned by phase QT-03b
/// (see `docs/qt-support/03b-rlvgl-widget-mapping.md` §11);
/// bumped to `3` by QT-04 when `onClicked` lowering shipped
/// (`docs/qt-support/04-signal-handlers.md` §11);
/// bumped to `4` by QT-04b when `ScreenState` + handler-body
/// lowering shipped (`docs/qt-support/04b-properties-bindings.md` §11);
/// bumped to `5` by QT-04c when initial-value text bindings shipped
/// (`docs/qt-support/04c-initial-value-bindings.md` §8);
/// bumped to `6` by QT-03c when the `anchors.centerIn` resolver
/// shipped (`docs/qt-support/03c-anchor-resolver.md` §8);
/// bumped to `7` by QT-04f when nested-id resolution shipped
/// (`docs/qt-support/04f-nested-id-resolution.md` §8);
/// bumped to `8` by the QT-03c §5 amendment promoting single edge
/// anchors (`anchors.left/right/top/bottom`) — see
/// `docs/qt-support/03c-anchor-resolver.md` §15 (2026-04-29 amendment);
/// bumped to `9` by the QT-03c §5 amendment #2 promoting corner
/// combinations (`left+top`, `right+top`, `left+bottom`, `right+bottom`);
/// bumped to `10` by QT-04d when QML `MouseArea` was promoted to
/// the new `rlvgl_widgets::click_area::ClickArea` widget
/// (`docs/qt-support/04d-mousearea.md` §8);
/// bumped to `11` by QT-04e when reactive Label-text bindings
/// shipped (`docs/qt-support/04e-reactive-bindings.md` §8). Closes
/// out the QT-04 family.
pub const QT_EMIT_VERSION_RLVGL: u32 = 13;

/// QT-10 strict-mode generation. Bumps when the chapter file set
/// (QT-10 §5), the CLI subcommand set (QT-10 §5), or the
/// version-constant snapshot (QT-10 §6) changes. The strict-mode
/// meta-test (`tests/creator_qt_strict_mode.rs`) asserts this
/// constant matches the expected generation.
pub const QT_FAMILY_STRICT_VERSION: u32 = 1;

/// Backward-compat alias for the data-target version constant.
/// Removed when QT-04 ships per QT-03b §11.
#[deprecated(note = "use QT_EMIT_VERSION_DATA (the rlvgl target uses QT_EMIT_VERSION_RLVGL)")]
pub const QT_EMIT_VERSION: u32 = QT_EMIT_VERSION_DATA;

/// Emit target selector for `qt emit`. Owned by phase QT-03b §11.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EmitTarget {
    /// QT-03 data-only `pub static SCREEN: Node = …;` shape.
    Data,
    /// QT-03b runnable `build_screen(bounds) -> WidgetNode` shape.
    Rlvgl,
}

/// Parse a `.qml` file and write a self-contained `<basename>.rs` (data
/// target) or `<basename>.rlvgl.rs` (rlvgl target) into `out/`.
///
/// The emit-shape contracts are owned by QT-03 (data) and QT-03b
/// (rlvgl); see the canonical goldens at `tests/fixtures/qt/`.
pub(crate) fn emit(input: &Path, out: &Path, target: EmitTarget) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating output dir {}", out.display()))?;
    if input.is_dir() {
        for qml in qt08_collect_qml_files(input)? {
            emit_one_file(&qml, out, target)?;
        }
        return Ok(());
    }
    emit_one_file(input, out, target)
}

fn emit_one_file(input: &Path, out: &Path, target: EmitTarget) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let mut module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    // QT-05a: link sibling .scjson side-file (silent fall-through if absent).
    attach_scjson_side_file(&mut module, input)?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("input has no usable file stem: {}", input.display()))?;

    let (out_path, rust) = match target {
        EmitTarget::Data => (out.join(format!("{stem}.rs")), render_rs(&module)),
        EmitTarget::Rlvgl => (out.join(format!("{stem}.rlvgl.rs")), render_rlvgl(&module)),
    };
    fs::write(&out_path, rust).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

/// Render a [`UiModule`] as the canonical QT-03 Rust emit-shape.
///
/// Public so the schema-drift / golden-file tests can compare without
/// going through a tempdir.
pub fn render_rs(module: &UiModule) -> String {
    let mut out = String::new();
    out.push_str("// SPDX-License-Identifier: MIT\n");
    out.push_str("//\n");
    out.push_str(&format!(
        "// Generated by `rlvgl-creator qt emit` from `{}`.\n",
        module.source
    ));
    out.push_str("// Do not edit by hand — regenerate with:\n");
    out.push_str("//   cargo run --features creator --bin rlvgl-creator -- \\\n");
    out.push_str("//       qt emit <input.qml> <out_dir>\n");
    out.push_str("//\n");
    out.push_str("// Emit-shape contract: docs/qt-support/03-rlvgl-emitter-widgets.md\n");
    out.push_str("// QT-03 phase notes:\n");
    out.push_str("//   * widget API mapping is intentionally deferred to QT-03b.\n");
    out.push_str("//   * properties / signals / handlers are not yet lowered;\n");
    out.push_str("//     they remain in the JSON IR (qt-ir.json) and are not\n");
    out.push_str("//     materialised in this static `Node` form.\n");
    out.push_str("//\n");
    out.push_str("// The emitted module uses no `String`/`Vec`/`std` paths and is\n");
    out.push_str("// safe to consume from a `no_std` crate. We deliberately do NOT\n");
    out.push_str("// emit `#![no_std]` because that is a crate-root attribute and\n");
    out.push_str("// would be rejected when this file is pulled in via `#[path]`.\n");
    out.push('\n');
    out.push_str("#![allow(dead_code)]\n");
    out.push('\n');
    out.push_str(&format!(
        "/// QT-03 emit-shape version. Bumping is Specification-Required.\n\
         pub const QT_EMIT_VERSION: u32 = {QT_EMIT_VERSION_DATA};\n\n"
    ));
    out.push_str(&format!(
        "/// `qt-ir` schema version this module was generated from.\n\
         pub const QT_IR_VERSION: u32 = {};\n\n",
        module.version
    ));
    out.push_str(&format!(
        "/// Source `.qml` file path as recorded at emit time.\n\
         pub const QT_SOURCE: &str = {};\n\n",
        rust_str_lit(&module.source)
    ));

    out.push_str(NODE_TYPES);
    out.push('\n');
    out.push_str("/// Top-level screen tree, lowered from the source `.qml`.\n");
    out.push_str("///\n");
    out.push_str("/// `#[rustfmt::skip]` keeps the deterministic emit-shape\n");
    out.push_str("/// stable across `cargo fmt` runs. The shape is owned by\n");
    out.push_str("/// QT-03 §6 / §7; rustfmt's collapsing heuristics are not.\n");
    out.push_str("#[rustfmt::skip]\n");
    out.push_str("pub static SCREEN: Node = ");
    emit_node(&module.root, 0, &mut out);
    out.push_str(";\n");
    out
}

const NODE_TYPES: &str = "\
/// One QML type instance lowered to static data.
///
/// QT-03 carries only type name, optional id, expression-valued
/// assignments, and children. Object-valued assignments and list-valued
/// assignments are surfaced as preceding `// emitter-skipped:` comments
/// so reviewers can see what was elided. Property declarations,
/// signal declarations, and signal handlers remain in the upstream
/// `qt-ir.json` and are not yet lowered here — see
/// `docs/qt-support/03-rlvgl-emitter-widgets.md` §6.
#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub type_name: &'static str,
    pub id: Option<&'static str>,
    pub assignments: &'static [Assignment],
    pub children: &'static [Node],
}

/// One `target: <expression>` assignment whose value is an opaque QML
/// expression. Object-valued and list-valued assignments are not
/// represented here at QT-03 — see the comments preceding each `Node`
/// literal for the elided entries.
#[derive(Debug, Clone, Copy)]
pub struct Assignment {
    pub target: &'static str,
    pub value: &'static str,
}
";

fn emit_node(item: &UiItem, depth: usize, out: &mut String) {
    let pad = "    ".repeat(depth);
    let pad1 = "    ".repeat(depth + 1);
    out.push_str("Node {\n");

    out.push_str(&format!(
        "{pad1}type_name: {},\n",
        rust_str_lit(&item.type_name)
    ));
    match &item.id {
        Some(id) => out.push_str(&format!("{pad1}id: Some({}),\n", rust_str_lit(id))),
        None => out.push_str(&format!("{pad1}id: None,\n")),
    }

    if !item.properties.is_empty() {
        out.push_str(&format!(
            "{pad1}// emitter-skipped (QT-04+): {} property declaration(s)\n",
            item.properties.len()
        ));
    }
    if !item.signals.is_empty() {
        out.push_str(&format!(
            "{pad1}// emitter-skipped (QT-04+): {} signal declaration(s)\n",
            item.signals.len()
        ));
    }
    if !item.handlers.is_empty() {
        out.push_str(&format!(
            "{pad1}// emitter-skipped (QT-04+): {} signal handler(s)\n",
            item.handlers.len()
        ));
    }

    out.push_str(&format!("{pad1}assignments: &["));
    let expression_assignments: Vec<&UiAssignment> = item
        .assignments
        .iter()
        .filter(|a| matches!(a.value, UiAssignmentValue::Expression { .. }))
        .collect();
    let nontrivial_skipped: Vec<&UiAssignment> = item
        .assignments
        .iter()
        .filter(|a| !matches!(a.value, UiAssignmentValue::Expression { .. }))
        .collect();

    if expression_assignments.is_empty() && nontrivial_skipped.is_empty() {
        out.push_str("],\n");
    } else {
        out.push('\n');
        for skipped in &nontrivial_skipped {
            let kind = match skipped.value {
                UiAssignmentValue::Object { .. } => "object",
                UiAssignmentValue::List { .. } => "list",
                UiAssignmentValue::Expression { .. } => unreachable!(),
            };
            out.push_str(&format!(
                "{}    // emitter-skipped (QT-03b): {}: <{}>\n",
                pad1, skipped.target, kind
            ));
        }
        for asn in &expression_assignments {
            let value_text = match &asn.value {
                UiAssignmentValue::Expression { text } => text.as_str(),
                _ => unreachable!(),
            };
            // Multi-line form so the generated file is idempotent
            // under `cargo fmt`. See QT-03 §7 / §8.
            out.push_str(&format!("{pad1}    Assignment {{\n"));
            out.push_str(&format!(
                "{pad1}        target: {},\n",
                rust_str_lit(&asn.target)
            ));
            out.push_str(&format!(
                "{pad1}        value: {},\n",
                rust_str_lit(value_text)
            ));
            out.push_str(&format!("{pad1}    }},\n"));
        }
        out.push_str(&format!("{pad1}],\n"));
    }

    out.push_str(&format!("{pad1}children: &["));
    if item.children.is_empty() {
        out.push_str("],\n");
    } else {
        out.push('\n');
        for child in &item.children {
            out.push_str(&"    ".repeat(depth + 2));
            emit_node(child, depth + 2, out);
            out.push_str(",\n");
        }
        out.push_str(&format!("{pad1}],\n"));
    }

    out.push_str(&format!("{pad}}}"));
}

fn rust_str_lit(s: &str) -> String {
    // `{:?}` on &str produces a valid Rust string literal: wraps in
    // double quotes, escapes `"`, `\`, control chars, and unicode.
    format!("{s:?}")
}

// ============================================================================
// QT-03b: rlvgl-target emit (build_screen → WidgetNode)
// ============================================================================

/// Render a [`UiModule`] as the canonical QT-03b rlvgl-target Rust shape.
///
/// Public so the schema-drift test can compare in-memory output against
/// `tests/fixtures/qt/hello.rlvgl.rs` without going through a tempdir.
pub fn render_rlvgl(module: &UiModule) -> String {
    let state_fields = collect_state_fields(&module.root);
    let sm_id = module.state_machine.as_ref().map(|sm| sm.id.clone());
    let dm_field_ids: Vec<String> = module
        .state_machine
        .as_ref()
        .map(|sm| sm.datamodel.iter().map(|f| f.id.clone()).collect())
        .unwrap_or_default();
    let mut ctx = RlvglEmitCtx::new_with_fields(state_fields.clone())
        .with_sm(sm_id.clone())
        .with_dm_fields(dm_field_ids);
    let root_fn = ctx.alloc_fn_name(&module.root);
    let root_body = ctx.emit_helper(&module.root, &root_fn, true);
    let has_sm = sm_id.is_some();
    let used_dm_fields = ctx.used_dm_fields.clone();

    let mut out = String::new();
    out.push_str("// SPDX-License-Identifier: MIT\n");
    out.push_str("//\n");
    out.push_str(&format!(
        "// Generated by `rlvgl-creator qt emit --target rlvgl` from `{}`.\n",
        module.source
    ));
    out.push_str("// Do not edit by hand — regenerate with:\n");
    out.push_str("//   cargo run --features creator --bin rlvgl-creator -- \\\n");
    out.push_str("//       qt emit --target rlvgl <input.qml> <out_dir>\n");
    out.push_str("//\n");
    out.push_str("// Emit-shape contract: docs/qt-support/03b-rlvgl-widget-mapping.md\n");
    out.push_str("// QT-03b notes:\n");
    out.push_str("//   * helper functions named per §8 (`build_<sanitized_id>` /\n");
    out.push_str("//     `build_node_<index>`) so reviewers have stable handles.\n");
    out.push_str("//   * bounds resolved per §7 trivial path; non-`fill`/`margins`\n");
    out.push_str("//     anchors are deferred to QT-03c.\n");
    out.push_str("//   * property lowering per §6; unsupported lower to TODOs.\n");
    out.push_str("// QT-04b notes (docs/qt-support/04b-properties-bindings.md):\n");
    out.push_str("//   * `pub struct ScreenState` carries every QML root-level\n");
    out.push_str("//     property declaration (see §3 / §5).\n");
    out.push_str("//   * `build_screen` returns `(WidgetNode, Rc<RefCell<ScreenState>>)`\n");
    out.push_str("//     and threads `state` through every helper.\n");
    out.push_str("//   * Handler bodies that match the §7 grammar lower to\n");
    out.push_str("//     `state.borrow_mut()...` mutations under a `// QT-04b body:`\n");
    out.push_str("//     marker; non-matching bodies fall through to `// QT-04 body:`.\n");
    out.push('\n');
    out.push_str("#![allow(dead_code)]\n");
    // The emitter unconditionally imports every widget type the
    // mapping table can produce; per-fixture pruning would make the
    // emit shape diff-noisy for small QML edits.
    out.push_str("#![allow(unused_imports)]\n");
    // Helpers thread `state` through their parameter list even when
    // they do not consume it (only their children do). Per QT-04b §11.
    out.push_str("#![allow(unused_variables)]\n");
    out.push('\n');
    out.push_str("extern crate alloc;\n");
    out.push('\n');
    out.push_str("use alloc::rc::Rc;\n");
    out.push_str("use alloc::string::String;\n");
    out.push_str("use alloc::vec::Vec;\n");
    out.push_str("use core::cell::RefCell;\n");
    out.push('\n');
    // Emit order matches rustfmt's preferred sort within the
    // `rlvgl_core::*` group (uppercase items before lowercase
    // modules), so the generated file stays idempotent under
    // `cargo fmt`.
    out.push_str("use rlvgl_core::WidgetNode;\n");
    out.push_str("use rlvgl_core::widget::{Color, Rect, Widget};\n");
    out.push_str("use rlvgl_widgets::button::Button;\n");
    out.push_str("use rlvgl_widgets::click_area::ClickArea;\n");
    out.push_str("use rlvgl_widgets::container::Container;\n");
    out.push_str("use rlvgl_widgets::label::Label;\n");
    // QT-05b §6: import the istate-codegen 6-symbol linkage surface
    // (the ones we actually reference at this phase: `Event` for
    // dispatch lowering, `Machine` for the threading parameter).
    // `State`/`DataModel`/`Externals` join when QT-05c/e land.
    if let Some(id) = &sm_id {
        // QT-05c §3 / §6: SM-attached modules import the full v1
        // linkage surface trio that QT-05b/05c reference — `Event`
        // for dispatch lowering, `Machine` for the threading
        // parameter, `DataModel` for QT-05c MachineBinding accessors.
        // The file's `#![allow(unused_imports)]` covers fixtures
        // that have an SM but no DM bindings.
        out.push_str(&format!("use {id}_gen::{{DataModel, Event, Machine}};\n"));
    }
    out.push('\n');
    out.push_str(&format!(
        "/// rlvgl-target emit-shape version. Bumping is Specification-Required\n\
         /// (see `docs/qt-support/04b-properties-bindings.md` §11).\n\
         pub const QT_EMIT_VERSION: u32 = {QT_EMIT_VERSION_RLVGL};\n\n"
    ));
    out.push_str(&format!(
        "/// `qt-ir` schema version this module was generated from.\n\
         pub const QT_IR_VERSION: u32 = {};\n\n",
        module.version
    ));
    out.push_str(&format!(
        "/// Source `.qml` file path as recorded at emit time.\n\
         pub const QT_SOURCE: &str = {};\n\n",
        rust_str_lit(&module.source)
    ));

    // QT-05b §3 / §7: SM-attached modules emit the linkage version
    // and SM name as `pub const`s so reviewers can confirm what
    // istate template their module is built against.
    if let Some(id) = &sm_id {
        out.push_str(
            "/// QT-05 §6 linkage version. v1 pins the istate Rust\n\
             /// template's std-profile shape (VecDeque + Box<dyn Externals>).\n\
             pub const ISTATE_LINKAGE_VERSION: u32 = 1;\n\n",
        );
        out.push_str(&format!(
            "/// QT-05a §8 derived state-machine ID; matches the\n\
             /// `<sm>_gen` crate name stem.\n\
             pub const QT_SM_NAME: &str = {};\n\n",
            rust_str_lit(id)
        ));
    }

    emit_screen_state_struct(&state_fields, &mut out);
    emit_label_binding_struct(&mut out);
    if has_sm {
        emit_machine_binding_struct(&mut out);
        emit_binding_enum(&mut out);
    }

    if has_sm {
        out.push_str(
            "/// Build the screen widget tree at `bounds` and return it\n\
             /// alongside the `ScreenState` handle (QT-04b §3), the\n\
             /// `Rc<RefCell<Machine>>` istate-codegen handle (QT-05b §3),\n\
             /// and the `Vec<Binding>` of reactive bindings (QT-04e §3,\n\
             /// QT-05c §3). Callers may dispatch external events via\n\
             /// `machine.borrow_mut().dispatch(Event::…)` — the QML-side\n\
             /// `dispatch(\"…\")` handlers route through this same machine.\n\
             #[rustfmt::skip]\n\
             pub fn build_screen(\n    \
                 bounds: Rect,\n) \
             -> (WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<Binding>) {\n",
        );
        emit_screen_state_init(&state_fields, &mut out);
        out.push_str("    let machine = Rc::new(RefCell::new(Machine::new()));\n");
        out.push_str("    let mut bindings: Vec<Binding> = Vec::new();\n");
        out.push_str(&format!(
            "    let node = {root_fn}(bounds, Rc::clone(&state), Rc::clone(&machine), &mut bindings);\n    \
             (node, state, machine, bindings)\n}}\n\n"
        ));
    } else {
        out.push_str(
            "/// Build the screen widget tree at `bounds` and return it\n\
             /// alongside the `ScreenState` handle (QT-04b) and the\n\
             /// `Vec<LabelBinding>` of reactive bindings (QT-04e §3).\n\
             /// Callers ignore the third element with a `_` if reactivity\n\
             /// is not needed.\n\
             #[rustfmt::skip]\n\
             pub fn build_screen(\n    \
                 bounds: Rect,\n) \
             -> (WidgetNode, Rc<RefCell<ScreenState>>, Vec<LabelBinding>) {\n",
        );
        emit_screen_state_init(&state_fields, &mut out);
        out.push_str("    let mut label_bindings: Vec<LabelBinding> = Vec::new();\n");
        out.push_str(&format!(
            "    let node = {root_fn}(bounds, Rc::clone(&state), &mut label_bindings);\n    \
             (node, state, label_bindings)\n}}\n\n"
        ));
    }
    emit_refresh_bindings_fn(&mut out, has_sm);
    // QT-05c §6: per-field `format_dm_<field>` free functions —
    // one per used DM field, emitted in first-use order. The
    // `f64::to_string()` representation is chosen for determinism;
    // locale-aware / multi-field formatters are deferred per QT-05c §9.
    for field in &used_dm_fields {
        out.push_str(&format!(
            "/// QT-05c §6: f64::to_string accessor for the bound DM field.\n\
             #[inline]\n\
             fn format_dm_{field}(dm: &DataModel) -> String {{\n    \
                 use alloc::string::ToString;\n    \
                 dm.{field}.to_string()\n}}\n\n"
        ));
    }
    out.push_str(&root_body);

    // Trim the trailing blank line that comes from the last helper's
    // `}\n\n` separator, then add exactly one `\n` so the file ends
    // with a single newline. Keeps the emit byte-stable under
    // `cargo fmt`, which strips trailing blank lines.
    format!("{}\n", out.trim_end())
}

/// Per-emit context carrying the linear node index counter and the
/// accumulated helper bodies.
struct RlvglEmitCtx {
    node_index: u32,
    /// `ScreenState` field set in declaration order (one entry per
    /// QT-04b §5-supported QML property on the root). Used to type-
    /// check handler-body grammar lowering at emit time.
    state_fields: Vec<StateField>,
    /// QT-05b: `Some(<sm>)` when the IR has a populated state machine.
    /// Drives the 4-tuple `build_screen` shape, the `Rc<RefCell<Machine>>`
    /// helper parameter, and the `dispatch("…")` handler grammar.
    sm_id: Option<String>,
    /// QT-05c: snapshot of `state_machine.datamodel` IDs (`f64` only
    /// at linkage v1). Used to validate `text: sm.dm.<field>`
    /// binding references against the istate-codegen `DataModel`
    /// shape.
    dm_field_ids: Vec<String>,
    /// QT-05c: DM field names consumed by lowered MachineBindings,
    /// in first-use order. Emitted once each as `format_dm_<field>`
    /// free functions at the tail of `render_rlvgl` so the function
    /// definition order is byte-stable.
    used_dm_fields: Vec<String>,
}

impl RlvglEmitCtx {
    fn new_with_fields(state_fields: Vec<StateField>) -> Self {
        Self {
            node_index: 0,
            state_fields,
            sm_id: None,
            dm_field_ids: Vec::new(),
            used_dm_fields: Vec::new(),
        }
    }

    fn with_sm(mut self, sm_id: Option<String>) -> Self {
        self.sm_id = sm_id;
        self
    }

    fn with_dm_fields(mut self, dm_field_ids: Vec<String>) -> Self {
        self.dm_field_ids = dm_field_ids;
        self
    }

    /// QT-05b §3 — does this emit have a state machine attached?
    fn has_sm(&self) -> bool {
        self.sm_id.is_some()
    }

    /// Allocate a helper-function name per QT-03b §8.
    fn alloc_fn_name(&mut self, item: &UiItem) -> String {
        let name = match &item.id {
            Some(id) => format!("build_{}", sanitize_ident(id)),
            None => format!("build_node_{}", self.node_index),
        };
        self.node_index += 1;
        name
    }

    /// Recursively emit one helper function for `item` and all its
    /// descendants, returning the concatenated source text. `fn_name`
    /// is the name `alloc_fn_name` previously returned for `item` —
    /// passed in explicitly so siblings sharing the counter cannot
    /// shadow each other.
    fn emit_helper(&mut self, item: &UiItem, fn_name: &str, is_root: bool) -> String {
        let kind = map_qml_type(&item.type_name);

        // Pre-compute child names so the parent body can call them.
        let mut child_fns: Vec<(String, &UiItem)> = Vec::new();
        for child in &item.children {
            let name = self.alloc_fn_name(child);
            child_fns.push((name, child));
        }

        let has_children = !child_fns.is_empty();
        let mut_kw = if has_children { "mut " } else { "" };

        let mut out = String::new();
        out.push_str(&format!(
            "// QML type: `{}`{}\n",
            item.type_name,
            match &item.id {
                Some(id) => format!(" (id: `{id}`)"),
                None => String::new(),
            }
        ));
        // `#[rustfmt::skip]` keeps the deterministic emit shape stable
        // across `cargo fmt` runs. Same precedent as QT-03's SCREEN const.
        out.push_str("#[rustfmt::skip]\n");
        // QT-05b §3 / §6: helpers gain `machine: Rc<RefCell<Machine>>`
        // between `state` and `bindings` when a state machine is attached.
        // QT-05c §3: the binding-list parameter is `Vec<Binding>`
        // (sealed enum) when SM attached; QT-04e `Vec<LabelBinding>`
        // otherwise. Pre-QT-05b shape preserved when no SM.
        if self.has_sm() {
            out.push_str(&format!(
                "fn {fn_name}(\n    bounds: Rect,\n    state: Rc<RefCell<ScreenState>>,\n    \
                 machine: Rc<RefCell<Machine>>,\n    \
                 bindings: &mut Vec<Binding>,\n) -> WidgetNode {{\n"
            ));
        } else {
            out.push_str(&format!(
                "fn {fn_name}(\n    bounds: Rect,\n    state: Rc<RefCell<ScreenState>>,\n    \
                 label_bindings: &mut Vec<LabelBinding>,\n) -> WidgetNode {{\n"
            ));
        }

        emit_widget_construction(
            &kind,
            item,
            &self.state_fields,
            self.sm_id.as_deref(),
            &self.dm_field_ids,
            &mut self.used_dm_fields,
            &mut out,
        );
        emit_skipped_summary(item, is_root, &self.state_fields, &mut out);

        let tag_lit = match &item.id {
            Some(id) => format!("Some({})", rust_str_lit(id)),
            None => "None".to_string(),
        };
        out.push_str(&format!(
            "    let {mut_kw}node = WidgetNode {{\n        \
             widget,\n        children: Vec::new(),\n        tag: {tag_lit},\n    }};\n"
        ));

        for (child_name, child) in &child_fns {
            emit_child_bounds(child, &mut out);
            // QT-05b/05c: thread `Rc::clone(&machine)` and the
            // sealed `Vec<Binding>` into the child call when a state
            // machine is attached.
            if self.has_sm() {
                out.push_str(&format!(
                    "    node.children.push({child_name}(child_bounds, Rc::clone(&state), Rc::clone(&machine), bindings));\n"
                ));
            } else {
                out.push_str(&format!(
                    "    node.children.push({child_name}(child_bounds, Rc::clone(&state), label_bindings));\n"
                ));
            }
        }

        out.push_str("    node\n}\n\n");

        // Append child helpers afterwards (depth-first, declaration
        // order matches the parent's child list).
        for (name, child) in child_fns {
            out.push_str(&self.emit_helper(child, &name, false));
        }
        out
    }
}

#[derive(Debug)]
enum WidgetKind {
    Container,
    Label,
    /// QT-04: lowers QML `Button` / `QC.Button` to
    /// [`rlvgl_widgets::button::Button`] with optional `set_on_click`
    /// wiring. See `docs/qt-support/04-signal-handlers.md` §5.
    Button,
    /// QT-04d: lowers QML `MouseArea` to
    /// [`rlvgl_widgets::click_area::ClickArea`] (transparent click
    /// region) with optional `set_on_click` wiring. See
    /// `docs/qt-support/04d-mousearea.md` §5.
    ClickArea,
    Fallback,
}

fn map_qml_type(name: &str) -> WidgetKind {
    match name {
        // QT-03b §5: explicit Container mappings.
        "Item" | "Rectangle" => WidgetKind::Container,

        // QT-03b §5: Text-family. `QC.Label` is the alias-resolved
        // form produced by QT-01a when the QML uses `import
        // QtQuick.Controls as QC`.
        "Text" | "Label" | "QC.Label" => WidgetKind::Label,

        // QT-04 §10 / §5: Button row promoted from Container
        // fallback to a typed mapping with handler support.
        "Button" | "QC.Button" => WidgetKind::Button,

        // QT-04d §5: MouseArea row promoted from Container fallback
        // to a typed ClickArea mapping with handler support.
        "MouseArea" => WidgetKind::ClickArea,

        // QT-03b §5: Column/Row remain Container fallbacks at
        // QT-03b initial implementation; `VStack`/`HStack`
        // lowering is deferred (per-child-height constructor needs
        // a richer bounds pass than the §7 trivial rule).
        "Column" | "Row" => WidgetKind::Container,

        // Anything else (CheckBox, Switch, Slider, ProgressBar,
        // Image, MouseArea, user-defined types) lowers to a
        // Container fallback at QT-03b initial implementation.
        _ => WidgetKind::Fallback,
    }
}

fn emit_widget_construction(
    kind: &WidgetKind,
    item: &UiItem,
    state_fields: &[StateField],
    sm_id: Option<&str>,
    dm_field_ids: &[String],
    used_dm_fields: &mut Vec<String>,
    out: &mut String,
) {
    match kind {
        WidgetKind::Container => {
            let color = lookup_assignment(item, "color").and_then(parse_qml_color_lit);
            if color.is_some() || lookup_assignment(item, "color").is_some() {
                out.push_str("    let mut w = Container::new(bounds);\n");
                if let Some((r, g, b, a)) = color {
                    out.push_str(&format!(
                        "    w.style.bg_color = Color({r:#04x}, {g:#04x}, {b:#04x}, {a:#04x});\n"
                    ));
                } else {
                    out.push_str("    // TODO QT-04e: bind color (non-literal QML expression)\n");
                }
                out.push_str(
                    "    let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(w));\n",
                );
            } else {
                out.push_str(
                    "    let widget: Rc<RefCell<dyn Widget>> =\n        \
                     Rc::new(RefCell::new(Container::new(bounds)));\n",
                );
            }
        }
        WidgetKind::Label => {
            let raw_text = lookup_assignment(item, "text");
            let text_lit = raw_text.and_then(parse_string_literal);
            if let Some(text) = text_lit {
                out.push_str(&format!(
                    "    let widget: Rc<RefCell<dyn Widget>> =\n        \
                     Rc::new(RefCell::new(Label::new({}, bounds)));\n",
                    rust_str_lit(&text)
                ));
            } else if sm_id.is_some()
                && let Some(field) = raw_text.and_then(parse_dm_text_ref)
            {
                // QT-05c §5/§6: `text: sm.dm.<field>` lowers to a
                // MachineBinding. Validate `<field>` against the
                // istate datamodel; unknown is an emit-time error.
                if !dm_field_ids.iter().any(|f| f == &field) {
                    panic!(
                        "QT-05c §5: Label `text: sm.dm.{field}` references an unknown DataModel \
                         field. Known fields: {dm_field_ids:?}. Either add the field to the \
                         scjson <datamodel> or fix the QML reference."
                    );
                }
                if !used_dm_fields.iter().any(|f| f == &field) {
                    used_dm_fields.push(field.clone());
                }
                out.push_str(&format!(
                    "    // QT-05c machine-bound: text → sm.dm.{field}\n"
                ));
                out.push_str(&format!(
                    "    let label_handle: Rc<RefCell<Label>> = Rc::new(RefCell::new(\n        \
                     Label::new(\n            \
                         {{ let m = machine.borrow(); format_dm_{field}(&m.dm) }},\n        \
                         bounds,\n    ),\n    ));\n"
                ));
                out.push_str("    let widget: Rc<RefCell<dyn Widget>> = label_handle.clone();\n");
                out.push_str(&format!(
                    "    bindings.push(Binding::Machine(MachineBinding {{\n        \
                     label: Rc::clone(&label_handle),\n        \
                     accessor: format_dm_{field},\n    }}));\n"
                ));
            } else if let Some(field) =
                raw_text.and_then(|expr| resolve_string_state_ref(expr, state_fields))
            {
                // QT-04e §6: keep a concrete Rc<RefCell<Label>> handle
                // and push a LabelBinding so refresh_bindings can
                // re-apply the binding after state mutations.
                out.push_str(&format!("    // QT-04c bound: text → state.{field}\n"));
                out.push_str(&format!(
                    "    let label_handle: Rc<RefCell<Label>> = Rc::new(RefCell::new(\n        \
                     Label::new(state.borrow().{field}.clone(), bounds),\n    ));\n"
                ));
                out.push_str("    let widget: Rc<RefCell<dyn Widget>> = label_handle.clone();\n");
                out.push_str(&format!(
                    "    // QT-04e bound: refresh state.{field} → label.set_text\n"
                ));
                if sm_id.is_some() {
                    // QT-05c §3: when SM attached, the binding list
                    // is `Vec<Binding>` and LabelBindings wrap as
                    // `Binding::Label(...)`.
                    out.push_str(&format!(
                        "    bindings.push(Binding::Label(LabelBinding {{\n        \
                         label: Rc::clone(&label_handle),\n        \
                         accessor: |s| s.{field}.clone(),\n    }}));\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "    label_bindings.push(LabelBinding {{\n        \
                         label: Rc::clone(&label_handle),\n        \
                         accessor: |s| s.{field}.clone(),\n    }});\n"
                    ));
                }
            } else {
                out.push_str(
                    "    // TODO QT-04e: reactive bind text (non-literal QML expression)\n",
                );
                out.push_str(
                    "    let widget: Rc<RefCell<dyn Widget>> =\n        \
                     Rc::new(RefCell::new(Label::new(\"\", bounds)));\n",
                );
            }
        }
        WidgetKind::Button => {
            let raw_text = lookup_assignment(item, "text");
            let (ctor_arg, bound_marker) =
                if let Some(text) = raw_text.and_then(parse_string_literal) {
                    (rust_str_lit(&text), None)
                } else if let Some(field) =
                    raw_text.and_then(|expr| resolve_string_state_ref(expr, state_fields))
                {
                    (
                        format!("state.borrow().{field}.clone()"),
                        Some(format!("    // QT-04c bound: text → state.{field}\n")),
                    )
                } else {
                    out.push_str(
                        "    // TODO QT-04e: reactive bind text (non-literal QML expression)\n",
                    );
                    ("\"\"".to_string(), None)
                };
            if let Some(marker) = bound_marker {
                out.push_str(&marker);
            }
            out.push_str(&format!(
                "    let mut button = Button::new({ctor_arg}, bounds);\n"
            ));
            for handler in item.handlers.iter().filter(|h| h.signal == "onClicked") {
                emit_qt04b_or_qt04_handler(&handler.body, state_fields, sm_id, out);
            }
            out.push_str(
                "    let widget: Rc<RefCell<dyn Widget>> = \
                 Rc::new(RefCell::new(button));\n",
            );
        }
        WidgetKind::ClickArea => {
            out.push_str("    let mut click_area = ClickArea::new(bounds);\n");
            for handler in item.handlers.iter().filter(|h| h.signal == "onClicked") {
                emit_qt04b_or_qt04_handler_for(
                    "click_area",
                    &handler.body,
                    state_fields,
                    sm_id,
                    out,
                );
            }
            out.push_str(
                "    let widget: Rc<RefCell<dyn Widget>> = \
                 Rc::new(RefCell::new(click_area));\n",
            );
        }
        WidgetKind::Fallback => {
            out.push_str(&format!(
                "    // emitter-fallback (QT-03b): unmapped QML type `{}`\n",
                item.type_name
            ));
            out.push_str(
                "    let widget: Rc<RefCell<dyn Widget>> =\n        \
                 Rc::new(RefCell::new(Container::new(bounds)));\n",
            );
        }
    }
}

/// Emit a per-line `// QT-04 body: ...` comment block above a
/// `set_on_click` call. Body is the verbatim QML handler text per
/// QT-04 §7. Empty bodies emit nothing.
fn emit_qt04_handler_body(body: &str, out: &mut String) {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return;
    }
    for line in trimmed.lines() {
        out.push_str(&format!("    // QT-04 body: {line}\n"));
    }
}

/// Count of handlers that the emitter lowers for this item. Per
/// QT-04 §6 + QT-04d §6, currently only `onClicked` on a Button or
/// ClickArea (MouseArea) widget. Used by [`emit_skipped_summary`]
/// to subtract from the elided-handler count.
fn lowered_handler_count(item: &UiItem) -> usize {
    if matches!(
        map_qml_type(&item.type_name),
        WidgetKind::Button | WidgetKind::ClickArea
    ) {
        item.handlers
            .iter()
            .filter(|h| h.signal == "onClicked")
            .count()
    } else {
        0
    }
}

fn emit_skipped_summary(
    item: &UiItem,
    is_root: bool,
    state_fields: &[StateField],
    out: &mut String,
) {
    let lowered_props = item
        .properties
        .iter()
        .filter(|p| {
            state_fields.iter().any(|f| {
                f.qml_prop == p.name
                    && match (is_root, &f.owner_id) {
                        (true, None) => true,
                        (false, Some(id)) => Some(id) == item.id.as_ref(),
                        _ => false,
                    }
            })
        })
        .count();
    let skipped_props = item.properties.len().saturating_sub(lowered_props);
    if skipped_props > 0 {
        out.push_str(&format!(
            "    // emitter-skipped (QT-04c+): {skipped_props} property declaration(s)\n",
        ));
    }
    if !item.signals.is_empty() {
        out.push_str(&format!(
            "    // emitter-skipped (QT-04+): {} signal declaration(s)\n",
            item.signals.len()
        ));
    }
    let skipped_handlers = item
        .handlers
        .len()
        .saturating_sub(lowered_handler_count(item));
    if skipped_handlers > 0 {
        out.push_str(&format!(
            "    // emitter-skipped (QT-04+): {skipped_handlers} signal handler(s)\n",
        ));
    }
}

/// Emit a `let child_bounds = …;` line per QT-03b §7 trivial path.
fn emit_child_bounds(child: &UiItem, out: &mut String) {
    let fill = lookup_assignment(child, "anchors.fill")
        .map(|s| s.trim() == "parent")
        .unwrap_or(false);
    let margins = lookup_assignment(child, "anchors.margins").and_then(parse_int_literal);
    let center_in = lookup_assignment(child, "anchors.centerIn")
        .map(|s| s.trim() == "parent")
        .unwrap_or(false);

    // QT-03c §5 amendment (2026-04-29): single edge anchors.
    // Detected here so the §7 evaluation order can reach them after
    // fill / centerIn but before the default trivial path.
    let anchor_left = lookup_assignment(child, "anchors.left").map(|s| s.trim().to_string());
    let anchor_right = lookup_assignment(child, "anchors.right").map(|s| s.trim().to_string());
    let anchor_top = lookup_assignment(child, "anchors.top").map(|s| s.trim().to_string());
    let anchor_bottom = lookup_assignment(child, "anchors.bottom").map(|s| s.trim().to_string());

    // Combined edge anchors fall through (QT-03c §5 amendment row
    // notes). Detect combinations early so the per-edge path can
    // refuse them cleanly.
    let edge_count = [&anchor_left, &anchor_right, &anchor_top, &anchor_bottom]
        .iter()
        .filter(|a| a.is_some())
        .count();
    let single_edge = edge_count == 1;
    // QT-03c §5 amendment #2: corner = exactly one X-axis edge plus
    // exactly one Y-axis edge. Other 2-edge combos (left+right or
    // top+bottom) are axial fills and remain deferred.
    let x_edge_count = anchor_left.is_some() as usize + anchor_right.is_some() as usize;
    let y_edge_count = anchor_top.is_some() as usize + anchor_bottom.is_some() as usize;
    let corner_edge = edge_count == 2 && x_edge_count == 1 && y_edge_count == 1;

    // Surface every unhandled anchor assignment as a comment per
    // QT-03c §3, before any bounds-resolution path emits its own
    // output. The §7 evaluation order then runs.
    for a in &child.assignments {
        if !a.target.starts_with("anchors.") {
            continue;
        }
        let lowered = matches!(
            a.target.as_str(),
            "anchors.fill" | "anchors.margins" | "anchors.centerIn"
        ) || ((single_edge || corner_edge)
            && matches!(
                a.target.as_str(),
                "anchors.left" | "anchors.right" | "anchors.top" | "anchors.bottom"
            ));
        if lowered {
            continue;
        }
        if let UiAssignmentValue::Expression { text } = &a.value {
            out.push_str(&format!(
                "    // emitter-skipped (QT-03c+): {}: {}\n",
                a.target,
                text.trim()
            ));
        }
    }

    // QT-03c §7 step 1: anchors.fill wins over everything.
    if fill {
        if center_in {
            out.push_str("    // QT-03c override: anchors.fill supersedes anchors.centerIn\n");
        }
        if let Some(m) = margins {
            out.push_str(&format!(
                "    let child_bounds = Rect {{\n        \
                 x: bounds.x + {m},\n        y: bounds.y + {m},\n        \
                 width: bounds.width - 2 * {m},\n        \
                 height: bounds.height - 2 * {m},\n    }};\n"
            ));
            return;
        }
        out.push_str("    let child_bounds = bounds;\n");
        return;
    }

    let x_lit = lookup_assignment(child, "x").and_then(parse_int_literal);
    let y_lit = lookup_assignment(child, "y").and_then(parse_int_literal);
    let w_lit = lookup_assignment(child, "width").and_then(parse_int_literal);
    let h_lit = lookup_assignment(child, "height").and_then(parse_int_literal);

    // QT-03c §7 step 2: anchors.centerIn with literal width+height.
    if center_in {
        match (w_lit, h_lit) {
            (Some(w), Some(h)) => {
                if x_lit.is_some() || y_lit.is_some() {
                    out.push_str(&format!(
                        "    // QT-03c override: anchors.centerIn supersedes literal x: {}, y: {}\n",
                        x_lit.unwrap_or(0),
                        y_lit.unwrap_or(0),
                    ));
                }
                out.push_str(&format!(
                    "    // QT-03c centered: anchors.centerIn: parent (child {w}×{h})\n"
                ));
                out.push_str(&format!(
                    "    let child_bounds = Rect {{\n        \
                     x: bounds.x + (bounds.width - {w}) / 2,\n        \
                     y: bounds.y + (bounds.height - {h}) / 2,\n        \
                     width: {w},\n        height: {h},\n    }};\n"
                ));
                return;
            }
            _ => {
                out.push_str(
                    "    // QT-03c centerIn: parent (no explicit size — defaulted to parent bounds)\n",
                );
                // Falls through to default path with width/height inherited from parent.
            }
        }
    }

    // QT-03c §5 amendment 2026-04-29: single edge anchor lowers
    // ahead of the default trivial path. Combined edge anchors
    // (`edge_count > 1`) fall through to the trivial path; the
    // skipped-anchor comments above already surfaced them.
    if single_edge {
        let edge_lowered = lower_single_edge_anchor(
            anchor_left.as_deref(),
            anchor_right.as_deref(),
            anchor_top.as_deref(),
            anchor_bottom.as_deref(),
            x_lit,
            y_lit,
            w_lit,
            h_lit,
            out,
        );
        if edge_lowered {
            return;
        }
    }

    // QT-03c §5 amendment #2 (2026-04-29): corner combinations.
    if corner_edge {
        let corner_lowered = lower_corner_anchor(
            anchor_left.as_deref(),
            anchor_right.as_deref(),
            anchor_top.as_deref(),
            anchor_bottom.as_deref(),
            w_lit,
            h_lit,
            out,
        );
        if corner_lowered {
            return;
        }
    }

    // QT-03c §7 step 3: default trivial path (literal x/y/w/h or
    // parent-inherited).
    let x = x_lit.unwrap_or(0);
    let y = y_lit.unwrap_or(0);
    let width = match w_lit {
        Some(n) => format!("{n}"),
        None => "bounds.width".to_string(),
    };
    let height = match h_lit {
        Some(n) => format!("{n}"),
        None => "bounds.height".to_string(),
    };

    out.push_str(&format!(
        "    let child_bounds = Rect {{\n        \
         x: bounds.x + {x},\n        y: bounds.y + {y},\n        \
         width: {width},\n        height: {height},\n    }};\n"
    ));
}

/// Try to lower a single edge anchor per the QT-03c §5 amendment
/// (2026-04-29). Returns `true` when a `let child_bounds = …;` block
/// was emitted; `false` when the anchor's value form was not
/// `parent.<edge>` or required dimensions were missing — in which
/// case the caller falls through to the default trivial path.
#[allow(clippy::too_many_arguments)]
fn lower_single_edge_anchor(
    left: Option<&str>,
    right: Option<&str>,
    top: Option<&str>,
    bottom: Option<&str>,
    x_lit: Option<i32>,
    y_lit: Option<i32>,
    w_lit: Option<i32>,
    h_lit: Option<i32>,
    out: &mut String,
) -> bool {
    let width_expr = match w_lit {
        Some(n) => format!("{n}"),
        None => "bounds.width".to_string(),
    };
    let height_expr = match h_lit {
        Some(n) => format!("{n}"),
        None => "bounds.height".to_string(),
    };

    if let Some(v) = left {
        if v != "parent.left" {
            return false;
        }
        let y = y_lit.unwrap_or(0);
        out.push_str("    // QT-03c edge: anchors.left: parent.left\n");
        out.push_str(&format!(
            "    let child_bounds = Rect {{\n        \
             x: bounds.x,\n        y: bounds.y + {y},\n        \
             width: {width_expr},\n        height: {height_expr},\n    }};\n"
        ));
        return true;
    }
    if let Some(v) = right {
        if v != "parent.right" {
            return false;
        }
        // `parent.right` requires literal width to position child.
        let Some(w) = w_lit else {
            out.push_str(
                "    // emitter-skipped (QT-03c+): anchors.right: parent.right (no literal width)\n",
            );
            return false;
        };
        let y = y_lit.unwrap_or(0);
        out.push_str("    // QT-03c edge: anchors.right: parent.right\n");
        out.push_str(&format!(
            "    let child_bounds = Rect {{\n        \
             x: bounds.x + bounds.width - {w},\n        y: bounds.y + {y},\n        \
             width: {w},\n        height: {height_expr},\n    }};\n"
        ));
        return true;
    }
    if let Some(v) = top {
        if v != "parent.top" {
            return false;
        }
        let x = x_lit.unwrap_or(0);
        out.push_str("    // QT-03c edge: anchors.top: parent.top\n");
        out.push_str(&format!(
            "    let child_bounds = Rect {{\n        \
             x: bounds.x + {x},\n        y: bounds.y,\n        \
             width: {width_expr},\n        height: {height_expr},\n    }};\n"
        ));
        return true;
    }
    if let Some(v) = bottom {
        if v != "parent.bottom" {
            return false;
        }
        let Some(h) = h_lit else {
            out.push_str(
                "    // emitter-skipped (QT-03c+): anchors.bottom: parent.bottom (no literal height)\n",
            );
            return false;
        };
        let x = x_lit.unwrap_or(0);
        out.push_str("    // QT-03c edge: anchors.bottom: parent.bottom\n");
        out.push_str(&format!(
            "    let child_bounds = Rect {{\n        \
             x: bounds.x + {x},\n        y: bounds.y + bounds.height - {h},\n        \
             width: {width_expr},\n        height: {h},\n    }};\n"
        ));
        return true;
    }
    false
}

/// Lower a corner-edge anchor combination per the QT-03c §5 amendment
/// #2 (2026-04-29). Caller has confirmed exactly one X-axis edge
/// (`anchor_left` or `anchor_right`) and one Y-axis edge
/// (`anchor_top` or `anchor_bottom`) are set. Mismatched value forms
/// or missing literal dimensions return `false` so the caller falls
/// through to the trivial path.
#[allow(clippy::too_many_arguments)]
fn lower_corner_anchor(
    left: Option<&str>,
    right: Option<&str>,
    top: Option<&str>,
    bottom: Option<&str>,
    w_lit: Option<i32>,
    h_lit: Option<i32>,
    out: &mut String,
) -> bool {
    // X-axis component → produces (label, x-expr, width-expr).
    let (x_label, x_expr, width_expr) = if let Some(v) = left {
        if v != "parent.left" {
            return false;
        }
        let width_expr = match w_lit {
            Some(n) => format!("{n}"),
            None => "bounds.width".to_string(),
        };
        ("left", "bounds.x".to_string(), width_expr)
    } else if let Some(v) = right {
        if v != "parent.right" {
            return false;
        }
        let Some(w) = w_lit else {
            out.push_str(
                "    // emitter-skipped (QT-03c+): anchors.right: parent.right (no literal width)\n",
            );
            return false;
        };
        (
            "right",
            format!("bounds.x + bounds.width - {w}"),
            format!("{w}"),
        )
    } else {
        return false;
    };

    // Y-axis component → produces (label, y-expr, height-expr).
    let (y_label, y_expr, height_expr) = if let Some(v) = top {
        if v != "parent.top" {
            return false;
        }
        let height_expr = match h_lit {
            Some(n) => format!("{n}"),
            None => "bounds.height".to_string(),
        };
        ("top", "bounds.y".to_string(), height_expr)
    } else if let Some(v) = bottom {
        if v != "parent.bottom" {
            return false;
        }
        let Some(h) = h_lit else {
            out.push_str(
                "    // emitter-skipped (QT-03c+): anchors.bottom: parent.bottom (no literal height)\n",
            );
            return false;
        };
        (
            "bottom",
            format!("bounds.y + bounds.height - {h}"),
            format!("{h}"),
        )
    } else {
        return false;
    };

    out.push_str(&format!(
        "    // QT-03c corner: anchors.{x_label}+anchors.{y_label}\n"
    ));
    out.push_str(&format!(
        "    let child_bounds = Rect {{\n        \
         x: {x_expr},\n        y: {y_expr},\n        \
         width: {width_expr},\n        height: {height_expr},\n    }};\n"
    ));
    true
}

fn lookup_assignment<'a>(item: &'a UiItem, target: &str) -> Option<&'a str> {
    item.assignments.iter().find_map(|a| {
        if a.target != target {
            return None;
        }
        match &a.value {
            UiAssignmentValue::Expression { text } => Some(text.as_str()),
            _ => None,
        }
    })
}

/// Parse a QML literal string `"..."` and return its inner text. Returns
/// None for any non-literal expression (e.g. `root.title`).
fn parse_string_literal(expr: &str) -> Option<String> {
    let s = expr.trim();
    if s.len() < 2 {
        return None;
    }
    let bytes = s.as_bytes();
    let q = bytes[0];
    if (q != b'"' && q != b'\'') || bytes[bytes.len() - 1] != q {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    // Reject embedded quote chars to keep the literal-detection
    // honest — anything more elaborate is not "literal" for our
    // purposes and should fall back to the TODO path.
    if inner.bytes().any(|b| b == q) {
        return None;
    }
    Some(inner.to_string())
}

/// Parse a QML literal int (`16`, `-3`). Returns None for non-literal expressions.
fn parse_int_literal(expr: &str) -> Option<i32> {
    expr.trim().parse::<i32>().ok()
}

/// Parse a QML literal color string `"#RRGGBB"` or `"#AARRGGBB"`.
/// Returns `(r, g, b, a)`. Named colours and bindings return None.
fn parse_qml_color_lit(expr: &str) -> Option<(u8, u8, u8, u8)> {
    let s = parse_string_literal(expr)?;
    let s = s.strip_prefix('#')?;
    let parse_byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    match s.len() {
        6 => Some((parse_byte(0)?, parse_byte(2)?, parse_byte(4)?, 0xff)),
        8 => Some((
            parse_byte(2)?,
            parse_byte(4)?,
            parse_byte(6)?,
            parse_byte(0)?,
        )),
        _ => None,
    }
}

/// One emitted `ScreenState` field, paired with the type info needed
/// to type-check QT-04b §7 handler-body grammar at emit time.
#[derive(Debug, Clone)]
struct StateField {
    /// Rust field name. For root-scope properties (QT-04b §8) this
    /// is the bare QML property name; for non-root id'd items
    /// (QT-04f §5) it is `<sanitized_id>_<prop>`.
    name: String,
    /// QML property name (used by the resolver to match
    /// `<id>.<prop>` references against the source declaration).
    qml_prop: String,
    /// Source `id:` of the item that declared this property. `None`
    /// for properties on the root (which are accessed un-namespaced
    /// per QT-04b §8). `Some("foo")` for non-root id'd items per
    /// QT-04f §5.
    owner_id: Option<String>,
    /// Resolved Rust type per QT-04b §5.
    ty: StateFieldType,
    /// Rust expression that initialises the field in `build_screen`.
    init_expr: String,
    /// `Some(comment)` when the QML default was non-literal; emitted
    /// above the field initializer per QT-04b §6.
    init_comment: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum StateFieldType {
    I32,
    F32,
    Bool,
    StringTy,
}

impl StateFieldType {
    fn rust(self) -> &'static str {
        match self {
            StateFieldType::I32 => "i32",
            StateFieldType::F32 => "f32",
            StateFieldType::Bool => "bool",
            StateFieldType::StringTy => "String",
        }
    }

    fn from_qml(qml_ty: &str) -> Option<Self> {
        match qml_ty {
            "int" => Some(StateFieldType::I32),
            "real" | "double" => Some(StateFieldType::F32),
            "bool" => Some(StateFieldType::Bool),
            "string" => Some(StateFieldType::StringTy),
            _ => None,
        }
    }

    fn rust_default(self) -> &'static str {
        match self {
            StateFieldType::I32 => "0",
            StateFieldType::F32 => "0.0",
            StateFieldType::Bool => "false",
            StateFieldType::StringTy => "String::new()",
        }
    }
}

/// Walk a root `UiItem` and produce the ScreenState field list per
/// QT-04b §3 / §5 + QT-04f §5. Root properties are un-namespaced;
/// non-root id'd items contribute `<sanitized_id>_<prop>` fields.
/// Unsupported types are skipped.
fn collect_state_fields(root: &UiItem) -> Vec<StateField> {
    let mut out = Vec::new();
    let mut seen_field_names = Vec::<String>::new();
    collect_state_fields_walk(root, true, &mut out, &mut seen_field_names);
    out
}

fn collect_state_fields_walk(
    item: &UiItem,
    is_root: bool,
    out: &mut Vec<StateField>,
    seen: &mut Vec<String>,
) {
    let owner_id = if is_root { None } else { item.id.clone() };
    for prop in &item.properties {
        let Some(ty) = StateFieldType::from_qml(&prop.ty) else {
            continue;
        };
        // QT-04f §5: non-root items without an `id:` cannot
        // contribute uniquely-resolvable references, so their
        // properties stay in the per-node skipped-summary count
        // rather than ScreenState.
        if !is_root && owner_id.is_none() {
            continue;
        }
        let field_name = match &owner_id {
            None => prop.name.clone(),
            Some(id) => format!("{}_{}", sanitize_ident(id), prop.name),
        };
        if seen.contains(&field_name) {
            // §5 collision rule: rare; emit a comment-bearing
            // placeholder rather than aborting the build, but keep
            // the field collision visible.
            continue;
        }
        seen.push(field_name.clone());
        let (init_expr, init_comment) = lower_property_default(prop, ty);
        out.push(StateField {
            name: field_name,
            qml_prop: prop.name.clone(),
            owner_id: owner_id.clone(),
            ty,
            init_expr,
            init_comment,
        });
    }
    for child in &item.children {
        collect_state_fields_walk(child, false, out, seen);
    }
}

fn lower_property_default(prop: &UiProperty, ty: StateFieldType) -> (String, Option<String>) {
    let Some(default) = prop.default_value.as_deref() else {
        return (ty.rust_default().to_string(), None);
    };
    match ty {
        StateFieldType::I32 => match parse_int_literal(default) {
            Some(n) => (format!("{n}"), None),
            None => (
                ty.rust_default().to_string(),
                Some(format!(
                    "QT-04b: non-literal default for `{}`: {}",
                    prop.name, default
                )),
            ),
        },
        StateFieldType::F32 => match parse_float_literal(default) {
            Some(f) => {
                let mut s = format!("{f}");
                if !s.contains('.') {
                    s.push_str(".0");
                }
                (s, None)
            }
            None => (
                ty.rust_default().to_string(),
                Some(format!(
                    "QT-04b: non-literal default for `{}`: {}",
                    prop.name, default
                )),
            ),
        },
        StateFieldType::Bool => match parse_bool_literal(default) {
            Some(b) => (format!("{b}"), None),
            None => (
                ty.rust_default().to_string(),
                Some(format!(
                    "QT-04b: non-literal default for `{}`: {}",
                    prop.name, default
                )),
            ),
        },
        StateFieldType::StringTy => match parse_string_literal(default) {
            Some(s) => (format!("String::from({})", rust_str_lit(&s)), None),
            None => (
                ty.rust_default().to_string(),
                Some(format!(
                    "QT-04b: non-literal default for `{}`: {}",
                    prop.name, default
                )),
            ),
        },
    }
}

/// Emit `pub struct ScreenState { ... }` per QT-04b §3. Always emits,
/// even when there are no fields, so consumers see a stable shape.
fn emit_screen_state_struct(fields: &[StateField], out: &mut String) {
    out.push_str(
        "/// State threaded through every helper. One field per\n\
         /// QT-04b §5-supported property declared on the QML root.\n\
         /// `#[rustfmt::skip]` so the field order matches QT-01a's IR\n\
         /// declaration order rather than rustfmt's alphabetical sort.\n\
         #[rustfmt::skip]\n\
         #[derive(Debug, Clone)]\n\
         pub struct ScreenState {\n",
    );
    if fields.is_empty() {
        out.push_str("    // QT-04b: root declares no QT-04b §5-supported properties.\n");
    } else {
        for f in fields {
            out.push_str(&format!("    pub {}: {},\n", f.name, f.ty.rust()));
        }
    }
    out.push_str("}\n\n");
}

/// Emit the `let state = Rc::new(RefCell::new(ScreenState { ... }));`
/// initializer at the top of `build_screen`.
fn emit_screen_state_init(fields: &[StateField], out: &mut String) {
    if fields.is_empty() {
        out.push_str("    let state = Rc::new(RefCell::new(ScreenState {}));\n");
        return;
    }
    out.push_str("    let state = Rc::new(RefCell::new(ScreenState {\n");
    for f in fields {
        if let Some(comment) = &f.init_comment {
            out.push_str(&format!("        // {comment}\n"));
        }
        out.push_str(&format!("        {}: {},\n", f.name, f.init_expr));
    }
    out.push_str("    }));\n");
}

/// Emit the QT-04e `pub struct LabelBinding` + `impl LabelBinding`
/// pair. Always emitted (even when no Labels are bound) so the
/// per-fixture API stays stable.
fn emit_label_binding_struct(out: &mut String) {
    out.push_str(
        "/// Reactive Label-text binding emitted by QT-04e §3. Each\n\
         /// entry pairs a concrete `Rc<RefCell<Label>>` with an\n\
         /// accessor that reads the bound state field. Use\n\
         /// [`refresh_bindings`] to re-apply every binding in one call.\n\
         pub struct LabelBinding {\n    \
             pub label: Rc<RefCell<Label>>,\n    \
             pub accessor: fn(&ScreenState) -> String,\n}\n\n\
         impl LabelBinding {\n    \
             /// Re-apply this binding from the supplied state.\n    \
             pub fn refresh(&self, state: &ScreenState) {\n        \
                 self.label.borrow_mut().set_text((self.accessor)(state));\n    \
             }\n}\n\n",
    );
}

/// QT-05c §3 — `pub struct MachineBinding` + `pub enum Binding`
/// emitted on SM-attached modules. Mirrors the `LabelBinding` shape
/// with `DataModel` as the accessor source.
fn emit_machine_binding_struct(out: &mut String) {
    out.push_str(
        "/// QT-05c §3 — reactive Label-text binding sourced from the\n\
         /// state-machine `DataModel`. Mirrors `LabelBinding` with\n\
         /// `&DataModel` instead of `&ScreenState` as the accessor input.\n\
         pub struct MachineBinding {\n    \
             pub label: Rc<RefCell<Label>>,\n    \
             pub accessor: fn(&DataModel) -> String,\n}\n\n\
         impl MachineBinding {\n    \
             /// Re-apply this binding from the supplied DataModel.\n    \
             pub fn refresh(&self, dm: &DataModel) {\n        \
                 self.label.borrow_mut().set_text((self.accessor)(dm));\n    \
             }\n}\n\n",
    );
}

/// QT-05c §3 — sealed enum over the two binding source kinds.
/// Emitted on SM-attached modules; the 4-tuple's binding slot is
/// `Vec<Binding>` instead of `Vec<LabelBinding>`.
fn emit_binding_enum(out: &mut String) {
    out.push_str(
        "/// QT-05c §3 — sealed enum over the binding sources reactive\n\
         /// `refresh_bindings` knows how to drive. `Label` reads from\n\
         /// `ScreenState`; `Machine` reads from `<sm>_gen::DataModel`.\n\
         pub enum Binding {\n    \
             Label(LabelBinding),\n    \
             Machine(MachineBinding),\n}\n\n",
    );
}

/// Emit the QT-04e §7 / QT-05c §7 `pub fn refresh_bindings` free
/// function. Signature varies by SM presence; single-line per branch
/// to stay rustfmt-idempotent.
fn emit_refresh_bindings_fn(out: &mut String, has_sm: bool) {
    if has_sm {
        // `#[rustfmt::skip]` on the SM-attached form because the
        // signature exceeds rustfmt's wrap threshold; keeping the
        // emit on one line keeps the output byte-stable across
        // `cargo fmt` runs.
        out.push_str(
            "/// Re-apply every QT-04e / QT-05c binding from the current\n\
             /// state and machine. Idempotent; safe to call after any\n\
             /// mutation. No-op when `bindings` is empty.\n\
             #[rustfmt::skip]\n\
             pub fn refresh_bindings(state: &Rc<RefCell<ScreenState>>, machine: &Rc<RefCell<Machine>>, bindings: &[Binding]) {\n    \
                 let s = state.borrow();\n    \
                 let m = machine.borrow();\n    \
                 for b in bindings {\n        \
                     match b {\n            \
                         Binding::Label(lb) => lb.refresh(&s),\n            \
                         Binding::Machine(mb) => mb.refresh(&m.dm),\n        \
                     }\n    \
                 }\n}\n\n",
        );
    } else {
        out.push_str(
            "/// Re-apply every QT-04e binding from the current state.\n\
             /// Idempotent; safe to call after every `state.borrow_mut()`\n\
             /// mutation. No-op when `bindings` is empty.\n\
             pub fn refresh_bindings(state: &Rc<RefCell<ScreenState>>, bindings: &[LabelBinding]) {\n    \
                 let s = state.borrow();\n    \
                 for b in bindings {\n        \
                     b.refresh(&s);\n    \
                 }\n}\n\n",
        );
    }
}

/// Parse a QML literal float (`1.5`, `-3.14`, `0`). Returns None for
/// non-literal expressions. Accepts integer literals as f32 too —
/// QML / JS make no distinction in expression position.
fn parse_float_literal(expr: &str) -> Option<f32> {
    expr.trim().parse::<f32>().ok()
}

/// Parse `true` / `false` (QML / JS lowercase).
fn parse_bool_literal(expr: &str) -> Option<bool> {
    match expr.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Either lower a handler body per QT-05b §5 (`dispatch("…")`),
/// QT-04b §7 (assignment grammar), or fall through to QT-04 §7
/// (verbatim comment + empty TODO closure). Tries each grammar in
/// the order listed; the first match wins.
fn emit_qt04b_or_qt04_handler(
    body: &str,
    state_fields: &[StateField],
    sm_id: Option<&str>,
    out: &mut String,
) {
    emit_qt04b_or_qt04_handler_for("button", body, state_fields, sm_id, out);
}

/// Same as [`emit_qt04b_or_qt04_handler`] but emits against an
/// arbitrary widget binding name. Used by QT-04d for
/// `click_area.set_on_click(...)` and shared with QT-05b's dispatch
/// lowering — the closure shape and markers stay consistent across
/// widget kinds.
fn emit_qt04b_or_qt04_handler_for(
    binding: &str,
    body: &str,
    state_fields: &[StateField],
    sm_id: Option<&str>,
    out: &mut String,
) {
    // QT-05b §5: try `dispatch("…")` grammar first, but only when a
    // state machine is actually attached (otherwise there's no
    // `Event` enum to refer to and the lowering would not compile).
    if sm_id.is_some()
        && let Some(events) = lower_dispatch_body(body)
    {
        out.push_str("    {\n        let machine = Rc::clone(&machine);\n");
        out.push_str(&format!("        {binding}.set_on_click(move |_b| {{\n"));
        out.push_str("            let mut m = machine.borrow_mut();\n");
        for ev in &events {
            out.push_str(&format!(
                "            // QT-05b dispatch: {qml} → Event::{pascal}\n",
                qml = ev.qml,
                pascal = ev.pascal,
            ));
            out.push_str(&format!(
                "            m.dispatch(Event::{pascal});\n",
                pascal = ev.pascal,
            ));
        }
        out.push_str("        });\n    }\n");
        return;
    }
    if let Some(stmts) = lower_handler_body(body, state_fields) {
        out.push_str("    {\n        let state = Rc::clone(&state);\n");
        out.push_str(&format!("        {binding}.set_on_click(move |_b| {{\n"));
        out.push_str("            let mut s = state.borrow_mut();\n");
        for line in body.trim().lines() {
            out.push_str(&format!("            // QT-04b body: {line}\n"));
        }
        for stmt in stmts {
            out.push_str(&format!("            {stmt}\n"));
        }
        out.push_str("        });\n    }\n");
    } else {
        emit_qt04_handler_body(body, out);
        out.push_str(&format!(
            "    {binding}.set_on_click(|_b| {{\n        \
             // TODO QT-04e: lower QML expression to Rust.\n    }});\n"
        ));
    }
}

/// QT-05b §3 — one parsed `dispatch("<event>")` call site.
struct DispatchEvent {
    /// The original QML literal text (e.g. `"start"` without quotes).
    qml: String,
    /// PascalCased Rust enum variant (e.g. `"Start"`) per QT-05b §5.
    pascal: String,
}

/// QT-05b §5 — try to lower `body` as a sequence of `dispatch("…")`
/// calls. Returns `Some(events)` only if **every** statement is a
/// `dispatch("<ident>")` form; on any failure (multi-arg, expression
/// argument, mixed grammar, empty), returns `None` so the caller
/// falls through to QT-04b's grammar.
fn lower_dispatch_body(body: &str) -> Option<Vec<DispatchEvent>> {
    let cleaned = body.trim();
    if cleaned.is_empty() {
        return None;
    }
    let mut events = Vec::new();
    for raw_stmt in cleaned.split(';') {
        let stmt = raw_stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        events.push(parse_dispatch_call(stmt)?);
    }
    if events.is_empty() {
        None
    } else {
        Some(events)
    }
}

fn parse_dispatch_call(stmt: &str) -> Option<DispatchEvent> {
    let after = stmt.strip_prefix("dispatch")?;
    let after = after.trim_start();
    let inner = after.strip_prefix('(')?.strip_suffix(')')?.trim();
    // Accept double- or single-quoted string literals only. QT-05b
    // §5 explicitly excludes expression-form arguments.
    let qml = if let Some(s) = inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        s
    } else if let Some(s) = inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        s
    } else {
        return None;
    };
    if qml.is_empty() {
        return None;
    }
    let pascal = pascal_case_event(qml)?;
    Some(DispatchEvent {
        qml: qml.to_string(),
        pascal,
    })
}

/// QT-05b §5 — normalise a QML event literal into the PascalCase
/// Rust enum variant that istate's template emits via
/// `to_rust_ident | capitalize`. Splits on `_`, `-`, `.`, ` `;
/// rejects empty input or leading-digit forms.
fn pascal_case_event(input: &str) -> Option<String> {
    let mut chars = input.chars();
    let first = chars.next()?;
    if first.is_ascii_digit() {
        return None;
    }
    let mut out = String::new();
    let mut next_upper = true;
    for ch in input.chars() {
        if matches!(ch, '_' | '-' | '.' | ' ') {
            next_upper = true;
            continue;
        }
        if !ch.is_ascii_alphanumeric() {
            return None;
        }
        if next_upper {
            for u in ch.to_uppercase() {
                out.push(u);
            }
        } else {
            for l in ch.to_lowercase() {
                out.push(l);
            }
        }
        next_upper = false;
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Try to lower every statement of `body` per QT-04b §7. Returns
/// `Some(rust_statements)` only if **every** statement matches the
/// grammar (per §7's all-or-nothing rule). On any failure, returns
/// `None`.
fn lower_handler_body(body: &str, state_fields: &[StateField]) -> Option<Vec<String>> {
    let cleaned = body.trim();
    if cleaned.is_empty() {
        return None;
    }
    let mut stmts = Vec::new();
    for raw_stmt in cleaned.split(';') {
        let stmt = raw_stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        let lowered = lower_handler_statement(stmt, state_fields)?;
        stmts.push(lowered);
    }
    if stmts.is_empty() { None } else { Some(stmts) }
}

fn lower_handler_statement(stmt: &str, state_fields: &[StateField]) -> Option<String> {
    // QT-04f §7: prefix may be the root id (stripped to bare prop)
    // or a non-root id whose property is namespaced into ScreenState.
    let (op, lhs_raw, rhs_raw) = split_assignment(stmt)?;
    let field = resolve_state_field_ref(lhs_raw, state_fields)?;

    match op {
        AssignOp::PlusEq => match field.ty {
            StateFieldType::I32 => {
                let n = parse_int_literal(rhs_raw)?;
                Some(format!(
                    "s.{} = s.{}.saturating_add({});",
                    field.name, field.name, n
                ))
            }
            StateFieldType::F32 => {
                let f = parse_float_literal(rhs_raw)?;
                let lit = if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                };
                Some(format!("s.{} += {};", field.name, lit))
            }
            StateFieldType::StringTy => {
                let s = parse_string_literal(rhs_raw)?;
                Some(format!("s.{}.push_str({});", field.name, rust_str_lit(&s)))
            }
            StateFieldType::Bool => None,
        },
        AssignOp::MinusEq => match field.ty {
            StateFieldType::I32 => {
                let n = parse_int_literal(rhs_raw)?;
                Some(format!(
                    "s.{} = s.{}.saturating_sub({});",
                    field.name, field.name, n
                ))
            }
            StateFieldType::F32 => {
                let f = parse_float_literal(rhs_raw)?;
                let lit = if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                };
                Some(format!("s.{} -= {};", field.name, lit))
            }
            _ => None,
        },
        AssignOp::Eq => {
            // Self-toggle: `<ident> = !<ident>` for bool.
            if field.ty == StateFieldType::Bool
                && let Some(toggled_raw) = rhs_raw.strip_prefix('!')
                && let Some(toggled_field) =
                    resolve_state_field_ref(toggled_raw.trim(), state_fields)
                && toggled_field.name == field.name
            {
                return Some(format!("s.{} = !s.{};", field.name, field.name));
            }
            match field.ty {
                StateFieldType::I32 => {
                    let n = parse_int_literal(rhs_raw)?;
                    Some(format!("s.{} = {};", field.name, n))
                }
                StateFieldType::F32 => {
                    let f = parse_float_literal(rhs_raw)?;
                    let lit = if f.fract() == 0.0 {
                        format!("{f:.1}")
                    } else {
                        format!("{f}")
                    };
                    Some(format!("s.{} = {};", field.name, lit))
                }
                StateFieldType::Bool => {
                    let b = parse_bool_literal(rhs_raw)?;
                    Some(format!("s.{} = {};", field.name, b))
                }
                StateFieldType::StringTy => {
                    let s = parse_string_literal(rhs_raw)?;
                    Some(format!(
                        "s.{} = String::from({});",
                        field.name,
                        rust_str_lit(&s)
                    ))
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
}

/// Split `<lhs> <op> <rhs>` where op is one of `+= -= =`. Returns the
/// operator and the trimmed sides. Falls through to None on anything
/// else (including chained assignments and operators outside §7).
fn split_assignment(stmt: &str) -> Option<(AssignOp, &str, &str)> {
    if let Some(idx) = stmt.find("+=") {
        return Some((AssignOp::PlusEq, stmt[..idx].trim(), stmt[idx + 2..].trim()));
    }
    if let Some(idx) = stmt.find("-=") {
        return Some((
            AssignOp::MinusEq,
            stmt[..idx].trim(),
            stmt[idx + 2..].trim(),
        ));
    }
    // Be conservative on `=`: reject `==`, `!=`, `<=`, `>=` by checking
    // the surrounding chars.
    let bytes = stmt.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
            let next = bytes.get(i + 1).copied();
            if matches!(prev, Some(b'!' | b'<' | b'>' | b'=')) || next == Some(b'=') {
                i += 1;
                continue;
            }
            return Some((AssignOp::Eq, stmt[..i].trim(), stmt[i + 1..].trim()));
        }
        i += 1;
    }
    None
}

// `strip_root_prefix` was the QT-04b helper that handled the single
// root-scope rule. QT-04f's `resolve_state_field_ref` superseded it;
// removed to avoid dead code (Specification-Required removal — no
// remaining caller references it).

/// Shared resolver used by both QT-04b's handler-body lowering and
/// QT-04c's initial-value text bindings. Implements the QT-04f §7
/// resolution walk:
///
/// ```text
/// <ident>            → root scope, bare lookup
/// <root_id>.<ident>  → strip prefix, root scope, bare lookup
/// <other_id>.<ident> → namespaced lookup against `<other_id>_<ident>`
/// <a>.<b>.<c>...     → fall through (deeper nesting unsupported at QT-04f)
/// anything else      → fall through
/// ```
fn resolve_state_field_ref<'a>(
    expr: &str,
    state_fields: &'a [StateField],
) -> Option<&'a StateField> {
    let s = expr.trim();
    if s.is_empty() {
        return None;
    }
    // Bare ident: validate identifier shape, look up against
    // root-scope (un-namespaced) fields.
    if !s.contains('.') {
        if !is_simple_ident(s) {
            return None;
        }
        return state_fields
            .iter()
            .find(|f| f.owner_id.is_none() && f.qml_prop == s);
    }
    // `<owner>.<prop>` — owner may be the root id (stripped) or a
    // non-root id'd item.
    let mut parts = s.split('.');
    let owner = parts.next()?.trim();
    let prop = parts.next()?.trim();
    if parts.next().is_some() {
        // Three-or-more-level dotted reference; not supported at QT-04f.
        return None;
    }
    if !is_simple_ident(owner) || !is_simple_ident(prop) {
        return None;
    }
    if owner == "root" {
        return state_fields
            .iter()
            .find(|f| f.owner_id.is_none() && f.qml_prop == prop);
    }
    state_fields
        .iter()
        .find(|f| f.owner_id.as_deref() == Some(owner) && f.qml_prop == prop)
}

fn is_simple_ident(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !s.bytes().next().is_some_and(|b| b.is_ascii_digit())
}

/// Resolve a non-literal QML text expression against `state_fields`
/// per QT-04c §5. Returns the matched field name when:
///
/// * the expression is a single identifier, or `<root_id>.<ident>`
///   (root id stripped per QT-04b §8 / QT-04c §5);
/// * the resulting bare identifier matches a `ScreenState` field;
/// * that field's type is `StateFieldType::StringTy`.
///
/// Returns `None` for any non-matching shape — the caller falls
/// through to the QT-04e TODO path.
fn resolve_string_state_ref<'a>(expr: &str, state_fields: &'a [StateField]) -> Option<&'a str> {
    let field = resolve_state_field_ref(expr, state_fields)?;
    if field.ty != StateFieldType::StringTy {
        return None;
    }
    Some(field.name.as_str())
}

/// QT-05c §5 — recognise the `sm.dm.<ident>` Label-text form.
/// Returns the trailing field name on match. Only the literal
/// `sm.dm.…` prefix is accepted per QT-05c §5; alternative forms
/// (`dm.…`, `machine.dm.…`, parenthesised, expression-form) fall
/// through to QT-04e's grammar.
fn parse_dm_text_ref(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    let after = trimmed.strip_prefix("sm.dm.")?;
    if after.is_empty() {
        return None;
    }
    if !after.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    if after
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
    {
        return None;
    }
    Some(after.to_string())
}

fn sanitize_ident(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Parse a `.qml` file and write `qt-ir.json` into `out/`.
pub(crate) fn ingest(input: &Path, out: &Path) -> Result<()> {
    if input.is_dir() {
        return ingest_dir(input, out);
    }
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;

    let mut module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    // QT-05a: link sibling .scjson side-file (silent fall-through if absent).
    attach_scjson_side_file(&mut module, input)?;

    fs::create_dir_all(out).with_context(|| format!("creating output dir {}", out.display()))?;

    let out_path: PathBuf = out.join("qt-ir.json");
    let json = serde_json::to_string_pretty(&module)?;
    fs::write(&out_path, json).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

/// QT-08 §5 / §7: directory-mode `qt ingest`. Walks `<input>/*.qml`
/// in lexical order and writes one `<basename>.qt-ir.json` per file.
fn ingest_dir(input: &Path, out: &Path) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating output dir {}", out.display()))?;
    for qml in qt08_collect_qml_files(input)? {
        let source =
            fs::read_to_string(&qml).with_context(|| format!("reading {}", qml.display()))?;
        let mut module =
            parse_module(&source, &qml).with_context(|| format!("parsing {}", qml.display()))?;
        // QT-05a: link sibling .scjson side-file (silent fall-through if absent).
        attach_scjson_side_file(&mut module, &qml)?;
        let stem = qml
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("input has no usable file stem: {}", qml.display()))?;
        let out_path = out.join(format!("{stem}.qt-ir.json"));
        let json = serde_json::to_string_pretty(&module)?;
        fs::write(&out_path, json).with_context(|| format!("writing {}", out_path.display()))?;
    }
    Ok(())
}

/// Collect `*.qml` files from `dir` per QT-08 §7: immediate children
/// only, lexical sort, hidden files skipped, basename collisions
/// rejected (defensive — same-dir basename collisions are
/// filesystem-impossible but the check guards against future moves
/// to recursive walking).
fn qt08_collect_qml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let read = fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))?;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut seen_stems: Vec<String> = Vec::new();
    for entry in read {
        let entry = entry.with_context(|| format!("iterating {}", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("qml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("no usable file stem: {}", path.display()))?
            .to_string();
        if seen_stems.contains(&stem) {
            bail!(
                "duplicate basename `{stem}` in {} — refusing to clobber output",
                dir.display()
            );
        }
        seen_stems.push(stem);
        files.push(path);
    }
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

/// Parse `source` (treated as one QML document) into a [`UiModule`].
pub fn parse_module(source: &str, source_path: &Path) -> Result<UiModule> {
    let mut p = Parser::new(source);
    let mut imports = Vec::new();
    let mut pragmas = Vec::new();

    p.skip_trivia();
    while !p.eof() {
        if p.match_keyword("import") {
            imports.push(p.parse_import()?);
            p.skip_trivia();
            continue;
        }
        if p.match_keyword("pragma") {
            pragmas.push(p.parse_pragma()?);
            p.skip_trivia();
            continue;
        }
        break;
    }

    let root = p.parse_item().context("parsing root item")?;

    p.skip_trivia();
    if !p.eof() {
        let (line, col) = p.line_col();
        bail!(
            "unexpected trailing content at line {line}, column {col} (expected single root item)"
        );
    }

    Ok(UiModule {
        version: QT_IR_VERSION,
        source: source_path.display().to_string(),
        imports,
        pragmas,
        root,
        // QT-05: structural parser doesn't link scjson side-files;
        // QT-05a will populate this on a second pass.
        state_machine: None,
    })
}

// ============================================================================
// Parser
// ============================================================================

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }

    fn line_col(&self) -> (usize, usize) {
        let mut line = 1usize;
        let mut col = 1usize;
        for &b in &self.src[..self.pos] {
            if b == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Skip whitespace and `// ...` / `/* ... */` comments.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b) if (b as char).is_whitespace() => {
                    self.pos += 1;
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    while let Some(b) = self.peek() {
                        self.pos += 1;
                        if b == b'\n' {
                            break;
                        }
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    self.pos += 2;
                    while !self.eof() {
                        if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => return,
            }
        }
    }

    /// Match a bare keyword (followed by non-ident char). Consumes on hit.
    fn match_keyword(&mut self, kw: &str) -> bool {
        self.skip_trivia();
        let bytes = kw.as_bytes();
        if self.pos + bytes.len() > self.src.len() {
            return false;
        }
        if &self.src[self.pos..self.pos + bytes.len()] != bytes {
            return false;
        }
        // Must not be followed by an ident-continuation char.
        if let Some(next) = self.src.get(self.pos + bytes.len()).copied()
            && is_ident_cont(next)
        {
            return false;
        }
        self.pos += bytes.len();
        true
    }

    fn expect(&mut self, ch: u8, what: &str) -> Result<()> {
        self.skip_trivia();
        match self.peek() {
            Some(b) if b == ch => {
                self.pos += 1;
                Ok(())
            }
            _ => {
                let (line, col) = self.line_col();
                bail!(
                    "expected `{}` ({what}) at line {line}, column {col}",
                    ch as char
                );
            }
        }
    }

    fn read_ident(&mut self) -> Result<String> {
        self.skip_trivia();
        let start = self.pos;
        match self.peek() {
            Some(b) if is_ident_start(b) => self.pos += 1,
            _ => {
                let (line, col) = self.line_col();
                bail!("expected identifier at line {line}, column {col}");
            }
        }
        while let Some(b) = self.peek() {
            if is_ident_cont(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(std::str::from_utf8(&self.src[start..self.pos])?.to_string())
    }

    /// Read `a.b.c` style dotted identifier.
    fn read_dotted_ident(&mut self) -> Result<String> {
        let mut s = self.read_ident()?;
        loop {
            self.skip_trivia();
            if self.peek() == Some(b'.') {
                let after = self.peek_at(1);
                if matches!(after, Some(b) if is_ident_start(b)) {
                    self.pos += 1;
                    s.push('.');
                    s.push_str(&self.read_ident()?);
                    continue;
                }
            }
            break;
        }
        Ok(s)
    }

    /// Read a quoted string literal and return its raw form *including* quotes.
    fn read_string_literal(&mut self) -> Result<String> {
        self.skip_trivia();
        let q = self.peek();
        if !matches!(q, Some(b'"') | Some(b'\'')) {
            let (line, col) = self.line_col();
            bail!("expected string literal at line {line}, column {col}");
        }
        let start = self.pos;
        let quote = q.unwrap();
        self.pos += 1;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'\\' {
                self.pos += 1; // skip next char
                continue;
            }
            if b == quote {
                break;
            }
        }
        Ok(std::str::from_utf8(&self.src[start..self.pos])?.to_string())
    }

    fn parse_import(&mut self) -> Result<UiImport> {
        self.skip_trivia();
        // Module is either an identifier (Qt.Quick.Controls) or a quoted path.
        let module = if matches!(self.peek(), Some(b'"') | Some(b'\'')) {
            // Strip quotes for storage.
            let raw = self.read_string_literal()?;
            raw.trim_matches(|c| c == '"' || c == '\'').to_string()
        } else {
            self.read_dotted_ident()?
        };

        // Optional version: digits (and dot).
        self.skip_trivia();
        let version = if matches!(self.peek(), Some(b'0'..=b'9')) {
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'.' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            Some(std::str::from_utf8(&self.src[start..self.pos])?.to_string())
        } else {
            None
        };

        // Optional alias.
        let alias = if self.match_keyword("as") {
            Some(self.read_ident()?)
        } else {
            None
        };

        // Optional trailing semicolon.
        self.skip_trivia();
        if self.peek() == Some(b';') {
            self.pos += 1;
        }
        Ok(UiImport {
            module,
            version,
            alias,
        })
    }

    fn parse_pragma(&mut self) -> Result<String> {
        self.skip_trivia();
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' {
                break;
            }
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.src[start..self.pos])?
            .trim()
            .to_string();
        if self.peek() == Some(b';') {
            self.pos += 1;
        }
        Ok(s)
    }

    fn parse_item(&mut self) -> Result<UiItem> {
        let type_name = self.read_dotted_ident().context("expected QML type name")?;
        self.skip_trivia();
        self.expect(b'{', "after type name")?;
        let mut item = UiItem {
            type_name,
            ..UiItem::default()
        };
        self.parse_item_body(&mut item)?;
        Ok(item)
    }

    fn parse_item_body(&mut self, item: &mut UiItem) -> Result<()> {
        loop {
            self.skip_trivia();
            match self.peek() {
                None => {
                    let (line, col) = self.line_col();
                    bail!("unterminated item body at line {line}, column {col}");
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(b';') => {
                    self.pos += 1;
                    continue;
                }
                _ => self.parse_member(item)?,
            }
        }
    }

    fn parse_member(&mut self, item: &mut UiItem) -> Result<()> {
        // Recognise property declarations and signal declarations by leading
        // keywords. Anything else is either an `id:` line, a property
        // assignment (possibly grouped), a signal handler binding, or a
        // child item.
        if self.match_keyword("default") {
            return self.parse_property_decl(item, true, false);
        }
        if self.match_keyword("readonly") {
            return self.parse_property_decl(item, false, true);
        }
        if self.match_keyword("property") {
            return self.finish_property_decl(item, false, false);
        }
        if self.match_keyword("signal") {
            let sig = self.parse_signal_decl()?;
            item.signals.push(sig);
            return Ok(());
        }
        if self.match_keyword("function") {
            // Capture as a special handler with name `function:<name>`. Body
            // is balanced-brace text. Out of scope for structural emit but
            // preserved verbatim so later passes can see it.
            let name = self.read_ident()?;
            self.skip_trivia();
            // Skip param list (...).
            if self.peek() == Some(b'(') {
                self.skip_balanced(b'(', b')')?;
            }
            self.skip_trivia();
            let body = if self.peek() == Some(b'{') {
                self.read_balanced(b'{', b'}')?
            } else {
                String::new()
            };
            item.handlers.push(UiHandler {
                signal: format!("function:{name}"),
                body,
            });
            return Ok(());
        }

        // Otherwise, lead with a (dotted) identifier. Decide what kind of
        // member we have based on what follows.
        let lead = self.read_dotted_ident()?;
        self.skip_trivia();
        match self.peek() {
            Some(b':') => {
                self.pos += 1;
                self.parse_assignment_after_colon(item, lead)
            }
            Some(b'{') => {
                // Either a child item (uppercase typename) or a grouped
                // property assignment (lowercase typename).
                if starts_uppercase(&lead) {
                    self.pos += 1;
                    let mut child = UiItem {
                        type_name: lead,
                        ..UiItem::default()
                    };
                    self.parse_item_body(&mut child)?;
                    item.children.push(child);
                } else {
                    self.pos += 1;
                    let mut group = UiItem {
                        type_name: lead.clone(),
                        ..UiItem::default()
                    };
                    self.parse_item_body(&mut group)?;
                    item.assignments.push(UiAssignment {
                        target: lead,
                        value: UiAssignmentValue::Object {
                            item: Box::new(group),
                        },
                    });
                }
                Ok(())
            }
            _ => {
                let (line, col) = self.line_col();
                bail!("expected `:` or `{{` after `{lead}` at line {line}, column {col}");
            }
        }
    }

    fn parse_property_decl(
        &mut self,
        item: &mut UiItem,
        default_kw: bool,
        readonly: bool,
    ) -> Result<()> {
        // `default` and `readonly` may combine, in either order, but must be
        // followed by `property`.
        if self.match_keyword("readonly") {
            return self.finish_property_decl(item, default_kw, true);
        }
        if self.match_keyword("default") {
            return self.finish_property_decl(item, true, readonly);
        }
        if self.match_keyword("property") {
            return self.finish_property_decl(item, default_kw, readonly);
        }
        let (line, col) = self.line_col();
        bail!("expected `property` after modifier at line {line}, column {col}");
    }

    fn finish_property_decl(
        &mut self,
        item: &mut UiItem,
        default_kw: bool,
        readonly: bool,
    ) -> Result<()> {
        let ty = self.read_dotted_ident().context("property type")?;
        let name = self.read_ident().context("property name")?;
        self.skip_trivia();
        let default_value = if self.peek() == Some(b':') {
            self.pos += 1;
            Some(self.read_expression_text())
        } else {
            None
        };
        item.properties.push(UiProperty {
            name,
            ty,
            default_value,
            readonly,
            default_kw,
        });
        Ok(())
    }

    fn parse_signal_decl(&mut self) -> Result<UiSignal> {
        let name = self.read_ident().context("signal name")?;
        self.skip_trivia();
        let mut params = Vec::new();
        if self.peek() == Some(b'(') {
            self.pos += 1;
            loop {
                self.skip_trivia();
                if self.peek() == Some(b')') {
                    self.pos += 1;
                    break;
                }
                let ty = self.read_dotted_ident().context("signal param type")?;
                let pname = self.read_ident().context("signal param name")?;
                params.push(UiSignalParam { name: pname, ty });
                self.skip_trivia();
                match self.peek() {
                    Some(b',') => {
                        self.pos += 1;
                    }
                    Some(b')') => {
                        self.pos += 1;
                        break;
                    }
                    _ => {
                        let (line, col) = self.line_col();
                        bail!(
                            "expected `,` or `)` in signal param list at line {line}, column {col}"
                        );
                    }
                }
            }
        }
        Ok(UiSignal { name, params })
    }

    fn parse_assignment_after_colon(&mut self, item: &mut UiItem, target: String) -> Result<()> {
        // `id: <ident>` is special.
        if target == "id" {
            self.skip_trivia();
            let id_val = self.read_ident().context("id value")?;
            item.id = Some(id_val);
            return Ok(());
        }
        if target.starts_with("on") && target.len() > 2 && is_upper_byte(target.as_bytes()[2]) {
            // Signal handler binding.
            let body = self.read_handler_body();
            item.handlers.push(UiHandler {
                signal: target,
                body,
            });
            return Ok(());
        }

        // Plain assignment. Decide between expression / object / list.
        self.skip_trivia();
        let value = match self.peek() {
            Some(b'[') => {
                self.pos += 1;
                let items = self.parse_assignment_list()?;
                UiAssignmentValue::List { items }
            }
            Some(b'{') => {
                // JS block — capture as opaque expression text.
                let body = self.read_balanced(b'{', b'}')?;
                UiAssignmentValue::Expression { text: body }
            }
            Some(b)
                if is_ident_start(b) && {
                    // Lookahead: if we see `Ident { ... }` with capitalised lead,
                    // treat as object value.
                    let saved = self.pos;
                    let id = self.read_dotted_ident().unwrap_or_default();
                    self.skip_trivia();
                    let is_object =
                        !id.is_empty() && starts_uppercase(&id) && self.peek() == Some(b'{');
                    self.pos = saved;
                    is_object
                } =>
            {
                let id = self.read_dotted_ident()?;
                self.skip_trivia();
                self.expect(b'{', "after object value type name")?;
                let mut sub = UiItem {
                    type_name: id,
                    ..UiItem::default()
                };
                self.parse_item_body(&mut sub)?;
                UiAssignmentValue::Object {
                    item: Box::new(sub),
                }
            }
            _ => UiAssignmentValue::Expression {
                text: self.read_expression_text(),
            },
        };
        item.assignments.push(UiAssignment { target, value });
        Ok(())
    }

    fn parse_assignment_list(&mut self) -> Result<Vec<UiAssignmentValue>> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(out);
            }
            // Each list element is an object-value or expression.
            let saved = self.pos;
            let id = self.read_dotted_ident().unwrap_or_default();
            self.skip_trivia();
            let value = if !id.is_empty() && starts_uppercase(&id) && self.peek() == Some(b'{') {
                self.pos += 1;
                let mut sub = UiItem {
                    type_name: id,
                    ..UiItem::default()
                };
                self.parse_item_body(&mut sub)?;
                UiAssignmentValue::Object {
                    item: Box::new(sub),
                }
            } else {
                self.pos = saved;
                let text = self.read_expression_text_until(b',', b']');
                UiAssignmentValue::Expression { text }
            };
            out.push(value);
            self.skip_trivia();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    continue;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(out);
                }
                _ => {
                    let (line, col) = self.line_col();
                    bail!("expected `,` or `]` in list at line {line}, column {col}");
                }
            }
        }
    }

    /// Read an expression after a `:` until newline or `;` at depth 0,
    /// honouring string and bracket nesting.
    fn read_expression_text(&mut self) -> String {
        // Skip leading inline whitespace (but not newline — newline is the
        // terminator for the typical 1-line form).
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let start = self.pos;
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut brace = 0i32;
        while let Some(b) = self.peek() {
            match b {
                b'"' | b'\'' => {
                    self.skip_string();
                    continue;
                }
                b'/' if self.peek_at(1) == Some(b'/') => {
                    // Line comment terminates expression on this line.
                    break;
                }
                b'/' if self.peek_at(1) == Some(b'*') => {
                    self.pos += 2;
                    while !self.eof() {
                        if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                    continue;
                }
                b'(' => paren += 1,
                b')' => paren -= 1,
                b'[' => bracket += 1,
                b']' => bracket -= 1,
                b'{' => brace += 1,
                b'}' => {
                    if brace == 0 && paren == 0 && bracket == 0 {
                        break;
                    }
                    brace -= 1;
                }
                b';' if paren == 0 && bracket == 0 && brace == 0 => break,
                b'\n' if paren == 0 && bracket == 0 && brace == 0 => break,
                _ => {}
            }
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.src[start..self.pos])
            .unwrap_or("")
            .trim()
            .to_string();
        // Consume the terminating semicolon if any.
        if self.peek() == Some(b';') {
            self.pos += 1;
        }
        text
    }

    /// Variant of `read_expression_text` used inside list literals: stops at
    /// either of the two terminator characters at depth 0.
    fn read_expression_text_until(&mut self, term1: u8, term2: u8) -> String {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let start = self.pos;
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut brace = 0i32;
        while let Some(b) = self.peek() {
            match b {
                b'"' | b'\'' => {
                    self.skip_string();
                    continue;
                }
                b'(' => paren += 1,
                b')' => paren -= 1,
                b'[' => bracket += 1,
                b']' if bracket == 0 && b == term2 && paren == 0 && brace == 0 => break,
                b']' => bracket -= 1,
                b'{' => brace += 1,
                b'}' => brace -= 1,
                _ if b == term1 && paren == 0 && bracket == 0 && brace == 0 => break,
                _ if b == term2 && paren == 0 && bracket == 0 && brace == 0 => break,
                _ => {}
            }
            self.pos += 1;
        }
        std::str::from_utf8(&self.src[start..self.pos])
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Read a signal handler body. Either a `{ ... }` block (kept verbatim
    /// without the outer braces) or a single-line expression.
    fn read_handler_body(&mut self) -> String {
        self.skip_trivia();
        if self.peek() == Some(b'{') {
            let raw = self.read_balanced(b'{', b'}').unwrap_or_default();
            // Strip outer braces and trim.
            let trimmed = raw.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                trimmed[1..trimmed.len() - 1].trim().to_string()
            } else {
                trimmed.to_string()
            }
        } else {
            self.read_expression_text()
        }
    }

    /// Read `(...)` or `{...}` balanced span and return the substring
    /// (including the outer delimiters).
    fn read_balanced(&mut self, open: u8, close: u8) -> Result<String> {
        self.skip_trivia();
        if self.peek() != Some(open) {
            let (line, col) = self.line_col();
            bail!("expected `{}` at line {line}, column {col}", open as char);
        }
        let start = self.pos;
        self.pos += 1;
        let mut depth = 1i32;
        while let Some(b) = self.peek() {
            match b {
                b'"' | b'\'' => {
                    self.skip_string();
                    continue;
                }
                b'/' if self.peek_at(1) == Some(b'/') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == b'\n' {
                            break;
                        }
                    }
                    continue;
                }
                b'/' if self.peek_at(1) == Some(b'*') => {
                    self.pos += 2;
                    while !self.eof() {
                        if self.peek() == Some(b'*') && self.peek_at(1) == Some(b'/') {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                    continue;
                }
                _ if b == open => depth += 1,
                _ if b == close => {
                    depth -= 1;
                    self.pos += 1;
                    if depth == 0 {
                        return Ok(std::str::from_utf8(&self.src[start..self.pos])?.to_string());
                    }
                    continue;
                }
                _ => {}
            }
            self.pos += 1;
        }
        let (line, col) = self.line_col();
        bail!(
            "unterminated `{}…{}` block at line {line}, column {col}",
            open as char,
            close as char
        );
    }

    fn skip_balanced(&mut self, open: u8, close: u8) -> Result<()> {
        let _ = self.read_balanced(open, close)?;
        Ok(())
    }

    /// Skip a string literal starting at the current position. Caller has
    /// already verified the leading quote.
    fn skip_string(&mut self) {
        let quote = match self.peek() {
            Some(q) => q,
            None => return,
        };
        self.pos += 1;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'\\' {
                self.pos += 1;
                continue;
            }
            if b == quote {
                return;
            }
        }
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_ident_cont(b: u8) -> bool {
    is_ident_start(b) || b.is_ascii_digit()
}

fn is_upper_byte(b: u8) -> bool {
    b.is_ascii_uppercase()
}

fn starts_uppercase(s: &str) -> bool {
    s.as_bytes()
        .first()
        .map(|b| b.is_ascii_uppercase())
        .unwrap_or(false)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse(src: &str) -> UiModule {
        parse_module(src, &PathBuf::from("test.qml")).expect("parse ok")
    }

    #[test]
    fn parses_imports_and_root() {
        let src = r#"
            import QtQuick 2.15
            import QtQuick.Controls as QC
            import "." as Local

            Item {
                id: root
                width: 800
                height: 480
            }
        "#;
        let m = parse(src);
        assert_eq!(m.version, QT_IR_VERSION);
        assert_eq!(m.imports.len(), 3);
        assert_eq!(m.imports[0].module, "QtQuick");
        assert_eq!(m.imports[0].version.as_deref(), Some("2.15"));
        assert_eq!(m.imports[1].module, "QtQuick.Controls");
        assert_eq!(m.imports[1].alias.as_deref(), Some("QC"));
        assert_eq!(m.imports[2].module, ".");
        assert_eq!(m.imports[2].alias.as_deref(), Some("Local"));
        assert_eq!(m.root.type_name, "Item");
        assert_eq!(m.root.id.as_deref(), Some("root"));
        assert_eq!(m.root.assignments.len(), 2);
        assert_eq!(m.root.assignments[0].target, "width");
    }

    #[test]
    fn parses_property_declarations() {
        let src = r#"
            Item {
                property string title: "Hello"
                property int count: 0
                readonly property real ratio: 1.5
                default property var children
            }
        "#;
        let m = parse(src);
        let p = &m.root.properties;
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].name, "title");
        assert_eq!(p[0].ty, "string");
        assert_eq!(p[0].default_value.as_deref(), Some("\"Hello\""));
        assert!(!p[0].readonly && !p[0].default_kw);
        assert_eq!(p[2].ty, "real");
        assert!(p[2].readonly);
        assert!(p[3].default_kw);
        assert_eq!(p[3].default_value, None);
    }

    #[test]
    fn parses_signal_decl_and_handlers() {
        let src = r#"
            Item {
                signal pressed(int x, int y)
                signal triggered()
                onPressed: console.log(x, y)
                onTriggered: { count += 1; emit() }
            }
        "#;
        let m = parse(src);
        assert_eq!(m.root.signals.len(), 2);
        assert_eq!(m.root.signals[0].name, "pressed");
        assert_eq!(m.root.signals[0].params.len(), 2);
        assert_eq!(m.root.signals[0].params[0].ty, "int");
        assert_eq!(m.root.signals[1].params.len(), 0);
        assert_eq!(m.root.handlers.len(), 2);
        assert_eq!(m.root.handlers[0].signal, "onPressed");
        assert_eq!(m.root.handlers[0].body, "console.log(x, y)");
        assert_eq!(m.root.handlers[1].signal, "onTriggered");
        assert!(m.root.handlers[1].body.contains("count += 1"));
    }

    #[test]
    fn parses_grouped_property_and_child() {
        let src = r##"
            Item {
                font { pixelSize: 48; family: "Inter" }
                Rectangle {
                    id: bg
                    color: "#1e1e2e"
                }
            }
        "##;
        let m = parse(src);
        // `font { ... }` → grouped assignment because lowercase.
        assert_eq!(m.root.assignments.len(), 1);
        assert_eq!(m.root.assignments[0].target, "font");
        match &m.root.assignments[0].value {
            UiAssignmentValue::Object { item } => {
                assert_eq!(item.assignments.len(), 2);
                assert_eq!(item.assignments[0].target, "pixelSize");
            }
            _ => panic!("expected grouped object"),
        }
        // `Rectangle { ... }` → child item because uppercase.
        assert_eq!(m.root.children.len(), 1);
        assert_eq!(m.root.children[0].type_name, "Rectangle");
        assert_eq!(m.root.children[0].id.as_deref(), Some("bg"));
    }

    #[test]
    fn parses_dotted_assignment_target() {
        let src = r#"
            Item {
                anchors.fill: parent
                anchors.margins: 16
            }
        "#;
        let m = parse(src);
        assert_eq!(m.root.assignments.len(), 2);
        assert_eq!(m.root.assignments[0].target, "anchors.fill");
        assert_eq!(m.root.assignments[1].target, "anchors.margins");
    }

    #[test]
    fn parses_object_value_assignment() {
        // `Behavior on <prop>` is QML-syntax-special and out of scope for
        // the structural MVP. Plain `target: TypeName { ... }` works.
        let src = r#"
            Item {
                transitions: Transition { from: "a"; to: "b" }
            }
        "#;
        let m = parse(src);
        assert_eq!(m.root.assignments.len(), 1);
        match &m.root.assignments[0].value {
            UiAssignmentValue::Object { item } => {
                assert_eq!(item.type_name, "Transition");
                assert_eq!(item.assignments.len(), 2);
            }
            _ => panic!("expected object value"),
        }
    }

    #[test]
    fn ignores_comments() {
        let src = r#"
            // top comment
            import QtQuick 2.15 // trailing
            /* block
               comment */
            Item {
                width: 100 // trailing on assignment
                /* inline */ height: 200
            }
        "#;
        let m = parse(src);
        assert_eq!(m.imports.len(), 1);
        assert_eq!(m.root.assignments.len(), 2);
        assert_eq!(m.root.assignments[0].target, "width");
        assert_eq!(m.root.assignments[1].target, "height");
    }

    #[test]
    fn round_trips_through_serde_json() {
        let src = r#"
            import QtQuick 2.15
            Item {
                id: root
                width: 100
                Rectangle { color: "red" }
            }
        "#;
        let m = parse(src);
        let json = serde_json::to_string(&m).unwrap();
        let m2: UiModule = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    /// QT-02 schema-drift gate: regenerate `qt-ir.schema.json` and
    /// require byte-equivalence with the checked-in canonical copy.
    /// On failure, prints the regen command — that's the contract the
    /// QT-02 concepts doc relies on.
    #[test]
    fn schema_matches_checked_in_canonical() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest_dir).join("schemas/qt-ir.schema.json");
        let canonical = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing canonical schema at {} ({e}). Run: \
                 cargo run --features creator --bin rlvgl-creator -- \
                 qt schema --out schemas/qt-ir.schema.json",
                path.display()
            )
        });
        let regenerated = render_schema().expect("render_schema");
        if canonical != regenerated {
            // Trim trailing newline differences — `fs::write` of
            // `to_string_pretty` output ends without `\n`, but a
            // hand-edit may have left one.
            let canonical_t = canonical.trim_end();
            let regenerated_t = regenerated.trim_end();
            if canonical_t == regenerated_t {
                return;
            }
            panic!(
                "qt-ir.schema.json drifted from the IR types defined in src/bin/creator/qt.rs.\n\
                 Regenerate with:\n  \
                 cargo run --features creator --bin rlvgl-creator -- \
                 qt schema --out schemas/qt-ir.schema.json\n\
                 If this drift is intentional, follow the QT-00 §7 / QT-02 bumping policy \
                 (docs/qt-support/02-ir-schema.md)."
            );
        }
    }

    /// QT-05 §11 acceptance gate: a synthetic [`UiStateMachine`] —
    /// covering every variant of [`UiAction`] and [`UiScriptOrigin`]
    /// plus a non-empty `datamodel` and `scripts` table — round-trips
    /// through `serde_json` without losing any field. This pins the
    /// IR shape **before** QT-05a-e begin populating it from real
    /// scjson side-files; if the JSON wire shape ever drifts (e.g.
    /// a tag rename), this test fails first.
    #[test]
    fn state_machine_ir_roundtrips() {
        let sm = UiStateMachine {
            id: "stopwatch".to_string(),
            source: "stopwatch.scjson".to_string(),
            initial: Some("idle".to_string()),
            states: vec![
                UiState {
                    id: "idle".to_string(),
                    on_entry: vec![UiAction::Assign {
                        location: "elapsed".to_string(),
                        expr: Some("0".to_string()),
                    }],
                    on_exit: vec![],
                },
                UiState {
                    id: "running".to_string(),
                    on_entry: vec![UiAction::Script {
                        name: "tick_start".to_string(),
                    }],
                    on_exit: vec![UiAction::Raise {
                        event: "stopped".to_string(),
                    }],
                },
            ],
            transitions: vec![
                UiTransition {
                    source: "idle".to_string(),
                    event: Some("start".to_string()),
                    target: Some("running".to_string()),
                    cond: Some("elapsed >= 0".to_string()),
                    actions: vec![UiAction::Assign {
                        location: "elapsed".to_string(),
                        expr: None,
                    }],
                },
                UiTransition {
                    source: "running".to_string(),
                    event: Some("stop".to_string()),
                    target: Some("idle".to_string()),
                    cond: None,
                    actions: vec![],
                },
            ],
            datamodel: vec![
                UiDmField {
                    id: "elapsed".to_string(),
                    initial: Some(0.0),
                },
                UiDmField {
                    id: "lap".to_string(),
                    initial: None,
                },
            ],
            scripts: vec![
                UiScript {
                    name: "tick_start".to_string(),
                    origin: UiScriptOrigin::OnEntry {
                        state: "running".to_string(),
                    },
                },
                UiScript {
                    name: "diag_log".to_string(),
                    origin: UiScriptOrigin::Transition {
                        index: 0,
                        from: "idle".to_string(),
                        to: Some("running".to_string()),
                    },
                },
                UiScript {
                    name: "wrap_up".to_string(),
                    origin: UiScriptOrigin::OnExit {
                        state: "running".to_string(),
                    },
                },
            ],
        };
        let json = serde_json::to_string_pretty(&sm).expect("serialize");
        let back: UiStateMachine = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sm, back, "UiStateMachine does not roundtrip");

        // Round-trip the field as part of UiModule (the on-the-wire
        // location). This exercises the additive-field upgrade path
        // for legacy v1 IR files (which omit `state_machine`).
        let module = UiModule {
            version: QT_IR_VERSION,
            source: "stopwatch.qml".to_string(),
            imports: vec![],
            pragmas: vec![],
            root: UiItem {
                type_name: "Item".to_string(),
                ..UiItem::default()
            },
            state_machine: Some(sm.clone()),
        };
        let json = serde_json::to_string_pretty(&module).expect("serialize");
        let back: UiModule = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(module, back);
        assert_eq!(back.version, 2, "QT-05 IR version is 2");

        // Backwards-compat: a v1 IR (no `state_machine` key) parses
        // with `state_machine = None`, since QT-05 made the field
        // additive.
        let v1_json = r#"{
            "version": 1,
            "source": "legacy.qml",
            "imports": [],
            "pragmas": [],
            "root": {
                "type_name": "Item",
                "properties": [],
                "assignments": [],
                "signals": [],
                "handlers": [],
                "children": []
            }
        }"#;
        let legacy: UiModule =
            serde_json::from_str(v1_json).expect("v1 IR (no state_machine key) must still parse");
        assert!(legacy.state_machine.is_none());
    }

    /// QT-02 roundtrip gate: ingest fixture → IR → JSON → IR equality.
    #[test]
    fn canonical_fixture_roundtrips() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let qml_path = std::path::Path::new(manifest_dir).join("tests/fixtures/qt/hello.qml");
        let source = std::fs::read_to_string(&qml_path).expect("read hello.qml");
        let parsed = parse_module(&source, &qml_path).expect("parse hello.qml");

        let json = serde_json::to_string_pretty(&parsed).expect("serialize");
        let reparsed: UiModule = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed, reparsed,
            "qt-ir does not roundtrip through serde_json"
        );
    }
}
