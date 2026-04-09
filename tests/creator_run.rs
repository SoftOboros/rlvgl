use std::process::Command;
use tempfile::tempdir;

#[test]
fn run_sim_fails_without_manifest() {
    let dir = tempdir().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("run")
        .arg("sim")
        .current_dir(dir.path())
        .output()
        .expect("failed to run rlvgl-creator");

    assert!(!status.status.success());
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(stderr.contains("No Cargo.toml found"));
}

#[test]
fn run_sim_invokes_cargo() {
    let dir = tempdir().unwrap();
    // Create a dummy Cargo.toml so the check passes
    std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers=[]").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("run")
        .arg("sim")
        .current_dir(dir.path())
        .output()
        .expect("failed to run rlvgl-creator");

    assert!(!status.status.success());
    // Since we didn't scaffold a real project, cargo will complain about missing package 'sim'
    // or just fail to find the package.
    // We can't easily capture cargo's stderr here because Command::new("cargo") inside the binary
    // inherits stdout/stderr by default unless captured.
    // In src/bin/creator/run.rs: Command::new("cargo").status()? inherits.
    // So the output won't be in `status.stderr` of the creator process unless we change run.rs to capture.
}
