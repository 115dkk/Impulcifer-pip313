#![forbid(unsafe_code)]
//! P08 object properties, complementary to the actual 2.x golden arrays.
use impulcifer_dsp::{
    conv,
    estimator::SweepEstimator,
    hrir::{Hrir, SpeakerIrs, ingest_recording},
    ir::ImpulseResponse,
};
use impulcifer_types::constants::{HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, Side};

/// Python ImpulseResponse constructor:16-19; synthetic P08 property fixture.
fn impulse(n: usize, peak: usize) -> ImpulseResponse {
    let mut data = vec![0.0; n];
    data[peak] = 1.0;
    ImpulseResponse {
        data,
        fs: 48000,
        recording: None,
    }
}
/// Python HRIR insertion-order pairs:368-425; synthetic P08 property fixture.
fn hrir() -> Hrir {
    Hrir {
        fs: 48000,
        speakers: vec![SpeakerIrs {
            speaker: "FL".into(),
            left: Some(impulse(512, 100)),
            right: Some(impulse(512, 107)),
        }],
    }
}
/// CPython int/round/floor fixtures; private production helpers additionally have unit tests.
#[test]
fn python_rounding_helpers_match_cpython() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/migration/goldens/p08_rounding.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    for (i, x) in v["inputs"]["values"].as_array().unwrap().iter().enumerate() {
        let x = x.as_f64().unwrap();
        assert_eq!(x.trunc() as i64, v["outputs"]["trunc"][i].as_i64().unwrap());
        assert_eq!(
            x.round_ties_even() as i64,
            v["outputs"]["round"][i].as_i64().unwrap()
        );
    }
    for (i, p) in v["inputs"]["divisions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let a = p[0].as_i64().unwrap();
        let b = p[1].as_i64().unwrap();
        assert_eq!(
            (a as f64 / b as f64).floor() as i64,
            v["outputs"]["floor"][i].as_i64().unwrap()
        );
    }
}
/// Python shift:92-108; zero padding makes bounded shifts invertible.
#[test]
fn shift_is_length_preserving_and_invertible_for_zeros() {
    for shift in -100..=100 {
        let mut ir = impulse(512, 256);
        let original = ir.data.clone();
        ir.shift(shift);
        assert_eq!(ir.len(), 512);
        ir.shift(-shift);
        assert_eq!(ir.data, original);
    }
    for shift in [i64::MIN, i64::MAX, -512, 512] {
        let mut ir = impulse(512, 256);
        ir.shift(shift);
        assert_eq!(ir.data, vec![0.0; 512]);
    }
}
/// Python crop_heads:567-631; one shared crop preserves interaural delay.
#[test]
fn crop_heads_keeps_itd() {
    let mut h = hrir();
    h.crop_heads(1.0).unwrap();
    let (l, r) = h.pair("FL").unwrap();
    assert_eq!(l.peak_index(0, None, 0.12589), 48);
    assert_eq!(r.peak_index(0, None, 0.12589), 55);
    assert_eq!(l.len(), r.len());
}
/// Python normalize:476-565; peak refers to the summed-ear frequency response.
#[test]
fn normalize_hits_peak_target() {
    let mut h = hrir();
    let gain = h.normalize(Some(-0.1), None).unwrap();
    assert!((gain + 0.1).abs() < 1e-12);
    for ir in [h.pair("FL").unwrap().0, h.pair("FL").unwrap().1] {
        let max = ir
            .magnitude_response()
            .1
            .into_iter()
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max + 0.1).abs() < 1e-12);
    }
    assert!(h.normalize(None, None).is_err());
    assert!(h.normalize(Some(0.0), Some(0.0)).is_err());
}
/// Python self-pair lag signs:950-961; delay the earlier ear by seven samples.
#[test]
fn align_ipsilateral_self_pair_converges() {
    let mut h = hrir();
    h.align_ipsilateral_all(&[("FL", "FL")], 30.0);
    let (l, r) = h.pair("FL").unwrap();
    assert_eq!(l.data, r.data);
    let corr = conv::correlate(&l.data, &r.data, conv::Mode::Full);
    let index = corr
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert_eq!(index, l.len() - 1);
}
/// Python trim_silent_extensions:29-36; never remove quiet/nontrailing rows.
#[test]
fn stack_tracks_trims_only_trailing_pairs() {
    let mut h = hrir();
    assert_eq!(h.stack_tracks(&HESUVI_TRACK_ORDER, true).unwrap().len(), 14);
    assert_eq!(
        h.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, true)
            .unwrap()
            .len(),
        16
    );
    let mut quiet = impulse(512, 100);
    quiet.data[100] = 1e-30;
    h.speakers.push(SpeakerIrs {
        speaker: "TSL".into(),
        left: Some(quiet),
        right: None,
    });
    let rows = h.stack_tracks(&HESUVI_TRACK_ORDER, true).unwrap();
    assert_eq!(rows.len(), 24);
    assert!(rows[14].iter().all(|x| *x == 0.0));
    assert_eq!(rows[22][100], 1e-30);
    assert_eq!(
        h.stack_tracks(&["FL-left", "LFE-left"], true)
            .unwrap()
            .len(),
        2
    );
}
/// Python decay_params:92-97 truncates the first-pass slice before reshape.
#[test]
fn decay_first_pass_clamps_windows_to_sample_count() {
    for (data, knee, noise) in [
        (vec![1.0; 32], 32, 0.0),
        (vec![0.0; 32], 32, -200.0),
        (
            (0..32).map(|i| (-(i as f64) / 4.0).exp()).collect(),
            15,
            -44.3293823498114,
        ),
    ] {
        let p = impulcifer_dsp::decay::decay_params(&data, 16);
        assert_eq!((p.peak_index, p.knee_index, p.window_size), (0, knee, 1));
        assert!((p.noise_floor_db - noise).abs() <= 1e-9);
    }
}

/// Python write_wav:453-458 stacks selected rows, using the first IR for zeros.
#[test]
fn stack_tracks_checks_only_selected_lengths() {
    let mut h = hrir();
    h.speakers.push(SpeakerIrs {
        speaker: "FR".into(),
        left: Some(impulse(768, 100)),
        right: Some(impulse(768, 107)),
    });
    for (speaker, n) in [("FL", 512), ("FR", 768)] {
        let order = [format!("{speaker}-left"), format!("{speaker}-right")];
        let rows = h.stack_tracks(&[&order[0], &order[1]], false).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], h.pair(speaker).unwrap().0.data);
        assert_eq!(rows[1], h.pair(speaker).unwrap().1.data);
        assert_eq!(rows[0].len(), n);
    }
    let missing = h.stack_tracks(&["LFE-left", "LFE-right"], false).unwrap();
    assert_eq!(missing, vec![vec![0.0; 512]; 2]);
    assert!(h.stack_tracks(&["FL-left", "FR-right"], false).is_err());
    assert!(h.stack_tracks(&["FR-left", "LFE-right"], false).is_err());
    assert!(h.stack_tracks(&[], false).is_err());
}

/// Python generate_test_signal:124-144 rejects negative dimensions and overlapping halves.
#[test]
fn sweep_fades_reject_invalid_windows() {
    for (fade_in, fade_out) in [
        (Some(1.5), Some(1.5)),
        (Some(3.0), None),
        (None, Some(3.0)),
        (Some(-0.5), None),
        (None, Some(-0.5)),
        (Some(-f64::EPSILON), None),
        (None, Some(-f64::EPSILON)),
        (Some(f64::NAN), None),
        (None, Some(f64::INFINITY)),
    ] {
        assert!(
            std::panic::catch_unwind(|| {
                SweepEstimator::generate_test_signal(100, 2.0, 1.0, fade_in, fade_out)
            })
            .is_err(),
            "accepted fades {fade_in:?}, {fade_out:?}"
        );
    }
    let plain = SweepEstimator::generate_test_signal(100, 2.0, 1.0, None, None);
    assert_eq!(
        plain,
        SweepEstimator::generate_test_signal(100, 2.0, 1.0, Some(0.0), Some(0.0))
    );
    for fades in [(Some(1.0), Some(1.0)), (Some(2.0), None), (None, Some(2.0))] {
        let signal = SweepEstimator::generate_test_signal(100, 2.0, 1.0, fades.0, fades.1);
        assert_eq!(signal.len(), plain.len());
        assert!(signal.iter().all(|x| x.is_finite()));
    }
}

/// Python adjust_decay:151 and decay.py:366 raise before applying a zero-target window.
#[test]
fn zero_decay_target_panics_without_modifying_data() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens");
    let bytes = std::fs::read(root.join("p08_decay_synthetic_input.f64")).unwrap();
    let data: Vec<_> = bytes
        .chunks_exact(8)
        .map(|x| f64::from_le_bytes(x.try_into().unwrap()))
        .collect();
    for target in [0.0, -0.0] {
        for data in [data.clone(), vec![0.0; 32]] {
            let mut ir = ImpulseResponse {
                data,
                fs: 48000,
                recording: Some(vec![1.0, 2.0]),
            };
            let before = ir.clone();
            assert!(std::panic::catch_unwind(|| ir.decay_adjustment_params(target)).is_err());
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ir.adjust_decay(target)))
                    .is_err()
            );
            assert_eq!(ir.data, before.data);
            assert_eq!(ir.recording, before.recording);
            assert_eq!(ir.fs, before.fs);
            assert!(ir.data.iter().all(|x| x.is_finite()));
        }
    }
}

/// Python ingest i//2 overwrite and short-recording fallback:264-355.
#[test]
fn ingest_preserves_single_side_mapping_and_fallback() {
    let e = SweepEstimator {
        fs: 10,
        low: 1.0,
        high: 5.0,
        n_octaves: 1.0,
        test_signal: vec![1.0; 10],
        duration: 1.0,
        inverse_filter: vec![1.0],
    };
    let tracks = vec![vec![1.0; 10], vec![2.0; 10], vec![3.0; 10], vec![4.0; 10]];
    let out = ingest_recording(
        &e,
        10,
        10,
        &tracks,
        &["FL", "FR", "FC", "BL"],
        Some(Side::Left),
        0.0,
    )
    .unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].left.as_ref().unwrap().data, vec![2.0; 10]);
    assert_eq!(out[1].left.as_ref().unwrap().data, vec![4.0; 10]);
    let short = vec![vec![1.0; 9]; 2];
    assert_eq!(
        ingest_recording(&e, 10, 10, &short, &["FL"], None, 0.0).unwrap()[0]
            .left
            .as_ref()
            .unwrap()
            .len(),
        9
    );
    assert!(ingest_recording(&e, 10, 11, &tracks, &["FL"], None, 0.0).is_err());
    assert!(ingest_recording(&e, 10, 10, &tracks, &["FL"], None, 0.15).is_err());
}
