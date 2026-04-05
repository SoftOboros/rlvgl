//! Minimal WAV (RIFF) header parser for embedded audio playback.
//!
//! Parses canonical and extended WAV headers to extract PCM format info.
//! Only uncompressed PCM (format tag 1) is supported.

/// Parsed WAV file header information.
#[derive(Debug, Clone, Copy)]
pub struct WavHeader {
    /// Sample rate in Hz (e.g. 48000).
    pub sample_rate: u32,
    /// Bits per sample (e.g. 16).
    pub bits_per_sample: u16,
    /// Number of channels (1 = mono, 2 = stereo).
    pub num_channels: u16,
    /// Byte offset of the PCM data chunk within the file.
    pub data_offset: u32,
    /// Length of the PCM data in bytes.
    pub data_length: u32,
}

/// WAV parsing errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavError {
    /// Buffer too small to contain a valid header.
    TooShort,
    /// Missing "RIFF" magic at offset 0.
    NotRiff,
    /// Missing "WAVE" format identifier at offset 8.
    NotWave,
    /// The "fmt " chunk was not found.
    NoFmtChunk,
    /// The "data" chunk was not found.
    NoDataChunk,
    /// Format tag is not 1 (uncompressed PCM).
    NotPcm,
}

fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]])
}

/// Parse a WAV header from the first bytes of a file.
///
/// `buf` should contain at least the first 128 bytes of the file (more if the
/// header contains extra chunks before the "data" chunk).
///
/// Returns the parsed header or an error.
pub fn parse_wav_header(buf: &[u8]) -> Result<WavHeader, WavError> {
    if buf.len() < 44 {
        return Err(WavError::TooShort);
    }

    // RIFF magic
    if &buf[0..4] != b"RIFF" {
        return Err(WavError::NotRiff);
    }

    // WAVE format
    if &buf[8..12] != b"WAVE" {
        return Err(WavError::NotWave);
    }

    // Scan chunks starting at offset 12
    let mut pos = 12usize;
    let mut fmt_found = false;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut num_channels = 0u16;
    let mut data_offset = 0u32;
    let mut data_length = 0u32;
    let mut data_found = false;

    while pos + 8 <= buf.len() {
        let chunk_id = &buf[pos..pos + 4];
        let chunk_size = read_u32_le(buf, pos + 4) as usize;

        if chunk_id == b"fmt " {
            if pos + 8 + 16 > buf.len() {
                return Err(WavError::TooShort);
            }
            let fmt_base = pos + 8;
            let format_tag = read_u16_le(buf, fmt_base);
            if format_tag != 1 {
                return Err(WavError::NotPcm);
            }
            num_channels = read_u16_le(buf, fmt_base + 2);
            sample_rate = read_u32_le(buf, fmt_base + 4);
            // skip byte_rate (4 bytes) and block_align (2 bytes)
            bits_per_sample = read_u16_le(buf, fmt_base + 14);
            fmt_found = true;
        } else if chunk_id == b"data" {
            data_offset = (pos + 8) as u32;
            data_length = chunk_size as u32;
            data_found = true;
            break;
        }

        // Advance to next chunk (chunks are word-aligned)
        pos += 8 + ((chunk_size + 1) & !1);
    }

    if !fmt_found {
        return Err(WavError::NoFmtChunk);
    }
    if !data_found {
        return Err(WavError::NoDataChunk);
    }

    Ok(WavHeader {
        sample_rate,
        bits_per_sample,
        num_channels,
        data_offset,
        data_length,
    })
}
