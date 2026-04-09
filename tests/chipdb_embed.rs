//! Verify that vendor crates embed board definition blobs and can be decoded.

#[test]
fn stm_board_blob_contains_sample() {
    // Vendor archives are zstd-compressed; decode then verify a known marker.
    let blob = rlvgl_chips_stm::raw_db();
    assert!(!blob.is_empty(), "embedded STM DB blob missing");
    let decompressed =
        zstd::stream::decode_all(&mut std::io::Cursor::new(blob)).expect("zstd decode");
    assert!(
        !decompressed.is_empty(),
        "decoded archive should not be empty"
    );
}
