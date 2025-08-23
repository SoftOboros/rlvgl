//! Test loading canonical board and MCU IR from vendor archives.

#[path = "../src/bin/creator/boards.rs"]
mod boards;

#[test]
fn loads_board_and_mcu_ir() {
    let (board, mcu) = boards::load_ir("stm", "STM32F4DISCOVERY").expect("load IR");
    assert_eq!(board["chip"], "STM32F4");
    assert_eq!(mcu["pins"]["PA0"][0]["af"], 7);
}
