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
    ("Emulator", &["Simulator"]),
    // QT-09 §3: Qt menu group surfacing the 10 `qt …` CLI
    // subcommands (docs/qt-support/09-desktop-ui.md).
    (
        "Qt",
        &[
            "Qt Ingest",
            "Qt Check",
            "Qt Schema",
            "Qt Emit",
            "Qt Emit Scjson",
            "Qt Emit Externals",
            "Qt Emit Tokens",
            "Qt List Assets",
            "Qt List Qmldir",
            "Qt List Qrc",
        ],
    ),
];
