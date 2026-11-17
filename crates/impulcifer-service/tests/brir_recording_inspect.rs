#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use serde_json::{Value, json};

fn log_sweep(fs: u32, seconds: f64, low: f64, high: f64) -> Vec<f64> {
    let length = (seconds * f64::from(fs)).round() as usize;
    let ratio = high / low;
    let scale = 2.0 * std::f64::consts::PI * low * seconds / ratio.ln();
    (0..length)
        .map(|index| {
            let time = index as f64 / f64::from(fs);
            (scale * (ratio.powf(time / seconds) - 1.0)).sin()
        })
        .collect()
}

fn linear_sweep(fs: u32, seconds: f64, low: f64, high: f64) -> Vec<f64> {
    let length = (seconds * f64::from(fs)).round() as usize;
    let slope = (high - low) / seconds;
    (0..length)
        .map(|index| {
            let time = index as f64 / f64::from(fs);
            (2.0 * std::f64::consts::PI * (low * time + 0.5 * slope * time * time)).sin()
        })
        .collect()
}

fn write_sequence(
    temp: &Temp,
    name: &str,
    fs: u32,
    sweep: &[f64],
    starts: &[usize],
) -> std::path::PathBuf {
    let length = starts.last().copied().unwrap_or_default() + sweep.len() + fs as usize;
    let mut tracks = vec![vec![0.0; length]; 2];
    for (sequence, start) in starts.iter().copied().enumerate() {
        for (ear, track) in tracks.iter_mut().enumerate() {
            let delay = 3 + sequence + 2 * ear;
            let gain = 1.0 - 0.1 * sequence as f64;
            for (target, sample) in track[start + delay..].iter_mut().zip(sweep) {
                *target += gain * sample;
            }
        }
    }
    let path = temp.0.join(name);
    impulcifer_io::write_wav(&path, fs, &tracks, 32).unwrap();
    path
}

fn inspect(temp: &Temp, request: Value) -> Value {
    service(temp, JobRegistry::new()).call("inspect_brir_recording", vec![request])
}

#[test]
fn inspect_brir_recording_counts_the_sweeps_of_a_sweep_file() {
    let temp = Temp::new();
    let fs = 8_000;
    let sweep = log_sweep(fs, 1.0, 10.0, 3_500.0);
    let starts = [1_000, 12_500, 26_000];
    let recording = write_sequence(&temp, "two-ear.wav", fs, &sweep, &starts);
    let sweep_path = temp.0.join("played.wav");
    impulcifer_io::write_wav(&sweep_path, fs, std::slice::from_ref(&sweep), 32).unwrap();

    let result = inspect(
        &temp,
        json!({"recording_file": recording, "recording_sweep": sweep_path}),
    );
    assert_eq!(result["ok"], true, "{result}");
    let data = &result["data"];
    assert_eq!(data["found"], starts.len());
    assert_eq!(data["reference"], "file");
    assert_eq!(data["fs"], fs);
    assert_eq!(data["sweep_seconds"], 1.0);
    let found = data["starts_seconds"].as_array().unwrap();
    for (actual, expected) in found.iter().zip(starts) {
        let actual = actual.as_f64().unwrap();
        let expected = expected as f64 / f64::from(fs);
        assert!((actual - expected).abs() <= 0.02, "{actual} vs {expected}");
    }
}

#[test]
fn inspect_brir_recording_uses_the_test_signal_without_a_sweep_file() {
    let temp = Temp::new();
    let fs = 8_000;
    let estimator = impulcifer_dsp::estimator::SweepEstimator::new(1.0, fs).unwrap();
    let gap = 2 * estimator.test_signal.len();
    let starts = [2_000, 2_000 + gap, 4_000 + 2 * gap];
    let recording = write_sequence(&temp, "generated.wav", fs, &estimator.test_signal, &starts);
    let result = inspect(
        &temp,
        json!({
            "recording_file": recording,
            "test_signal": "  generate:1.0s@8000  ",
            "dir_path": temp.0.join("missing-folder")
        }),
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["data"]["found"], starts.len());
    assert_eq!(result["data"]["reference"], "test_signal");
    let found = result["data"]["starts_seconds"].as_array().unwrap();
    for (actual, expected) in found.iter().zip(starts) {
        let actual = actual.as_f64().unwrap();
        let expected = expected as f64 / f64::from(fs);
        assert!((actual - expected).abs() <= 0.02, "{actual} vs {expected}");
    }
}

#[test]
fn inspect_brir_recording_reports_zero_and_errors() {
    let temp = Temp::new();
    let fs = 8_000;
    let sweep = log_sweep(fs, 1.0, 10.0, 3_500.0);
    let sweep_path = temp.0.join("played.wav");
    impulcifer_io::write_wav(&sweep_path, fs, std::slice::from_ref(&sweep), 32).unwrap();
    let silent = temp.0.join("silent.wav");
    impulcifer_io::write_wav(&silent, fs, &vec![vec![0.0; 24_000]; 2], 32).unwrap();

    let zero = inspect(
        &temp,
        json!({"recording_file": silent, "recording_sweep": sweep_path}),
    );
    assert_eq!(zero["ok"], true, "{zero}");
    assert_eq!(zero["data"]["found"], 0);
    assert_eq!(zero["data"]["starts_seconds"], json!([]));

    let missing_path = temp.0.join("missing.wav");
    let missing = inspect(
        &temp,
        json!({"recording_file": missing_path, "recording_sweep": sweep_path}),
    );
    assert_eq!(missing["error"]["code"], "FILE_NOT_FOUND", "{missing}");
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing.wav")
    );

    let recording = write_sequence(&temp, "sequence.wav", fs, &sweep, &[1_000, 12_000]);
    let wrong_rate = temp.0.join("wrong-rate.wav");
    impulcifer_io::write_wav(&wrong_rate, 44_100, std::slice::from_ref(&sweep), 32).unwrap();
    let rate = inspect(
        &temp,
        json!({"recording_file": recording, "recording_sweep": wrong_rate}),
    );
    assert_eq!(rate["error"]["code"], "INVALID_REQUEST", "{rate}");
    assert_eq!(
        rate["error"]["message"],
        "The sweep file and the recording have different sampling rates."
    );
    assert_eq!(rate["error"]["details"]["sweep_fs"], 44_100);
    assert_eq!(rate["error"]["details"]["recording_fs"], fs);

    let linear = temp.0.join("linear.wav");
    impulcifer_io::write_wav(&linear, fs, &[linear_sweep(fs, 1.0, 10.0, 3_500.0)], 32).unwrap();
    let bad_sweep = inspect(
        &temp,
        json!({"recording_file": recording, "recording_sweep": linear}),
    );
    assert_eq!(bad_sweep["error"]["code"], "INVALID_REQUEST");
    assert_eq!(
        bad_sweep["error"]["message"],
        "The sweep file is not a logarithmic sweep."
    );

    for request in [json!({}), json!({"recording_file":"  "}), json!("file.wav")] {
        let invalid = inspect(&temp, request);
        assert_eq!(invalid["error"]["code"], "INVALID_REQUEST", "{invalid}");
    }
}
