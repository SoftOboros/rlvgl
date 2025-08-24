//! Verify MiniJinja rendering from vendor IR.

#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn renders_board_context() {
    let tmpl = "{{ board.chip }} AF={{ mcu.pins.PA0[0].af }}";
    let out = boards::render_template("stm", "STM32F4DISCOVERY", tmpl).expect("render");
    assert!(out.contains("STM32F4"));
    assert!(out.contains("AF=7"));
}
