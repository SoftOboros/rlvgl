//! Demo application for rlvgl showcasing core widgets and plugin features.
//!
//! Implements the [`Application`] trait so it can be loaded statically or
//! dynamically by the simulator and hardware runtimes.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(any(
    test,
    feature = "gif",
    all(feature = "jpeg", not(target_os = "none")),
    all(feature = "png", not(target_os = "none")),
    all(feature = "qrcode", not(target_os = "none"))
))]
extern crate std;

use alloc::boxed::Box;
use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

#[cfg(feature = "gif")]
use rlvgl_core::gif;
#[cfg(all(feature = "jpeg", not(target_os = "none")))]
use rlvgl_core::jpeg;
#[cfg(all(feature = "png", not(target_os = "none")))]
use rlvgl_core::png;
#[cfg(all(feature = "qrcode", not(target_os = "none")))]
use rlvgl_core::qrcode;

use rlvgl_core::widget::Color;
use rlvgl_core::{
    WidgetNode,
    application::{AppInfo, Application},
    event::Event,
    widget::{Rect, Widget},
};
#[cfg(any(
    all(feature = "png", not(target_os = "none")),
    all(feature = "jpeg", not(target_os = "none")),
    feature = "gif"
))]
use rlvgl_widgets::image::Image;
use rlvgl_widgets::{
    arc::Arc,
    bar::Bar,
    button::Button,
    button_matrix::ButtonMatrix,
    chart::{Chart, ChartAxis},
    container::Container,
    label::Label,
    led::Led,
    table::Table,
};

use rlvgl_i18n::t;

type WidgetHandle = Rc<RefCell<dyn Widget>>;
type WidgetSlot = Rc<RefCell<Option<WidgetHandle>>>;

// 1x1 pixel PNG and JPEG images used to exercise the decoders without relying on
// external binary assets.
#[cfg(all(feature = "png", not(target_os = "none")))]
const PNG_LOGO: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

#[cfg(all(feature = "jpeg", not(target_os = "none")))]
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
    0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32, 0x32,
    0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01,
    0x03, 0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04,
    0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05,
    0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1,
    0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a,
    0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38,
    0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
    0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
    0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5,
    0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3,
    0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9,
    0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01,
    0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5,
    0x11, 0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02,
    0x77, 0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61,
    0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52,
    0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a,
    0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47,
    0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67,
    0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86,
    0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4,
    0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2,
    0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9,
    0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7,
    0xf8, 0xf9, 0xfa, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f,
    0x00, 0xe2, 0xe8, 0xa2, 0x8a, 0xf9, 0x93, 0xf7, 0x13, 0xff, 0xd9,
];

#[cfg(feature = "gif")]
const GIF_LOGO: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x37, 0x61, 0x01, 0x00, 0x01, 0x00, 0xf0, 0x00, 0x00, 0xff, 0x00, 0x00,
    0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

/// Demo application state.
pub struct DemoApp {
    pending: Rc<RefCell<Vec<WidgetNode>>>,
    to_remove: Rc<RefCell<Vec<WidgetHandle>>>,
    counter: Rc<RefCell<u32>>,
}

impl DemoApp {
    /// Create a new demo application instance.
    pub fn new() -> Self {
        Self {
            pending: Rc::new(RefCell::new(Vec::new())),
            to_remove: Rc::new(RefCell::new(Vec::new())),
            counter: Rc::new(RefCell::new(0)),
        }
    }
}

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl Application for DemoApp {
    fn info(&self) -> AppInfo {
        AppInfo {
            name: "rlvgl-demo",
            version: "0.1.0",
            preferred_width: 320,
            preferred_height: 240,
        }
    }

    fn build(&mut self, width: u32, height: u32) -> WidgetNode {
        let w = width as i32;
        let h = height as i32;
        let root_w = width;
        let root_h = height;

        let button = Rc::new(RefCell::new(Button::new(
            t!("demo.clicks_zero"),
            Rect {
                x: 10,
                y: 40,
                width: 80,
                height: 20,
            },
        )));

        {
            let counter = self.counter.clone();
            button.borrow_mut().set_on_click(move |btn: &mut Button| {
                let mut count = counter.borrow_mut();
                *count += 1;
                btn.set_text(t!("demo.clicks", count = *count));
            });
        }

        let label = Label::new(
            t!("demo.title", version = "0.1.9"),
            Rect {
                x: 10,
                y: 10,
                width: 200,
                height: 20,
            },
        );

        let plugins = Rc::new(RefCell::new(Button::new(
            t!("demo.plugins"),
            Rect {
                x: 100,
                y: 40,
                width: 80,
                height: 20,
            },
        )));
        let menu_widget: WidgetSlot = Rc::new(RefCell::new(None));
        #[cfg(all(feature = "qrcode", not(target_os = "none")))]
        let qr_demo: WidgetSlot = Rc::new(RefCell::new(None));
        #[cfg(all(feature = "png", not(target_os = "none")))]
        let png_demo: WidgetSlot = Rc::new(RefCell::new(None));
        #[cfg(feature = "gif")]
        let gif_demo: WidgetSlot = Rc::new(RefCell::new(None));
        #[cfg(all(feature = "jpeg", not(target_os = "none")))]
        let jpeg_demo: WidgetSlot = Rc::new(RefCell::new(None));
        {
            let pending_add = self.pending.clone();
            let pending_rm = self.to_remove.clone();
            let menu_ref = menu_widget.clone();
            #[cfg(all(feature = "qrcode", not(target_os = "none")))]
            let qr_ref = qr_demo.clone();
            #[cfg(all(feature = "png", not(target_os = "none")))]
            let png_ref = png_demo.clone();
            #[cfg(feature = "gif")]
            let gif_ref = gif_demo.clone();
            #[cfg(all(feature = "jpeg", not(target_os = "none")))]
            let jpeg_ref = jpeg_demo.clone();
            let _root_w = root_w;
            let _root_h = root_h;
            plugins.borrow_mut().set_on_click(move |_btn: &mut Button| {
                if let Some(menu_w) = menu_ref.borrow_mut().take() {
                    pending_rm.borrow_mut().push(menu_w);
                } else {
                    let menu_w: WidgetHandle = Rc::new(RefCell::new(Container::new(Rect {
                        x: 10,
                        y: 70,
                        width: 100,
                        height: 170,
                    })));
                    #[allow(unused_mut)]
                    let mut children = Vec::new();

                    #[cfg(all(feature = "qrcode", not(target_os = "none")))]
                    {
                        let qr_button = Rc::new(RefCell::new(Button::new(
                            t!("demo.qr_code"),
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
                            let w = root_w;
                            let h = root_h;
                            qr_button.borrow_mut().set_on_click(move |_b: &mut Button| {
                                if let Some(qr_w) = qr_demo.borrow_mut().take() {
                                    pending_rm.borrow_mut().push(qr_w);
                                } else {
                                    let demo = build_plugin_demo(w, h);
                                    let handle = demo.widget.clone();
                                    qr_demo.borrow_mut().replace(handle.clone());
                                    pending_add.borrow_mut().push(demo);
                                }
                            });
                        }
                        children.push(WidgetNode {
                            widget: qr_button,
                            children: Vec::new(),
                            tag: None,
                        });
                    }

                    #[cfg(all(feature = "png", not(target_os = "none")))]
                    {
                        let png_button = Rc::new(RefCell::new(Button::new(
                            t!("demo.png"),
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
                            let w = root_w;
                            let h = root_h;
                            png_button
                                .borrow_mut()
                                .set_on_click(move |_b: &mut Button| {
                                    if let Some(png_w) = png_demo.borrow_mut().take() {
                                        pending_rm.borrow_mut().push(png_w);
                                    } else {
                                        let demo = build_png_demo(w, h);
                                        let handle = demo.widget.clone();
                                        png_demo.borrow_mut().replace(handle.clone());
                                        pending_add.borrow_mut().push(demo);
                                    }
                                });
                        }
                        children.push(WidgetNode {
                            widget: png_button,
                            children: Vec::new(),
                            tag: None,
                        });
                    }

                    #[cfg(feature = "gif")]
                    {
                        let gif_button = Rc::new(RefCell::new(Button::new(
                            t!("demo.gif"),
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
                            let gif_demo = gif_ref.clone();
                            let w = root_w;
                            let h = root_h;
                            gif_button
                                .borrow_mut()
                                .set_on_click(move |_b: &mut Button| {
                                    if let Some(gif_w) = gif_demo.borrow_mut().take() {
                                        pending_rm.borrow_mut().push(gif_w);
                                    } else {
                                        let demo = build_gif_demo(w, h);
                                        let handle = demo.widget.clone();
                                        gif_demo.borrow_mut().replace(handle.clone());
                                        pending_add.borrow_mut().push(demo);
                                    }
                                });
                        }
                        children.push(WidgetNode {
                            widget: gif_button,
                            children: Vec::new(),
                            tag: None,
                        });
                    }

                    #[cfg(all(feature = "jpeg", not(target_os = "none")))]
                    {
                        let jpeg_button = Rc::new(RefCell::new(Button::new(
                            t!("demo.jpeg"),
                            Rect {
                                x: 20,
                                y: 170,
                                width: 80,
                                height: 20,
                            },
                        )));
                        {
                            let pending_add = pending_add.clone();
                            let pending_rm = pending_rm.clone();
                            let jpeg_demo = jpeg_ref.clone();
                            let w = root_w;
                            let h = root_h;
                            jpeg_button
                                .borrow_mut()
                                .set_on_click(move |_b: &mut Button| {
                                    if let Some(jpeg_w) = jpeg_demo.borrow_mut().take() {
                                        pending_rm.borrow_mut().push(jpeg_w);
                                    } else {
                                        let demo = build_jpeg_demo(w, h);
                                        let handle = demo.widget.clone();
                                        jpeg_demo.borrow_mut().replace(handle.clone());
                                        pending_add.borrow_mut().push(demo);
                                    }
                                });
                        }
                        children.push(WidgetNode {
                            widget: jpeg_button,
                            children: Vec::new(),
                            tag: None,
                        });
                    }

                    let menu = WidgetNode {
                        widget: menu_w.clone(),
                        children,
                        tag: None,
                    };
                    menu_ref.borrow_mut().replace(menu_w);
                    pending_add.borrow_mut().push(menu);
                }
            });
        }

        WidgetNode {
            widget: Rc::new(RefCell::new(Container::new(Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            }))),
            children: alloc::vec![
                WidgetNode {
                    widget: Rc::new(RefCell::new(label)),
                    children: Vec::new(),
                    tag: None,
                },
                WidgetNode {
                    widget: button.clone(),
                    children: Vec::new(),
                    tag: None,
                },
                WidgetNode {
                    widget: plugins.clone(),
                    children: Vec::new(),
                    tag: None,
                },
            ],
            tag: None,
        }
    }

    fn after_event(&mut self, root: &Rc<RefCell<WidgetNode>>, _event: &Event) {
        flush_pending(root, &self.pending, &self.to_remove);
    }

    fn tick(&mut self, _root: &Rc<RefCell<WidgetNode>>) {}

    fn destroy(&mut self) {}
}

/// Create a new demo application as a boxed trait object.
pub fn create_app() -> Box<dyn Application> {
    Box::new(DemoApp::new())
}

/// Build a small host-simulator parity gallery covering representative LPAR
/// widgets from the primitive, control, and data-rich waves.
pub fn build_lpar_parity_demo(root_w: u32, root_h: u32) -> WidgetNode {
    let root = Rect {
        x: 0,
        y: 0,
        width: root_w as i32,
        height: root_h as i32,
    };

    let mut arc = Arc::new(
        Rect {
            x: 12,
            y: 14,
            width: 72,
            height: 72,
        },
        0,
        100,
    );
    arc.set_value(68);

    let mut bar = Bar::new(
        Rect {
            x: 96,
            y: 20,
            width: 138,
            height: 20,
        },
        0,
        100,
    );
    bar.set_value(72);
    bar.indicator_color = Color(0, 150, 110, 255);

    let mut led = Led::new(Rect {
        x: 248,
        y: 14,
        width: 46,
        height: 46,
    });
    led.set_color(Color(0, 190, 80, 255));
    led.set_brightness(220);

    let mut buttons = ButtonMatrix::new(Rect {
        x: 96,
        y: 54,
        width: 198,
        height: 54,
    });
    buttons.set_map(&["One", "Two", "\n", "Three", "Four"]);

    let mut chart = Chart::new(Rect {
        x: 12,
        y: 118,
        width: 138,
        height: 92,
    });
    let series = chart.add_series(Color(20, 110, 230, 255), ChartAxis::Primary);
    chart.set_points(series, &[12, 55, 38, 80, 64, 92, 42, 70]);

    let mut table = Table::new(Rect {
        x: 164,
        y: 118,
        width: 130,
        height: 92,
    });
    table.set_column_count(2);
    table.set_row_count(3);
    table.set_column_width(0, 54);
    table.set_cell_value(0, 0, "LPAR");
    table.set_cell_value(0, 1, "OK");
    table.set_cell_value(1, 0, "Draw");
    table.set_cell_value(1, 1, "6");
    table.set_cell_value(2, 0, "Gate");
    table.set_cell_value(2, 1, "Sim");

    WidgetNode {
        widget: Rc::new(RefCell::new(Container::new(root))),
        children: alloc::vec![
            WidgetNode {
                widget: Rc::new(RefCell::new(arc)),
                children: Vec::new(),
                tag: None,
            },
            WidgetNode {
                widget: Rc::new(RefCell::new(bar)),
                children: Vec::new(),
                tag: None,
            },
            WidgetNode {
                widget: Rc::new(RefCell::new(led)),
                children: Vec::new(),
                tag: None,
            },
            WidgetNode {
                widget: Rc::new(RefCell::new(buttons)),
                children: Vec::new(),
                tag: None,
            },
            WidgetNode {
                widget: Rc::new(RefCell::new(chart)),
                children: Vec::new(),
                tag: None,
            },
            WidgetNode {
                widget: Rc::new(RefCell::new(table)),
                children: Vec::new(),
                tag: None,
            },
        ],
        tag: None,
    }
}

// ---------------------------------------------------------------------------
// FFI exports for cdylib loading
// ---------------------------------------------------------------------------

/// FFI entry point: create a new `DemoApp` and return an owned raw pointer.
///
/// # Safety contract
///
/// The caller and callee must be compiled with the same `rustc` version and
/// target. The fat pointer (`*mut dyn Application`) is stable within a single
/// compiler+target pair.
#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn rlvgl_create_app() -> *mut dyn Application {
    Box::into_raw(create_app())
}

/// FFI entry point: destroy a previously created application.
///
/// # Safety
///
/// `ptr` must have been returned by [`rlvgl_create_app`] and must not be used
/// after this call.
#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub unsafe extern "C" fn rlvgl_destroy_app(ptr: *mut dyn Application) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin demo builders (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "qrcode", not(target_os = "none")))]
/// Build a widget demonstrating plugin features such as QR code generation.
pub fn build_plugin_demo(root_w: u32, root_h: u32) -> WidgetNode {
    let (pixels_vec, width, _) = qrcode::generate(b"https://github.com/SoftOboros/rlvgl").unwrap();
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
        tag: None,
    }
}

#[cfg(all(feature = "png", not(target_os = "none")))]
/// Build a widget displaying an embedded PNG at the given scale.
pub fn build_png_demo_scaled(scale: f32, root_w: u32, root_h: u32) -> WidgetNode {
    let (pixels_vec, width, height) =
        png::decode(PNG_LOGO).expect("failed to decode built-in PNG asset");

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
        tag: None,
    }
}

#[cfg(all(feature = "png", not(target_os = "none")))]
/// Build a PNG demo using the default scale of `0.5`.
pub fn build_png_demo(root_w: u32, root_h: u32) -> WidgetNode {
    build_png_demo_scaled(0.5, root_w, root_h)
}

#[cfg(feature = "gif")]
/// Build a widget displaying an embedded GIF at the given scale.
pub fn build_gif_demo_scaled(scale: f32, root_w: u32, root_h: u32) -> WidgetNode {
    let (frames, width, height) =
        gif::decode(GIF_LOGO).expect("failed to decode built-in GIF asset");
    let frame = &frames[0];
    let width = width as u32;
    let height = height as u32;
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
            let color = frame
                .pixels
                .get(idx)
                .copied()
                .unwrap_or(Color(0, 0, 0, 255));
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
        tag: None,
    }
}

#[cfg(feature = "gif")]
/// Build a GIF demo using the default scale of `0.5`.
pub fn build_gif_demo(root_w: u32, root_h: u32) -> WidgetNode {
    build_gif_demo_scaled(0.5, root_w, root_h)
}

#[cfg(all(feature = "jpeg", not(target_os = "none")))]
/// Build a widget displaying an embedded JPEG at the given scale.
pub fn build_jpeg_demo_scaled(scale: f32, root_w: u32, root_h: u32) -> WidgetNode {
    let (pixels_vec, width, height) =
        jpeg::decode(JPEG_LOGO).expect("failed to decode built-in JPEG asset");

    let width = width as u32;
    let height = height as u32;
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
        tag: None,
    }
}

#[cfg(all(feature = "jpeg", not(target_os = "none")))]
/// Build a JPEG demo using the default scale of `0.5`.
pub fn build_jpeg_demo(root_w: u32, root_h: u32) -> WidgetNode {
    build_jpeg_demo_scaled(0.5, root_w, root_h)
}

/// Flush any widgets queued during event callbacks into the root tree.
///
/// This is also available as part of the [`Application::after_event`] flow,
/// but exposed publicly for use by targets that manage their own widget tree.
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
