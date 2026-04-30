// SPDX-License-Identifier: MIT
//! QT-05 scjson subset.
//!
//! Hand-rolled, on-disk-wire-compatible serde types for the
//! 10-element scjson subset frozen by `docs/qt-support/05-state-machines.md`
//! §5. These are **not** a Cargo dep on the upstream `scjson` crate —
//! the upstream lives at `vendor/scjson/` as a reference-only
//! submodule (BSD-1-Clause) and never gets linked. Field names and
//! `#[serde(rename = …)]` attributes mirror the **Python pydantic
//! wire format** (`vendor/scjson/py/scjson/pydantic/`) since that's
//! what `softoboros/backend/istate-codegen` parses on disk:
//!
//! - `<raise>` → JSON key `raise` (Rust field `raise_value` to dodge
//!   the `raise` keyword in the future, with `#[serde(rename)]`).
//! - `<if>` / `<else>` / `<elseif>` → JSON keys `if` / `else` /
//!   `elseif` (deferred per QT-05 §5; only `<raise>` is in the
//!   subset).
//! - `type` attribute → JSON key `type` (Rust field `type_value`
//!   with `#[serde(rename)]`).
//! - `<final>` → JSON key `final` (deferred).
//!
//! The Rust crate at `vendor/scjson/rust/` *does* expose these as
//! `raise_value` / `if_value` / `type_value` JSON keys (it lacks
//! the rename plumbing the Python side has). For QT-05 we follow
//! the **Python wire shape** because istate is the consumer.
//!
//! Element coverage (matches QT-05 §5):
//!
//! | Element       | Type           |
//! | ------------- | -------------- |
//! | `<scxml>`     | [`Scxml`]      |
//! | `<state>`     | [`State`]      |
//! | `<transition>`| [`Transition`] |
//! | `<datamodel>` | [`Datamodel`]  |
//! | `<data>`      | [`Data`]       |
//! | `<onentry>`   | [`Onentry`]    |
//! | `<onexit>`    | [`Onexit`]     |
//! | `<assign>`    | [`Assign`]     |
//! | `<raise>`     | [`Raise`]      |
//! | `<script>`    | [`Script`]     |
//!
//! Deferred elements (parallel/final/initial/history/invoke/send/
//! cancel/log/if/elseif/else/foreach/param/finalize/donedata/content)
//! pass through as opaque `serde_json::Value` in `other_element` /
//! `other_attributes`.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `scjson` subset version, bumps when this on-disk shape changes.
/// Tracks `ISTATE_LINKAGE_VERSION` from QT-05 §6 — they bump
/// together when istate's parser changes.
pub const QT_SCJSON_SUBSET_VERSION: u32 = 1;

/// `<scxml>` root element. Per QT-05 §5 we surface only the four
/// child element categories the subset names; everything else is
/// preserved opaquely so a round-trip through istate-codegen is
/// faithful.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scxml {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datamodel: Vec<Datamodel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script: Vec<Script>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Value>,
    /// Mirrors pydantic's `datamodel_attribute` (XML attribute
    /// `datamodel="ecmascript|null|…"`). Defaults to `"null"`.
    #[serde(
        default = "default_datamodel_attribute",
        rename = "datamodel_attribute"
    )]
    pub datamodel_attribute: String,
    /// Carries any deferred-element children verbatim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    /// Carries any unknown attributes verbatim.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

fn default_datamodel_attribute() -> String {
    "null".to_string()
}

impl Default for Scxml {
    fn default() -> Self {
        Self {
            state: Vec::new(),
            datamodel: Vec::new(),
            script: Vec::new(),
            initial: Vec::new(),
            name: None,
            version: None,
            datamodel_attribute: default_datamodel_attribute(),
            other_element: Vec::new(),
            other_attributes: Map::new(),
        }
    }
}

/// `<state>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub onentry: Vec<Onentry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub onexit: Vec<Onexit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transition: Vec<Transition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datamodel: Vec<Datamodel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<transition>`. The on-disk `target` attribute is a list per
/// pydantic (XSD `tokens=true` → string-split). Keep that shape.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Transition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cond: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target: Vec<String>,
    /// `<raise>` children. JSON key is `raise` (pydantic alias).
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "raise")]
    pub raise_value: Vec<Raise>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assign: Vec<Assign>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script: Vec<Script>,
    /// `type` attribute. JSON key is `type` (pydantic alias).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<datamodel>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Datamodel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<Data>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<data>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Value>,
}

/// `<onentry>` — same shape as `<onexit>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Onentry {
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "raise")]
    pub raise_value: Vec<Raise>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assign: Vec<Assign>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script: Vec<Script>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<onexit>`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Onexit {
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "raise")]
    pub raise_value: Vec<Raise>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assign: Vec<Assign>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script: Vec<Script>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_element: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<assign>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assign {
    #[serde(default)]
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    /// `type` attribute. JSON key is `type` (pydantic alias).
    /// Pydantic default is `"replacechildren"`.
    #[serde(default = "default_assign_type", rename = "type")]
    pub type_value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attr: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

fn default_assign_type() -> String {
    "replacechildren".to_string()
}

impl Default for Assign {
    fn default() -> Self {
        Self {
            location: String::new(),
            expr: None,
            type_value: default_assign_type(),
            attr: None,
            content: Vec::new(),
            other_attributes: Map::new(),
        }
    }
}

/// `<raise>`. SCXML requires the `event` attribute.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Raise {
    #[serde(default)]
    pub event: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

/// `<script>`. The on-disk `content` is a list of JSON values
/// (typically strings — body text — or nested object payloads).
/// Per QT-05 §3 we surface `name` as a deferred convention: istate
/// uses `name` to map a `<script>` to an `Externals` callout method.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Script {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub other_attributes: Map<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: round-trip a minimal SCXML document through
    /// `serde_json` without losing any field.
    #[test]
    fn scxml_roundtrip_minimal() {
        let doc = Scxml {
            state: vec![State {
                id: Some("idle".to_string()),
                transition: vec![Transition {
                    event: Some("go".to_string()),
                    target: vec!["running".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            initial: vec!["idle".to_string()],
            ..Default::default()
        };
        let s = serde_json::to_string(&doc).expect("serialize");
        let back: Scxml = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(doc, back);
    }

    /// QT-05 §3 / §5 wire-shape test: confirm the JSON keys for
    /// `<raise>` (`raise`), `<assign type=…>` (`type`), and the
    /// transition `target` array form match the pydantic alias map.
    #[test]
    fn raise_and_type_renames_render_as_canonical_scxml_keys() {
        let onentry = Onentry {
            raise_value: vec![Raise {
                event: "tick".to_string(),
                ..Default::default()
            }],
            assign: vec![Assign {
                location: "count".to_string(),
                expr: Some("count + 1".to_string()),
                type_value: "replacechildren".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let s = serde_json::to_string(&onentry).expect("serialize");
        // <raise> → "raise", <assign type=…> → "type"
        assert!(s.contains(r#""raise""#), "expected `raise` key, got: {s}");
        assert!(
            !s.contains(r#""raise_value""#),
            "expected NO `raise_value` key, got: {s}"
        );
        assert!(
            s.contains(r#""type":"replacechildren""#),
            "expected `\"type\":\"replacechildren\"`, got: {s}"
        );
    }

    /// Deferred elements pass through as `other_element` opaque
    /// JSON without dropping fields. This is the round-trip
    /// faithfulness guarantee in QT-05 §5.
    #[test]
    fn deferred_elements_pass_through_unchanged() {
        // pydantic emits `<parallel>` (a deferred element) as
        // `parallel: [...]` at scxml top level. Our subset stores
        // it as opaque other_element entries.
        let raw = r#"{
            "state": [{"id": "s1"}],
            "other_element": [{"parallel": [{"id": "p1"}]}]
        }"#;
        let doc: Scxml = serde_json::from_str(raw).expect("parse");
        assert_eq!(doc.state.len(), 1);
        assert_eq!(doc.other_element.len(), 1);
        let s = serde_json::to_string(&doc).expect("re-serialize");
        // Confirm parallel survived.
        assert!(s.contains(r#""parallel""#));
    }

    /// `<scxml datamodel="ecmascript">` round-trips via the
    /// `datamodel_attribute` field (pydantic's metadata-name dodge).
    #[test]
    fn datamodel_attribute_default_and_override() {
        let default_doc = Scxml::default();
        // The default value is "null" so it serializes (since we
        // don't skip on default-string-equality).
        let s = serde_json::to_string(&default_doc).expect("serialize");
        assert!(s.contains(r#""datamodel_attribute":"null""#));

        let custom = Scxml {
            datamodel_attribute: "ecmascript".to_string(),
            ..Default::default()
        };
        let s = serde_json::to_string(&custom).expect("serialize");
        assert!(s.contains(r#""datamodel_attribute":"ecmascript""#));
    }
}
