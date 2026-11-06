#![forbid(unsafe_code)]

use impulcifer_dsp::{
    filters::{BType, butter, sosfilt},
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    itd::{Ear, ItdMode, analyze, correct, estimate, fractional_delay, measure_after},
    windows,
};
use impulcifer_types::layouts::SLOTS;
use std::f64::consts::PI;

const FS: u32 = 48_000;
const RADIUS_CM: f64 = 9.0;
const YAW_DEG: f64 = 5.0;
const OFFSET_US: f64 = -20.0;

fn lowpassed_click() -> Vec<f64> {
    let mut click = vec![0.0; 2_048];
    click[480] = 1.0;
    let lowpass = butter(2, 1_500.0 / (FS as f64 / 2.0), BType::Lowpass);
    sosfilt(&lowpass, &click)
}

fn model_us(azimuth: f64, elevation: f64, radius_cm: f64) -> f64 {
    let theta = (azimuth.to_radians().sin().abs() * elevation.to_radians().cos()).asin();
    if theta.to_degrees() < 5.0 {
        0.0
    } else {
        (radius_cm / 100.0 / 343.0 * (theta + theta.sin()) * 1_000_000.0).copysign(azimuth)
    }
}

fn pulse_components() -> (Vec<f64>, Vec<f64>) {
    let n = 4_096;
    let mut low = vec![0.0; n];
    low[768] = 1.0;
    low = sosfilt(
        &butter(2, 1_200.0 / (FS as f64 / 2.0), BType::Lowpass),
        &low,
    );
    let burst_len = 240;
    let window = windows::hann(burst_len, false);
    let mut high = vec![0.0; n];
    for (index, value) in high[648..648 + burst_len].iter_mut().enumerate() {
        *value = 0.2 * window[index] * (2.0 * PI * 8_000.0 * index as f64 / FS as f64).sin();
    }
    (low, high)
}

fn delayed_ears(code: &str) -> (Vec<f64>, Vec<f64>) {
    let geometry = SLOTS.iter().find(|slot| slot.code == code).unwrap();
    let itd_us = if geometry.azimuth.to_radians().sin().abs() < 0.05 {
        OFFSET_US
    } else {
        model_us(geometry.azimuth - YAW_DEG, geometry.elevation, RADIUS_CM) + OFFSET_US
    };
    let (low, high) = pulse_components();
    let left_gain = 10_f64.powf(-6.5 / 20.0);
    let left_high_gain = 10_f64.powf(-3.5 / 20.0);
    let left: Vec<_> = low
        .iter()
        .zip(&high)
        .map(|(low, high)| low * left_gain + high * left_high_gain)
        .collect();
    let right: Vec<_> = low
        .iter()
        .zip(&high)
        .map(|(low, high)| low + high)
        .collect();
    if itd_us >= 0.0 {
        (left, fractional_delay(&right, FS, itd_us))
    } else {
        (fractional_delay(&left, FS, -itd_us), right)
    }
}

fn synthetic_head() -> Hrir {
    let speakers = ["FL", "FR", "FC", "SL", "SR", "BL", "BR", "TFL", "TFR"]
        .into_iter()
        .map(|speaker| {
            let (left, right) = delayed_ears(speaker);
            SpeakerIrs {
                speaker: speaker.into(),
                left: Some(ImpulseResponse {
                    data: left,
                    fs: FS,
                    recording: None,
                }),
                right: Some(ImpulseResponse {
                    data: right,
                    fs: FS,
                    recording: None,
                }),
            }
        })
        .collect();
    Hrir { fs: FS, speakers }
}

#[test]
fn itd_estimate_recovers_integer_and_fractional_delays() {
    let left = lowpassed_click();
    for samples in [7.0, 7.4] {
        let right = fractional_delay(&left, FS, samples * 1_000_000.0 / FS as f64);
        let result = estimate(&left, &right, FS, 1.0).unwrap();
        let expected = samples * 1_000_000.0 / FS as f64;
        assert!((result.itd_us - expected).abs() <= 2.0, "{result:?}");
        let swapped = estimate(&right, &left, FS, 1.0).unwrap();
        assert!((swapped.itd_us + expected).abs() <= 2.0, "{swapped:?}");
        let inverted: Vec<_> = right.iter().map(|value| -*value).collect();
        assert!(estimate(&left, &inverted, FS, 1.0).unwrap().inverted);
    }
}

#[test]
fn fractional_delay_matches_integer_shift_and_keeps_length() {
    let source = lowpassed_click();
    let delay_samples = 11;
    let delayed = fractional_delay(&source, FS, delay_samples as f64 * 1_000_000.0 / FS as f64);
    let mut shifted = ImpulseResponse {
        data: source.clone(),
        fs: FS,
        recording: None,
    };
    shifted.shift(delay_samples);
    assert_eq!(delayed.len(), source.len());
    let peak = source.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let difference = delayed
        .iter()
        .zip(&shifted.data)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(difference < 1e-9 * peak, "{difference:e}");

    let unchanged = fractional_delay(&source, FS, 0.0);
    assert_eq!(unchanged.len(), source.len());
    assert!(
        unchanged
            .iter()
            .zip(&source)
            .all(|(a, b)| (a - b).abs() < 1e-12)
    );
}

#[test]
fn analyze_reports_yaw_offset_and_balance_on_a_synthetic_head() {
    let report = analyze(&synthetic_head(), None, 1.5);
    assert!(
        (report.input_balance_db.unwrap() + 6.5).abs() <= 0.2,
        "{report:#?}"
    );
    assert!(
        (report.common_offset_us.unwrap() - OFFSET_US).abs() <= 5.0,
        "{report:#?}"
    );
    assert!(
        (report.yaw_deg.unwrap() - YAW_DEG).abs() <= 1.0,
        "{report:#?}"
    );
    assert!(
        (report.head_radius_cm - RADIUS_CM).abs() <= 1.0,
        "{report:#?}"
    );
    assert!(report.hf_asymmetry_db.unwrap().abs() > 0.5);
    for pair in &report.pairs {
        let left = SLOTS.iter().find(|slot| slot.code == pair.left).unwrap();
        let right = SLOTS.iter().find(|slot| slot.code == pair.right).unwrap();
        let expected_sum = model_us(left.azimuth - YAW_DEG, left.elevation, RADIUS_CM)
            + model_us(right.azimuth - YAW_DEG, right.elevation, RADIUS_CM)
            + 2.0 * OFFSET_US;
        assert_eq!(pair.sum_us.signum(), expected_sum.signum(), "{pair:?}");
    }
}

fn peak_index(data: &[f64]) -> usize {
    data.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .unwrap()
        .0
}

#[test]
fn symmetric_correction_makes_pairs_symmetric_and_centres_zero() {
    let mut hrir = synthetic_head();
    let original = hrir.clone();
    let mut report = analyze(&hrir, None, 1.5);
    let corrections = correct(&mut hrir, &mut report, ItdMode::Symmetric);
    measure_after(&hrir, &mut report, 1.5);
    assert!(corrections.iter().all(|correction| !correction.skipped));
    for pair in &report.pairs {
        let left = report
            .speakers
            .iter()
            .find(|speaker| speaker.speaker == pair.left)
            .unwrap()
            .after_us
            .unwrap();
        let right = report
            .speakers
            .iter()
            .find(|speaker| speaker.speaker == pair.right)
            .unwrap()
            .after_us
            .unwrap();
        assert!((left + right).abs() <= 3.0, "{} {}", pair.left, pair.right);
    }
    let centre = report
        .speakers
        .iter()
        .find(|speaker| speaker.speaker == "FC")
        .unwrap();
    assert!(centre.after_us.unwrap().abs() <= 3.0);
    for correction in &corrections {
        let before = original.get(&correction.speaker).unwrap();
        let after = hrir.get(&correction.speaker).unwrap();
        match correction.ear {
            Ear::Left => {
                assert_eq!(
                    after.right.as_ref().unwrap().data,
                    before.right.as_ref().unwrap().data
                );
                assert!(
                    peak_index(&after.left.as_ref().unwrap().data)
                        >= peak_index(&before.left.as_ref().unwrap().data)
                );
            }
            Ear::Right => {
                assert_eq!(
                    after.left.as_ref().unwrap().data,
                    before.left.as_ref().unwrap().data
                );
                assert!(
                    peak_index(&after.right.as_ref().unwrap().data)
                        >= peak_index(&before.right.as_ref().unwrap().data)
                );
            }
        }
    }
}

#[test]
fn model_correction_targets_the_spherical_head() {
    let mut hrir = synthetic_head();
    let mut report = analyze(&hrir, Some(RADIUS_CM), 1.5);
    correct(&mut hrir, &mut report, ItdMode::Model);
    measure_after(&hrir, &mut report, 1.5);
    for speaker in &report.speakers {
        let geometry = SLOTS
            .iter()
            .find(|slot| slot.code == speaker.speaker)
            .unwrap();
        if geometry.azimuth.to_radians().sin().abs() >= 5.0_f64.to_radians().sin() {
            assert!(
                (speaker.after_us.unwrap() - speaker.model_us).abs() <= 3.0,
                "{speaker:?}"
            );
        }
    }
}

#[test]
fn corrections_beyond_the_limit_are_skipped() {
    let mut hrir = Hrir {
        fs: FS,
        speakers: synthetic_head()
            .speakers
            .into_iter()
            .filter(|speaker| speaker.speaker == "FL")
            .collect(),
    };
    let before = hrir.clone();
    let mut report = analyze(&hrir, Some(RADIUS_CM), 1.5);
    report.common_offset_us = Some(2_000.0);
    let corrections = correct(&mut hrir, &mut report, ItdMode::Symmetric);
    assert_eq!(corrections.len(), 1);
    assert!(corrections[0].skipped);
    assert_eq!(
        hrir.speakers[0].left.as_ref().unwrap().data,
        before.speakers[0].left.as_ref().unwrap().data
    );
    assert_eq!(
        hrir.speakers[0].right.as_ref().unwrap().data,
        before.speakers[0].right.as_ref().unwrap().data
    );
}

#[test]
fn itd_off_reads_but_does_not_write() {
    let mut hrir = synthetic_head();
    let before = hrir.clone();
    let mut report = analyze(&hrir, None, 1.5);
    assert!(correct(&mut hrir, &mut report, ItdMode::Off).is_empty());
    for (actual, expected) in hrir.speakers.iter().zip(&before.speakers) {
        assert_eq!(
            actual.left.as_ref().unwrap().data,
            expected.left.as_ref().unwrap().data
        );
        assert_eq!(
            actual.right.as_ref().unwrap().data,
            expected.right.as_ref().unwrap().data
        );
    }
}
