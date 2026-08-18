// SPDX-License-Identifier: MIT
//! Opt-in compositor evidence for the WLD-01 XDG and SHM path.

#![cfg(all(feature = "wayland", target_os = "linux"))]

use std::time::{Duration, Instant};

use rlvgl_core::widget::{Color, Rect};
use rlvgl_platform::{
    DisplayDriver,
    wayland::{
        WaylandConfig, WaylandIoReadiness, WaylandLifecycleEvent, WaylandSession,
        WaylandSessionState,
    },
};

fn dispatch_once(session: &mut WaylandSession) {
    let interest = session.prepare_io().expect("prepare Wayland I/O");
    session
        .dispatch_ready(WaylandIoReadiness {
            readable: interest.readable,
            writable: interest.writable,
        })
        .expect("dispatch Wayland I/O");
}

#[test]
#[ignore = "requires a running Wayland compositor; run explicitly in the Weston evidence job"]
fn maps_xdg_window_and_observes_frame_callback() {
    let mut config = WaylandConfig::new("rlvgl WLD-01 smoke", "com.softoboros.rlvgl.smoke", 64, 48)
        .expect("valid smoke configuration");
    config.registry_timeout = Duration::from_secs(2);
    let mut session = WaylandSession::connect(config).expect("connect to compositor");

    let configure_deadline = Instant::now() + Duration::from_secs(2);
    let token = loop {
        dispatch_once(&mut session);
        match session.poll_lifecycle() {
            Some(WaylandLifecycleEvent::Configure { token, .. }) => break token,
            Some(WaylandLifecycleEvent::ConnectionFailed(error)) => {
                panic!("connection failed before configure: {error}")
            }
            Some(WaylandLifecycleEvent::CloseRequested) => {
                panic!("compositor closed the smoke window before configure")
            }
            None if Instant::now() >= configure_deadline => {
                panic!("timed out waiting for initial configure")
            }
            None => std::thread::yield_now(),
        }
    };
    session
        .accept_configure(token)
        .expect("accept initial configure");

    let screen = session.display_mut().screen();
    let pixels = vec![Color(0x25, 0x6f, 0xa1, 0xff); (screen.width * screen.height) as usize];
    session.display_mut().flush(
        Rect {
            x: 0,
            y: 0,
            width: screen.width as i32,
            height: screen.height as i32,
        },
        &pixels,
    );
    session.display_mut().vsync();
    assert_eq!(session.display_mut().stats().submitted_frames, 1);

    let frame_deadline = Instant::now() + Duration::from_secs(2);
    while session.display_mut().stats().frame_callbacks == 0 {
        assert!(
            Instant::now() < frame_deadline,
            "timed out waiting for compositor frame callback"
        );
        dispatch_once(&mut session);
        std::thread::yield_now();
    }
    assert_eq!(session.state(), WaylandSessionState::Ready);
}
