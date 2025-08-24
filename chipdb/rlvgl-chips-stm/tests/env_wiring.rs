/*!
Verifies that the STM vendor crate embeds board definitions supplied via the
`RLVGL_CHIP_SRC` environment variable and exposes them through its API.
*/
use rlvgl_chips_stm::{boards, find, raw_db};
use std::io::Read;
use zstd::stream::read::Decoder;

#[test]
fn raw_db_includes_board_data() {
    let blob = raw_db();
    let mut decoder = Decoder::new(blob).expect("decoder");
    let mut text = String::new();
    decoder.read_to_string(&mut text).expect("read");
    assert!(text.contains(">boards.json"), "missing boards.json marker");
    assert!(text.contains("STM32F4DISCOVERY"), "board content missing");
}

#[test]
fn boards_and_find_work() {
    assert_eq!(boards().len(), 1);
    let b = find("STM32F4DISCOVERY").expect("board exists");
    assert_eq!(b.chip, "STM32F407");
}
