//! Ratatui-to-rlvgl color conversion.

use ratatui_core::style::Color;
use rlvgl_core::widget::Color as RlvglColor;

pub(crate) const fn resolve(color: Color, reset: RlvglColor) -> RlvglColor {
    let (r, g, b) = match color {
        Color::Reset => return reset,
        Color::Black => indexed(0),
        Color::Red => indexed(1),
        Color::Green => indexed(2),
        Color::Yellow => indexed(3),
        Color::Blue => indexed(4),
        Color::Magenta => indexed(5),
        Color::Cyan => indexed(6),
        Color::Gray => indexed(7),
        Color::DarkGray => indexed(8),
        Color::LightRed => indexed(9),
        Color::LightGreen => indexed(10),
        Color::LightYellow => indexed(11),
        Color::LightBlue => indexed(12),
        Color::LightMagenta => indexed(13),
        Color::LightCyan => indexed(14),
        Color::White => indexed(15),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(index) => indexed(index),
    };
    RlvglColor(r, g, b, 255)
}

const fn indexed(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    if index < 16 {
        return ANSI[index as usize];
    }
    if index < 232 {
        let cube = index - 16;
        let r = cube / 36;
        let g = (cube % 36) / 6;
        let b = cube % 6;
        return (cube_level(r), cube_level(g), cube_level(b));
    }
    let gray = 8 + (index - 232) * 10;
    (gray, gray, gray)
}

const fn cube_level(value: u8) -> u8 {
    if value == 0 { 0 } else { 55 + value * 40 }
}

pub(crate) const fn dim(color: RlvglColor) -> RlvglColor {
    RlvglColor(color.0 / 2, color.1 / 2, color.2 / 2, color.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_palette_has_expected_boundaries() {
        assert_eq!(
            resolve(Color::Indexed(16), RlvglColor(1, 2, 3, 4)),
            RlvglColor(0, 0, 0, 255)
        );
        assert_eq!(
            resolve(Color::Indexed(231), RlvglColor(1, 2, 3, 4)),
            RlvglColor(255, 255, 255, 255)
        );
        assert_eq!(
            resolve(Color::Indexed(232), RlvglColor(1, 2, 3, 4)),
            RlvglColor(8, 8, 8, 255)
        );
        assert_eq!(
            resolve(Color::Indexed(255), RlvglColor(1, 2, 3, 4)),
            RlvglColor(238, 238, 238, 255)
        );
    }

    #[test]
    fn reset_uses_surface_default() {
        let reset = RlvglColor(1, 2, 3, 4);
        assert_eq!(resolve(Color::Reset, reset), reset);
    }
}
