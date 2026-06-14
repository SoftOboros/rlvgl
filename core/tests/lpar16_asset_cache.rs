// SPDX-License-Identifier: MIT
//! LPAR-16 conformance fixtures for LPAR-09 asset/filesystem sources.
//!
//! Discharges the no_std-feasible portion of the LPAR-09 row of the LPAR-16 §6
//! ledger: embedded source lookup (hit + miss), the `SlotCache<N>` LRU eviction
//! sequence, and an `ImageData::Asset` round-trip through `resolve_image` with a
//! `MemoryAssetSource`. These are §5.B **kind 1 — determinism fixtures** (fixed
//! call order → fixed outcome) with concrete expected values.
//!
//! Invariant: LPAR-09 §5.D — the LRU eviction order is fully deterministic
//! given a fixed `get`/`insert` sequence on a fresh `SlotCache<N>` (monotonic
//! `u32` counter).
//!
//! The FATFS-over-`SimBlockDevice` source fixture stays Open: it needs a
//! `FatfsAssetSource` (not yet implemented) and the std-only `rlvgl-fs-sim`
//! crate, neither reachable from a `core` integration test (LPAR-16 §6, §14).
//!
//! Requires the `fs` feature (the `asset`/`fs` modules and `ImageData::Asset`
//! are gated). Compiles empty without it.
#![cfg(feature = "fs")]

use rlvgl_core::asset::{
    AssetPath, AssetRegistry, EmbeddedAssetSource, MemoryAssetSource, SlotCache,
};
use rlvgl_core::fs::{AssetError, AssetSource, FsError};
use rlvgl_core::image::{ImageData, ImageDescriptor, PixelFormat};

// ── embedded source: hit + miss ─────────────────────────────────────────────

static DOT_BYTES: &[u8] = &[0xFF, 0x00, 0x00, 0xFF];
static EMBED_TABLE: &[(&str, &[u8])] = &[("icons/dot.raw", DOT_BYTES)];

#[test]
fn embedded_source_lookup_hit_and_miss() {
    let src = EmbeddedAssetSource::new(EMBED_TABLE);

    assert!(src.exists("icons/dot.raw"), "hit: known path exists");
    let mut reader = src.open("icons/dot.raw").unwrap();
    assert_eq!(reader.len(), 4);
    let mut buf = [0u8; 4];
    assert_eq!(reader.read(&mut buf).unwrap(), 4);
    assert_eq!(buf, [0xFF, 0x00, 0x00, 0xFF], "exact embedded bytes");

    assert!(!src.exists("missing.raw"), "miss: unknown path absent");
    assert!(
        matches!(
            src.open("missing.raw"),
            Err(AssetError::Fs(FsError::NoSuchFile))
        ),
        "miss returns NoSuchFile"
    );
}

// ── SlotCache LRU eviction: deterministic order ─────────────────────────────

/// A 1-byte descriptor tagged with `marker` so cache contents are identifiable.
fn marker_desc(marker: u8) -> ImageDescriptor<'static> {
    ImageDescriptor::new(
        PixelFormat::L8,
        1,
        1,
        ImageData::Owned(alloc_vec(marker)),
        None,
    )
}

fn alloc_vec(b: u8) -> Vec<u8> {
    vec![b]
}

#[test]
fn slot_cache_lru_eviction_is_deterministic() {
    // Fresh SlotCache<2>, fixed call order (LPAR-09 §5.D).
    let mut cache: SlotCache<2> = SlotCache::new();
    let h1 = cache.insert(marker_desc(1)); // slot 0
    let h2 = cache.insert(marker_desc(2)); // slot 1
    let _ = cache.get(h1); // touch h1 → h2 becomes the LRU victim
    let h3 = cache.insert(marker_desc(3)); // evicts h2

    assert!(cache.get(h2).is_none(), "h2 evicted: LRU after h1 touch");
    assert_eq!(
        cache.get(h1).unwrap().data.as_bytes().unwrap(),
        &[1u8],
        "h1 resident with its original bytes"
    );
    assert!(cache.get(h3).is_some(), "h3 occupies the evicted slot");
}

// ── ImageData::Asset round-trip via resolve_image + MemoryAssetSource ───────

#[test]
fn asset_round_trip_through_resolve_image() {
    let mut src = MemoryAssetSource::new();
    src.insert("test/sprite.raw", vec![0xDE, 0xAD, 0xBE, 0xEF]);

    let mut reg: AssetRegistry<4> = AssetRegistry::new();
    reg.register_source(&AssetPath::Memory(String::new()), Box::new(src))
        .unwrap();
    let handle = reg.register(AssetPath::Memory("test/sprite.raw".into()));

    // First resolve: cache miss → decode from source.
    let bytes1 = reg
        .resolve_image(handle)
        .unwrap()
        .data
        .as_bytes()
        .unwrap()
        .to_vec();
    assert_eq!(bytes1, [0xDE, 0xAD, 0xBE, 0xEF]);

    // Second resolve: cache hit → identical bytes (deterministic).
    let bytes2 = reg
        .resolve_image(handle)
        .unwrap()
        .data
        .as_bytes()
        .unwrap()
        .to_vec();
    assert_eq!(bytes2, bytes1, "cache hit returns identical bytes");

    // The pre-resolve placeholder carries no bytes.
    let placeholder = ImageData::Asset(handle);
    assert_eq!(placeholder.byte_len(), 0);
    assert!(placeholder.as_bytes().is_none());
}
