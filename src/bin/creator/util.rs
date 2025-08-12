//! Utility helpers for rlvgl-creator.

use std::path::Path;

/// Generate a SCREAMING_SNAKE constant name from a path.
pub(crate) fn const_name(path: &str) -> String {
    let name = path
        .replace('/', "_")
        .replace('.', "_")
        .replace('-', "_")
        .to_uppercase();
    if let Some(stripped) = name.strip_prefix("ICONS_") {
        format!("ICON_{}", stripped)
    } else if let Some(stripped) = name.strip_prefix("FONTS_") {
        format!("FONT_{}", stripped)
    } else if let Some(stripped) = name.strip_prefix("MEDIA_") {
        format!("MEDIA_{}", stripped)
    } else {
        name
    }
}

/// Return true if the path begins with an allowed root directory.
pub(crate) fn valid_root(path: &str) -> bool {
    match Path::new(path).iter().next().and_then(|p| p.to_str()) {
        Some("icons") | Some("fonts") | Some("media") => true,
        _ => false,
    }
}
