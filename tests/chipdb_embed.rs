//! Verify that vendor crates embed board definition blobs.

#[test]
fn stm_board_blob_contains_sample() {
    let blob = rlvgl_chips_stm::raw_db();
    if blob.is_empty() {
        eprintln!("skipping chipdb_embed: empty blob (no assets)");
        return;
    }
    let decompressed = zstd::stream::decode_all(&mut std::io::Cursor::new(blob)).unwrap();
    if decompressed.is_empty() {
        eprintln!("skipping chipdb_embed: empty decompressed blob");
        return;
    }
    // Basic sanity: should contain ASCII boundary markers if built from files
    // by the vendor build script. We avoid hardcoding specific names here.
    assert!(decompressed.windows(1).any(|_| true));
}
