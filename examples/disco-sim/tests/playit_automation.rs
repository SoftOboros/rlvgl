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

// ── Screen rendering regression tests ──────────────────────────────────
//
// These tests freeze the simulator's pixel-level output for three
// behaviours that have regressed in the past:
//
// 1. The dashboard panel rounded corners must trace a convex
//    quarter-circle (not the inverted "L-bracket" shape).
// 2. Widgets with fully transparent backgrounds (alpha=0) must NOT
//    overwrite the underlying framebuffer with zero pixels — they
//    used to render as solid black bars on the wgpu sRGB surface.
// 3. The platform `ColorFormat` profile must end-to-end quantize the
//    PNG output so an Argb8888 snapshot differs from a Mono snapshot.

/// Parse a row of `D` dump pixels into 24-bit (R, G, B) tuples,
/// dropping the alpha channel. The dump format is the packed u32
/// printed in `AARRGGBB` order.
fn parse_pixels(line: &str) -> Vec<(u8, u8, u8)> {
    line.split_whitespace()
        .map(|hex| {
            let v = u32::from_str_radix(hex, 16).expect("invalid pixel hex");
            (((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
        })
        .collect()
}

/// Brightness (sum of channels) used by the corner-shape test below.
fn brightness(p: (u8, u8, u8)) -> u32 {
    p.0 as u32 + p.1 as u32 + p.2 as u32
}

#[test]
fn dashboard_rounded_corner_arcs_outward() {
    // Regression for the bug where draw_rounded_border called
    // arc_dx(r, dy) with dy as a row index instead of an axis distance,
    // producing a triangle that cut INTO the corner. With the fix the
    // top-left arc tangent sits near the top-right of the corner box
    // and curves down to the bottom-left.
    //
    // Dashboard panel is at PANEL_X=84, PANEL_Y=84, radius 18.
    let mut session = SimulatorSession::launch();

    // Dump a 20x20 region from the panel's top-left corner.
    session.send("D84,84,20,20,1");
    assert_eq!(session.read_line(), "DUMP:queued");
    assert_eq!(session.read_line(), "F");
    let mut rows = Vec::with_capacity(20);
    for _ in 0..20 {
        rows.push(parse_pixels(&session.read_line()));
    }
    assert_eq!(session.read_line(), "END");

    // Border colour PANEL_BORDER = (75, 94, 122) ≈ brightness 291;
    // panel fill ≈ brightness 92; window bg ≈ brightness 62. Pick a
    // mid threshold that catches border pixels and AA fringe.
    let border_threshold = 200u32;
    let bright_cols_in_row = |row: &[(u8, u8, u8)]| -> Vec<usize> {
        row.iter()
            .enumerate()
            .filter_map(|(i, p)| (brightness(*p) >= border_threshold).then_some(i))
            .collect()
    };

    // Top row (y=84): the arc is tangent to the top edge near col 18,
    // so the brightest pixels should sit in the right half of the
    // 20-wide window, NOT at the left edge.
    let top_row = bright_cols_in_row(&rows[0]);
    assert!(
        !top_row.is_empty(),
        "no border pixels visible in top row of corner: {:?}",
        rows[0]
    );
    let min_top = *top_row.iter().min().unwrap();
    assert!(
        min_top >= 12,
        "top row arc is too far left (col {min_top}); the corner is \
         drawn upside-down. Bright cols: {top_row:?}"
    );

    // Bottom row of the arc region (y=84+17=101): the arc reaches the
    // left edge, so brightest pixels should be at small column indices.
    let bottom_row = bright_cols_in_row(&rows[17]);
    assert!(
        !bottom_row.is_empty(),
        "no border pixels visible in bottom row of corner: {:?}",
        rows[17]
    );
    let min_bot = *bottom_row.iter().min().unwrap();
    assert!(
        min_bot <= 5,
        "bottom row arc is too far right (col {min_bot}); the corner \
         is drawn upside-down. Bright cols: {bottom_row:?}"
    );
}

#[test]
fn transparent_label_backgrounds_do_not_blacken_window() {
    // Regression for the disco sim "black bars" bug. themed_label()
    // creates labels with bg_color = Color(0,0,0,0) and style.alpha = 0,
    // which used to call fill_rect with a fully transparent colour.
    // CpuBlitter overwrote the destination with 0x00000000 instead of
    // leaving the underlying pixels alone, producing solid black bars
    // where the title/subtitle/footer labels live.
    //
    // After the fix the label bands should be the dark navy window
    // background (Color(13, 19, 30) = 0xFF0D131E), not 0x00000000.
    let mut session = SimulatorSession::launch();

    // Title sits at y=24..42 (themed_label rect at lib.rs:712-717).
    // Subtitle at y=48..66. Footer at the bottom (y = h - 32 .. h - 14).
    // Sample inside each band, well to the left of the panel border
    // and to the right of the screen edge.
    //
    // The dump command caps width and height at 40 each.
    let sample_xs: [(i32, &str); 3] = [(110, "title"), (110, "subtitle"), (250, "footer")];
    let sample_ys: [(i32, &str); 3] = [(30, "title"), (54, "subtitle"), (450, "footer")];

    // Window background should be a non-zero "dark navy" colour with
    // sum of channels around 62. We just need to ensure pixels are
    // NOT 0x00000000 — that's the regression.
    let dump_command = |y: i32| format!("D110,{y},20,1,1");

    for ((_, label), (y, _)) in sample_xs.iter().zip(sample_ys.iter()) {
        session.send(&dump_command(*y));
        assert_eq!(session.read_line(), "DUMP:queued");
        assert_eq!(session.read_line(), "F");
        let row = session.read_line();
        assert_eq!(session.read_line(), "END");

        let pixels = parse_pixels(&row);
        assert!(
            pixels.iter().all(|p| brightness(*p) > 0),
            "{label} band at y={y} contains zero pixels: {pixels:?}"
        );
    }
}

#[test]
fn color_format_cli_changes_png_output() {
    // Regression test for the Screen::color_format pipeline. Spawning
    // the simulator with --color=argb and --color=mono should produce
    // PNG snapshots that differ at the byte level, because the mono
    // path runs every pixel through ColorFormat::Mono::quantize.
    //
    // We don't compare against a golden image (anti-aliasing across
    // wgpu adapters makes that brittle); we just confirm that the
    // colour profile actually flows from the CLI through the headless
    // path into the saved PNG.
    use std::fs;
    use std::process::Command;

    let bin = SimulatorSession::binary_path();
    let dir = std::env::temp_dir();
    let argb_path = dir.join("rlvgl_test_color_argb.png");
    let mono_path = dir.join("rlvgl_test_color_mono.png");

    let _ = fs::remove_file(&argb_path);
    let _ = fs::remove_file(&mono_path);

    let run = |fmt: &str, path: &std::path::Path| {
        let status = Command::new(&bin)
            .arg(format!("--color={fmt}"))
            .arg(path)
            .status()
            .expect("failed to spawn disco-sim for headless dump");
        assert!(
            status.success(),
            "disco-sim --color={fmt} exited with status {status:?}"
        );
    };

    run("argb", &argb_path);
    run("mono", &mono_path);

    let argb_bytes = fs::read(&argb_path).expect("failed to read argb png");
    let mono_bytes = fs::read(&mono_path).expect("failed to read mono png");

    assert!(!argb_bytes.is_empty(), "argb png is empty");
    assert!(!mono_bytes.is_empty(), "mono png is empty");
    assert_ne!(
        argb_bytes, mono_bytes,
        "argb and mono color profiles produced byte-identical PNGs; \
         the --color CLI flag is not flowing into the headless path"
    );

    // Best-effort cleanup; ignore errors.
    let _ = fs::remove_file(&argb_path);
    let _ = fs::remove_file(&mono_path);
}
