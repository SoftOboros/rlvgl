//! Helpers for rlvgl simulator examples.
//!
//! This crate provides a small widget tree and renderer used by the
//! simulator example. The demos showcase core rlvgl widgets and plugin
//! features using placeholder graphics.
#![no_std]

extern crate alloc;

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::RefCell;

#[cfg(not(feature = "fontdue"))]
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    text::Text,
};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use rlvgl::core::{
    WidgetNode, png, qrcode,
    renderer::Renderer,
    widget::{Color, Rect, Widget},
};
#[cfg(feature = "fontdue")]
use rlvgl::fontdue::{line_metrics, rasterize_glyph};
use rlvgl::widgets::{button::Button, container::Container, image::Image, label::Label};
#[cfg(feature = "fontdue")]
const FONT_DATA: &[u8] = include_bytes!("../../../lvgl/scripts/built_in_font/DejaVuSans.ttf");

type WidgetHandle = Rc<RefCell<dyn Widget>>;
type WidgetSlot = Rc<RefCell<Option<WidgetHandle>>>;

/// State returned by [`build_demo`] containing the root widget tree and related
/// bookkeeping used by the simulator.
pub struct Demo {
    /// Root of the demo widget tree.
    pub root: Rc<RefCell<WidgetNode>>,
    /// Counter incremented when the main button is clicked.
    pub counter: Rc<RefCell<u32>>,
    /// Widgets scheduled to be appended after event dispatch.
    pub pending: Rc<RefCell<Vec<WidgetNode>>>,
}

/// Build a simple widget tree demonstrating basic rlvgl widgets.
///
/// Returns a [`Demo`] struct containing the root [`WidgetNode`], a counter
/// incremented whenever the button is clicked, and a queue of widgets that
/// should be appended to the root after event dispatch. A `Plugins` button is
/// included to showcase optional features.
pub fn build_demo() -> Demo {
    let click_count = Rc::new(RefCell::new(0));
    let pending = Rc::new(RefCell::new(Vec::new()));

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

    let root = Rc::new(RefCell::new(WidgetNode {
        widget: Rc::new(RefCell::new(Container::new(Rect {
            x: 0,
            y: 0,
            width: 320,
            height: 240,
        }))),
        children: Vec::new(),
    }));

    let label = Label::new(
        "rlvgl demo",
        Rect {
            x: 10,
            y: 10,
            width: 120,
            height: 20,
        },
    );
    root.borrow_mut().children.push(WidgetNode {
        widget: Rc::new(RefCell::new(label)),
        children: Vec::new(),
    });
    root.borrow_mut().children.push(WidgetNode {
        widget: button.clone(),
        children: Vec::new(),
    });

    let plugins = Rc::new(RefCell::new(Button::new(
        "Plugins",
        Rect {
            x: 100,
            y: 40,
            width: 80,
            height: 20,
        },
    )));
    let menu_widget: WidgetSlot = Rc::new(RefCell::new(None));
    let qr_demo: WidgetSlot = Rc::new(RefCell::new(None));
    let png_demo: WidgetSlot = Rc::new(RefCell::new(None));
    {
        let pending_ref = pending.clone();
        let root_ref = root.clone();
        let menu_ref = menu_widget.clone();
        let qr_ref = qr_demo.clone();
        let png_ref = png_demo.clone();
        plugins.borrow_mut().set_on_click(move |_btn: &mut Button| {
            if let Some(menu_w) = menu_ref.borrow_mut().take() {
                root_ref
                    .borrow_mut()
                    .children
                    .retain(|n| !Rc::ptr_eq(&n.widget, &menu_w));
            } else {
                let menu_w: WidgetHandle = Rc::new(RefCell::new(Container::new(Rect {
                    x: 10,
                    y: 70,
                    width: 100,
                    height: 80,
                })));
                let mut menu = WidgetNode {
                    widget: menu_w.clone(),
                    children: Vec::new(),
                };

                let qr_button = Rc::new(RefCell::new(Button::new(
                    "QR Code",
                    Rect {
                        x: 20,
                        y: 80,
                        width: 80,
                        height: 20,
                    },
                )));
                {
                    let pending_ref = pending_ref.clone();
                    let root = root_ref.clone();
                    let qr_demo = qr_ref.clone();
                    qr_button.borrow_mut().set_on_click(move |_b: &mut Button| {
                        if let Some(qr_w) = qr_demo.borrow_mut().take() {
                            root.borrow_mut()
                                .children
                                .retain(|n| !Rc::ptr_eq(&n.widget, &qr_w));
                        } else {
                            let demo = build_plugin_demo();
                            let handle = demo.widget.clone();
                            qr_demo.borrow_mut().replace(handle.clone());
                            pending_ref.borrow_mut().push(demo);
                        }
                    });
                }
                menu.children.push(WidgetNode {
                    widget: qr_button,
                    children: Vec::new(),
                });

                let png_button = Rc::new(RefCell::new(Button::new(
                    "PNG",
                    Rect {
                        x: 20,
                        y: 110,
                        width: 80,
                        height: 20,
                    },
                )));
                {
                    let pending_ref = pending_ref.clone();
                    let root = root_ref.clone();
                    let png_demo = png_ref.clone();
                    png_button
                        .borrow_mut()
                        .set_on_click(move |_b: &mut Button| {
                            if let Some(png_w) = png_demo.borrow_mut().take() {
                                root.borrow_mut()
                                    .children
                                    .retain(|n| !Rc::ptr_eq(&n.widget, &png_w));
                            } else {
                                let demo = build_png_demo();
                                let handle = demo.widget.clone();
                                png_demo.borrow_mut().replace(handle.clone());
                                pending_ref.borrow_mut().push(demo);
                            }
                        });
                }
                menu.children.push(WidgetNode {
                    widget: png_button,
                    children: Vec::new(),
                });

                menu_ref.borrow_mut().replace(menu_w);
                pending_ref.borrow_mut().push(menu);
            }
        });
    }
    root.borrow_mut().children.push(WidgetNode {
        widget: plugins.clone(),
        children: Vec::new(),
    });

    Demo {
        root,
        counter: click_count,
        pending,
    }
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

/// Build a widget displaying the rlvgl logo decoded from a PNG asset.
pub fn build_png_demo() -> WidgetNode {
    let data = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/rlvgl-logo.png"
    ));
    let (pixels_vec, width, height) = png::decode(data).unwrap();
    let max_dim = 100u32;
    let scale = (max_dim as f32 / width as f32)
        .min(max_dim as f32 / height as f32)
        .min(1.0);
    let new_w = (width as f32 * scale).round() as u32;
    let new_h = (height as f32 * scale).round() as u32;
    let mut scaled = Vec::with_capacity((new_w * new_h) as usize);
    for y in 0..new_h {
        for x in 0..new_w {
            let src_x = (x as f32 / scale).floor() as usize;
            let src_y = (y as f32 / scale).floor() as usize;
            let idx = src_y * width as usize + src_x;
            scaled.push(pixels_vec[idx]);
        }
    }
    let pixels: &'static [Color] = Box::leak(scaled.into_boxed_slice());
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: 150,
                y: 40,
                width: new_w as i32,
                height: new_h as i32,
            },
            new_w as i32,
            new_h as i32,
            pixels,
        ))),
        children: Vec::new(),
    }
}

/// Flush any widgets queued during event callbacks into the root tree.
pub fn flush_pending(root: &Rc<RefCell<WidgetNode>>, pending: &Rc<RefCell<Vec<WidgetNode>>>) {
    let mut root_ref = root.borrow_mut();
    let mut pending_nodes = pending.borrow_mut();
    root_ref.children.extend(pending_nodes.drain(..));
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
        #[cfg(feature = "fontdue")]
        {
            if let Ok(vm) = line_metrics(FONT_DATA, 16.0) {
                let baseline = position.1 + vm.descent.round() as i32;
                let mut x_cursor = position.0;
                for ch in text.chars() {
                    if let Ok((bitmap, metrics)) = rasterize_glyph(FONT_DATA, ch, 16.0) {
                        let w = metrics.width as i32;
                        let h = metrics.height as i32;
                        let top = baseline + metrics.ymin;
                        let bottom = top + h;
                        if bottom < 0 || top >= self.height as i32 {
                            x_cursor += metrics.advance_width.round() as i32;
                            continue;
                        }
                        for y in 0..h {
                            let py = baseline + metrics.ymin + y;
                            if py < 0 || (py as usize) >= self.height {
                                continue;
                            }
                            for x in 0..w {
                                let px = x_cursor + metrics.xmin + x;
                                if px < 0 || (px as usize) >= self.width {
                                    continue;
                                }
                                let alpha = bitmap[y as usize * metrics.width + x as usize];
                                if alpha > 0 {
                                    let idx = ((py as usize) * self.width + px as usize) * 4;
                                    let bg_r = self.frame[idx];
                                    let bg_g = self.frame[idx + 1];
                                    let bg_b = self.frame[idx + 2];
                                    let inv_alpha = 255 - alpha as u16;
                                    let r = ((color.0 as u16 * alpha as u16
                                        + bg_r as u16 * inv_alpha)
                                        / 255) as u8;
                                    let g = ((color.1 as u16 * alpha as u16
                                        + bg_g as u16 * inv_alpha)
                                        / 255) as u8;
                                    let b = ((color.2 as u16 * alpha as u16
                                        + bg_b as u16 * inv_alpha)
                                        / 255) as u8;
                                    self.frame[idx] = r;
                                    self.frame[idx + 1] = g;
                                    self.frame[idx + 2] = b;
                                    self.frame[idx + 3] = 0xff;
                                }
                            }
                        }
                        x_cursor += metrics.advance_width.round() as i32;
                    }
                }
            }
        }
        #[cfg(not(feature = "fontdue"))]
        {
            let style = MonoTextStyle::new(&FONT_6X10, Rgb888::new(color.0, color.1, color.2));
            let _ = Text::new(text, Point::new(position.0, position.1), style).draw(self);
        }
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
