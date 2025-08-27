//! Board selection helpers for rlvgl-creator UI.
//!
//! Provides display labels combining vendor and board names for populating
//! drop-down menus.

use super::boards;

/// Generate board selection labels in the form `"vendor / board"`.
#[must_use]
pub(crate) fn board_labels() -> Vec<String> {
    boards::enumerate()
        .into_iter()
        .map(|b| format!("{} / {}", b.vendor, b.board))
        .collect()
}
