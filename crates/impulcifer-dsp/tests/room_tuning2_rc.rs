#![forbid(unsafe_code)]
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft::Complex64,
    fr::FrequencyResponse,
    hrir::Hrir,
    ir::ImpulseResponse,
    stages::{room, room_tuning2::*},
};
use impulcifer_io::read_wav;
use serde_json::Value;
use std::{collections::BTreeMap, f64::consts::PI, path::PathBuf};
const THRESHOLDS_DB: [f64; 4] = [0.001, 0.01, 0.1, 1.];

#[test]
fn linear_sessions_are_bit_identical_across_thread_counts() {
    let measurements = demo();
    let points: Vec<_> = ["left", "right"]
        .iter()
        .map(|side| measurements[&("FL".into(), (*side).into())].clone())
        .collect();
    let opts = RcOptions {
        mode: Tuning2Mode::Linear,
        ..Default::default()
    };
    let band = correction_band(&[&points], &opts).unwrap();
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| design_rc(&points, &opts, &band, None, None).unwrap())
    };
    let one = run(1);
    let four = run(4);
    assert_eq!(
        one.fir.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        four.fir.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(
        one.response
            .iter()
            .map(|x| (x.re.to_bits(), x.im.to_bits()))
            .collect::<Vec<_>>(),
        four.response
            .iter()
            .map(|x| (x.re.to_bits(), x.im.to_bits()))
            .collect::<Vec<_>>()
    );
    assert_eq!(one.peq, four.peq);
    assert_eq!(one.peq_fits, four.peq_fits);
    assert_eq!(one.phase_strength.to_bits(), four.phase_strength.to_bits());
    assert_eq!(one.phase_frac.to_bits(), four.phase_frac.to_bits());
}

fn rounding_allowance(reference: f64) -> f64 {
    // Half a decimal unit at ten significant digits, plus floating-point noise.
    let half_unit = if reference == 0. {
        0.
    } else {
        0.5 * 10_f64.powf(reference.abs().log10().floor() - 9.)
    };
    half_unit + 1e-10
}

fn fit_references(root: &std::path::Path, entry: &Value, speaker: &str) -> Vec<Value> {
    let dir = root.join("tests/migration/goldens");
    // Intermediate fits live in per-channel s2 fixtures, not in the RC entry.
    // These cases use the default boost; do not match the boost6 integration case.
    if entry["max_boost_db"] != 10. {
        return Vec::new();
    }
    for file in std::fs::read_dir(&dir).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if !name.starts_with("s2_rc_") || !name.ends_with(".json") {
            continue;
        }
        let fixture: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let input = &fixture["inputs"];
        if input["speaker"] != speaker
            || input["sides"] != entry["sides"]
            || input["mode"] != entry["mode"]
            || input["main_weight"] != entry["main_weight"]
            || input["target"] != entry["target"]
            || input["rc_range"][0] != entry["rc_lo"]
            || input["rc_range"][1] != entry["rc_hi_setting"]
            || input["room_volume_m3"] != entry["room_volume_m3"]
        {
            continue;
        }
        let mut fits = Vec::new();
        for session in fixture["outputs"]["sessions"].as_array().unwrap() {
            let session = session.as_str().unwrap();
            let stage: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.join(format!("s2_{session}.json"))).unwrap(),
            )
            .unwrap();
            for i in 0..stage["outputs"]["fits"].as_u64().unwrap() {
                let fit: Value = serde_json::from_str(
                    &std::fs::read_to_string(dir.join(format!(
                        "s2_fit_{}_{i}.json",
                        session.strip_prefix("rc_").unwrap()
                    )))
                    .unwrap(),
                )
                .unwrap();
                fits.push(fit["outputs"]["bands"].clone());
            }
        }
        return fits;
    }
    Vec::new()
}
fn values(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
fn demo() -> BTreeMap<(String, String), ImpulseResponse> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let w = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let est = SweepEstimator::from_samples(w.sample_rate, &w.tracks[0]).unwrap();
    let mut out = BTreeMap::new();
    for file in std::fs::read_dir(root.join("data/demo")).unwrap() {
        let path = file.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        let Some(names) = room::parse_room_measurement_name(name) else {
            continue;
        };
        if names.side.is_none() {
            continue;
        }
        let w = read_wav(&path).unwrap();
        let mut h = Hrir {
            fs: w.sample_rate,
            speakers: vec![],
        };
        h.open_recording_samples(
            &est,
            w.sample_rate,
            &w.tracks,
            &names
                .speakers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            names.side,
            2.,
        )
        .unwrap();
        for name in &names.speakers {
            let s = h.get(name).unwrap();
            if let Some(ir) = &s.left {
                out.insert((name.clone(), "left".into()), ir.clone());
            }
            if let Some(ir) = &s.right {
                out.insert((name.clone(), "right".into()), ir.clone());
            }
        }
    }
    out
}
#[test]
fn room_tuning2_matches_secs2_rc() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs2_rc.json"
    ))
    .unwrap();
    let variance: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs2_variance.json"
    ))
    .unwrap();
    let envelope = &variance["summary"]["envelope"];
    let caps: Vec<_> = values(&envelope["max_mag_db"])
        .iter()
        .map(|v| (v * 1.25 * 100.).ceil() / 100.)
        .collect();
    let counts: Vec<_> = ["0.001", "0.01", "0.1", "1.0"]
        .iter()
        .map(|k| envelope["channels_over"][k].as_u64().unwrap() as usize)
        .collect();
    let mut strength_changes = 0;
    let mut count_changes = 0;
    let demo = demo();
    let bins: Vec<_> = values(&golden["bins"])
        .iter()
        .map(|v| *v as usize)
        .collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = FrequencyResponse::read_from_csv(&root.join("data/demo/room-target.csv")).unwrap();
    let mut failures = Vec::new();
    let mut maxima_mag = [0_f64; 4];
    let mut maxima_phase = [0_f64; 4];
    let mut channel_maxima = Vec::new();
    let mut above = [0_usize; 4];
    let mut level_only_count = 0;
    let filter = std::env::var("RC_ENTRY").unwrap_or_default();
    for (entry, e) in golden["entries"].as_object().unwrap() {
        if !filter.is_empty() && !entry.contains(&filter) {
            continue;
        }
        let opts = RcOptions {
            mode: if e["mode"] == "ultra-60" {
                Tuning2Mode::Ultra(60.)
            } else {
                Tuning2Mode::parse(e["mode"].as_str().unwrap()).unwrap()
            },
            max_boost: e["max_boost_db"].as_f64().unwrap(),
            low: e["rc_lo"].as_f64().unwrap(),
            high: e["rc_hi_setting"].as_f64(),
            room_volume: e["room_volume_m3"].as_f64(),
            policy: PositionPolicy::Listening {
                index: 0,
                weight: e["main_weight"].as_f64().unwrap(),
            },
            ..Default::default()
        };
        let mut inputs = Vec::new();
        for (speaker, _) in e["channels"].as_object().unwrap() {
            inputs.push((
                speaker,
                e["sides"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|side| demo[&(speaker.clone(), side.as_str().unwrap().to_owned())].clone())
                    .collect::<Vec<_>>(),
            ));
        }
        let target = if e["target"] == "demo" {
            Some(&target)
        } else {
            None
        };
        let levels: Vec<_> = inputs
            .iter()
            .map(|(_, p)| channel_level(p, &opts.policy, target, None).unwrap())
            .collect();
        let min = levels.iter().copied().fold(f64::INFINITY, f64::min);
        if entry == "all/mixed" {
            for ((speaker, _), level) in inputs.iter().zip(&levels) {
                let r = &e["channels"][*speaker];
                let reference_level = r["level_db"].as_f64().unwrap();
                let gain = (min - level - r["gain_db"].as_f64().unwrap()).abs();
                let delta = (level - reference_level).abs();
                println!("RC {entry}/{speaker} levels-only gain={gain:.9e} level={delta:.9e}");
                if !gain.is_finite()
                    || gain > 1e-8
                    || !delta.is_finite()
                    || delta > rounding_allowance(reference_level)
                {
                    failures.push(format!("{entry}/{speaker}: gain/level"));
                }
                level_only_count += 1;
            }
            continue;
        }
        let band = correction_band(
            &inputs.iter().map(|(_, p)| p.as_slice()).collect::<Vec<_>>(),
            &opts,
        )
        .unwrap();
        if (band.high - e["rc_hi"].as_f64().unwrap()).abs() >= 1e-6 {
            failures.push(format!("band {entry}: {}", band.high));
        }
        if let Some(s) = e["schroeder_hz"].as_f64()
            && !band.schroeder.is_some_and(|ours| (ours - s).abs() < 1e-6)
        {
            failures.push(format!("Schroeder {entry}"));
        }
        for (speaker, points) in inputs {
            let start = std::time::Instant::now();
            let out = design_rc(&points, &opts, &band, target, None).unwrap();
            let r = &e["channels"][speaker];
            let mag = values(&r["mag_db"]);
            let phase = values(&r["phase_rad"]);
            let mut md = [0_f64; 4];
            let mut pd = [0_f64; 4];
            let design = if r["design_db"].is_array() {
                Some(values(&r["design_db"]))
            } else {
                None
            };
            let mut dd = 0_f64;
            for (i, k) in bins.iter().copied().enumerate() {
                let f = k as f64 * 48000. / 131072.;
                let ours = 20. * (out.response[k].norm() + 1e-12).log10();
                let angle = (out.response[k]
                    * Complex64::from_polar(1., 2. * PI * f * out.lift_samples as f64 / 48000.))
                .arg();
                for (b, (lo, hi)) in [(15., 100.), (100., 500.), (500., 2000.), (2000., 23000.)]
                    .into_iter()
                    .enumerate()
                {
                    if f >= lo && f < hi {
                        md[b] = md[b].max((ours - mag[i]).abs());
                        pd[b] =
                            pd[b].max((angle - phase[i] + PI).rem_euclid(2. * PI).sub(PI).abs());
                    }
                }
                if let Some(d) = &design {
                    dd = dd.max((out.design_db[k] - d[i]).abs());
                }
            }
            let gain = (min - out.level_db - r["gain_db"].as_f64().unwrap()).abs();
            let reference_level = r["level_db"].as_f64().unwrap();
            let level = (out.level_db - reference_level).abs();
            let mut fit_diff = Vec::new();
            let fits = fit_references(&root, e, speaker);
            if !fits.is_empty() {
                fit_diff.push(format!("fits={}/{}", out.peq_fits.len(), fits.len()));
                for (i, (a, b)) in out.peq_fits.iter().zip(fits).enumerate() {
                    let mut delta = [0_f64; 3];
                    if a.len() != b.as_array().unwrap().len() {
                        fit_diff.push(format!(
                            "fit{i}: count {} != {}",
                            a.len(),
                            b.as_array().unwrap().len()
                        ));
                        continue;
                    }
                    for (a, b) in a.iter().zip(b.as_array().unwrap()) {
                        for (j, x) in [a.0, a.1, a.2].iter().enumerate() {
                            delta[j] = delta[j].max((x - b[j].as_f64().unwrap()).abs());
                        }
                    }
                    fit_diff.push(format!("fit{i}: {delta:.6?}"));
                }
            }
            let count = out.peq.len() == r["peq"].as_array().unwrap().len();
            let strength = out.phase_strength == r["phase_strength"].as_f64().unwrap();
            let resolution =
                e["mode"] == "zero" || out.phase_frac == r["phase_frac_used"].as_f64().unwrap();
            let taps = out.fir.len() == r["fir_taps"].as_u64().unwrap() as usize;
            println!(
                "RC {entry}/{speaker} mag={md:.6?} phase={pd:.6?} design={dd:.9} gain={gain:.9e} level={level:.9e} count={count} strength={}/{} beta={}/{} (match={resolution}; ignored for zero) taps={}/{} time={:.3}s SECS={}s {}",
                out.phase_strength,
                r["phase_strength"],
                out.phase_frac,
                r["phase_frac_used"],
                out.fir.len(),
                r["fir_taps"],
                start.elapsed().as_secs_f64(),
                e["secs_seconds"],
                fit_diff.join("; ")
            );
            strength_changes += usize::from(!strength);
            count_changes += usize::from(!count);
            let bad = (0..4).any(|i| md[i] > caps[i])
                || !gain.is_finite()
                || gain > 1e-8
                || !level.is_finite()
                || level > rounding_allowance(reference_level)
                || !resolution
                || !taps
                || out
                    .response
                    .iter()
                    .any(|v| !v.re.is_finite() || !v.im.is_finite())
                || dd > 0.000051;
            let maximum = md.into_iter().fold(0_f64, f64::max);
            for (count, threshold) in above.iter_mut().zip(THRESHOLDS_DB) {
                *count += usize::from(maximum > threshold);
            }
            channel_maxima.push(md);
            for i in 0..4 {
                maxima_mag[i] = maxima_mag[i].max(md[i]);
                maxima_phase[i] = maxima_phase[i].max(pd[i]);
            }
            if bad {
                failures.push(format!("{entry}/{speaker}"));
            }
        }
    }
    println!(
        "RC coverage: {} response references, {level_only_count} levels-only references; filter={filter:?}",
        channel_maxima.len()
    );
    println!(
        "RC band Hz | magnitude max / SECS cap dB | median / SECS dB | phase max / SECS rad (report only)"
    );
    for (i, band) in ["15-100", "100-500", "500-2000", "2000-23000"]
        .iter()
        .enumerate()
    {
        let mut sorted: Vec<_> = channel_maxima.iter().map(|v| v[i]).collect();
        sorted.sort_by(f64::total_cmp);
        let median = if sorted.is_empty() {
            f64::NAN
        } else {
            (sorted[(sorted.len() - 1) / 2] + sorted[sorted.len() / 2]) / 2.
        };
        println!(
            "RC {band} | {:.9} / {} | {median:.9} / {} | phase {:.9}",
            maxima_mag[i], caps[i], envelope["median_mag_db"][i], maxima_phase[i]
        );
    }
    println!(
        "MEASURE RC distribution: thresholds={THRESHOLDS_DB:?} ours={above:?} envelope={counts:?} max_mag_db={maxima_mag:?} strength_changes={strength_changes} peq_count_changes={count_changes}"
    );
    for (name, summary) in variance["summary"].as_object().unwrap() {
        println!("RC VARIANT {name}: {summary}");
    }
    if above[2] > counts[2] || above[3] > counts[3] {
        failures.push(format!("distribution: {above:?}"));
    }
    if filter.is_empty() && (channel_maxima.len() != 61 || level_only_count != 7) {
        failures.push("expected 61 response and 7 levels-only references".into());
    }
    println!("RC integration is not an equivalence proof until the solver trace fixtures pass.");
    assert!(failures.is_empty(), "RC divergences: {failures:?}");
}
use std::ops::Sub;
fn impulse(fs: u32) -> ImpulseResponse {
    let mut data = vec![0.; fs as usize];
    data[100] = 1.;
    ImpulseResponse {
        fs,
        data,
        recording: None,
    }
}
#[test]
fn room_tuning2_symmetric_one_point_is_secs() {
    let demo = demo();
    let p = [demo[&("FL".into(), "left".into())].clone()];
    let a = RcOptions::default();
    let b = RcOptions {
        policy: PositionPolicy::Listening {
            index: 0,
            weight: 5.,
        },
        ..a.clone()
    };
    let band = correction_band(&[&p], &a).unwrap();
    let a = design_rc(&p, &a, &band, None, None).unwrap();
    let b = design_rc(&p, &b, &band, None, None).unwrap();
    for (a, b) in a.response.iter().zip(b.response) {
        assert!((*a - b).norm() < 1e-12);
    }
}
#[test]
fn room_tuning2_symmetric_is_order_free() {
    let demo = demo();
    let mut p = vec![
        demo[&("FL".into(), "left".into())].clone(),
        demo[&("FL".into(), "right".into())].clone(),
    ];
    let opts = RcOptions::default();
    let band = correction_band(&[&p], &opts).unwrap();
    let a = design_rc(&p, &opts, &band, None, None).unwrap();
    p.swap(0, 1);
    let b = design_rc(&p, &opts, &band, None, None).unwrap();
    for (a, b) in a.response.iter().zip(b.response) {
        assert!((20. * (a.norm() / b.norm()).log10()).abs() < 1e-9);
        assert!((*a / b).arg().abs() < 1e-9);
    }
}
#[test]
fn room_tuning2_modes_have_their_latency() {
    for (s, taps, peq, lift) in [
        ("zero", 0, 32, 0.),
        ("mixed", 2048, 16, 8.),
        ("linear", 2048, 16, 28.),
        ("long", 32768, 16, 200.),
        ("60", 4096, 16, 58.),
        ("200", 16384, 16, 198.),
    ] {
        let mode = Tuning2Mode::parse(s).unwrap();
        for fs in [48000, 96000, 44100] {
            let p = mode.params(fs);
            assert_eq!(
                p.taps,
                (taps as f64 * fs as f64 / 48000.).round_ties_even() as usize
            );
            assert_eq!(p.peq_bands, peq);
            assert!((p.lift_ms - lift).abs() < 1e-12);
            assert_eq!(analysis_len(fs) % 2, 0);
        }
    }
    for s in ["29", "201", "NaN", "inf", "ultra", "Mixed", ""] {
        assert!(Tuning2Mode::parse(s).is_none());
    }
    assert!(Tuning2Mode::parse("30").is_some());
}
#[test]
fn room_tuning2_band_rules() {
    let p = [impulse(48000)];
    let mut o = RcOptions::default();
    assert_eq!(correction_band(&[&p], &o).unwrap().high, 500.);
    o.schroeder = Some(60.);
    assert_eq!(correction_band(&[&p], &o).unwrap().high, 100.);
    o.schroeder = Some(250.);
    assert_eq!(
        correction_band(&[&p], &o).unwrap().source,
        BandSource::Override
    );
    assert_eq!(correction_band(&[&p], &o).unwrap().high, 250.);
    o.high = Some(300.);
    assert_eq!(
        correction_band(&[&p], &o).unwrap().source,
        BandSource::Manual
    );
    o.high = None;
    o.schroeder = None;
    o.room_volume = Some(40.);
    assert_eq!(
        correction_band(&[&p], &o).unwrap().source,
        BandSource::Volume
    );
}
#[test]
fn room_tuning2_rejects_invalid_inputs() {
    let opts = RcOptions::default();
    let band = CorrectionBand {
        low: 20.,
        high: 500.,
        source: BandSource::Default,
        schroeder: None,
    };
    assert!(design_rc(&[], &opts, &band, None, None).is_err());
    assert!(design_rc(&[impulse(32000)], &opts, &band, None, None).is_err());
    let mut p = impulse(48000);
    p.data[0] = f64::NAN;
    assert!(design_rc(&[p], &opts, &band, None, None).is_err());
}
