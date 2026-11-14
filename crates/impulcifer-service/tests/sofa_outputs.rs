#![forbid(unsafe_code)]

mod brir_support;

use brir_support::*;
use impulcifer_jobs::registry::{JobRegistry, PollResult};
use impulcifer_types::{constants::HEXADECAGONAL_TRACK_ORDER, job::JobStatus, layouts::SLOTS};
use serde_json::{Value, json};
use sofar::reader::OpenOptions;
use std::{path::Path, process::Command};

fn run(temp: &Temp, request: Value) -> PollResult {
    let jobs = JobRegistry::new();
    let service = service(temp, jobs.clone());
    let result = service.call("start_brir", vec![request]);
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    poll
}

fn open(path: &Path, fs: u32) -> sofar::reader::Sofar {
    OpenOptions::new()
        .normalized(false)
        .sample_rate(fs as f32)
        .open(path)
        .unwrap()
}

fn speakers_in_hrir(track_count: usize) -> Vec<(&'static str, usize, usize)> {
    SLOTS
        .iter()
        .filter_map(|slot| {
            let left = HEXADECAGONAL_TRACK_ORDER
                .iter()
                .position(|name| *name == format!("{}-left", slot.code))?;
            let right = HEXADECAGONAL_TRACK_ORDER
                .iter()
                .position(|name| *name == format!("{}-right", slot.code))?;
            (left < track_count && right < track_count).then_some((slot.code, left, right))
        })
        .collect()
}

#[test]
fn sofa_holds_the_speakers_of_hrir_wav() {
    let temp = Temp::demo();
    let poll = run(&temp, json!({"dir_path":temp.0,"sofa":true}));
    let hrir = impulcifer_io::read_wav(&temp.0.join("hrir.wav")).unwrap();
    let speakers = speakers_in_hrir(hrir.tracks.len());
    let sofa = open(&temp.0.join("brir.sofa"), hrir.sample_rate);
    assert_eq!(sofa.num_measurements() as usize, speakers.len());
    assert_eq!(sofa.sample_rate(), hrir.sample_rate as f32);
    let hrtf = sofa.hrtf();
    let samples = sofa.filter_len();
    for (measurement, (code, left_track, right_track)) in speakers.iter().enumerate() {
        let slot = SLOTS.iter().find(|slot| slot.code == *code).unwrap();
        let azimuth = slot.azimuth.to_radians();
        let elevation = slot.elevation.to_radians();
        let expected = [
            (2.0 * elevation.cos() * azimuth.cos()) as f32,
            (2.0 * elevation.cos() * azimuth.sin()) as f32,
            (2.0 * elevation.sin()) as f32,
        ];
        for (actual, expected) in hrtf.source_position.values[3 * measurement..3 * measurement + 3]
            .iter()
            .zip(expected)
        {
            assert!((actual - expected).abs() < 1e-5, "{code} position");
        }
        for (ear, track) in [*left_track, *right_track].into_iter().enumerate() {
            let sofa_track = &hrtf.data_ir.values
                [(measurement * 2 + ear) * samples..(measurement * 2 + ear + 1) * samples];
            assert_eq!(sofa_track.len(), hrir.tracks[track].len());
            let worst = sofa_track
                .iter()
                .zip(&hrir.tracks[track])
                .map(|(sofa, wav)| (*sofa as f64 - wav).abs())
                .fold(0.0_f64, f64::max);
            assert!(worst <= 1.2e-7, "{code} ear {ear}: {worst}");
        }
    }
    assert!(
        poll.events
            .iter()
            .any(|event| event.payload["key"] == "cli_success_sofa")
    );
    assert!(temp.0.join("brir.sofa").is_file());
}

fn spectrum(track: &[f64], n: usize) -> Vec<f64> {
    let mut padded = track.to_vec();
    padded.resize(n, 0.0);
    impulcifer_dsp::fft::rfft(&padded)
        .into_iter()
        .map(|value| value.norm_sqr())
        .collect()
}

fn smooth_third_octave(power: &[f64], fs: u32, n: usize, frequency: f64) -> f64 {
    let low = frequency / 2.0_f64.powf(1.0 / 6.0);
    let high = frequency * 2.0_f64.powf(1.0 / 6.0);
    let lo = (low * n as f64 / fs as f64).ceil() as usize;
    let hi = ((high * n as f64 / fs as f64).floor() as usize).min(power.len() - 1);
    power[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64
}

fn worst_third_octave_db(
    final_ir: &[f64],
    split_ir: &[f64],
    eq: &[f64],
    fs: u32,
) -> (f64, f64, f64, f64) {
    let n = (split_ir.len() + eq.len() - 1)
        .max(final_ir.len())
        .next_power_of_two();
    let final_power = spectrum(final_ir, n);
    let split = spectrum(split_ir, n);
    let eq = spectrum(eq, n);
    let reconstructed: Vec<_> = split
        .iter()
        .zip(eq)
        .map(|(left, right)| left * right)
        .collect();
    let mut worst = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut frequency = 100.0_f64;
    while frequency <= 12_000.0 {
        let final_mean = smooth_third_octave(&final_power, fs, n, frequency);
        let reconstructed_mean = smooth_third_octave(&reconstructed, fs, n, frequency);
        let final_db = 10.0 * final_mean.max(f64::MIN_POSITIVE).log10();
        let reconstructed_db = 10.0 * reconstructed_mean.max(f64::MIN_POSITIVE).log10();
        let deviation = reconstructed_db - final_db;
        if deviation.abs() > worst.0.abs() {
            worst = (deviation, frequency, final_db, reconstructed_db);
        }
        frequency *= 1.01;
    }
    worst
}

#[test]
fn sofa_split_reassembles_the_final_brir() {
    let temp = Temp::demo();
    run(
        &temp,
        json!({"dir_path":temp.0,"sofa":true,"sofa_split_headphone_eq":true}),
    );
    for name in ["brir.sofa", "brir-no-headphone-eq.sofa", "headphone-eq.wav"] {
        assert!(temp.0.join(name).is_file(), "{name}");
    }
    let eq = impulcifer_io::read_wav(&temp.0.join("headphone-eq.wav")).unwrap();
    assert_eq!(eq.tracks.len(), 2);
    let final_sofa = open(&temp.0.join("brir.sofa"), eq.sample_rate);
    let split_sofa = open(&temp.0.join("brir-no-headphone-eq.sofa"), eq.sample_rate);
    assert_eq!(final_sofa.num_measurements(), split_sofa.num_measurements());
    let samples = final_sofa.filter_len();
    assert_eq!(samples, split_sofa.filter_len());
    let final_data = &final_sofa.hrtf().data_ir.values;
    let split_data = &split_sofa.hrtf().data_ir.values;
    let mut worst = (0.0_f64, String::new(), 0.0_f64, 0.0_f64, 0.0_f64);
    for (measurement, slot) in SLOTS
        .iter()
        .enumerate()
        .take(final_sofa.num_measurements() as usize)
    {
        let speaker = slot.code;
        for ear in 0..2_usize {
            let range = (measurement * 2 + ear) * samples..(measurement * 2 + ear + 1) * samples;
            let final_ir: Vec<_> = final_data[range.clone()]
                .iter()
                .map(|value| *value as f64)
                .collect();
            let split_ir: Vec<_> = split_data[range]
                .iter()
                .map(|value| *value as f64)
                .collect();
            let (deviation, frequency, final_db, reconstructed_db) =
                worst_third_octave_db(&final_ir, &split_ir, &eq.tracks[ear], eq.sample_rate);
            if deviation.abs() > worst.0.abs() {
                worst = (
                    deviation,
                    format!("{speaker} {}", if ear == 0 { "left" } else { "right" }),
                    frequency,
                    final_db,
                    reconstructed_db,
                );
            }
        }
    }
    println!(
        "MEASURE sofa split worst deviation: {:+.6} dB at {:.1} Hz ({}; final {:.3} dB, reconstructed {:.3} dB)",
        worst.0, worst.2, worst.1, worst.3, worst.4
    );
    assert!(
        worst.0.abs() <= 1.0,
        "worst split deviation {:+.6} dB at {:.1} Hz ({})",
        worst.0,
        worst.2,
        worst.1
    );
}

#[test]
fn sofa_split_without_headphone_compensation_saves_only_brir_sofa() {
    let temp = Temp::demo();
    let poll = run(
        &temp,
        json!({"dir_path":temp.0,"sofa":true,"sofa_split_headphone_eq":true,"do_headphone_compensation":false}),
    );
    assert!(temp.0.join("brir.sofa").is_file());
    assert!(!temp.0.join("brir-no-headphone-eq.sofa").exists());
    assert!(!temp.0.join("headphone-eq.wav").exists());
    assert!(
        poll.events
            .iter()
            .any(|event| event.payload["key"] == "cli_info_sofa_split_skipped")
    );
}

#[test]
fn sofa_off_writes_no_sofa_file() {
    let temp = Temp::demo();
    run(&temp, json!({"dir_path":temp.0}));
    assert!(!temp.0.join("brir.sofa").exists());
    assert!(!temp.0.join("brir-no-headphone-eq.sofa").exists());
    assert!(!temp.0.join("headphone-eq.wav").exists());
}

#[test]
fn sofa_epoch_child() {
    let Some(output) = std::env::var_os("IMPULCIFER_SOFA_EPOCH_OUTPUT") else {
        return;
    };
    let temp = Temp::demo();
    run(&temp, json!({"dir_path":temp.0,"sofa":true}));
    std::fs::copy(temp.0.join("brir.sofa"), output).unwrap();
}

#[test]
fn sofa_files_are_reproducible_with_source_date_epoch() {
    let temp = Temp::new();
    let first = temp.0.join("first.sofa");
    let second = temp.0.join("second.sofa");
    for output in [&first, &second] {
        let status = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("sofa_epoch_child")
            .arg("--nocapture")
            .env("SOURCE_DATE_EPOCH", "1763078400")
            .env("IMPULCIFER_SOFA_EPOCH_OUTPUT", output)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let first = std::fs::read(first).unwrap();
    assert_eq!(first, std::fs::read(second).unwrap());
    assert!(
        first
            .windows(b"2025-11-14 00:00:00".len())
            .any(|window| window == b"2025-11-14 00:00:00")
    );
}
