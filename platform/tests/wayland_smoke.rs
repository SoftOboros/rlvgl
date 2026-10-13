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

fn smoke_config(title: &str, app_id: &str) -> WaylandConfig {
    let mut config = WaylandConfig::new(title, app_id, 64, 48).expect("valid smoke configuration");
    config.registry_timeout = Duration::from_secs(2);
    config
}

fn await_initial_configure(session: &mut WaylandSession) {
    let configure_deadline = Instant::now() + Duration::from_secs(2);
    let token = loop {
        dispatch_once(session);
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
}

fn present_one_frame(session: &mut WaylandSession) {
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
        dispatch_once(session);
        std::thread::yield_now();
    }
    assert_eq!(session.state(), WaylandSessionState::Ready);
}

#[test]
#[ignore = "requires a running Wayland compositor; run explicitly in the Weston evidence job"]
fn maps_xdg_window_and_observes_frame_callback() {
    let config = smoke_config("rlvgl WLD-01 smoke", "com.softoboros.rlvgl.smoke");
    let mut session = WaylandSession::connect(config).expect("connect to compositor");
    await_initial_configure(&mut session);
    present_one_frame(&mut session);
}

#[test]
#[ignore = "requires a running Wayland compositor; run explicitly in the Weston evidence job"]
fn canceled_prepared_read_remains_nonblocking_and_dispatchable() {
    let config = smoke_config(
        "rlvgl WLD-01 readiness smoke",
        "com.softoboros.rlvgl.readiness-smoke",
    );
    let mut session = WaylandSession::connect(config).expect("connect to compositor");

    let interest = session.prepare_io().expect("prepare first read");
    assert!(
        interest.readable,
        "a prepared read must request readability"
    );
    let started = Instant::now();
    session
        .dispatch_ready(WaylandIoReadiness {
            readable: false,
            writable: false,
        })
        .expect("cancel prepared read without socket readiness");
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "canceling a prepared read must not block"
    );

    let next = session
        .prepare_io()
        .expect("prepare read after cancellation");
    assert!(
        next.readable,
        "read interest must recover after cancellation"
    );
    await_initial_configure(&mut session);
    present_one_frame(&mut session);
}

#[test]
#[ignore = "requires a running Wayland compositor; run explicitly in the Weston evidence job"]
fn repeated_connect_present_and_drop_keeps_compositor_usable() {
    for attempt in 0..2 {
        let config = smoke_config(
            &format!("rlvgl WLD-01 teardown smoke {attempt}"),
            "com.softoboros.rlvgl.teardown-smoke",
        );
        let mut session = WaylandSession::connect(config).expect("connect to compositor");
        await_initial_configure(&mut session);
        present_one_frame(&mut session);
        drop(session);
    }
}
