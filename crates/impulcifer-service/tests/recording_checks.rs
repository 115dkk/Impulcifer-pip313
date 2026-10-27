#![forbid(unsafe_code)]

use impulcifer_service::recording::{
    checks::{CheckResults, check_recording},
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
    let no_expected = check_recording(&tracks, FS, &[segment], Some(&playback.estimator), None);
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
    );
    let itd = result.measurements[0].itd_us.unwrap();
    assert!((itd - 250.0).abs() <= 25.0, "{itd}");
    assert!(result.checks.is_empty(), "{:?}", result.checks);
}
