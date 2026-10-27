#![forbid(unsafe_code)]

use impulcifer_service::recording::{
    checks::{CheckResults, ReferenceMeasurement, check_recording},
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
        tracks: "stereo".into(),
    })
    .unwrap();
    let n = playback.tracks[0].len() + 128;
    let mut tracks = vec![vec![0.0; n], vec![0.0; n]];
    for (segment_index, segment) in playback.segments.iter().enumerate() {
        let source_start = (segment.start * FS as f64).round() as usize;
        let source_end = (segment.end * FS as f64).round() as usize;
        let source = &playback.tracks[segment_index.min(playback.tracks.len() - 1)]
            [source_start..source_end];
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
    );
    assert!(has(&result, "record_check_polarity"), "{:?}", result.checks);
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
            azimuth: Some(30.0),
        },
        ReferenceMeasurement {
            speaker: "FR".into(),
            itd_us: Some(-250.0),
            lf_db: None,
            azimuth: Some(-30.0),
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
    let clean = nonlinear_take(0.0);
    assert!(
        !has(&clean, "record_check_distortion"),
        "{:?}",
        clean.checks
    );
    eprintln!("recording impulse checks: distortion_db={distortion_db}, clean_distortion=none");
}

#[test]
fn take_checks_detect_level_change_and_swapped_microphones() {
    let reference = [
        ReferenceMeasurement {
            speaker: "FR".into(),
            itd_us: Some(250.0),
            lf_db: Some(-20.0),
            azimuth: Some(-30.0),
        },
        ReferenceMeasurement {
            speaker: "FL".into(),
            itd_us: Some(-250.0),
            lf_db: Some(-20.0),
            azimuth: Some(30.0),
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
