//! Verify MiniJinja rendering from vendor IR.

#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn renders_board_context() {
    let tmpl = "{{ meta.vendor }} {{ meta.board }} {{ meta.chip }} AF={{ mcu.pins.PA0[0].af }}";
    let out = boards::render_template("stm", "STM32F4DISCOVERY", tmpl).expect("render");
    assert!(out.contains("stm"));
    assert!(out.contains("STM32F4DISCOVERY"));
    assert!(out.contains("STM32F407"));
    assert!(out.contains("AF=7"));
}
