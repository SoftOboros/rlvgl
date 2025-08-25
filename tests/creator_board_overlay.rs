#![cfg(feature = "creator")]
//! Verify board overlay loading and metadata access within templates.

#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn load_ir_and_render_template_expose_board_meta() {
    let (board, mcu) = boards::load_ir("test", "demo").expect("load");
    assert_eq!(board["board"], "demo");
    assert_eq!(board["chip"], "STM32F4");
    assert!(mcu["pins"]["PA0"].is_array());

    let out = boards::render_template(
        "test",
        "demo",
        "{{ meta.board }} {{ meta.chip }} {{ board.chip }} {{ mcu.pins.PA0[0].instance }}",
    )
    .expect("render");
    assert!(out.contains("demo STM32F4 STM32F4 USART2"));
}
