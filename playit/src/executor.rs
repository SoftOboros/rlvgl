//! The playit executor: polls a transport, parses commands, and dispatches
//! them against the widget tree.

use crate::command::{Command, DumpSpec, EventSpec, QuerySpec};
use crate::framebuffer::FramebufferReader;
use crate::protocol::{format_event_spec, format_response, parse_command, write_hex_u32};
use crate::recorder::EventRecorder;
use crate::response::{Response, StatusData};
use crate::tag::{find_by_tag, find_by_tag_mut};
use crate::transport::PlayitTransport;
use rlvgl_core::WidgetNode;
use rlvgl_core::event::Event;

/// Maximum line length accepted by the executor.
const MAX_LINE: usize = 128;

/// Event transform pipeline (e.g. gesture recognition).
///
/// The executor feeds each injected raw event through [`process`](Self::process)
/// and calls [`tick`](Self::tick) once per [`PlayitExecutor::poll`] invocation.
/// Returned events are dispatched to the widget tree.
///
/// Two return slots cover the worst case of `DoubleTapRecognizer::process`
/// which may emit a buffered first tap plus the current event.
///
/// Use [`NullPipeline`] to skip gesture processing (direct dispatch).
///
/// # Example
///
/// ```
/// use rlvgl_core::event::Event;
/// use rlvgl_playit::EventPipeline;
///
/// /// Pipeline that swallows Tick events and passes everything else through.
/// struct FilterTicks;
///
/// impl EventPipeline for FilterTicks {
///     fn process(&mut self, event: Event) -> (Option<Event>, Option<Event>) {
///         match event {
///             Event::Tick => (None, None),
///             other => (Some(other), None),
///         }
///     }
///     fn tick(&mut self) -> (Option<Event>, Option<Event>) {
///         (None, None)
///     }
/// }
///
/// let mut pipeline = FilterTicks;
/// let (a, _b) = pipeline.process(Event::Tick);
/// assert!(a.is_none());
///
/// let (a, _b) = pipeline.process(Event::PressRelease { x: 10, y: 20 });
/// assert!(a.is_some());
/// ```
pub trait EventPipeline {
    /// Transform one input event into zero, one, or two output events.
    fn process(&mut self, event: Event) -> (Option<Event>, Option<Event>);
    /// Advance internal timers.  Called once per frame.
    fn tick(&mut self) -> (Option<Event>, Option<Event>);
}

/// No-op pipeline that passes events through unchanged.
pub struct NullPipeline;

impl EventPipeline for NullPipeline {
    fn process(&mut self, event: Event) -> (Option<Event>, Option<Event>) {
        (Some(event), None)
    }
    fn tick(&mut self) -> (Option<Event>, Option<Event>) {
        (None, None)
    }
}

/// State for an in-progress framebuffer dump.
struct DumpState {
    spec: DumpSpec,
    remaining: u8,
    last_present_seen: u32,
}

/// Processes playit commands arriving over a [`PlayitTransport`] and executes
/// them against a [`WidgetNode`] tree.
///
/// The const generic `REC_CAP` sets the maximum number of events the built-in
/// recorder can capture (default 256).
///
/// Call [`poll`](Self::poll) once per frame from your main loop.
pub struct PlayitExecutor<T: PlayitTransport, const REC_CAP: usize = 256> {
    transport: T,
    line_buf: [u8; MAX_LINE],
    line_len: usize,
    dump: Option<DumpState>,
    recorder: EventRecorder<REC_CAP>,
}

impl<T: PlayitTransport, const REC_CAP: usize> PlayitExecutor<T, REC_CAP> {
    /// Create a new executor wrapping the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            line_buf: [0; MAX_LINE],
            line_len: 0,
            dump: None,
            recorder: EventRecorder::new(),
        }
    }

    /// Drain incoming bytes, parse complete command lines, and execute them.
    ///
    /// * `root` — mutable reference to the widget tree root.
    /// * `status` — current telemetry snapshot for `?` commands.
    /// * `fb` — optional framebuffer reader for `D` pixel dump commands.
    /// * `pipeline` — event pipeline for gesture processing.  Pass
    ///   [`&mut NullPipeline`](NullPipeline) for direct dispatch.
    /// * `on_extension` — callback for application-defined extension commands.
    pub fn poll<P: EventPipeline, F>(
        &mut self,
        root: &mut WidgetNode,
        status: &StatusData,
        fb: Option<&dyn FramebufferReader>,
        pipeline: &mut P,
        mut on_extension: F,
    ) where
        F: FnMut(&[u8]),
    {
        // Drain transport bytes and accumulate lines.
        while let Some(byte) = self.transport.read_byte() {
            if byte == b'\n' || byte == b'\r' {
                if self.line_len > 0 {
                    let len = self.line_len;
                    let mut line = [0u8; MAX_LINE];
                    line[..len].copy_from_slice(&self.line_buf[..len]);
                    self.line_len = 0;
                    self.handle_line(&line[..len], root, status, pipeline, &mut on_extension);
                }
            } else if self.line_len < MAX_LINE {
                self.line_buf[self.line_len] = byte;
                self.line_len += 1;
            }
        }

        // Advance pipeline timers and dispatch any deferred gesture events.
        {
            let (a, b) = pipeline.tick();
            if let Some(evt) = a {
                root.dispatch_event(&evt);
            }
            if let Some(evt) = b {
                root.dispatch_event(&evt);
            }
        }

        // Advance recorder tick counter.
        self.recorder.tick();

        // Emit queued framebuffer dump rows if a new present has occurred.
        if let Some(reader) = fb {
            self.emit_dump_if_ready(reader);
        }
    }

    // ------------------------------------------------------------------
    // Internal dispatch
    // ------------------------------------------------------------------

    fn handle_line<P: EventPipeline, F>(
        &mut self,
        line: &[u8],
        root: &mut WidgetNode,
        status: &StatusData,
        pipeline: &mut P,
        on_extension: &mut F,
    ) where
        F: FnMut(&[u8]),
    {
        let Some(cmd) = parse_command(line) else {
            self.send_response(&Response::Error("parse error"));
            return;
        };

        match cmd {
            Command::Status => {
                self.send_response(&Response::Status(*status));
            }

            Command::Inject(spec) => {
                self.inject_event(spec, root, pipeline);
                self.send_response(&Response::Ok);
            }

            Command::InjectTagged(tag, spec) => {
                if let Some(node) = find_by_tag_mut(root, tag) {
                    // Tagged injection bypasses pipeline — targets a specific widget.
                    let event = spec.to_event();
                    if self.recorder.is_running() {
                        self.recorder.record(spec);
                    }
                    node.dispatch_event(&event);
                    self.send_response(&Response::Ok);
                } else {
                    self.send_response(&Response::Error("tag not found"));
                }
            }

            Command::Query(q) => self.handle_query(root, &q),

            Command::DumpPixels(spec) => {
                let present = spec.frames;
                self.dump = Some(DumpState {
                    spec,
                    remaining: present,
                    last_present_seen: 0,
                });
                self.send_str(b"DUMP:queued\r\n");
            }

            Command::RecordStart => {
                self.recorder.start();
                self.send_str(b"REC:recording\r\n");
            }

            Command::RecordStop => {
                self.recorder.stop();
                self.dump_recording();
            }

            Command::RecordDump => {
                self.dump_recording();
            }

            Command::Extension(payload) => {
                on_extension(payload);
                self.send_response(&Response::Ok);
            }
        }
    }

    /// Inject an event through the pipeline and into the widget tree,
    /// recording if the recorder is active.
    fn inject_event<P: EventPipeline>(
        &mut self,
        spec: EventSpec,
        root: &mut WidgetNode,
        pipeline: &mut P,
    ) {
        if self.recorder.is_running() {
            self.recorder.record(spec);
        }

        let event = spec.to_event();
        let (a, b) = pipeline.process(event);
        if let Some(evt) = a {
            root.dispatch_event(&evt);
        }
        if let Some(evt) = b {
            root.dispatch_event(&evt);
        }
    }

    fn handle_query(&mut self, root: &WidgetNode, q: &QuerySpec<'_>) {
        match q {
            QuerySpec::Bounds(tag) => {
                if let Some(node) = find_by_tag(root, tag) {
                    let b = node.widget.borrow().bounds();
                    self.send_response(&Response::Bounds {
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                    });
                } else {
                    self.send_response(&Response::Error("tag not found"));
                }
            }
            QuerySpec::Exists(tag) => {
                let found = find_by_tag(root, tag).is_some();
                self.send_response(&Response::Exists(found));
            }
            QuerySpec::ChildCount(tag) => {
                if let Some(node) = find_by_tag(root, tag) {
                    let count = node.children.len().min(u16::MAX as usize) as u16;
                    self.send_response(&Response::ChildCount(count));
                } else {
                    self.send_response(&Response::Error("tag not found"));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Recording dump
    // ------------------------------------------------------------------

    fn dump_recording(&mut self) {
        let len = self.recorder.len();
        // Header: REC:START,<count>
        {
            let mut hdr = [0u8; 32];
            let mut w = crate::protocol::BufWriter::new(&mut hdr);
            w.write_str("REC:START,");
            w.write_i32(len as i32);
            w.write_str("\r\n");
            let n = w.pos;
            self.transport.write_bytes(&hdr[..n]);
        }

        // Entries: @<delta> <command>\r\n
        // Drain to avoid borrowing self.recorder while writing to transport.
        for entry in self.recorder.drain() {
            // Format prefix: @<delta>
            let mut prefix = [0u8; 16];
            let pn = {
                let mut w = crate::protocol::BufWriter::new(&mut prefix);
                w.write_byte(b'@');
                w.write_i32(entry.tick_delta as i32);
                w.write_byte(b' ');
                w.pos
            };
            self.transport.write_bytes(&prefix[..pn]);

            // Format event spec
            let mut spec_buf = [0u8; 128];
            let sn = format_event_spec(&entry.spec, &mut spec_buf);
            self.transport.write_bytes(&spec_buf[..sn]);
            self.transport.write_bytes(b"\r\n");
        }

        self.transport.write_bytes(b"REC:END\r\n");
    }

    // ------------------------------------------------------------------
    // Framebuffer dump
    // ------------------------------------------------------------------

    fn emit_dump_if_ready(&mut self, reader: &dyn FramebufferReader) {
        let Some(state) = self.dump.as_mut() else {
            return;
        };

        let current_present = reader.present_count();
        if state.last_present_seen == 0 && state.remaining == state.spec.frames {
            state.last_present_seen = current_present;
            return;
        }
        if current_present == state.last_present_seen {
            return;
        }

        self.transport.write_bytes(b"F\r\n");

        let spec = state.spec;
        let mut row_buf = [0u32; 40];
        for row in 0..spec.height {
            let n = reader.read_row(
                spec.x,
                spec.y + row as i32,
                spec.width,
                &mut row_buf[..spec.width as usize],
            );
            for (i, &pixel) in row_buf[..n].iter().enumerate() {
                let mut hex = [0u8; 8];
                write_hex_u32(pixel, &mut hex);
                self.transport.write_bytes(&hex);
                if i + 1 < n {
                    self.transport.write_bytes(b" ");
                }
            }
            self.transport.write_bytes(b"\r\n");
        }

        state.last_present_seen = current_present;
        state.remaining -= 1;

        if state.remaining == 0 {
            self.dump = None;
            let mut buf = [0u8; 16];
            let n = format_response(&Response::DumpEnd, &mut buf);
            self.transport.write_bytes(&buf[..n]);
        }
    }

    // ------------------------------------------------------------------
    // Output helpers
    // ------------------------------------------------------------------

    fn send_response(&mut self, resp: &Response<'_>) {
        let mut buf = [0u8; 128];
        let n = format_response(resp, &mut buf);
        self.transport.write_bytes(&buf[..n]);
    }

    fn send_str(&mut self, s: &[u8]) {
        self.transport.write_bytes(s);
    }
}
