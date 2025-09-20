//! Simple cross-core mailbox (CM4 -> CM7) using a fixed location in D2 SRAM3.
//! This is deliberately tiny and blocking for bring-up.

use core::sync::atomic::{Ordering, compiler_fence};

// Mailbox base in D2 SRAM3 (declared in memory scripts): 0x3004_7000
const MAILBOX_BASE: usize = 0x3004_7000;

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CmdKind {
    None = 0,
    SetBacklight = 1, // a: duty (u16 in low bits)
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CommandRaw {
    pub kind: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

impl Default for CommandRaw {
    fn default() -> Self {
        Self {
            kind: 0,
            a: 0,
            b: 0,
            c: 0,
        }
    }
}

#[repr(C)]
struct Mailbox {
    write: u32,
    read: u32,
    cmd: CommandRaw,
}

fn mb() -> &'static mut Mailbox {
    unsafe { &mut *(MAILBOX_BASE as *mut Mailbox) }
}

pub fn init() {
    let m = mb();
    m.write = 0;
    m.read = 0;
    m.cmd = CommandRaw::default();
    compiler_fence(Ordering::SeqCst);
}

pub fn try_push(cmd: CommandRaw) -> bool {
    let m = mb();
    // single-slot: if write != read, consumer hasn't drained
    if m.write != m.read {
        return false;
    }
    compiler_fence(Ordering::SeqCst);
    m.cmd = cmd;
    // publish
    m.write = m.write.wrapping_add(1);
    compiler_fence(Ordering::SeqCst);
    true
}

pub fn pop() -> Option<CommandRaw> {
    let m = mb();
    if m.write == m.read {
        return None;
    }
    compiler_fence(Ordering::SeqCst);
    let cmd = m.cmd;
    m.read = m.write;
    compiler_fence(Ordering::SeqCst);
    Some(cmd)
}

pub fn cmd_set_backlight(duty: u16) -> CommandRaw {
    CommandRaw {
        kind: CmdKind::SetBacklight as u32,
        a: duty as u32,
        b: 0,
        c: 0,
    }
}
