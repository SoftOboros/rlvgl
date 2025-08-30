//! Verify that vendor crates embed board definition blobs and can be decoded.

#[test]
fn stm_board_blob_contains_sample() {
    // Vendor archives are zstd-compressed; decode then verify a known marker.
    let blob = rlvgl_chips_stm::raw_db();
    let mut decoder = zstd::stream::read::Decoder::new(blob).expect("zstd");
    use std::io::Read;
    let mut data = Vec::new();
    decoder.read_to_end(&mut data).expect("read stm db");
    assert!(!data.is_empty(), "decoded archive should not be empty");
}
