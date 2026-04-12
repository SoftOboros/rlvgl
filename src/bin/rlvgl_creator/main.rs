//! rlgvl-creator binary entry point.
//!
//! Launches the desktop UI when run without arguments. Executes the
//! command-line interface when arguments are provided.

use anyhow::Result;

#[path = "../creator/cli.rs"]
mod cli;

#[cfg(feature = "creator_ui")]
#[path = "../creator_ui/mod.rs"]
mod ui;

#[path = "../creator/bsp/af.rs"]
pub mod af;
#[path = "../creator/ast.rs"]
pub mod ast;
#[path = "../creator/bsp/espressif/mod.rs"]
pub mod espressif;
#[path = "../creator/bsp/ioc.rs"]
pub mod ioc;
#[path = "../creator/bsp/ir.rs"]
pub mod ir;
#[path = "../creator/bsp/nordic/mod.rs"]
pub mod nordic;
#[path = "../creator/bsp/nxp/mod.rs"]
pub mod nxp;
#[path = "../creator/bsp/rp/mod.rs"]
pub mod rp;

/// Re-exported board support modules for CLI utilities.
mod bsp {
    pub use super::af;
    pub use super::espressif;
    pub use super::ioc;
    pub use super::ir;
    pub use super::nordic;
    pub use super::nxp;
    pub use super::rp;
}

pub use cli::*;

fn main() -> Result<()> {
    if std::env::args().len() > 1 {
        cli::run()
    } else {
        #[cfg(feature = "creator_ui")]
        {
            ui::run()
        }
        #[cfg(not(feature = "creator_ui"))]
        {
            cli::run()
        }
    }
}
