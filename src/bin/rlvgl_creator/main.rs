//! rlgvl-creator binary entry point.
//!
//! Launches the desktop UI when run without arguments. Executes the
//! command-line interface when arguments are provided.

use anyhow::Result;

#[path = "../creator/cli.rs"]
mod cli;
#[path = "../creator_ui/mod.rs"]
mod ui;

pub use cli::*;

fn main() -> Result<()> {
    if std::env::args().len() > 1 {
        cli::run()
    } else {
        ui::run()
    }
}
