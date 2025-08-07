// SPDX-License-Identifier: MIT
//! Experimental `view!` macro for declarative widget trees.
//!
//! Enabled via the optional `view` feature flag.

#[macro_export]
macro_rules! view {
    ($($tt:tt)*) => {
        $($tt)*
    };
}
