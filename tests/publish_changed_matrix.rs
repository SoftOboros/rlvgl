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
        "rlvgl-decomp",
        "rlvgl-i18n",
        "rlvgl-playit",
        "rlvgl-platform",
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
    assert!(find("rlvgl-core") < find("rlvgl-platform"));
    assert!(find("rlvgl-decomp") < find("rlvgl-platform"));
    assert!(find("rlvgl-playit") < find("rlvgl-platform"));
    assert!(find("rlvgl-platform") < find("rlvgl-widgets"));
    assert!(find("rlvgl-widgets") < find("rlvgl-ui"));
    assert!(find("rlvgl-api") < find("rlvgl-micropython"));
    assert!(find("rlvgl-core") < find("rlvgl-app-demo"));
    assert!(find("rlvgl-i18n") < find("rlvgl-app-demo"));
    assert!(find("rlvgl-ui") < find("rlvgl-app-disco-demo"));
    assert!(find("rlvgl-platform") < find("rlvgl-app-disco-demo"));
    assert!(find("rlvgl-app-demo") < find("rlvgl"));
    assert!(find("rlvgl-app-disco-demo") < find("rlvgl"));
}
