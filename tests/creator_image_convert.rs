//! End-to-end tests for `rlvgl-creator compress` / `lvgl` image conversion
//! (`src/bin/creator/compress.rs` + `emit.rs`).
//!
//! Subprocess-based: invokes the built `rlvgl-creator` binary against a real
//! repo asset, then validates the output bytes — the LVGL `.bin` is decoded
//! back through `rlvgl_decomp::lvgl`, and the C/Rust array shapes are checked
//! for the expected symbols. Mirrors `creator_app_bsp_gen.rs`'s subprocess
//! style so the CLI wiring (not just the library codec) is exercised.
#![cfg(feature = "creator")]

use std::path::PathBuf;
use std::process::Command;

use rlvgl_decomp::lvgl::{self, LvglCf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn creator_bin() -> &'static str {
    env!("CARGO_BIN_EXE_rlvgl-creator")
}

/// A small, known repo asset: a 60×60 RGBA PNG.
fn sample_png() -> PathBuf {
    workspace_root().join("assets/icons/60/folder-open.png")
}

fn run(args: &[&str]) {
    let status = Command::new(creator_bin())
        .arg("--silent")
        .args(args)
        .status()
        .expect("spawn rlvgl-creator");
    assert!(status.success(), "rlvgl-creator {args:?} failed");
}

#[test]
fn lvgl_bin_round_trips_through_decoder() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("icon.bin");
    run(&[
        "lvgl",
        sample_png().to_str().unwrap(),
        out.to_str().unwrap(),
        "--cf",
        "argb8888",
    ]);

    let data = std::fs::read(&out).unwrap();
    let (w, h, cf, rgba) = lvgl::decode_bin(&data).expect("decode LVGL bin");
    assert_eq!((w, h), (60, 60));
    assert_eq!(cf, LvglCf::Argb8888);
    assert_eq!(rgba.len(), 60 * 60 * 4);
}

#[test]
fn lvgl_rle_bin_is_smaller_and_decodes_identically() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain.bin");
    let rle = tmp.path().join("rle.bin");
    let png = sample_png();
    run(&[
        "lvgl",
        png.to_str().unwrap(),
        plain.to_str().unwrap(),
        "--cf",
        "argb8888",
    ]);
    run(&[
        "lvgl",
        png.to_str().unwrap(),
        rle.to_str().unwrap(),
        "--cf",
        "argb8888",
        "--rle",
    ]);

    let plain_bytes = std::fs::read(&plain).unwrap();
    let rle_bytes = std::fs::read(&rle).unwrap();
    assert!(
        rle_bytes.len() < plain_bytes.len(),
        "RLE ({}) should be smaller than uncompressed ({})",
        rle_bytes.len(),
        plain_bytes.len(),
    );

    // Compressed flag set on the RLE output, clear on the plain one.
    let plain_flags = u16::from_le_bytes([plain_bytes[2], plain_bytes[3]]);
    let rle_flags = u16::from_le_bytes([rle_bytes[2], rle_bytes[3]]);
    assert_eq!(plain_flags & lvgl::LV_IMAGE_FLAGS_COMPRESSED, 0);
    assert_ne!(rle_flags & lvgl::LV_IMAGE_FLAGS_COMPRESSED, 0);

    // Both decode to the same pixels.
    let (_, _, _, a) = lvgl::decode_bin(&plain_bytes).unwrap();
    let (_, _, _, b) = lvgl::decode_bin(&rle_bytes).unwrap();
    assert_eq!(a, b);
}

#[test]
fn compress_emit_rust_array_starts_with_rlec_magic() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("icon_rle.rs");
    run(&[
        "compress",
        sample_png().to_str().unwrap(),
        out.to_str().unwrap(),
        "--emit",
        "rust",
        "--name",
        "folder_open",
    ]);

    let src = std::fs::read_to_string(&out).unwrap();
    assert!(src.contains("pub static FOLDER_OPEN: [u8;"));
    assert!(src.contains("pub const FOLDER_OPEN_LEN: usize ="));
    // "RLEC" magic = 0x52 0x4c 0x45 0x43.
    assert!(
        src.contains("0x52, 0x4c, 0x45, 0x43,"),
        "Rust array should embed the RLEC blob header bytes"
    );
}

#[test]
fn lvgl_emit_c_writes_image_descriptor() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("icon.c");
    run(&[
        "lvgl",
        sample_png().to_str().unwrap(),
        out.to_str().unwrap(),
        "--emit",
        "c",
        "--name",
        "folder_open",
    ]);

    let src = std::fs::read_to_string(&out).unwrap();
    assert!(src.contains("const lv_image_dsc_t folder_open = {"));
    assert!(src.contains("static const LV_ATTRIBUTE_MEM_ALIGN uint8_t folder_open_map[] = {"));
    assert!(src.contains(".header.cf = LV_COLOR_FORMAT_RGB565,"));
    assert!(src.contains(".header.w = 60,"));
    assert!(src.contains(".header.stride = 120,")); // 60px * 2 bytes (RGB565)
}

/// Write a 4×2 RGBA PNG: left half opaque white, right half transparent.
fn transparent_png(path: &std::path::Path) {
    let mut img = image::RgbaImage::new(4, 2);
    for y in 0..2 {
        for x in 0..4 {
            let a = if x < 2 { 255 } else { 0 };
            img.put_pixel(x, y, image::Rgba([255, 255, 255, a]));
        }
    }
    img.save(path).unwrap();
}

#[test]
fn lvgl_a8_coverage_round_trips_from_alpha() {
    let tmp = tempfile::tempdir().unwrap();
    let png = tmp.path().join("mask.png");
    transparent_png(&png);
    let out = tmp.path().join("mask.bin");
    run(&[
        "lvgl",
        png.to_str().unwrap(),
        out.to_str().unwrap(),
        "--cf",
        "a8",
    ]);

    let data = std::fs::read(&out).unwrap();
    assert_eq!(data[1], 0x0E); // A8 cf code
    let (w, h, cf, cover) = lvgl::decode_alpha_bin(&data).expect("decode A8");
    assert_eq!((w, h, cf), (4, 2, lvgl::LvglAlphaCf::A8));
    // Auto picks alpha (image is transparent): left opaque, right clear.
    assert_eq!(cover, vec![255, 255, 0, 0, 255, 255, 0, 0]);
}

#[test]
fn lvgl_a8_tints_with_fill_over_background() {
    let tmp = tempfile::tempdir().unwrap();
    let png = tmp.path().join("mask.png");
    transparent_png(&png);
    let out = tmp.path().join("mask.bin");
    run(&[
        "lvgl",
        png.to_str().unwrap(),
        out.to_str().unwrap(),
        "--cf",
        "a8",
    ]);

    // Composite a green fill over a black 4×2 ARGB8888 background.
    let data = std::fs::read(&out).unwrap();
    let mut dst = vec![0u8; 4 * 2 * 4]; // [B,G,R,A] black, A=0
    let (w, h) = lvgl::blend_alpha_bin_into_argb(&data, (0, 255, 0), &mut dst).unwrap();
    assert_eq!((w, h), (4, 2));
    // px0 covered -> green [B,G,R,A]=[0,255,0,255]; px2 clear -> stays black.
    assert_eq!(&dst[0..4], &[0, 255, 0, 0xFF]);
    assert_eq!(&dst[8..12], &[0, 0, 0, 0xFF]);
}
