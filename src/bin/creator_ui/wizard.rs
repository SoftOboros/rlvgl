//! Guided workflows for common creator tasks.
//!
//! Provides sequential wizards such as Scan→Convert→Preview with progress
//! callbacks for each step.

use anyhow::Result;

/// Progress milestones for the Scan→Convert→Preview wizard.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WizardStep {
    /// Choose the asset root directory.
    SelectRoot,
    /// Scanning raw assets.
    Scan,
    /// Converting asset formats.
    Convert,
    /// Previewing converted results.
    Preview,
    /// Wizard complete summary.
    Summary,
}

/// Run the Scan→Convert→Preview wizard using the provided operations.
///
/// `scan`, `convert`, and `preview` are closures wrapping the corresponding
/// commands. `progress` is invoked at each milestone to report status.
pub(crate) fn run_scan_convert_preview_wizard<S, C, P, R>(
    mut scan: S,
    mut convert: C,
    mut preview: P,
    mut progress: R,
) -> Result<()>
where
    S: FnMut() -> Result<()>,
    C: FnMut() -> Result<()>,
    P: FnMut() -> Result<()>,
    R: FnMut(WizardStep),
{
    progress(WizardStep::SelectRoot);
    progress(WizardStep::Scan);
    scan()?;
    progress(WizardStep::Convert);
    convert()?;
    progress(WizardStep::Preview);
    preview()?;
    progress(WizardStep::Summary);
    Ok(())
}
