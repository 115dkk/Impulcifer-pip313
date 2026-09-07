#![forbid(unsafe_code)]
//! P09 deterministic property sweeps and error/reset/quirk contracts.
use impulcifer_dsp::fr::{
    self, CenterAt, CompensateOptions, EqualizeParams, FrFields, FrequencyResponse, ResetFlags,
    SmoothingParams,
};

/// Synthetic log response for Python FR properties (42-117); p09_interpolate_*.
fn response(phase: f64) -> FrequencyResponse {
    let f = fr::generate_frequencies(20.0, 20000.0, 1.01);
    let raw = f
        .iter()
        .map(|f| 4.0 * (f.log10() * 3.0 + phase).sin())
        .collect();
    FrequencyResponse::new("property", Some(f), Some(raw)).unwrap()
}

/// Python round used by _window_size (1047); p09_window.json.
#[test]
fn round_half_even_matches_python() {
    for (v, e) in [
        (23.5, 24),
        (24.5, 24),
        (-0.5, 0),
        (2.5, 2),
        (-1.5, -2),
        (-2.5, -2),
    ] {
        assert_eq!(fr::round_half_even(v), e);
    }
    for n in -1000..1000 {
        let expected = if n % 2 == 0 { n } else { n + 1 };
        assert_eq!(fr::round_half_even(n as f64 + 0.5), expected);
    }
    let flags = ResetFlags::default();
    assert!(
        !flags.raw
            && flags.smoothed
            && flags.error
            && flags.error_smoothed
            && flags.equalization
            && flags.equalized_raw
            && flags.equalized_smoothed
            && flags.target
            && flags.parametric_eq
            && flags.fixed_band_eq
    );
}

/// Python interpolate/_sort (119-146,859-901); p09_interpolate_*.json.
#[test]
fn interpolate_is_identity_on_its_own_grid() {
    for phase in [0.0, 0.5, 2.0, 8.0] {
        for k in [1, 2, 3] {
            let mut fr = response(phase);
            let original = fr.raw.clone();
            fr.smoothed = original.clone();
            fr.error = original.clone();
            let f = fr.frequency.clone();
            fr.interpolate(Some(&f), 1.01, k, 20.0, 20000.0).unwrap();
            assert_eq!(fr.frequency, f);
            assert!(fr.smoothed.is_empty());
            for (a, b) in fr.raw.iter().zip(original) {
                assert!((a - b).abs() < 1e-12);
            }
            assert_eq!(fr.raw, fr.error);
        }
    }
    let sorted = FrequencyResponse::with_fields(
        " sorted ",
        Some(vec![200.0, 20.0, 100.0]),
        FrFields {
            raw: Some(vec![2.0, 0.0, 1.0]),
            target: Some(vec![12.0, 10.0, 11.0]),
            ..FrFields::default()
        },
    )
    .unwrap();
    assert_eq!(sorted.name, "sorted");
    assert_eq!(sorted.raw, [0.0, 1.0, 2.0]);
    assert_eq!(sorted.target, [10.0, 11.0, 12.0]);
    assert!(FrequencyResponse::new("  ", None, None).is_err());
    assert!(FrequencyResponse::new("shape", Some(vec![20.0, 30.0]), Some(vec![1.0])).is_err());
    let error = FrequencyResponse::new("dup", Some(vec![30.0, 20.0, 20.0]), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("20"));
    let mut fr = response(0.0);
    fr.error = fr.raw.clone();
    fr.raw[17] = f64::NAN;
    assert!(fr.interpolate_default().is_err()); // only raw/frequency prune, error does not
    let mut fr = response(0.0);
    fr.raw[17] = f64::NAN;
    assert!(
        fr.smoothen_fractional_octave(&SmoothingParams::default())
            .is_err()
    );
    let mut fr = response(0.0);
    assert!(fr.smoothen(1.0 / 3.0, 1.0 / 3.0, 100.0, 100.0).is_err());
}

/// Python center (903-940), core/hrir.py get_center_value (44-74); p09_center_*.
#[test]
fn center_band_then_center_value_is_zero() {
    for phase in [0.0, 0.5, 1.0, 8.0] {
        let mut fr = response(phase); // same grid as center's equal-energy copy
        fr.error = fr.raw.clone();
        fr.smoothed = fr.raw.clone();
        fr.error_smoothed = fr.raw.clone();
        fr.target = vec![2.0; fr.frequency.len()];
        fr.equalization = fr.target.clone();
        fr.equalized_raw = fr.target.clone();
        fr.equalized_smoothed = fr.target.clone();
        let before = fr.clone();
        let shift = fr.center(CenterAt::Band(100.0, 10000.0)).unwrap();
        assert!(fr.center_value((100.0, 10000.0)).abs() < 1e-12);
        assert_eq!(fr.target, before.target);
        assert!(
            fr.equalization.is_empty()
                && fr.equalized_raw.is_empty()
                && fr.equalized_smoothed.is_empty()
        );
        for i in 0..fr.frequency.len() {
            assert!((fr.raw[i] - before.raw[i] - shift).abs() < 1e-12);
            assert!((fr.error[i] - before.error[i] + shift).abs() < 1e-12);
            assert_eq!(fr.raw[i], fr.smoothed[i]);
            assert_eq!(fr.error[i], fr.error_smoothed[i]);
        }
    }
}

/// Python equalize (1241-1310); p09_equalize_*.json. Kept samples obey the
/// ceiling; kink gaps may overshoot because Python does not re-clamp.
#[test]
fn equalize_never_exceeds_max_gain_away_from_kinks() {
    for phase in [0.0, 0.5, 2.0, 8.0] {
        for smoothen in [false, true] {
            let mut fr = response(phase);
            fr.error = fr.raw.iter().map(|v| v * 4.0).collect();
            let params = EqualizeParams {
                smoothen,
                max_gain: 6.0,
                ..EqualizeParams::default()
            };
            let clipped: Vec<_> = fr.error.iter().map(|e| -e > 6.0).collect();
            let half = (fr::window_size(&fr.frequency, 1.0 / 12.0) - 1) / 2;
            let mut away = vec![true; clipped.len()];
            for i in 1..clipped.len() {
                if clipped[i] != clipped[i - 1] {
                    away[i.saturating_sub(half)..(i + half + 1).min(clipped.len())].fill(false);
                }
            }
            fr.equalized_smoothed = vec![123.0];
            fr.equalize(&params).unwrap();
            assert_eq!(fr.equalized_smoothed, [123.0]); // retained without smoothed input
            let mut compared = 0;
            for (i, &value) in fr.equalization.iter().enumerate() {
                if !smoothen || away[i] {
                    assert!(value <= 6.0 + 1e-12);
                    compared += 1;
                }
            }
            assert!(compared > 500);
        }
    }
    let mut fr = response(0.0);
    assert!(fr.equalize(&EqualizeParams::default()).is_err());
    fr.error = vec![f64::NAN; fr.frequency.len()];
    assert!(fr.equalize(&EqualizeParams::default()).is_err());
    // Explicit signed-error source selection, overriding the unsmoothed error.
    fr.error_smoothed = vec![-2.0; fr.frequency.len()];
    fr.equalize(&EqualizeParams::default()).unwrap();
    assert!(fr.equalization.iter().all(|e| (e - 2.0).abs() < 1e-12));
}

/// Python compensate (984-1031); p09_compensate_*.json. Includes scalar creation,
/// borrowed compensation immutability and both mean-recentring choices.
#[test]
fn compensate_error_is_raw_minus_target() {
    for phase in [0.0, 0.5, 2.0, 8.0] {
        for minimum in [false, true] {
            let mut fr = response(phase);
            let target = response(phase + 1.0);
            let before = target.clone();
            fr.compensate(
                &target,
                &CompensateOptions {
                    min_mean_error: minimum,
                    bass_boost_gain: 4.0,
                    tilt: Some(-1.0),
                    ..CompensateOptions::default()
                },
            )
            .unwrap();
            assert_eq!(target, before);
            for i in 0..fr.frequency.len() {
                assert!((fr.error[i] - (fr.raw[i] - fr.target[i])).abs() < 1e-12);
            }
            if minimum {
                let f = FrequencyResponse::new(
                    "error",
                    Some(fr.frequency.clone()),
                    Some(fr.error.clone()),
                )
                .unwrap();
                assert!(f.center_value((100.0, 10000.0)).abs() < 1e-12);
            }
        }
    }
    let fr = FrequencyResponse::constant("constant", None, 0.0, 0.0).unwrap();
    assert_eq!(fr.frequency.len(), 695);
    assert!(fr.raw.iter().chain(&fr.error).all(|&v| v == 0.0));
    let mut fr = response(0.0);
    let target = FrequencyResponse::constant("short", Some(vec![20.0, 30.0]), 0.0, 0.0).unwrap();
    assert!(
        fr.compensate(&target, &CompensateOptions::default())
            .is_err()
    );
}

/// Python read_from_csv float regex (191-240); p09_csv_single.json.
#[test]
fn read_from_csv_rejects_single_digit_numbers() {
    let fr = FrequencyResponse::parse_csv("single", "5,2\n-3,4\n20.0,3\n200.0,-3.5\n").unwrap();
    assert_eq!(fr.frequency, [200.0]);
    assert_eq!(fr.raw, [-3.5]);
    assert!(fr.error.is_empty());
    let empty = FrequencyResponse::parse_csv("empty", "5,2\n-3,4\n").unwrap();
    assert_eq!(
        empty.frequency,
        fr::generate_frequencies(20.0, 20000.0, 1.01)
    );
    assert!(empty.raw.is_empty());
    let strict = "frequency,raw,target\n20.0,0.0,1.0\n200.0,1.0,2.0\n";
    assert_eq!(
        FrequencyResponse::parse_csv("strict", strict)
            .unwrap()
            .target,
        [1.0, 2.0]
    );
    // Literal CRLF and a BOM miss the strict header and enter the guess branch.
    assert!(
        FrequencyResponse::parse_csv("crlf", &strict.replace('\n', "\r\n"))
            .unwrap()
            .target
            .is_empty()
    );
    assert!(
        FrequencyResponse::parse_csv("bom", &format!("\u{feff}{strict}"))
            .unwrap()
            .target
            .is_empty()
    );
}
