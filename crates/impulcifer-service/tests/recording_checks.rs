#![forbid(unsafe_code)]

use impulcifer_service::recording::{
    checks::{CheckResults, DirectSound, Measurement, ReferenceMeasurement, check_recording},
    sweep::{SweepSpec, build_sweep_playback},
};
use std::collections::BTreeMap;

const FS: u32 = 48_000;

fn signal(
    speaker: &str,
    left_delay: usize,
    right_delay: usize,
    gain: f64,
    noise: f64,
) -> CheckResults {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec![speaker.to_owned()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let segment = playback.segments[0].clone();
    let n = playback.tracks[0].len() + left_delay.max(right_delay) + 64;
    let source = &playback.tracks[0];
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    let (left, right) = tracks.split_at_mut(1);
    for (i, samples) in left[0].iter_mut().zip(&mut right[0]).enumerate() {
        let deterministic = if i % 2 == 0 { noise } else { -noise };
        *samples.0 = deterministic;
        *samples.1 = -deterministic;
    }
    for (i, sample) in source.iter().enumerate() {
        tracks[0][i + left_delay] += sample * gain;
        tracks[1][i + right_delay] += sample * gain;
    }
    let expected = [(
        speaker.to_owned(),
        match speaker {
            "FR" => -30.0,
            "FC" => 0.0,
            "BL" => 150.0,
            _ => 30.0,
        },
    )]
    .into();
    check_recording(
        &tracks,
        FS,
        std::slice::from_ref(&segment),
        Some(&playback.estimator),
        Some(&expected),
        None,
        false,
    )
}

fn has(result: &CheckResults, key: &str) -> bool {
    result.checks.iter().any(|check| check.key == key)
}

#[test]
fn take_checks_measure_expected_and_excessive_itd() {
    let correct = signal("FL", 0, 12, 0.2, 1e-6);
    assert!(correct.checks.is_empty(), "{:?}", correct.checks);
    let itd = correct.measurements[0].itd_us.unwrap();
    assert!((itd - 250.0).abs() <= 25.0, "{itd}");

    let far = signal("FL", 0, 21, 0.2, 1e-6);
    let warning = far
        .checks
        .iter()
        .find(|check| check.key == "record_check_angle_too_far")
        .unwrap();
    let measured = warning.values["measured"].as_f64().unwrap();
    assert!((45.0..=60.0).contains(&measured), "{measured}");
    eprintln!(
        "guided checks: 30deg itd_us={}, 437.5us itd_us={}, measured={}",
        correct.measurements[0].itd_us.unwrap(),
        far.measurements[0].itd_us.unwrap(),
        measured
    );
}

#[test]
fn take_checks_direction_rules_and_optional_angle_gate() {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FL".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let with_expected = signal("FL", 0, 21, 0.2, 1e-6);
    assert!(has(&with_expected, "record_check_angle_too_far"));

    let segment = playback.segments[0].clone();
    let n = playback.tracks[0].len() + 100;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (i, sample) in playback.tracks[0].iter().enumerate() {
        tracks[0][i] += sample * 0.2;
        tracks[1][i + 21] += sample * 0.2;
    }
    let no_expected = check_recording(
        &tracks,
        FS,
        &[segment],
        Some(&playback.estimator),
        None,
        None,
        false,
    );
    assert!(
        !no_expected
            .checks
            .iter()
            .any(|check| check.key.contains("angle_"))
    );

    assert!(has(
        &signal("FL", 12, 0, 0.2, 1e-6),
        "record_check_wrong_side_left"
    ));
    assert!(has(
        &signal("FC", 0, 10, 0.2, 1e-6),
        "record_check_off_center"
    ));
    assert!(has(
        &signal("BL", 0, 3, 0.2, 1e-6),
        "record_check_angle_too_far"
    ));
}

fn level_result(signal_gain: f64, noise_gain: f64) -> CheckResults {
    signal("FL", 0, 12, signal_gain, noise_gain)
}

#[test]
fn take_checks_level_noise_and_missing_signal_rules() {
    assert!(has(&level_result(1.2, 1e-6), "record_check_clipping"));
    assert!(has(
        &level_result(10_f64.powf(-50.0 / 20.0), 1e-8),
        "record_check_quiet"
    ));
    let noisy = level_result(0.2, 0.2 / 10.0);
    assert!(has(&noisy, "record_check_noisy"), "{:?}", noisy.checks);

    let estimator = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FL".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let n = estimator.tracks[0].len();
    let silent = check_recording(
        &[vec![1e-3; n], vec![1e-3; n]],
        FS,
        &estimator.segments,
        Some(&estimator.estimator),
        Some(&BTreeMap::from([("FL".to_owned(), 30.0)])),
        None,
        false,
    );
    assert!(has(&silent, "record_check_no_signal"));
    assert_eq!(silent.checks.len(), 1);
}

/// A back centre slot (BC, 180°) has no side: an ITD there is off centre, not
/// a left speaker heard on the right.
#[test]
fn centre_slots_are_never_on_the_wrong_side() {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["BC".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let source = &playback.tracks[0];
    let n = source.len() + 64;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (i, sample) in source.iter().enumerate() {
        tracks[0][i + 12] += sample * 0.2;
        tracks[1][i] += sample * 0.2;
    }
    let result = check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        None,
        None,
        false,
    );
    assert!(
        !has(&result, "record_check_wrong_side_left"),
        "{:?}",
        result.checks
    );
    assert!(
        has(&result, "record_check_off_center"),
        "{:?}",
        result.checks
    );
}

/// Digital silence is no signal, not a quiet take heard from one side.
#[test]
fn digital_silence_is_no_signal() {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FR".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let n = playback.tracks[0].len();
    let result = check_recording(
        &[vec![0.0; n], vec![0.0; n]],
        FS,
        &playback.segments,
        Some(&playback.estimator),
        Some(&BTreeMap::from([("FR".to_owned(), -30.0)])),
        None,
        false,
    );
    let keys: Vec<_> = result
        .checks
        .iter()
        .map(|check| check.key.to_string())
        .collect();
    assert_eq!(keys, ["record_check_no_signal"]);
}

/// A reflection louder than the direct sound and arriving from the other side
/// 10 ms later does not decide the time difference: the first arrival does.
#[test]
fn the_first_arrival_decides_the_side_over_a_louder_reflection() {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FL".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let source = &playback.tracks[0];
    let reflection = 480;
    let n = source.len() + reflection + 64;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (i, sample) in source.iter().enumerate() {
        tracks[0][i] += sample * 0.12;
        tracks[1][i + 12] += sample * 0.12;
        tracks[0][i + reflection + 12] += sample * 0.2;
        tracks[1][i + reflection] += sample * 0.2;
    }
    let expected = BTreeMap::from([("FL".to_owned(), 30.0)]);
    let result = check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        Some(&expected),
        None,
        false,
    );
    let itd = result.measurements[0].itd_us.unwrap();
    assert!((itd - 250.0).abs() <= 25.0, "{itd}");
    assert!(result.checks.is_empty(), "{:?}", result.checks);
}

fn custom_take(
    speakers: &[&str],
    delays: &[(usize, usize)],
    gains: &[(f64, f64)],
    expected: Option<&BTreeMap<String, f64>>,
    references: Option<&[ReferenceMeasurement]>,
) -> CheckResults {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: speakers
            .iter()
            .map(|speaker| (*speaker).to_owned())
            .collect(),
        tracks: if speakers.len() > 2 { "7.1" } else { "stereo" }.into(),
    })
    .unwrap();
    let n = playback.tracks[0].len() + 128;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (segment_index, segment) in playback.segments.iter().enumerate() {
        let source_start = (segment.start * FS as f64).round() as usize;
        let source_end = (segment.end * FS as f64).round() as usize;
        let source = playback
            .tracks
            .iter()
            .map(|track| &track[source_start..source_end])
            .find(|samples| samples.iter().any(|sample| *sample != 0.0))
            .unwrap_or_else(|| panic!("missing sweep for segment {segment_index}"));
        for (i, sample) in source.iter().enumerate() {
            tracks[0][source_start + i + delays[segment_index].0] +=
                sample * gains[segment_index].0;
            tracks[1][source_start + i + delays[segment_index].1] +=
                sample * gains[segment_index].1;
        }
    }
    check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        expected,
        references,
        false,
    )
}

#[test]
fn take_checks_ear_silence_suppresses_itd_findings() {
    let result = custom_take(
        &["FL"],
        &[(0, 12)],
        &[(0.2 * 10_f64.powf(-30.0 / 20.0), 0.2)],
        Some(&BTreeMap::from([("FL".into(), 30.0)])),
        None,
    );
    assert!(
        has(&result, "record_check_ear_silent_left"),
        "{:?}",
        result.checks
    );
    assert!(!result.checks.iter().any(|check| matches!(
        check.key,
        "record_check_wrong_side_left"
            | "record_check_wrong_side_right"
            | "record_check_off_center"
            | "record_check_angle_too_far"
            | "record_check_angle_too_little"
            | "record_check_angle_off"
            | "record_check_polarity"
            | "record_check_itd_mismatch"
    )));
    assert_eq!(result.measurements[0].itd_us, None);
}

#[test]
fn take_checks_detect_reversed_microphone_polarity_without_changing_itd() {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FL".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let source = &playback.tracks[0];
    let mut tracks = vec![vec![0.0; source.len() + 64], vec![0.0; source.len() + 64]];
    for (i, sample) in source.iter().enumerate() {
        tracks[0][i] = sample * 0.2;
        tracks[1][i + 12] = -sample * 0.2;
    }
    let result = check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        Some(&BTreeMap::from([("FL".into(), 30.0)])),
        None,
        false,
    );
    assert!(has(&result, "record_check_polarity"), "{:?}", result.checks);
    assert!(result.measurements[0].direct.is_none());
    assert!(!has(&result, "record_check_speaker_polarity"));
    assert!((result.measurements[0].itd_us.unwrap() - 250.0).abs() <= 25.0);
}

#[test]
fn take_checks_compare_same_angle_current_and_reference_measurements() {
    let expected = BTreeMap::from([("FL".into(), 30.0), ("FR".into(), -30.0)]);
    let normal = custom_take(
        &["FL", "FR"],
        &[(0, 12), (12, 0)],
        &[(0.2, 0.2); 2],
        Some(&expected),
        None,
    );
    assert!(
        !has(&normal, "record_check_itd_mismatch"),
        "{:?}",
        normal.checks
    );
    let mismatch = custom_take(
        &["FL", "FR"],
        &[(0, 12), (21, 0)],
        &[(0.2, 0.2); 2],
        Some(&expected),
        None,
    );
    let warning = mismatch
        .checks
        .iter()
        .find(|check| check.key == "record_check_itd_mismatch")
        .unwrap();
    assert_eq!(warning.values["reference"], "FL");

    let references = [
        ReferenceMeasurement {
            speaker: "FL".into(),
            itd_us: Some(250.0),
            lf_db: None,
            lf_ild_db: None,
            azimuth: Some(30.0),
            direct: None,
        },
        ReferenceMeasurement {
            speaker: "FR".into(),
            itd_us: Some(-250.0),
            lf_db: None,
            lf_ild_db: None,
            azimuth: Some(-30.0),
            direct: None,
        },
    ];
    let result = custom_take(
        &["FR"],
        &[(21, 0)],
        &[(0.2, 0.2)],
        Some(&BTreeMap::from([("FR".into(), -30.0)])),
        Some(&references),
    );
    let warning = result
        .checks
        .iter()
        .find(|check| check.key == "record_check_itd_mismatch")
        .unwrap();
    assert_eq!(warning.values["reference"], "FL");

    let no_expected = custom_take(
        &["FL", "SL"],
        &[(0, 12), (0, 21)],
        &[(0.2, 0.2); 2],
        None,
        None,
    );
    assert!(!has(&no_expected, "record_check_itd_mismatch"));
}

fn nonlinear_take(nonlinearity: f64) -> CheckResults {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 1.0,
        speakers: vec!["FL".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let source = &playback.tracks[0];
    let mut tracks = vec![vec![0.0; source.len() + 64], vec![0.0; source.len() + 64]];
    for (i, sample) in source.iter().enumerate() {
        let transformed = 0.2 * (sample + nonlinearity * sample * sample);
        tracks[0][i] += transformed;
        tracks[1][i + 12] += transformed;
    }
    check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        Some(&BTreeMap::from([("FL".into(), 30.0)])),
        None,
        false,
    )
}

#[test]
fn take_checks_detect_speaker_distortion() {
    let distorted = nonlinear_take(0.4);
    let distortion_db = distorted
        .checks
        .iter()
        .find(|check| check.key == "record_check_distortion")
        .and_then(|check| check.values["distortion_db"].as_f64())
        .expect("nonlinear sweep must report distortion");
    assert!(distorted.measurements[0].direct.is_none());
    assert!(!has(&distorted, "record_check_speaker_polarity"));
    let clean = nonlinear_take(0.0);
    assert!(
        !has(&clean, "record_check_distortion"),
        "{:?}",
        clean.checks
    );
    assert!(clean.measurements[0].direct.is_some());
    eprintln!("recording impulse checks: distortion_db={distortion_db}, clean_distortion=none");
}

#[test]
fn take_checks_detect_level_change_and_swapped_microphones() {
    let reference = [
        ReferenceMeasurement {
            speaker: "FR".into(),
            itd_us: Some(250.0),
            lf_db: Some(-20.0),
            lf_ild_db: None,
            azimuth: Some(-30.0),
            direct: None,
        },
        ReferenceMeasurement {
            speaker: "FL".into(),
            itd_us: Some(-250.0),
            lf_db: Some(-20.0),
            lf_ild_db: None,
            azimuth: Some(30.0),
            direct: None,
        },
    ];
    let expected = BTreeMap::from([("FC".into(), 0.0)]);
    let level = custom_take(
        &["FC"],
        &[(0, 0)],
        &[(0.025, 0.025)],
        Some(&expected),
        Some(&reference),
    );
    assert!(
        has(&level, "record_check_level_changed"),
        "{:?}",
        level.checks
    );
    assert!(
        has(&level, "record_check_mics_swapped"),
        "{:?}",
        level.checks
    );
    assert_eq!(
        level
            .checks
            .iter()
            .filter(|check| check.key == "record_check_mics_swapped")
            .count(),
        1
    );

    let rerecorded = custom_take(
        &["FR"],
        &[(0, 12)],
        &[(0.2, 0.2)],
        Some(&BTreeMap::from([("FR".into(), -30.0)])),
        Some(&reference),
    );
    assert!(
        has(&rerecorded, "record_check_mics_swapped"),
        "{:?}",
        rerecorded.checks
    );
}

/// Retaking FL and FR with the microphones swapped is diagnosed even though
/// this folder's earlier, correct FL and FR are among the references.
#[test]
fn take_checks_report_the_input_balance() {
    let expected = BTreeMap::from([("FL".into(), 30.0), ("FR".into(), -30.0)]);
    let left_gain = 0.2 * 10_f64.powf(-6.0 / 20.0);
    let unbalanced = custom_take(
        &["FL", "FR"],
        &[(0, 12), (12, 0)],
        &[(left_gain, 0.2), (left_gain, 0.2)],
        Some(&expected),
        None,
    );
    for measurement in &unbalanced.measurements {
        assert!(
            (measurement.lf_ild_db.unwrap() + 6.0).abs() <= 0.5,
            "{measurement:?}"
        );
    }
    let warnings: Vec<_> = unbalanced
        .checks
        .iter()
        .filter(|check| check.key == "record_check_input_balance")
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", unbalanced.checks);
    assert!((warnings[0].values["db_diff"].as_f64().unwrap() + 6.0).abs() <= 0.5);

    let balanced = custom_take(
        &["FL", "FR"],
        &[(0, 12), (12, 0)],
        &[(0.2, 0.2), (0.2, 0.2)],
        Some(&expected),
        None,
    );
    assert!(!has(&balanced, "record_check_input_balance"));
}

fn headphone_take(right_gain_db: f64, reference_balance_db: f64) -> CheckResults {
    let playback = build_sweep_playback(&SweepSpec {
        fs: FS,
        duration: 0.1,
        speakers: vec!["FL".into(), "FR".into()],
        tracks: "stereo".into(),
    })
    .unwrap();
    let n = playback.tracks[0].len() + 64;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (segment_index, segment) in playback.segments.iter().enumerate() {
        let start = (segment.start * FS as f64).round() as usize;
        let end = (segment.end * FS as f64).round() as usize;
        let source = &playback.tracks[segment_index][start..end];
        for (index, sample) in source.iter().enumerate() {
            if segment.speaker == "FL" {
                tracks[0][start + index] = sample * 0.2;
            } else {
                tracks[1][start + index] = sample * 0.2 * 10_f64.powf(right_gain_db / 20.0);
            }
        }
    }
    let references = [ReferenceMeasurement {
        speaker: "FC".into(),
        itd_us: Some(0.0),
        lf_db: None,
        lf_ild_db: Some(reference_balance_db),
        azimuth: Some(0.0),
        direct: None,
    }];
    check_recording(
        &tracks,
        FS,
        &playback.segments,
        Some(&playback.estimator),
        None,
        Some(&references),
        true,
    )
}

#[test]
fn headphone_take_checks_predict_the_final_balance() {
    let swapped = headphone_take(-7.0, -6.5);
    let warning = swapped
        .checks
        .iter()
        .find(|check| check.key == "record_check_headphone_balance")
        .unwrap_or_else(|| panic!("{:?}", swapped.checks));
    assert!((warning.values["db_diff"].as_f64().unwrap() - 7.0).abs() <= 0.5);
    assert!((warning.values["predicted_db"].as_f64().unwrap() + 13.5).abs() <= 0.5);

    let matched = headphone_take(7.0, -7.0);
    assert!(!has(&matched, "record_check_headphone_balance"));
}

#[test]
fn superseded_references_do_not_hide_swapped_microphones() {
    let reference = [
        ReferenceMeasurement {
            speaker: "FL".into(),
            itd_us: Some(250.0),
            lf_db: Some(-20.0),
            lf_ild_db: None,
            azimuth: Some(30.0),
            direct: None,
        },
        ReferenceMeasurement {
            speaker: "FR".into(),
            itd_us: Some(-250.0),
            lf_db: Some(-20.0),
            lf_ild_db: None,
            azimuth: Some(-30.0),
            direct: None,
        },
    ];
    let expected = BTreeMap::from([("FL".into(), 30.0), ("FR".into(), -30.0)]);
    let result = custom_take(
        &["FL", "FR"],
        &[(12, 0), (0, 12)],
        &[(0.2, 0.2), (0.2, 0.2)],
        Some(&expected),
        Some(&reference),
    );
    assert!(
        has(&result, "record_check_mics_swapped"),
        "{:?}",
        result.checks
    );
}

fn polarity_expected() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("FL".into(), 30.0),
        ("SL".into(), 90.0),
        ("BL".into(), 150.0),
    ])
}

fn polarity_take(
    speakers: &[&str],
    gains: &[(f64, f64)],
    references: Option<&[ReferenceMeasurement]>,
) -> CheckResults {
    custom_take(
        speakers,
        &vec![(0, 12); speakers.len()],
        gains,
        Some(&polarity_expected()),
        references,
    )
}

/// The speaker-polarity checks in order, after asserting they are all
/// warnings and come after every other check.
fn polarity_checks(result: &CheckResults) -> Vec<(&'static str, String)> {
    let is_polarity = |key: &str| key.starts_with("record_check_speaker_polarity");
    let first = result
        .checks
        .iter()
        .position(|check| is_polarity(check.key))
        .unwrap_or(result.checks.len());
    assert!(
        result.checks[first..]
            .iter()
            .all(|check| is_polarity(check.key) && check.severity == "warning"),
        "{:?}",
        result.checks
    );
    result.checks[first..]
        .iter()
        .map(|check| (check.key, check.speaker.clone()))
        .collect()
}

#[test]
fn speaker_polarity_reports_one_reversed_speaker() {
    let result = polarity_take(
        &["FL", "SL", "BL"],
        &[(0.2, 0.2), (-0.2, -0.2), (0.2, 0.2)],
        None,
    );
    assert_eq!(
        polarity_checks(&result),
        [("record_check_speaker_polarity", "SL".to_owned())]
    );
    let warning = result
        .checks
        .iter()
        .find(|check| check.key == "record_check_speaker_polarity")
        .unwrap_or_else(|| panic!("{:?}", result.checks));
    assert_eq!(warning.speaker, "SL");
    assert_eq!(warning.severity, "warning");
    assert_eq!(warning.values["compared"], 2);
    assert_eq!(warning.values["opposite"], 2);
    assert_eq!(warning.values["confidence"], "1.00");
    assert!(
        result
            .measurements
            .iter()
            .all(|measurement| measurement.direct.is_some())
    );
}

#[test]
fn speaker_polarity_reports_an_equal_split_without_choosing_a_side() {
    let result = polarity_take(&["FL", "SL"], &[(0.2, 0.2), (-0.2, -0.2)], None);
    assert_eq!(
        polarity_checks(&result),
        [("record_check_speaker_polarity_split", "FL".to_owned())]
    );
    let warning = result
        .checks
        .iter()
        .find(|check| check.key == "record_check_speaker_polarity_split")
        .unwrap_or_else(|| panic!("{:?}", result.checks));
    assert_eq!(warning.speaker, "FL");
    assert_eq!(warning.values["group_a"], "FL");
    assert_eq!(warning.values["group_b"], "SL");
    assert_eq!(warning.values["confidence"], "1.00");
}

fn reversed_bl_reference() -> ReferenceMeasurement {
    let mut earlier = polarity_take(&["BL"], &[(0.2, 0.2)], None);
    let mut direct = earlier.measurements.remove(0).direct.unwrap();
    direct.left.iter_mut().for_each(|sample| *sample = -*sample);
    direct
        .right
        .iter_mut()
        .for_each(|sample| *sample = -*sample);
    ReferenceMeasurement {
        speaker: "BL".into(),
        itd_us: Some(250.0),
        lf_db: None,
        lf_ild_db: None,
        azimuth: Some(150.0),
        direct: Some(DirectSound {
            fs: direct.fs,
            left: direct.left,
            right: direct.right,
        }),
    }
}

#[test]
fn speaker_polarity_reports_a_reversed_reference_speaker() {
    let reference = [reversed_bl_reference()];
    let result = polarity_take(&["FL", "SL"], &[(0.2, 0.2), (0.2, 0.2)], Some(&reference));
    let warning = result
        .checks
        .iter()
        .find(|check| check.key == "record_check_speaker_polarity")
        .unwrap_or_else(|| panic!("{:?}", result.checks));
    assert_eq!(warning.speaker, "BL");
}

#[test]
fn rerecorded_speaker_replaces_its_polarity_reference() {
    let reference = [reversed_bl_reference()];
    let result = polarity_take(
        &["FL", "SL", "BL"],
        &[(0.2, 0.2), (0.2, 0.2), (0.2, 0.2)],
        Some(&reference),
    );
    assert!(!has(&result, "record_check_speaker_polarity"));
    assert!(!has(&result, "record_check_speaker_polarity_split"));
}

#[test]
fn rerecorded_speaker_without_signal_still_replaces_its_polarity_reference() {
    // The new BL take has no signal, so it carries no window; the earlier,
    // reversed BL reference must not come back in its place.
    let reference = [reversed_bl_reference()];
    let result = polarity_take(
        &["FL", "SL", "BL"],
        &[(0.2, 0.2), (0.2, 0.2), (0.0, 0.0)],
        Some(&reference),
    );
    let bl = result
        .measurements
        .iter()
        .find(|measurement| measurement.speaker == "BL")
        .unwrap();
    assert!(bl.direct.is_none());
    assert!(
        result
            .checks
            .iter()
            .any(|check| check.key == "record_check_no_signal" && check.speaker == "BL"),
        "{:?}",
        result.checks
    );
    assert!(polarity_checks(&result).is_empty(), "{:?}", result.checks);
}

#[test]
fn headphone_runs_emit_no_direct_sound_or_speaker_polarity_checks() {
    let result = headphone_take(0.0, 0.0);
    assert!(
        result
            .measurements
            .iter()
            .all(|measurement| measurement.direct.is_none())
    );
    assert!(!has(&result, "record_check_speaker_polarity"));
    assert!(!has(&result, "record_check_speaker_polarity_split"));
}

#[test]
fn measurement_direct_serializes_only_when_present() {
    let mut measurement = Measurement {
        speaker: "FL".into(),
        peak_db: -12.3,
        snr_db: 40.0,
        itd_us: Some(250.0),
        lf_db: Some(-20.5),
        lf_ild_db: Some(0.0),
        azimuth: Some(30.0),
        direct: None,
    };
    assert!(
        serde_json::to_value(&measurement)
            .unwrap()
            .get("direct")
            .is_none()
    );
    measurement.direct = Some(DirectSound {
        fs: 8_000,
        left: vec![0.0; 48],
        right: vec![0.0; 48],
    });
    let value = serde_json::to_value(&measurement).unwrap();
    assert_eq!(value["direct"]["fs"], 8_000);
    assert_eq!(value["direct"]["left"].as_array().unwrap().len(), 48);
    assert_eq!(value["direct"]["right"].as_array().unwrap().len(), 48);
}
