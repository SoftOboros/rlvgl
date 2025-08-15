//! Raw RGBA image sequence format utilities for rlvgl-creator.
//!
//! Provides a simple `.raw` container with a max-frame header and optional
//! per-frame headers. Single images omit frame headers and store pixel data
//! directly after the file header.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Result, anyhow};
use image::{DynamicImage, GenericImageView};

/// Magic identifier for `.raw` files.
const MAGIC: &[u8; 8] = b"RLVGLRAW";

/// A single frame within a raw image sequence.
#[derive(Debug)]
pub struct Frame {
    /// X offset of the frame within the maximum bounds.
    pub x: u32,
    /// Y offset of the frame within the maximum bounds.
    pub y: u32,
    /// Width of the frame in pixels.
    pub width: u32,
    /// Height of the frame in pixels.
    pub height: u32,
    /// Raw RGBA8 pixel data for the frame.
    pub data: Vec<u8>,
}

/// Raw image sequence container.
#[derive(Debug)]
pub struct Sequence {
    /// Maximum width of any frame.
    pub max_width: u32,
    /// Maximum height of any frame.
    pub max_height: u32,
    /// Frames contained in the sequence.
    pub frames: Vec<Frame>,
}

impl Sequence {
    /// Create a sequence from a single [`DynamicImage`].
    pub fn from_image(img: DynamicImage) -> Self {
        let (width, height) = img.dimensions();
        Sequence {
            max_width: width,
            max_height: height,
            frames: vec![Frame {
                x: 0,
                y: 0,
                width,
                height,
                data: img.to_rgba8().into_raw(),
            }],
        }
    }

    /// Encode the sequence to a `.raw` file, applying simple token-based RLE
    /// compression when beneficial.
    pub fn encode<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        file.write_all(MAGIC)?;
        file.write_all(&self.max_width.to_le_bytes())?;
        file.write_all(&self.max_height.to_le_bytes())?;
        file.write_all(&(self.frames.len() as u32).to_le_bytes())?;

        if self.frames.len() == 1 {
            encode_frame(&mut file, &self.frames[0], true)?;
        } else {
            for f in &self.frames {
                file.write_all(&f.x.to_le_bytes())?;
                file.write_all(&f.y.to_le_bytes())?;
                file.write_all(&f.width.to_le_bytes())?;
                file.write_all(&f.height.to_le_bytes())?;
                encode_frame(&mut file, f, false)?;
            }
        }
        Ok(())
    }

    /// Decode a sequence from a `.raw` file.
    pub fn decode<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(anyhow!("invalid raw magic"));
        }

        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)?;
        let max_width = u32::from_le_bytes(buf);
        file.read_exact(&mut buf)?;
        let max_height = u32::from_le_bytes(buf);
        file.read_exact(&mut buf)?;
        let frame_count = u32::from_le_bytes(buf);

        let mut frames = Vec::with_capacity(frame_count as usize);
        if frame_count == 1 {
            frames.push(decode_frame(&mut file, max_width, max_height, true)?);
        } else {
            for _ in 0..frame_count {
                file.read_exact(&mut buf)?;
                let x = u32::from_le_bytes(buf);
                file.read_exact(&mut buf)?;
                let y = u32::from_le_bytes(buf);
                file.read_exact(&mut buf)?;
                let width = u32::from_le_bytes(buf);
                file.read_exact(&mut buf)?;
                let height = u32::from_le_bytes(buf);
                frames.push(decode_frame(&mut file, width, height, false)?.with_pos(x, y));
            }
        }

        Ok(Sequence {
            max_width,
            max_height,
            frames,
        })
    }
}

/// Convert a `DynamicImage` directly to a `.raw` file.
pub fn encode_image<P: AsRef<Path>>(img: DynamicImage, path: P) -> Result<()> {
    Sequence::from_image(img).encode(path)
}

/// Encode a frame to the file, optionally omitting position/size data.
fn encode_frame(file: &mut File, frame: &Frame, is_single: bool) -> Result<()> {
    if !is_single {
        // position and size already written by caller
    }
    let (tokens, indices) = tokenize(&frame.data);
    if tokens.is_empty() || tokens.len() > 256 {
        file.write_all(&0u32.to_le_bytes())?;
        file.write_all(&frame.data)?;
        return Ok(());
    }

    file.write_all(&(tokens.len() as u32).to_le_bytes())?;
    for t in &tokens {
        file.write_all(t)?;
    }
    let compressed = rle_encode(&indices);
    file.write_all(&(compressed.len() as u32).to_le_bytes())?;
    file.write_all(&compressed)?;
    Ok(())
}

/// Decode a frame from the file, using the given dimensions.
fn decode_frame(file: &mut File, width: u32, height: u32, is_single: bool) -> Result<Frame> {
    if !is_single {
        // position handled by caller
    }
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)?;
    let token_count = u32::from_le_bytes(buf);
    if token_count == 0 {
        let mut data = vec![0u8; (width * height * 4) as usize];
        file.read_exact(&mut data)?;
        return Ok(Frame {
            x: 0,
            y: 0,
            width,
            height,
            data,
        });
    }

    let mut tokens = Vec::with_capacity(token_count as usize);
    for _ in 0..token_count {
        let mut t = [0u8; 4];
        file.read_exact(&mut t)?;
        tokens.push(t);
    }
    file.read_exact(&mut buf)?;
    let comp_len = u32::from_le_bytes(buf);
    let mut comp = vec![0u8; comp_len as usize];
    file.read_exact(&mut comp)?;
    let indices = rle_decode(&comp);
    let mut data = Vec::with_capacity(indices.len() * 4);
    for idx in indices {
        data.extend_from_slice(&tokens[idx as usize]);
    }
    Ok(Frame {
        x: 0,
        y: 0,
        width,
        height,
        data,
    })
}

/// Build a token table and index stream for the pixel data.
fn tokenize(data: &[u8]) -> (Vec<[u8; 4]>, Vec<u8>) {
    let mut map: HashMap<u32, u8> = HashMap::new();
    let mut tokens = Vec::new();
    let mut indices = Vec::new();

    for chunk in data.chunks_exact(4) {
        let key = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if let Some(&idx) = map.get(&key) {
            indices.push(idx);
        } else {
            let idx = tokens.len() as u8;
            if idx == u8::MAX {
                return (Vec::new(), Vec::new());
            }
            tokens.push([chunk[0], chunk[1], chunk[2], chunk[3]]);
            map.insert(key, idx);
            indices.push(idx);
        }
    }

    (tokens, indices)
}

/// Run-length encode a slice of token indices.
fn rle_encode(indices: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if indices.is_empty() {
        return out;
    }
    let mut prev = indices[0];
    let mut run: u8 = 1;
    for &b in &indices[1..] {
        if b == prev && run < u8::MAX {
            run += 1;
        } else {
            out.push(run);
            out.push(prev);
            prev = b;
            run = 1;
        }
    }
    out.push(run);
    out.push(prev);
    out
}

/// Decode a run-length encoded slice of token indices.
fn rle_decode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut iter = data.chunks_exact(2);
    for pair in &mut iter {
        let run = pair[0];
        let val = pair[1];
        out.extend(std::iter::repeat(val).take(run as usize));
    }
    out
}

impl Frame {
    /// Attach position information after decoding.
    fn with_pos(mut self, x: u32, y: u32) -> Self {
        self.x = x;
        self.y = y;
        self
    }
}
