#![forbid(unsafe_code)]
mod brir_support;
#[path = "support/room_measurement.rs"]
mod room_builder;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{
    BrirError, BrirEvents, Catalog, discovery::discover, estimator::open_estimator,
    inputs::load_inputs, run::run_brir,
};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobKind, JobStatus},
};
use serde_json::{Value, json};
use std::path::Path;
#[derive(Default)]
struct Events(Vec<String>, Vec<(String, Value)>);
impl BrirEvents for Events {
    fn step(&mut self, key: &str, _: Value) -> Result<(), BrirError> {
        self.0.push(key.into());
        Ok(())
    }
    fn log(&mut self, _: &str, key: &str, args: Value) {
        self.0.push(key.into());
        self.1.push((key.into(), args));
    }
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
#[test]
fn estimator_alias_generate_sidecar_and_auto_match() {
    let temp = Temp::new();
    let config = ProcessingConfig::default();
    let dir = discover(&temp.0, &config).unwrap();
    let generated = open_estimator(&dir, Some("generate:6.15s@48000")).unwrap();
    let alias = open_estimator(&dir, Some("default")).unwrap();
    assert_eq!(generated.test_signal, alias.test_signal);
    impulcifer_io::write_wav(
        &temp.0.join("test.wav"),
        48000,
        std::slice::from_ref(&generated.test_signal),
        32,
    )
    .unwrap();
    let dir = discover(&temp.0, &config).unwrap();
    let automatic = open_estimator(&dir, None).unwrap();
    assert_eq!(automatic.test_signal, generated.test_signal);
    assert!(matches!(
        open_estimator(&dir, Some("test.pkl")),
        Err(BrirError::Unsupported(_))
    ));
}
#[test]
fn room_and_eq_inputs_use_real_measurements_and_write_responses() {
    let temp = Temp::demo();
    let source = temp.0.join("FL,FR.wav");
    std::fs::copy(&source, temp.0.join("room-FL,FR.wav")).unwrap();
    let recording = impulcifer_io::read_wav(&source).unwrap();
    let mono: Vec<_> = recording.tracks[0]
        .iter()
        .zip(&recording.tracks[1])
        .map(|(l, r)| (l + r) * 0.5)
        .collect();
    impulcifer_io::write_wav(&temp.0.join("room.wav"), recording.sample_rate, &[mono], 32).unwrap();
    for name in [
        "room-target.csv",
        "room-mic-calibration.csv",
        "eq.csv",
        "eq-left.csv",
        "eq-right.csv",
    ] {
        std::fs::write(
            temp.0.join(name),
            "frequency,raw\n10.0,0.0\n100.0,1.0\n1000.0,0.0\n10000.0,-1.0\n24000.0,0.0\n",
        )
        .unwrap();
    }
    std::fs::write(temp.0.join("eq.txt"), "Preamp: -30 dB").unwrap();
    let config = ProcessingConfig::default();
    let dir = discover(&temp.0, &config).unwrap();
    assert_eq!(
        dir.eq.common.as_ref().unwrap().file_name().unwrap(),
        "eq.csv"
    );
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    assert!(!inputs.room.as_ref().unwrap().frs.entries.is_empty());
    assert!(inputs.headphone.is_some());
    assert!(inputs.eq_left.is_some());
    assert!(inputs.eq_right.is_some());
    for name in ["room-responses.wav", "headphone-responses.wav"] {
        let wav = impulcifer_io::read_wav(&temp.0.join(name)).unwrap();
        assert_eq!(wav.tracks.len(), 32);
        assert_eq!(wav.sample_rate, 48000);
    }
    assert!(events.0.iter().any(|k| k == "cli_eq_plain_gain_curve"));
}
#[test]
fn eqapo_input_is_parsed_and_logged() {
    // core/pipeline_stages.py:199-291 through stages::eqapo (P12): a preamp and a
    // peaking filter at 1 kHz become an eq curve whose error is -gain.
    let temp = Temp::demo();
    std::fs::write(
        temp.0.join("eq.txt"),
        "Preamp: -3 dB\nFilter: ON PK Fc 1000 Hz Gain -6 dB Q 1.0\nFilter: ON XX Fc 1 Hz\n",
    )
    .unwrap();
    let config = ProcessingConfig {
        do_room_correction: false,
        do_headphone_compensation: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = impulcifer_dsp::estimator::SweepEstimator::new(5.0, 48000).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    let left = inputs.eq_left.expect("eq.txt becomes eq_left");
    // No channel scopes: 2.x applies the single curve to both ears (select_eq_pair).
    assert_eq!(inputs.eq_right.as_ref(), Some(&left));
    let at_1k = left.frequency.iter().position(|f| *f >= 1000.0).unwrap();
    assert!(
        (left.raw[at_1k] + 9.0).abs() < 0.5,
        "raw at 1 kHz {}",
        left.raw[at_1k]
    );
    assert!((left.error[at_1k] - 9.0).abs() < 0.5, "error = -gain");
    assert!(events.0.contains(&"cli_eqapo_detected".to_string()));
    assert!(events.0.contains(&"cli_eqapo_preamp".to_string()));
    assert!(
        events.0.contains(&"cli_eqapo_bypassed_line".to_string()),
        "unknown XX filter is bypassed"
    );
    assert!(!events.0.contains(&"cli_eqapo_channel_split".to_string()));
}

#[test]
fn room_measurement_file_replaces_the_folder_room_recordings() {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        room_measurement_file: Some(
            root()
                .join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat")
                .to_string_lossy()
                .into(),
        ),
        room_measurement_map: Some("FL.left=1,FL.right=2".into()),
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    assert!(!dir.room.recordings.is_empty());
    // Poison the discovered files: success proves they are not opened.
    for (path, _) in &dir.room.recordings {
        std::fs::write(path, b"not a wav").unwrap();
    }
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    let room = inputs.room.unwrap();
    assert_eq!(room.frs.entries.len(), 2);
    assert!(room.frs.entries.iter().all(|(slot, _, _)| slot == "FL"));
    assert!(events.1.contains(&(
        "cli_room_measurement_file_replaces_folder".into(),
        serde_json::json!({"count":dir.room.recordings.len()})
    )));
    assert!(events.1.contains(&(
        "cli_room_measurement_file_loaded".into(),
        serde_json::json!({"file":"rew_two_irs_and_fr.mdat","format":"REW","count":2})
    )));
    assert!(
        !events
            .0
            .contains(&"cli_room_measurement_file_mic_calibration".into())
    );

    let mut off = config.clone();
    off.do_room_correction = false;
    off.room_measurement_file = Some("missing.mdat".into());
    load_inputs(&dir, &estimator, &off, &mut Events::default()).unwrap();
    off.do_room_correction = true;
    let err = load_inputs(&dir, &estimator, &off, &mut Events::default()).unwrap_err();
    assert_eq!(err.envelope()["error"]["code"], "FILE_NOT_FOUND");
    let mut bad = config;
    bad.room_measurement_map = Some("FL=3".into());
    let err = load_inputs(&dir, &estimator, &bad, &mut Events::default()).unwrap_err();
    assert_eq!(err.envelope()["error"]["code"], "INVALID_REQUEST");
}

#[test]
fn dirac_project_room_measurements_are_resampled_and_bring_their_calibration() {
    let temp = Temp::demo();
    for name in ["room-mic-calibration.csv", "room-mic-calibration.txt"] {
        let path = temp.0.join(name);
        if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
    }
    let path = temp.0.join("test.liveproject");
    std::fs::write(&path, room_builder::dirac_project(44100)).unwrap();
    let config = ProcessingConfig {
        room_measurement_file: Some(path.to_string_lossy().into()),
        room_measurement_map: Some(String::new()),
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    let room = inputs.room.unwrap();
    assert_eq!(room.frs.entries.len(), 4);
    assert!(
        room.frs
            .entries
            .iter()
            .all(|(slot, _, _)| ["FL", "FR"].contains(&slot.as_str()))
    );
    let wav = impulcifer_io::read_wav(&temp.0.join("room-responses.wav")).unwrap();
    assert_eq!(wav.sample_rate, estimator.fs);
    assert!(
        events
            .0
            .contains(&"cli_room_measurement_file_mic_calibration".into())
    );
    assert!(events.1.contains(&(
        "cli_room_measurement_file_loaded".into(),
        serde_json::json!({"file":"test.liveproject","format":"Dirac Live","count":4})
    )));
    std::fs::write(
        temp.0.join("room-mic-calibration.csv"),
        "frequency,raw\n10.0,0.0\n100.0,0.0\n1000.0,0.0\n10000.0,0.0\n24000.0,0.0\n",
    )
    .unwrap();
    let dir = discover(&temp.0, &config).unwrap();
    let mut events = Events::default();
    let folder = load_inputs(&dir, &estimator, &config, &mut events)
        .unwrap()
        .room
        .unwrap();
    assert!(
        !events
            .0
            .contains(&"cli_room_measurement_file_mic_calibration".into())
    );
    assert_ne!(room.frs.entries, folder.frs.entries);
    // An explicit calibration wins even when a folder calibration also exists.
    let explicit = temp.0.join("explicit.csv");
    std::fs::write(
        &explicit,
        "frequency,raw\n10.0,-2.0\n100.0,-1.0\n1000.0,0.0\n10000.0,1.0\n24000.0,0.0\n",
    )
    .unwrap();
    let config = ProcessingConfig {
        room_mic_calibration: Some(explicit.to_string_lossy().into()),
        ..config
    };
    let dir = discover(&temp.0, &config).unwrap();
    let mut events = Events::default();
    let explicit = load_inputs(&dir, &estimator, &config, &mut events)
        .unwrap()
        .room
        .unwrap();
    assert_eq!(room.frs.entries, explicit.frs.entries);
    assert!(
        !events
            .0
            .contains(&"cli_room_measurement_file_mic_calibration".into())
    );
}

#[test]
fn brir_measurement_file_preserves_rew_time_axis_and_replaces_folder() {
    let temp = Temp::demo();
    let external = root().join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat");
    let config = ProcessingConfig {
        brir_measurement_file: Some(external.to_string_lossy().into()),
        brir_measurement_map: Some("FL.left=1,FL.right=2".into()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    for (path, _) in &dir.recordings {
        std::fs::write(path, b"poison").unwrap();
    }
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    assert_eq!(inputs.hrir.speakers.len(), 1);
    assert!(inputs.lfe.is_empty());
    let speaker = &inputs.hrir.speakers[0];
    let left = speaker.left.as_ref().unwrap();
    let right = speaker.right.as_ref().unwrap();
    assert_eq!(left.data.len(), right.data.len());
    let file = impulcifer_io::external::read_external(&external).unwrap();
    let expected = ((file.measurements[1].start_time - file.measurements[0].start_time)
        * f64::from(file.sample_rate))
    .round() as usize;
    assert!(right.data[..expected].iter().all(|sample| *sample == 0.0));
    assert!(events.1.contains(&(
        "cli_brir_measurement_file_replaces_folder".into(),
        serde_json::json!({"count":dir.recordings.len()})
    )));
    assert!(events.1.contains(&(
        "cli_brir_measurement_file_loaded".into(),
        serde_json::json!({"file":"rew_two_irs_and_fr.mdat","format":"REW","count":1})
    )));
    let loaded = events
        .0
        .iter()
        .position(|key| key == "cli_brir_measurement_file_loaded")
        .unwrap();
    let itd = events
        .0
        .iter()
        .position(|key| key == "cli_brir_measurement_file_itd")
        .unwrap();
    assert!(loaded < itd, "loaded event must precede ITD diagnostics");
}

#[test]
fn dirac_brir_logs_large_and_zero_itd_warnings() {
    for (delays, expected, expected_count) in [
        (
            [0, 0, 0, 500, 500, 500],
            "cli_brir_measurement_file_itd_large",
            2,
        ),
        ([0; 6], "cli_brir_measurement_file_itd_zero", 1),
    ] {
        let temp = Temp::demo();
        let path = temp.0.join("ears.liveproject");
        std::fs::write(
            &path,
            room_builder::dirac_project_with_delays(48000, &delays),
        )
        .unwrap();
        let config = ProcessingConfig {
            brir_measurement_file: Some(path.to_string_lossy().into()),
            do_room_correction: false,
            do_headphone_compensation: false,
            do_equalization: false,
            ..Default::default()
        };
        let dir = discover(&temp.0, &config).unwrap();
        let estimator = open_estimator(&dir, Some("default")).unwrap();
        let mut events = Events::default();
        load_inputs(&dir, &estimator, &config, &mut events).unwrap();
        let warnings = events
            .0
            .iter()
            .filter(|key| key.as_str() == expected)
            .count();
        assert_eq!(
            warnings, expected_count,
            "warning count for {expected}: {:?}",
            events.0
        );
    }
}

#[test]
fn brir_measurement_errors_are_invalid_requests_or_missing_files() {
    let temp = Temp::demo();
    let external = root().join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat");
    let estimator = impulcifer_dsp::estimator::SweepEstimator::new(5.0, 48000).unwrap();
    for map in ["FL.left=1", "FR.left=1,FR.right=2"] {
        let config = ProcessingConfig {
            brir_measurement_file: Some(external.to_string_lossy().into()),
            brir_measurement_map: Some(map.into()),
            do_room_correction: false,
            do_headphone_compensation: false,
            do_equalization: false,
            ..Default::default()
        };
        let dir = discover(&temp.0, &config).unwrap();
        assert_eq!(
            load_inputs(&dir, &estimator, &config, &mut Events::default())
                .unwrap_err()
                .envelope()["error"]["code"],
            "INVALID_REQUEST"
        );
    }
    let config = ProcessingConfig {
        brir_measurement_file: Some(temp.0.join("missing.mdat").to_string_lossy().into()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    assert_eq!(
        load_inputs(&dir, &estimator, &config, &mut Events::default())
            .unwrap_err()
            .envelope()["error"]["code"],
        "FILE_NOT_FOUND"
    );
}

#[test]
fn headphone_measurement_file_replaces_folder_and_is_not_opened_when_disabled() {
    let temp = Temp::demo();
    let external = root().join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat");
    let file = impulcifer_io::external::read_external(&external).unwrap();
    let config = ProcessingConfig {
        headphone_measurement_file: Some(external.to_string_lossy().into()),
        headphone_measurement_map: Some("left=1,right=2".into()),
        do_room_correction: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let folder = dir.headphones.as_ref().unwrap();
    std::fs::write(folder, b"poison").unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    assert!(inputs.headphone.is_some());
    assert!(temp.0.join("headphone-responses.wav").is_file());
    assert!(events.1.contains(&(
        "cli_headphone_measurement_file_replaces_folder".into(),
        serde_json::json!({"file":"headphones.wav"})
    )));
    assert!(
        events
            .1
            .iter()
            .any(|(key, args)| key == "cli_headphone_measurement_file_loaded"
                && args["left"] == file.measurements[0].name
                && args["right"] == file.measurements[1].name)
    );
    for key in [
        "cli_info_hp_param_provided",
        "cli_info_hp_default",
        "cli_info_using_hp_file",
        "cli_error_hp_file_missing",
    ] {
        assert!(!events.0.contains(&key.into()));
    }
    let mut off = config;
    off.do_headphone_compensation = false;
    off.headphone_measurement_file = Some("missing.mdat".into());
    load_inputs(&dir, &estimator, &off, &mut Events::default()).unwrap();
}

#[test]
fn headphone_recording_without_two_sweeps_names_the_test_signal() {
    // The reported job: the test signal was a 13 s sweep file and the
    // headphones were recorded with the default 6.15 s sweep. One sweep of the
    // job's length fits into that recording, FR gets no response, and the job
    // ended in "FR right headphone measurement missing". Here the job's signal
    // is the 12.3 s generated sweep and the recording the demo's.
    let temp = Temp::new();
    let headphones = temp.0.join("headphones.wav");
    std::fs::copy(root().join("data/demo/headphones.wav"), &headphones).unwrap();
    let config = ProcessingConfig {
        do_room_correction: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let job = open_estimator(&dir, Some("generate:12.3s@48000")).unwrap();
    let failure = |seconds: f64| {
        let envelope = load_inputs(&dir, &job, &config, &mut Events::default())
            .expect_err("the recording does not hold two sweeps")
            .envelope();
        let error = &envelope["error"];
        assert_eq!(error["code"], "INVALID_REQUEST");
        assert_eq!(
            error["message"],
            "The test signal does not match the headphone recording."
        );
        assert_eq!(error["details"]["recording_seconds"], seconds);
        assert_eq!(error["details"]["sweep_seconds"], 12.3);
        assert_eq!(error["details"]["needed_seconds"], 28.61);
        assert_eq!(
            Path::new(error["details"]["path"].as_str().unwrap()),
            dir.headphones.as_ref().unwrap()
        );
        // A failed job leaves no responses cut with the wrong signal behind.
        assert!(!temp.0.join("headphone-responses.wav").exists());
    };
    failure(18.3);
    // Four seconds hold no sweep at all: the same error instead of the
    // ingestion's text about columns as an internal error.
    impulcifer_io::write_wav(&headphones, 48000, &vec![vec![0.0; 4 * 48000]; 2], 32).unwrap();
    failure(4.0);
}

fn run_config(config: ProcessingConfig) -> impulcifer_jobs::registry::PollResult {
    let jobs = JobRegistry::new();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            let run = run_brir(&config, &Catalog::english(), ctx)?;
            Ok(json!({"output_path":run.output_path}))
        })
        .unwrap();
    wait(&jobs, &job.job_id)
}

fn service_log_sweep(fs: u32, seconds: f64, low: f64, high: f64) -> Vec<f64> {
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

fn foreign_sequence(temp: &Temp, sweep_rate: u32) -> (std::path::PathBuf, std::path::PathBuf) {
    let fs = 8_000;
    let sweep = service_log_sweep(fs, 1.0, 10.0, 3_500.0);
    let starts = [1_000, 11_500, 22_000, 32_500, 43_000];
    let gains = [1.0, 0.8, 0.65, 0.5, 0.4];
    let mut tracks = vec![vec![0.0; 52_000]; 2];
    for (sequence, (start, gain)) in starts.into_iter().zip(gains).enumerate() {
        for (ear, track) in tracks.iter_mut().enumerate() {
            let delay = 3 + sequence + ear * 2;
            for (index, sample) in sweep.iter().copied().enumerate() {
                track[start + delay + index] += gain * sample;
                track[start + delay + 23 + index] += 0.15 * gain * sample;
            }
        }
    }
    let recording = temp.0.join("foreign-two-ear.wav");
    let sweep_path = temp.0.join("foreign-sweep.wav");
    impulcifer_io::write_wav(&recording, fs, &tracks, 32).unwrap();
    impulcifer_io::write_wav(&sweep_path, sweep_rate, &[sweep], 32).unwrap();
    (recording, sweep_path)
}

#[test]
fn recording_file_without_sweep_matches_the_folder_recording() {
    let folder = Temp::new();
    let imported = Temp::new();
    let outside = Temp::new();
    let source = root().join("data/demo/FL,FR.wav");
    std::fs::copy(&source, folder.0.join("FL,FR.wav")).unwrap();
    let arbitrary = outside.0.join("arbitrary-name.wav");
    std::fs::copy(source, &arbitrary).unwrap();
    // This discovered recording must neither affect auto detection nor be read.
    std::fs::write(imported.0.join("FC.wav"), b"not a WAV").unwrap();

    let common = |directory: &Path| ProcessingConfig {
        dir_path: Some(directory.to_string_lossy().into_owned()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let folder_poll = run_config(common(&folder.0));
    assert_eq!(
        folder_poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        folder_poll.job.error
    );
    let imported_poll = run_config(ProcessingConfig {
        brir_recording_file: Some(arbitrary.to_string_lossy().into_owned()),
        brir_recording_speakers: Some("FL,FR".into()),
        ..common(&imported.0)
    });
    assert_eq!(
        imported_poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        imported_poll.job.error
    );
    assert!(
        imported_poll
            .events
            .iter()
            .any(|event| { event.payload["key"] == "cli_brir_recording_replaces_folder" })
    );
    assert_eq!(
        std::fs::read(folder.0.join("hesuvi.wav")).unwrap(),
        std::fs::read(imported.0.join("hesuvi.wav")).unwrap()
    );

    // Only the first two tracks are the ears. A folder recording would take
    // tracks 3 and 4 as the next speaker's pair, so FR would come from them
    // instead of from the second sweep.
    let four_tracks = Temp::new();
    let wav = impulcifer_io::read_wav(&arbitrary).unwrap();
    let mut tracks = wav.tracks.clone();
    tracks.push(wav.tracks[1].iter().map(|x| -x).collect());
    tracks.push(wav.tracks[0].iter().map(|x| 0.5 * x).collect());
    let wide = outside.0.join("four-tracks.wav");
    impulcifer_io::write_wav(&wide, wav.sample_rate, &tracks, 32).unwrap();
    let two = outside.0.join("two-tracks.wav");
    impulcifer_io::write_wav(&two, wav.sample_rate, &wav.tracks, 32).unwrap();
    let narrow = Temp::new();
    for (directory, file) in [(&four_tracks, &wide), (&narrow, &two)] {
        let poll = run_config(ProcessingConfig {
            brir_recording_file: Some(file.to_string_lossy().into_owned()),
            brir_recording_speakers: Some("FL,FR".into()),
            ..common(&directory.0)
        });
        assert_eq!(
            poll.job.status,
            JobStatus::Succeeded,
            "{:?}",
            poll.job.error
        );
    }
    assert_eq!(
        std::fs::read(narrow.0.join("hesuvi.wav")).unwrap(),
        std::fs::read(four_tracks.0.join("hesuvi.wav")).unwrap()
    );

    // One track is not a pair of ears.
    let mono = outside.0.join("mono.wav");
    impulcifer_io::write_wav(&mono, wav.sample_rate, &wav.tracks[..1], 32).unwrap();
    let mono_output = Temp::new();
    let poll = run_config(ProcessingConfig {
        brir_recording_file: Some(mono.to_string_lossy().into_owned()),
        brir_recording_speakers: Some("FL,FR".into()),
        ..common(&mono_output.0)
    });
    assert_eq!(poll.job.status, JobStatus::Failed);
    assert_eq!(
        poll.job.error.as_ref().unwrap()["message"],
        "brir_recording_file must have at least two tracks"
    );
}

#[test]
fn recording_file_with_sweep_reads_a_foreign_sequence() {
    let temp = Temp::new();
    let (recording, sweep) = foreign_sequence(&temp, 8_000);
    let poll = run_config(ProcessingConfig {
        dir_path: Some(temp.0.to_string_lossy().into_owned()),
        test_signal: Some("generate:1.0s@8000".into()),
        brir_recording_file: Some(recording.to_string_lossy().into_owned()),
        brir_recording_speakers: Some("FL,FC,X,FR,LFE".into()),
        brir_recording_sweep: Some(sweep.to_string_lossy().into_owned()),
        layout_files: "9.1.4".into(),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    });
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    poll.events
        .iter()
        .find(|event| event.payload["key"] == "cli_brir_recording_loaded")
        .expect("loaded log");
    assert!(
        poll.events
            .iter()
            .any(|event| event.payload["key"] == "cli_brir_measurement_file_itd")
    );

    let hrir = impulcifer_io::read_wav(&temp.0.join("hrir.wav")).unwrap();
    for name in [
        "FL-left", "FL-right", "FC-left", "FC-right", "FR-left", "FR-right",
    ] {
        let index = impulcifer_types::constants::HEXADECAGONAL_TRACK_ORDER
            .iter()
            .position(|track| *track == name)
            .unwrap();
        assert!(
            hrir.tracks[index].iter().any(|sample| *sample != 0.0),
            "{name}"
        );
    }
    let layout = impulcifer_types::layouts::IMMERSIVE_LAYOUTS
        .iter()
        .find(|layout| layout.id == "9.1.4")
        .unwrap();
    let lfe = layout
        .channels
        .iter()
        .position(|channel| channel.label == "LFE")
        .unwrap();
    let output = impulcifer_io::read_wav(&temp.0.join("atmos_9.1.4.wav")).unwrap();
    assert!(output.tracks[2 * lfe].iter().any(|sample| *sample != 0.0));
    assert!(
        output.tracks[2 * lfe + 1]
            .iter()
            .any(|sample| *sample != 0.0)
    );
}

#[test]
fn recording_file_errors_name_the_cause() {
    let temp = Temp::new();
    let (recording, sweep) = foreign_sequence(&temp, 8_000);
    let estimator = impulcifer_dsp::estimator::SweepEstimator::new(1.0, 8_000).unwrap();
    let base = |speakers: &str| ProcessingConfig {
        brir_recording_file: Some(recording.to_string_lossy().into_owned()),
        brir_recording_speakers: Some(speakers.into()),
        brir_recording_sweep: Some(sweep.to_string_lossy().into_owned()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let config = base("FL,FC,X,FR");
    let dir = discover(&temp.0, &config).unwrap();
    let error = load_inputs(&dir, &estimator, &config, &mut Events::default()).unwrap_err();
    assert!(matches!(error, BrirError::RecordingSweepCount { .. }));
    let assert_error = |error: &BrirError, message: &str, details: Value| {
        let envelope = error.envelope();
        assert_eq!(envelope["error"]["code"], "INVALID_REQUEST");
        assert_eq!(envelope["error"]["message"], message);
        assert_eq!(envelope["error"]["details"], details);
        assert_eq!(envelope["error"]["retryable"], false);
    };
    assert_error(
        &error,
        "The recording does not hold the listed number of sweeps.",
        json!({"path": recording, "found": 5, "listed": 4,
            "starts_seconds": [0.13, 1.44, 2.75, 4.06, 5.38]}),
    );
    assert_eq!(
        error.to_string(),
        "Found 5 sweeps in the recording but the list has 4 entries. Sweeps start at 0.13, 1.44, 2.75, 4.06, 5.38 s. Write X for a sweep to skip."
    );

    for config in [
        ProcessingConfig {
            brir_recording_file: Some(recording.to_string_lossy().into_owned()),
            brir_recording_speakers: Some("FC,FR".into()),
            ..Default::default()
        },
        ProcessingConfig {
            brir_recording_file: Some(recording.to_string_lossy().into_owned()),
            brir_recording_speakers: Some("FL,FR".into()),
            brir_measurement_file: Some("project.mdat".into()),
            ..Default::default()
        },
    ] {
        assert!(config.validate_room_options().is_err());
    }

    let missing = ProcessingConfig {
        brir_recording_file: Some(temp.0.join("missing.wav").to_string_lossy().into_owned()),
        brir_recording_speakers: Some("FL".into()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &missing).unwrap();
    let error = load_inputs(&dir, &estimator, &missing, &mut Events::default()).unwrap_err();
    assert_eq!(error.envelope()["error"]["code"], "FILE_NOT_FOUND");
    assert!(error.to_string().contains("missing.wav"));

    let (_, wrong_rate) = foreign_sequence(&temp, 44_100);
    let wrong = ProcessingConfig {
        brir_recording_sweep: Some(wrong_rate.to_string_lossy().into_owned()),
        ..base("FL,FC,X,FR,LFE")
    };
    let dir = discover(&temp.0, &wrong).unwrap();
    let error = load_inputs(&dir, &estimator, &wrong, &mut Events::default()).unwrap_err();
    assert!(matches!(error, BrirError::RecordingSweepRate { .. }));
    assert_error(
        &error,
        "The sweep file and the recording have different sampling rates.",
        json!({"path": wrong_rate, "sweep_fs": 44100, "recording_fs": 8000}),
    );
    assert_eq!(
        error.to_string(),
        "The sweep file (44100 Hz) and the recording (8000 Hz) have different sampling rates."
    );

    // A valid reference with a silent recording reports zero found, not an internal error.
    foreign_sequence(&temp, 8_000);
    impulcifer_io::write_wav(&recording, 8000, &vec![vec![0.0; 52_000]; 2], 32).unwrap();
    let config = base("FL,FC,X,FR,LFE");
    let dir = discover(&temp.0, &config).unwrap();
    let error = load_inputs(&dir, &estimator, &config, &mut Events::default()).unwrap_err();
    assert!(matches!(
        error,
        BrirError::RecordingSweepCount { found: 0, .. }
    ));
    assert_error(
        &error,
        "The recording does not hold the listed number of sweeps.",
        json!({"path": recording, "found": 0, "listed": 5, "starts_seconds": []}),
    );

    // Each trim failure and a full-length linear sweep use the same translated error.
    foreign_sequence(&temp, 8_000);
    let linear: Vec<_> = (0..8000)
        .map(|index| {
            let time = f64::from(index) / 8000.0;
            (2.0 * std::f64::consts::PI * (10.0 * time + 0.5 * 3490.0 * time * time)).sin()
        })
        .collect();
    for reference in [vec![0.0; 8000], vec![1.0; 100], vec![1.0; 1000], linear] {
        impulcifer_io::write_wav(&sweep, 8000, &[reference], 32).unwrap();
        let error = load_inputs(&dir, &estimator, &config, &mut Events::default()).unwrap_err();
        assert!(matches!(error, BrirError::RecordingSweepFile { .. }));
        assert_error(
            &error,
            "The sweep file is not a logarithmic sweep.",
            json!({"path": sweep}),
        );
        assert_eq!(
            error.to_string(),
            format!(
                "The sweep file is not a logarithmic sweep covering 200–2000 Hz: {}",
                sweep.display()
            )
        );
    }

    let folder_recording = root().join("data/demo/FL,FR.wav");
    let no_sweep = ProcessingConfig {
        brir_recording_file: Some(folder_recording.to_string_lossy().into_owned()),
        brir_recording_speakers: Some("FL,FR".into()),
        do_room_correction: false,
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &no_sweep).unwrap();
    let wrong_estimator = open_estimator(&dir, Some("generate:12.3s@48000")).unwrap();
    let error = load_inputs(&dir, &wrong_estimator, &no_sweep, &mut Events::default()).unwrap_err();
    assert!(matches!(error, BrirError::RecordingTestSignal { .. }));
    assert_error(
        &error,
        "The test signal does not match the recording file.",
        json!({"path": folder_recording, "read": 1, "listed": 2}),
    );
    assert_eq!(
        error.to_string(),
        "The recording holds 1 of the 2 listed sweeps for this test signal. A recording made with another sweep needs brir_recording_sweep."
    );
}
