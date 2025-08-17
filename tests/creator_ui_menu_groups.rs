//! Integration test validating creator UI menu grouping.

#[path = "../src/bin/creator_ui/menus.rs"]
mod menus;

#[test]
fn menus_have_expected_groups() {
    let groups = menus::MENU_GROUPS;
    assert_eq!(groups.len(), 3);

    let assets = groups.iter().find(|(n, _)| *n == "Assets").unwrap();
    assert!(assets.1.contains(&"Init"));
    assert!(assets.1.contains(&"Scan"));
    assert!(assets.1.contains(&"Preview"));

    let build = groups.iter().find(|(n, _)| *n == "Build").unwrap();
    assert!(build.1.contains(&"AddTarget"));
    assert!(build.1.contains(&"Scaffold"));

    let deploy = groups.iter().find(|(n, _)| *n == "Deploy").unwrap();
    assert!(deploy.1.contains(&"Lottie Import"));
    assert!(deploy.1.contains(&"Run Preset"));
}
