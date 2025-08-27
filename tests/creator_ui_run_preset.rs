//! Ensure presets dispatch commands in sequence.
#![cfg(feature = "creator_ui")]

#[path = "../src/bin/creator_ui/presets.rs"]
mod presets;

use presets::{CommandPreset, run_preset_commands};

#[test]
fn run_preset_dispatches_in_order() {
    let preset = CommandPreset {
        commands: vec![
            "Scan".to_string(),
            "Convert".to_string(),
            "Preview".to_string(),
        ],
    };
    let mut executed = Vec::new();
    run_preset_commands(&preset.commands, |c| executed.push(c.to_string()));
    assert_eq!(executed, vec!["Scan", "Convert", "Preview"]);
}
