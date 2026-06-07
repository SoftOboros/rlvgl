//! Smoke tests for `rlvgl-creator app new <NAME>` —
//! starter `app.yaml` scaffolder. Verifies the generated
//! manifest validates cleanly, the scaffolded directory is
//! refused on overwrite, and ref-id format rules from chapter
//! 01 §3 are enforced.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/app.rs"]
mod app;

use std::path::PathBuf;
use std::process::Command;

fn creator_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rlvgl-creator"))
}

#[test]
fn scaffolded_manifest_validates_and_inspects_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = app::new_scaffold(tmp.path(), "my-app").expect("scaffold succeeds");

    // The scaffold itself runs the validator at the end — but
    // also explicit re-validate to make sure we get the same
    // Manifest shape externally.
    let m = app::validate(&manifest).expect("scaffolded manifest validates");
    assert_eq!(m.schema, "rlvgl-app/v0");
    assert_eq!(m.name, "my-app");
    assert_eq!(m.target.prong, "bare_metal");
    assert_eq!(m.target.generator.as_deref(), Some("hosted"));
    assert_eq!(m.screens.len(), 1);
    assert_eq!(m.screens[0].id, "main-screen");
    assert!(m.screens[0].default);

    // The starter layout file exists.
    assert!(tmp.path().join("my-app/layouts/main_screen.rs").is_file());
}

#[test]
fn scaffold_refuses_to_overwrite_existing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let _first = app::new_scaffold(tmp.path(), "my-app").expect("first scaffold succeeds");
    let err = app::new_scaffold(tmp.path(), "my-app")
        .expect_err("second scaffold must refuse overwrite")
        .to_string();
    assert!(err.contains("refusing to scaffold"), "got: {err}");
    assert!(err.contains("my-app"), "got: {err}");
}

#[test]
fn scaffold_rejects_invalid_ref_id_names() {
    let tmp = tempfile::tempdir().unwrap();
    // Each of these violates `^[a-z][a-z0-9-]*$` per chapter 01 §3:
    // - upper-case start
    // - digit start
    // - whitespace inside
    // - hyphen start (also fails first-char-is-lowercase)
    for bad in ["My-App", "1invalid", "with spaces", "--double"] {
        let err = app::new_scaffold(tmp.path(), bad)
            .expect_err(&format!("expected ref-id rejection for '{bad}'"))
            .to_string();
        assert!(
            err.contains("not a valid kebab-case ref-id") || err.contains("§3"),
            "{bad}: got: {err}"
        );
    }
}

#[test]
fn scaffold_subcommand_writes_to_disk_via_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let status = Command::new(creator_bin())
        .arg("--silent")
        .arg("app")
        .arg("new")
        .arg("hello-rlvgl")
        .arg("--dir")
        .arg(tmp.path())
        .status()
        .expect("spawn rlvgl-creator");
    assert!(status.success(), "rlvgl-creator exited non-zero");
    assert!(tmp.path().join("hello-rlvgl/app.yaml").is_file());
    assert!(
        tmp.path()
            .join("hello-rlvgl/layouts/main_screen.rs")
            .is_file()
    );
}
