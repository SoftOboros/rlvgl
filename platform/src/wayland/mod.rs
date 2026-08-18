// SPDX-License-Identifier: MIT
//! Native Linux Wayland session with XDG-shell lifecycle and bounded SHM presentation.
//!
//! The session owns every protocol object and all socket sequencing. The
//! [`crate::wayland::WaylandDisplay`] adapter implements the existing framebuffer-style
//! [`crate::display::DisplayDriver`] contract over a private Shadow Frame.

mod display;
mod model;
mod shm;

use alloc::{
    collections::VecDeque,
    rc::Rc,
    string::{String, ToString},
};
use core::{
    cell::RefCell,
    fmt,
    num::{NonZeroU8, NonZeroU32, NonZeroUsize},
};
use std::{
    net::Shutdown,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::net::UnixStream,
    },
    sync::mpsc,
    thread,
    time::Duration,
};

use display::Presenter;
pub use display::{WaylandDisplay, WaylandDisplayStats};
use model::{Geometry, ModelError, resolve_configure_geometry, slot_count_is_valid};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
    shm::{Shm, ShmHandler},
};
use wayland_client::{
    Connection, EventQueue, Proxy, QueueHandle,
    backend::{ReadEventsGuard, WaylandError as ClientWaylandError},
    globals::registry_queue_init,
    protocol::{wl_output, wl_surface},
};

/// Opaque identifier for one compositor configure proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigureToken(u64);

/// Hard limits applied to WLD-owned memory and queues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaylandLimits {
    /// Maximum combined bytes for the Shadow Frame plus active and retired SHM generations.
    pub max_allocation_bytes: NonZeroUsize,
    /// Maximum retained logical damage rectangles before promotion to full damage.
    pub max_damage_rects: NonZeroUsize,
    /// Maximum lifecycle notices; values below three are rejected.
    pub lifecycle_capacity: NonZeroUsize,
}

impl Default for WaylandLimits {
    fn default() -> Self {
        Self {
            max_allocation_bytes: NonZeroUsize::new(128 * 1024 * 1024)
                .expect("constant is nonzero"),
            max_damage_rects: NonZeroUsize::new(32).expect("constant is nonzero"),
            lifecycle_capacity: NonZeroUsize::new(8).expect("constant is nonzero"),
        }
    }
}

/// Policy relating compositor surface size to the rlvgl logical canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizePolicy {
    /// Adopt the compositor's latest valid surface size at a frame boundary.
    Adaptive,
    /// Keep an exact logical canvas and letterbox a larger configured surface.
    FixedCanvas {
        /// Fixed logical canvas width.
        width: NonZeroU32,
        /// Fixed logical canvas height.
        height: NonZeroU32,
    },
}

/// Construction parameters for one native Wayland session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaylandConfig {
    /// Human-readable XDG toplevel title.
    pub title: String,
    /// Stable reverse-DNS XDG application identifier.
    pub app_id: String,
    /// Requested initial surface size in logical coordinates.
    pub initial_size: (u32, u32),
    /// Geometry adoption policy.
    pub size_policy: SizePolicy,
    /// Whether to request XDG fullscreen state.
    pub fullscreen: bool,
    /// Exact presentation slot count; only two or three are admitted.
    pub buffer_count: NonZeroU8,
    /// Maximum time allowed for the compositor's initial registry roundtrip.
    pub registry_timeout: Duration,
    /// WLD-owned allocation and queue bounds.
    pub limits: WaylandLimits,
}

impl WaylandConfig {
    /// Construct an adaptive three-slot configuration with bounded defaults.
    pub fn new(
        title: impl Into<String>,
        app_id: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Result<Self, WaylandError> {
        let config = Self {
            title: title.into(),
            app_id: app_id.into(),
            initial_size: (width, height),
            size_policy: SizePolicy::Adaptive,
            fullscreen: false,
            buffer_count: NonZeroU8::new(3).expect("constant is nonzero"),
            registry_timeout: Duration::from_secs(5),
            limits: WaylandLimits::default(),
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), WaylandError> {
        if self.title.is_empty() {
            return Err(WaylandError::InvalidConfig("title must not be empty"));
        }
        if self.app_id.is_empty() {
            return Err(WaylandError::InvalidConfig("app_id must not be empty"));
        }
        if self.initial_size.0 == 0 || self.initial_size.1 == 0 {
            return Err(WaylandError::InvalidConfig(
                "initial width and height must be nonzero",
            ));
        }
        if !slot_count_is_valid(self.buffer_count.get()) {
            return Err(WaylandError::InvalidConfig(
                "buffer_count must be exactly two or three",
            ));
        }
        if self.registry_timeout.is_zero() {
            return Err(WaylandError::InvalidConfig(
                "registry_timeout must be nonzero",
            ));
        }
        if self.limits.lifecycle_capacity.get() < 3 {
            return Err(WaylandError::InvalidConfig(
                "lifecycle_capacity must reserve configure, close, and failure",
            ));
        }
        let logical_size = self.logical_size();
        let surface_size = match self.size_policy {
            SizePolicy::Adaptive => self.initial_size,
            SizePolicy::FixedCanvas { .. } => {
                if self.initial_size.0 < logical_size.0 || self.initial_size.1 < logical_size.1 {
                    return Err(WaylandError::SurfaceTooSmall {
                        surface: self.initial_size,
                        canvas: logical_size,
                    });
                }
                self.initial_size
            }
        };
        let geometry = Geometry::checked(logical_size, surface_size, 1)?;
        let pool_bytes = geometry
            .aligned_slot_bytes
            .checked_mul(usize::from(self.buffer_count.get()))
            .ok_or(WaylandError::GeometryOverflow)?;
        i32::try_from(pool_bytes).map_err(|_| WaylandError::GeometryOverflow)?;
        let required = geometry.steady_bytes(usize::from(self.buffer_count.get()))?;
        if required > self.limits.max_allocation_bytes.get() {
            return Err(WaylandError::GeometryTooLarge {
                required,
                limit: self.limits.max_allocation_bytes.get(),
            });
        }
        Ok(())
    }

    fn logical_size(&self) -> (u32, u32) {
        match self.size_policy {
            SizePolicy::Adaptive => self.initial_size,
            SizePolicy::FixedCanvas { width, height } => (width.get(), height.get()),
        }
    }
}

/// Lifecycle information kept separate from widget input events.
#[derive(Debug)]
pub enum WaylandLifecycleEvent {
    /// A compositor geometry proposal awaiting [`WaylandSession::accept_configure`].
    Configure {
        /// Opaque latest-configure identifier.
        token: ConfigureToken,
        /// Proposed surface width in logical Wayland coordinates.
        width: u32,
        /// Proposed surface height in logical Wayland coordinates.
        height: u32,
        /// Proposed positive integer buffer scale.
        scale: u32,
    },
    /// The compositor requested that the toplevel close.
    CloseRequested,
    /// The connection or presentation path reached a typed terminal failure.
    ConnectionFailed(WaylandError),
}

/// Current lifecycle state of a Wayland session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaylandSessionState {
    /// Registry setup and object construction are in progress.
    Connecting,
    /// The empty surface is committed and no configure has been accepted.
    AwaitingConfigure,
    /// Renderer and SHM geometry agree and presentation is admitted.
    Ready,
    /// The compositor requested close.
    Closing,
    /// A terminal connection or protocol failure occurred.
    Failed,
}

/// File-descriptor readiness currently requested by the session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaylandIoInterest {
    /// Poll the session descriptor for readability.
    pub readable: bool,
    /// Poll the session descriptor for writability.
    pub writable: bool,
}

/// Readiness returned by an application poller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WaylandIoReadiness {
    /// The session descriptor is readable.
    pub readable: bool,
    /// The session descriptor is writable.
    pub writable: bool,
}

/// Work completed by one nonblocking dispatch step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DispatchProgress {
    /// Number of queued protocol callbacks dispatched.
    pub dispatched_events: usize,
    /// Number of frames committed during this step.
    pub submitted_frames: u64,
}

/// Typed WLD construction, lifecycle, geometry, and presentation failures.
#[derive(Clone, Debug)]
pub enum WaylandError {
    /// No usable compositor connection was available.
    Connection(String),
    /// Registry enumeration or required-global binding failed.
    Registry(String),
    /// The compositor did not complete initial registry setup within the configured bound.
    RegistryTimeout(Duration),
    /// A required global was advertised below the WLD minimum version.
    ProtocolVersion {
        /// Wayland interface name.
        interface: &'static str,
        /// Minimum admitted version.
        required: u32,
        /// Version offered by the compositor.
        offered: u32,
    },
    /// Protocol dispatch or object construction failed.
    Protocol(String),
    /// Shared-memory pool or buffer construction failed.
    Shm(String),
    /// A public configuration value violated the ratified bounds.
    InvalidConfig(&'static str),
    /// Checked geometry or byte arithmetic overflowed.
    GeometryOverflow,
    /// The configured surface is smaller than a Fixed Canvas.
    SurfaceTooSmall {
        /// Configured surface size.
        surface: (u32, u32),
        /// Required logical canvas size.
        canvas: (u32, u32),
    },
    /// A steady-state generation exceeds the configured byte budget.
    GeometryTooLarge {
        /// Required bytes.
        required: usize,
        /// Configured byte limit.
        limit: usize,
    },
    /// Retired Busy slots temporarily prevent replacement allocation.
    AllocationDeferred {
        /// Temporary peak bytes.
        required: usize,
        /// Configured byte limit.
        limit: usize,
    },
    /// A superseded or already-consumed configure token was supplied.
    StaleConfigure(ConfigureToken),
    /// Drawing was attempted before geometry was accepted.
    NotConfigured,
    /// A dirty rectangle did not fit the accepted logical screen.
    InvalidDamage(rlvgl_core::widget::Rect),
    /// A pixel slice did not have its exact required length.
    PixelLength {
        /// Required element or byte count.
        expected: usize,
        /// Supplied element or byte count.
        actual: usize,
    },
    /// An internal exact-allocation assertion failed.
    AllocationInvariant {
        /// Expected value.
        expected: usize,
        /// Observed value.
        actual: usize,
    },
}

impl fmt::Display for WaylandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(message) => write!(formatter, "Wayland connection failed: {message}"),
            Self::Registry(message) => write!(formatter, "Wayland registry failed: {message}"),
            Self::RegistryTimeout(timeout) => {
                write!(formatter, "Wayland registry setup exceeded {timeout:?}")
            }
            Self::ProtocolVersion {
                interface,
                required,
                offered,
            } => write!(
                formatter,
                "{interface} version {offered} is below required version {required}"
            ),
            Self::Protocol(message) => write!(formatter, "Wayland protocol failed: {message}"),
            Self::Shm(message) => write!(formatter, "Wayland SHM failed: {message}"),
            Self::InvalidConfig(message) => write!(formatter, "invalid Wayland config: {message}"),
            Self::GeometryOverflow => formatter.write_str("Wayland geometry overflow"),
            Self::SurfaceTooSmall { surface, canvas } => write!(
                formatter,
                "configured surface {}x{} is smaller than canvas {}x{}",
                surface.0, surface.1, canvas.0, canvas.1
            ),
            Self::GeometryTooLarge { required, limit } => write!(
                formatter,
                "Wayland geometry requires {required} bytes, limit is {limit}"
            ),
            Self::AllocationDeferred { required, limit } => write!(
                formatter,
                "Wayland resize peak requires {required} bytes, limit is {limit}; retry after release"
            ),
            Self::StaleConfigure(token) => write!(formatter, "stale configure token {token:?}"),
            Self::NotConfigured => formatter.write_str("Wayland display is not configured"),
            Self::InvalidDamage(rect) => write!(formatter, "invalid Wayland damage {rect:?}"),
            Self::PixelLength { expected, actual } => write!(
                formatter,
                "pixel length mismatch: expected {expected}, got {actual}"
            ),
            Self::AllocationInvariant { expected, actual } => write!(
                formatter,
                "SHM allocation invariant: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for WaylandError {}

impl From<ModelError> for WaylandError {
    fn from(error: ModelError) -> Self {
        match error {
            ModelError::InvalidConfig(message) => Self::InvalidConfig(message),
            ModelError::GeometryOverflow => Self::GeometryOverflow,
            ModelError::SurfaceTooSmall { surface, canvas } => {
                Self::SurfaceTooSmall { surface, canvas }
            }
            ModelError::PixelLength { expected, actual } => Self::PixelLength { expected, actual },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ConfigureSpec {
    token: ConfigureToken,
    surface_size: (u32, u32),
    logical_size: (u32, u32),
    scale: u32,
}

/// Owner of one Wayland connection, event queue, toplevel, and display adapter.
pub struct WaylandSession {
    connection: Connection,
    event_queue: EventQueue<ProtocolState>,
    protocol: ProtocolState,
    display: WaylandDisplay,
    prepared_read: Option<ReadEventsGuard>,
    needs_write: bool,
}

impl fmt::Debug for WaylandSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WaylandSession")
            .field("state", &self.protocol.session_state)
            .field("needs_write", &self.needs_write)
            .finish_non_exhaustive()
    }
}

impl WaylandSession {
    /// Connect and construct one unmapped XDG toplevel without waiting for its configure.
    pub fn connect(config: WaylandConfig) -> Result<Self, WaylandError> {
        config.validate()?;
        let connection = Connection::connect_to_env()
            .map_err(|error| WaylandError::Connection(error.to_string()))?;
        let (globals, event_queue) = bounded_registry_init(&connection, config.registry_timeout)?;
        let qh = event_queue.handle();
        let compositor = CompositorState::bind(&globals, &qh)
            .map_err(|error| WaylandError::Registry(error.to_string()))?;
        let offered = compositor.wl_compositor().version();
        if offered < 4 {
            return Err(WaylandError::ProtocolVersion {
                interface: "wl_compositor",
                required: 4,
                offered,
            });
        }
        let xdg_shell = XdgShell::bind(&globals, &qh)
            .map_err(|error| WaylandError::Registry(error.to_string()))?;
        let shm =
            Shm::bind(&globals, &qh).map_err(|error| WaylandError::Registry(error.to_string()))?;
        // wl_shm version 1 guarantees ARGB8888 and XRGB8888 even when the
        // compositor omits format events for those two baseline formats.
        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);

        let surface = compositor.create_surface(&qh);
        let window = xdg_shell.create_window(surface, WindowDecorations::None, &qh);
        window.set_title(config.title.clone());
        window.set_app_id(config.app_id.clone());
        if let SizePolicy::FixedCanvas { width, height } = config.size_policy {
            let size = (width.get(), height.get());
            window.set_min_size(Some(size));
            window.set_max_size(Some(size));
        }
        if config.fullscreen {
            window.set_fullscreen(None);
        }

        let presenter = Rc::new(RefCell::new(Presenter::new(
            &config,
            qh.clone(),
            window.clone(),
            compositor,
        )));
        let display = WaylandDisplay {
            presenter: Rc::clone(&presenter),
        };
        let protocol = ProtocolState {
            registry_state,
            output_state,
            shm,
            window,
            presenter,
            config,
            lifecycle: VecDeque::new(),
            session_state: WaylandSessionState::Connecting,
            next_token: 1,
            latest_configure: None,
            last_surface_size: None,
            scale: 1,
        };

        protocol.window.commit();
        let mut protocol = protocol;
        protocol.session_state = WaylandSessionState::AwaitingConfigure;
        let needs_write = match connection.flush() {
            Ok(()) => false,
            Err(ClientWaylandError::Io(error))
                if error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                true
            }
            Err(error) => return Err(WaylandError::Protocol(error.to_string())),
        };

        Ok(Self {
            connection,
            event_queue,
            protocol,
            display,
            prepared_read: None,
            needs_write,
        })
    }

    /// Return the current lifecycle state.
    pub fn state(&self) -> WaylandSessionState {
        self.protocol.session_state
    }

    /// Prepare a nonblocking poll cycle while retaining socket sequencing.
    pub fn prepare_io(&mut self) -> Result<WaylandIoInterest, WaylandError> {
        let result = self.prepare_io_inner();
        if let Err(error) = &result {
            self.protocol.fail(error.clone());
        }
        result
    }

    fn prepare_io_inner(&mut self) -> Result<WaylandIoInterest, WaylandError> {
        if self.prepared_read.is_some() {
            return Ok(self.io_interest());
        }
        self.event_queue
            .dispatch_pending(&mut self.protocol)
            .map_err(|error| WaylandError::Protocol(error.to_string()))?;
        self.progress_presenter();
        self.flush_outbound()?;
        self.prepared_read = self.event_queue.prepare_read();
        if self.prepared_read.is_none() {
            self.event_queue
                .dispatch_pending(&mut self.protocol)
                .map_err(|error| WaylandError::Protocol(error.to_string()))?;
            self.progress_presenter();
            self.prepared_read = self.event_queue.prepare_read();
        }
        Ok(self.io_interest())
    }

    /// Return the descriptor interests prepared by [`Self::prepare_io`].
    pub fn io_interest(&self) -> WaylandIoInterest {
        WaylandIoInterest {
            readable: self.prepared_read.is_some(),
            writable: self.needs_write,
        }
    }

    /// Consume caller-reported readiness and dispatch without blocking.
    pub fn dispatch_ready(
        &mut self,
        readiness: WaylandIoReadiness,
    ) -> Result<DispatchProgress, WaylandError> {
        let result = self.dispatch_ready_inner(readiness);
        if let Err(error) = &result {
            self.protocol.fail(error.clone());
        }
        result
    }

    fn dispatch_ready_inner(
        &mut self,
        readiness: WaylandIoReadiness,
    ) -> Result<DispatchProgress, WaylandError> {
        if self.prepared_read.is_none() {
            self.prepare_io_inner()?;
        }
        if let Some(guard) = self.prepared_read.take()
            && readiness.readable
        {
            match guard.read() {
                Ok(_) => {}
                Err(ClientWaylandError::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(WaylandError::Protocol(error.to_string())),
            }
        }
        if readiness.writable || !self.needs_write {
            self.flush_outbound()?;
        }
        let before = self.display.stats().submitted_frames;
        let dispatched_events = self
            .event_queue
            .dispatch_pending(&mut self.protocol)
            .map_err(|error| WaylandError::Protocol(error.to_string()))?;
        self.progress_presenter();
        self.flush_outbound()?;
        let submitted_frames = self.display.stats().submitted_frames.saturating_sub(before);
        Ok(DispatchProgress {
            dispatched_events,
            submitted_frames,
        })
    }

    /// Adopt the latest configure after renderer geometry is ready.
    ///
    /// [`WaylandError::AllocationDeferred`] is retryable after further
    /// dispatch releases retired buffers; it does not consume `token`.
    pub fn accept_configure(&mut self, token: ConfigureToken) -> Result<(), WaylandError> {
        let spec = self
            .protocol
            .latest_configure
            .filter(|spec| spec.token == token)
            .ok_or(WaylandError::StaleConfigure(token))?;
        let geometry = Geometry::checked(spec.logical_size, spec.surface_size, spec.scale)?;
        self.protocol
            .presenter
            .borrow_mut()
            .accept_geometry(geometry, &self.protocol.shm)?;
        self.protocol.latest_configure = None;
        self.protocol.session_state = WaylandSessionState::Ready;
        if let Err(error) = self.flush_outbound() {
            self.protocol.fail(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// Pop the oldest lifecycle notice, coalescing configure proposals by token.
    pub fn poll_lifecycle(&mut self) -> Option<WaylandLifecycleEvent> {
        self.progress_presenter();
        self.protocol.lifecycle.pop_front()
    }

    /// Borrow the session-owned display adapter.
    pub fn display_mut(&mut self) -> &mut WaylandDisplay {
        &mut self.display
    }

    /// Borrow the Wayland socket descriptor for an external poller.
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.connection.as_fd()
    }

    fn flush_outbound(&mut self) -> Result<(), WaylandError> {
        match self.connection.flush() {
            Ok(()) => {
                self.needs_write = false;
                Ok(())
            }
            Err(ClientWaylandError::Io(error))
                if error.kind() == std::io::ErrorKind::WouldBlock =>
            {
                self.needs_write = true;
                Ok(())
            }
            Err(error) => Err(WaylandError::Protocol(error.to_string())),
        }
    }

    fn progress_presenter(&mut self) {
        let error = {
            let mut presenter = self.protocol.presenter.borrow_mut();
            if let Err(error) = presenter.progress_after_dispatch() {
                presenter.record_error(error);
            }
            presenter.take_error()
        };
        if let Some(error) = error {
            self.protocol.fail(error);
        }
    }
}

fn bounded_registry_init(
    connection: &Connection,
    timeout: Duration,
) -> Result<
    (
        wayland_client::globals::GlobalList,
        EventQueue<ProtocolState>,
    ),
    WaylandError,
> {
    let shutdown_socket = UnixStream::from(
        connection
            .as_fd()
            .try_clone_to_owned()
            .map_err(|error| WaylandError::Connection(error.to_string()))?,
    );
    let worker_connection = connection.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = thread::Builder::new()
        .name("rlvgl-wayland-registry".into())
        .spawn(move || {
            let result = registry_queue_init::<ProtocolState>(&worker_connection);
            let _ = sender.send(result);
        })
        .map_err(|error| WaylandError::Connection(error.to_string()))?;

    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            worker
                .join()
                .map_err(|_| WaylandError::Connection("registry worker panicked".into()))?;
            result.map_err(|error| WaylandError::Registry(error.to_string()))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = shutdown_socket.shutdown(Shutdown::Both);
            drop(worker);
            Err(WaylandError::RegistryTimeout(timeout))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = worker.join();
            Err(WaylandError::Connection(
                "registry worker exited without a result".into(),
            ))
        }
    }
}

pub(crate) struct ProtocolState {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    window: Window,
    presenter: Rc<RefCell<Presenter>>,
    config: WaylandConfig,
    lifecycle: VecDeque<WaylandLifecycleEvent>,
    session_state: WaylandSessionState,
    next_token: u64,
    latest_configure: Option<ConfigureSpec>,
    last_surface_size: Option<(u32, u32)>,
    scale: u32,
}

impl ProtocolState {
    fn publish_configure(&mut self, suggested: (Option<u32>, Option<u32>)) {
        if matches!(
            self.session_state,
            WaylandSessionState::Closing | WaylandSessionState::Failed
        ) {
            return;
        }
        let fallback = self.last_surface_size.unwrap_or(self.config.initial_size);
        let (logical_size, surface_size) = resolve_configure_geometry(
            matches!(self.config.size_policy, SizePolicy::Adaptive),
            self.config.logical_size(),
            suggested,
            fallback,
        );
        if let Err(error) = Geometry::checked(logical_size, surface_size, self.scale) {
            self.fail(error.into());
            return;
        }
        let token = ConfigureToken(self.next_token);
        let Some(next_token) = self.next_token.checked_add(1) else {
            self.fail(WaylandError::Protocol("configure token exhausted".into()));
            return;
        };
        self.next_token = next_token;
        self.last_surface_size = Some(surface_size);
        self.latest_configure = Some(ConfigureSpec {
            token,
            surface_size,
            logical_size,
            scale: self.scale,
        });
        self.lifecycle
            .retain(|event| !matches!(event, WaylandLifecycleEvent::Configure { .. }));
        self.ensure_lifecycle_room();
        self.lifecycle.push_back(WaylandLifecycleEvent::Configure {
            token,
            width: surface_size.0,
            height: surface_size.1,
            scale: self.scale,
        });
    }

    fn request_close(&mut self) {
        if matches!(
            self.session_state,
            WaylandSessionState::Closing | WaylandSessionState::Failed
        ) {
            return;
        }
        self.session_state = WaylandSessionState::Closing;
        self.presenter.borrow_mut().stop();
        self.latest_configure = None;
        self.lifecycle
            .retain(|event| !matches!(event, WaylandLifecycleEvent::Configure { .. }));
        self.ensure_lifecycle_room();
        self.lifecycle
            .push_back(WaylandLifecycleEvent::CloseRequested);
    }

    fn fail(&mut self, error: WaylandError) {
        if self.session_state == WaylandSessionState::Failed {
            return;
        }
        self.session_state = WaylandSessionState::Failed;
        self.presenter.borrow_mut().stop();
        self.latest_configure = None;
        self.lifecycle
            .retain(|event| !matches!(event, WaylandLifecycleEvent::Configure { .. }));
        self.ensure_lifecycle_room();
        self.lifecycle
            .push_back(WaylandLifecycleEvent::ConnectionFailed(error));
    }

    fn ensure_lifecycle_room(&mut self) {
        let capacity = self.config.limits.lifecycle_capacity.get();
        if self.lifecycle.len() < capacity {
            return;
        }
        if let Some(index) = self
            .lifecycle
            .iter()
            .position(|event| matches!(event, WaylandLifecycleEvent::Configure { .. }))
        {
            self.lifecycle.remove(index);
        }
    }
}

impl CompositorHandler for ProtocolState {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let Ok(scale) = u32::try_from(new_factor) else {
            self.fail(WaylandError::Protocol(
                "compositor supplied non-positive buffer scale".into(),
            ));
            return;
        };
        if scale == 0 || scale == self.scale {
            return;
        }
        self.scale = scale;
        self.publish_configure((None, None));
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Physical output rotation remains compositor-owned; WLD always uses Normal.
    }

    fn frame(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let result = self.presenter.borrow_mut().frame_done();
        if let Err(error) = result {
            self.fail(error);
        }
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for ProtocolState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl WindowHandler for ProtocolState {
    fn request_close(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
    ) {
        self.request_close();
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        self.publish_configure((
            configure.new_size.0.map(NonZeroU32::get),
            configure.new_size.1.map(NonZeroU32::get),
        ));
    }
}

impl ShmHandler for ProtocolState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(ProtocolState);

impl ProvidesRegistryState for ProtocolState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_dispatch2!(ProtocolState);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_bootstrap_returns_at_configured_timeout() {
        let (client, _server) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(client).unwrap();
        let timeout = Duration::from_millis(10);
        let started = std::time::Instant::now();
        assert!(matches!(
            bounded_registry_init(&connection, timeout),
            Err(WaylandError::RegistryTimeout(actual)) if actual == timeout
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn config_rejects_slot_counts_outside_two_or_three() {
        let mut config = WaylandConfig::new("test", "com.example.test", 800, 480).unwrap();
        config.buffer_count = NonZeroU8::new(4).unwrap();
        assert!(matches!(
            config.validate(),
            Err(WaylandError::InvalidConfig(_))
        ));
    }

    #[test]
    fn config_rejects_zero_registry_timeout() {
        let mut config = WaylandConfig::new("test", "com.example.test", 800, 480).unwrap();
        config.registry_timeout = Duration::ZERO;
        assert!(matches!(
            config.validate(),
            Err(WaylandError::InvalidConfig(_))
        ));
    }

    #[test]
    fn config_budget_includes_shadow_and_all_slots() {
        let mut config = WaylandConfig::new("test", "com.example.test", 800, 480).unwrap();
        config.limits.max_allocation_bytes = NonZeroUsize::new(1).unwrap();
        assert!(matches!(
            config.validate(),
            Err(WaylandError::GeometryTooLarge { .. })
        ));
    }

    #[test]
    fn fixed_canvas_rejects_smaller_initial_surface() {
        let mut config = WaylandConfig::new("test", "com.example.test", 640, 360).unwrap();
        config.size_policy = SizePolicy::FixedCanvas {
            width: NonZeroU32::new(800).unwrap(),
            height: NonZeroU32::new(480).unwrap(),
        };
        assert!(matches!(
            config.validate(),
            Err(WaylandError::SurfaceTooSmall { .. })
        ));
    }
}
