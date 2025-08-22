/*!
Verifies that the STM vendor crate embeds board definitions supplied via the
`RLVGL_CHIP_SRC` environment variable and exposes them through its API.
*/
use rlvgl_chips_stm::{boards, find, raw_db};

#[test]
fn raw_db_includes_board_data() {
    let blob = raw_db();
    let text = core::str::from_utf8(blob).expect("utf8");
    assert!(text.contains(">boards.json"), "missing boards.json marker");
    assert!(text.contains("STM32F4DISCOVERY"), "board content missing");
}

#[test]
fn boards_and_find_work() {
    assert_eq!(boards().len(), 1);
    let b = find("STM32F4DISCOVERY").expect("board exists");
    assert_eq!(b.chip, "STM32F407");
}
