// SPDX-License-Identifier: MIT
//! Controlled protocol fixtures for WLD-01 configure admission and terminal failure.
//! These fixtures supplement, rather than replace, live compositor evidence.

use super::*;
use crate::display::DisplayDriver;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use wayland_protocols::xdg::shell::server::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New,
    backend::ClientData,
    protocol::{wl_compositor, wl_region, wl_shm, wl_surface},
};

#[derive(Default)]
struct Probe {
    stop: AtomicBool,
    buffer_attaches: AtomicUsize,
    pool_requests: AtomicUsize,
}

struct ClientState;
impl ClientData for ClientState {}

struct FixtureState {
    probe: Arc<Probe>,
    surface: Option<xdg_surface::XdgSurface>,
    toplevel: Option<xdg_toplevel::XdgToplevel>,
    configured: bool,
}

macro_rules! bind_global {
    ($resource:ty) => {
        impl GlobalDispatch<$resource, ()> for FixtureState {
            fn bind(
                _state: &mut Self,
                _handle: &DisplayHandle,
                _client: &Client,
                resource: New<$resource>,
                _data: &(),
                init: &mut DataInit<'_, Self>,
            ) {
                init.init(resource, ());
            }
        }
    };
}
bind_global!(wl_compositor::WlCompositor);
bind_global!(wl_shm::WlShm);
bind_global!(xdg_wm_base::XdgWmBase);

impl Dispatch<wl_compositor::WlCompositor, ()> for FixtureState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _data: &(),
        _handle: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                init.init(id, ());
            }
            wl_compositor::Request::CreateRegion { id } => {
                init.init(id, ());
            }
            other => panic!("unexpected compositor request: {other:?}"),
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for FixtureState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &wl_shm::WlShm,
        request: wl_shm::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { .. } = request {
            state.probe.pool_requests.fetch_add(1, Ordering::AcqRel);
            panic!("SHM allocation before renderer accepted configure");
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for FixtureState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &wl_surface::WlSurface,
        request: wl_surface::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_surface::Request::Attach {
                buffer: Some(_), ..
            } => {
                state.probe.buffer_attaches.fetch_add(1, Ordering::AcqRel);
            }
            wl_surface::Request::Commit if !state.configured => {
                state
                    .toplevel
                    .as_ref()
                    .expect("XDG toplevel before initial commit")
                    .configure(64, 48, alloc::vec::Vec::new());
                state
                    .surface
                    .as_ref()
                    .expect("XDG surface before initial commit")
                    .configure(1);
                state.configured = true;
            }
            wl_surface::Request::Frame { .. } => {
                panic!("frame callback requested before configure acceptance");
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_region::WlRegion, ()> for FixtureState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_region::WlRegion,
        _request: wl_region::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _init: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for FixtureState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &xdg_wm_base::XdgWmBase,
        request: xdg_wm_base::Request,
        _data: &(),
        _handle: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let xdg_wm_base::Request::GetXdgSurface { id, .. } = request {
            state.surface = Some(init.init(id, ()));
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for FixtureState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &xdg_surface::XdgSurface,
        request: xdg_surface::Request,
        _data: &(),
        _handle: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let xdg_surface::Request::GetToplevel { id } = request {
            state.toplevel = Some(init.init(id, ()));
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for FixtureState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &xdg_toplevel::XdgToplevel,
        _request: xdg_toplevel::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _init: &mut DataInit<'_, Self>,
    ) {
    }
}

struct Fixture {
    probe: Arc<Probe>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Fixture {
    fn connect() -> (Self, WaylandSession) {
        let (client, server) = UnixStream::pair().expect("fixture socket pair");
        let connection = Connection::from_socket(client).expect("fixture connection");
        let mut display = Display::<FixtureState>::new().expect("fixture display");
        let mut handle = display.handle();
        handle.create_global::<FixtureState, wl_compositor::WlCompositor, _>(4, ());
        handle.create_global::<FixtureState, wl_shm::WlShm, _>(1, ());
        handle.create_global::<FixtureState, xdg_wm_base::XdgWmBase, _>(1, ());
        handle
            .insert_client(server, Arc::new(ClientState))
            .expect("fixture client");
        let probe = Arc::new(Probe::default());
        let worker_probe = Arc::clone(&probe);
        let worker = thread::spawn(move || {
            let mut state = FixtureState {
                probe: Arc::clone(&worker_probe),
                surface: None,
                toplevel: None,
                configured: false,
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !worker_probe.stop.load(Ordering::Acquire) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "fixture exceeded bounded deadline"
                );
                display
                    .dispatch_clients(&mut state)
                    .expect("fixture dispatch");
                display.flush_clients().expect("fixture flush");
                thread::yield_now();
            }
        });
        let fixture = Self {
            probe,
            worker: Some(worker),
        };
        let mut config =
            WaylandConfig::new("admission fixture", "com.softoboros.rlvgl.fixture", 64, 48)
                .expect("fixture config");
        config.registry_timeout = Duration::from_millis(500);
        let session = WaylandSession::connect_with_connection(config, connection)
            .expect("connect to controlled fixture");
        (fixture, session)
    }

    fn disconnect(&mut self) {
        self.probe.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            // Retain the primary assertion if fixture cleanup runs during unwinding.
            if !thread::panicking() {
                result.expect("fixture worker");
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn await_configure(session: &mut WaylandSession) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let interest = session.prepare_io().expect("prepare configure I/O");
        session
            .dispatch_ready(WaylandIoReadiness {
                readable: interest.readable,
                writable: interest.writable,
            })
            .expect("dispatch configure I/O");
        if let Some(WaylandLifecycleEvent::Configure { width, height, .. }) =
            session.poll_lifecycle()
        {
            assert_eq!((width, height), (64, 48));
            return;
        }
        assert!(std::time::Instant::now() < deadline, "configure deadline");
        thread::yield_now();
    }
}

#[test]
fn configure_notice_without_renderer_acceptance_never_allocates_or_attaches() {
    let (fixture, mut session) = Fixture::connect();
    assert_eq!(session.state(), WaylandSessionState::AwaitingConfigure);
    session.display_mut().vsync();
    await_configure(&mut session);
    // A protocol configure notice is distinct from renderer acceptance of its token.
    session.display_mut().vsync();
    session
        .connection
        .roundtrip()
        .expect("drain queued fixture requests");
    assert_eq!(session.state(), WaylandSessionState::AwaitingConfigure);
    assert_eq!(session.display_mut().stats().submitted_frames, 0);
    assert_eq!(session.display_mut().stats().allocated_bytes, 0);
    assert_eq!(fixture.probe.buffer_attaches.load(Ordering::Acquire), 0);
    assert_eq!(fixture.probe.pool_requests.load(Ordering::Acquire), 0);
}

#[test]
fn disconnect_before_configure_acceptance_fails_without_presentation_revival() {
    let (mut fixture, mut session) = Fixture::connect();
    await_configure(&mut session);
    fixture.disconnect();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let result = session.prepare_io().and_then(|interest| {
            session.dispatch_ready(WaylandIoReadiness {
                readable: interest.readable,
                writable: interest.writable,
            })
        });
        if result.is_err() {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "disconnect deadline");
        thread::yield_now();
    }
    assert_eq!(session.state(), WaylandSessionState::Failed);
    assert!(matches!(
        session.poll_lifecycle(),
        Some(WaylandLifecycleEvent::ConnectionFailed(_))
    ));
    assert!(session.protocol.latest_configure.is_none());
    session.display_mut().vsync();
    assert_eq!(session.state(), WaylandSessionState::Failed);
    assert_eq!(session.display_mut().stats().submitted_frames, 0);
}
