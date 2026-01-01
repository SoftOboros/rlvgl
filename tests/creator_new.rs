use std::process::Command;
use tempfile::tempdir;

#[test]
fn new_project_scaffolds_workspace() {
    let dir = tempdir().unwrap();
    let project_name = "my_test_project";
    let status = Command::new(env!("CARGO_BIN_EXE_rlvgl-creator"))
        .arg("new")
        .arg(project_name)
        .current_dir(dir.path())
        .status()
        .expect("failed to run rlvgl-creator");

    assert!(status.success());

    let project_path = dir.path().join(project_name);
    assert!(project_path.exists());
    assert!(project_path.join("Cargo.toml").exists());
    assert!(project_path.join("creator.yml").exists());
    assert!(project_path.join("crates/app-core/src/lib.rs").exists());
    assert!(project_path.join("crates/bsp/src/lib.rs").exists());
    assert!(project_path.join("apps/sim/src/main.rs").exists());
    assert!(project_path.join("apps/firmware/src/main.rs").exists());

    // Optional: Verify content
    let cargo_toml = std::fs::read_to_string(project_path.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains("[workspace]"));
}
