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

    // Images: any item whose stripped type is `Image` / `BorderImage` /
    // `AnimatedImage` with a `source:` assignment. A literal `source: "<path>"`
    // is captured directly; a state-bound `source:` (e.g. a ternary choosing
    // between artwork files) has every quoted image-path literal harvested so
    // designer-authored conditional artwork still vendors.
    if matches!(stripped_type, "Image" | "BorderImage" | "AnimatedImage")
        && let Some(raw_source) = lookup_assignment(item, "source")
    {
        if let Some(s) = parse_string_literal(raw_source) {
            inv.images.insert(strip_qrc_prefix(&s).to_string());
        } else {
            for s in extract_asset_literals(raw_source) {
                inv.images.insert(strip_qrc_prefix(&s).to_string());
            }
        }
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

/// Resolve an `Image { source: … }` to the single asset the emitter should
/// blit: a literal `source: "path"` directly, or the first artwork branch of a
/// state-bound ternary. Returns `None` when no image-path literal is present.
fn pick_image_source(item: &UiItem) -> Option<AssetRef> {
    let raw = lookup_assignment(item, "source")?;
    let path = parse_string_literal(raw)
        .filter(|s| {
            let l = s.to_ascii_lowercase();
            l.starts_with("qrc:") || is_image_path(&l)
        })
        .or_else(|| extract_asset_literals(raw).into_iter().next())?;
    let stripped = strip_qrc_prefix(&path).to_string();
    Some(AssetRef {
        symbol: asset_symbol(&stripped),
        path: stripped,
    })
}

/// Whether a path looks like a supported raster/vector image by extension.
fn is_image_path(lower: &str) -> bool {
    const IMAGE_EXTS: [&str; 7] = [".png", ".jpg", ".jpeg", ".gif", ".svg", ".bmp", ".webp"];
    IMAGE_EXTS.iter().any(|e| lower.ends_with(e))
}

/// Derive a stable `qt_assets` module symbol from an image path: the file stem
/// uppercased with every non-alphanumeric byte folded to `_`, prefixed `IMG_`.
/// e.g. `Qml/Images/ImgPlay_48.png` → `IMG_IMGPLAY_48`.
fn asset_symbol(path: &str) -> String {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(path);
    let mut sym = String::from("IMG_");
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            sym.extend(ch.to_uppercase());
        } else {
            sym.push('_');
        }
    }
    sym
}

/// Harvest quoted string literals that look like asset paths from a binding
/// expression (e.g. the branches of a state-bound `source:` ternary). A
/// literal qualifies if it carries a known image extension or a `qrc:` prefix.
fn extract_asset_literals(expr: &str) -> Vec<String> {
    let bytes = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let q = bytes[i];
        if q == b'"' || q == b'\'' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != q {
                // Honour simple backslash escapes so an escaped quote does not
                // terminate the literal early.
                if bytes[j] == b'\\' && j + 1 < bytes.len() {
                    j += 2;
                    continue;
                }
                j += 1;
            }
            if j <= bytes.len() {
                let lit = &expr[start..j.min(expr.len())];
                let lower = lit.to_ascii_lowercase();
                if lower.starts_with("qrc:") || is_image_path(&lower) {
                    out.push(lit.to_string());
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
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
/// Bumped to `15` (QT-03b §15 2026-06-26): structural QML nodes
/// (`Item`/`Row`/`Column`) now emit a transparent Container background
/// instead of inheriting the opaque-white `Style::default()`, and
/// `qt_image` emits a transparent image background plus QML-default
/// `Image.Stretch` scaling (source→dest). Without this the emitted
/// tree rendered all-white on real hardware (opaque structural
/// containers buried the artwork; 1:1 blit never filled the slots).
pub const QT_EMIT_VERSION_RLVGL: u32 = 21;

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
pub(crate) fn emit(
    input: &Path,
    out: &Path,
    target: EmitTarget,
    scxml_context: Option<String>,
) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("creating output dir {}", out.display()))?;
    // QT-05g: an externally-injected SCXML context object linked to a
    // machine crate, as `--scxml-context <ctx>=<crate>`.
    let ctx = scxml_context
        .as_deref()
        .map(parse_scxml_context)
        .transpose()?;
    if input.is_dir() {
        for qml in qt08_collect_qml_files(input)? {
            emit_one_file(&qml, out, target, ctx.as_ref())?;
        }
        return Ok(());
    }
    emit_one_file(input, out, target, ctx.as_ref())
}

/// QT-05g: the `--scxml-context <ctx>=<crate>` linkage — declares that QML
/// predicates qualified by context object `<ctx>` resolve against
/// `<crate>::Machine` via the istate M1P6 (linkage-v2) surface.
#[derive(Debug, Clone)]
pub(crate) struct ScxmlContext {
    /// The QML context-object id, e.g. `scxmlBolero`.
    pub context: String,
    /// The machine crate name, e.g. `media_player`.
    pub krate: String,
}

/// Parse `--scxml-context <ctx>=<crate>` into a [`ScxmlContext`]. Both sides
/// must be non-empty Rust-ident-shaped tokens.
fn parse_scxml_context(s: &str) -> Result<ScxmlContext> {
    let (ctx, krate) = s
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("--scxml-context expects `<ctx>=<crate>`, got {s:?}"))?;
    let (ctx, krate) = (ctx.trim(), krate.trim());
    let ident_ok = |t: &str| {
        !t.is_empty()
            && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !t.chars().next().unwrap().is_ascii_digit()
    };
    if !ident_ok(ctx) || !ident_ok(krate) {
        anyhow::bail!("--scxml-context `<ctx>=<crate>` parts must be identifiers, got {s:?}");
    }
    Ok(ScxmlContext {
        context: ctx.to_string(),
        krate: krate.to_string(),
    })
}

fn emit_one_file(
    input: &Path,
    out: &Path,
    target: EmitTarget,
    scxml_context: Option<&ScxmlContext>,
) -> Result<()> {
    let source =
        fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
    let mut module =
        parse_module(&source, input).with_context(|| format!("parsing {}", input.display()))?;
    // QT-05a: link sibling .scjson side-file (silent fall-through if absent).
    attach_scjson_side_file(&mut module, input)?;

    // Cross-component instantiation (rlvgl target): inline user-defined
    // component children (`<Type>.qml`) so the full composed widget tree —
    // including leaf artwork inside reusable components — reaches the emitter.
    if matches!(target, EmitTarget::Rlvgl) {
        let root_dir = component_search_root(input);
        let mut stack = Vec::new();
        let mut cache = std::collections::HashMap::new();
        expand_components_in(&mut module.root, &root_dir, &mut stack, &mut cache);
        // QT-Repeater: turn `Repeater { model: [...] }` arrays into positioned
        // icon children (runs after component inlining so the model literals are
        // present and stable). When a state-machine context is linked
        // (QT-05g), a model item's `imageKeySource:` predicate ternary is
        // preserved verbatim so the Image arm can lower it to a reactive
        // binding; otherwise the resting else-branch literal is used.
        expand_repeaters_in(&mut module.root, 0, 0, scxml_context.is_some());
        // QT-05j: when a state-machine context is linked, give every untagged
        // button that dispatches a `submitBtnSetupEvent("…")` a synthetic tag so
        // the emitted `BUTTON_TAP_EVENTS` table can name it for the consumer.
        if scxml_context.is_some() {
            synthesize_button_tap_tags(&mut module.root);
        }
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("input has no usable file stem: {}", input.display()))?;

    let (out_path, rust) = match target {
        EmitTarget::Data => (out.join(format!("{stem}.rs")), render_rs(&module)),
        EmitTarget::Rlvgl => {
            // QT-03c: build the dimension resolver (JS constants + root-property
            // defaults) so the anchor solver can evaluate non-literal extents.
            let resolver = DimResolver::from_qml(input, &module.root);
            (
                out.join(format!("{stem}.rlvgl.rs")),
                render_rlvgl_with_resolver(&module, &resolver, scxml_context),
            )
        }
    };
    fs::write(&out_path, rust).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

/// Pick the directory to resolve user-component `.qml` files against: the
/// nearest ancestor directory named `Qml` (the conventional QML source root),
/// else the input file's own directory.
fn component_search_root(input: &Path) -> PathBuf {
    let mut cur = input.parent();
    let fallback = cur
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    while let Some(d) = cur {
        if d.file_name().and_then(|s| s.to_str()) == Some("Qml") {
            return d.to_path_buf();
        }
        cur = d.parent();
    }
    fallback
}

/// Find `<type_name>.qml` under `root` via a bounded recursive search.
fn find_component_file(root: &Path, type_name: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, target: &str, depth: u32) -> Option<PathBuf> {
        if depth > 6 {
            return None;
        }
        let entries = fs::read_dir(dir).ok()?;
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else if path.file_name().and_then(|s| s.to_str()) == Some(target) {
                return Some(path);
            }
        }
        for sub in subdirs {
            if let Some(hit) = walk(&sub, target, depth + 1) {
                return Some(hit);
            }
        }
        None
    }
    let target = format!("{type_name}.qml");
    walk(root, &target, 0)
}

/// Load and parse a component `.qml` root item, cached by type name. Returns
/// `None` (cached) when the file is absent or fails to parse — the instance is
/// then left as a fallback container.
fn load_component(
    type_name: &str,
    root_dir: &Path,
    cache: &mut std::collections::HashMap<String, Option<UiItem>>,
) -> Option<UiItem> {
    if let Some(hit) = cache.get(type_name) {
        return hit.clone();
    }
    let resolved = find_component_file(root_dir, type_name).and_then(|path| {
        let src = fs::read_to_string(&path).ok()?;
        parse_module(&src, &path).ok().map(|m| m.root)
    });
    cache.insert(type_name.to_string(), resolved.clone());
    resolved
}

/// Merge a component definition's root into an instance node: the node renders
/// as the component root's type with the component body prepended to any
/// instance-provided children, instance assignments overriding component
/// defaults (so the parent's anchors win), and the instance `id` preserved.
fn merge_component_into_instance(instance: &mut UiItem, def: UiItem) {
    instance.type_name = def.type_name;
    // Component body first, then instance-supplied extra children.
    let mut children = def.children;
    children.append(&mut instance.children);
    instance.children = children;
    // Assignments: component defaults, then instance overrides by target.
    let mut merged = def.assignments;
    for asn in std::mem::take(&mut instance.assignments) {
        if let Some(slot) = merged.iter_mut().find(|a| a.target == asn.target) {
            *slot = asn;
        } else {
            merged.push(asn);
        }
    }
    instance.assignments = merged;
    // Handlers / declarations: component then instance.
    let mut handlers = def.handlers;
    handlers.append(&mut instance.handlers);
    instance.handlers = handlers;
    let mut props = def.properties;
    props.append(&mut instance.properties);
    instance.properties = props;
    let mut signals = def.signals;
    signals.append(&mut instance.signals);
    instance.signals = signals;
}

/// Recursively inline user-defined component children of `item`. `stack` guards
/// against component-reference cycles; `cache` memoises parsed component roots.
fn expand_components_in(
    item: &mut UiItem,
    root_dir: &Path,
    stack: &mut Vec<String>,
    cache: &mut std::collections::HashMap<String, Option<UiItem>>,
) {
    // Expand this node if it instantiates a user-defined component: an
    // uppercase type that the widget table doesn't map and that resolves to a
    // sibling `.qml`, and isn't already being expanded (cycle guard).
    // Chase the component base-type chain: each merge may retype the node to
    // another user component (e.g. MediaRepeatButton → SelectButton →
    // FocusButton → Rectangle), so loop until the node is a mapped widget, a
    // framework type with no `.qml`, or a cycle is detected.
    let mut pushed = 0usize;
    loop {
        let stripped = item
            .type_name
            .rsplit('.')
            .next()
            .unwrap_or(&item.type_name)
            .to_string();
        if !(starts_uppercase(&stripped)
            && matches!(map_qml_type(&stripped), WidgetKind::Fallback)
            && !stack.contains(&stripped))
        {
            break;
        }
        let Some(def) = load_component(&stripped, root_dir, cache) else {
            break;
        };
        stack.push(stripped);
        pushed += 1;
        merge_component_into_instance(item, def);
    }

    for child in &mut item.children {
        expand_components_in(child, root_dir, stack, cache);
    }

    for _ in 0..pushed {
        stack.pop();
    }
}

/// QT-Repeater: expand a `Repeater { model: [ {…}, … ] }` whose model is a
/// literal array into one positioned `Image` child per model entry's
/// `imageKeySource`. The delegate's button frame is intentionally dropped (it
/// lowers to a transparent background); the visible artwork is the per-item
/// icon. Icons are laid out left-to-right and centred as a group across the
/// containing layout's width via sibling anchors (so the anchor solver, which
/// already handles `verticalCenter` + `<id>.right` chains, does the placement).
///
/// `layout_w` / `spacing` are the containing Row/RowLayout's `width:` /
/// `spacing:` (0 when unknown — the Repeater is then left untouched).
fn expand_repeaters_in(item: &mut UiItem, layout_w: i32, spacing: i32, preserve_ternary: bool) {
    // A Repeater nested directly in this node uses this node's width/spacing.
    let my_w = lookup_assignment(item, "width")
        .and_then(parse_int_literal)
        .unwrap_or(layout_w);
    let my_spacing = lookup_assignment(item, "spacing")
        .and_then(parse_int_literal)
        .unwrap_or(spacing);
    for child in &mut item.children {
        if child.type_name == "Repeater" && my_w > 0 {
            expand_one_repeater(child, my_w, my_spacing, preserve_ternary);
        }
        expand_repeaters_in(child, my_w, my_spacing, preserve_ternary);
    }
}

fn expand_one_repeater(rep: &mut UiItem, layout_w: i32, spacing: i32, preserve_ternary: bool) {
    let items: Vec<String> = match rep.assignments.iter().find(|a| a.target == "model") {
        Some(UiAssignment {
            value: UiAssignmentValue::List { items },
            ..
        }) => items
            .iter()
            .filter_map(|v| match v {
                UiAssignmentValue::Expression { text } => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => return,
    };
    // Each model entry contributes its `imageKeySource` (the icon) and, when
    // present, its `eventName` (the QT-05j button event the delegate dispatches
    // via `submitBtnSetupEvent(eventName, …)`).
    let entries: Vec<(String, Option<String>)> = items
        .iter()
        .filter_map(|t| {
            extract_image_key_source(t, preserve_ternary).map(|s| (s, extract_model_event_name(t)))
        })
        .collect();
    let n = entries.len() as i32;
    if n == 0 {
        return;
    }
    // The model-driven transport icons in this corpus are 48px square.
    let icon = 48;
    let group_w = n * icon + (n - 1) * spacing.max(0);
    let start = ((layout_w - group_w) / 2).max(0);
    let mut kids = Vec::new();
    for (i, (src, event)) in entries.iter().enumerate() {
        let id = format!("__rep_btn_{i}");
        let mut a: Vec<UiAssignment> = vec![
            ui_assign("source", src.clone()),
            ui_assign("width", format!("{icon}")),
            ui_assign("height", format!("{icon}")),
            ui_assign(
                "anchors.verticalCenter",
                "parent.verticalCenter".to_string(),
            ),
        ];
        if i == 0 {
            a.push(ui_assign("anchors.left", "parent.left".to_string()));
            a.push(ui_assign("anchors.leftMargin", format!("{start}")));
        } else {
            a.push(ui_assign(
                "anchors.left",
                format!("__rep_btn_{}.right", i - 1),
            ));
            a.push(ui_assign(
                "anchors.leftMargin",
                format!("{}", spacing.max(0)),
            ));
        }
        // QT-05j: carry the model item's `eventName` as a resolved
        // `submitBtnSetupEvent("<EVENT>")` handler on the synthesized icon node
        // (the delegate button frame is dropped), so the button-event walker
        // wires this tap target like any literal-arg button.
        let handlers = match event {
            Some(ev) => vec![UiHandler {
                signal: "onReleased".to_string(),
                body: format!("submitBtnSetupEvent(\"{ev}\")"),
            }],
            None => Vec::new(),
        };
        kids.push(UiItem {
            type_name: "Image".to_string(),
            id: Some(id),
            properties: Vec::new(),
            assignments: a,
            signals: Vec::new(),
            handlers,
            children: Vec::new(),
        });
    }
    rep.children = kids;
}

/// Build an `Expression`-valued assignment.
fn ui_assign(target: &str, text: String) -> UiAssignment {
    UiAssignment {
        target: target.to_string(),
        value: UiAssignmentValue::Expression { text },
    }
}

/// QT-05j: parse the first `submitBtnSetupEvent("<EVENT>"…)` call out of a
/// handler body, returning the QML button-event name `<EVENT>` (the first
/// string-literal argument). Returns `None` when the call is absent or its
/// first argument is a bare identifier (the Repeater delegate's
/// `submitBtnSetupEvent(eventName, 1)` form — resolved during expansion).
fn parse_submit_btn_event(body: &str) -> Option<String> {
    let idx = body.find("submitBtnSetupEvent")?;
    let args = &body[idx + "submitBtnSetupEvent".len()..];
    let args = args.trim_start().strip_prefix('(')?;
    // The first argument must be a string literal (`"MediaFunc.X"`); a bare
    // identifier before the first quote (e.g. a `,`-separated later arg) means
    // the event name is dynamic and not lowerable here.
    let q1 = args.find('"')?;
    if args[..q1].contains(',') {
        return None;
    }
    let rest = &args[q1 + 1..];
    let q2 = rest.find('"')?;
    Some(rest[..q2].to_string())
}

/// QT-05j: the QML button-event name a node's handlers dispatch via
/// `submitBtnSetupEvent("…")`, if any.
fn extract_submit_btn_event(item: &UiItem) -> Option<String> {
    item.handlers
        .iter()
        .find_map(|h| parse_submit_btn_event(&h.body))
}

/// QT-05j: pull the `eventName: "<EVENT>"` literal out of a Repeater model
/// entry (the per-item button event the delegate dispatches via
/// `submitBtnSetupEvent(eventName, …)`).
fn extract_model_event_name(obj_text: &str) -> Option<String> {
    let idx = obj_text.find("eventName")?;
    let after = obj_text[idx + "eventName".len()..]
        .trim_start()
        .strip_prefix(':')?;
    let q1 = after.find('"')?;
    let rest = &after[q1 + 1..];
    let q2 = rest.find('"')?;
    Some(rest[..q2].to_string())
}

/// QT-05j: give every untagged button that dispatches a
/// `submitBtnSetupEvent("…")` a synthetic, deterministic `id` (hence a node
/// tag) so the consumer can resolve its bounds to route a tap. Tagged buttons
/// (a real QML `id:`, e.g. `repeatBtn`) keep their tag. Runs after component +
/// Repeater expansion so synthesized delegate handlers are present.
fn synthesize_button_tap_tags(item: &mut UiItem) {
    if item.id.is_none()
        && let Some(ev) = extract_submit_btn_event(item)
    {
        item.id = Some(format!(
            "__btn_{}",
            sanitize_ident(&ev.to_ascii_lowercase())
        ));
    }
    for child in &mut item.children {
        synthesize_button_tap_tags(child);
    }
}

/// Pull the `imageKeySource:` value out of a model entry. When
/// `preserve_ternary` is set (QT-05g: a state-machine context is linked) and
/// the value is a predicate ternary (`<ctx>.<state> ? "A" : "B"`), the full
/// ternary expression is returned verbatim so the Image arm can lower it to a
/// reactive `Binding::Predicate`. Otherwise — no context linked, or a plain
/// single-literal source — the resting else-branch literal is returned,
/// re-quoted as a string literal for `pick_image_source`.
fn extract_image_key_source(obj_text: &str, preserve_ternary: bool) -> Option<String> {
    let idx = obj_text.find("imageKeySource")?;
    let after = obj_text[idx + "imageKeySource".len()..].trim_start();
    let after = after.strip_prefix(':')?;
    // `imageKeySource` is the last property in the model object literal; trim
    // the closing `}` of the object so only the value expression remains.
    let expr = after
        .rsplit_once('}')
        .map(|(e, _)| e)
        .unwrap_or(after)
        .trim();
    if preserve_ternary && split_ternary(expr).is_some() {
        // Preserve the full predicate ternary; the Image arm lowers it.
        return Some(expr.to_string());
    }
    // For a `cond ? A : B` source, the else-branch (B, the last literal) is the
    // resting-state icon (`mediaPlaying ? Pause : Play` → Play at rest). For a
    // single-literal source, last == only.
    let lit = extract_asset_literals(expr).into_iter().last()?;
    Some(format!("\"{lit}\""))
}

/// Quote-aware split of a `cond ? then : else` ternary into its three parts.
/// The scan tracks string-literal state so a `:` inside a `qrc:/…` path does
/// not masquerade as the ternary colon. Returns `None` if `expr` is not a
/// single top-level ternary. Chained ternaries (an `else` that is itself a
/// ternary) are detected by the caller via a `?` in the returned `else` part.
fn split_ternary(expr: &str) -> Option<(&str, &str, &str)> {
    let bytes = expr.as_bytes();
    let mut in_q = 0u8;
    let mut qpos = None;
    let mut cpos = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_q != 0 {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == in_q {
                in_q = 0;
            }
        } else if b == b'"' || b == b'\'' {
            in_q = b;
        } else if b == b'?' && qpos.is_none() {
            qpos = Some(i);
        } else if b == b':' && qpos.is_some() && cpos.is_none() {
            cpos = Some(i);
            break;
        }
        i += 1;
    }
    let (q, c) = (qpos?, cpos?);
    Some((
        expr[..q].trim(),
        expr[q + 1..c].trim(),
        expr[c + 1..].trim(),
    ))
}

/// QT-05g: parse a predicate-bound `source:` ternary `<ctx>.<state> ? "A" : "B"`
/// into `(state_id, on_path, off_path)` (qrc-stripped), for the declared
/// context `ctx`. Returns `None` for a non-ternary, a non-matching context, a
/// chained ternary (deferred — `then`/`else` carry a nested `?`), or a branch
/// that is not a single asset literal.
fn parse_predicate_source(expr: &str, ctx: &str) -> Option<(String, String, String)> {
    let (cond, then_b, else_b) = split_ternary(expr)?;
    if then_b.contains('?') || else_b.contains('?') {
        return None; // chained ternary deferred (QT-05g §5)
    }
    let state = cond.strip_prefix(ctx)?.strip_prefix('.')?.trim();
    if state.is_empty() || !state.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let on = extract_asset_literals(then_b).into_iter().next()?;
    let off = extract_asset_literals(else_b).into_iter().next()?;
    Some((
        state.to_string(),
        strip_qrc_prefix(&on).to_string(),
        strip_qrc_prefix(&off).to_string(),
    ))
}

/// QT-05i: parse a chained predicate `source:` of the form
/// `<ctx>.<s1> ? "A" : <ctx>.<s2> ? "B" : … : "Z"` into ordered first-true-wins
/// arms `[(state_id, asset_path), …]` plus the final else asset (`"Z"`, the
/// resting icon). For the declared context `ctx`. Returns `None` for a plain
/// single (non-chained) ternary — that is `parse_predicate_source`'s job and is
/// tried first — a non-matching context, a then-branch that is itself a ternary,
/// or a branch that is not a single asset literal. qrc prefixes are stripped.
fn parse_chained_predicate_source(
    expr: &str,
    ctx: &str,
) -> Option<(Vec<(String, String)>, String)> {
    let mut arms: Vec<(String, String)> = Vec::new();
    let mut cur = expr.trim();
    loop {
        let (cond, then_b, else_b) = split_ternary(cur)?;
        if then_b.contains('?') {
            return None; // a then-branch must be a single asset literal
        }
        let state = cond.strip_prefix(ctx)?.strip_prefix('.')?.trim();
        if state.is_empty() || !state.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
        let on = extract_asset_literals(then_b).into_iter().next()?;
        arms.push((state.to_string(), strip_qrc_prefix(&on).to_string()));
        let else_b = else_b.trim();
        if split_ternary(else_b).is_some() {
            cur = else_b; // the else is itself a ternary — continue the chain
            continue;
        }
        // Final else: the resting asset literal. Require ≥2 arms so a binary
        // ternary (handled by `parse_predicate_source`) never reaches here.
        if arms.len() < 2 {
            return None;
        }
        let default = extract_asset_literals(else_b).into_iter().next()?;
        return Some((arms, strip_qrc_prefix(&default).to_string()));
    }
}

/// QT-05h: parse a bare visibility predicate `<ctx>.<state>` into `state_id`
/// for the declared context `ctx`. Returns `None` for a non-matching context
/// or a non-bare-predicate expression (negation / boolean / literal), which
/// are deferred (QT-05h §5).
fn parse_visible_predicate(expr: &str, ctx: &str) -> Option<String> {
    let state = expr.trim().strip_prefix(ctx)?.strip_prefix('.')?.trim();
    if state.is_empty() || !state.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(state.to_string())
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
    render_rlvgl_with_resolver(module, &DimResolver::default(), None)
}

/// As [`render_rlvgl`], but with a pre-built [`DimResolver`] so the anchor
/// solver can evaluate JS-constant / root-property dimension expressions, and
/// an optional QT-05g `--scxml-context` linkage that attaches an istate
/// linkage-v2 machine for state-predicate Image bindings.
fn render_rlvgl_with_resolver(
    module: &UiModule,
    resolver: &DimResolver,
    scxml_context: Option<&ScxmlContext>,
) -> String {
    let state_fields = collect_state_fields(&module.root);
    // QT-05g: linkage v2 attaches via `--scxml-context` when there is no
    // `.scjson` side-file (v1). The two are mutually exclusive here; a v1 SM
    // takes precedence if both are somehow present.
    let v1_sm_id = module.state_machine.as_ref().map(|sm| sm.id.clone());
    let v2 = v1_sm_id.is_none() && scxml_context.is_some();
    let sm_id = v1_sm_id
        .clone()
        .or_else(|| scxml_context.map(|c| c.krate.clone()));
    let sm_context = scxml_context.map(|c| c.context.clone());
    let dm_field_ids: Vec<String> = module
        .state_machine
        .as_ref()
        .map(|sm| sm.datamodel.iter().map(|f| f.id.clone()).collect())
        .unwrap_or_default();
    let mut ctx = RlvglEmitCtx::new_with_fields(state_fields.clone())
        .with_resolver(resolver.clone())
        .with_sm(sm_id.clone())
        .with_dm_fields(dm_field_ids)
        .with_v2(v2, sm_context.clone());
    let root_fn = ctx.alloc_fn_name(&module.root);
    let root_body = ctx.emit_helper(&module.root, &root_fn, true);
    let has_sm = sm_id.is_some();
    let used_predicate = ctx.used_predicate;
    let used_visibility = ctx.used_visibility;
    let used_predicate_chain = ctx.used_predicate_chain;
    let button_tap_events = ctx.button_tap_events.clone();
    let used_dm_fields = ctx.used_dm_fields.clone();
    let used_assets = ctx.used_assets.clone();
    let has_images = !used_assets.is_empty();

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
    if has_images {
        // Image-bearing modules reference the generated `qt_assets` module
        // (one `&[u8]` RLE blob per symbol). Emitted first — `crate` sorts
        // ahead of external crates under rustfmt, so the output stays
        // idempotent under `cargo fmt`. The integrator provides
        // `crate::qt_assets` (hand-written or vendored).
        out.push_str("use crate::qt_assets;\n");
    }
    out.push_str("use alloc::rc::Rc;\n");
    out.push_str("use alloc::string::String;\n");
    out.push_str("use alloc::vec::Vec;\n");
    out.push_str("use core::cell::RefCell;\n");
    out.push('\n');
    // QT-05g: the linkage-v2 machine crate (`--scxml-context`) is an external
    // crate sorting before `rlvgl_*` — emit it here so the file stays
    // fmt-idempotent. (v1's `<sm>_gen` sorts after `rlvgl_widgets`; emitted
    // below.)
    if v2 && let Some(id) = &sm_id {
        out.push_str(&format!("use {id}::Machine;\n"));
    }
    // Emit order matches rustfmt's preferred sort within the
    // `rlvgl_core::*` group (uppercase items before lowercase
    // modules), so the generated file stays idempotent under
    // `cargo fmt`.
    out.push_str("use rlvgl_core::WidgetNode;\n");
    if has_images {
        // `image` sorts after `WidgetNode` and before `widget` within the
        // `rlvgl_core::*` group (uppercase item, then lowercase modules
        // alphabetically) — fmt-stable. `BlitOpts` drives the `qt_image`
        // Stretch scaling below.
        out.push_str("use rlvgl_core::image::BlitOpts;\n");
    }
    out.push_str("use rlvgl_core::widget::{Color, Rect, Widget};\n");
    out.push_str("use rlvgl_widgets::button::Button;\n");
    out.push_str("use rlvgl_widgets::click_area::ClickArea;\n");
    out.push_str("use rlvgl_widgets::container::Container;\n");
    if has_images {
        // `image` sorts between `container` and `label` — fmt-stable.
        out.push_str("use rlvgl_widgets::image::Image;\n");
    }
    out.push_str("use rlvgl_widgets::label::Label;\n");
    // QT-05b §6: import the istate-codegen 6-symbol linkage surface
    // (the ones we actually reference at this phase: `Event` for
    // dispatch lowering, `Machine` for the threading parameter).
    // `State`/`DataModel`/`Externals` join when QT-05c/e land.
    // QT-05c §3 / §6: SM-attached (linkage v1) modules import the full
    // linkage surface trio — `Event` for dispatch lowering, `Machine` for
    // the threading parameter, `DataModel` for MachineBinding accessors.
    // `<sm>_gen` sorts after `rlvgl_widgets`. (v2 imports `Machine` from the
    // `--scxml-context` crate above, where it sorts before `rlvgl_*`.)
    if !v2 && let Some(id) = &sm_id {
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
        if v2 {
            out.push_str(
                "/// QT-05 §6 linkage version. v2 is the istate M1P6 dynamic-string\n\
                 /// surface (`step`/`is_active`/`get_var`), linked via `--scxml-context`.\n\
                 pub const ISTATE_LINKAGE_VERSION: u32 = 2;\n\n",
            );
        } else {
            out.push_str(
                "/// QT-05 §6 linkage version. v1 pins the istate Rust\n\
                 /// template's std-profile shape (VecDeque + Box<dyn Externals>).\n\
                 pub const ISTATE_LINKAGE_VERSION: u32 = 1;\n\n",
            );
        }
        out.push_str(&format!(
            "/// QT-05a §8 derived state-machine ID; matches the\n\
             /// `<sm>_gen` crate name stem (v1) or the `--scxml-context` crate (v2).\n\
             pub const QT_SM_NAME: &str = {};\n\n",
            rust_str_lit(id)
        ));
    }

    // QT-05j: tap-target table lowered from `<ctx>.submitBtnSetupEvent("…")`
    // button handlers. Each entry is `(node tag, raw QML button-event)`; the
    // QML event string round-trips verbatim (authority: derive). The consumer
    // owns the QML-event → machine-event vocabulary map (the role Bolero's C++
    // `submitBtnSetupEvent` plays), so this table is deliberately app-agnostic.
    if !button_tap_events.is_empty() {
        out.push_str(
            "/// QT-05j — tap targets lowered from `submitBtnSetupEvent(\"…\")` button\n\
             /// handlers: `(node tag, raw QML button-event)`. The consumer maps each\n\
             /// QML event to a machine event via `machine.step(…)`.\n\
             pub const BUTTON_TAP_EVENTS: &[(&str, &str)] = &[\n",
        );
        for (tag, ev) in &button_tap_events {
            out.push_str(&format!(
                "    ({}, {}),\n",
                rust_str_lit(tag),
                rust_str_lit(ev)
            ));
        }
        out.push_str("];\n\n");
    }

    emit_screen_state_struct(&state_fields, &mut out);
    emit_label_binding_struct(&mut out);
    if has_sm {
        if v2 {
            if used_predicate || used_visibility || used_predicate_chain {
                emit_image_art_struct(&mut out);
            }
            if used_predicate {
                emit_predicate_binding_struct(&mut out);
            }
            if used_visibility {
                emit_visibility_binding_struct(&mut out);
            }
            if used_predicate_chain {
                emit_predicate_chain_binding_struct(&mut out);
            }
            emit_binding_enum_v2(
                &mut out,
                used_predicate,
                used_visibility,
                used_predicate_chain,
            );
        } else {
            emit_machine_binding_struct(&mut out);
            emit_binding_enum(&mut out);
        }
    }

    if has_sm {
        if v2 {
            out.push_str(
                "/// Build the screen widget tree at `bounds` and return it\n\
                 /// alongside the `ScreenState` handle (QT-04b §3), the\n\
                 /// `Rc<RefCell<Machine>>` istate (linkage v2) handle, and the\n\
                 /// `Vec<Binding>` of reactive bindings (QT-05g §3). Callers\n\
                 /// drive the machine via `machine.borrow_mut().step(\"…\", …)`\n\
                 /// and call `refresh_bindings` to re-apply state-predicate\n\
                 /// artwork (e.g. Play↔Pause via `is_active`).\n\
                 #[rustfmt::skip]\n\
                 pub fn build_screen(\n    \
                     bounds: Rect,\n) \
                 -> (WidgetNode, Rc<RefCell<ScreenState>>, Rc<RefCell<Machine>>, Vec<Binding>) {\n",
            );
        } else {
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
        }
        emit_screen_state_init(&state_fields, &mut out);
        if v2 {
            // Linkage v2: the M1P6 machine must be `start()`ed to enter its
            // initial configuration before `is_active` is meaningful.
            out.push_str(
                "    let machine = Rc::new(RefCell::new({ let mut m = Machine::new(); m.start(); m }));\n",
            );
        } else {
            out.push_str("    let machine = Rc::new(RefCell::new(Machine::new()));\n");
        }
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
    emit_refresh_bindings_fn(
        &mut out,
        has_sm,
        v2,
        used_predicate,
        used_visibility,
        used_predicate_chain,
    );
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
    if has_images {
        emit_qt_image_helper(&used_assets, &mut out);
    }
    if used_predicate || used_visibility || used_predicate_chain {
        emit_qt_image_art_helper(&mut out);
    }
    if used_predicate {
        emit_qt_predicate_image_helper(&mut out);
    }
    if used_visibility {
        emit_qt_visibility_image_helper(&mut out);
    }
    if used_predicate_chain {
        emit_qt_predicate_chain_image_helper(&mut out);
    }
    if root_body.contains("qt_label(") {
        // QML `Text`/`Label` items have no background; emit a constructor that
        // clears the opaque-white `Style::default()` so text never sits on an
        // opaque white box that buries content drawn beneath it.
        out.push_str(
            "/// Construct a `Label` with a transparent background (QML text has\n\
             /// no fill), so it does not paint the opaque-white default `Style`.\n\
             fn qt_label(text: impl Into<String>, bounds: Rect) -> Label {\n    \
                 let mut l = Label::new(text, bounds);\n    \
                 l.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
                 l\n}\n\n",
        );
    }

    out.push_str(&root_body);

    // Trim the trailing blank line that comes from the last helper's
    // `}\n\n` separator, then add exactly one `\n` so the file ends
    // with a single newline. Keeps the emit byte-stable under
    // `cargo fmt`, which strips trailing blank lines.
    format!("{}\n", out.trim_end())
}

/// Emit the `qt_image` decode helper plus a `qt_assets` symbol manifest. The
/// helper decodes a vendored RLE blob into an owned, leaked pixel buffer and
/// wraps it in an [`rlvgl_widgets::image::Image`]; leaking is acceptable because
/// demo artwork is allocated once at startup and lives for the program.
///
/// The manifest lists every `qt_assets::<SYMBOL>` the module references. The
/// integrator provides a `qt_assets` module (hand-written or generated by
/// `rlvgl-creator` vendoring) exposing one `pub static <SYMBOL>: &[u8]` RLE blob
/// per entry. Decode mirrors the sctd-demo philosophers-table pipeline.
fn emit_qt_image_helper(used_assets: &[AssetRef], out: &mut String) {
    out.push_str("// Required `qt_assets` symbols (one RLE `&[u8]` blob each):\n");
    for a in used_assets {
        out.push_str(&format!("//   qt_assets::{}  ←  {}\n", a.symbol, a.path));
    }
    out.push_str(
        "/// Decode a vendored RLE asset into an owned, leaked pixel buffer and\n\
         /// wrap it in an `Image` widget (see emit-time docs above).\n\
         #[rustfmt::skip]\n\
         fn qt_image(bounds: Rect, rle: &'static [u8]) -> Rc<RefCell<dyn Widget>> {\n    \
             let (w, h, palette_bytes, stream) =\n        \
                 rlvgl_decomp::parse_rle_blob(rle).expect(\"qt_image: malformed RLE asset\");\n    \
             let palette_len = palette_bytes.len() / 2;\n    \
             let mut palette = alloc::vec![0u16; palette_len];\n    \
             for i in 0..palette_len {\n        \
                 palette[i] = u16::from_le_bytes([palette_bytes[i * 2], palette_bytes[i * 2 + 1]]);\n    \
             }\n    \
             let rgba = rlvgl_decomp::decode_rgba(w as usize, h as usize, &palette, stream)\n        \
                 .expect(\"qt_image: RLE decode failed\");\n    \
             let pixels: Vec<Color> = rgba\n        \
                 .chunks_exact(4)\n        \
                 .map(|c| if c[0] == 0xFF && c[1] == 0x00 && c[2] == 0xFF {\n            \
                     Color(0x00, 0x00, 0x00, 0x00) // magenta sentinel → transparent (RGB565 has no alpha)\n        \
                 } else {\n            \
                     Color(c[0], c[1], c[2], c[3])\n        \
                 })\n        \
                 .collect();\n    \
             let pixels: &'static [Color] = Vec::leak(pixels);\n    \
             let mut img = Image::new(bounds, w as i32, h as i32, pixels);\n    \
             // An Image paints its own pixels; the widget background MUST be\n    \
             // transparent so the default opaque-white `Style` does not bury\n    \
             // the artwork (and content drawn behind it).\n    \
             img.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
             // QML's default `fillMode` is `Image.Stretch`: scale the source to\n    \
             // fill the destination bounds. `scale` is 8.8 fixed-point, so 256 =\n    \
             // 1:1; dest/src * 256 stretches source pixels across `bounds`.\n    \
             let scale_x = if w > 0 {\n        \
                 ((bounds.width.max(0) as i64 * 256 / w as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let scale_y = if h > 0 {\n        \
                 ((bounds.height.max(0) as i64 * 256 / h as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let img = img.with_blit_opts(BlitOpts { scale_x, scale_y, ..BlitOpts::default() });\n    \
             Rc::new(RefCell::new(img))\n}\n\n",
    );
}

/// QT-05g: emit the shared `qt_image_art` RLE decoder (one asset → leaked
/// `ImageArt`). Emitted when any predicate or visibility binding is present.
fn emit_qt_image_art_helper(out: &mut String) {
    out.push_str(
        "/// QT-05g: decode one vendored RLE asset into a leaked `ImageArt`\n\
         /// (magenta-keyed → transparent, RGB565 has no alpha).\n\
         #[rustfmt::skip]\n\
         fn qt_image_art(rle: &'static [u8]) -> ImageArt {\n    \
             let (w, h, palette_bytes, stream) =\n        \
                 rlvgl_decomp::parse_rle_blob(rle).expect(\"qt_image_art: malformed RLE asset\");\n    \
             let palette_len = palette_bytes.len() / 2;\n    \
             let mut palette = alloc::vec![0u16; palette_len];\n    \
             for i in 0..palette_len {\n        \
                 palette[i] = u16::from_le_bytes([palette_bytes[i * 2], palette_bytes[i * 2 + 1]]);\n    \
             }\n    \
             let rgba = rlvgl_decomp::decode_rgba(w as usize, h as usize, &palette, stream)\n        \
                 .expect(\"qt_image_art: RLE decode failed\");\n    \
             let pixels: Vec<Color> = rgba\n        \
                 .chunks_exact(4)\n        \
                 .map(|c| if c[0] == 0xFF && c[1] == 0x00 && c[2] == 0xFF {\n            \
                     Color(0x00, 0x00, 0x00, 0x00)\n        \
                 } else {\n            \
                     Color(c[0], c[1], c[2], c[3])\n        \
                 })\n        \
                 .collect();\n    \
             let pixels: &'static [Color] = Vec::leak(pixels);\n    \
             ImageArt { width: w as i32, height: h as i32, pixels }\n}\n\n",
    );
}

/// QT-05g: build a predicate-bound Image — decodes both branches, builds the
/// concrete `Rc<RefCell<Image>>` at the machine-driven branch, and returns it
/// as a `dyn Widget` alongside the `Binding::Predicate` that swaps it.
fn emit_qt_predicate_image_helper(out: &mut String) {
    out.push_str(
        "/// QT-05g: build a predicate-bound Image. Decodes both branches, builds\n\
         /// the Image at the machine-driven branch, and returns it as a\n\
         /// `dyn Widget` plus the `Binding::Predicate` that swaps it on refresh.\n\
         #[rustfmt::skip]\n\
         fn qt_predicate_image(\n    \
             bounds: Rect,\n    on_rle: &'static [u8],\n    off_rle: &'static [u8],\n    \
             state_id: &'static str,\n    active: bool,\n) -> (Rc<RefCell<dyn Widget>>, Binding) {\n    \
             let on = qt_image_art(on_rle);\n    \
             let off = qt_image_art(off_rle);\n    \
             let cur = if active { on } else { off };\n    \
             let mut img = Image::new(bounds, cur.width, cur.height, cur.pixels);\n    \
             img.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
             // QML default `Image.Stretch`: scale source → bounds (8.8 fixed-point).\n    \
             let scale_x = if cur.width > 0 {\n        \
                 ((bounds.width.max(0) as i64 * 256 / cur.width as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let scale_y = if cur.height > 0 {\n        \
                 ((bounds.height.max(0) as i64 * 256 / cur.height as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let img = img.with_blit_opts(BlitOpts { scale_x, scale_y, ..BlitOpts::default() });\n    \
             let image = Rc::new(RefCell::new(img));\n    \
             let widget: Rc<RefCell<dyn Widget>> = image.clone();\n    \
             (widget, Binding::Predicate(PredicateBinding { image, state_id, on, off }))\n}\n\n",
    );
}

/// QT-05h: build a visibility-bound Image — decodes the single source asset,
/// builds the concrete `Rc<RefCell<Image>>` initialised hidden-or-shown from
/// the machine, and returns it as a `dyn Widget` plus the `Binding::Visibility`
/// that drives its visibility on refresh.
fn emit_qt_visibility_image_helper(out: &mut String) {
    out.push_str(
        "/// QT-05h: build a visibility-bound Image. Decodes the source, builds\n\
         /// the Image (initially hidden iff the bound state is inactive), and\n\
         /// returns it as a `dyn Widget` plus its `Binding::Visibility`.\n\
         #[rustfmt::skip]\n\
         fn qt_visibility_image(\n    \
             bounds: Rect,\n    rle: &'static [u8],\n    \
             state_id: &'static str,\n    visible: bool,\n) -> (Rc<RefCell<dyn Widget>>, Binding) {\n    \
             let art = qt_image_art(rle);\n    \
             let mut img = Image::new(bounds, art.width, art.height, art.pixels);\n    \
             img.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
             let scale_x = if art.width > 0 {\n        \
                 ((bounds.width.max(0) as i64 * 256 / art.width as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let scale_y = if art.height > 0 {\n        \
                 ((bounds.height.max(0) as i64 * 256 / art.height as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let mut img = img.with_blit_opts(BlitOpts { scale_x, scale_y, ..BlitOpts::default() });\n    \
             img.set_hidden(!visible);\n    \
             let image = Rc::new(RefCell::new(img));\n    \
             let widget: Rc<RefCell<dyn Widget>> = image.clone();\n    \
             (widget, Binding::Visibility(VisibilityBinding { image, state_id }))\n}\n\n",
    );
}

/// QT-05i: build a chained-predicate Image — decodes every arm asset plus the
/// default, builds the concrete `Rc<RefCell<Image>>` at the machine-driven arm
/// (first active wins, else default), and returns it as a `dyn Widget` plus the
/// `Binding::Chain` that swaps it on refresh.
fn emit_qt_predicate_chain_image_helper(out: &mut String) {
    out.push_str(
        "/// QT-05i: build a chained-predicate Image. Decodes every arm + the\n\
         /// default, builds the Image at the machine-driven arm (first active\n\
         /// wins, else default), and returns it as a `dyn Widget` plus the\n\
         /// `Binding::Chain` that swaps it on refresh.\n\
         #[rustfmt::skip]\n\
         fn qt_predicate_chain_image(\n    \
             bounds: Rect,\n    arms: &[(&'static [u8], &'static str)],\n    \
             default_rle: &'static [u8],\n    machine: &Machine,\n) -> (Rc<RefCell<dyn Widget>>, Binding) {\n    \
             let decoded: Vec<PredicateArm> = arms\n        \
                 .iter()\n        \
                 .map(|(rle, state_id)| PredicateArm { state_id, art: qt_image_art(rle) })\n        \
                 .collect();\n    \
             let default = qt_image_art(default_rle);\n    \
             let cur = decoded\n        \
                 .iter()\n        \
                 .find(|a| machine.is_active(a.state_id))\n        \
                 .map(|a| a.art)\n        \
                 .unwrap_or(default);\n    \
             let mut img = Image::new(bounds, cur.width, cur.height, cur.pixels);\n    \
             img.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
             // QML default `Image.Stretch`: scale source → bounds (8.8 fixed-point).\n    \
             let scale_x = if cur.width > 0 {\n        \
                 ((bounds.width.max(0) as i64 * 256 / cur.width as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let scale_y = if cur.height > 0 {\n        \
                 ((bounds.height.max(0) as i64 * 256 / cur.height as i64).clamp(1, 0xffff)) as u16\n    \
             } else { 256 };\n    \
             let img = img.with_blit_opts(BlitOpts { scale_x, scale_y, ..BlitOpts::default() });\n    \
             let image = Rc::new(RefCell::new(img));\n    \
             let widget: Rc<RefCell<dyn Widget>> = image.clone();\n    \
             (widget, Binding::Chain(PredicateChainBinding { image, arms: decoded, default }))\n}\n\n",
    );
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
    /// Helper-function names already emitted, to de-duplicate `build_<id>`
    /// collisions produced by inlining multiple instances of a component.
    used_fn_names: std::collections::HashSet<String>,
    /// Image assets referenced by emitted `Image` widgets, in first-use
    /// order. Each entry is `(symbol, qrc_stripped_path)`. Drives the
    /// conditional `qt_image` helper / import emission and the companion
    /// `qt_assets` module reference list.
    used_assets: Vec<AssetRef>,
    /// QT-03c dimension resolver (JS constants + root-property defaults) used
    /// by the anchor solver to evaluate non-literal `width`/`height`/margins.
    resolver: DimResolver,
    /// QT-05g: istate linkage v2 (M1P6 `step`/`is_active`/`get_var` surface)
    /// rather than v1 (`dispatch(Event)`/`dm.<f64>`). Set when attachment came
    /// from `--scxml-context` rather than a `.scjson` side-file.
    linkage_v2: bool,
    /// QT-05g: the QML context-object id (`scxmlBolero`) whose `<ctx>.<state>`
    /// predicates lower to `machine.is_active("<state>")`. Set with `linkage_v2`.
    sm_context: Option<String>,
    /// QT-05g: set once at least one `Binding::Predicate` is emitted, so the
    /// `qt_image_art` / `qt_predicate_image` helpers are emitted at the tail.
    used_predicate: bool,
    /// QT-05h: set once at least one `Binding::Visibility` is emitted, so the
    /// `qt_visibility_image` helper + `VisibilityBinding` type are emitted.
    used_visibility: bool,
    /// QT-05i: set once at least one `Binding::Chain` is emitted, so the
    /// `qt_predicate_chain_image` helper + `PredicateChainBinding`/`PredicateArm`
    /// types are emitted.
    used_predicate_chain: bool,
    /// QT-05j: `(node tag, raw QML button-event)` for every button whose
    /// handlers dispatch `<ctx>.submitBtnSetupEvent("…")`, in emit order. Drives
    /// the emitted `BUTTON_TAP_EVENTS` table.
    button_tap_events: Vec<(String, String)>,
}

/// One image asset referenced by the emitted widget tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetRef {
    /// `qt_assets` module symbol, e.g. `IMG_IMGPLAY_48`.
    symbol: String,
    /// `qrc:`-stripped source path, e.g. `Qml/Images/ImgPlay_48.png`.
    path: String,
}

impl RlvglEmitCtx {
    fn new_with_fields(state_fields: Vec<StateField>) -> Self {
        Self {
            node_index: 0,
            state_fields,
            sm_id: None,
            dm_field_ids: Vec::new(),
            used_dm_fields: Vec::new(),
            used_assets: Vec::new(),
            used_fn_names: std::collections::HashSet::new(),
            resolver: DimResolver::default(),
            linkage_v2: false,
            sm_context: None,
            used_predicate: false,
            used_visibility: false,
            used_predicate_chain: false,
            button_tap_events: Vec::new(),
        }
    }

    fn with_resolver(mut self, resolver: DimResolver) -> Self {
        self.resolver = resolver;
        self
    }

    fn with_sm(mut self, sm_id: Option<String>) -> Self {
        self.sm_id = sm_id;
        self
    }

    /// QT-05g: mark this emit as istate linkage v2 with the given QML
    /// context-object id (`scxmlBolero`).
    fn with_v2(mut self, v2: bool, sm_context: Option<String>) -> Self {
        self.linkage_v2 = v2;
        self.sm_context = sm_context;
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
        let idx = self.node_index;
        self.node_index += 1;
        let base = match &item.id {
            Some(id) => format!("build_{}", sanitize_ident(id)),
            // Index-keyed names are already unique.
            None => return format!("build_node_{idx}"),
        };
        // Component instantiation can inline several instances that share a QML
        // `id`, which would collide as `build_<id>`. Suffix collisions with the
        // node index so they stay unique. Non-colliding ids keep the bare form,
        // preserving the byte-stable emit shape for single-instance modules.
        if self.used_fn_names.insert(base.clone()) {
            base
        } else {
            let unique = format!("{base}_{idx}");
            self.used_fn_names.insert(unique.clone());
            unique
        }
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
            &mut self.used_assets,
            self.sm_context.as_deref(),
            self.linkage_v2,
            &mut self.used_predicate,
            &mut self.used_visibility,
            &mut self.used_predicate_chain,
            &mut out,
        );
        emit_skipped_summary(item, is_root, &self.state_fields, &mut out);

        let tag_lit = match &item.id {
            Some(id) => format!("Some({})", rust_str_lit(id)),
            None => "None".to_string(),
        };
        // QT-05j: record this node as a tap target when it dispatches a
        // `submitBtnSetupEvent("…")` and the module is SM-attached. The tag is
        // guaranteed present (`synthesize_button_tap_tags` ran in the pre-pass).
        if self.sm_context.is_some()
            && let Some(id) = &item.id
            && let Some(ev) = extract_submit_btn_event(item)
        {
            self.button_tap_events.push((id.clone(), ev));
        }
        out.push_str(&format!(
            "    let {mut_kw}node = WidgetNode {{\n        \
             widget,\n        children: Vec::new(),\n        tag: {tag_lit},\n    }};\n"
        ));

        // QT-03c sibling-relative extension: if any child anchors to a
        // sibling (`<id>.<edge>`), switch to the layout-solver path — resolve
        // each child's bounds into a uniquely-named `cb_<i>` Rect in dependency
        // order so later siblings can reference earlier ones, then push the
        // children in source (z) order. Parents with no sibling anchors keep
        // the legacy per-child `child_bounds` path verbatim (byte-stable
        // goldens).
        let needs_solver = item.children.iter().any(child_has_sibling_anchor);
        if needs_solver {
            emit_solved_child_bounds(&child_fns, &self.resolver, &mut out);
            for (i, (child_name, _child)) in child_fns.iter().enumerate() {
                if self.has_sm() {
                    out.push_str(&format!(
                        "    node.children.push({child_name}(cb_{i}, Rc::clone(&state), Rc::clone(&machine), bindings));\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "    node.children.push({child_name}(cb_{i}, Rc::clone(&state), label_bindings));\n"
                    ));
                }
            }
        } else {
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
    /// Lowers QML `Image` / `AnimatedImage` to
    /// [`rlvgl_widgets::image::Image`] backed by a vendored RLE asset
    /// (runtime-decoded to an owned, leaked pixel buffer). The asset is
    /// referenced through the generated `qt_assets` module by a symbol
    /// derived from the `source:` path. State-bound `source:` ternaries
    /// default to the first artwork branch (reactive swapping is wired by
    /// the integrator via the returned image handle).
    Image,
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

        // Image family → rlvgl Image widget backed by a vendored asset.
        // `BorderImage` (9-slice) is approximated as a plain Image for now.
        "Image" | "AnimatedImage" | "BorderImage" => WidgetKind::Image,

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

#[allow(clippy::too_many_arguments)]
fn emit_widget_construction(
    kind: &WidgetKind,
    item: &UiItem,
    state_fields: &[StateField],
    sm_id: Option<&str>,
    dm_field_ids: &[String],
    used_dm_fields: &mut Vec<String>,
    used_assets: &mut Vec<AssetRef>,
    sm_context: Option<&str>,
    v2: bool,
    used_predicate: &mut bool,
    used_visibility: &mut bool,
    used_predicate_chain: &mut bool,
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
                    // Non-literal `color:` (theme ref / gradient / binding)
                    // that QT-04e cannot resolve to an RGBA literal yet. Default
                    // to a transparent background rather than inheriting the
                    // opaque-white `Style::default()` — a full-bounds node whose
                    // real fill we could not determine MUST NOT bury content
                    // beneath an arbitrary white rectangle.
                    out.push_str("    // TODO QT-04e: bind color (non-literal QML expression)\n");
                    out.push_str("    w.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n");
                }
                out.push_str(
                    "    let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(w));\n",
                );
            } else {
                // Every container we cannot resolve to a literal fill — pure
                // structural nodes (`Item`, `Row`, `Column`, layouts) and
                // `Rectangle`s whose fill is a theme ref / `gradient:` we cannot
                // yet evaluate — defaults to a TRANSPARENT background. The
                // opaque-white `Style::default()` actively buries content drawn
                // beneath (root background image, sibling artwork), so only a
                // resolved literal `color:` (the branch above) paints opaque.
                // Faithful gradient/theme resolution is a separate follow-up;
                // until then transparent is strictly safer than an arbitrary
                // white rectangle. A truly bare `Rectangle {}` (QML default
                // white) is rare and accepted as transparent here.
                out.push_str(
                    "    let mut w = Container::new(bounds);\n    \
                     w.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
                     let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(w));\n",
                );
            }
        }
        WidgetKind::Label => {
            let raw_text = lookup_assignment(item, "text");
            let text_lit = raw_text.and_then(parse_string_literal);
            if let Some(text) = text_lit {
                out.push_str(&format!(
                    "    let widget: Rc<RefCell<dyn Widget>> =\n        \
                     Rc::new(RefCell::new(qt_label({}, bounds)));\n",
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
                     qt_label(\n            \
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
                     qt_label(state.borrow().{field}.clone(), bounds),\n    ));\n"
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
                     Rc::new(RefCell::new(qt_label(\"\", bounds)));\n",
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
            // rlvgl's `Button` paints its internal `Label`'s background, which
            // defaults to opaque white — so a themed QML button (whose real
            // `background:` is a translucent/gradient `Rectangle`) renders as a
            // solid white box. Clear it to transparent; the button's visual is
            // its child content (icon/text) over the dark UI. (Faithful
            // gradient/theme-colour fills are a separate follow-up.)
            out.push_str("    button.style_mut().bg_color = Color(0x00, 0x00, 0x00, 0x00);\n");
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
        WidgetKind::Image => {
            // QT-05g: when a state-machine context is linked (`--scxml-context`)
            // and the `source:` is a predicate ternary `<ctx>.<state> ? "A" : "B"`,
            // lower it to a reactive `Binding::Predicate` driven by
            // `machine.is_active("<state>")` rather than a static asset.
            let predicate = if v2 {
                sm_context
                    .zip(lookup_assignment(item, "source"))
                    .and_then(|(ctx, raw)| parse_predicate_source(raw, ctx))
            } else {
                None
            };
            if let Some((state, on_path, off_path)) = predicate {
                let on = AssetRef {
                    symbol: asset_symbol(&on_path),
                    path: on_path,
                };
                let off = AssetRef {
                    symbol: asset_symbol(&off_path),
                    path: off_path,
                };
                for a in [&on, &off] {
                    if !used_assets.iter().any(|x| x.symbol == a.symbol) {
                        used_assets.push(a.clone());
                    }
                }
                *used_predicate = true;
                out.push_str(&format!(
                    "    // QT-05g predicate-bound: source → {ctx}.{state} ? \
                     on={on_sym} : off={off_sym}\n",
                    ctx = sm_context.unwrap_or(""),
                    on_sym = on.symbol,
                    off_sym = off.symbol,
                ));
                out.push_str(&format!(
                    "    let active = machine.borrow().is_active({state_lit});\n    \
                     let (widget, __pb): (Rc<RefCell<dyn Widget>>, Binding) =\n        \
                         qt_predicate_image(bounds, qt_assets::{on_sym}, qt_assets::{off_sym}, {state_lit}, active);\n    \
                     bindings.push(__pb);\n",
                    state_lit = rust_str_lit(&state),
                    on_sym = on.symbol,
                    off_sym = off.symbol,
                ));
                return;
            }
            // QT-05i: a chained predicate `source:`
            // `<ctx>.<s1> ? "A" : <ctx>.<s2> ? "B" : "C"` (the repeat-mode icon)
            // lowers to a `Binding::Chain` — first active arm wins, else the
            // resting default — driven by `machine.is_active(<state>)`.
            let chain = if v2 {
                sm_context
                    .zip(lookup_assignment(item, "source"))
                    .and_then(|(ctx, raw)| parse_chained_predicate_source(raw, ctx))
            } else {
                None
            };
            if let Some((arms, default_path)) = chain {
                let arm_refs: Vec<(AssetRef, String)> = arms
                    .into_iter()
                    .map(|(state, path)| {
                        (
                            AssetRef {
                                symbol: asset_symbol(&path),
                                path,
                            },
                            state,
                        )
                    })
                    .collect();
                let default_ref = AssetRef {
                    symbol: asset_symbol(&default_path),
                    path: default_path,
                };
                for a in arm_refs
                    .iter()
                    .map(|(r, _)| r)
                    .chain(core::iter::once(&default_ref))
                {
                    if !used_assets.iter().any(|x| x.symbol == a.symbol) {
                        used_assets.push(a.clone());
                    }
                }
                *used_predicate_chain = true;
                out.push_str(&format!(
                    "    // QT-05i predicate-chain-bound: source → {ctx} chain \
                     [{arms}] default={def_sym}\n",
                    ctx = sm_context.unwrap_or(""),
                    arms = arm_refs
                        .iter()
                        .map(|(r, s)| format!("{s}→{}", r.symbol))
                        .collect::<Vec<_>>()
                        .join(", "),
                    def_sym = default_ref.symbol,
                ));
                out.push_str("    let __arms: &[(&'static [u8], &'static str)] = &[\n");
                for (r, state) in &arm_refs {
                    out.push_str(&format!(
                        "        (qt_assets::{}, {}),\n",
                        r.symbol,
                        rust_str_lit(state),
                    ));
                }
                out.push_str("    ];\n");
                out.push_str(&format!(
                    "    let (widget, __pcb): (Rc<RefCell<dyn Widget>>, Binding) =\n        \
                         qt_predicate_chain_image(bounds, __arms, qt_assets::{def_sym}, &machine.borrow());\n    \
                     bindings.push(__pcb);\n",
                    def_sym = default_ref.symbol,
                ));
                return;
            }
            // QT-05h: an `Image` with a literal `source:` and a predicate
            // `visible: <ctx>.<state>` lowers to a `Binding::Visibility` that
            // hides/shows the artwork from `machine.is_active("<state>")`.
            let visible_state = if v2 {
                sm_context
                    .zip(lookup_assignment(item, "visible"))
                    .and_then(|(ctx, raw)| parse_visible_predicate(raw, ctx))
            } else {
                None
            };
            if let Some(state) = visible_state
                && let Some(asset) = pick_image_source(item)
            {
                if !used_assets.iter().any(|a| a.symbol == asset.symbol) {
                    used_assets.push(asset.clone());
                }
                *used_visibility = true;
                out.push_str(&format!(
                    "    // QT-05h visibility-bound: visible → {ctx}.{state} (source {sym})\n",
                    ctx = sm_context.unwrap_or(""),
                    sym = asset.symbol,
                ));
                out.push_str(&format!(
                    "    let visible = machine.borrow().is_active({state_lit});\n    \
                     let (widget, __vb): (Rc<RefCell<dyn Widget>>, Binding) =\n        \
                         qt_visibility_image(bounds, qt_assets::{sym}, {state_lit}, visible);\n    \
                     bindings.push(__vb);\n",
                    state_lit = rust_str_lit(&state),
                    sym = asset.symbol,
                ));
                return;
            }
            // Resolve the `source:` to a single vendored asset. A literal
            // path maps directly; a state-bound ternary uses its first
            // artwork branch as the static default (the integrator can swap
            // the image at runtime via the node's tagged handle).
            match pick_image_source(item) {
                Some(asset) => {
                    if !used_assets.iter().any(|a| a.symbol == asset.symbol) {
                        used_assets.push(asset.clone());
                    }
                    out.push_str(&format!(
                        "    // QT-IMG: Image source → qt_assets::{} ({})\n",
                        asset.symbol, asset.path
                    ));
                    out.push_str(&format!(
                        "    let widget: Rc<RefCell<dyn Widget>> = \
                         qt_image(bounds, qt_assets::{});\n",
                        asset.symbol
                    ));
                }
                None => {
                    out.push_str(
                        "    // QT-IMG: Image with no resolvable source literal; \
                         emitting an empty transparent container\n",
                    );
                    // A QML `Image` with no source draws nothing; the placeholder
                    // MUST be transparent, not the opaque-white `Style::default()`.
                    out.push_str(
                        "    let mut w = Container::new(bounds);\n    \
                         w.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
                         let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(w));\n",
                    );
                }
            }
        }
        WidgetKind::Fallback => {
            out.push_str(&format!(
                "    // emitter-fallback (QT-03b): unmapped QML type `{}`\n",
                item.type_name
            ));
            // A fallback is a transparent placeholder for an unmapped type: it
            // only carries its children. It MUST NOT paint the opaque-white
            // `Style::default()` background, or an unmapped layout node that
            // receives full parent bounds (e.g. anchor-fallback `RowLayout` /
            // `Repeater`) buries everything drawn beneath it.
            out.push_str(
                "    let mut w = Container::new(bounds);\n    \
                 w.style.bg_color = Color(0x00, 0x00, 0x00, 0x00);\n    \
                 let widget: Rc<RefCell<dyn Widget>> = Rc::new(RefCell::new(w));\n",
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

/// Resolved child geometry as Rust expression strings (absolute, i.e. each
/// already includes the parent's `bounds.x`/`bounds.y` offset).
struct ResolvedBounds {
    x: String,
    y: String,
    w: String,
    h: String,
}

/// An anchor value of the form `<obj>.<edge>` where `<obj>` is a sibling id
/// (not `parent`). Drives the layout-solver activation and dependency sort.
fn anchor_sibling_target(value: &str) -> Option<&str> {
    let v = value.trim();
    let (obj, edge) = v.split_once('.')?;
    let edge_ok = matches!(
        edge,
        "left" | "right" | "top" | "bottom" | "horizontalCenter" | "verticalCenter"
    );
    if edge_ok && obj != "parent" && is_ident_str(obj) {
        Some(obj)
    } else {
        None
    }
}

/// Whether `s` is a plain identifier (no dots, ident chars only).
fn is_ident_str(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().next().map(is_ident_start).unwrap_or(false)
        && s.bytes().all(is_ident_cont)
}

/// Does any of this child's `anchors.*` assignments reference a sibling
/// (`<id>.<edge>`)? Such parents need the sibling-aware layout solver.
fn child_has_sibling_anchor(child: &UiItem) -> bool {
    child.assignments.iter().any(|a| {
        a.target.starts_with("anchors.")
            && matches!(&a.value, UiAssignmentValue::Expression { text }
                if anchor_sibling_target(text).is_some())
    })
}

/// Sibling ids this child's anchors depend on (deduplicated, source order).
fn child_anchor_deps(child: &UiItem) -> Vec<String> {
    let mut deps = Vec::new();
    for a in &child.assignments {
        if !a.target.starts_with("anchors.") {
            continue;
        }
        if let UiAssignmentValue::Expression { text } = &a.value
            && let Some(obj) = anchor_sibling_target(text)
            && !deps.iter().any(|d| d == obj)
        {
            deps.push(obj.to_string());
        }
    }
    deps
}

/// Resolve an anchor edge value (`parent.left`, `imageSource.bottom`, …) to an
/// absolute Rust expression. `base_of` maps a sibling id to its emitted Rect
/// variable name; `parent` maps to `bounds`. Returns `None` for unresolvable
/// references (unknown sibling), so the caller can fall back.
fn anchor_edge_expr(value: &str, base_of: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let v = value.trim();
    let (obj, edge) = v.split_once('.')?;
    let base = if obj == "parent" {
        "bounds".to_string()
    } else {
        base_of(obj)?
    };
    let expr = match edge {
        "left" => format!("{base}.x"),
        "right" => format!("({base}.x + {base}.width)"),
        "top" => format!("{base}.y"),
        "bottom" => format!("({base}.y + {base}.height)"),
        "horizontalCenter" => format!("({base}.x + {base}.width / 2)"),
        "verticalCenter" => format!("({base}.y + {base}.height / 2)"),
        _ => return None,
    };
    Some(expr)
}

/// Read a literal-int margin (`anchors.<name>`), defaulting to 0 when absent or
/// non-literal (e.g. a JS-constant margin the emitter can't evaluate).
/// QT-03c dimension resolver: evaluates QML width/height/margin expressions
/// that reference imported JS numeric constants (`AppConstants.js`, exposed via
/// `import "…" as AppConsts`) and root-scope property defaults
/// (e.g. `panelHeight: height / 6 - AppConsts.i_DISPLAY_PADDING`). Without this
/// such dimensions fell back to `bounds.width`/`bounds.height`, collapsing the
/// layout (an anchored sibling computed against the wrong full-parent extent
/// got zero/negative height). Fail-closed: anything it cannot resolve returns
/// `None`, preserving the prior fallback behaviour.
#[derive(Default, Clone)]
struct DimResolver {
    /// `i_DISPLAY_PADDING` → 8.0, parsed from `AppConstants.js`.
    consts: std::collections::HashMap<String, f64>,
    /// Root-scope QML property name → its raw default expression
    /// (e.g. `panelHeight` → `height / 6 - AppConsts.i_DISPLAY_PADDING`).
    root_props: std::collections::HashMap<String, String>,
    /// The root item's `id:` (e.g. `pane`), so root-property references written
    /// `pane.panelHeight` resolve the same as the bare `panelHeight` form.
    root_id: Option<String>,
    /// Directory the `qrc:`-stripped asset paths resolve against (the QML
    /// project root — the parent of the nearest ancestor `Qml/` dir). Used to
    /// read an `Image` source's natural pixel size for content-sizing.
    asset_root: Option<PathBuf>,
    /// The nearest ancestor `Qml/` directory — a second candidate root for
    /// component-relative `source:` paths (`Images/Foo.png` declared inside a
    /// `Qml/`-rooted component), plus the base for a basename fallback search.
    qml_root: Option<PathBuf>,
}

impl DimResolver {
    /// Build a resolver for `qml_path`: ingest the nearest `AppConstants.js`
    /// numeric `var` constants and capture every root-scope property's default
    /// expression (so dimension references to them resolve recursively).
    fn from_qml(qml_path: &Path, root: &UiItem) -> Self {
        let consts = parse_js_numeric_constants(qml_path);
        let mut root_props = std::collections::HashMap::new();
        for p in &root.properties {
            if let Some(d) = &p.default_value {
                root_props.insert(p.name.clone(), d.clone());
            }
        }
        // `qrc:/Qml/Images/Foo.png` → file `<projectRoot>/Qml/Images/Foo.png`,
        // where `<projectRoot>` is the parent of the nearest ancestor `Qml/`.
        let qml_root = component_search_root(qml_path);
        let asset_root = qml_root.parent().map(|p| p.to_path_buf());
        Self {
            consts,
            root_props,
            root_id: root.id.clone(),
            asset_root,
            qml_root: Some(qml_root),
        }
    }

    /// Natural pixel size of an `Image` item's source asset, read from the PNG
    /// header at emit time. `None` when the source is non-literal, the file is
    /// absent/unreadable, or not a PNG. Tries the project root and the `Qml/`
    /// root (component-relative paths lose their source dir during inlining),
    /// then a basename search under `Qml/`.
    fn image_natural_size(&self, item: &UiItem) -> Option<(i32, i32)> {
        let asset = pick_image_source(item)?;
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(root) = &self.asset_root {
            candidates.push(root.join(&asset.path));
        }
        if let Some(root) = &self.qml_root {
            candidates.push(root.join(&asset.path));
        }
        if let Some(hit) = candidates.into_iter().find(|p| p.exists()) {
            return png_dimensions(&hit);
        }
        // Fallback: locate the file by basename anywhere under the `Qml/` root.
        let base = asset.path.rsplit('/').next()?;
        let found = self
            .qml_root
            .as_ref()
            .and_then(|r| find_file_by_name(r, base, 0))?;
        png_dimensions(&found)
    }

    /// Resolve a width/height expression into a Rust `i32` expression in terms
    /// of `bounds`, or `None` if any term is unresolvable.
    fn resolve_dim(&self, expr: &str) -> Option<String> {
        let toks = tokenize_expr(expr)?;
        let mut p = ExprParser {
            toks: &toks,
            pos: 0,
            res: self,
            depth: 0,
        };
        let out = p.parse_expr()?;
        if p.pos == p.toks.len() {
            Some(out)
        } else {
            None
        }
    }

    /// Resolve a margin expression to a constant `i32` (literal or a pure
    /// numeric expression over JS constants — no `bounds` reference). Returns
    /// `None` if it references layout extents or is otherwise unresolvable.
    fn resolve_margin_i32(&self, expr: &str) -> Option<i32> {
        if let Some(n) = parse_int_literal(expr) {
            return Some(n);
        }
        let rust = self.resolve_dim(expr)?;
        // Only accept pure-constant results (no runtime `bounds` reference).
        if rust.contains("bounds") {
            return None;
        }
        // The constant path emits plain integer arithmetic; evaluate it.
        eval_const_int_rust(&rust)
    }
}

/// Recursive-descent evaluator producing Rust `i32` expressions. Grammar:
/// `expr := term (('+'|'-') term)*`, `term := factor (('*'|'/') factor)*`,
/// `factor := NUMBER | IDENT('.'IDENT)? | '(' expr ')'`.
struct ExprParser<'a> {
    toks: &'a [String],
    pos: usize,
    res: &'a DimResolver,
    depth: usize,
}

impl ExprParser<'_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(|s| s.as_str())
    }
    fn bump(&mut self) -> Option<String> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn parse_expr(&mut self) -> Option<String> {
        let mut lhs = self.parse_term()?;
        while let Some(op) = self.peek()
            && (op == "+" || op == "-")
        {
            let op = self.bump().unwrap();
            let rhs = self.parse_term()?;
            lhs = format!("({lhs} {op} {rhs})");
        }
        Some(lhs)
    }
    fn parse_term(&mut self) -> Option<String> {
        let mut lhs = self.parse_factor()?;
        while let Some(op) = self.peek()
            && (op == "*" || op == "/")
        {
            let op = self.bump().unwrap();
            let rhs = self.parse_factor()?;
            lhs = format!("({lhs} {op} {rhs})");
        }
        Some(lhs)
    }
    fn parse_factor(&mut self) -> Option<String> {
        let t = self.bump()?;
        if t == "(" {
            let e = self.parse_expr()?;
            if self.bump().as_deref() != Some(")") {
                return None;
            }
            return Some(format!("({e})"));
        }
        if let Ok(n) = t.parse::<f64>() {
            return Some(format!("{}", n.round() as i64));
        }
        // Identifier, optionally `<a>.<b>`.
        let (head, member) = if self.peek() == Some(".") {
            self.bump(); // '.'
            let m = self.bump()?;
            (t, Some(m))
        } else {
            (t, None)
        };
        self.resolve_ident(&head, member.as_deref())
    }
    fn resolve_ident(&mut self, head: &str, member: Option<&str>) -> Option<String> {
        match (head, member) {
            // `parent.width` / `parent.height` → local bounds.
            ("parent", Some("width")) => Some("bounds.width".to_string()),
            ("parent", Some("height")) => Some("bounds.height".to_string()),
            // `<rootId>.<prop>` (e.g. `pane.panelHeight`) → recurse the root
            // property's default expression, same as the bare form. NOTE: the
            // expression evaluates against the *local* `bounds` at the use site,
            // an approximation when the property derives from the root extent
            // and is referenced from a non-root-sized parent (acceptable: it
            // keeps the layout from collapsing; full root-extent threading is a
            // follow-up).
            (id, Some(name))
                if self.res.root_id.as_deref() == Some(id)
                    && self.res.root_props.contains_key(name) =>
            {
                self.recurse_root_prop(name)
            }
            // `AppConsts.<NAME>` → ingested numeric constant.
            (_, Some(name)) => self
                .res
                .consts
                .get(name)
                .map(|v| format!("{}", v.round() as i64)),
            // Bare `width` / `height` (appear inside a root-property default,
            // referring to the root's own extent → local bounds at the use site).
            ("width", None) => Some("bounds.width".to_string()),
            ("height", None) => Some("bounds.height".to_string()),
            // Bare identifier: a JS constant, or a root property to recurse into.
            (id, None) => {
                if let Some(v) = self.res.consts.get(id) {
                    return Some(format!("{}", v.round() as i64));
                }
                self.recurse_root_prop(id)
            }
        }
    }

    /// Resolve a root property name by evaluating its default expression
    /// (recursion-guarded). Returns a parenthesised Rust expression.
    fn recurse_root_prop(&self, name: &str) -> Option<String> {
        if self.depth >= 8 {
            return None;
        }
        let expr = self.res.root_props.get(name).cloned()?;
        let toks = tokenize_expr(&expr)?;
        let mut sub = ExprParser {
            toks: &toks,
            pos: 0,
            res: self.res,
            depth: self.depth + 1,
        };
        let out = sub.parse_expr()?;
        if sub.pos == sub.toks.len() {
            Some(format!("({out})"))
        } else {
            None
        }
    }
}

/// Tokenize a QML numeric expression into numbers, identifiers, `.`, operators,
/// and parens. Returns `None` on any unexpected character (fail-closed).
fn tokenize_expr(s: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() || d == '.' {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(num);
        } else if c.is_alphabetic() || c == '_' {
            let mut id = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_alphanumeric() || d == '_' {
                    id.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(id);
        } else if "+-*/().".contains(c) {
            out.push(c.to_string());
            chars.next();
        } else {
            return None;
        }
    }
    Some(out)
}

/// Evaluate a Rust integer-arithmetic expression string (only digits, spaces,
/// `+ - * /`, parens — as emitted by the constant path) into an `i32`.
fn eval_const_int_rust(s: &str) -> Option<i32> {
    let toks = tokenize_expr(s)?;
    // Reuse the parser with an empty resolver, then fold the all-numeric output.
    let empty = DimResolver::default();
    let mut p = ExprParser {
        toks: &toks,
        pos: 0,
        res: &empty,
        depth: 0,
    };
    let folded = p.parse_expr()?;
    if p.pos != p.toks.len() || folded.contains("bounds") {
        return None;
    }
    fold_int_expr(&folded)
}

/// Fold a fully-numeric Rust arithmetic expression to an `i32` (integer math,
/// matching the emitted runtime semantics).
fn fold_int_expr(s: &str) -> Option<i32> {
    let toks = tokenize_expr(s)?;
    fn expr(t: &[String], i: &mut usize) -> Option<i64> {
        let mut v = term(t, i)?;
        while let Some(op) = t.get(*i) {
            if op == "+" || op == "-" {
                *i += 1;
                let r = term(t, i)?;
                v = if op == "+" { v + r } else { v - r };
            } else {
                break;
            }
        }
        Some(v)
    }
    fn term(t: &[String], i: &mut usize) -> Option<i64> {
        let mut v = factor(t, i)?;
        while let Some(op) = t.get(*i) {
            if op == "*" || op == "/" {
                *i += 1;
                let r = factor(t, i)?;
                if op == "*" {
                    v *= r;
                } else {
                    if r == 0 {
                        return None;
                    }
                    v /= r;
                }
            } else {
                break;
            }
        }
        Some(v)
    }
    fn factor(t: &[String], i: &mut usize) -> Option<i64> {
        let tok = t.get(*i)?;
        if tok == "(" {
            *i += 1;
            let v = expr(t, i)?;
            if t.get(*i).map(|s| s.as_str()) != Some(")") {
                return None;
            }
            *i += 1;
            return Some(v);
        }
        *i += 1;
        tok.parse::<i64>().ok()
    }
    let mut i = 0;
    let v = expr(&toks, &mut i)?;
    if i == toks.len() {
        i32::try_from(v).ok()
    } else {
        None
    }
}

/// Parse numeric `var NAME = <number>` declarations from the nearest
/// `AppConstants.js` (searched in the same `Qml/` root used for component
/// resolution, then the QML file's own directory). Non-numeric `var`s and
/// anything else are ignored. Missing file → empty map.
fn parse_js_numeric_constants(qml_path: &Path) -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    let mut candidates: Vec<PathBuf> = Vec::new();
    let root = component_search_root(qml_path);
    candidates.push(root.join("AppConstants.js"));
    if let Some(parent) = qml_path.parent() {
        candidates.push(parent.join("AppConstants.js"));
    }
    let Some(path) = candidates.into_iter().find(|p| p.exists()) else {
        return out;
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return out;
    };
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("var ") else {
            continue;
        };
        let Some((name, val)) = rest.split_once('=') else {
            continue;
        };
        let name = name.trim();
        // strip trailing `;` / comment from the value
        let val = val.split([';', '/']).next().unwrap_or("").trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        if let Ok(n) = val.parse::<f64>() {
            out.insert(name.to_string(), n);
        }
    }
    out
}

/// Read a PNG's pixel dimensions from its IHDR chunk (width at byte offset 16,
/// height at 20, both big-endian u32). Returns `None` if the file is missing,
/// too short, or lacks the PNG signature.
fn png_dimensions(path: &Path) -> Option<(i32, i32)> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 24 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((i32::try_from(w).ok()?, i32::try_from(h).ok()?))
}

/// Bounded recursive search for a file named `name` under `dir` (depth ≤ 6).
fn find_file_by_name(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    if depth > 6 {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(p);
        }
    }
    for d in subdirs {
        if let Some(hit) = find_file_by_name(&d, name, depth + 1) {
            return Some(hit);
        }
    }
    None
}

fn lit_margin(child: &UiItem, name: &str, resolver: &DimResolver) -> i32 {
    let Some(raw) = lookup_assignment(child, name) else {
        return 0;
    };
    resolver.resolve_margin_i32(raw.trim()).unwrap_or(0)
}

/// Full QML anchor box-model solver: resolves a child's `x/y/w/h` from its
/// `anchors.*` (fill / centerIn / per-edge, parent- or sibling-relative) plus
/// per-edge margins and literal `x/y/width/height`. Sibling references resolve
/// through `base_of`. This is the QT-03c sibling-relative extension.
fn solve_child_bounds(
    child: &UiItem,
    base_of: &dyn Fn(&str) -> Option<String>,
    resolver: &DimResolver,
) -> ResolvedBounds {
    // anchors.fill: parent expands to all four parent edges.
    let fill = lookup_assignment(child, "anchors.fill")
        .map(|s| s.trim() == "parent")
        .unwrap_or(false);
    let center_in = lookup_assignment(child, "anchors.centerIn")
        .map(|s| s.trim() == "parent")
        .unwrap_or(false);
    let all_margin = lookup_assignment(child, "anchors.margins")
        .and_then(|raw| resolver.resolve_margin_i32(raw.trim()));

    // Per-edge anchor values, fill/centerIn synthesised into edges.
    let edge = |name: &str, fill_val: Option<&str>| -> Option<String> {
        if let Some(v) = lookup_assignment(child, name) {
            Some(v.trim().to_string())
        } else {
            fill_val.map(|s| s.to_string())
        }
    };
    let a_left = edge("anchors.left", fill.then_some("parent.left"));
    let a_right = edge("anchors.right", fill.then_some("parent.right"));
    let a_top = edge("anchors.top", fill.then_some("parent.top"));
    let a_bottom = edge("anchors.bottom", fill.then_some("parent.bottom"));
    let a_hcenter = lookup_assignment(child, "anchors.horizontalCenter")
        .map(|s| s.trim().to_string())
        .or_else(|| center_in.then(|| "parent.horizontalCenter".to_string()));
    let a_vcenter = lookup_assignment(child, "anchors.verticalCenter")
        .map(|s| s.trim().to_string())
        .or_else(|| center_in.then(|| "parent.verticalCenter".to_string()));

    let x_lit = lookup_assignment(child, "x").and_then(parse_int_literal);
    let y_lit = lookup_assignment(child, "y").and_then(parse_int_literal);
    // `implicitWidth`/`implicitHeight` are QML's content-size hints, used when
    // `width`/`height` are unset — honour them as a dimension fallback so a
    // button declaring only `implicitWidth: 65` is 65px wide, not full-parent.
    let w_lit = lookup_assignment(child, "width")
        .or_else(|| lookup_assignment(child, "implicitWidth"))
        .and_then(parse_int_literal);
    let h_lit = lookup_assignment(child, "height")
        .or_else(|| lookup_assignment(child, "implicitHeight"))
        .and_then(parse_int_literal);

    let lm = all_margin.unwrap_or_else(|| lit_margin(child, "anchors.leftMargin", resolver));
    let rm = all_margin.unwrap_or_else(|| lit_margin(child, "anchors.rightMargin", resolver));
    let tm = all_margin.unwrap_or_else(|| lit_margin(child, "anchors.topMargin", resolver));
    let bm = all_margin.unwrap_or_else(|| lit_margin(child, "anchors.bottomMargin", resolver));

    let add_margin = |expr: String, m: i32| -> String {
        if m == 0 {
            expr
        } else {
            format!("({expr} + {m})")
        }
    };
    let sub_margin = |expr: String, m: i32| -> String {
        if m == 0 {
            expr
        } else {
            format!("({expr} - {m})")
        }
    };
    // Non-literal `width:` / `height:` (e.g. `height: panelHeight`,
    // `parent.width / 2 - AppConsts.i_DISPLAY_PADDING`) resolve through the
    // DimResolver into a Rust expression over `bounds`; an `Image` with no
    // explicit extent falls back to its source's natural pixel size (QML sizes
    // an Image to its `sourceSize`, not the parent); only a genuinely
    // unresolvable extent falls back to the full parent dimension.
    let w_expr = lookup_assignment(child, "width");
    let h_expr = lookup_assignment(child, "height");
    let img_natural = if matches!(map_qml_type(&child.type_name), WidgetKind::Image) {
        resolver.image_natural_size(child)
    } else {
        None
    };
    let default_w = || match w_lit {
        Some(n) => format!("{n}"),
        None => w_expr
            .and_then(|e| resolver.resolve_dim(e.trim()))
            .or_else(|| img_natural.map(|(w, _)| format!("{w}")))
            .unwrap_or_else(|| "bounds.width".to_string()),
    };
    let default_h = || match h_lit {
        Some(n) => format!("{n}"),
        None => h_expr
            .and_then(|e| resolver.resolve_dim(e.trim()))
            .or_else(|| img_natural.map(|(_, h)| format!("{h}")))
            .unwrap_or_else(|| "bounds.height".to_string()),
    };

    // ---- X axis ----
    let left_e = a_left
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of))
        .map(|e| add_margin(e, lm));
    let right_e = a_right
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of))
        .map(|e| sub_margin(e, rm));
    let hcenter_e = a_hcenter
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of));

    let (x, w) = match (left_e, right_e, hcenter_e) {
        (Some(l), Some(r), _) => {
            let w = format!("(({r}) - ({l}))");
            (l, w)
        }
        (Some(l), None, _) => (l, default_w()),
        (None, Some(r), _) => {
            let w = default_w();
            (format!("(({r}) - ({w}))"), w)
        }
        (None, None, Some(c)) => {
            let w = default_w();
            (format!("(({c}) - ({w}) / 2)"), w)
        }
        (None, None, None) => (format!("bounds.x + {}", x_lit.unwrap_or(0)), default_w()),
    };

    // ---- Y axis ----
    let top_e = a_top
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of))
        .map(|e| add_margin(e, tm));
    let bottom_e = a_bottom
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of))
        .map(|e| sub_margin(e, bm));
    let vcenter_e = a_vcenter
        .as_deref()
        .and_then(|v| anchor_edge_expr(v, base_of));

    let (y, h) = match (top_e, bottom_e, vcenter_e) {
        (Some(t), Some(b), _) => {
            let h = format!("(({b}) - ({t}))");
            (t, h)
        }
        (Some(t), None, _) => (t, default_h()),
        (None, Some(b), _) => {
            let h = default_h();
            (format!("(({b}) - ({h}))"), h)
        }
        (None, None, Some(c)) => {
            let h = default_h();
            (format!("(({c}) - ({h}) / 2)"), h)
        }
        (None, None, None) => (format!("bounds.y + {}", y_lit.unwrap_or(0)), default_h()),
    };

    ResolvedBounds { x, y, w, h }
}

/// Emit one `let cb_<i> = Rect { … };` per child (source-indexed names) using
/// the sibling-aware [`solve_child_bounds`], ordered so a child is declared
/// after every sibling it anchors to. Children are pushed in source order by
/// the caller; only the *declaration* order is topologically sorted here.
fn emit_solved_child_bounds(
    child_fns: &[(String, &UiItem)],
    resolver: &DimResolver,
    out: &mut String,
) {
    use std::collections::BTreeMap;
    let n = child_fns.len();

    // Sibling id → source index.
    let mut id_to_idx: BTreeMap<String, usize> = BTreeMap::new();
    for (i, (_name, child)) in child_fns.iter().enumerate() {
        if let Some(id) = &child.id {
            id_to_idx.insert(id.clone(), i);
        }
    }
    let base_of = |id: &str| -> Option<String> { id_to_idx.get(id).map(|i| format!("cb_{i}")) };

    // Dependency edges to known siblings only.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, (_name, child)) in child_fns.iter().enumerate() {
        for dep_id in child_anchor_deps(child) {
            if let Some(&j) = id_to_idx.get(&dep_id)
                && j != i
            {
                deps[i].push(j);
            }
        }
    }

    // DFS topological order; stable by source index and cycle-tolerant
    // (a back-edge into an in-progress node is skipped rather than looped).
    fn visit(i: usize, deps: &[Vec<usize>], st: &mut [u8], order: &mut Vec<usize>) {
        if st[i] != 0 {
            return;
        }
        st[i] = 1;
        for &j in &deps[i] {
            visit(j, deps, st, order);
        }
        st[i] = 2;
        order.push(i);
    }
    let mut order = Vec::with_capacity(n);
    let mut st = vec![0u8; n];
    for i in 0..n {
        visit(i, &deps, &mut st, &mut order);
    }

    for &i in &order {
        let (_name, child) = &child_fns[i];
        let rb = solve_child_bounds(child, &base_of, resolver);
        out.push_str(&format!(
            "    let cb_{i} = Rect {{\n        x: {},\n        y: {},\n        \
             width: {},\n        height: {},\n    }};\n",
            rb.x, rb.y, rb.w, rb.h
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
    // `implicitWidth`/`implicitHeight` are QML's content-size hints, used when
    // `width`/`height` are unset — honour them as a dimension fallback so a
    // button declaring only `implicitWidth: 65` is 65px wide, not full-parent.
    let w_lit = lookup_assignment(child, "width")
        .or_else(|| lookup_assignment(child, "implicitWidth"))
        .and_then(parse_int_literal);
    let h_lit = lookup_assignment(child, "height")
        .or_else(|| lookup_assignment(child, "implicitHeight"))
        .and_then(parse_int_literal);

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

/// QT-05g §3 — `pub struct PredicateBinding` + `pub struct ImageArt`:
/// a reactive `Image`-artwork binding driven by `machine.is_active(state_id)`
/// (istate linkage v2). Both branches' pixels are decoded once at build time;
/// `refresh` is a pointer swap.
/// QT-05g §3 — the shared decoded-artwork record. Emitted on linkage-v2
/// modules that have any predicate or visibility binding.
fn emit_image_art_struct(out: &mut String) {
    out.push_str(
        "/// QT-05g §3 — a decoded, magenta-keyed, `'static`-leaked artwork\n\
         /// buffer plus its natural dimensions.\n\
         #[derive(Clone, Copy)]\n\
         pub struct ImageArt {\n    \
             pub width: i32,\n    \
             pub height: i32,\n    \
             pub pixels: &'static [Color],\n}\n\n",
    );
}

fn emit_predicate_binding_struct(out: &mut String) {
    out.push_str(
        "/// QT-05g §3 — reactive `Image`-source binding. Shows `on` when the\n\
         /// bound state is active, else `off`. `state_id` is the QML predicate\n\
         /// passed through verbatim to `Machine::is_active` (authority: derive).\n\
         pub struct PredicateBinding {\n    \
             pub image: Rc<RefCell<Image<'static>>>,\n    \
             pub state_id: &'static str,\n    \
             pub on: ImageArt,\n    \
             pub off: ImageArt,\n}\n\n\
         impl PredicateBinding {\n    \
             /// Re-apply this binding from the supplied machine.\n    \
             pub fn refresh(&self, machine: &Machine) {\n        \
                 let art = if machine.is_active(self.state_id) {\n            \
                     &self.on\n        \
                 } else {\n            \
                     &self.off\n        \
                 };\n        \
                 self.image\n            \
                     .borrow_mut()\n            \
                     .set_pixels(art.width, art.height, art.pixels);\n    \
             }\n}\n\n",
    );
}

/// QT-05h §3 — reactive `Image`-visibility binding. Hides the Image when the
/// bound state is inactive, shows it when active.
fn emit_visibility_binding_struct(out: &mut String) {
    out.push_str(
        "/// QT-05h §3 — reactive `Image`-visibility binding driven by\n\
         /// `Machine::is_active`. `state_id` is the QML predicate passed\n\
         /// through verbatim (authority: derive).\n\
         pub struct VisibilityBinding {\n    \
             pub image: Rc<RefCell<Image<'static>>>,\n    \
             pub state_id: &'static str,\n}\n\n\
         impl VisibilityBinding {\n    \
             /// Hide the Image when the bound state is inactive, else show it.\n    \
             pub fn refresh(&self, machine: &Machine) {\n        \
                 self.image\n            \
                     .borrow_mut()\n            \
                     .set_hidden(!machine.is_active(self.state_id));\n    \
             }\n}\n\n",
    );
}

/// QT-05i §3 — reactive `Image`-source binding over a CHAIN of predicates:
/// the first `arm` whose `state_id` is active wins; if none are active the
/// `default` (resting) artwork shows. Models the repeat-mode icon
/// (`mediaRepeatTrack ? … : mediaRepeatFolder ? … : NoRepeat`).
fn emit_predicate_chain_binding_struct(out: &mut String) {
    out.push_str(
        "/// QT-05i §3 — one arm of a chained predicate binding: the artwork\n\
         /// shown when `state_id` is the first active arm.\n\
         pub struct PredicateArm {\n    \
             pub state_id: &'static str,\n    \
             pub art: ImageArt,\n}\n\n\
         /// QT-05i §3 — reactive `Image`-source binding driven by a chain of\n\
         /// `Machine::is_active` checks (first-true wins; `default` is the\n\
         /// resting else). Each `state_id` is the QML predicate passed through\n\
         /// verbatim (authority: derive).\n\
         pub struct PredicateChainBinding {\n    \
             pub image: Rc<RefCell<Image<'static>>>,\n    \
             pub arms: Vec<PredicateArm>,\n    \
             pub default: ImageArt,\n}\n\n\
         impl PredicateChainBinding {\n    \
             /// Re-apply this binding: show the first active arm's artwork,\n    \
             /// else the resting default.\n    \
             pub fn refresh(&self, machine: &Machine) {\n        \
                 let art = self\n            \
                     .arms\n            \
                     .iter()\n            \
                     .find(|a| machine.is_active(a.state_id))\n            \
                     .map(|a| &a.art)\n            \
                     .unwrap_or(&self.default);\n        \
                 self.image\n            \
                     .borrow_mut()\n            \
                     .set_pixels(art.width, art.height, art.pixels);\n    \
             }\n}\n\n",
    );
}

/// QT-05g §3 / QT-05h §3 / QT-05i §3 — sealed binding enum for linkage-v2
/// modules: `Label` always; `Predicate` (state-driven artwork), `Visibility`
/// (state-driven hide/show), and `Chain` (chained-predicate artwork) when those
/// bindings are present.
fn emit_binding_enum_v2(
    out: &mut String,
    used_predicate: bool,
    used_visibility: bool,
    used_predicate_chain: bool,
) {
    out.push_str(
        "/// QT-05g §3 / QT-05h §3 / QT-05i §3 — sealed enum over the binding\n\
         /// sources reactive `refresh_bindings` knows how to drive.\n\
         pub enum Binding {\n    \
             Label(LabelBinding),\n",
    );
    if used_predicate {
        out.push_str("    Predicate(PredicateBinding),\n");
    }
    if used_visibility {
        out.push_str("    Visibility(VisibilityBinding),\n");
    }
    if used_predicate_chain {
        out.push_str("    Chain(PredicateChainBinding),\n");
    }
    out.push_str("}\n\n");
}

/// Emit the QT-04e §7 / QT-05c §7 / QT-05g §7 / QT-05h §7 `pub fn
/// refresh_bindings` free function. Signature varies by SM presence + linkage
/// version; the v2 match arms track the emitted `Binding` variants.
fn emit_refresh_bindings_fn(
    out: &mut String,
    has_sm: bool,
    v2: bool,
    used_predicate: bool,
    used_visibility: bool,
    used_predicate_chain: bool,
) {
    if has_sm && v2 {
        let mut arms =
            String::from("                         Binding::Label(lb) => lb.refresh(&s),\n");
        if used_predicate {
            arms.push_str("                         Binding::Predicate(pb) => pb.refresh(&m),\n");
        }
        if used_visibility {
            arms.push_str("                         Binding::Visibility(vb) => vb.refresh(&m),\n");
        }
        if used_predicate_chain {
            arms.push_str("                         Binding::Chain(cb) => cb.refresh(&m),\n");
        }
        out.push_str(&format!(
            "/// Re-apply every QT-04e / QT-05g / QT-05h binding from the\n\
             /// current state and machine. Idempotent; safe to call after any\n\
             /// `machine.step(…)`. No-op when `bindings` is empty.\n\
             #[rustfmt::skip]\n\
             pub fn refresh_bindings(state: &Rc<RefCell<ScreenState>>, machine: &Rc<RefCell<Machine>>, bindings: &[Binding]) {{\n    \
                 let s = state.borrow();\n    \
                 let m = machine.borrow();\n    \
                 for b in bindings {{\n        \
                     match b {{\n\
{arms}    \
                     }}\n    \
                 }}\n}}\n\n"
        ));
        return;
    }
    if has_sm {
        // Linkage v1 (`<sm>_gen`): `Machine`/`MachineBinding` read DM via
        // `m.dm`. `#[rustfmt::skip]` because the signature exceeds rustfmt's
        // wrap threshold; one line keeps the output byte-stable.
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
        if self.match_keyword("enum") {
            // QML enum declaration: `enum Name { A, B = 2, C }`. Enums are
            // referenced only inside opaque expression text, so the structural
            // emit doesn't need them — parse and discard.
            let _name = self.read_ident().context("enum name")?;
            self.skip_trivia();
            let _body = self.read_balanced(b'{', b'}')?;
            return Ok(());
        }
        if self.match_keyword("required") {
            // `required property <ty> <name>` (optionally combined with
            // `default`/`readonly`), or a bare `required <inheritedName>` that
            // marks an inherited property as required (no value follows).
            if self.match_keyword("default") {
                return self.finish_property_decl(item, true, false);
            }
            if self.match_keyword("readonly") {
                return self.finish_property_decl(item, false, true);
            }
            if self.match_keyword("property") {
                return self.finish_property_decl(item, false, false);
            }
            let _name = self.read_ident().context("required property name")?;
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
        // `<Type> on <property> { ... }` — a property value source (Behavior,
        // NumberAnimation on x, …). Non-visual; consume and discard so the
        // structural emit isn't tripped by animation primitives.
        if starts_uppercase(&lead) && self.match_keyword("on") {
            let _prop = self.read_dotted_ident().context("`on` target property")?;
            self.skip_trivia();
            let _body = self.read_balanced(b'{', b'}')?;
            return Ok(());
        }
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
        // Last two significant (non-whitespace) bytes consumed, newest first.
        // Used to decide whether a bare newline continues the expression onto
        // the next line (multi-line bindings, e.g. chained ternaries).
        let mut last_sig: Option<u8> = None;
        let mut last_sig2: Option<u8> = None;
        while let Some(b) = self.peek() {
            match b {
                b'"' | b'\'' => {
                    self.skip_string();
                    // A string literal is a complete operand; record the
                    // closing quote so a trailing-operator check sees it.
                    last_sig2 = last_sig;
                    last_sig = Some(b'"');
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
                b'\n' if paren == 0 && bracket == 0 && brace == 0 => {
                    // A newline ends the binding unless the expression is
                    // syntactically incomplete (QML/JS line continuation).
                    if !self.expr_newline_continues(last_sig, last_sig2) {
                        break;
                    }
                    // Continuation: fall through to consume the newline.
                }
                _ => {}
            }
            if !matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                last_sig2 = last_sig;
                last_sig = Some(b);
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

    /// Decide whether a depth-0 newline continues the current binding
    /// expression onto the next line, mirroring JS/QML automatic-semicolon
    /// rules closely enough for designer-authored bindings.
    ///
    /// Continues when either the line's last significant byte demands a right
    /// operand (`a +`, `cond ?`, `x :`) — guarding against postfix `++`/`--` —
    /// or the next line begins with a binary/ternary operator that can never
    /// start a new QML member (`. + - * / ? :` …).
    fn expr_newline_continues(&self, last_sig: Option<u8>, last_sig2: Option<u8>) -> bool {
        if let Some(b) = last_sig {
            let trailing = matches!(
                b,
                b'?' | b':'
                    | b','
                    | b'.'
                    | b'+'
                    | b'-'
                    | b'*'
                    | b'/'
                    | b'%'
                    | b'&'
                    | b'|'
                    | b'^'
                    | b'='
                    | b'<'
                    | b'>'
                    | b'~'
            );
            if trailing {
                let postfix = matches!((last_sig2, b), (Some(b'+'), b'+') | (Some(b'-'), b'-'));
                if !postfix {
                    return true;
                }
            }
        }
        match self.peek_next_significant() {
            Some(b) => matches!(
                b,
                b'?' | b':'
                    | b','
                    | b'.'
                    | b'+'
                    | b'-'
                    | b'*'
                    | b'/'
                    | b'%'
                    | b'&'
                    | b'|'
                    | b'^'
                    | b'='
                    | b'<'
                    | b'>'
            ),
            None => false,
        }
    }

    /// Peek the next non-whitespace, non-comment byte from the current position
    /// without consuming anything.
    fn peek_next_significant(&self) -> Option<u8> {
        let mut i = self.pos;
        while i < self.src.len() {
            let b = self.src[i];
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                i += 1;
            } else if b == b'/' && self.src.get(i + 1) == Some(&b'/') {
                i += 2;
                while i < self.src.len() && self.src[i] != b'\n' {
                    i += 1;
                }
            } else if b == b'/' && self.src.get(i + 1) == Some(&b'*') {
                i += 2;
                while i + 1 < self.src.len() && !(self.src[i] == b'*' && self.src[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            } else {
                return Some(b);
            }
        }
        None
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
