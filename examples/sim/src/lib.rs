//! Helpers for rlvgl simulator examples.
//!
//! This crate provides a small widget tree and renderer used by the
//! simulator example. The demos showcase core rlvgl widgets and plugin
//! features using placeholder graphics.
#![no_std]

extern crate alloc;

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::cell::RefCell;

use rlvgl::core::{
    WidgetNode, png, qrcode,
    widget::{Color, Rect, Widget},
};
use rlvgl::widgets::{button::Button, container::Container, image::Image, label::Label};

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
    let qr_demo: WidgetSlot = Rc::new(RefCell::new(None));
    let png_demo: WidgetSlot = Rc::new(RefCell::new(None));
    {
        let pending_add = pending.clone();
        let pending_rm = to_remove.clone();
        let menu_ref = menu_widget.clone();
        let qr_ref = qr_demo.clone();
        let png_ref = png_demo.clone();
        plugins.borrow_mut().set_on_click(move |_btn: &mut Button| {
            if let Some(menu_w) = menu_ref.borrow_mut().take() {
                pending_rm.borrow_mut().push(menu_w);
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
                menu.children.push(WidgetNode {
                    widget: png_button,
                    children: Vec::new(),
                });

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

/// Build a widget demonstrating plugin features such as QR code generation.
pub fn build_plugin_demo() -> WidgetNode {
    let (pixels_vec, width, height) = qrcode::generate(b"Echo Go").unwrap();
    let pixels: &'static [Color] = Box::leak(pixels_vec.into_boxed_slice());
    WidgetNode {
        widget: Rc::new(RefCell::new(Image::new(
            Rect {
                x: 200,
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
///
/// `scale` controls the desired scaling factor. The final image is clamped so
/// it never exceeds the 320x240 screen bounds, and it is anchored to the lower
/// right corner of the display.
pub fn build_png_demo_scaled(scale: f32) -> WidgetNode {
    let data = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/rlvgl-logo.png"
    ));
    let (pixels_vec, width, height) =
        png::decode(data).expect("failed to decode built-in PNG asset");

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
            let color = pixels_vec.get(idx).copied().unwrap_or(Color(0, 0, 0));
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

/// Build a PNG demo using the default scale of `0.5`.
pub fn build_png_demo() -> WidgetNode {
    build_png_demo_scaled(0.5)
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

#[cfg(test)]
extern crate std;
