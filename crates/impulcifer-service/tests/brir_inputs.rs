#![forbid(unsafe_code)]
mod brir_support;
#[path = "support/room_measurement.rs"]
mod room_builder;
use brir_support::*;
use impulcifer_service::brir::{
    BrirError, BrirEvents, discovery::discover, estimator::open_estimator, inputs::load_inputs,
};
use impulcifer_types::config::ProcessingConfig;
use serde_json::Value;
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
    // The demo headphones were recorded with the job's own sweep.
    assert!(!events.0.iter().any(|k| k == "cli_headphone_own_sweep"));
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
fn headphone_recording_made_with_another_sweep_is_read_with_its_own() {
    // The job's test signal belongs to the speaker measurement (12.3 s here),
    // the demo headphones were recorded with the default 6.15 s sweep. Cut
    // with the job's sweep only one sweep fits, FR has no response and the
    // job ended in "FR right headphone measurement missing".
    let temp = Temp::new();
    std::fs::copy(
        root().join("data/demo/headphones.wav"),
        temp.0.join("headphones.wav"),
    )
    .unwrap();
    let config = ProcessingConfig {
        do_room_correction: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let job = open_estimator(&dir, Some("generate:12.3s@48000")).unwrap();
    impulcifer_io::write_wav(
        &temp.0.join("FL,FR.wav"),
        job.fs,
        &job.sweep_sequence(&["FL", "FR"], "stereo").unwrap(),
        32,
    )
    .unwrap();
    let dir = discover(&temp.0, &config).unwrap();
    let mut events = Events::default();
    let own = load_inputs(&dir, &job, &config, &mut events)
        .unwrap()
        .headphone
        .unwrap();
    assert!(events.1.contains(&(
        "cli_headphone_own_sweep".into(),
        serde_json::json!({"duration":"6.15","job":"12.30"})
    )));
    // The same responses as a job whose test signal is that sweep.
    let default = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let reference = load_inputs(&dir, &default, &config, &mut events)
        .unwrap()
        .headphone
        .unwrap();
    assert!(!events.0.iter().any(|k| k == "cli_headphone_own_sweep"));
    assert_eq!(own.left, reference.left);
    assert_eq!(own.right, reference.right);
    assert_eq!(own.responses_tracks, reference.responses_tracks);
}

#[test]
fn headphone_recording_too_short_for_two_sweeps_says_so_and_writes_nothing() {
    // 16 s made with a 5 s signal that is neither the job's nor a generated
    // sweep: nothing names its sweep, and one 12.3 s sweep is all that fits.
    let temp = Temp::new();
    let rate = 48000;
    let mut tracks = vec![vec![0.0; 16 * rate]; 2];
    for (track, start) in [(0, 2 * rate), (1, 9 * rate)] {
        for (i, sample) in tracks[track][start..start + 5 * rate]
            .iter_mut()
            .enumerate()
        {
            *sample = 0.5 * (i as f64 * 0.05).sin();
        }
    }
    impulcifer_io::write_wav(&temp.0.join("headphones.wav"), rate as u32, &tracks, 32).unwrap();
    let config = ProcessingConfig {
        do_room_correction: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let job = open_estimator(&dir, Some("generate:12.3s@48000")).unwrap();
    let mut events = Events::default();
    let error = load_inputs(&dir, &job, &config, &mut events)
        .expect_err("one sweep is not a headphone measurement");
    assert!(!events.0.iter().any(|k| k == "cli_headphone_own_sweep"));
    assert_eq!(error.envelope()["error"]["code"], "INVALID_REQUEST");
    let message = error.to_string();
    for part in ["16.00 s long", "two 12.30 s sweeps", "(28.61 s)"] {
        assert!(message.contains(part), "{message}");
    }
    assert!(!temp.0.join("headphone-responses.wav").exists());
}
