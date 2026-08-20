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
    mem::ManuallyDrop,
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

fn configure_spec_for_token(
    latest_configure: Option<ConfigureSpec>,
    token: ConfigureToken,
) -> Result<ConfigureSpec, WaylandError> {
    latest_configure
        .filter(|spec| spec.token == token)
        .ok_or(WaylandError::StaleConfigure(token))
}

fn ensure_lifecycle_room(lifecycle: &mut VecDeque<WaylandLifecycleEvent>, capacity: usize) {
    if lifecycle.len() < capacity {
        return;
    }
    if let Some(index) = lifecycle
        .iter()
        .position(|event| matches!(event, WaylandLifecycleEvent::Configure { .. }))
    {
        lifecycle.remove(index);
    }
}

fn enqueue_configure_lifecycle(
    lifecycle: &mut VecDeque<WaylandLifecycleEvent>,
    capacity: usize,
    event: WaylandLifecycleEvent,
) {
    debug_assert!(matches!(event, WaylandLifecycleEvent::Configure { .. }));
    lifecycle.retain(|event| !matches!(event, WaylandLifecycleEvent::Configure { .. }));
    ensure_lifecycle_room(lifecycle, capacity);
    lifecycle.push_back(event);
}

fn enqueue_terminal_lifecycle(
    lifecycle: &mut VecDeque<WaylandLifecycleEvent>,
    capacity: usize,
    event: WaylandLifecycleEvent,
) {
    debug_assert!(matches!(
        event,
        WaylandLifecycleEvent::CloseRequested | WaylandLifecycleEvent::ConnectionFailed(_)
    ));
    lifecycle.retain(|event| !matches!(event, WaylandLifecycleEvent::Configure { .. }));
    ensure_lifecycle_room(lifecycle, capacity);
    lifecycle.push_back(event);
}

fn flush_connection(connection: &Connection, needs_write: &mut bool) -> Result<(), WaylandError> {
    match connection.flush() {
        Ok(()) => {
            *needs_write = false;
            Ok(())
        }
        Err(ClientWaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
            *needs_write = true;
            Ok(())
        }
        Err(error) => Err(WaylandError::Protocol(error.to_string())),
    }
}

/// Owner of one Wayland connection, event queue, toplevel, and display adapter.
pub struct WaylandSession {
    connection: Connection,
    event_queue: EventQueue<ProtocolState>,
    protocol: ManuallyDrop<ProtocolState>,
    display: ManuallyDrop<WaylandDisplay>,
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
        Self::connect_with_connection(config, connection)
    }

    fn connect_with_connection(
        config: WaylandConfig,
        connection: Connection,
    ) -> Result<Self, WaylandError> {
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
        let mut needs_write = false;
        flush_connection(&connection, &mut needs_write)?;

        Ok(Self {
            connection,
            event_queue,
            protocol: ManuallyDrop::new(protocol),
            display: ManuallyDrop::new(display),
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
        let spec = configure_spec_for_token(self.protocol.latest_configure, token)?;
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
        flush_connection(&self.connection, &mut self.needs_write)
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

impl Drop for WaylandSession {
    fn drop(&mut self) {
        self.prepared_read.take();
        // The display and protocol state share the Presenter, and each owns a
        // Window clone. Drop both owners while the connection is still alive
        // so SCTK can enqueue its protocol-object destructors, including the
        // required XDG role-before-surface order, then make a best-effort
        // final flush.
        // SAFETY: these fields are ManuallyDrop and are dropped exactly once
        // here; no field access follows except to the independent connection.
        unsafe {
            ManuallyDrop::drop(&mut self.display);
            ManuallyDrop::drop(&mut self.protocol);
        }
        let _ = self.connection.flush();
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
        enqueue_configure_lifecycle(
            &mut self.lifecycle,
            self.config.limits.lifecycle_capacity.get(),
            WaylandLifecycleEvent::Configure {
                token,
                width: surface_size.0,
                height: surface_size.1,
                scale: self.scale,
            },
        );
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
        enqueue_terminal_lifecycle(
            &mut self.lifecycle,
            self.config.limits.lifecycle_capacity.get(),
            WaylandLifecycleEvent::CloseRequested,
        );
    }

    fn fail(&mut self, error: WaylandError) {
        if self.session_state == WaylandSessionState::Failed {
            return;
        }
        self.session_state = WaylandSessionState::Failed;
        self.presenter.borrow_mut().stop();
        self.latest_configure = None;
        enqueue_terminal_lifecycle(
            &mut self.lifecycle,
            self.config.limits.lifecycle_capacity.get(),
            WaylandLifecycleEvent::ConnectionFailed(error),
        );
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
    use crate::display::DisplayDriver;
    use rlvgl_core::widget::{Color, Rect};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::{eprintln, format, fs, io::Read, path::Path, vec};
    use wayland_client::protocol::wl_callback;
    use wayland_protocols::xdg::shell::server::xdg_wm_base;
    use wayland_server::{
        Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New,
        backend::{ClientData, ClientId, DisconnectReason},
        protocol::wl_compositor,
    };

    #[derive(Default)]
    struct FixtureClientData {
        disconnected: AtomicBool,
    }

    impl ClientData for FixtureClientData {
        fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
            self.disconnected.store(true, Ordering::Release);
        }
    }

    struct FixtureState;

    struct BackpressureState;

    impl wayland_client::Dispatch<wl_callback::WlCallback, ()> for BackpressureState {
        fn event(
            _state: &mut Self,
            _proxy: &wl_callback::WlCallback,
            _event: wl_callback::Event,
            _data: &(),
            _connection: &Connection,
            _qh: &QueueHandle<Self>,
        ) {
        }
    }

    impl GlobalDispatch<wl_compositor::WlCompositor, ()> for FixtureState {
        fn bind(
            _state: &mut Self,
            _handle: &DisplayHandle,
            _client: &Client,
            resource: New<wl_compositor::WlCompositor>,
            _global_data: &(),
            data_init: &mut DataInit<'_, Self>,
        ) {
            data_init.init(resource, ());
        }
    }

    impl Dispatch<wl_compositor::WlCompositor, ()> for FixtureState {
        fn request(
            _state: &mut Self,
            _client: &Client,
            _resource: &wl_compositor::WlCompositor,
            request: wl_compositor::Request,
            _data: &(),
            _handle: &DisplayHandle,
            _data_init: &mut DataInit<'_, Self>,
        ) {
            panic!("unexpected compositor request in constructor fixture: {request:?}");
        }
    }

    impl GlobalDispatch<xdg_wm_base::XdgWmBase, ()> for FixtureState {
        fn bind(
            _state: &mut Self,
            _handle: &DisplayHandle,
            _client: &Client,
            resource: New<xdg_wm_base::XdgWmBase>,
            _global_data: &(),
            data_init: &mut DataInit<'_, Self>,
        ) {
            data_init.init(resource, ());
        }
    }

    impl Dispatch<xdg_wm_base::XdgWmBase, ()> for FixtureState {
        fn request(
            _state: &mut Self,
            _client: &Client,
            _resource: &xdg_wm_base::XdgWmBase,
            request: xdg_wm_base::Request,
            _data: &(),
            _handle: &DisplayHandle,
            _data_init: &mut DataInit<'_, Self>,
        ) {
            panic!("unexpected XDG shell request in constructor fixture: {request:?}");
        }
    }

    fn fixture_connection(
        compositor_version: Option<u32>,
        advertise_xdg_shell: bool,
    ) -> (Connection, thread::JoinHandle<()>) {
        let (client_socket, server_socket) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(client_socket).unwrap();
        let mut display = Display::<FixtureState>::new().unwrap();
        if let Some(version) = compositor_version {
            display
                .handle()
                .create_global::<FixtureState, wl_compositor::WlCompositor, _>(version, ());
        }
        if advertise_xdg_shell {
            display
                .handle()
                .create_global::<FixtureState, xdg_wm_base::XdgWmBase, _>(1, ());
        }
        let client_data = Arc::new(FixtureClientData::default());
        display
            .handle()
            .insert_client(server_socket, client_data.clone())
            .unwrap();
        let worker = thread::spawn(move || {
            let mut state = FixtureState;
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            while !client_data.disconnected.load(Ordering::Acquire)
                && std::time::Instant::now() < deadline
            {
                display.dispatch_clients(&mut state).unwrap();
                display.flush_clients().unwrap();
                thread::yield_now();
            }
            assert!(
                client_data.disconnected.load(Ordering::Acquire),
                "fixture client did not disconnect before deadline"
            );
        });
        (connection, worker)
    }

    fn fixture_config() -> WaylandConfig {
        let mut config = WaylandConfig::new("fixture", "com.softoboros.rlvgl.fixture", 64, 48)
            .expect("valid fixture configuration");
        config.registry_timeout = Duration::from_millis(500);
        config
    }

    fn dispatch_live_once(session: &mut WaylandSession) {
        let interest = session.prepare_io().expect("prepare live Wayland I/O");
        session
            .dispatch_ready(WaylandIoReadiness {
                readable: interest.readable,
                writable: interest.writable,
            })
            .expect("dispatch live Wayland I/O");
    }

    fn await_live_configure(
        session: &mut WaylandSession,
        context: &str,
    ) -> (ConfigureToken, u32, u32) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            dispatch_live_once(session);
            match session.poll_lifecycle() {
                Some(WaylandLifecycleEvent::Configure {
                    token,
                    width,
                    height,
                    ..
                }) => return (token, width, height),
                Some(WaylandLifecycleEvent::ConnectionFailed(error)) => {
                    panic!("connection failed while waiting for {context}: {error}")
                }
                Some(WaylandLifecycleEvent::CloseRequested) => {
                    panic!("compositor closed the window while waiting for {context}")
                }
                None if std::time::Instant::now() >= deadline => {
                    panic!("timed out waiting for {context}")
                }
                None => thread::yield_now(),
            }
        }
    }

    fn present_live_frame(session: &mut WaylandSession, color: Color) {
        let screen = session.display_mut().screen();
        let pixels = vec![color; (screen.width * screen.height) as usize];
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
    }

    #[test]
    #[ignore = "requires a running Wayland compositor; run explicitly in the live evidence job"]
    fn live_maximize_reconfigures_and_presents_new_generation() {
        let mut config = WaylandConfig::new(
            "rlvgl WLD-01 live resize",
            "com.softoboros.rlvgl.live-resize",
            64,
            48,
        )
        .expect("valid live resize configuration");
        config.registry_timeout = Duration::from_secs(2);
        let mut session = WaylandSession::connect(config).expect("connect to live compositor");

        let (initial_token, initial_width, initial_height) =
            await_live_configure(&mut session, "initial configure");
        session
            .accept_configure(initial_token)
            .expect("accept initial configure");
        let initial_screen = session.display_mut().screen();
        assert_eq!(
            (initial_screen.width, initial_screen.height),
            (initial_width, initial_height)
        );

        present_live_frame(&mut session, Color(0x25, 0x6f, 0xa1, 0xff));
        assert_eq!(session.display_mut().stats().submitted_frames, 1);
        session.protocol.window.set_maximized();

        let (resize_token, resize_width, resize_height) =
            await_live_configure(&mut session, "maximized configure");
        assert_ne!(
            (resize_width, resize_height),
            (initial_width, initial_height),
            "maximize did not produce a distinct compositor geometry"
        );
        session
            .accept_configure(resize_token)
            .expect("accept maximized configure");
        let resized_screen = session.display_mut().screen();
        assert_eq!(
            (resized_screen.width, resized_screen.height),
            (resize_width, resize_height)
        );
        let retired_after_resize = session.display_mut().stats().retired_generations;

        present_live_frame(&mut session, Color(0xa1, 0x52, 0x25, 0xff));
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while session.display_mut().stats().submitted_frames < 2
            || session.display_mut().stats().frame_callbacks < 1
            || session.display_mut().stats().retired_generations != 0
        {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for resized frame completion and retired release"
            );
            dispatch_live_once(&mut session);
            thread::yield_now();
        }
        assert_eq!(session.display_mut().stats().submitted_frames, 2);

        eprintln!(
            "live resize: {initial_width}x{initial_height} -> {resize_width}x{resize_height}; retired immediately after accept={retired_after_resize}; callbacks={}",
            session.display_mut().stats().frame_callbacks
        );
        assert_eq!(session.state(), WaylandSessionState::Ready);
    }

    fn wait_for_live_marker(path: &Path, context: &str) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !path.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for {context} marker at {}",
                path.display()
            );
            thread::yield_now();
        }
    }

    struct CompositorResumeMarker(std::path::PathBuf);

    impl Drop for CompositorResumeMarker {
        fn drop(&mut self) {
            let _ = fs::write(&self.0, b"resume");
        }
    }

    #[test]
    #[ignore = "requires an externally controlled Weston compositor; run explicitly in the live evidence job"]
    fn live_vsync_recovers_from_socket_backpressure() {
        let control_dir = std::env::var_os("WLD01_BACKPRESSURE_CONTROL_DIR")
            .map(std::path::PathBuf::from)
            .expect("WLD01_BACKPRESSURE_CONTROL_DIR must name the controller directory");
        let ready_path = control_dir.join("client-ready");
        let stopped_path = control_dir.join("compositor-stopped");
        let resume_path = control_dir.join("client-resume");
        let _resume_guard = CompositorResumeMarker(resume_path.clone());

        let mut config = WaylandConfig::new(
            "rlvgl WLD-01 live backpressure",
            "com.softoboros.rlvgl.live-backpressure",
            64,
            48,
        )
        .expect("valid live backpressure configuration");
        config.registry_timeout = Duration::from_secs(2);
        let mut session = WaylandSession::connect(config).expect("connect to live compositor");
        let (token, _, _) = await_live_configure(&mut session, "initial configure");
        session
            .accept_configure(token)
            .expect("accept initial configure");

        fs::write(&ready_path, b"ready").expect("signal that the client is ready");
        wait_for_live_marker(&stopped_path, "stopped compositor");
        rustix::net::sockopt::set_socket_send_buffer_size(session.as_fd(), 4 * 1024)
            .expect("shrink live Wayland socket send buffer");

        let mut saturated = false;
        for request in 0..100_000 {
            session
                .protocol
                .window
                .set_title(format!("rlvgl WLD-01 saturated {request}"));
            match session.connection.flush() {
                Ok(()) => {}
                Err(ClientWaylandError::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    saturated = true;
                    break;
                }
                Err(error) => panic!("unexpected flush error while saturating socket: {error}"),
            }
        }
        assert!(saturated, "live client socket did not reach WouldBlock");

        let present_started = std::time::Instant::now();
        present_live_frame(&mut session, Color(0x31, 0x9a, 0x5b, 0xff));
        assert!(
            present_started.elapsed() < Duration::from_millis(100),
            "vsync blocked while the Wayland socket was saturated"
        );
        assert_eq!(session.display_mut().stats().submitted_frames, 1);
        let interest = session.prepare_io().expect("prepare saturated Wayland I/O");
        assert!(
            interest.writable,
            "vsync queued behind WouldBlock must request writable readiness"
        );

        fs::write(&resume_path, b"resume").expect("resume the controlled compositor");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while session.display_mut().stats().frame_callbacks < 1 || session.needs_write {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out draining backpressure and completing the queued frame"
            );
            dispatch_live_once(&mut session);
            thread::yield_now();
        }

        eprintln!(
            "live backpressure: WouldBlock observed, writable interest set, queued vsync completed"
        );
        assert_eq!(session.state(), WaylandSessionState::Ready);
    }

    #[test]
    fn constructor_rejects_missing_required_globals() {
        let (connection, worker) = fixture_connection(None, false);
        let result = WaylandSession::connect_with_connection(fixture_config(), connection);
        assert!(matches!(result, Err(WaylandError::Registry(_))));
        worker.join().unwrap();
    }

    #[test]
    fn constructor_rejects_missing_xdg_shell_global() {
        let (connection, worker) = fixture_connection(Some(4), false);
        let result = WaylandSession::connect_with_connection(fixture_config(), connection);
        assert!(matches!(result, Err(WaylandError::Registry(_))));
        worker.join().unwrap();
    }

    #[test]
    fn constructor_rejects_missing_shm_global() {
        let (connection, worker) = fixture_connection(Some(4), true);
        let result = WaylandSession::connect_with_connection(fixture_config(), connection);
        assert!(matches!(result, Err(WaylandError::Registry(_))));
        worker.join().unwrap();
    }

    #[test]
    fn constructor_rejects_old_compositor_version() {
        let (connection, worker) = fixture_connection(Some(3), false);
        let result = WaylandSession::connect_with_connection(fixture_config(), connection);
        assert!(matches!(
            result,
            Err(WaylandError::ProtocolVersion {
                interface: "wl_compositor",
                required: 4,
                offered: 3,
            })
        ));
        worker.join().unwrap();
    }

    #[test]
    fn configure_token_selection_rejects_superseded_token() {
        let superseded = ConfigureToken(41);
        let current = ConfigureSpec {
            token: ConfigureToken(42),
            surface_size: (800, 480),
            logical_size: (800, 480),
            scale: 1,
        };

        assert!(matches!(
            configure_spec_for_token(Some(current), superseded),
            Err(WaylandError::StaleConfigure(token)) if token == superseded
        ));
        let selected = configure_spec_for_token(Some(current), current.token).unwrap();
        assert_eq!(selected.token, current.token);
        assert_eq!(selected.surface_size, current.surface_size);
        assert_eq!(selected.logical_size, current.logical_size);
        assert_eq!(selected.scale, current.scale);
    }

    #[test]
    fn lifecycle_queue_coalesces_configure_and_preserves_terminal_events() {
        let mut lifecycle = VecDeque::from([
            WaylandLifecycleEvent::CloseRequested,
            WaylandLifecycleEvent::ConnectionFailed(WaylandError::Protocol("first".into())),
            WaylandLifecycleEvent::Configure {
                token: ConfigureToken(1),
                width: 640,
                height: 360,
                scale: 1,
            },
        ]);

        enqueue_configure_lifecycle(
            &mut lifecycle,
            3,
            WaylandLifecycleEvent::Configure {
                token: ConfigureToken(2),
                width: 800,
                height: 480,
                scale: 1,
            },
        );

        assert_eq!(lifecycle.len(), 3);
        assert!(matches!(
            lifecycle.front(),
            Some(WaylandLifecycleEvent::CloseRequested)
        ));
        assert!(matches!(
            lifecycle.get(1),
            Some(WaylandLifecycleEvent::ConnectionFailed(_))
        ));
        assert!(matches!(
            lifecycle.back(),
            Some(WaylandLifecycleEvent::Configure {
                token: ConfigureToken(2),
                width: 800,
                height: 480,
                scale: 1,
            })
        ));

        enqueue_terminal_lifecycle(
            &mut lifecycle,
            3,
            WaylandLifecycleEvent::ConnectionFailed(WaylandError::Protocol("second".into())),
        );
        assert_eq!(lifecycle.len(), 3);
        assert!(
            !lifecycle
                .iter()
                .any(|event| matches!(event, WaylandLifecycleEvent::Configure { .. }))
        );
        assert!(matches!(
            lifecycle.front(),
            Some(WaylandLifecycleEvent::CloseRequested)
        ));
        assert_eq!(
            lifecycle
                .iter()
                .filter(|event| matches!(event, WaylandLifecycleEvent::ConnectionFailed(_)))
                .count(),
            2
        );
    }

    #[test]
    fn outbound_backpressure_sets_write_interest_and_recovers_after_drain() {
        let (client_socket, mut server_socket) = UnixStream::pair().unwrap();
        rustix::net::sockopt::set_socket_send_buffer_size(&client_socket, 4 * 1024).unwrap();
        server_socket.set_nonblocking(true).unwrap();
        let connection = Connection::from_socket(client_socket).unwrap();
        let event_queue = connection.new_event_queue::<BackpressureState>();
        let qh = event_queue.handle();
        let display = connection.display();

        let mut saturated = false;
        for _ in 0..100_000 {
            let _callback = display.sync(&qh, ());
            match connection.flush() {
                Ok(()) => {}
                Err(ClientWaylandError::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    saturated = true;
                    break;
                }
                Err(error) => panic!("unexpected flush error while saturating socket: {error}"),
            }
        }
        assert!(saturated, "fixture did not saturate the client socket");

        let mut needs_write = false;
        flush_connection(&connection, &mut needs_write).unwrap();
        assert!(needs_write, "WouldBlock must request writable readiness");

        let mut buffer = [0_u8; 16 * 1024];
        loop {
            match server_socket.read(&mut buffer) {
                Ok(0) => panic!("client socket closed while draining backpressure"),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("unexpected socket drain error: {error}"),
            }
        }
        flush_connection(&connection, &mut needs_write).unwrap();
        assert!(!needs_write, "a successful retry must clear write interest");
    }

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
