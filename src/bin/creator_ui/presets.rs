//! Automation presets for chaining creator UI commands.

#[cfg(feature = "creator_ui")]
use serde::{Deserialize, Serialize};

/// Serializable sequence of command labels.
#[cfg_attr(feature = "creator_ui", derive(Serialize, Deserialize))]
pub(crate) struct CommandPreset {
    /// Ordered list of command labels to execute.
    pub(crate) commands: Vec<String>,
}

/// Run each command label using the provided dispatcher.
pub(crate) fn run_preset_commands<F>(cmds: &[String], mut dispatch: F)
where
    F: FnMut(&str),
{
    for cmd in cmds {
        dispatch(cmd);
    }
}
