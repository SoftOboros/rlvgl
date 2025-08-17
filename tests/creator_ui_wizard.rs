//! Ensure the Scan→Convert→Preview wizard reports progress and dispatches steps.
#![cfg(feature = "creator_ui")]

#[path = "../src/bin/creator_ui/wizard.rs"]
mod wizard;

use anyhow::Result;
use std::cell::RefCell;
use wizard::{WizardStep, run_scan_convert_preview_wizard};

#[test]
fn wizard_runs_steps_in_order() -> Result<()> {
    let mut progress = Vec::new();
    let executed = RefCell::new(Vec::new());
    run_scan_convert_preview_wizard(
        || {
            executed.borrow_mut().push("scan");
            Ok(())
        },
        || {
            executed.borrow_mut().push("convert");
            Ok(())
        },
        || {
            executed.borrow_mut().push("preview");
            Ok(())
        },
        |step| progress.push(step),
    )?;
    assert_eq!(executed.into_inner(), vec!["scan", "convert", "preview"]);
    assert_eq!(
        progress,
        vec![
            WizardStep::SelectRoot,
            WizardStep::Scan,
            WizardStep::Convert,
            WizardStep::Preview,
            WizardStep::Summary,
        ]
    );
    Ok(())
}
