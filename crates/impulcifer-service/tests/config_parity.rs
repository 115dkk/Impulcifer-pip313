#![forbid(unsafe_code)]
//! P25: the ProcessingConfig fields that the p10/p11 demo goldens leave at
//! their defaults, one service run each on a demo copy, held to the real 2.x
//! pipeline (tests/migration/export_goldens_config.py) with the demo_parity
//! budget: per hesuvi.wav track max |a - b| <= 1e-3 * max |ref| over the first
//! 512 and last 256 samples, max-abs and RMS ratios within 1e-4 and the same
//! absolute-peak index; hrir.wav the same without the sample windows.
//! Runs without headphone compensation use a wider ratio budget, see
//! `ratio_budget` and tests/migration/README-config.md.
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{Catalog, run::run_brir};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobKind, JobStatus},
};
use serde_json::{Value, json};
use std::path::Path;

fn fixture() -> Value {
    serde_json::from_slice(&std::fs::read(golden("p25_config_scenarios.json")).unwrap()).unwrap()
}

/// "{outside}" in a config value or a setup path is the second temporary
/// directory, as in the exporter.
fn expand(value: &Value, outside: &Path) -> Value {
    match value {
        Value::String(s) => json!(s.replace("{outside}", &outside.to_string_lossy())),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), expand(v, outside)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn prepare(dir: &Path, outside: &Path, spec: &Value) {
    for name in spec["remove"].as_array().unwrap() {
        std::fs::remove_file(dir.join(name.as_str().unwrap())).unwrap();
    }
    for pair in spec["move"].as_array().unwrap() {
        let target = expand(&pair[1], outside);
        std::fs::rename(
            dir.join(pair[0].as_str().unwrap()),
            target.as_str().unwrap(),
        )
        .unwrap();
    }
    for (target, text) in spec["write"].as_object().unwrap() {
        std::fs::write(
            expand(&json!(target), outside).as_str().unwrap(),
            text.as_str().unwrap(),
        )
        .unwrap();
    }
    if let Some(generic) = spec["setup"].get("generic_room") {
        // p10 generic-room construction: the first track of FL,FR.wav and a
        // copy rolled by 17 samples, written as PCM_32 like soundfile does.
        let source =
            impulcifer_io::read_wav(&dir.join(generic["source"].as_str().unwrap())).unwrap();
        let first = source.tracks[0][..generic["frames"].as_u64().unwrap() as usize].to_vec();
        let mut second = first.clone();
        second.rotate_right(generic["roll"].as_u64().unwrap() as usize);
        impulcifer_io::write_wav(
            &dir.join("room.wav"),
            source.sample_rate,
            &[first, second],
            32,
        )
        .unwrap();
    }
}

fn ratio(v: f64, r: f64) -> f64 {
    if r == 0.0 {
        if v == 0.0 { 1.0 } else { f64::INFINITY }
    } else {
        v / r
    }
}

/// Without headphone compensation the EQ FIR carries only the room
/// correction, and the 2.x output moves by itself: perturbing its linear-phase
/// FIRs by 1e-12 of the peak before `minimum_phase` (the size of the
/// Rust/Python `firwin2` difference, tests/migration/README-fr.md) moved the
/// max-abs ratio by up to 2.2e-4 without headphone compensation and 5.6e-4
/// with mic deviation correction, and the RMS ratio by up to 1.6e-4, over ten
/// trials each (tests/migration/oracle_noise_config.py). A 1e-4 ratio budget
/// is inside the oracle's own noise there, so those runs get 5e-4; every other
/// scenario keeps 1e-4.
fn ratio_budget(spec: &Value) -> f64 {
    if spec["config"]["do_headphone_compensation"] == json!(false) {
        5e-4
    } else {
        1e-4
    }
}

fn scenario(name: &str) {
    let fixture = fixture();
    let spec = &fixture["scenarios"][name];
    assert!(spec.is_object(), "no P25 scenario {name}");
    let dir = Temp::demo();
    let outside = Temp::new();
    prepare(&dir.0, &outside.0, spec);
    let mut config: ProcessingConfig =
        serde_json::from_value(expand(&spec["config"], &outside.0)).unwrap();
    config.dir_path = Some(dir.0.to_string_lossy().into_owned());
    config.test_signal = Some(
        root()
            .join(fixture["meta"]["sweep"].as_str().unwrap())
            .to_string_lossy()
            .into_owned(),
    );
    let jobs = JobRegistry::new();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            let run = run_brir(&config, &Catalog::english(), ctx)?;
            Ok(json!({"output_path":run.output_path}))
        })
        .unwrap();
    let poll = wait(&jobs, &job.job_id);
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{name}: {:?}",
        poll.job.error
    );
    let window = &fixture["meta"]["window"];
    let (head, tail) = (
        window[0].as_u64().unwrap() as usize,
        window[1].as_u64().unwrap() as usize,
    );
    let samples: Vec<f64> = std::fs::read(golden(spec["windows"].as_str().unwrap()))
        .unwrap()
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let budget = ratio_budget(spec);
    let mut failures = Vec::new();
    for filename in ["hesuvi.wav", "hrir.wav"] {
        let product = &spec["products"][filename];
        let wav = impulcifer_io::read_wav(&dir.0.join(filename)).unwrap();
        assert_eq!(
            wav.sample_rate as u64,
            product["fs"].as_u64().unwrap(),
            "{name} {filename}"
        );
        let expected = product["tracks"].as_array().unwrap();
        assert_eq!(wav.tracks.len(), expected.len(), "{name} {filename} tracks");
        for (index, (track, reference)) in wav.tracks.iter().zip(expected).enumerate() {
            let label = reference["name"].as_str().unwrap();
            assert_eq!(
                track.len() as u64,
                product["frames"].as_u64().unwrap(),
                "{name} {filename} {label} frames"
            );
            let max = track.iter().map(|x| x.abs()).fold(0.0, f64::max);
            let rms = (track.iter().map(|x| x * x).sum::<f64>() / track.len() as f64).sqrt();
            let peak = track
                .iter()
                .enumerate()
                .max_by(|(i, a), (j, b)| a.abs().total_cmp(&b.abs()).then_with(|| j.cmp(i)))
                .unwrap()
                .0;
            let refmax = reference["max_abs"].as_f64().unwrap();
            let refrms = reference["rms"].as_f64().unwrap();
            let mut error = 0.0_f64;
            if filename == "hesuvi.wav" {
                let got = track[..head].iter().chain(&track[track.len() - tail..]);
                let want = &samples[index * (head + tail)..(index + 1) * (head + tail)];
                for (a, b) in got.zip(want) {
                    error = error.max((a - b).abs());
                }
            }
            println!(
                "{name} {filename} {label}: max_error={error:.3e} max_ratio={:.9} rms_ratio={:.9} peak={peak}/{}",
                ratio(max, refmax),
                ratio(rms, refrms),
                reference["peak_index"]
            );
            if error > 1e-3 * refmax
                || (ratio(max, refmax) - 1.0).abs() > budget
                || (ratio(rms, refrms) - 1.0).abs() > budget
                || peak as u64 != reference["peak_index"].as_u64().unwrap()
            {
                failures.push(format!("{filename}/{label}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "P25 {name} parity failures: {failures:?}"
    );
}

/// Every exported scenario has its own test below (features.toml cites them).
const SCENARIOS: [&str; 18] = [
    "head_ms",
    "bass_boost_shelf",
    "vbass_crossover",
    "vbass_normal",
    "vbass_invert",
    "mic_deviation",
    "mic_deviation_strength",
    "no_room_correction",
    "no_headphone_compensation",
    "no_equalization",
    "decay",
    "channel_balance",
    "room_files",
    "room_target_missing",
    "headphone_file",
    "specific_unlimited",
    "generic_conservative",
    "generic_unlimited",
];

#[test]
fn config_scenarios_are_all_checked() {
    let fixture = fixture();
    let mut exported: Vec<_> = fixture["scenarios"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    exported.sort_unstable();
    let mut checked = SCENARIOS.to_vec();
    checked.sort_unstable();
    assert_eq!(exported, checked);
}
#[test]
fn config_head_ms_matches_python() {
    scenario("head_ms");
}
#[test]
fn config_bass_boost_shelf_matches_python() {
    scenario("bass_boost_shelf");
}
#[test]
fn config_vbass_crossover_and_highpass_match_python() {
    scenario("vbass_crossover");
}
#[test]
fn config_vbass_normal_polarity_matches_python() {
    scenario("vbass_normal");
}
#[test]
fn config_vbass_inverted_polarity_matches_python() {
    scenario("vbass_invert");
}
#[test]
fn config_mic_deviation_matches_python() {
    scenario("mic_deviation");
}
#[test]
fn config_mic_deviation_strength_matches_python() {
    scenario("mic_deviation_strength");
}
#[test]
fn config_without_room_correction_matches_python() {
    scenario("no_room_correction");
}
#[test]
fn config_without_headphone_compensation_matches_python() {
    scenario("no_headphone_compensation");
}
#[test]
fn config_without_equalization_matches_python() {
    scenario("no_equalization");
}
#[test]
fn config_decay_matches_python() {
    scenario("decay");
}
#[test]
fn config_channel_balance_matches_python() {
    scenario("channel_balance");
}
#[test]
fn config_room_target_and_calibration_files_match_python() {
    scenario("room_files");
}
#[test]
fn config_missing_room_target_is_flat_like_python() {
    scenario("room_target_missing");
}
#[test]
fn config_headphone_file_outside_the_folder_matches_python() {
    scenario("headphone_file");
}
#[test]
fn config_specific_room_unlimited_matches_python() {
    scenario("specific_unlimited");
}
#[test]
fn config_generic_room_conservative_matches_python() {
    scenario("generic_conservative");
}
#[test]
fn config_generic_room_unlimited_matches_python() {
    scenario("generic_unlimited");
}
