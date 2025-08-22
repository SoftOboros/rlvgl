//! Verify that vendor crates embed board definition blobs.

#[test]
fn stm_board_blob_contains_sample() {
    let blob = rlvgl_chips_stm::raw_db();
    let text = core::str::from_utf8(blob).unwrap();
    assert!(text.contains("STM32F4DISCOVERY"));
}
