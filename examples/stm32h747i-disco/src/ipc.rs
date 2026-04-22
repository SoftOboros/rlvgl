// This module is compiled into both CM7 and CM4 binaries; each side only uses
// a subset of the public API, so unused-code warnings are expected and silenced.
#![allow(dead_code)]

//! Bidirectional cross-core IPC (CM4 ↔ CM7) using ring buffers in D2 SRAM3.
//!
//! Two independent queues share the 1KB mailbox region at 0x3004_7000:
//! - **CmdQueue** (CM4 → CM7): application commands (navigate, update labels, etc.)
//! - **EvtQueue** (CM7 → CM4): UI events (touch, button press, frame sync)
//!
//! All queue pointer updates use DMB (data memory barrier) for cross-core
//! visibility — `compiler_fence` alone is insufficient because CM4's write buffer
//! may defer stores to the AHB bus matrix.

// ── Memory layout ───────────────────────────────────────────────────────────
//
// MAILBOX region in D2 SRAM3: 0x3004_7000, 1KB (declared in memory.x / memory_cm4.x)
//
// 0x30047000  CmdQueue : head(4) + tail(4) + 16 × 20 = 328 bytes
// 0x30047200  EvtQueue : head(4) + tail(4) +  8 × 20 = 168 bytes
// Total: 496 bytes (well within 1KB)

const MAILBOX_BASE: usize = 0x3004_7000;
const CMD_QUEUE_ADDR: usize = MAILBOX_BASE;
const EVT_QUEUE_ADDR: usize = MAILBOX_BASE + 0x200;

const CMD_CAPACITY: u32 = 16;
const EVT_CAPACITY: u32 = 8;

// ── Shared data types ───────────────────────────────────────────────────────

/// Command kinds sent from CM4 (application) to CM7 (display server).
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CmdKind {
    /// Zero sentinel — never sent, used for init.
    None = 0,
    /// Set backlight duty cycle.  a: duty (u16 in low bits).
    SetBacklight = 1,
    /// Navigate to a screen.  a: screen_id.
    Navigate = 2,
    /// Update a label's text.  a: widget_id, b+c+d: up to 12 bytes UTF-8.
    UpdateLabel = 3,
    /// Update a numeric value.  a: widget_id, b: value (i32).
    UpdateValue = 4,
    /// Show or hide a widget.  a: widget_id, b: 1=show / 0=hide.
    ShowWidget = 5,
}

impl CmdKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::SetBacklight,
            2 => Self::Navigate,
            3 => Self::UpdateLabel,
            4 => Self::UpdateValue,
            5 => Self::ShowWidget,
            _ => Self::None,
        }
    }
}

/// Event kinds sent from CM7 (display server) to CM4 (application).
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Zero sentinel — never sent.
    None = 0,
    /// Touch/pointer pressed.  a: x, b: y.
    PointerDown = 1,
    /// Touch/pointer moved.  a: x, b: y.
    PointerMove = 2,
    /// Touch/pointer released.  a: x, b: y.
    PointerUp = 3,
    /// A button widget was pressed.  a: widget_id.
    ButtonPressed = 4,
    /// A frame was rendered and presented.  a: frame_counter.
    FrameRendered = 5,
}

impl EventKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::PointerDown,
            2 => Self::PointerMove,
            3 => Self::PointerUp,
            4 => Self::ButtonPressed,
            5 => Self::FrameRendered,
            _ => Self::None,
        }
    }
}

/// Raw command payload (5 × u32 = 20 bytes).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CommandRaw {
    pub kind: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

impl Default for CommandRaw {
    fn default() -> Self {
        Self {
            kind: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        }
    }
}

/// Raw event payload (5 × u32 = 20 bytes).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct EventRaw {
    pub kind: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

impl Default for EventRaw {
    fn default() -> Self {
        Self {
            kind: 0,
            a: 0,
            b: 0,
            c: 0,
            d: 0,
        }
    }
}

// ── Ring buffer structures in shared memory ─────────────────────────────────

#[repr(C)]
struct CmdQueue {
    head: u32,
    tail: u32,
    slots: [CommandRaw; CMD_CAPACITY as usize],
}

#[repr(C)]
struct EvtQueue {
    head: u32,
    tail: u32,
    slots: [EventRaw; EVT_CAPACITY as usize],
}

fn cmd_queue() -> &'static mut CmdQueue { // rlvgl-discipline: allow(static_mut)
    unsafe { &mut *(CMD_QUEUE_ADDR as *mut CmdQueue) }
}

fn evt_queue() -> &'static mut EvtQueue { // rlvgl-discipline: allow(static_mut)
    unsafe { &mut *(EVT_QUEUE_ADDR as *mut EvtQueue) }
}

// ── Initialisation ──────────────────────────────────────────────────────────

/// Zero both ring buffers.  Called once by CM7 before releasing CM4.
pub fn init() {
    let cq = cmd_queue();
    cq.head = 0;
    cq.tail = 0;
    for slot in cq.slots.iter_mut() {
        *slot = CommandRaw::default();
    }

    let eq = evt_queue();
    eq.head = 0;
    eq.tail = 0;
    for slot in eq.slots.iter_mut() {
        *slot = EventRaw::default();
    }

    cortex_m::asm::dmb();
}

/// CM4 init — does NOT zero the queues (CM7 already did).
/// Just executes a DMB so CM4 observes CM7's writes.
pub fn init_cm4() {
    cortex_m::asm::dmb();
}

// ── Command queue (CM4 → CM7) ───────────────────────────────────────────────

/// Push a command into the CM4→CM7 queue.  Returns false if queue is full.
#[allow(dead_code)] // CM7 only pops
pub fn cmd_push(cmd: CommandRaw) -> bool {
    let q = cmd_queue();
    cortex_m::asm::dmb();
    let head = q.head;
    let tail = q.tail;
    if head.wrapping_sub(tail) >= CMD_CAPACITY {
        return false;
    }
    q.slots[(head % CMD_CAPACITY) as usize] = cmd;
    cortex_m::asm::dmb();
    q.head = head.wrapping_add(1);
    cortex_m::asm::dmb();
    true
}

/// Spin-wait until a slot is free, then push.
#[allow(dead_code)] // CM7 only pops
pub fn cmd_push_blocking(cmd: CommandRaw) {
    while !cmd_push(cmd) {
        cortex_m::asm::nop();
    }
}

/// Pop one command from the CM4→CM7 queue.  Returns None if empty.
#[allow(dead_code)] // CM4 only pushes
pub fn cmd_pop() -> Option<CommandRaw> {
    let q = cmd_queue();
    cortex_m::asm::dmb();
    let head = q.head;
    let tail = q.tail;
    if head == tail {
        return None;
    }
    cortex_m::asm::dmb();
    let cmd = q.slots[(tail % CMD_CAPACITY) as usize];
    q.tail = tail.wrapping_add(1);
    cortex_m::asm::dmb();
    Some(cmd)
}

// ── Event queue (CM7 → CM4) ─────────────────────────────────────────────────

/// Push an event into the CM7→CM4 queue.  Returns false if queue is full.
#[allow(dead_code)] // CM4 only pops
pub fn event_push(evt: EventRaw) -> bool {
    let q = evt_queue();
    cortex_m::asm::dmb();
    let head = q.head;
    let tail = q.tail;
    if head.wrapping_sub(tail) >= EVT_CAPACITY {
        return false;
    }
    q.slots[(head % EVT_CAPACITY) as usize] = evt;
    cortex_m::asm::dmb();
    q.head = head.wrapping_add(1);
    cortex_m::asm::dmb();
    true
}

/// Pop one event from the CM7→CM4 queue.  Returns None if empty.
#[allow(dead_code)] // CM7 only pushes
pub fn event_pop() -> Option<EventRaw> {
    let q = evt_queue();
    cortex_m::asm::dmb();
    let head = q.head;
    let tail = q.tail;
    if head == tail {
        return None;
    }
    cortex_m::asm::dmb();
    let evt = q.slots[(tail % EVT_CAPACITY) as usize];
    q.tail = tail.wrapping_add(1);
    cortex_m::asm::dmb();
    Some(evt)
}

// ── Command helpers ─────────────────────────────────────────────────────────

/// Build a SetBacklight command.
#[allow(dead_code)]
pub fn cmd_set_backlight(duty: u16) -> CommandRaw {
    CommandRaw {
        kind: CmdKind::SetBacklight as u32,
        a: duty as u32,
        ..Default::default()
    }
}

/// Build a Navigate command.
#[allow(dead_code)]
pub fn cmd_navigate(screen_id: u32) -> CommandRaw {
    CommandRaw {
        kind: CmdKind::Navigate as u32,
        a: screen_id,
        ..Default::default()
    }
}

/// Build an UpdateLabel command with up to 12 bytes of inline UTF-8 text.
/// Text beyond 12 bytes is silently truncated.
#[allow(dead_code)]
pub fn cmd_update_label(widget_id: u32, text: &[u8]) -> CommandRaw {
    let mut b: u32 = 0;
    let mut c: u32 = 0;
    let mut d: u32 = 0;
    let len = text.len().min(12);
    for (i, &byte) in text[..len].iter().enumerate() {
        match i / 4 {
            0 => b |= (byte as u32) << ((i % 4) * 8),
            1 => c |= (byte as u32) << ((i % 4) * 8),
            2 => d |= (byte as u32) << ((i % 4) * 8),
            _ => {}
        }
    }
    CommandRaw {
        kind: CmdKind::UpdateLabel as u32,
        a: widget_id,
        b,
        c,
        d,
    }
}

/// Extract inline text from an UpdateLabel command.
/// Returns the number of valid bytes written to `buf` (max 12).
pub fn extract_label_text(cmd: &CommandRaw, buf: &mut [u8; 12]) -> usize {
    let words = [cmd.b, cmd.c, cmd.d];
    for (wi, &word) in words.iter().enumerate() {
        for bi in 0..4 {
            buf[wi * 4 + bi] = ((word >> (bi * 8)) & 0xFF) as u8;
        }
    }
    // Find first null byte to determine length
    buf.iter().position(|&b| b == 0).unwrap_or(12)
}

/// Build an UpdateValue command.
#[allow(dead_code)]
pub fn cmd_update_value(widget_id: u32, value: i32) -> CommandRaw {
    CommandRaw {
        kind: CmdKind::UpdateValue as u32,
        a: widget_id,
        b: value as u32,
        ..Default::default()
    }
}

/// Build a ShowWidget command.
#[allow(dead_code)]
pub fn cmd_show_widget(widget_id: u32, visible: bool) -> CommandRaw {
    CommandRaw {
        kind: CmdKind::ShowWidget as u32,
        a: widget_id,
        b: if visible { 1 } else { 0 },
        ..Default::default()
    }
}

// ── Event helpers ───────────────────────────────────────────────────────────

/// Build a PointerDown event.
#[allow(dead_code)]
pub fn evt_pointer_down(x: i32, y: i32) -> EventRaw {
    EventRaw {
        kind: EventKind::PointerDown as u32,
        a: x as u32,
        b: y as u32,
        ..Default::default()
    }
}

/// Build a PointerMove event.
#[allow(dead_code)]
pub fn evt_pointer_move(x: i32, y: i32) -> EventRaw {
    EventRaw {
        kind: EventKind::PointerMove as u32,
        a: x as u32,
        b: y as u32,
        ..Default::default()
    }
}

/// Build a PointerUp event.
#[allow(dead_code)]
pub fn evt_pointer_up(x: i32, y: i32) -> EventRaw {
    EventRaw {
        kind: EventKind::PointerUp as u32,
        a: x as u32,
        b: y as u32,
        ..Default::default()
    }
}

/// Build a ButtonPressed event.
#[allow(dead_code)]
pub fn evt_button_pressed(widget_id: u32) -> EventRaw {
    EventRaw {
        kind: EventKind::ButtonPressed as u32,
        a: widget_id,
        ..Default::default()
    }
}

/// Build a FrameRendered event.
#[allow(dead_code)]
pub fn evt_frame_rendered(frame_counter: u32) -> EventRaw {
    EventRaw {
        kind: EventKind::FrameRendered as u32,
        a: frame_counter,
        ..Default::default()
    }
}

// ── Well-known widget IDs (shared between CM4 and CM7 by convention) ────────

pub mod widget_id {
    /// Title label at top of screen.
    pub const TITLE: u32 = 1;
    /// Click counter button.
    pub const CLICK_COUNTER: u32 = 2;
    /// Status label for application messages.
    pub const STATUS_LABEL: u32 = 3;
}
