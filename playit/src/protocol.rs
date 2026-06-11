//! Text wire protocol codec.
//!
//! Each command is a single line (without the trailing newline).  The parser
//! operates on `&[u8]` and requires no allocator.
//!
//! # Command grammar
//!
//! | Wire format                    | Meaning                        |
//! |--------------------------------|--------------------------------|
//! | `T<x>,<y>`                     | PressRelease (backward compat) |
//! | `TD<x>,<y>`                    | PressDown                      |
//! | `TT<x>,<y>`                    | DoubleTap                      |
//! | `TK`                           | Tick                           |
//! | `T@<tag>:<x>,<y>`              | Tagged PressRelease            |
//! | `PD<x>,<y>`                    | PointerDown                    |
//! | `PU<x>,<y>`                    | PointerUp                      |
//! | `PM<x>,<y>`                    | PointerMove                    |
//! | `KD:<key>`                     | KeyDown                        |
//! | `KU:<key>`                     | KeyUp                          |
//! | `D<x>,<y>,<w>,<h>[,<frames>]`  | DumpPixels                     |
//! | `QB:<tag>`                     | Query Bounds                   |
//! | `QE:<tag>`                     | Query Exists                   |
//! | `QC:<tag>`                     | Query ChildCount               |
//! | `?`                            | Status                         |
//! | `X<payload>`                   | Extension                      |

use crate::command::{Command, DumpSpec, EventSpec, KeySpec, QuerySpec};
use crate::response::Response;

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a decimal integer from `bytes`, returning the value and the number
/// of bytes consumed.  Supports an optional leading `-`.
fn parse_i32(bytes: &[u8]) -> Option<(i32, usize)> {
    if bytes.is_empty() {
        return None;
    }
    let (neg, start) = if bytes[0] == b'-' {
        (true, 1)
    } else {
        (false, 0)
    };
    if start >= bytes.len() || !bytes[start].is_ascii_digit() {
        return None;
    }
    let mut value: i32 = 0;
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add((bytes[i] - b'0') as i32);
        i += 1;
    }
    if neg {
        value = -value;
    }
    Some((value, i))
}

/// Parse `<x>,<y>` from `bytes`.
fn parse_xy(bytes: &[u8]) -> Option<(i32, i32)> {
    let (x, consumed) = parse_i32(bytes)?;
    let rest = bytes.get(consumed..)?;
    if rest.first().copied() != Some(b',') {
        return None;
    }
    let (y, _) = parse_i32(&rest[1..])?;
    Some((x, y))
}

/// Parse a key name from a byte slice.
fn parse_key(bytes: &[u8]) -> Option<KeySpec> {
    let s = core::str::from_utf8(bytes).ok()?;
    Some(match s {
        "Escape" | "Esc" => KeySpec::Escape,
        "Enter" | "Return" => KeySpec::Enter,
        "Space" => KeySpec::Space,
        "Backspace" | "BS" => KeySpec::Backspace,
        "ArrowUp" | "Up" => KeySpec::ArrowUp,
        "ArrowDown" | "Down" => KeySpec::ArrowDown,
        "ArrowLeft" | "Left" => KeySpec::ArrowLeft,
        "ArrowRight" | "Right" => KeySpec::ArrowRight,
        _ => {
            // F1–F12
            if s.len() >= 2 && s.as_bytes()[0] == b'F' {
                if let Ok(n) = s[1..].parse::<u8>() {
                    return Some(KeySpec::Function(n));
                }
            }
            // Single character
            let mut chars = s.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() {
                    return Some(KeySpec::Character(c));
                }
            }
            return None;
        }
    })
}

// ---------------------------------------------------------------------------
// Command parser
// ---------------------------------------------------------------------------

/// Parse a single command line (without trailing newline).
///
/// Returns `None` if the line is empty or malformed.
pub fn parse_command(line: &[u8]) -> Option<Command<'_>> {
    if line.is_empty() {
        return None;
    }

    match line[0] {
        b'?' => Some(Command::Status),

        b'T' | b't' => parse_t_command(line),

        b'P' | b'p' => parse_p_command(line),

        b'K' | b'k' => parse_k_command(line),

        b'D' | b'd' => parse_d_command(line),

        b'Q' | b'q' => parse_q_command(line),

        b'M' | b'm' => parse_m_command(line),

        b'R' | b'r' => parse_r_command(line),

        b'X' | b'x' => Some(Command::Extension(&line[1..])),

        _ => Some(Command::Extension(line)),
    }
}

fn parse_t_command<'a>(line: &'a [u8]) -> Option<Command<'a>> {
    if line.len() < 2 {
        return None;
    }
    match line[1] {
        // TK — Tick
        b'K' | b'k' => Some(Command::Inject(EventSpec::Tick)),

        // TD<x>,<y> — PressDown
        b'D' | b'd' => {
            let (x, y) = parse_xy(&line[2..])?;
            Some(Command::Inject(EventSpec::PressDown { x, y }))
        }

        // TT<x>,<y> — DoubleTap
        b'T' | b't' => {
            let (x, y) = parse_xy(&line[2..])?;
            Some(Command::Inject(EventSpec::DoubleTap { x, y }))
        }

        // T@<tag>:<x>,<y> — Tagged PressRelease
        b'@' => {
            let rest = &line[2..];
            let colon = rest.iter().position(|&b| b == b':')?;
            let tag = core::str::from_utf8(&rest[..colon]).ok()?;
            let (x, y) = parse_xy(&rest[colon + 1..])?;
            Some(Command::InjectTagged(tag, EventSpec::PressRelease { x, y }))
        }

        // T<x>,<y> — PressRelease (digit or minus sign starts coordinates)
        b'0'..=b'9' | b'-' => {
            let (x, y) = parse_xy(&line[1..])?;
            Some(Command::Inject(EventSpec::PressRelease { x, y }))
        }

        _ => None,
    }
}

fn parse_p_command(line: &[u8]) -> Option<Command<'_>> {
    if line.len() < 2 {
        return None;
    }
    match line[1] {
        b'D' | b'd' => {
            let (x, y) = parse_xy(&line[2..])?;
            Some(Command::Inject(EventSpec::PointerDown { x, y }))
        }
        b'U' | b'u' => {
            let (x, y) = parse_xy(&line[2..])?;
            Some(Command::Inject(EventSpec::PointerUp { x, y }))
        }
        b'M' | b'm' => {
            let (x, y) = parse_xy(&line[2..])?;
            Some(Command::Inject(EventSpec::PointerMove { x, y }))
        }
        _ => None,
    }
}

fn parse_k_command(line: &[u8]) -> Option<Command<'_>> {
    if line.len() < 3 || line[2] != b':' {
        return None;
    }
    let key = parse_key(&line[3..])?;
    match line[1] {
        b'D' | b'd' => Some(Command::Inject(EventSpec::KeyDown { key })),
        b'U' | b'u' => Some(Command::Inject(EventSpec::KeyUp { key })),
        _ => None,
    }
}

fn parse_d_command(line: &[u8]) -> Option<Command<'_>> {
    // D<x>,<y>,<w>,<h>[,<frames>]
    let mut parts = [0i32; 5];
    parts[4] = 1; // default frames
    let mut idx = 0usize;
    let mut value = 0i32;
    let mut have_digits = false;

    for &byte in &line[1..] {
        if byte == b',' {
            if have_digits && idx < parts.len() {
                parts[idx] = value;
                idx += 1;
            }
            value = 0;
            have_digits = false;
        } else if byte.is_ascii_digit() {
            value = value.wrapping_mul(10).wrapping_add((byte - b'0') as i32);
            have_digits = true;
        } else {
            return None;
        }
    }
    if have_digits && idx < parts.len() {
        parts[idx] = value;
    }

    Some(Command::DumpPixels(DumpSpec {
        x: parts[0],
        y: parts[1],
        width: parts[2].clamp(1, 40) as u16,
        height: parts[3].clamp(1, 40) as u16,
        frames: parts[4].clamp(1, 4) as u8,
    }))
}

fn parse_q_command<'a>(line: &'a [u8]) -> Option<Command<'a>> {
    if line.len() < 3 || line[2] != b':' {
        return None;
    }
    let tag = core::str::from_utf8(&line[3..]).ok()?;
    match line[1] {
        b'B' | b'b' => Some(Command::Query(QuerySpec::Bounds(tag))),
        b'E' | b'e' => Some(Command::Query(QuerySpec::Exists(tag))),
        b'C' | b'c' => Some(Command::Query(QuerySpec::ChildCount(tag))),
        _ => None,
    }
}

fn parse_m_command(line: &[u8]) -> Option<Command<'_>> {
    if line.len() < 2 {
        return None;
    }
    match line[1] {
        // MT<count>:<id>,<state>,<x>,<y>;<id>,<state>,<x>,<y>;...
        b'T' | b't' => parse_mt_command(&line[2..]),
        _ => None,
    }
}

fn parse_mt_command(data: &[u8]) -> Option<Command<'_>> {
    use crate::command::{TouchPointSpec, TouchStateSpec};
    use rlvgl_core::event::MAX_TOUCH_POINTS;

    // Parse count before ':'
    let (count, consumed) = parse_i32(data)?;
    if count < 1 || count > MAX_TOUCH_POINTS as i32 {
        return None;
    }
    let rest = data.get(consumed..)?;
    if rest.first().copied() != Some(b':') {
        return None;
    }
    let mut remaining = &rest[1..];
    let mut points = [TouchPointSpec::default(); MAX_TOUCH_POINTS];

    for (i, point) in points.iter_mut().enumerate().take(count as usize) {
        if i > 0 {
            // Expect ';' separator
            if remaining.first().copied() != Some(b';') {
                return None;
            }
            remaining = &remaining[1..];
        }
        // Parse id
        let (id, c) = parse_i32(remaining)?;
        remaining = remaining.get(c..)?;
        if remaining.first().copied() != Some(b',') {
            return None;
        }
        remaining = &remaining[1..];

        // Parse state: D/U/C
        let state = match remaining.first().copied()? {
            b'D' | b'd' => TouchStateSpec::Down,
            b'U' | b'u' => TouchStateSpec::Up,
            b'C' | b'c' => TouchStateSpec::Contact,
            _ => return None,
        };
        remaining = &remaining[1..];
        if remaining.first().copied() != Some(b',') {
            return None;
        }
        remaining = &remaining[1..];

        // Parse x,y
        let (x, c) = parse_i32(remaining)?;
        remaining = remaining.get(c..)?;
        if remaining.first().copied() != Some(b',') {
            return None;
        }
        remaining = &remaining[1..];
        let (y, c) = parse_i32(remaining)?;
        remaining = remaining.get(c..).unwrap_or(&[]);

        *point = TouchPointSpec {
            id: id as u8,
            state,
            x,
            y,
        };
    }

    Some(Command::Inject(EventSpec::Touch {
        count: count as u8,
        points,
    }))
}

fn parse_r_command(line: &[u8]) -> Option<Command<'_>> {
    if line.len() < 2 {
        return None;
    }
    match line[1] {
        b'S' | b's' => Some(Command::RecordStart),
        b'E' | b'e' => Some(Command::RecordStop),
        b'D' | b'd' => Some(Command::RecordDump),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Event spec formatter (for recording dump output)
// ---------------------------------------------------------------------------

/// Write an [`EventSpec`] in wire-protocol format into `buf`.
///
/// Returns the number of bytes written.  Used by the executor to dump
/// recorded events.
pub fn format_event_spec(spec: &EventSpec, buf: &mut [u8]) -> usize {
    use crate::command::TouchStateSpec;

    let mut w = BufWriter::new(buf);
    match spec {
        EventSpec::Tick => w.write_str("TK"),
        EventSpec::PressRelease { x, y } => {
            w.write_byte(b'T');
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::PressDown { x, y } => {
            w.write_str("TD");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::PointerDown { x, y } => {
            w.write_str("PD");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::PointerUp { x, y } => {
            w.write_str("PU");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::PointerMove { x, y } => {
            w.write_str("PM");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::DoubleTap { x, y } => {
            w.write_str("TT");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
        }
        EventSpec::KeyDown { key } => {
            w.write_str("KD:");
            write_key_spec(&mut w, key);
        }
        EventSpec::KeyUp { key } => {
            w.write_str("KU:");
            write_key_spec(&mut w, key);
        }
        EventSpec::Touch { count, points } => {
            w.write_str("MT");
            w.write_i32(*count as i32);
            w.write_byte(b':');
            for (i, p) in points.iter().enumerate().take(*count as usize) {
                if i > 0 {
                    w.write_byte(b';');
                }
                w.write_i32(p.id as i32);
                w.write_byte(b',');
                w.write_byte(match p.state {
                    TouchStateSpec::Down => b'D',
                    TouchStateSpec::Up => b'U',
                    TouchStateSpec::Contact => b'C',
                });
                w.write_byte(b',');
                w.write_i32(p.x);
                w.write_byte(b',');
                w.write_i32(p.y);
            }
        }
    }
    w.pos
}

fn write_key_spec(w: &mut BufWriter<'_>, key: &KeySpec) {
    match key {
        KeySpec::Escape => w.write_str("Escape"),
        KeySpec::Enter => w.write_str("Enter"),
        KeySpec::Space => w.write_str("Space"),
        KeySpec::Backspace => w.write_str("Backspace"),
        KeySpec::ArrowUp => w.write_str("ArrowUp"),
        KeySpec::ArrowDown => w.write_str("ArrowDown"),
        KeySpec::ArrowLeft => w.write_str("ArrowLeft"),
        KeySpec::ArrowRight => w.write_str("ArrowRight"),
        KeySpec::Function(n) => {
            w.write_byte(b'F');
            w.write_i32(*n as i32);
        }
        KeySpec::Character(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            w.write_str(s);
        }
        KeySpec::Other(v) => {
            w.write_i32(*v as i32);
        }
    }
}

// ---------------------------------------------------------------------------
// Response formatter
// ---------------------------------------------------------------------------

/// Format a response into `buf`, returning the number of bytes written.
///
/// The output includes the trailing `\r\n`.  If the buffer is too small the
/// output is silently truncated.
pub fn format_response(resp: &Response<'_>, buf: &mut [u8]) -> usize {
    let mut w = BufWriter::new(buf);
    match resp {
        Response::Ok => w.write_str("OK"),
        Response::Error(reason) => {
            w.write_str("ERR: ");
            w.write_str(reason);
        }
        Response::Bounds {
            x,
            y,
            width,
            height,
        } => {
            w.write_str("BOUNDS:");
            w.write_i32(*x);
            w.write_byte(b',');
            w.write_i32(*y);
            w.write_byte(b',');
            w.write_i32(*width);
            w.write_byte(b',');
            w.write_i32(*height);
        }
        Response::Exists(yes) => {
            w.write_str("EXISTS:");
            w.write_str(if *yes { "1" } else { "0" });
        }
        Response::ChildCount(n) => {
            w.write_str("CHILDREN:");
            w.write_i32(*n as i32);
        }
        Response::Status(s) => {
            w.write_str("STAT:");
            w.write_i32(s.tick_count as i32);
            w.write_byte(b',');
            w.write_i32(s.present_count as i32);
        }
        Response::DumpEnd => w.write_str("END"),
    }
    w.write_str("\r\n");
    w.pos
}

// ---------------------------------------------------------------------------
// Tiny no-alloc buffer writer
// ---------------------------------------------------------------------------

pub(crate) struct BufWriter<'a> {
    buf: &'a mut [u8],
    pub(crate) pos: usize,
}

impl<'a> BufWriter<'a> {
    pub(crate) fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(crate) fn write_byte(&mut self, b: u8) {
        if self.pos < self.buf.len() {
            self.buf[self.pos] = b;
            self.pos += 1;
        }
    }

    pub(crate) fn write_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.write_byte(b);
        }
    }

    pub(crate) fn write_i32(&mut self, mut v: i32) {
        if v < 0 {
            self.write_byte(b'-');
            // Handle i32::MIN carefully
            if v == i32::MIN {
                self.write_str("2147483648");
                return;
            }
            v = -v;
        }
        let mut tmp = [0u8; 10];
        let mut i = 0;
        if v == 0 {
            self.write_byte(b'0');
            return;
        }
        while v > 0 {
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            self.write_byte(tmp[i]);
        }
    }
}

/// Write a u32 as 8 uppercase hex digits into `buf`.
/// Returns the number of bytes written (always 8 if buf is large enough).
pub fn write_hex_u32(value: u32, buf: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut n = 0;
    for i in (0..8).rev() {
        if n < buf.len() {
            buf[n] = HEX[((value >> (i * 4)) & 0xF) as usize];
            n += 1;
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::*;
    use crate::response::StatusData;

    #[test]
    fn parse_status() {
        assert_eq!(parse_command(b"?"), Some(Command::Status));
    }

    #[test]
    fn parse_press_release() {
        assert_eq!(
            parse_command(b"T100,200"),
            Some(Command::Inject(EventSpec::PressRelease { x: 100, y: 200 }))
        );
    }

    #[test]
    fn parse_press_release_negative() {
        assert_eq!(
            parse_command(b"T-10,50"),
            Some(Command::Inject(EventSpec::PressRelease { x: -10, y: 50 }))
        );
    }

    #[test]
    fn parse_press_down() {
        assert_eq!(
            parse_command(b"TD30,40"),
            Some(Command::Inject(EventSpec::PressDown { x: 30, y: 40 }))
        );
    }

    #[test]
    fn parse_double_tap() {
        assert_eq!(
            parse_command(b"TT50,60"),
            Some(Command::Inject(EventSpec::DoubleTap { x: 50, y: 60 }))
        );
    }

    #[test]
    fn parse_tick() {
        assert_eq!(parse_command(b"TK"), Some(Command::Inject(EventSpec::Tick)));
    }

    #[test]
    fn parse_pointer_down() {
        assert_eq!(
            parse_command(b"PD10,20"),
            Some(Command::Inject(EventSpec::PointerDown { x: 10, y: 20 }))
        );
    }

    #[test]
    fn parse_pointer_up() {
        assert_eq!(
            parse_command(b"PU10,20"),
            Some(Command::Inject(EventSpec::PointerUp { x: 10, y: 20 }))
        );
    }

    #[test]
    fn parse_pointer_move() {
        assert_eq!(
            parse_command(b"PM10,20"),
            Some(Command::Inject(EventSpec::PointerMove { x: 10, y: 20 }))
        );
    }

    #[test]
    fn parse_key_down() {
        assert_eq!(
            parse_command(b"KD:Enter"),
            Some(Command::Inject(EventSpec::KeyDown {
                key: KeySpec::Enter
            }))
        );
    }

    #[test]
    fn parse_key_up_char() {
        assert_eq!(
            parse_command(b"KU:a"),
            Some(Command::Inject(EventSpec::KeyUp {
                key: KeySpec::Character('a')
            }))
        );
    }

    #[test]
    fn parse_key_function() {
        assert_eq!(
            parse_command(b"KD:F5"),
            Some(Command::Inject(EventSpec::KeyDown {
                key: KeySpec::Function(5)
            }))
        );
    }

    #[test]
    fn parse_dump() {
        assert_eq!(
            parse_command(b"D10,20,5,5,2"),
            Some(Command::DumpPixels(DumpSpec {
                x: 10,
                y: 20,
                width: 5,
                height: 5,
                frames: 2,
            }))
        );
    }

    #[test]
    fn parse_dump_defaults() {
        assert_eq!(
            parse_command(b"D0,0,8,4"),
            Some(Command::DumpPixels(DumpSpec {
                x: 0,
                y: 0,
                width: 8,
                height: 4,
                frames: 1,
            }))
        );
    }

    #[test]
    fn parse_query_bounds() {
        assert_eq!(
            parse_command(b"QB:dashboard"),
            Some(Command::Query(QuerySpec::Bounds("dashboard")))
        );
    }

    #[test]
    fn parse_query_exists() {
        assert_eq!(
            parse_command(b"QE:footer"),
            Some(Command::Query(QuerySpec::Exists("footer")))
        );
    }

    #[test]
    fn parse_query_children() {
        assert_eq!(
            parse_command(b"QC:root"),
            Some(Command::Query(QuerySpec::ChildCount("root")))
        );
    }

    #[test]
    fn parse_tagged_inject() {
        assert_eq!(
            parse_command(b"T@btn_ok:50,100"),
            Some(Command::InjectTagged(
                "btn_ok",
                EventSpec::PressRelease { x: 50, y: 100 }
            ))
        );
    }

    #[test]
    fn parse_extension() {
        assert_eq!(parse_command(b"Xhello"), Some(Command::Extension(b"hello")));
    }

    #[test]
    fn parse_legacy_crawl_as_extension() {
        // 'C' doesn't have a dedicated parser — falls through to extension
        assert_eq!(parse_command(b"C"), Some(Command::Extension(b"C")));
    }

    #[test]
    fn parse_empty_returns_none() {
        assert_eq!(parse_command(b""), None);
    }

    #[test]
    fn format_ok() {
        let mut buf = [0u8; 32];
        let n = format_response(&Response::Ok, &mut buf);
        assert_eq!(&buf[..n], b"OK\r\n");
    }

    #[test]
    fn format_error() {
        let mut buf = [0u8; 64];
        let n = format_response(&Response::Error("bad tap"), &mut buf);
        assert_eq!(&buf[..n], b"ERR: bad tap\r\n");
    }

    #[test]
    fn format_bounds() {
        let mut buf = [0u8; 64];
        let n = format_response(
            &Response::Bounds {
                x: 10,
                y: 20,
                width: 100,
                height: 50,
            },
            &mut buf,
        );
        assert_eq!(&buf[..n], b"BOUNDS:10,20,100,50\r\n");
    }

    #[test]
    fn format_exists() {
        let mut buf = [0u8; 32];
        let n = format_response(&Response::Exists(true), &mut buf);
        assert_eq!(&buf[..n], b"EXISTS:1\r\n");
    }

    #[test]
    fn format_children() {
        let mut buf = [0u8; 32];
        let n = format_response(&Response::ChildCount(7), &mut buf);
        assert_eq!(&buf[..n], b"CHILDREN:7\r\n");
    }

    #[test]
    fn format_status() {
        let mut buf = [0u8; 64];
        let n = format_response(
            &Response::Status(StatusData {
                tick_count: 1234,
                present_count: 56,
            }),
            &mut buf,
        );
        assert_eq!(&buf[..n], b"STAT:1234,56\r\n");
    }

    #[test]
    fn format_dump_end() {
        let mut buf = [0u8; 32];
        let n = format_response(&Response::DumpEnd, &mut buf);
        assert_eq!(&buf[..n], b"END\r\n");
    }

    #[test]
    fn hex_u32() {
        let mut buf = [0u8; 8];
        write_hex_u32(0xFF00_AA55, &mut buf);
        assert_eq!(&buf, b"FF00AA55");
    }

    // -- Multi-touch -------------------------------------------------------

    #[test]
    fn parse_multi_touch() {
        let cmd = parse_command(b"MT2:0,D,100,200;1,C,300,400");
        let expected = Command::Inject(EventSpec::Touch {
            count: 2,
            points: {
                let mut p = [TouchPointSpec::default(); 5];
                p[0] = TouchPointSpec {
                    id: 0,
                    state: TouchStateSpec::Down,
                    x: 100,
                    y: 200,
                };
                p[1] = TouchPointSpec {
                    id: 1,
                    state: TouchStateSpec::Contact,
                    x: 300,
                    y: 400,
                };
                p
            },
        });
        assert_eq!(cmd, Some(expected));
    }

    #[test]
    fn parse_multi_touch_single() {
        let cmd = parse_command(b"MT1:0,U,50,60");
        assert!(matches!(
            cmd,
            Some(Command::Inject(EventSpec::Touch { count: 1, .. }))
        ));
    }

    // -- Recorder commands -------------------------------------------------

    #[test]
    fn parse_record_start() {
        assert_eq!(parse_command(b"RS"), Some(Command::RecordStart));
    }

    #[test]
    fn parse_record_stop() {
        assert_eq!(parse_command(b"RE"), Some(Command::RecordStop));
    }

    #[test]
    fn parse_record_dump() {
        assert_eq!(parse_command(b"RD"), Some(Command::RecordDump));
    }

    // -- Event spec formatter (round-trip) ---------------------------------

    #[test]
    fn format_event_spec_press_release() {
        let mut buf = [0u8; 64];
        let n = format_event_spec(&EventSpec::PressRelease { x: 100, y: 200 }, &mut buf);
        assert_eq!(&buf[..n], b"T100,200");
    }

    #[test]
    fn format_event_spec_pointer_move() {
        let mut buf = [0u8; 64];
        let n = format_event_spec(&EventSpec::PointerMove { x: -5, y: 30 }, &mut buf);
        assert_eq!(&buf[..n], b"PM-5,30");
    }

    #[test]
    fn format_event_spec_tick() {
        let mut buf = [0u8; 16];
        let n = format_event_spec(&EventSpec::Tick, &mut buf);
        assert_eq!(&buf[..n], b"TK");
    }

    #[test]
    fn format_event_spec_key() {
        let mut buf = [0u8; 32];
        let n = format_event_spec(
            &EventSpec::KeyDown {
                key: KeySpec::Enter,
            },
            &mut buf,
        );
        assert_eq!(&buf[..n], b"KD:Enter");
    }

    #[test]
    fn format_event_spec_touch() {
        let mut points = [TouchPointSpec::default(); 5];
        points[0] = TouchPointSpec {
            id: 0,
            state: TouchStateSpec::Down,
            x: 10,
            y: 20,
        };
        points[1] = TouchPointSpec {
            id: 1,
            state: TouchStateSpec::Contact,
            x: 30,
            y: 40,
        };
        let mut buf = [0u8; 128];
        let n = format_event_spec(&EventSpec::Touch { count: 2, points }, &mut buf);
        assert_eq!(&buf[..n], b"MT2:0,D,10,20;1,C,30,40");
    }

    #[test]
    fn round_trip_press_release() {
        let spec = EventSpec::PressRelease { x: 42, y: -7 };
        let mut buf = [0u8; 64];
        let n = format_event_spec(&spec, &mut buf);
        let parsed = parse_command(&buf[..n]);
        assert_eq!(parsed, Some(Command::Inject(spec)));
    }
}
