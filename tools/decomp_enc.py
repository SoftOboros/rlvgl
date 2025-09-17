#!/usr/bin/env python3
"""
decomp_enc.py — Encode images (incl. SVG) into the RLE format consumed by BitBltSplashScreen.

Usage:
  python decomp_enc.py input.{png,jpg,bmp,gif,svg} \
      --out SplashScreen --width 800 --height 480 \
      --bg "#000000" --max-colors 192 --dither floyd --emit-header

Outputs:
  - SplashScreen.c   (always)
  - SplashScreen.h   (if --emit-header)

Dependencies:
  - Pillow (PIL):    pip install pillow
  - CairoSVG (SVG):  pip install cairosvg   (only required for .svg inputs)
"""

from __future__ import annotations
import argparse
import io
import os
import sys
from typing import List, Tuple, Dict

# Pillow for raster + palette
from PIL import Image, ImageOps

# Optional: only used when input is SVG
try:
    import cairosvg  # type: ignore
except Exception:
    cairosvg = None


# ------------------------------ Image helpers ------------------------------

def load_image_any(path: str) -> Image.Image:
    """
    Load an image. If it's SVG and cairosvg is available, rasterize to PNG first.

    Args:
        path: Path to the input image.

    Returns:
        PIL.Image in RGB mode.

    Raises:
        RuntimeError: If the image cannot be loaded or SVG without cairosvg.
    """
    ext = os.path.splitext(path)[1].lower()
    if ext == ".svg":
        if cairosvg is None:
            raise RuntimeError("SVG input requires 'cairosvg' (pip install cairosvg)")
        png_bytes = cairosvg.svg2png(url=path)
        img = Image.open(io.BytesIO(png_bytes))
    else:
        img = Image.open(path)
    return img.convert("RGB")


def letterbox(img: Image.Image, target_w: int, target_h: int, bg_rgb: Tuple[int, int, int]) -> Image.Image:
    """
    Scale into target WxH without distortion; pad “wings” with bg color.

    Args:
        img: Source RGB image.
        target_w, target_h: Destination dimensions.
        bg_rgb: Background color as (R,G,B).

    Returns:
        RGB image of size (target_w, target_h).
    """
    # Compute scale while preserving aspect
    src_w, src_h = img.size
    scale = min(target_w / src_w, target_h / src_h)
    new_w = max(1, int(round(src_w * scale)))
    new_h = max(1, int(round(src_h * scale)))
    img_scaled = img.resize((new_w, new_h), Image.LANCZOS)

    # Paste centered into a solid background canvas
    canvas = Image.new("RGB", (target_w, target_h), bg_rgb)
    ox = (target_w - new_w) // 2
    oy = (target_h - new_h) // 2
    canvas.paste(img_scaled, (ox, oy))
    return canvas


# ------------------------------ Palette helpers ------------------------------

def rgb888_to_rgb565(r: int, g: int, b: int) -> int:
    """
    Convert 8-bit/channel RGB to RGB565 (0..0xFFFF).

    Args:
        r,g,b: 0..255

    Returns:
        16-bit RGB565.
    """
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)


def quantize_to_palette565(img: Image.Image, max_colors: int, dither: str) -> Tuple[List[int], List[int]]:
    """
    Quantize image to <= max_colors colors; produce:
      - palette565: list of unique RGB565 colors actually used (<= max_colors_safe)
      - pixels_idx: flat list of palette indices per pixel.

    Args:
        img: RGB image of target size.
        max_colors: user requested maximum palette size (will be clipped to safe range).
        dither: 'none' or 'floyd'

    Returns:
        (palette565, pixels_idx)

    Raises:
        RuntimeError: if resulting palette would exceed safety constraints for encoding.
    """
    # Safety: short-repeat byte = palette_size + (repeat-1) must be <= 255 and avoid 0xFD..0xFF.
    # Max short repeat is 60, so palette_size must be <= 196 to allow 196 + 59 = 255.
    max_colors_safe = min(max_colors, 196)
    if max_colors != max_colors_safe:
        print(f"[warn] --max-colors clipped to {max_colors_safe} for encoder safety.", file=sys.stderr)

    dither_flag = Image.FLOYDSTEINBERG if dither.lower() == "floyd" else Image.NONE
    # Use ADAPTIVE palette. Pillow returns a P-mode image with palette up to max_colors_safe entries.
    pal_img = img.quantize(colors=max_colors_safe, method=Image.MEDIANCUT, dither=dither_flag)

    # Obtain raw palette (RGB triples, up to 256 entries), then restrict to used indices.
    palette = pal_img.getpalette() or []
    # Build list of used indices and remap so palette contains only used entries.
    used = sorted({p for p in pal_img.getdata()})
    if len(used) > max_colors_safe:
        raise RuntimeError(f"Quantizer produced {len(used)} colors > allowed {max_colors_safe}.")

    # Build compact palette565 and a map from original index -> compact index
    palette565: List[int] = []
    idx_map: Dict[int, int] = {}
    for i, pal_idx in enumerate(used):
        r = palette[pal_idx * 3 + 0]
        g = palette[pal_idx * 3 + 1]
        b = palette[pal_idx * 3 + 2]
        c565 = rgb888_to_rgb565(r, g, b)
        idx_map[pal_idx] = i
        palette565.append(c565)

    # Remap pixel stream to compact indices
    pixels_idx = [idx_map[p] for p in pal_img.getdata()]

    # Final safety: palette size must be <= 196 (already ensured), and != 0xFD/0xFE/0xFF base collision
    # We'll handle reserved collisions per-run during encoding (by splitting).
    return palette565, pixels_idx


# ------------------------------ RLE encoder (matches your decoder) ------------------------------

# From your C decoder:
ENCODE_KEY_SINGLE_INLINE_PIXEL = 0xFF
ENCODE_KEY_DOUBLE_INLINE_PIXEL = 0xFE
ENCODE_KEY_LONG_REPEAT         = 0xFD
SHORT_REPEAT_MAX = 60   # encoder will emit 1..60 via a single byte
LONG_REPEAT_MAX  = 316  # encoder will emit 61..316 via 0xFD + countByte (count = 60+1+countByte)
RESERVED = {ENCODE_KEY_LONG_REPEAT, ENCODE_KEY_DOUBLE_INLINE_PIXEL, ENCODE_KEY_SINGLE_INLINE_PIXEL}


def emit_short_repeat(buf: bytearray, palette_size: int, repeat_count: int) -> None:
    """
    Emit a short-repeat token for 'repeat_count' additional pixels (1..60).

    Encoding is a single byte: value = palette_size + (repeat_count - 1).
    Must avoid colliding with 0xFD/0xFE/0xFF; if collision, split into safe chunks.

    Args:
        buf: output buffer.
        palette_size: number of colors in ColorData[].
        repeat_count: 1..60.

    Raises:
        AssertionError if repeat_count out of range.
    """
    assert 1 <= repeat_count <= SHORT_REPEAT_MAX
    code = palette_size + (repeat_count - 1)
    if code in RESERVED:
        # Split into two chunks that avoid the reserved value.
        # For repeat_count >=2, split as (repeat_count-1) + 1 (both map away from the bad byte).
        first = repeat_count - 1
        second = 1
        if first >= 1:
            emit_short_repeat(buf, palette_size, first)
            emit_short_repeat(buf, palette_size, second)
        else:
            # Fallback: shouldn't happen because repeat_count>=1
            buf.append(ENCODE_KEY_LONG_REPEAT)
            buf.append(0)  # 61 total, close enough for single-pixel oddity
    else:
        buf.append(code)


def emit_long_repeat(buf: bytearray, extra_count: int) -> None:
    """
    Emit one long-repeat token (0xFD, countByte) for extra_count in [61..316].

    Args:
        buf: output buffer
        extra_count: number of additional pixels to write for current color (61..316)

    Raises:
        AssertionError: invalid range.
    """
    assert SHORT_REPEAT_MAX + 1 <= extra_count <= LONG_REPEAT_MAX
    buf.append(ENCODE_KEY_LONG_REPEAT)
    buf.append(extra_count - (SHORT_REPEAT_MAX + 1))  # decoder: 60+1+next_byte


def encode_rle_indices(pixels_idx: List[int], palette_size: int) -> bytes:
    """
    Encode a stream of palette indices into ImageData bytes that your C code reads.

    Protocol (per SplashScreen.c):
      - A byte < palette_size selects the current color index AND draws one pixel.
      - Then:
        * short-repeat: a single byte X in [palette_size .. palette_size+59] draws (X - palette_size + 1) more
        * long-repeat:  0xFD <countByte> draws (60 + 1 + countByte) more
      - 0xFE/0xFF support inline literal pixels; we avoid them by keeping colors in the palette.

    Args:
        pixels_idx: flattened list of indices (row-major).
        palette_size: len(ColorData)

    Returns:
        bytes array for ImageData.
    """
    out = bytearray()
    n = len(pixels_idx)
    i = 0
    while i < n:
        color = pixels_idx[i]
        # Count run length
        run = 1
        j = i + 1
        while j < n and pixels_idx[j] == color and run < 10_000_000:
            run += 1
            j += 1

        # Emit color index byte (draws 1 pixel)
        out.append(color & 0xFF)
        remaining = run - 1  # additional pixels to emit for this color

        # Emit repeats using a mix of long and short tokens
        while remaining > 0:
            if remaining >= (SHORT_REPEAT_MAX + 1):
                chunk = min(remaining, LONG_REPEAT_MAX)
                emit_long_repeat(out, chunk)
                remaining -= chunk
            else:
                emit_short_repeat(out, palette_size, remaining)
                remaining = 0

        i = j
    return bytes(out)


# ------------------------------ C emitter ------------------------------

def format_c_arrays(name: str,
                    palette565: List[int],
                    image_bytes: bytes) -> Tuple[str, str]:
    """
    Create C source (and header) strings that mirror SplashScreen.{c,h} style.

    Args:
        name: Base symbol name (e.g., 'SplashScreen')
        palette565: list of 16-bit RGB565 colors
        image_bytes: encoded image bytes

    Returns:
        (c_source, h_header)
    """
    # Pretty-print ColorData
    color_lines = []
    line = []
    for i, c in enumerate(palette565):
        line.append(f"0x{c:04X}")
        if len(line) == 12:
            color_lines.append(", ".join(line))
            line = []
    if line:
        color_lines.append(", ".join(line))
    color_body = ",\n    ".join(color_lines) if color_lines else ""

    # Pretty-print ImageData as hex bytes, 12/line
    img_lines = []
    line = []
    for i, b in enumerate(image_bytes):
        line.append(f"0x{b:02X}")
        if len(line) == 12:
            img_lines.append(", ".join(line))
            line = []
    if line:
        img_lines.append(", ".join(line))
    img_body = ",\n    ".join(img_lines) if img_lines else ""

    c_src = f"""\
/**
 * @file {name}.c
 * @brief Generated splash image (RLE) – compatible with BitBltSplashScreen decoder.
 * @note Format derived from existing SplashScreen.c/.h (RLE keys 0xFF/0xFE/0xFD, SHORT=60, LONG=316).
 */

#include <stdint.h>
#include <stddef.h>

static const uint16_t ColorData[] = {{
    {color_body}
}};

static const uint8_t ImageData[] = {{
    {img_body}
}};

// Optional reference: your decoder logic lives in BitBltSplashScreen(), see project code.
"""
    h_hdr = f"""\
/**
 * @file {name}.h
 * @brief Declarations for generated splash image arrays.
 */
#ifndef {name.upper()}_H_
#define {name.upper()}_H_

#include <stdint.h>

extern const uint16_t ColorData[];
extern const uint8_t  ImageData[];

#endif
"""
    return c_src, h_hdr


# ------------------------------ CLI ------------------------------

def parse_color(s: str) -> Tuple[int, int, int]:
    """
    Parse '#RRGGBB' or 'R,G,B' or hex '0xRRGGBB' into (R,G,B).

    Raises:
        ValueError on invalid format.
    """
    s = s.strip()
    if s.startswith("#"):
        v = int(s[1:], 16)
        return (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF
    if s.startswith("0x"):
        v = int(s, 16)
        return (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF
    if "," in s:
        parts = [int(p) for p in s.split(",")]
        if len(parts) != 3 or any(not (0 <= x <= 255) for x in parts):
            raise ValueError("RGB must be 3 bytes (0..255).")
        return tuple(parts)  # type: ignore
    raise ValueError("Color must be '#RRGGBB', '0xRRGGBB', or 'R,G,B'.")


def main() -> None:
    ap = argparse.ArgumentParser(description="Encode images into RLE format for BitBltSplashScreen.")
    ap.add_argument("input", help="Input image (png/jpg/bmp/gif/svg)")
    ap.add_argument("--out", default="SplashScreen", help="Output base name (default: SplashScreen)")
    ap.add_argument("--width", type=int, default=800, help="Target width (default: 800)")
    ap.add_argument("--height", type=int, default=480, help="Target height (default: 480)")
    ap.add_argument("--bg", default="#000000", help="Background color for wings (default: #000000)")
    ap.add_argument("--max-colors", type=int, default=192, help="Max palette size (<=196; default: 192)")
    ap.add_argument("--dither", choices=["none", "floyd"], default="floyd", help="Palette dither (default: floyd)")
    ap.add_argument("--emit-header", action="store_true", help="Also write a .h with externs")
    args = ap.parse_args()

    try:
        bg_rgb = parse_color(args.bg)
    except ValueError as e:
        sys.exit(f"Invalid --bg: {e}")

    # 1) Load & letterbox
    img = load_image_any(args.input)
    img2 = letterbox(img, args.width, args.height, bg_rgb)

    # 2) Quantize to RGB565 palette
    palette565, pixels_idx = quantize_to_palette565(img2, args.max_colors, args.dither)
    palette_size = len(palette565)
    if palette_size == 0:
        sys.exit("Quantization produced empty palette (unexpected).")

    # 3) Encode to ImageData bytes (no inline literals; palette indices + repeats only)
    image_bytes = encode_rle_indices(pixels_idx, palette_size)

    # 4) Emit .c (and .h)
    c_src, h_hdr = format_c_arrays(args.out, palette565, image_bytes)
    with open(f"{args.out}.c", "w", encoding="utf-8") as f:
        f.write(c_src)
    if args.emit_header:
        with open(f"{args.out}.h", "w", encoding="utf-8") as f:
            f.write(h_hdr)

    # Quick stats
    total_px = len(pixels_idx)
    raw_bytes = total_px * 2  # RGB565 raw
    rle_bytes = len(image_bytes)
    ratio = raw_bytes / max(1, rle_bytes)
    print(f"[ok] W={args.width} H={args.height} palette={palette_size}  ImageData={rle_bytes}B  "
          f"raw={raw_bytes}B  comp≈{ratio:.2f}x  -> {args.out}.c" + (" + .h" if args.emit_header else ""))


if __name__ == "__main__":
    main()
