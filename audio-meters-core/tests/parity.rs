//! Cross-runtime parity fixtures.
//!
//! For each `(input, ballistic)` pair, we compute the per-frame reading and
//! either:
//!
//! - Compare against a committed expected file under
//!   `fixtures/expected/<input_name>__<ballistic>.json`, OR
//! - With `RLVGL_AUDIO_METERS_REGENERATE=1`, **write** the expected file
//!   from the current Rust output.
//!
//! The TypeScript port (`@rlvgl/audio-meters`, AM-02) consumes the same
//! `inputs/` files and asserts against the same `expected/` files. Any
//! divergence between the two implementations breaks one side's CI.
//!
//! Tolerance is `1e-4` dB — float-trig (`expf`, `log10f`, `exp10f`)
//! determinism between Rust `libm` and TS `Math` is not bit-identical, so
//! we allow a small epsilon. If TS-side parity creeps wider than this, the
//! correct fix is to identify the divergence (often: order-of-operations
//! in the running-mean update) rather than relax the tolerance.

use std::{env, fs, path::PathBuf};

use rlvgl_audio_meters_core::{Ballistic, BallisticState};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    name: String,
    #[allow(dead_code)]
    description: String,
    frame_dt_s: f32,
    frames: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct Expected {
    input: String,
    ballistic: String,
    frame_dt_s: f32,
    readings_db: Vec<f32>,
}

const TOLERANCE_DB: f32 = 1e-4;

const ALL_BALLISTICS: &[(Ballistic, &str)] = &[
    (Ballistic::Vu, "Vu"),
    (Ballistic::PpmTypeI, "PpmTypeI"),
    (Ballistic::PpmTypeIIa, "PpmTypeIIa"),
    (Ballistic::PpmTypeIIb, "PpmTypeIIb"),
    (Ballistic::DigitalPeak, "DigitalPeak"),
    (Ballistic::Rms, "Rms"),
    (Ballistic::LufsM, "LufsM"),
    (Ballistic::LufsS, "LufsS"),
    (Ballistic::LufsI, "LufsI"),
    (Ballistic::Instant, "Instant"),
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn load_inputs() -> Vec<Input> {
    let dir = fixtures_dir().join("inputs");
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .expect("fixtures/inputs missing")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap_or_else(|_| panic!("read {}", p.display()));
            serde_json::from_str::<Input>(&text)
                .unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
        })
        .collect()
}

fn run_ballistic(kind: Ballistic, input: &Input) -> Vec<f32> {
    let mut s = BallisticState::new(kind);
    input
        .frames
        .iter()
        .map(|&db| s.update(db, input.frame_dt_s))
        .collect()
}

fn expected_path(input_name: &str, ballistic_name: &str) -> PathBuf {
    fixtures_dir()
        .join("expected")
        .join(format!("{input_name}__{ballistic_name}.json"))
}

#[test]
fn cross_runtime_parity_fixtures() {
    let regenerate = env::var("RLVGL_AUDIO_METERS_REGENERATE")
        .map(|v| v == "1")
        .unwrap_or(false);
    let inputs = load_inputs();
    assert!(!inputs.is_empty(), "no fixtures under fixtures/inputs/");

    let mut compared = 0usize;
    for input in &inputs {
        for &(kind, name) in ALL_BALLISTICS {
            let readings = run_ballistic(kind, input);
            let path = expected_path(&input.name, name);

            if regenerate || !path.exists() {
                let exp = Expected {
                    input: input.name.clone(),
                    ballistic: name.to_string(),
                    frame_dt_s: input.frame_dt_s,
                    readings_db: readings.clone(),
                };
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                let text = serde_json::to_string_pretty(&exp).unwrap();
                fs::write(&path, text).expect("write expected file");
                if !regenerate {
                    eprintln!("[parity] generated initial expected: {}", path.display());
                }
                continue;
            }

            let text = fs::read_to_string(&path).expect("read expected file");
            let exp: Expected = serde_json::from_str(&text).expect("parse expected file");
            assert_eq!(exp.input, input.name);
            assert_eq!(exp.ballistic, name);
            assert!(
                (exp.frame_dt_s - input.frame_dt_s).abs() < 1e-9,
                "frame_dt mismatch in {}",
                path.display()
            );
            assert_eq!(
                exp.readings_db.len(),
                readings.len(),
                "frame-count mismatch in {}",
                path.display()
            );
            for (i, (got, want)) in readings.iter().zip(exp.readings_db.iter()).enumerate() {
                let delta = (got - want).abs();
                assert!(
                    delta <= TOLERANCE_DB,
                    "{}/{} frame {}: got {got:.6} want {want:.6} Δ={delta:.6}",
                    input.name,
                    name,
                    i
                );
            }
            compared += 1;
        }
    }

    if compared == 0 {
        // First run — fixtures were just generated. Fail loudly so the
        // commit-time generator step is intentional, but include a
        // helpful message.
        panic!(
            "no expected fixtures existed; initial set has been written. \
             Re-run the test (without RLVGL_AUDIO_METERS_REGENERATE) to \
             confirm parity, then commit fixtures/expected/."
        );
    }
}
