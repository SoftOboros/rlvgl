//! Generate QR codes as pixel buffers.
use crate::widget::Color;
use alloc::vec::Vec;
use qrcode::{
    QrCode,
    types::{Color as QrColor, QrError},
};

/// Generate a QR code bitmap for the provided data.
pub fn generate(data: &[u8]) -> Result<(Vec<Color>, u32, u32), QrError> {
    let code = QrCode::new(data)?;
    let width = code.width() as u32;
    let modules = code.into_colors();
    let mut pixels = Vec::with_capacity((width * width) as usize);
    for m in modules {
        match m {
            QrColor::Dark => pixels.push(Color(0, 0, 0, 255)),
            QrColor::Light => pixels.push(Color(255, 255, 255, 255)),
        }
    }
    Ok((pixels, width, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_simple_qr() {
        let (pixels, w, h) = generate(b"hello").unwrap();
        assert_eq!(w, h);
        assert_eq!(pixels.len(), (w * h) as usize);
        assert_eq!(pixels[0], Color(0, 0, 0, 255));
    }
}
