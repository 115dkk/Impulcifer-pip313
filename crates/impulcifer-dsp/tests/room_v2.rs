#![forbid(unsafe_code)]
use impulcifer_dsp::{
    fr::{FrequencyResponse, generate_frequencies},
    ir::ImpulseResponse,
    stages::{
        equalize::{EqInputs, equalization_curve},
        room::{FrCombination, RoomCorrectionOptions, RoomFrs, RoomTerm},
        room_v2::*,
    },
};
use impulcifer_types::constants::Side;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn room_v2_legacy_range_is_bit_exact() {
    use impulcifer_dsp::{estimator::SweepEstimator, hrir::Hrir, stages::room::*};
    let wav =
        impulcifer_io::read_wav(&root().join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"))
            .unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut rir = Hrir {
        fs: 48000,
        speakers: Vec::new(),
    };
    let mut paths: Vec<_> = std::fs::read_dir(root().join("data/demo"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        if let Some(name) = parse_room_measurement_name(path.file_name().unwrap().to_str().unwrap())
        {
            let wav = impulcifer_io::read_wav(&path).unwrap();
            rir.open_recording_samples(
                &estimator,
                wav.sample_rate,
                &wav.tracks,
                &name.speakers.iter().map(String::as_str).collect::<Vec<_>>(),
                name.side,
                2.0,
            )
            .unwrap();
        }
    }
    assert!(!rir.speakers.is_empty());
    let target = prepare_room_target(None, 48000).unwrap();
    let mut old = rir.clone();
    old.for_each_ir(|ir| ir.crop_head(1.0));
    old.crop_tails(&estimator).unwrap();
    let expected = calculate_specific_room_corrections(&old, &target, None, 400.0).unwrap();
    let actual = room_correction(
        &mut rir,
        &[],
        &target,
        None,
        &estimator,
        &RoomCorrectionOptions {
            range: RoomRange::Legacy,
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(actual.frs.term, RoomTerm::LegacyError);
    assert!(actual.diagnostics.is_none());
    assert_eq!(actual.frs.entries, expected.entries);
}

fn grid() -> Vec<f64> {
    generate_frequencies(10.0, 24000.0, 1.01)
}
fn hp(f: f64, corner: f64, order: i32) -> f64 {
    -10.0 * (1.0 + (corner / f).powi(2 * order)).log10()
}
fn index(f: &[f64], hz: f64) -> usize {
    f.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - hz).abs().total_cmp(&(*b - hz).abs()))
        .unwrap()
        .0
}
fn response(fun: impl Fn(f64) -> f64) -> FrequencyResponse {
    let f = grid();
    let mut fr = FrequencyResponse::constant("synthetic", Some(f.clone()), 0.0, 0.0).unwrap();
    fr.raw = f.iter().map(|f| fun(*f)).collect();
    fr.error = fr.raw.clone();
    fr
}
fn options(range: RoomRange) -> RoomCorrectionOptions {
    RoomCorrectionOptions {
        range,
        schroeder_freq: Some(400.0),
        ..Default::default()
    }
}
fn peak(f: f64, center: f64, q: f64, gain: f64) -> f64 {
    gain / (1.0 + (q * (f / center - center / f)).powi(2))
}
fn gain(
    fr: &mut FrequencyResponse,
    options: &RoomCorrectionOptions,
    generic: bool,
    snr: Option<&[f64]>,
    roll: Option<Rolloff>,
) {
    correction_gain(fr, snr, roll, 400.0, generic, 48000, options).unwrap();
}
#[test]
fn room_v2_rolloff_detects_sealed_and_ported_speakers() {
    let f = grid();
    for (corner, order) in [(50.0, 2), (45.0, 4)] {
        let m: Vec<_> = f.iter().map(|f| hp(*f, corner, order)).collect();
        let r = detect_rolloff(&f, &m, None).unwrap();
        let minus6 = corner / (10.0_f64.powf(0.6) - 1.0).powf(1.0 / (2 * order) as f64);
        assert!(
            (r.freq / minus6).log2().abs() <= 0.25,
            "{r:?}, expected {minus6}"
        );
        assert!(
            r.knee >= corner && r.knee <= corner * 2.0_f64.powf(0.75),
            "{r:?}"
        );
        let boundary: Vec<_> = f
            .iter()
            .zip(&m)
            .map(|(f, m)| m + 8.0 / (1.0 + (f / 100.0).powi(2)))
            .collect();
        let b = detect_rolloff(&f, &boundary, None).unwrap();
        assert!(b.freq < r.freq, "{b:?} vs {r:?}");
    }
    for boundary in [0.0, 8.0] {
        let m: Vec<_> = f
            .iter()
            .map(|f| {
                -12.0 * (-0.5 * ((f / 35.0).log2() / (1.0 / 12.0)).powi(2)).exp()
                    + boundary / (1.0 + (f / 100.0).powi(2))
            })
            .collect();
        assert!(detect_rolloff(&f, &m, None).is_none());
    }
}
fn noise_ir(t60: f64) -> ImpulseResponse {
    let mut seed = 7_u64;
    let data = (0..96000)
        .map(|i| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((seed >> 11) as f64 / (1_u64 << 53) as f64) * 2.0 - 1.0;
            noise * 10.0_f64.powf(-3.0 * i as f64 / (48000.0 * t60))
        })
        .collect();
    ImpulseResponse {
        data,
        fs: 48000,
        recording: None,
    }
}
#[test]
fn room_v2_schroeder_estimate_sources_and_clamps() {
    let ir = noise_ir(0.4);
    let s = estimate_schroeder(&[&ir], None, None);
    assert_eq!(s.source, SchroederSource::AssumedVolume);
    assert!((s.t60.unwrap() / 0.4 - 1.0).abs() < 0.15, "{s:?}");
    assert!((120.0..=300.0).contains(&s.freq));
    let v = estimate_schroeder(&[&ir], Some(50.0), None);
    assert_eq!(v.source, SchroederSource::Volume);
    assert!((v.freq - 2000.0 * (v.t60.unwrap() / 50.0).sqrt()).abs() < 1e-10);
    assert_eq!(estimate_schroeder(&[&ir], Some(0.01), None).freq, 500.0);
    assert_eq!(estimate_schroeder(&[&ir], Some(10000.0), None).freq, 80.0);
    assert_eq!(
        estimate_schroeder(&[], None, None).source,
        SchroederSource::Fallback
    );
    assert_eq!(estimate_schroeder(&[], None, None).freq, 200.0);
    for (input, expected) in [(20.0, 50.0), (400.0, 400.0), (2000.0, 1000.0)] {
        let s = estimate_schroeder(&[&ir], Some(0.01), Some(input));
        assert_eq!(s.source, SchroederSource::Override);
        assert_eq!(s.freq, expected);
    }
}
#[test]
fn room_v2_schroeder_range_corrects_modes_and_fills_specific_dips() {
    let mut fr = response(|f| {
        peak(f, 60.0, 6.0, 8.0) + peak(f, 120.0, 6.0, -10.0) + peak(f, 210.0, 6.0, 6.0)
    });
    gain(&mut fr, &options(RoomRange::Schroeder), false, None, None);
    for (f, g) in [(60.0, -8.0), (120.0, 10.0), (210.0, -6.0)] {
        let actual = fr.equalization[index(&fr.frequency, f)];
        assert!((actual - g).abs() <= 1.0, "{f}: {actual}");
    }
    assert!(
        fr.frequency
            .iter()
            .zip(fr.equalization)
            .filter(|(f, _)| **f >= 400.0)
            .all(|(_, g)| g == 0.0)
    );
}
#[test]
fn room_v2_modes_range_keeps_bass_balance() {
    let mut fr = response(|f| 6.0 / (1.0 + (f / 80.0).powi(12)) + peak(f, 60.0, 5.0, 8.0));
    gain(&mut fr, &options(RoomRange::Modes), false, None, None);
    assert!(
        fr.equalization[index(&fr.frequency, 60.0)] <= -6.0,
        "{}",
        fr.equalization[index(&fr.frequency, 60.0)]
    );
    assert!(fr.equalization[index(&fr.frequency, 30.0)].abs() <= 1.5);
}
#[test]
fn room_v2_rolloff_is_never_boosted() {
    let mut fr = response(|f| hp(f, 45.0, 4));
    let r = detect_rolloff(&fr.frequency, &fr.raw, None).unwrap();
    gain(
        &mut fr,
        &options(RoomRange::Schroeder),
        false,
        None,
        Some(r),
    );
    for (f, g) in fr.frequency.iter().zip(fr.equalization) {
        if *f <= r.freq {
            assert!(g <= 0.01);
        } else if *f < r.knee {
            let ramp = 0.5
                * (1.0
                    - (std::f64::consts::PI * (f / r.freq).log2() / (r.knee / r.freq).log2())
                        .cos());
            assert!(g <= 12.0 * ramp + 1e-12);
        }
    }
}
#[test]
fn room_v2_self_check_warns_when_residual_is_large() {
    let mut fr = response(|_| -20.0);
    let original = fr.error.clone();
    let opt = options(RoomRange::Schroeder);
    gain(&mut fr, &opt, false, None, None);
    let (rms, lo, hi) = residual(&fr, &original, None, &opt, 400.0).unwrap();
    assert!(rms.unwrap() > 3.0);
    assert_eq!(lo, 20.0);
    assert_eq!(hi, 400.0 / 2.0_f64.sqrt());
}
#[test]
fn room_v2_snr_caps_boosts() {
    let mut fr = response(|f| peak(f, 120.0, 5.0, -10.0));
    let snr = vec![25.0; fr.frequency.len()];
    gain(
        &mut fr,
        &options(RoomRange::Schroeder),
        false,
        Some(&snr),
        None,
    );
    assert!(fr.equalization.iter().all(|g| *g <= 5.0));
    assert!((fr.equalization[index(&fr.frequency, 120.0)] - 5.0).abs() < 1e-12);
    assert!(
        snr_db(
            &ImpulseResponse {
                data: vec![0.0; 100],
                fs: 48000,
                recording: None
            },
            &grid()
        )
        .is_none()
    );
    let mut silent_tail = ImpulseResponse {
        data: vec![0.0; 48000],
        fs: 48000,
        recording: None,
    };
    silent_tail.data[0] = 1.0;
    assert!(
        snr_db(&silent_tail, &grid())
            .unwrap()
            .iter()
            .all(|s| s.is_finite() && *s > 100.0)
    );
    let snr = snr_db(&noise_ir(0.4), &grid()).unwrap();
    assert!(snr.iter().all(|v| v.is_finite()));
}
#[test]
fn room_v2_generic_average_is_cut_only() {
    let mut fr = response(|f| peak(f, 120.0, 5.0, -10.0) + peak(f, 60.0, 5.0, 8.0));
    let mut opt = options(RoomRange::Schroeder);
    opt.fr_combination_method = FrCombination::Average;
    gain(&mut fr, &opt, true, None, None);
    assert!(fr.equalization.iter().all(|g| *g <= 0.0));
    assert!(fr.equalization[index(&fr.frequency, 60.0)] < -6.0);
}
#[test]
fn room_v2_extreme_is_diotic_above_700_hz() {
    let mut left = response(|f| peak(f, 2000.0, 4.0, 4.0));
    let mut right = response(|f| peak(f, 2000.0, 4.0, -4.0));
    let snr = vec![21.0; left.frequency.len()];
    gain(&mut left, &options(RoomRange::Extreme), false, None, None);
    gain(
        &mut right,
        &options(RoomRange::Extreme),
        false,
        Some(&snr),
        None,
    );
    let mut frs = RoomFrs {
        term: RoomTerm::Gain,
        entries: vec![
            ("FL".into(), Side::Left, left),
            ("FL".into(), Side::Right, right),
        ],
    };
    blend_diotic(&mut frs);
    for (i, f) in frs.entries[0].2.frequency.iter().enumerate() {
        let l = frs.entries[0].2.equalization[i];
        let r = frs.entries[1].2.equalization[i];
        if *f >= 700.0 {
            assert!((l - r).abs() < 1e-9);
        }
        if *f >= 10000.0 {
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
        }
    }
}
#[test]
fn room_v2_vbass_handoff_mask_follows_lr8() {
    let f: Vec<_> = [-0.25, 0.0, 0.25, 0.5, 1.0]
        .iter()
        .map(|x| 250.0 * 2.0_f64.powf(*x))
        .collect();
    for (g, expected) in vbass_handoff_mask(&f, 250.0, 48000)
        .iter()
        .zip([0.20, 0.50, 0.80, 0.94, 0.996])
    {
        assert!((g - expected).abs() < 0.02);
    }
    assert_eq!(
        vbass_handoff_mask(&[10.0, 100.0, 176.0], 250.0, 48000),
        vec![0.0; 3]
    );
}
#[test]
fn room_v2_gain_is_added_after_heavy_light_smoothing() {
    let target = response(|_| 0.0);
    let mut room = response(|_| 0.0);
    room.equalization = room
        .frequency
        .iter()
        .map(|f| {
            if (f / 120.0).log2().abs() <= 1.0 / 12.0 {
                10.0
            } else {
                0.0
            }
        })
        .collect();
    room.error = room.equalization.iter().map(|g| -g).collect();
    let room = RoomFrs {
        term: RoomTerm::Gain,
        entries: vec![("FL".into(), Side::Left, room)],
    };
    let eq = equalization_curve(
        &EqInputs {
            room_frs: Some(&room),
            hp: None,
            eq_left: None,
            eq_right: None,
            target: &target,
            fs: 48000,
        },
        "FL",
        Side::Left,
    )
    .unwrap();
    assert!((eq.equalization[index(&eq.frequency, 120.0)] - 10.0).abs() < 0.5);
}
