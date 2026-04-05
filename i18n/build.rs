use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::Path;

/// Binary translation format (RLTN v1):
///
/// ```text
/// [0..4]   magic   b"RLTN"
/// [4]      version 1
/// [5]      num_locales
/// [6..8]   num_keys   (u16 LE)
/// [8..]    entries    (num_locales * num_keys) × 6 bytes each:
///            offset: u32 LE  (into string data)
///            len:    u16 LE
/// [..]     string data  (UTF-8, packed)
/// ```
///
/// Lookup: `entries[locale * num_keys + key]` → (offset, len) → `&str`
const BLOB_MAGIC: &[u8; 4] = b"RLTN";
const BLOB_VERSION: u8 = 1;
const ENTRY_SIZE: usize = 6; // u32 offset + u16 len

fn to_pascal(key: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for ch in key.chars() {
        if ch == '.' || ch == '_' || ch == '-' {
            upper_next = true;
        } else if upper_next {
            out.push(ch.to_ascii_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn main() {
    let locales_dir = Path::new("locales");
    println!("cargo:rerun-if-changed=locales/");

    // Collect locale files sorted alphabetically.
    let mut locale_files: Vec<_> = fs::read_dir(locales_dir)
        .expect("cannot read locales/ directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .collect();
    locale_files.sort_by_key(|e| e.file_name());

    assert!(!locale_files.is_empty(), "no locale JSON files found in locales/");

    // Parse each locale into an ordered map.
    let mut locales: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for entry in &locale_files {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let map: BTreeMap<String, Value> = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", path.display()));
        let strings: BTreeMap<String, String> = map
            .into_iter()
            .map(|(k, v)| {
                let s = match v {
                    Value::String(s) => s,
                    _ => panic!("value for key \"{k}\" in {} must be a string", path.display()),
                };
                (k, s)
            })
            .collect();
        locales.push((name, strings));
    }

    // Use the first locale (alphabetically) as the default / fallback.
    let default_map = locales[0].1.clone();
    let default_keys: Vec<String> = default_map.keys().cloned().collect();

    // Validate and fill missing keys with fallback.
    for (name, map) in &mut locales[1..] {
        for key in &default_keys {
            if !map.contains_key(key) {
                eprintln!(
                    "cargo:warning=i18n: key \"{key}\" missing from {name}.json, using fallback"
                );
                map.insert(key.clone(), default_map[key].clone());
            }
        }
    }

    let keys: Vec<&str> = default_keys.iter().map(|s| s.as_str()).collect();
    let num_locales = locales.len();
    let num_keys = keys.len();

    // ── Build binary blob ──────────────────────────────────────────────
    let header_size = 8;
    let index_size = num_locales * num_keys * ENTRY_SIZE;
    let data_offset = header_size + index_size;

    // Collect all strings in order (locale-major) and compute offsets.
    let mut string_data = Vec::<u8>::new();
    let mut entries: Vec<(u32, u16)> = Vec::with_capacity(num_locales * num_keys);

    for (_name, map) in &locales {
        for key in &keys {
            let val = map.get(*key).unwrap_or(&default_map[*key]);
            let offset = string_data.len() as u32;
            let len = val.len() as u16;
            string_data.extend_from_slice(val.as_bytes());
            entries.push((offset, len));
        }
    }

    // Assemble blob.
    let blob_len = data_offset + string_data.len();
    let mut blob = Vec::with_capacity(blob_len);

    // Header.
    blob.extend_from_slice(BLOB_MAGIC);
    blob.push(BLOB_VERSION);
    blob.push(num_locales as u8);
    blob.extend_from_slice(&(num_keys as u16).to_le_bytes());

    // Index entries.
    for (offset, len) in &entries {
        blob.extend_from_slice(&offset.to_le_bytes());
        blob.extend_from_slice(&len.to_le_bytes());
    }

    // String data.
    blob.extend_from_slice(&string_data);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Write .bin blob to OUT_DIR (embedded via include_bytes).
    fs::write(out_path.join("translations.bin"), &blob).unwrap();

    // Print the blob path so it can be copied to SD / media.
    eprintln!("cargo:warning=i18n blob: {}/translations.bin ({} bytes)",
              out_dir, blob.len());

    // ── Generate Rust source ───────────────────────────────────────────
    let mut out = String::with_capacity(4096);

    // -- Locale enum --
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum Locale {{").unwrap();
    for (i, (name, _)) in locales.iter().enumerate() {
        let variant = to_pascal(name);
        writeln!(out, "    {variant} = {i},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl Locale {{").unwrap();
    writeln!(out, "    /// Number of compiled-in locales.").unwrap();
    writeln!(out, "    pub const COUNT: usize = {num_locales};").unwrap();
    writeln!(out, "    /// Default (fallback) locale.").unwrap();
    writeln!(
        out,
        "    pub const DEFAULT: Locale = Locale::{};",
        to_pascal(&locales[0].0)
    )
    .unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // -- Key enum --
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(out, "#[repr(u16)]").unwrap();
    writeln!(out, "pub enum Key {{").unwrap();
    let mut pascal_keys = Vec::with_capacity(num_keys);
    for (i, key) in keys.iter().enumerate() {
        let variant = to_pascal(key);
        if pascal_keys.contains(&variant) {
            panic!("i18n key collision: \"{key}\" maps to variant {variant} which already exists");
        }
        pascal_keys.push(variant.clone());
        writeln!(out, "    {variant} = {i},").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl Key {{").unwrap();
    writeln!(out, "    /// Number of translation keys.").unwrap();
    writeln!(out, "    pub const COUNT: usize = {num_keys};").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // -- Built-in blob --
    writeln!(
        out,
        "static BUILTIN_BLOB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/translations.bin\"));"
    )
    .unwrap();
    writeln!(out).unwrap();

    // -- t! macro --
    writeln!(out, "/// Translate a key, optionally with parameters.").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// - `t!(\"key\")` returns `&'static str`").unwrap();
    writeln!(out, "/// - `t!(\"key\", param = value, ...)` returns `alloc::string::String`").unwrap();
    writeln!(out, "#[macro_export]").unwrap();
    writeln!(out, "macro_rules! t {{").unwrap();
    for (key, variant) in keys.iter().zip(pascal_keys.iter()) {
        writeln!(
            out,
            "    (\"{key}\") => {{ $crate::t_static($crate::Key::{variant}) }};"
        )
        .unwrap();
    }
    for (key, variant) in keys.iter().zip(pascal_keys.iter()) {
        writeln!(
            out,
            "    (\"{key}\", $($name:ident = $val:expr),+ $(,)?) => {{"
        )
        .unwrap();
        writeln!(
            out,
            "        $crate::t_format($crate::Key::{variant}, &[$(( stringify!($name), &$val as &dyn core::fmt::Display )),+])"
        )
        .unwrap();
        writeln!(out, "    }};").unwrap();
    }
    writeln!(
        out,
        "    ($other:literal $(, $($rest:tt)*)?) => {{ compile_error!(concat!(\"Unknown i18n key: \", $other)) }};"
    )
    .unwrap();
    writeln!(out, "}}").unwrap();

    // -- locale_from_u8 helper --
    writeln!(out).unwrap();
    writeln!(out, "#[inline]").unwrap();
    writeln!(out, "pub fn locale_from_u8(v: u8) -> Locale {{").unwrap();
    writeln!(out, "    match v {{").unwrap();
    for (i, (name, _)) in locales.iter().enumerate() {
        let variant = to_pascal(name);
        writeln!(out, "        {i} => Locale::{variant},").unwrap();
    }
    writeln!(out, "        _ => Locale::DEFAULT,").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    fs::write(out_path.join("translations.rs"), &out).unwrap();
}
