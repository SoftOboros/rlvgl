//! Top-level menu group definitions for rlvgl-creator UI.
//!
//! Provides grouping of commands into user-facing menus.

/// Menu group names with their associated command labels.
pub const MENU_GROUPS: &[(&str, &[&str])] = &[
    (
        "Assets",
        &[
            "Init",
            "Scan",
            "Check",
            "Vendor",
            "Convert",
            "Preview",
            "Add Asset",
            "Scan Convert Preview",
        ],
    ),
    (
        "Build",
        &[
            "AddTarget",
            "Sync",
            "Scaffold",
            "Fonts Pack",
            "Svg",
            "Apng",
            "Schema",
        ],
    ),
    (
        "Deploy",
        &["Lottie Import", "Lottie CLI", "Run Preset", "Save Preset"],
    ),
];
