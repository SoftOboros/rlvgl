// SPDX-License-Identifier: MIT
//! Experimental `view!` macro for declarative widget trees.
//!
//! Bridges [`rlvgl-core`](rlvgl_core) widgets with constructors from
//! [`rlvgl-widgets`](rlvgl_widgets). Enabled via the optional `view`
//! feature flag.

#[macro_export]
macro_rules! view {
    ($($tt:tt)*) => {
        $($tt)*
    };
}
