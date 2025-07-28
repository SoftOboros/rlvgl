use std::process::Command;

#[test]
fn leak_detection() {
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping leak_detection: valgrind not installed");
        return;
    }
    let status = Command::new("valgrind")
        .args([
            "--quiet",
            "--leak-check=full",
            "--error-exitcode=1",
            "cargo",
            "run",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--example",
            "leak",
            "--quiet",
        ])
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_INCREMENTAL")
        .env_remove("LLVM_PROFILE_FILE")
        .status()
        .expect("failed to run valgrind");
    assert!(status.success());
}
