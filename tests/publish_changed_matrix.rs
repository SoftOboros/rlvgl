/*!
Ensures the release publish script enumerates publishable workspace crates so
they are published when their sources change.
*/
use std::fs;

#[test]
fn script_lists_chipdb_crates() {
    let script = fs::read_to_string("scripts/publish_changed.sh").expect("read script");
    for name in [
        "rlvgl-chips-stm",
        "rlvgl-chips-esp",
        "rlvgl-chips-nrf",
        "rlvgl-chips-nxp",
        "rlvgl-chips-silabs",
        "rlvgl-chips-microchip",
        "rlvgl-chips-renesas",
        "rlvgl-chips-ti",
        "rlvgl-chips-rp2040",
    ] {
        assert!(
            script.contains(name),
            "missing {name} entry in publish script"
        );
    }
}

#[test]
fn script_lists_publishable_workspace_crates() {
    let script = fs::read_to_string("scripts/publish_changed.sh").expect("read script");
    for name in [
        "disco-assets",
        "rlvgl-api",
        "rlvgl-core",
        "ratatui-rlvgl",
        "rlvgl-decomp",
        "rlvgl-i18n",
        "rlvgl-playit",
        "rlvgl-platform",
        "rlvgl-audio-meters-core",
        "rlvgl-widgets",
        "rlvgl-ui",
        "rlvgl-fs-sim",
        "rlvgl-micropython",
        "rlvgl-app-demo",
        "rlvgl-app-disco-demo",
        "rlvgl",
    ] {
        assert!(
            script.contains(name),
            "missing {name} entry in publish script"
        );
    }
}

#[test]
fn script_orders_publish_dependencies_topologically() {
    let script = fs::read_to_string("scripts/publish_changed.sh").expect("read script");
    let start = script
        .find("ordered_crates=(")
        .expect("missing ordered_crates list");
    let ordered = &script[start..];
    let find = |name: &str| {
        ordered
            .find(&format!("\n  {name}\n"))
            .unwrap_or_else(|| panic!("missing {name} entry in publish order"))
    };

    assert!(find("rlvgl-core") < find("rlvgl-playit"));
    assert!(find("rlvgl-core") < find("ratatui-rlvgl"));
    assert!(find("rlvgl-core") < find("rlvgl-platform"));
    assert!(find("rlvgl-decomp") < find("rlvgl-platform"));
    assert!(find("rlvgl-playit") < find("rlvgl-platform"));
    assert!(find("rlvgl-platform") < find("rlvgl-widgets"));
    assert!(find("rlvgl-audio-meters-core") < find("rlvgl-widgets"));
    assert!(find("rlvgl-widgets") < find("rlvgl-ui"));
    assert!(find("rlvgl-api") < find("rlvgl-micropython"));
    assert!(find("rlvgl-core") < find("rlvgl-app-demo"));
    assert!(find("rlvgl-i18n") < find("rlvgl-app-demo"));
    assert!(find("rlvgl-ui") < find("rlvgl-app-disco-demo"));
    assert!(find("rlvgl-platform") < find("rlvgl-app-disco-demo"));
    assert!(find("rlvgl-app-demo") < find("rlvgl"));
    assert!(find("rlvgl-app-disco-demo") < find("rlvgl"));
}

#[test]
fn publish_workflows_use_trusted_publishing() {
    for path in [
        ".github/workflows/publish.yml",
        ".github/workflows/publish-continue.yml",
    ] {
        let workflow = fs::read_to_string(path).expect("read publish workflow");
        assert!(workflow.contains("environment: release"), "{path}");
        assert!(workflow.contains("id-token: write"), "{path}");
        assert!(
            workflow.contains("uses: rust-lang/crates-io-auth-action@v1"),
            "{path}"
        );
        assert!(
            workflow.contains("CARGO_REGISTRY_TOKEN: ${{ steps.crates_io_auth.outputs.token }}"),
            "{path}"
        );
        assert!(
            !workflow.contains("secrets.CARGO_REGISTRY_TOKEN"),
            "{path} must not restore the long-lived registry secret"
        );
    }

    let workflow =
        fs::read_to_string(".github/workflows/publish.yml").expect("read publish workflow");
    assert!(
        workflow.contains("bash scripts/publish_changed.sh \"${{ steps.shas.outputs.base }}\"")
    );
}

/// CRATES-CI-01: every workspace crate with `publish = true` (or
/// unrestricted publish) MUST appear in the script's `ordered_crates`
/// list. Derived from `cargo metadata` so the list cannot silently
/// omit a publishable crate — the failure mode that left
/// `rlvgl-audio-meters-core` unpublishable-by-script while
/// `rlvgl-widgets` depended on it (caught during Gate P bring-up;
/// see docs/crates-ci/CRATES-CI-00-CONCEPTS.md §2 class P-ORDER).
#[test]
fn script_order_covers_every_publishable_workspace_crate() {
    let script = fs::read_to_string("scripts/publish_changed.sh").expect("read script");
    let start = script
        .find("ordered_crates=(")
        .expect("missing ordered_crates list");
    let end = start
        + script[start..]
            .find(')')
            .expect("unterminated ordered_crates list");
    let ordered = &script[start..end];

    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata");

    let mut missing = Vec::new();
    for package in metadata["packages"].as_array().expect("packages array") {
        let name = package["name"].as_str().expect("package name");
        // `publish: null` means unrestricted (publishable); an empty
        // registry array means `publish = false`.
        let publishable = match &package["publish"] {
            serde_json::Value::Null => true,
            serde_json::Value::Array(registries) => !registries.is_empty(),
            other => panic!("unexpected publish field for {name}: {other}"),
        };
        if publishable && !ordered.contains(&format!("\n  {name}\n")) {
            missing.push(name.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "publishable workspace crates missing from ordered_crates: {missing:?}"
    );
}
