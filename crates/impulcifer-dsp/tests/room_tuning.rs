#![forbid(unsafe_code)]
use impulcifer_dsp::{
    conv::{self, Mode},
    fft::{self, Complex64},
    filters::{self, BType},
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    stages::room_tuning::{self, TuningOptions},
};
use impulcifer_types::{
    config::{PhaseLimit, TuningDelay},
    constants::Side,
};
use std::f64::consts::PI;
const FS: u32 = 48000;
fn design(h: &Hrir, options: &TuningOptions) -> room_tuning::TuningPlan {
    room_tuning::prepare_tuning(h, options, None, None, &options.pairs).unwrap()
}
fn options() -> TuningOptions {
    TuningOptions {
        level_match: false,
        ..Default::default()
    }
}
fn peak(x: &[f64]) -> usize {
    x.iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0
}
#[test]
fn room_tuning_excess_part_is_all_pass() {
    let p = design(&synthetic(), &options());
    assert!(
        p.report.speakers[0]
            .excess
            .iter()
            .all(|v| (v.norm() - 1.).abs() <= 1e-6)
    );
}
#[test]
fn room_tuning_uses_one_filter_per_speaker() {
    let h = synthetic();
    let p = design(&h, &options());
    let f = &p.report.speakers[0].filter;
    let s = &h.speakers[0];
    let l = &s.left.as_ref().unwrap().data;
    let r = &s.right.as_ref().unwrap().data;
    let a = room_tuning::spectrum(l, 131072);
    let b = room_tuning::spectrum(r, 131072);
    let c = room_tuning::spectrum(&conv::convolve(l, f, Mode::Full), 131072);
    let d = room_tuning::spectrum(&conv::convolve(r, f, Mode::Full), 131072);
    let error = a
        .iter()
        .zip(b)
        .zip(c.iter().zip(d))
        .map(|((a, b), (c, d))| ((c / d) / (a / b) - Complex64::new(1., 0.)).norm())
        .fold(0., f64::max);
    println!("shared FIR interaural ratio error {error}");
    assert!(error < 1e-9);
}
#[test]
fn room_tuning_reduces_room_excess_group_delay() {
    let mut h = synthetic();
    h.speakers[0].right = None;
    let p = design(&h, &options());
    let ir = h.speakers[0].left.as_ref().unwrap();
    let after = ImpulseResponse {
        fs: FS,
        data: conv::convolve(&ir.data, &p.report.speakers[0].filter, Mode::Full),
        recording: None,
    };
    let gd = |ir: &ImpulseResponse| {
        let (e, _) = room_tuning::excess_spectrum(ir, FS as usize);
        room_tuning::smooth_real(
            &e.windows(2)
                .map(|z| -(z[1] * z[0].conj()).arg() * 1000. / (2. * PI))
                .collect::<Vec<_>>(),
            1. / 6.,
        )
    };
    let a = gd(ir);
    let b = gd(&after);
    let bins: Vec<_> = (30..=300)
        .filter(|k| p.report.speakers[0].weights[*k] >= 0.9)
        .collect();
    assert!(!bins.is_empty());
    let rms =
        |v: &[f64]| (bins.iter().map(|k| v[*k].powi(2)).sum::<f64>() / bins.len() as f64).sqrt();
    println!(
        "omni GD RMS {} -> {} ms, bins {}",
        rms(&a),
        rms(&b),
        bins.len()
    );
    assert!(rms(&b) <= 0.5 * rms(&a));
}
#[test]
fn room_tuning_keeps_speaker_timing() {
    let mut h = synthetic();
    let mut s = h.speakers[0].clone();
    s.speaker = "FR".into();
    h.speakers.push(s);
    let p = design(&h, &options());
    assert!(p.report.speakers.iter().all(|s| s.design_origin == 480));
    let before = h.clone();
    let report = room_tuning::apply_tuning(&mut h, &p).unwrap();
    for s in &report.speakers {
        println!(
            "SYNTHETIC {} EDT {} -> {} ms; median excess GD {} -> {} ms; weak={} pre_echo={}",
            s.speaker,
            s.lf_edt_before_ms,
            s.lf_edt_after_ms,
            s.excess_median_before_ms,
            s.excess_median_after_ms,
            s.weak(),
            s.pre_echo_db
        );
    }
    for (a, b) in before.speakers.iter().zip(&h.speakers) {
        for (a, b) in a
            .left
            .iter()
            .chain(&a.right)
            .zip(b.left.iter().chain(&b.right))
        {
            assert_eq!(peak(&b.data), peak(&a.data) + 480);
        }
    }
}
#[test]
fn room_tuning_off_adds_no_delay() {
    let h = synthetic();
    let full = design(&h, &options());
    let off = design(
        &h,
        &TuningOptions {
            phase_limit: PhaseLimit::Off,
            ..options()
        },
    );
    assert_eq!(off.report.delay_samples, 0);
    let a = fft::rfft(&full.report.speakers[0].full_filter);
    let b = fft::rfft(&off.report.speakers[0].full_filter);
    let max = a[20..20001]
        .iter()
        .zip(&b[20..20001])
        .map(|(a, b)| (20. * (a.norm() / b.norm()).log10()).abs())
        .fold(0., f64::max);
    println!("off/full magnitude max {max} dB");
    // Full applies SECS post windows to the minimum-phase inverse; off does not.
    // The measured synthetic difference is 0.216 dB, not a phase-only discrepancy.
    assert!(max <= 0.35);
}
#[test]
fn room_tuning_pre_echo_is_buried() {
    let mut h = synthetic();
    let p = design(&h, &options());
    let report = room_tuning::apply_tuning(&mut h, &p).unwrap();
    println!("pre echo {} dB", report.speakers[0].pre_echo_db);
    assert!(report.speakers[0].pre_echo_db <= -30.);
    let ir = h.speakers[0].left.as_ref().unwrap();
    let p = peak(&ir.data);
    let low = filters::sosfilt(&filters::butter(4, 300. / 24000., BType::Lowpass), &ir.data);
    let energy = |a, b| low[a..b].iter().map(|v| v * v).sum::<f64>();
    let ratio = 10. * (energy(p - 480, p - 24) / energy(p, p + 960)).log10();
    println!("pre echo LF energy {ratio} dB");
    assert!(ratio <= -20.);
}
#[test]
fn room_tuning_preecho_uses_direct_and_baseline() {
    let mut data = vec![0.; 500];
    data[2] = 0.002; // More than 2 ms (96 samples) before the direct sound.
    data[60] = 0.1; // Inside the 2 ms guard: part of the onset, not counted.
    data[100] = 0.2;
    data[300] = 1.; // A later reflection must not become the direct sound.
    let ir = ImpulseResponse {
        fs: FS,
        data,
        recording: None,
    };
    assert!((room_tuning::pre_echo(&ir) + 40.).abs() < 1e-9);
    let diagnostic = |before_db, after_db| room_tuning::PreEcho {
        side: Side::Left,
        before_db,
        after_db,
    };
    assert!(!diagnostic(-50., -30.).warns());
    assert!(!diagnostic(-25., -20.).warns());
    assert!(diagnostic(-26., -20.).warns());
    let mut speaker = design(&synthetic(), &options()).report.speakers.remove(0);
    speaker.pre_echo_channels = vec![
        diagnostic(-50., -40.),
        room_tuning::PreEcho {
            side: Side::Right,
            before_db: -10.,
            after_db: -9.,
        },
    ];
    assert!(speaker.pre_echo_warning_db().is_none()); // Never compare across ears.
}
#[test]
fn room_tuning_weak_uses_representation() {
    let mut speaker = design(&synthetic(), &options()).report.speakers.remove(0);
    speaker.lf_edt_after_ms = 10000.;
    speaker.excess_median_after_ms = 10000.;
    assert!(!speaker.weak());
    speaker.representation_rms_db = vec![(Side::Left, 3.), (Side::Right, 5.)];
    assert!(!speaker.weak());
    speaker.representation_rms_db[1].1 = 5.01;
    assert!(speaker.weak());
}
#[test]
fn room_tuning_level_match_uses_median() {
    let levels = [1., 10_f64.powf(-3. / 20.), 10_f64.powf(-10. / 20.)];
    let trims = room_tuning::level_trims(&levels, &levels);
    println!("median trims {trims:?}");
    for (actual, expected) in trims.iter().zip([-3., 0., 6.]) {
        assert!((actual.0 - expected).abs() < 1e-9);
    }
}
#[test]
fn room_tuning_level_match_skips_inconsistent() {
    let out = room_tuning::level_trims(&[1., 0.5, 0.25], &[1., 0.5 * 10_f64.powf(3. / 20.), 0.25]);
    assert!(out.iter().any(|v| v.1));
}
#[test]
fn room_tuning_auto_delay_is_one_value() {
    let mut h = synthetic();
    let mut s = h.speakers[0].clone();
    s.speaker = "FR".into();
    h.speakers.push(s);
    let pairs = room_tuning::recording_pairs(&h, &["FL".into(), "FR".into()], None);
    let options = TuningOptions {
        delay: TuningDelay::Auto,
        pairs,
        ..options()
    };
    let a = design(&h, &options);
    let b = design(&h, &options);
    assert_eq!(a.report.auto_scores, b.report.auto_scores);
    assert_eq!(a.report.delay_samples, b.report.delay_samples);
    let min = a
        .report
        .auto_scores
        .iter()
        .map(|s| s.1)
        .fold(f64::INFINITY, f64::min);
    let best = a.report.auto_scores.iter().find(|s| s.1 == min).unwrap().0;
    assert_eq!(a.report.delay_samples, best as usize * 48);
}
#[test]
fn room_tuning_detects_swapped_files() {
    let ir = |p| {
        let mut data = vec![0.; 1000];
        data[p] = 1.;
        ImpulseResponse {
            fs: FS,
            data,
            recording: None,
        }
    };
    let mut pair = room_tuning::PairMeasurement {
        speakers: ["FL".into(), "FR".into()],
        side: Side::Left,
        irs: [ir(100), ir(112)],
    };
    assert!(!room_tuning::geometry_check(&pair).unwrap().mismatch);
    pair.side = Side::Right;
    assert!(room_tuning::geometry_check(&pair).unwrap().mismatch);
    pair.speakers = ["BL".into(), "SL".into()];
    assert!(room_tuning::geometry_check(&pair).is_none());
}
#[test]
fn room_tuning_two_points_use_power_average() {
    let ir = |gain| ImpulseResponse {
        fs: FS,
        data: vec![gain],
        recording: None,
    };
    let p = room_tuning::design_speaker(
        "FL",
        &[(Side::Left, ir(0.1)), (Side::Right, ir(1.))],
        &options(),
        10.,
        None,
        None,
    )
    .unwrap();
    assert!((20. * p.reference.log10() + 2.9670862188).abs() < 1e-8);
    println!(
        "two-point level {} dB; broad tone layer bounded separately",
        20. * p.reference.log10()
    );
}
#[test]
fn room_tuning_magnitude_follows_rolloff() {
    let mut mag = vec![0.; 24001];
    for (k, m) in mag.iter_mut().enumerate() {
        let f = k as f64;
        let hp = 1. / (1. + (45. / f.max(1e-12)).powi(8)).sqrt();
        let mode = 8. * (-0.5 * ((f - 70.) / 5.).powi(2)).exp();
        let dip = -15. * (-0.5 * ((f - 120.) / 5.).powi(2)).exp();
        *m = hp * 10_f64.powf((mode + dip) / 20.);
    }
    let ir = ImpulseResponse {
        fs: FS,
        data: fft::irfft(&room_tuning::minimum_phase(&mag, 48000), 48000),
        recording: None,
    };
    let p = room_tuning::design_speaker("FL", &[(Side::Left, ir)], &options(), 10., None, None)
        .unwrap();
    println!(
        "cutoff {} inv20 {} inv70 {} inv120 {}",
        p.low_cutoff, p.inverse[20], p.inverse[70], p.inverse[120]
    );
    assert!(p.inverse[20] <= 1.000001);
    assert!(p.inverse[70] < 1.);
    assert!(p.inverse[120] <= 10_f64.powf(6. / 20.));
    assert!(p.inverse[600..].iter().all(|v| *v == 1.));
}
#[test]
fn room_tuning_tone_layer_is_bounded() {
    let mag: Vec<_> = (0..24001)
        .map(|k| 10_f64.powf(-10. / 20. * ((k as f64 / 2000.).log2() + 0.5).clamp(0., 1.)))
        .collect();
    let ir = ImpulseResponse {
        fs: FS,
        data: fft::irfft(&room_tuning::minimum_phase(&mag, 48000), 48000),
        recording: None,
    };
    let p = room_tuning::design_speaker("FL", &[(Side::Left, ir)], &options(), 10., None, None)
        .unwrap();
    assert!(p.macro_gain.iter().all(|v| *v <= 2. + 1e-10));
    println!(
        "tone at 20k {} dB (smoothing straddles fade endpoint)",
        20. * p.macro_gain[20000].log10()
    );
}

#[test]
fn room_tuning_gates_disagreeing_points() {
    let n = FS as usize;
    let mut identity = vec![0.; n];
    identity[48] = 1.;
    let mut opposite = room_tuning::spectrum(&identity, n);
    for v in &mut opposite[40..=400] {
        *v = -*v;
    }
    let ir = |data| ImpulseResponse {
        fs: FS,
        data,
        recording: None,
    };
    let points = [
        (Side::Left, ir(identity)),
        (Side::Right, ir(fft::irfft(&opposite, n))),
    ];
    let out = room_tuning::design_speaker("FL", &points, &options(), 10., None, None).unwrap();
    assert!(out.weights[80..=200].iter().all(|g| *g == 0.));
    let spectrum = fft::rfft(&out.full_filter);
    let min = room_tuning::minimum_phase(&spectrum.iter().map(|v| v.norm()).collect::<Vec<_>>(), n);
    let excess: Vec<_> = spectrum
        .iter()
        .zip(min)
        .enumerate()
        .map(|(k, (v, m))| v / m * Complex64::from_polar(1., 2. * PI * k as f64 * 0.010))
        .collect();
    let max = (80..=200)
        .map(|k| ((excess[k + 1] * excess[k - 1].conj()).arg() * 1000. / (4. * PI)).abs())
        .fold(0., f64::max);
    println!("disagreeing points identity excess GD max {max} ms");
    assert!(max < 0.1);
}

#[test]
fn room_tuning_unmeasured_channels_receive_exact_delay() {
    let mut h = synthetic();
    let p = design(&h, &options());
    let mut extra = h.speakers[0].clone();
    extra.speaker = "FR".into();
    let before = extra.clone();
    h.speakers.push(extra);
    room_tuning::apply_tuning(&mut h, &p).unwrap();
    for (a, b) in before
        .left
        .iter()
        .chain(&before.right)
        .zip(h.speakers[1].left.iter().chain(&h.speakers[1].right))
    {
        assert_eq!(&b.data[480..], a.data);
        assert!(b.data[..480].iter().all(|v| *v == 0.));
        assert_eq!(peak(&b.data) - peak(&a.data), 480);
    }
}

fn synthetic() -> Hrir {
    let mut x = vec![0.0; 72000];
    x[960] = 1.0;
    let w = 2.0 * PI * 80.0 / FS as f64;
    let alpha = w.sin() / (2.0 * 0.7);
    let b = [
        (1.0 - alpha) / (1.0 + alpha),
        -2.0 * w.cos() / (1.0 + alpha),
        1.0,
    ];
    let a = [1.0, b[1], b[0]];
    let mut y = vec![0.0; x.len()];
    for i in 0..y.len() {
        y[i] = b[0] * x[i];
        if i >= 1 {
            y[i] += b[1] * x[i - 1] - a[1] * y[i - 1];
        }
        if i >= 2 {
            y[i] += b[2] * x[i - 2] - a[2] * y[i - 2];
        }
    }
    let original = y.clone();
    for i in 288..y.len() {
        y[i] += 10.0_f64.powf(-6.0 / 20.0) * original[i - 288];
    }
    let mut seed = 0x12345678_u64;
    for v in &mut y {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *v += ((seed >> 32) as f64 / u32::MAX as f64 * 2.0 - 1.0) * 10.0_f64.powf(-70.0 / 20.0);
    }
    let mut right = vec![0.0; y.len()];
    right[14..].copy_from_slice(&y[..y.len() - 14]);
    let ir = |data| ImpulseResponse {
        data,
        fs: FS,
        recording: None,
    };
    Hrir {
        fs: FS,
        speakers: vec![SpeakerIrs {
            speaker: "FL".into(),
            left: Some(ir(y)),
            right: Some(ir(right)),
        }],
    }
}
#[test]
fn room_tuning_rejects_low_sample_rates() {
    // The analysis reads fixed 1 Hz bins up to 500 Hz: refuse instead of panicking.
    let mut h = synthetic();
    h.fs = 4000;
    let o = options();
    assert!(room_tuning::prepare_tuning(&h, &o, None, None, &o.pairs).is_err());
}
#[test]
fn room_tuning_level_match_skips_silent_ears() {
    let trims = room_tuning::level_trims(&[1.0, 0.5, 2.0], &[1.0, 0.0, 2.0]);
    assert!(trims.iter().all(|(t, _, d)| t.is_finite() && d.is_finite()));
    assert_eq!(trims[1], (0.0, true, 0.0));
}
