// SPDX-License-Identifier: MIT
//! Integration coverage for the disco simulator playit automation socket.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

struct SimulatorSession {
    child: Child,
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl SimulatorSession {
    fn binary_path() -> PathBuf {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_rlvgl-disco-sim") {
            return PathBuf::from(path);
        }
        if let Some(path) = option_env!("CARGO_BIN_EXE_rlvgl-disco-sim") {
            return PathBuf::from(path);
        }

        let exe = std::env::current_exe().expect("failed to resolve current test executable");
        exe.parent()
            .and_then(|path| path.parent())
            .map(|path| path.join(format!("rlvgl-disco-sim{}", std::env::consts::EXE_SUFFIX)))
            .expect("failed to derive disco simulator binary path")
    }

    fn launch() -> Self {
        let mut child = Command::new(Self::binary_path())
            .arg("--automation-headless")
            .arg("--playit-port")
            .arg("0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn disco simulator");

        let stdout = child.stdout.take().expect("missing simulator stdout");
        let mut stdout = BufReader::new(stdout);
        let mut ready = String::new();
        stdout
            .read_line(&mut ready)
            .expect("failed to read simulator ready line");
        assert!(
            ready.starts_with("PLAYIT_READY tcp://127.0.0.1:"),
            "unexpected ready line: {ready:?}"
        );

        let port = ready
            .trim()
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .expect("failed to parse ready port");
        let writer = TcpStream::connect(("127.0.0.1", port)).expect("failed to connect to playit");
        writer
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("failed to set read timeout");
        writer
            .set_write_timeout(Some(Duration::from_secs(3)))
            .expect("failed to set write timeout");
        let reader = BufReader::new(writer.try_clone().expect("failed to clone playit socket"));

        Self {
            child,
            reader,
            writer,
        }
    }

    fn send(&mut self, command: &str) {
        self.writer
            .write_all(command.as_bytes())
            .expect("failed to write playit command");
        self.writer
            .write_all(b"\n")
            .expect("failed to terminate playit command");
        self.writer.flush().expect("failed to flush playit command");
    }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .expect("failed to read playit response");
        assert!(!line.is_empty(), "playit connection closed unexpectedly");
        line.trim_end_matches(['\r', '\n']).to_string()
    }
}

impl Drop for SimulatorSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_status(line: &str) -> (u32, u32) {
    let payload = line
        .strip_prefix("STAT:")
        .expect("status line did not start with STAT:");
    let (tick, present) = payload
        .split_once(',')
        .expect("status line was missing a comma");
    (
        tick.parse::<u32>().expect("invalid tick count"),
        present.parse::<u32>().expect("invalid present count"),
    )
}

fn parse_bounds(line: &str) -> (i32, i32, i32, i32) {
    let payload = line
        .strip_prefix("BOUNDS:")
        .expect("bounds line did not start with BOUNDS:");
    let mut parts = payload.split(',');
    let x = parts
        .next()
        .expect("missing bounds x")
        .parse::<i32>()
        .expect("invalid bounds x");
    let y = parts
        .next()
        .expect("missing bounds y")
        .parse::<i32>()
        .expect("invalid bounds y");
    let width = parts
        .next()
        .expect("missing bounds width")
        .parse::<i32>()
        .expect("invalid bounds width");
    let height = parts
        .next()
        .expect("missing bounds height")
        .parse::<i32>()
        .expect("invalid bounds height");
    (x, y, width, height)
}

#[test]
fn automation_headless_emits_ready_status_and_dump_frames() {
    let mut session = SimulatorSession::launch();

    session.send("?");
    let first_status = parse_status(&session.read_line());
    std::thread::sleep(Duration::from_millis(100));
    session.send("?");
    let second_status = parse_status(&session.read_line());
    assert!(
        second_status.0 > first_status.0,
        "tick count did not advance"
    );
    assert!(
        second_status.1 > first_status.1,
        "present count did not advance"
    );

    session.send("QE:disco.root");
    assert_eq!(session.read_line(), "EXISTS:1");

    session.send("QB:disco.settings.audio");
    assert_eq!(parse_bounds(&session.read_line()), (0, 0, 0, 0));

    session.send("T@disco.main.settings:760,50");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.settings.audio");
    let (_, _, width, height) = parse_bounds(&session.read_line());
    assert!(width > 0, "settings hotspot did not become visible");
    assert!(height > 0, "settings hotspot did not become visible");

    session.send("D0,0,4,2,1");
    assert_eq!(session.read_line(), "DUMP:queued");
    assert_eq!(session.read_line(), "F");
    let row_a = session.read_line();
    let row_b = session.read_line();
    assert!(
        row_a.split_whitespace().any(|pixel| pixel != "00000000")
            || row_b.split_whitespace().any(|pixel| pixel != "00000000"),
        "frame dump was unexpectedly blank"
    );
    assert_eq!(session.read_line(), "END");
}

#[test]
fn keyboard_opens_settings_wing() {
    let mut session = SimulatorSession::launch();

    // Initial state: settings wing collapsed
    session.send("QB:disco.settings.audio");
    assert_eq!(parse_bounds(&session.read_line()), (0, 0, 0, 0));

    // Send Enter to open settings wing (focus starts on Settings)
    session.send("KD:Enter");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.settings.audio");
    let (_, _, width, height) = parse_bounds(&session.read_line());
    assert!(width > 0, "settings wing did not open after Enter");
    assert!(height > 0);
}

#[test]
fn escape_closes_wing() {
    let mut session = SimulatorSession::launch();

    session.send("KD:Enter");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.settings.audio");
    let (_, _, width, _) = parse_bounds(&session.read_line());
    assert!(width > 0, "settings wing should be open");

    session.send("KD:Escape");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.settings.audio");
    assert_eq!(parse_bounds(&session.read_line()), (0, 0, 0, 0));
}

#[test]
fn hotkey_shortcuts_via_playit() {
    let mut session = SimulatorSession::launch();

    // 's' opens settings wing
    session.send("KD:s");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.settings.audio");
    let (_, _, width, _) = parse_bounds(&session.read_line());
    assert!(width > 0, "settings wing should be open after 's'");

    // Escape to close
    session.send("KD:Escape");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    // 'i' opens info wing
    session.send("KD:i");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    session.send("QB:disco.info.diagnostics");
    let (_, _, width, _) = parse_bounds(&session.read_line());
    assert!(width > 0, "info wing should be open after 'i'");
}

#[test]
fn all_tags_exist_at_startup() {
    let mut session = SimulatorSession::launch();

    let tags = [
        "disco.root",
        "disco.dashboard",
        "disco.subtitle",
        "disco.footer",
        "disco.events",
        "disco.main.settings",
        "disco.main.files",
        "disco.main.info",
        "disco.settings.audio",
        "disco.settings.camera",
        "disco.settings.display",
        "disco.settings.locale",
        "disco.settings.backlight",
        "disco.info.diagnostics",
        "disco.info.live_stats",
        "disco.info.star_crawl",
        "disco.info.audio_scope",
    ];

    for tag in tags {
        session.send(&format!("QE:{tag}"));
        assert_eq!(session.read_line(), "EXISTS:1", "missing tag: {tag}");
    }
}

#[test]
fn framebuffer_has_content_at_startup() {
    let mut session = SimulatorSession::launch();

    session.send("D0,0,40,20,1");
    assert_eq!(session.read_line(), "DUMP:queued");
    assert_eq!(session.read_line(), "F");

    let mut has_content = false;
    for _ in 0..20 {
        let row = session.read_line();
        if row.split_whitespace().any(|pixel| pixel != "00000000") {
            has_content = true;
        }
    }
    assert_eq!(session.read_line(), "END");
    assert!(has_content, "framebuffer was blank at startup");
}

#[test]
fn focus_highlight_moves_with_arrow_keys() {
    let mut session = SimulatorSession::launch();

    // Dump a small region over the icon strip area
    session.send("D740,10,40,20,1");
    assert_eq!(session.read_line(), "DUMP:queued");
    assert_eq!(session.read_line(), "F");
    let mut pixels_a = Vec::new();
    for _ in 0..20 {
        pixels_a.push(session.read_line());
    }
    assert_eq!(session.read_line(), "END");

    // Move focus down
    session.send("KD:ArrowDown");
    assert_eq!(session.read_line(), "OK");
    std::thread::sleep(Duration::from_millis(100));

    // Dump same region after focus moved
    session.send("D740,10,40,20,1");
    assert_eq!(session.read_line(), "DUMP:queued");
    assert_eq!(session.read_line(), "F");
    let mut pixels_b = Vec::new();
    for _ in 0..20 {
        pixels_b.push(session.read_line());
    }
    assert_eq!(session.read_line(), "END");

    // The pixel content should differ because the highlight border moved
    assert_ne!(pixels_a, pixels_b, "icon strip pixels should differ after focus change");
}
