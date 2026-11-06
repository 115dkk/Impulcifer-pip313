#![forbid(unsafe_code)]

mod brir_support;
use brir_support::*;
use impulcifer_io::{read_wav, write_wav};
use impulcifer_jobs::registry::{JobRegistry, PollResult};
use impulcifer_types::{constants::SPEAKER_NAMES, job::JobStatus};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn run(temp: &Temp, request: Value) -> PollResult {
    let jobs = JobRegistry::new();
    let service = service(temp, jobs.clone());
    let started = service.call("start_brir", vec![request]);
    assert_eq!(started["ok"], true, "{started}");
    let poll = wait(&jobs, started["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    poll
}

fn alignment(poll: &PollResult) -> &Value {
    &poll.job.result.as_ref().unwrap()["alignment"]
}

fn has_log(poll: &PollResult, key: &str) -> bool {
    poll.events.iter().any(|event| event.payload["key"] == key)
}

#[test]
fn brir_result_carries_the_alignment_report() {
    let temp = Temp::demo();
    let poll = run(&temp, json!({"dir_path":temp.0}));
    let result = poll.job.result.as_ref().unwrap();
    eprintln!(
        "DEMO_RESULT_JSON={}",
        serde_json::to_string(result).unwrap()
    );
    let report = &result["alignment"];
    assert_eq!(report["mode"], "off");
    let speakers = report["speakers"].as_array().unwrap();
    assert_eq!(speakers.len(), 7);
    assert_eq!(
        speakers
            .iter()
            .map(|speaker| speaker["speaker"].as_str().unwrap())
            .collect::<Vec<_>>(),
        SPEAKER_NAMES[..7]
    );
    for speaker in speakers {
        assert!(speaker["measured_us"].is_number(), "{speaker}");
        assert!(speaker["after_us"].is_number(), "{speaker}");
        assert!(speaker["target_us"].is_null(), "{speaker}");
    }
    for key in [
        "input_balance_db",
        "headphone_balance_db",
        "predicted_balance_db",
    ] {
        assert!(report[key].is_number(), "{key}: {report}");
    }
    assert!(has_log(&poll, "cli_itd_report"));
    assert!(has_log(&poll, "cli_itd_pairing") || has_log(&poll, "cli_itd_pairing_warning"));
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    assert!(readme.contains("## Interaural time differences"));
}

#[test]
fn itd_symmetric_on_the_demo_makes_the_pairs_symmetric() {
    let off = Temp::demo();
    let off_poll = run(&off, json!({"dir_path":off.0}));
    assert_eq!(alignment(&off_poll)["mode"], "off");
    let off_wav = std::fs::read(off.0.join("hesuvi.wav")).unwrap();

    let corrected = Temp::demo();
    let poll = run(
        &corrected,
        json!({"dir_path":corrected.0,"itd_mode":"symmetric"}),
    );
    assert_eq!(alignment(&poll)["mode"], "symmetric");
    let speakers = alignment(&poll)["speakers"].as_array().unwrap();
    let after = |code: &str| {
        speakers
            .iter()
            .find(|speaker| speaker["speaker"] == code)
            .unwrap()["after_us"]
            .as_i64()
            .unwrap()
    };
    for (left, right) in [("FL", "FR"), ("SL", "SR"), ("BL", "BR")] {
        assert!((after(left) + after(right)).abs() <= 3, "{left}/{right}");
    }
    assert!(after("FC").abs() <= 3);
    assert!(has_log(&poll, "cli_itd_corrected"));
    assert_ne!(
        std::fs::read(corrected.0.join("hesuvi.wav")).unwrap(),
        off_wav
    );
}

#[test]
fn itd_off_output_is_byte_identical_to_today() {
    let root = root().canonicalize().unwrap();
    let baseline = root
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target/release/impulcifer.exe");
    if !baseline.is_file() {
        eprintln!("SKIP baseline CLI is absent: {}", baseline.display());
        return;
    }
    let current = Temp::demo();
    run(&current, json!({"dir_path":current.0,"itd_mode":"off"}));
    let reference = Temp::demo();
    let status = Command::new(&baseline)
        .arg("--dir_path")
        .arg(&reference.0)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::read(current.0.join("hesuvi.wav")).unwrap(),
        std::fs::read(reference.0.join("hesuvi.wav")).unwrap()
    );
}

fn scale_track(path: &Path, track: usize, gain_db: f64) {
    let mut wav = read_wav(path).unwrap();
    let gain = 10_f64.powf(gain_db / 20.0);
    for sample in &mut wav.tracks[track] {
        *sample *= gain;
    }
    write_wav(path, wav.sample_rate, &wav.tracks, 32).unwrap();
}

#[test]
fn pairing_warning_when_the_inputs_were_swapped_between_the_recordings() {
    let control = Temp::demo();
    let control_poll = run(&control, json!({"dir_path":control.0}));
    let before = alignment(&control_poll)["predicted_balance_db"]
        .as_f64()
        .unwrap();

    let temp = Temp::demo();
    for name in ["FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav"] {
        scale_track(&temp.0.join(name), 0, -6.5);
    }
    scale_track(&temp.0.join("headphones.wav"), 1, -7.0);
    let poll = run(&temp, json!({"dir_path":temp.0}));
    let report = alignment(&poll);
    let predicted = report["predicted_balance_db"].as_f64().unwrap();
    // The demo itself is not perfectly balanced. The injected input swap adds
    // -6.5 - 7.0 = -13.5 dB to whatever the unmodified demo predicts.
    assert!((predicted - before + 13.5).abs() <= 0.5, "{report}");
    assert!(has_log(&poll, "cli_itd_pairing_warning"));
}
