//! Regression controls for optional BSP clock ownership in the PAC template.
//! Unassigned selectors do not manufacture clock ownership; assigned selectors
//! retain the existing owning and secondary-core rendering behavior.
#![cfg(feature = "creator")]

#[path = "../src/bin/creator/bsp/ir.rs"]
mod ir;

use indexmap::IndexMap;
use minijinja::{Environment, context};

fn spec(mcu: &str) -> ir::Ir {
    ir::Ir {
        mcu: mcu.to_owned(),
        package: "LQFP64".to_owned(),
        clocks: ir::Clocks {
            sys_src: Some("RCC_SYSCLKSOURCE_PLLCLK".to_owned()),
            pll: IndexMap::from([(
                "pll1".to_owned(),
                ir::Pll {
                    m: 5,
                    n: 400,
                    p: 2,
                    q: 4,
                    r: 2,
                },
            )]),
            ..Default::default()
        },
        pinctrl: vec![],
        peripherals: IndexMap::new(),
        pwr: ir::Power::default(),
    }
}

fn render(spec: &ir::Ir, owner: Option<&str>, core: Option<&str>) -> String {
    let mut env = Environment::new();
    env.add_template(
        "pac",
        include_str!("../src/bin/creator/bsp/templates/pac.rs.jinja"),
    )
    .unwrap();
    env.get_template("pac")
        .unwrap()
        .render(context! {
            spec, init_by => owner, this_core => core,
            grouped_writes => false, with_deinit => false
        })
        .unwrap()
}

#[test]
fn unassigned_selectors_do_not_emit_clock_ownership_or_initialization() {
    let mut board = spec("STM32F030R8Tx");
    board.clocks.pll.clear();
    for (owner, core) in [(None, None), (None, Some("cm7")), (Some("cm7"), None)] {
        let output = render(&board, owner, core);
        assert!(!output.contains("CLOCKS_INIT_BY"));
        assert!(!output.contains("pub fn init_clocks"));
        assert!(!output.contains("pub fn signal_clocks_ready"));
        assert!(!output.contains("pub fn wait_for_clocks"));
        assert!(!output.contains("pllckselr"));
    }
}

#[test]
fn assigned_h7_owner_and_secondary_core_retain_existing_helpers() {
    let board = spec("STM32H747XIHx");
    for (owner, core) in [("cm7", "cm7"), ("cm7", "cm4"), ("cm4", "cm4")] {
        let output = render(&board, Some(owner), Some(core));
        assert!(output.contains(&format!("pub const CLOCKS_INIT_BY: &str = \"{owner}\";")));
        assert_eq!(output.contains("pub fn init_clocks"), owner == core);
        assert_eq!(output.contains("pub fn signal_clocks_ready"), owner == core);
        assert_eq!(output.contains("pub fn wait_for_clocks"), owner != core);
    }
}

#[test]
fn omitted_optional_context_still_renders_without_an_owner() {
    let mut board = spec("STM32F030R8Tx");
    board.clocks.pll.clear();
    let mut env = Environment::new();
    env.add_template(
        "pac",
        include_str!("../src/bin/creator/bsp/templates/pac.rs.jinja"),
    )
    .unwrap();
    let output = env
        .get_template("pac")
        .unwrap()
        .render(context! {
            spec => &board, grouped_writes => false, with_deinit => false
        })
        .unwrap();
    assert!(!output.contains("CLOCKS_INIT_BY"));
    assert!(!output.contains("pub fn init_clocks"));
}
