// SPDX-License-Identifier: MIT
//! Chakra UI theme parser for rlvgl-creator.
//!
//! Reads a Chakra `defineTokens` / `defineSemanticTokens` TypeScript file
//! and emits a `tokens.yaml` compatible with the `svelte tokens` pipeline.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

/// Parse a Chakra theme TypeScript file and write a `tokens.yaml` into `out`.
pub(crate) fn ingest(input: &Path, out: &Path) -> Result<()> {
    let source = fs::read_to_string(input)?;
    let base_tokens = extract_define_tokens(&source)?;
    let semantic = extract_semantic_tokens(&source)?;

    // Build the full color palette: Chakra defaults + user-defined colors.
    // Only the `colors` subtree of defineTokens goes into the palette;
    // fonts/radii are separate namespaces.
    let mut palette: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in CHAKRA_DEFAULTS {
        palette.insert(k.to_string(), v.to_string());
    }
    if let Some(JsValue::Obj(colors_obj)) = base_tokens.get("colors") {
        flatten_colors(colors_obj, "", &mut palette);
    }

    // Multi-pass reference resolution (handles chained refs)
    for _ in 0..4 {
        let snapshot = palette.clone();
        let mut changed = false;
        for val in palette.values_mut() {
            if val.starts_with('{') && val.ends_with('}')
                && let Some(resolved) = resolve_ref(val, &snapshot) {
                    *val = resolved;
                    changed = true;
                }
        }
        if !changed {
            break;
        }
    }

    // Semantic tokens live under `colors` key in defineSemanticTokens
    let sem_colors = match semantic.get("colors") {
        Some(JsValue::Obj(obj)) => obj.clone(),
        _ => BTreeMap::new(),
    };

    // Collect raw references from semantic tokens (before resolving)
    let mut default_refs: BTreeMap<String, String> = BTreeMap::new();
    let mut dark_refs: BTreeMap<String, String> = BTreeMap::new();
    collect_semantic_refs(&sem_colors, "", &mut default_refs, &mut dark_refs);

    // Resolve default values against the base palette
    let default_colors = resolve_refs_map(&default_refs, &palette);

    // Build dark palette: base palette + resolved dark semantic values.
    // First pass resolves direct palette refs (brand.400, gray.800, etc.)
    let dark_pass1 = resolve_refs_map(&dark_refs, &palette);
    // Merge into a dark-aware palette so cross-refs between semantic tokens
    // resolve to their dark values (e.g. {colors.heroTitle} → heroTitle._dark)
    let mut dark_palette = palette.clone();
    dark_palette.extend(dark_pass1);
    // Second pass re-resolves against the dark-aware palette
    let dark_colors = resolve_refs_map(&dark_refs, &dark_palette);

    // Assemble output colors: user-defined base colors + resolved semantics
    let mut all_colors: BTreeMap<String, String> = BTreeMap::new();

    // Include user-defined base colors (not Chakra defaults)
    for (k, v) in &palette {
        if !CHAKRA_DEFAULTS.iter().any(|(dk, _)| *dk == k.as_str()) {
            all_colors.insert(k.clone(), v.clone());
        }
    }
    // Semantic defaults override base
    for (k, v) in &default_colors {
        all_colors.insert(k.clone(), v.clone());
    }

    // Ensure required rlvgl keys
    if !all_colors.contains_key("primary")
        && let Some(v) = default_colors
            .get("brand.solid")
            .or(palette.get("brand.500"))
        {
            all_colors.insert("primary".into(), v.clone());
        }
    if !all_colors.contains_key("background") {
        all_colors.insert(
            "background".into(),
            palette.get("gray.900").cloned().unwrap_or("#171923".into()),
        );
    }
    if !all_colors.contains_key("text")
        && let Some(v) = default_colors.get("text") {
            all_colors.insert("text".into(), v.clone());
        }

    // Build YAML output
    let mut yaml =
        String::from("# Auto-generated from Chakra theme by rlvgl-creator\nversion: 1\n");
    yaml.push_str("colors:\n");
    for (k, v) in &all_colors {
        let key = sanitize_key(k);
        yaml.push_str(&format!("  {}: \"{}\"\n", key, v));
    }

    yaml.push_str("spacing:\n  xs: 2\n  sm: 4\n  md: 8\n  lg: 16\n  xl: 24\n");
    yaml.push_str("radii:\n  none: 0\n  sm: 2\n  md: 4\n  lg: 8\n  full: 255\n");
    yaml.push_str("fonts:\n  small: \"tiny\"\n  body: \"default\"\n  heading: \"bold\"\n");

    if !dark_colors.is_empty() {
        yaml.push_str("modes:\n  dark:\n    colors:\n");
        for (k, v) in &dark_colors {
            let key = sanitize_key(k);
            yaml.push_str(&format!("      {}: \"{}\"\n", key, v));
        }
    }

    fs::create_dir_all(out)?;
    let out_path = out.join("tokens.yaml");
    fs::write(&out_path, &yaml)?;
    println!("Generated {}", out_path.display());
    Ok(())
}

// ── JS object literal parser ──────────────────────────────────────

type JsObj = BTreeMap<String, JsValue>;

#[derive(Debug, Clone)]
enum JsValue {
    Str(String),
    Obj(JsObj),
}

/// Extract the object passed to `defineTokens({...})`.
fn extract_define_tokens(source: &str) -> Result<JsObj> {
    extract_call_arg(source, "defineTokens")
}

/// Extract the object passed to `defineSemanticTokens({...})`.
fn extract_semantic_tokens(source: &str) -> Result<JsObj> {
    extract_call_arg(source, "defineSemanticTokens")
}

fn extract_call_arg(source: &str, fn_name: &str) -> Result<JsObj> {
    let pattern = format!("{}({{", fn_name);
    let start = match source.find(&pattern) {
        Some(pos) => pos + fn_name.len() + 1, // skip past the opening `(`
        None => bail!("could not find `{}({{` in source", fn_name),
    };
    let slice = &source[start..];
    let (obj, _) = parse_js_obj(slice)?;
    Ok(obj)
}

/// Parse a JS object literal from the beginning of `s`. Returns the object
/// and the number of bytes consumed (including the closing `}`).
fn parse_js_obj(s: &str) -> Result<(JsObj, usize)> {
    let s = s.trim_start();
    if !s.starts_with('{') {
        bail!("expected '{{' at start of JS object");
    }
    let mut map = BTreeMap::new();
    let mut pos = 1; // skip '{'

    loop {
        let rest = &s[pos..];
        let rest = rest.trim_start();
        pos = s.len() - rest.len();

        if rest.starts_with('}') {
            pos += 1;
            break;
        }
        if rest.is_empty() {
            bail!("unexpected end of input in JS object");
        }

        // Skip commas
        if rest.starts_with(',') {
            pos += 1;
            continue;
        }
        // Skip line comments
        if rest.starts_with("//") {
            if let Some(nl) = rest.find('\n') {
                pos += nl + 1;
            } else {
                pos += rest.len();
            }
            continue;
        }

        // Parse key
        let (key, consumed) = parse_js_key(rest)?;
        pos += consumed;

        let rest = &s[pos..];
        let rest = rest.trim_start();
        pos = s.len() - rest.len();

        // Expect ':'
        if !rest.starts_with(':') {
            bail!("expected ':' after key '{}'", key);
        }
        pos += 1;

        let rest = &s[pos..];
        let rest = rest.trim_start();
        pos = s.len() - rest.len();

        // Parse value
        if rest.starts_with('{') {
            let (obj, consumed) = parse_js_obj(rest)?;
            pos += consumed;
            map.insert(key, JsValue::Obj(obj));
        } else if rest.starts_with('"') || rest.starts_with('\'') {
            let (val, consumed) = parse_js_string(rest)?;
            pos += consumed;
            map.insert(key, JsValue::Str(val));
        } else {
            // Skip unrecognized values until comma/closing brace/newline
            let end = rest
                .find([',', '}', '\n'])
                .unwrap_or(rest.len());
            let raw = rest[..end].trim();
            pos += end;
            map.insert(key, JsValue::Str(raw.to_string()));
        }
    }

    Ok((map, pos))
}

fn parse_js_key(s: &str) -> Result<(String, usize)> {
    if s.starts_with('"') || s.starts_with('\'') {
        parse_js_string(s)
    } else {
        // Unquoted key: ident chars or digits (for Chakra numeric keys like `50`, `100`)
        let end = s
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
            .unwrap_or(s.len());
        if end == 0 {
            bail!("empty unquoted key");
        }
        Ok((s[..end].to_string(), end))
    }
}

fn parse_js_string(s: &str) -> Result<(String, usize)> {
    let quote = s.as_bytes()[0] as char;
    let mut pos = 1;
    let mut val = String::new();
    while pos < s.len() {
        let ch = s.as_bytes()[pos] as char;
        if ch == '\\' && pos + 1 < s.len() {
            val.push(s.as_bytes()[pos + 1] as char);
            pos += 2;
        } else if ch == quote {
            pos += 1;
            return Ok((val, pos));
        } else {
            val.push(ch);
            pos += 1;
        }
    }
    bail!("unterminated string");
}

// ── Token flattening & resolution ─────────────────────────────────

/// Flatten a nested JS color object into `"path.path" → "value"` pairs.
fn flatten_colors(obj: &JsObj, prefix: &str, out: &mut BTreeMap<String, String>) {
    for (key, val) in obj {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        match val {
            JsValue::Str(s) => {
                out.insert(path, s.clone());
            }
            JsValue::Obj(inner) => {
                // Chakra leaf node: { value: "..." }
                if let Some(JsValue::Str(v)) = inner.get("value") {
                    out.insert(path, v.clone());
                } else {
                    flatten_colors(inner, &path, out);
                }
            }
        }
    }
}

/// Resolve a `{colors.brand.600}` reference against the palette.
fn resolve_ref(reference: &str, palette: &BTreeMap<String, String>) -> Option<String> {
    let inner = reference.trim_start_matches('{').trim_end_matches('}');
    // Strip "colors." prefix if present
    let key = inner.strip_prefix("colors.").unwrap_or(inner);
    if let Some(val) = palette.get(key) {
        if val.starts_with('{') && val.ends_with('}') {
            // Recursive resolution (one level)
            return resolve_ref(val, palette);
        }
        return Some(val.clone());
    }
    None
}

/// Collect raw semantic token references (before resolution).
/// Returns (default_refs, dark_refs) where values are the raw `{colors.xxx}` strings.
fn collect_semantic_refs(
    obj: &JsObj,
    prefix: &str,
    default_out: &mut BTreeMap<String, String>,
    dark_out: &mut BTreeMap<String, String>,
) {
    for (key, val) in obj {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        match val {
            JsValue::Obj(inner) => {
                let has_default = inner.contains_key("default");
                let has_dark = inner.contains_key("_dark");

                if has_default || has_dark {
                    if let Some(JsValue::Obj(def)) = inner.get("default")
                        && let Some(JsValue::Str(v)) = def.get("value") {
                            default_out.insert(path.clone(), v.clone());
                        }
                    if let Some(JsValue::Obj(dark)) = inner.get("_dark")
                        && let Some(JsValue::Str(v)) = dark.get("value") {
                            dark_out.insert(path.clone(), v.clone());
                        }
                } else {
                    // Nested namespace (e.g. brand: { solid: ..., contrast: ... })
                    collect_semantic_refs(inner, &path, default_out, dark_out);
                }
            }
            JsValue::Str(_) => {}
        }
    }
}

/// Resolve raw references against a palette, returning resolved values.
fn resolve_refs_map(
    refs: &BTreeMap<String, String>,
    palette: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    refs.iter()
        .map(|(k, v)| {
            let resolved = resolve_ref(v, palette).unwrap_or_else(|| v.clone());
            (k.clone(), resolved)
        })
        .collect()
}

/// Sanitize a dot-path key to a YAML-safe identifier.
fn sanitize_key(key: &str) -> String {
    key.replace('.', "-")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

// ── Chakra default color palette ──────────────────────────────────
// Subset of Chakra UI v3 default colors used for reference resolution.

const CHAKRA_DEFAULTS: &[(&str, &str)] = &[
    ("white", "#ffffff"),
    ("black", "#000000"),
    // Gray
    ("gray.50", "#f7fafc"),
    ("gray.100", "#edf2f7"),
    ("gray.200", "#e2e8f0"),
    ("gray.300", "#cbd5e0"),
    ("gray.400", "#a0aec0"),
    ("gray.500", "#718096"),
    ("gray.600", "#4a5568"),
    ("gray.700", "#2d3748"),
    ("gray.800", "#1a202c"),
    ("gray.900", "#171923"),
    // Red
    ("red.50", "#fff5f5"),
    ("red.100", "#fed7d7"),
    ("red.200", "#feb2b2"),
    ("red.300", "#fc8181"),
    ("red.400", "#f56565"),
    ("red.500", "#e53e3e"),
    ("red.600", "#c53030"),
    ("red.700", "#9b2c2c"),
    ("red.800", "#822727"),
    ("red.900", "#63171b"),
    // Orange
    ("orange.50", "#fffaf0"),
    ("orange.100", "#feebc8"),
    ("orange.200", "#fbd38d"),
    ("orange.300", "#f6ad55"),
    ("orange.400", "#ed8936"),
    ("orange.500", "#dd6b20"),
    ("orange.600", "#c05621"),
    ("orange.700", "#9c4221"),
    ("orange.800", "#7b341e"),
    ("orange.900", "#652b19"),
    // Yellow
    ("yellow.50", "#fffff0"),
    ("yellow.100", "#fefcbf"),
    ("yellow.200", "#faf089"),
    ("yellow.300", "#f6e05e"),
    ("yellow.400", "#ecc94b"),
    ("yellow.500", "#d69e2e"),
    ("yellow.600", "#b7791f"),
    ("yellow.700", "#975a16"),
    ("yellow.800", "#744210"),
    ("yellow.900", "#5f370e"),
    // Green
    ("green.50", "#f0fff4"),
    ("green.100", "#c6f6d5"),
    ("green.200", "#9ae6b4"),
    ("green.300", "#68d391"),
    ("green.400", "#48bb78"),
    ("green.500", "#38a169"),
    ("green.600", "#2f855a"),
    ("green.700", "#276749"),
    ("green.800", "#22543d"),
    ("green.900", "#1c4532"),
    // Teal
    ("teal.50", "#e6fffa"),
    ("teal.100", "#b2f5ea"),
    ("teal.200", "#81e6d9"),
    ("teal.300", "#4fd1c5"),
    ("teal.400", "#38b2ac"),
    ("teal.500", "#319795"),
    ("teal.600", "#2c7a7b"),
    ("teal.700", "#285e61"),
    ("teal.800", "#234e52"),
    ("teal.900", "#1d4044"),
    // Blue
    ("blue.50", "#ebf8ff"),
    ("blue.100", "#bee3f8"),
    ("blue.200", "#90cdf4"),
    ("blue.300", "#63b3ed"),
    ("blue.400", "#4299e1"),
    ("blue.500", "#3182ce"),
    ("blue.600", "#2b6cb0"),
    ("blue.700", "#2c5282"),
    ("blue.800", "#2a4365"),
    ("blue.900", "#1a365d"),
    // Cyan
    ("cyan.50", "#edfdfd"),
    ("cyan.100", "#c4f1f9"),
    ("cyan.200", "#9decf9"),
    ("cyan.300", "#76e4f7"),
    ("cyan.400", "#0bc5ea"),
    ("cyan.500", "#00b5d8"),
    ("cyan.600", "#00a3c4"),
    ("cyan.700", "#0987a0"),
    ("cyan.800", "#086f83"),
    ("cyan.900", "#065666"),
    // Purple
    ("purple.50", "#faf5ff"),
    ("purple.100", "#e9d8fd"),
    ("purple.200", "#d6bcfa"),
    ("purple.300", "#b794f4"),
    ("purple.400", "#9f7aea"),
    ("purple.500", "#805ad5"),
    ("purple.600", "#6b46c1"),
    ("purple.700", "#553c9a"),
    ("purple.800", "#44337a"),
    ("purple.900", "#322659"),
    // Pink
    ("pink.50", "#fff5f7"),
    ("pink.100", "#fed7e2"),
    ("pink.200", "#fbb6ce"),
    ("pink.300", "#f687b3"),
    ("pink.400", "#ed64a6"),
    ("pink.500", "#d53f8c"),
    ("pink.600", "#b83280"),
    ("pink.700", "#97266d"),
    ("pink.800", "#702459"),
    ("pink.900", "#521b41"),
];
