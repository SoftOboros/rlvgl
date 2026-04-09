//! Svelte alignment command scaffolding for rlvgl-creator.
//!
//! Provides CLI entry points for token generation, component compilation,
//! validation, and renderer glue emission. Token generation is implemented;
//! other commands are stubbed until the Svelte integration matures.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail, ensure};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Generate token outputs for web and rlvgl targets.
pub(crate) fn tokens(input: &Path, out: &Path, mode: Option<&str>) -> Result<()> {
    let raw = fs::read_to_string(input)?;
    let token_file: TokensFile = serde_yaml::from_str(&raw)?;
    let resolved = resolve_tokens(token_file, mode)?;

    fs::create_dir_all(out)?;

    let json_path = out.join("tokens.json");
    fs::write(&json_path, serde_json::to_string_pretty(&resolved)?)?;

    let css_path = out.join("tokens.css");
    fs::write(&css_path, emit_css(&resolved))?;

    let tailwind_path = out.join("tailwind.tokens.cjs");
    fs::write(&tailwind_path, emit_tailwind(&resolved))?;

    let theme_path = out.join("theme.rs");
    fs::write(&theme_path, emit_theme_rs(&resolved)?)?;

    println!(
        "Generated tokens: {}, {}, {}, {}",
        json_path.display(),
        css_path.display(),
        tailwind_path.display(),
        theme_path.display()
    );
    Ok(())
}

/// Compile Svelte sources into rlvgl output.
pub(crate) fn compile(input: &Path, out: &Path, tokens: Option<&Path>) -> Result<()> {
    let _ = (input, out, tokens);
    bail!("svelte compile is not implemented yet")
}

/// Emit renderer glue for Svelte → WASM → rlvgl.
pub(crate) fn wasm(out: &Path, name: Option<&str>) -> Result<()> {
    let _ = (out, name);
    bail!("svelte wasm is not implemented yet")
}

/// Emit JSON schema for tokens and UI IR.
pub(crate) fn schema(out: Option<&Path>) -> Result<()> {
    let schema = schemars::schema_for!(TokensFile);
    let json = serde_json::to_string_pretty(&schema)?;
    if let Some(path) = out {
        fs::write(path, json)?;
    } else {
        println!("{}", json);
    }
    Ok(())
}

/// Validate tokens and Svelte subset constraints.
pub(crate) fn check(input: &Path, tokens: Option<&Path>, mode: Option<&str>) -> Result<()> {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut validated_tokens = false;
    let mut validated_sources = false;

    if let Some(path) = resolve_tokens_path(input, tokens) {
        let raw = fs::read_to_string(&path)?;
        let token_file: TokensFile = serde_yaml::from_str(&raw)?;
        validate_tokens_file(&token_file, mode, &mut warnings, &mut errors);
        validated_tokens = true;
    }

    if let Some(paths) = resolve_svelte_sources(input)? {
        validate_svelte_sources(&paths, &mut warnings, &mut errors)?;
        validated_sources = true;
    }

    if !validated_tokens && !validated_sources {
        bail!("no token or svelte sources provided");
    }

    if !validated_tokens {
        warnings.push("token validation skipped (no token file provided)".to_string());
    }

    if !validated_sources {
        warnings.push("svelte source validation skipped".to_string());
    }

    if !errors.is_empty() {
        let mut msg = String::from("svelte check failed:");
        for err in errors {
            msg.push_str("\n- ");
            msg.push_str(&err);
        }
        bail!(msg);
    }

    for warn in warnings {
        eprintln!("warning: {}", warn);
    }

    println!("svelte check: ok");
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct TokensFile {
    #[serde(default)]
    version: Option<u8>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    spacing: BTreeMap<String, u32>,
    #[serde(default)]
    radii: BTreeMap<String, u32>,
    #[serde(default)]
    typography: BTreeMap<String, TypographyStyle>,
    #[serde(default)]
    motion: MotionTokens,
    #[serde(default)]
    fonts: BTreeMap<String, String>,
    #[serde(default)]
    modes: BTreeMap<String, ModeTokens>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct ModeTokens {
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    spacing: BTreeMap<String, u32>,
    #[serde(default)]
    radii: BTreeMap<String, u32>,
    #[serde(default)]
    typography: BTreeMap<String, TypographyStyle>,
    #[serde(default)]
    motion: MotionTokens,
    #[serde(default)]
    fonts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct TypographyStyle {
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    weight: Option<u32>,
    #[serde(default)]
    line_height: Option<u32>,
    #[serde(default)]
    letter_spacing: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct MotionTokens {
    #[serde(default)]
    durations: BTreeMap<String, u32>,
    #[serde(default)]
    easings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct ResolvedTokens {
    mode: Option<String>,
    colors: BTreeMap<String, String>,
    spacing: BTreeMap<String, u32>,
    radii: BTreeMap<String, u32>,
    typography: BTreeMap<String, TypographyStyle>,
    motion: MotionTokens,
    fonts: BTreeMap<String, String>,
}

fn resolve_tokens(mut tokens: TokensFile, mode: Option<&str>) -> Result<ResolvedTokens> {
    let (chosen_mode, warning) = select_mode_key(&tokens.modes, mode)?;
    if let Some(msg) = warning {
        eprintln!("warning: {}", msg);
    }
    if let Some(mode_key) = &chosen_mode {
        if let Some(mode_tokens) = tokens.modes.remove(mode_key) {
            merge_mode(&mut tokens, mode_tokens);
        }
    }

    Ok(ResolvedTokens {
        mode: chosen_mode,
        colors: tokens.colors,
        spacing: tokens.spacing,
        radii: tokens.radii,
        typography: tokens.typography,
        motion: tokens.motion,
        fonts: tokens.fonts,
    })
}

fn merge_mode(base: &mut TokensFile, mode: ModeTokens) {
    base.colors.extend(mode.colors);
    base.spacing.extend(mode.spacing);
    base.radii.extend(mode.radii);
    base.typography.extend(mode.typography);
    base.motion.durations.extend(mode.motion.durations);
    base.motion.easings.extend(mode.motion.easings);
    base.fonts.extend(mode.fonts);
}

fn select_mode_key(
    modes: &BTreeMap<String, ModeTokens>,
    mode: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    if modes.is_empty() {
        return Ok((None, None));
    }
    if let Some(m) = mode {
        ensure!(modes.contains_key(m), "unknown token mode '{}'", m);
        return Ok((Some(m.to_string()), None));
    }
    if modes.contains_key("default") {
        return Ok((Some("default".to_string()), None));
    }
    if modes.len() == 1 {
        return Ok((Some(modes.keys().next().unwrap().to_string()), None));
    }
    let first = modes.keys().next().unwrap().to_string();
    Ok((
        Some(first.clone()),
        Some(format!(
            "multiple modes found; using '{}' (pass --mode to select)",
            first
        )),
    ))
}

fn resolve_tokens_path(input: &Path, tokens: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = tokens {
        return Some(path.to_path_buf());
    }
    if is_token_file(input) {
        return Some(input.to_path_buf());
    }
    None
}

fn resolve_svelte_sources(input: &Path) -> Result<Option<Vec<PathBuf>>> {
    if is_svelte_file(input) {
        return Ok(Some(vec![input.to_path_buf()]));
    }
    if input.is_dir() {
        let files = collect_svelte_files(input)?;
        if files.is_empty() {
            return Ok(None);
        }
        return Ok(Some(files));
    }
    Ok(None)
}

fn collect_svelte_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();
            if is_svelte_file(path) {
                files.push(path.to_path_buf());
            }
        }
    }
    Ok(files)
}

fn is_svelte_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case("svelte"),
        None => false,
    }
}

fn is_token_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            ext == "yml" || ext == "yaml"
        }
        None => false,
    }
}

fn validate_tokens_file(
    tokens: &TokensFile,
    mode: Option<&str>,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(version) = tokens.version {
        if version != 1 {
            warnings.push(format!(
                "token version {} is not recognized; expected 1",
                version
            ));
        }
    }

    validate_colors_map(&tokens.colors, "colors", errors);
    validate_u8_map(&tokens.spacing, "spacing", errors);
    validate_u8_map(&tokens.radii, "radii", errors);
    validate_typography_map(&tokens.typography, "typography", errors);
    validate_motion_tokens(&tokens.motion, "motion", errors);
    validate_fonts_map(&tokens.fonts, "fonts", errors);

    for (mode_name, mode_tokens) in &tokens.modes {
        if !is_valid_key(mode_name) {
            errors.push(format!(
                "mode name '{}' must be lowercase letters, digits, '-' or '_'",
                mode_name
            ));
        }
        let prefix = format!("modes.{}", mode_name);
        validate_colors_map(&mode_tokens.colors, &format!("{prefix}.colors"), errors);
        validate_u8_map(&mode_tokens.spacing, &format!("{prefix}.spacing"), errors);
        validate_u8_map(&mode_tokens.radii, &format!("{prefix}.radii"), errors);
        validate_typography_map(
            &mode_tokens.typography,
            &format!("{prefix}.typography"),
            errors,
        );
        validate_motion_tokens(&mode_tokens.motion, &format!("{prefix}.motion"), errors);
        validate_fonts_map(&mode_tokens.fonts, &format!("{prefix}.fonts"), errors);
    }

    let (selected_mode, warn) = match select_mode_key(&tokens.modes, mode) {
        Ok(value) => value,
        Err(err) => {
            errors.push(err.to_string());
            return;
        }
    };
    if let Some(msg) = warn {
        warnings.push(msg);
    }

    let resolved = resolve_for_mode(tokens, selected_mode.as_deref());
    validate_required_tokens(&resolved, warnings);
}

fn resolve_for_mode(tokens: &TokensFile, mode: Option<&str>) -> ResolvedTokens {
    let mut merged = tokens.clone();
    let chosen = mode.map(|m| m.to_string());
    if let Some(mode_key) = mode {
        if let Some(mode_tokens) = merged.modes.remove(mode_key) {
            merge_mode(&mut merged, mode_tokens);
        }
    }
    ResolvedTokens {
        mode: chosen,
        colors: merged.colors,
        spacing: merged.spacing,
        radii: merged.radii,
        typography: merged.typography,
        motion: merged.motion,
        fonts: merged.fonts,
    }
}

fn validate_required_tokens(tokens: &ResolvedTokens, warnings: &mut Vec<String>) {
    warn_missing(&tokens.colors, "primary", "colors", warnings);
    warn_missing(&tokens.colors, "background", "colors", warnings);
    warn_missing(&tokens.colors, "text", "colors", warnings);

    warn_missing(&tokens.spacing, "xs", "spacing", warnings);
    warn_missing(&tokens.spacing, "sm", "spacing", warnings);
    warn_missing(&tokens.spacing, "md", "spacing", warnings);
    warn_missing(&tokens.spacing, "lg", "spacing", warnings);
    warn_missing(&tokens.spacing, "xl", "spacing", warnings);

    warn_missing(&tokens.radii, "none", "radii", warnings);
    warn_missing(&tokens.radii, "sm", "radii", warnings);
    warn_missing(&tokens.radii, "md", "radii", warnings);
    warn_missing(&tokens.radii, "lg", "radii", warnings);
    warn_missing(&tokens.radii, "full", "radii", warnings);

    warn_missing(&tokens.fonts, "small", "fonts", warnings);
    warn_missing(&tokens.fonts, "body", "fonts", warnings);
    warn_missing(&tokens.fonts, "heading", "fonts", warnings);
}

fn warn_missing<T>(map: &BTreeMap<String, T>, key: &str, group: &str, warnings: &mut Vec<String>) {
    if !map.contains_key(key) {
        warnings.push(format!("missing {} token '{}'", group, key));
    }
}

fn validate_colors_map(map: &BTreeMap<String, String>, group: &str, errors: &mut Vec<String>) {
    validate_token_keys(map, group, errors);
    for (name, value) in map {
        if let Err(err) = parse_color(Some(value), (0, 0, 0, 255)) {
            errors.push(format!("{} color '{}' invalid: {}", group, name, err));
        }
    }
}

fn validate_u8_map(map: &BTreeMap<String, u32>, group: &str, errors: &mut Vec<String>) {
    validate_token_keys(map, group, errors);
    for (name, value) in map {
        if *value > u8::MAX as u32 {
            errors.push(format!(
                "{} token '{}' exceeds 255 (got {})",
                group, name, value
            ));
        }
    }
}

fn validate_typography_map(
    map: &BTreeMap<String, TypographyStyle>,
    group: &str,
    errors: &mut Vec<String>,
) {
    validate_token_keys(map, group, errors);
    for (name, style) in map {
        if let Some(family) = &style.family {
            if family.trim().is_empty() {
                errors.push(format!("{} '{}' has empty font family", group, name));
            }
        }
        if let Some(size) = style.size {
            if size == 0 {
                errors.push(format!("{} '{}' has size 0", group, name));
            }
        }
        if let Some(weight) = style.weight {
            if weight == 0 {
                errors.push(format!("{} '{}' has weight 0", group, name));
            }
        }
        if let Some(height) = style.line_height {
            if height == 0 {
                errors.push(format!("{} '{}' has line_height 0", group, name));
            }
        }
        if let Some(spacing) = style.letter_spacing {
            if !spacing.is_finite() {
                errors.push(format!("{} '{}' has invalid letter_spacing", group, name));
            }
        }
    }
}

fn validate_motion_tokens(tokens: &MotionTokens, group: &str, errors: &mut Vec<String>) {
    validate_token_keys(&tokens.durations, &format!("{group}.durations"), errors);
    validate_token_keys(&tokens.easings, &format!("{group}.easings"), errors);
    for (name, value) in &tokens.easings {
        if value.trim().is_empty() {
            errors.push(format!("{} easing '{}' is empty", group, name));
        }
    }
}

fn validate_fonts_map(map: &BTreeMap<String, String>, group: &str, errors: &mut Vec<String>) {
    validate_token_keys(map, group, errors);
    for (name, value) in map {
        if value.trim().is_empty() {
            errors.push(format!("{} '{}' is empty", group, name));
        }
    }
}

fn validate_token_keys<T>(map: &BTreeMap<String, T>, group: &str, errors: &mut Vec<String>) {
    for key in map.keys() {
        if !is_valid_key(key) {
            errors.push(format!(
                "{} token key '{}' must be lowercase letters, digits, '-' or '_'",
                group, key
            ));
        }
    }
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn validate_svelte_sources(
    files: &[PathBuf],
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> Result<()> {
    for path in files {
        let content = fs::read_to_string(path)?;
        let stripped = strip_script_style(&content);
        validate_svelte_content(path, &stripped, warnings, errors)?;
    }
    Ok(())
}

fn strip_script_style(input: &str) -> String {
    let mut output = input.to_string();
    for tag in ["script", "style"] {
        let re = Regex::new(&format!(r"(?s)<{tag}\b[^>]*>.*?</{tag}>")).unwrap();
        output = re.replace_all(&output, "").to_string();
    }
    output
}

fn validate_svelte_content(
    path: &Path,
    content: &str,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> Result<()> {
    if content.contains("{@html") {
        errors.push(format!("{}: {{@html}} is not allowed", path.display()));
    }
    if content.contains("{#await") {
        errors.push(format!("{}: {{#await}} is not allowed", path.display()));
    }
    if content.contains("use:") {
        errors.push(format!(
            "{}: use: directives are not allowed",
            path.display()
        ));
    }
    if content.contains("transition:") || content.contains("in:") || content.contains("out:") {
        errors.push(format!(
            "{}: transition/in/out directives are not allowed",
            path.display()
        ));
    }
    if content.contains("animate:") {
        errors.push(format!(
            "{}: animate: directives are not allowed",
            path.display()
        ));
    }
    if Regex::new(r"\bslot\s*=")?.is_match(content) {
        errors.push(format!("{}: named slots are not allowed", path.display()));
    }

    check_each_blocks(path, content, warnings)?;
    check_tags(path, content, errors)?;
    Ok(())
}

fn check_each_blocks(path: &Path, content: &str, warnings: &mut Vec<String>) -> Result<()> {
    let re = Regex::new(r"\{#each[^}]*\}")?;
    for cap in re.find_iter(content) {
        let text = cap.as_str();
        if !text.contains('(') || !text.contains(')') {
            warnings.push(format!(
                "{}: {{#each}} should include a key (e.g., {{#each items as item (item.id)}})",
                path.display()
            ));
        }
    }
    Ok(())
}

fn check_tags(path: &Path, content: &str, errors: &mut Vec<String>) -> Result<()> {
    let re = Regex::new(r"<\s*/?\s*([A-Za-z][A-Za-z0-9:_-]*)")?;
    for cap in re.captures_iter(content) {
        let tag = cap.get(1).unwrap().as_str();
        if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
            continue;
        }
        if tag.eq_ignore_ascii_case("slot") {
            continue;
        }
        if tag.contains(':') {
            errors.push(format!(
                "{}: namespaced tag '{}' is not allowed",
                path.display(),
                tag
            ));
            continue;
        }
        if tag
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        {
            errors.push(format!(
                "{}: raw HTML tag '{}' is not allowed",
                path.display(),
                tag
            ));
            continue;
        }
        if !is_allowed_component(tag) {
            errors.push(format!(
                "{}: component '{}' is not allowed",
                path.display(),
                tag
            ));
        }
    }
    Ok(())
}

fn is_allowed_component(tag: &str) -> bool {
    matches!(
        tag,
        "Button" | "Text" | "Image" | "Stack" | "Row" | "Column"
    )
}

fn emit_css(tokens: &ResolvedTokens) -> String {
    let mut out = String::from("/* rlvgl-creator generated tokens */\n:root {\n");
    for (name, value) in &tokens.colors {
        out.push_str(&format!("  --rlvgl-color-{}: {};\n", name, value));
    }
    for (name, value) in &tokens.spacing {
        out.push_str(&format!("  --rlvgl-spacing-{}: {}px;\n", name, value));
    }
    for (name, value) in &tokens.radii {
        out.push_str(&format!("  --rlvgl-radius-{}: {}px;\n", name, value));
    }
    for (name, style) in &tokens.typography {
        if let Some(family) = &style.family {
            out.push_str(&format!(
                "  --rlvgl-font-{}-family: \"{}\";\n",
                name,
                escape_css_string(family)
            ));
        }
        if let Some(size) = style.size {
            out.push_str(&format!("  --rlvgl-font-{}-size: {}px;\n", name, size));
        }
        if let Some(weight) = style.weight {
            out.push_str(&format!("  --rlvgl-font-{}-weight: {};\n", name, weight));
        }
        if let Some(height) = style.line_height {
            out.push_str(&format!(
                "  --rlvgl-font-{}-line-height: {}px;\n",
                name, height
            ));
        }
        if let Some(spacing) = style.letter_spacing {
            out.push_str(&format!(
                "  --rlvgl-font-{}-letter-spacing: {}em;\n",
                name, spacing
            ));
        }
    }
    for (name, value) in &tokens.motion.durations {
        out.push_str(&format!(
            "  --rlvgl-motion-duration-{}: {}ms;\n",
            name, value
        ));
    }
    for (name, value) in &tokens.motion.easings {
        out.push_str(&format!("  --rlvgl-motion-easing-{}: {};\n", name, value));
    }
    out.push_str("}\n");
    out
}

fn emit_tailwind(tokens: &ResolvedTokens) -> String {
    let mut out = String::from("module.exports = {\n  theme: {\n    extend: {\n");
    if !tokens.colors.is_empty() {
        out.push_str("      colors: {\n");
        for (name, value) in &tokens.colors {
            out.push_str(&format!("        \"{}\": \"{}\",\n", name, value));
        }
        out.push_str("      },\n");
    }
    if !tokens.spacing.is_empty() {
        out.push_str("      spacing: {\n");
        for (name, value) in &tokens.spacing {
            out.push_str(&format!("        \"{}\": \"{}px\",\n", name, value));
        }
        out.push_str("      },\n");
    }
    if !tokens.radii.is_empty() {
        out.push_str("      borderRadius: {\n");
        for (name, value) in &tokens.radii {
            out.push_str(&format!("        \"{}\": \"{}px\",\n", name, value));
        }
        out.push_str("      },\n");
    }
    if !tokens.typography.is_empty() {
        out.push_str("      fontFamily: {\n");
        for (name, style) in &tokens.typography {
            if let Some(family) = &style.family {
                out.push_str(&format!(
                    "        \"{}\": [\"{}\"],\n",
                    name,
                    escape_js_string(family)
                ));
            }
        }
        out.push_str("      },\n");
        out.push_str("      fontSize: {\n");
        for (name, style) in &tokens.typography {
            if let Some(size) = style.size {
                if style.line_height.is_some()
                    || style.letter_spacing.is_some()
                    || style.weight.is_some()
                {
                    out.push_str(&format!("        \"{}\": [\"{}px\", {{", name, size));
                    if let Some(height) = style.line_height {
                        out.push_str(&format!(" lineHeight: \"{}px\",", height));
                    }
                    if let Some(spacing) = style.letter_spacing {
                        out.push_str(&format!(" letterSpacing: \"{}em\",", spacing));
                    }
                    if let Some(weight) = style.weight {
                        out.push_str(&format!(" fontWeight: \"{}\",", weight));
                    }
                    out.push_str(" }],\n");
                } else {
                    out.push_str(&format!("        \"{}\": \"{}px\",\n", name, size));
                }
            }
        }
        out.push_str("      },\n");
    }
    if !tokens.motion.durations.is_empty() {
        out.push_str("      transitionDuration: {\n");
        for (name, value) in &tokens.motion.durations {
            out.push_str(&format!("        \"{}\": \"{}ms\",\n", name, value));
        }
        out.push_str("      },\n");
    }
    if !tokens.motion.easings.is_empty() {
        out.push_str("      transitionTimingFunction: {\n");
        for (name, value) in &tokens.motion.easings {
            out.push_str(&format!("        \"{}\": \"{}\",\n", name, value));
        }
        out.push_str("      },\n");
    }
    out.push_str("    },\n  },\n};\n");
    out
}

fn emit_theme_rs(tokens: &ResolvedTokens) -> Result<String> {
    let spacing = resolve_spacing(tokens)?;
    let radii = resolve_radii(tokens)?;
    let colors = resolve_colors(tokens)?;
    let fonts = resolve_fonts(tokens);
    Ok(format!(
        "// SPDX-License-Identifier: MIT\n\
//! rlvgl-creator generated theme tokens from shared tokens.\n\
//!\n\
//! This file is meant to be included by user crates.\n\
\n\
use rlvgl_ui::style::Color;\n\
use rlvgl_ui::theme::{{Colors, Fonts, Radii, Spacing, Theme, Tokens}};\n\
\n\
pub const THEME: Theme = Theme {{\n\
    tokens: Tokens {{\n\
        spacing: Spacing {{ xs: {sx}, sm: {sm}, md: {md}, lg: {lg}, xl: {xl} }},\n\
        radii: Radii {{ none: {rn}, sm: {rs}, md: {rm}, lg: {rl}, full: {rf} }},\n\
        colors: Colors {{\n\
            primary: Color({cr}, {cg}, {cb}, {ca}),\n\
            background: Color({br}, {bg}, {bb}, {ba}),\n\
            text: Color({tr}, {tg}, {tb}, {ta}),\n\
        }},\n\
        fonts: Fonts {{ small: \"{fs}\", body: \"{fb}\", heading: \"{fh}\" }},\n\
    }},\n\
}};\n",
        sx = spacing.xs,
        sm = spacing.sm,
        md = spacing.md,
        lg = spacing.lg,
        xl = spacing.xl,
        rn = radii.none,
        rs = radii.sm,
        rm = radii.md,
        rl = radii.lg,
        rf = radii.full,
        cr = colors.primary.0,
        cg = colors.primary.1,
        cb = colors.primary.2,
        ca = colors.primary.3,
        br = colors.background.0,
        bg = colors.background.1,
        bb = colors.background.2,
        ba = colors.background.3,
        tr = colors.text.0,
        tg = colors.text.1,
        tb = colors.text.2,
        ta = colors.text.3,
        fs = escape_rust_string(&fonts.small),
        fb = escape_rust_string(&fonts.body),
        fh = escape_rust_string(&fonts.heading),
    ))
}

#[derive(Clone, Copy)]
struct ThemeSpacing {
    xs: u8,
    sm: u8,
    md: u8,
    lg: u8,
    xl: u8,
}

#[derive(Clone, Copy)]
struct ThemeRadii {
    none: u8,
    sm: u8,
    md: u8,
    lg: u8,
    full: u8,
}

#[derive(Clone, Copy)]
struct ThemeColors {
    primary: (u8, u8, u8, u8),
    background: (u8, u8, u8, u8),
    text: (u8, u8, u8, u8),
}

#[derive(Clone)]
struct ThemeFonts {
    small: String,
    body: String,
    heading: String,
}

fn resolve_spacing(tokens: &ResolvedTokens) -> Result<ThemeSpacing> {
    let xs = pick_u8(&tokens.spacing, "xs", 2)?;
    let sm = pick_u8(&tokens.spacing, "sm", 4)?;
    let md = pick_u8(&tokens.spacing, "md", 8)?;
    let lg = pick_u8(&tokens.spacing, "lg", 16)?;
    let xl = pick_u8(&tokens.spacing, "xl", 24)?;
    Ok(ThemeSpacing { xs, sm, md, lg, xl })
}

fn resolve_radii(tokens: &ResolvedTokens) -> Result<ThemeRadii> {
    let none = pick_u8(&tokens.radii, "none", 0)?;
    let sm = pick_u8(&tokens.radii, "sm", 2)?;
    let md = pick_u8(&tokens.radii, "md", 4)?;
    let lg = pick_u8(&tokens.radii, "lg", 8)?;
    let full = pick_u8(&tokens.radii, "full", 255)?;
    Ok(ThemeRadii {
        none,
        sm,
        md,
        lg,
        full,
    })
}

fn resolve_colors(tokens: &ResolvedTokens) -> Result<ThemeColors> {
    let primary = parse_color(tokens.colors.get("primary"), (98, 0, 238, 255))?;
    let background = parse_color(tokens.colors.get("background"), (255, 255, 255, 255))?;
    let text = parse_color(tokens.colors.get("text"), (0, 0, 0, 255))?;
    Ok(ThemeColors {
        primary,
        background,
        text,
    })
}

fn resolve_fonts(tokens: &ResolvedTokens) -> ThemeFonts {
    let small = tokens
        .fonts
        .get("small")
        .cloned()
        .unwrap_or_else(|| "tiny".to_string());
    let body = tokens
        .fonts
        .get("body")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let heading = tokens
        .fonts
        .get("heading")
        .cloned()
        .unwrap_or_else(|| "bold".to_string());
    ThemeFonts {
        small,
        body,
        heading,
    }
}

fn pick_u8(map: &BTreeMap<String, u32>, key: &str, fallback: u8) -> Result<u8> {
    match map.get(key) {
        Some(v) if *v <= u8::MAX as u32 => Ok(*v as u8),
        Some(v) => bail!("token '{}' exceeds u8 range: {}", key, v),
        None => Ok(fallback),
    }
}

fn parse_color(input: Option<&String>, fallback: (u8, u8, u8, u8)) -> Result<(u8, u8, u8, u8)> {
    let Some(raw) = input else {
        return Ok(fallback);
    };
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if let Some(parsed) = parse_rgb_function(value)? {
        return Ok(parsed);
    }
    bail!("unsupported color format '{}'", value)
}

fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8, u8)> {
    let bytes = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            let a = u8::from_str_radix(&hex[6..8], 16)?;
            (r, g, b, a)
        }
        _ => bail!("invalid hex color '{}'", hex),
    };
    Ok(bytes)
}

fn parse_rgb_function(input: &str) -> Result<Option<(u8, u8, u8, u8)>> {
    let raw = input.trim();
    let (is_rgba, body) = if let Some(rest) = raw.strip_prefix("rgba(") {
        (true, rest)
    } else if let Some(rest) = raw.strip_prefix("rgb(") {
        (false, rest)
    } else {
        return Ok(None);
    };
    let body = body
        .strip_suffix(')')
        .ok_or_else(|| anyhow::anyhow!("invalid color"))?;
    let parts: Vec<&str> = body.split(',').map(|p| p.trim()).collect();
    ensure!(
        parts.len() == if is_rgba { 4 } else { 3 },
        "invalid color '{}' ",
        input
    );
    let r = parts[0].parse::<u8>()?;
    let g = parts[1].parse::<u8>()?;
    let b = parts[2].parse::<u8>()?;
    let a = if is_rgba {
        let raw_a = parts[3].parse::<f32>()?;
        if raw_a <= 1.0 {
            (raw_a * 255.0).round() as u8
        } else {
            raw_a as u8
        }
    } else {
        255
    };
    Ok(Some((r, g, b, a)))
}

fn escape_css_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_js_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_rust_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
