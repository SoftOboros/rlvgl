//! Shared helpers for rlvgl demonstration examples.
//!
//! Provides a reusable widget tree for both simulator and hardware demos.
//! The examples showcase core rlvgl widgets and plugin features using
//! placeholder graphics. Designed for `no_std` builds so that the same
//! module powers both simulator and embedded demonstrations.

extern crate alloc;

#[cfg(any(feature = "png", feature = "jpeg"))]
use alloc::boxed::Box;
use alloc::{format, rc::Rc, vec::Vec};
use core::cell::RefCell;

#[cfg(feature = "jpeg")]
use rlvgl::core::jpeg;
#[cfg(feature = "png")]
use rlvgl::core::png;
#[cfg(feature = "qrcode")]
use rlvgl::core::qrcode;
#[cfg(any(feature = "png", feature = "jpeg"))]
use rlvgl::core::widget::Color;
use rlvgl::core::{
    WidgetNode,
    widget::{Rect, Widget},
};
#[cfg(any(feature = "png", feature = "jpeg"))]
use rlvgl::widgets::image::Image;
use rlvgl::widgets::{button::Button, container::Container, label::Label};

type WidgetHandle = Rc<RefCell<dyn Widget>>;
type WidgetSlot = Rc<RefCell<Option<WidgetHandle>>>;

// 1x1 pixel PNG and JPEG images used to exercise the decoders without relying on
// external binary assets.
#[cfg(feature = "png")]
const PNG_LOGO: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xff, 0x9f, 0xa1,
    0x1e, 0x00, 0x07, 0x82, 0x02, 0x7f, 0x3e, 0x76, 0x52, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[cfg(feature = "jpeg")]
const JPEG_LOGO: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08,
    0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0a, 0x0c, 0x14, 0x0d, 0x0c, 0x0b, 0x0b, 0x0c, 0x19, 0x12,
    0x13, 0x0f, 0x14, 0x1d, 0x1a, 0x1f, 0x1e, 0x1d, 0x1a, 0x1c, 0x1c, 0x20, 0x24, 0x2e, 0x27, 0x20,
    0x22, 0x2c, 0x23, 0x1c, 0x1c, 0x28, 0x37, 0x29, 0x2c, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1f, 0x27,
    0x39, 0x3d, 0x38, 0x32, 0x3c, 0x2e, 0x33, 0x34, 0x32, 0xff, 0xdb, 0x00, 0x43, 0x01, 0x09, 0x09,
    0x09, 0x0c, 0x0b, 0x0c, 0x18, 0x0d, 0x0d, 0x18, 0x32, 0x21, 0x1c, 0x21, 0x32, 0x32, 0x32, 0x32,
    0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32,
    0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32,
    0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0xff,
    0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03,
    0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03,
    0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12,
    0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
    0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16,
    0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
    0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
    0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
    0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
    0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01, 0x00,
    0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x11,
    0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77,
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
    0xe2, 0xe8, 0xa2, 0x8a, 0xf9, 0x93, 0xf7, 0x13, 0xff, 0xd9,
];

/// State returned by [`build_demo`] containing the root widget tree and related
/// bookkeeping used by the demos.
pub struct Demo {
    /// Root of the demo widget tree.
    pub root: Rc<RefCell<WidgetNode>>,
    /// Counter incremented when the main button is clicked.
    pub counter: Rc<RefCell<u32>>,
    /// Widgets scheduled to be appended after event dispatch.
    pub pending: Rc<RefCell<Vec<WidgetNode>>>,
    /// Widget handles scheduled for removal after event dispatch.
    pub to_remove: Rc<RefCell<Vec<WidgetHandle>>>,
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
    let to_remove = Rc::new(RefCell::new(Vec::new()));

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
        "rlvgl Demo",
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
    #[cfg(feature = "qrcode")]
    let qr_demo: WidgetSlot = Rc::new(RefCell::new(None));
    #[cfg(feature = "png")]
    let png_demo: WidgetSlot = Rc::new(RefCell::new(None));
    #[cfg(feature = "jpeg")]
    let jpeg_demo: WidgetSlot = Rc::new(RefCell::new(None));
    {
        let pending_add = pending.clone();
        let pending_rm = to_remove.clone();
        let menu_ref = menu_widget.clone();
        #[cfg(feature = "qrcode")]
        let qr_ref = qr_demo.clone();
        #[cfg(feature = "png")]
        let png_ref = png_demo.clone();
        #[cfg(feature = "jpeg")]
        let jpeg_ref = jpeg_demo.clone();
        plugins.borrow_mut().set_on_click(move |_btn: &mut Button| {
            if let Some(menu_w) = menu_ref.borrow_mut().take() {
                pending_rm.borrow_mut().push(menu_w);
            } else {
                let menu_w: WidgetHandle = Rc::new(RefCell::new(Container::new(Rect {
                    x: 10,
                    y: 70,
                    width: 100,
                    height: 110,
                })));
                let mut children = Vec::new();

                #[cfg(feature = "qrcode")]
                {
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
                        let pending_add = pending_add.clone();
                        let pending_rm = pending_rm.clone();
                        let qr_demo = qr_ref.clone();
                        qr_button.borrow_mut().set_on_click(move |_b: &mut Button| {
                            if let Some(qr_w) = qr_demo.borrow_mut().take() {
                                pending_rm.borrow_mut().push(qr_w);
                            } else {
                                let demo = build_plugin_demo();
                                let handle = demo.widget.clone();
                                qr_demo.borrow_mut().replace(handle.clone());
                                pending_add.borrow_mut().push(demo);
                            }
                        });
                    }
                    children.push(WidgetNode {
                        widget: qr_button,
                        children: Vec::new(),
                    });
                }

                #[cfg(feature = "png")]
                {
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
                        let pending_add = pending_add.clone();
                        let pending_rm = pending_rm.clone();
                        let png_demo = png_ref.clone();
                        png_button
                            .borrow_mut()
                            .set_on_click(move |_b: &mut Button| {
                                if let Some(png_w) = png_demo.borrow_mut().take() {
                                    pending_rm.borrow_mut().push(png_w);
                                } else {
                                    let demo = build_png_demo();
                                    let handle = demo.widget.clone();
                                    png_demo.borrow_mut().replace(handle.clone());
                                    pending_add.borrow_mut().push(demo);
                                }
                            });
                    }
                    children.push(WidgetNode {
                        widget: png_button,
                        children: Vec::new(),
                    });
                }

                #[cfg(feature = "jpeg")]
                {
                    let jpeg_button = Rc::new(RefCell::new(Button::new(
                        "JPEG",
                        Rect {
                            x: 20,
                            y: 140,
                            width: 80,
                            height: 20,
                        },
                    )));
                    {
                        let pending_add = pending_add.clone();
                        let pending_rm = pending_rm.clone();
                        let jpeg_demo = jpeg_ref.clone();
                        jpeg_button
                            .borrow_mut()
                            .set_on_click(move |_b: &mut Button| {
                                if let Some(jpeg_w) = jpeg_demo.borrow_mut().take() {
                                    pending_rm.borrow_mut().push(jpeg_w);
                                } else {
                                    let demo = build_jpeg_demo();
                                    let handle = demo.widget.clone();
                                    jpeg_demo.borrow_mut().replace(handle.clone());
                                    pending_add.borrow_mut().push(demo);
                                }
                            });
                    }
                    children.push(WidgetNode {
                        widget: jpeg_button,
                        children: Vec::new(),
                    });
                }

                let menu = WidgetNode {
                    widget: menu_w.clone(),
                    children,
                };
                menu_ref.borrow_mut().replace(menu_w);
                pending_add.borrow_mut().push(menu);
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
        to_remove,
    }
}

#[cfg(feature = "qrcode")]
/// Build a widget demonstrating plugin features such as QR code generation.
pub fn build_plugin_demo() -> WidgetNode {
    let (pixels_vec, width, _) = qrcode::generate(b"https://github.com/SoftOboros/rlvgl").unwrap();
    let root_w = 320u32;
    let root_h = 240u32;
    // Match the area used by the PNG/JPEG demos: the lower-right 2/3rds of the display.
    let target = root_h * 2 / 3;
    let scale = target as f32 / width as f32;
    let new_w = target;
    let new_h = target;
    let mut scaled = Vec::with_capacity((new_w * new_h) as usize);
    for y in 0..new_h {
        for x in 0..new_w {
            let src_x = (x as f32 / scale).floor() as usize;
            let src_y = (y as f32 / scale).floor() as usize;
            let idx = src_y * width as usize + src_x;
            let color = pixels_vec
                .get(idx)
                .copied()
                .unwrap_or(Color(255, 255, 255, 255));
            scaled.push(color);
        }
    }
    let pixels: &'static [Color] = Box::leak(scaled.into_boxed_slice());
    let x_pos = (root_w - new_w) as i32;
    let y_pos = (root_h - new_h) as i32;
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: x_pos,
                y: y_pos,
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

#[cfg(feature = "png")]
/// Build a widget displaying the rlvgl logo decoded from an embedded PNG.
///
/// `scale` controls the desired scaling factor. The final image is clamped so
/// it never exceeds the 320x240 screen bounds, and it is anchored to the lower
/// right corner of the display.
pub fn build_png_demo_scaled(scale: f32) -> WidgetNode {
    let (pixels_vec, width, height) =
        png::decode(PNG_LOGO).expect("failed to decode built-in PNG asset");

    let root_w = 320u32;
    let root_h = 240u32;
    let mut scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    scale = scale
        .min(root_w as f32 / width as f32)
        .min(root_h as f32 / height as f32);

    let new_w = (width as f32 * scale).max(1.0).round() as u32;
    let new_h = (height as f32 * scale).max(1.0).round() as u32;

    let mut scaled = Vec::with_capacity((new_w * new_h) as usize);
    for y in 0..new_h {
        for x in 0..new_w {
            let src_x = (x as f32 / scale).floor() as usize;
            let src_y = (y as f32 / scale).floor() as usize;
            let idx = src_y * width as usize + src_x;
            let color = pixels_vec.get(idx).copied().unwrap_or(Color(0, 0, 0, 255));
            scaled.push(color);
        }
    }
    let pixels: &'static [Color] = Box::leak(scaled.into_boxed_slice());

    let x_pos = (root_w - new_w) as i32;
    let y_pos = (root_h - new_h) as i32;
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: x_pos,
                y: y_pos,
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

#[cfg(feature = "png")]
/// Build a PNG demo using the default scale of `0.5`.
pub fn build_png_demo() -> WidgetNode {
    build_png_demo_scaled(0.5)
}

#[cfg(feature = "jpeg")]
/// Build a widget displaying the rlvgl logo decoded from an embedded JPEG.
///
/// Mirrors [`build_png_demo_scaled`] but decodes a JPEG image instead.
pub fn build_jpeg_demo_scaled(scale: f32) -> WidgetNode {
    let (pixels_vec, width, height) =
        jpeg::decode(JPEG_LOGO).expect("failed to decode built-in JPEG asset");

    let width = width as u32;
    let height = height as u32;
    let root_w = 320u32;
    let root_h = 240u32;
    let mut scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    scale = scale
        .min(root_w as f32 / width as f32)
        .min(root_h as f32 / height as f32);

    let new_w = (width as f32 * scale).max(1.0).round() as u32;
    let new_h = (height as f32 * scale).max(1.0).round() as u32;

    let mut scaled = Vec::with_capacity((new_w * new_h) as usize);
    for y in 0..new_h {
        for x in 0..new_w {
            let src_x = (x as f32 / scale).floor() as usize;
            let src_y = (y as f32 / scale).floor() as usize;
            let idx = src_y * width as usize + src_x;
            let color = pixels_vec.get(idx).copied().unwrap_or(Color(0, 0, 0, 255));
            scaled.push(color);
        }
    }
    let pixels: &'static [Color] = Box::leak(scaled.into_boxed_slice());

    let x_pos = (root_w - new_w) as i32;
    let y_pos = (root_h - new_h) as i32;
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: x_pos,
                y: y_pos,
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

#[cfg(feature = "jpeg")]
/// Build a JPEG demo using the default scale of `0.5`.
pub fn build_jpeg_demo() -> WidgetNode {
    build_jpeg_demo_scaled(0.5)
}

/// Flush any widgets queued during event callbacks into the root tree.
pub fn flush_pending(
    root: &Rc<RefCell<WidgetNode>>,
    pending: &Rc<RefCell<Vec<WidgetNode>>>,
    to_remove: &Rc<RefCell<Vec<WidgetHandle>>>,
) {
    let mut root_ref = root.borrow_mut();
    for handle in to_remove.borrow_mut().drain(..) {
        root_ref
            .children
            .retain(|n| !Rc::ptr_eq(&n.widget, &handle));
    }
    let mut pending_nodes = pending.borrow_mut();
    root_ref.children.extend(pending_nodes.drain(..));
}
