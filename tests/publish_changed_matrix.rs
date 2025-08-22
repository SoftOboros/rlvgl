/*!
Ensures the release publish script enumerates all chip database crates so they
are published when their sources change.
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
