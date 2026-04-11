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
fn script_lists_rlvgl_decomp_before_platform() {
    let script = fs::read_to_string("scripts/publish_changed.sh").expect("read script");
    let decomp = script
        .find("append_unique \"rlvgl-decomp\"")
        .expect("missing rlvgl-decomp entry in publish script");
    let platform = script
        .find("append_unique \"rlvgl-platform\"")
        .expect("missing rlvgl-platform entry in publish script");
    assert!(
        decomp < platform,
        "rlvgl-decomp must be published before rlvgl-platform"
    );
}
