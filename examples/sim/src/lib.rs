//! Helpers for rlvgl simulator examples.
//!
//! This crate provides a small widget tree and renderer used by the
//! simulator example. The demos showcase core rlvgl widgets and plugin
//! features using placeholder graphics.
#![no_std]

extern crate alloc;

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::RefCell;

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};
use rlvgl::core::{
    WidgetNode, qrcode,
    renderer::Renderer,
    widget::{Color, Rect},
};
use rlvgl::widgets::{button::Button, container::Container, image::Image, label::Label};

/// Build a simple widget tree demonstrating basic rlvgl widgets.
///
/// Returns the root [`WidgetNode`] and a counter incremented whenever the
/// button is clicked.
pub fn build_demo() -> (WidgetNode, Rc<RefCell<u32>>) {
    let click_count = Rc::new(RefCell::new(0));

    let button = Rc::new(RefCell::new(Button::new(
        "Clicks: 0",
        Rect {
            x: 10,
            y: 40,
            width: 80,
            height: 20,
        },
    )));

    {
        let counter = click_count.clone();
        button.borrow_mut().set_on_click(move |btn: &mut Button| {
            let mut count = counter.borrow_mut();
            *count += 1;
            btn.set_text(format!("Clicks: {}", *count));
        });
    }

    let mut root = WidgetNode {
        widget: Rc::new(RefCell::new(Container::new(Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 240,
        }))),
        children: Vec::new(),
    };

    let label = Label::new(
        "rlvgl demo",
        Rect {
            x: 10,
            y: 10,
            width: 120,
            height: 20,
        },
    );
    root.children.push(WidgetNode {
        widget: Rc::new(RefCell::new(label)),
        children: Vec::new(),
    });
    root.children.push(WidgetNode {
        widget: button.clone(),
        children: Vec::new(),
    });

    (root, click_count)
}

/// Build a widget demonstrating plugin features such as QR code generation.
pub fn build_plugin_demo() -> WidgetNode {
    let (pixels_vec, width, height) = qrcode::generate(b"Echo Go").unwrap();
    let pixels: &'static [Color] = Box::leak(pixels_vec.into_boxed_slice());
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: 150,
                y: 40,
                width: width as i32,
                height: height as i32,
            },
            width as i32,
            height as i32,
            pixels,
        ))),
        children: Vec::new(),
    }
}

/// Renderer that draws into the pixel buffer supplied by [`PixelsDisplay`].
pub struct PixelsRenderer<'a> {
    frame: &'a mut [u8],
    width: usize,
    height: usize,
}

impl<'a> PixelsRenderer<'a> {
    /// Create a new renderer for the given frame buffer.
    pub fn new(frame: &'a mut [u8], width: usize, height: usize) -> Self {
        Self {
            frame,
            width,
            height,
        }
    }

    fn put_pixel(&mut self, x: i32, y: i32, color: Rgb888) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let idx = ((y as usize) * self.width + x as usize) * 4;
            self.frame[idx] = color.r();
            self.frame[idx + 1] = color.g();
            self.frame[idx + 2] = color.b();
            self.frame[idx + 3] = 0xff;
        }
    }
}

impl<'a> Renderer for PixelsRenderer<'a> {
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rgb = Rgb888::new(color.0, color.1, color.2);
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width).min(self.width as i32);
        let y1 = (rect.y + rect.height).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                self.put_pixel(x, y, rgb);
            }
        }
    }

    fn draw_text(&mut self, position: (i32, i32), text: &str, color: Color) {
        let style = MonoTextStyle::new(&FONT_6X10, Rgb888::new(color.0, color.1, color.2));
        let _ = Text::new(text, Point::new(position.0, position.1), style).draw(self);
    }
}

impl<'a> DrawTarget for PixelsRenderer<'a> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            self.put_pixel(point.x, point.y, color);
        }
        Ok(())
    }
}

impl<'a> OriginDimensions for PixelsRenderer<'a> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

#[cfg(test)]
extern crate std;
