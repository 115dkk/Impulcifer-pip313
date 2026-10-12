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
// provisional: set by the owner after review
const MAG_MAX_DB: [f64; 4] = [0.01; 4];
// provisional: set by the owner after review
const PHASE_MAX_RAD: [f64; 4] = [0.001; 4];
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
        let band = correction_band(
            &inputs.iter().map(|(_, p)| p.as_slice()).collect::<Vec<_>>(),
            &opts,
        );
        assert!(
            (band.high - e["rc_hi"].as_f64().unwrap()).abs() < 1e-6,
            "band {entry}: {}",
            band.high
        );
        if let Some(s) = e["schroeder_hz"].as_f64() {
            assert!(
                (band.schroeder.unwrap() - s).abs() < 1e-6,
                "Schroeder {entry}"
            );
        }
        let target = if e["target"] == "demo" {
            Some(&target)
        } else {
            None
        };
        let levels: Vec<_> = inputs
            .iter()
            .map(|(_, p)| channel_level(p, &opts.policy, target, None))
            .collect();
        let min = levels.iter().copied().fold(f64::INFINITY, f64::min);
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
            let mut fit_diff = Vec::new();
            if let Some(fits) = e["peq_fits"].as_array() {
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
            println!(
                "RC {entry}/{speaker} mag={md:.6?} phase={pd:.6?} design={dd:.9} gain={gain:.9} count={count} strength={}/{} beta={}/{} taps={}/{} time={:.3}s SECS={}s {}",
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
            let bad = (0..4).any(|i| md[i] > MAG_MAX_DB[i] || pd[i] > PHASE_MAX_RAD[i])
                || gain > 1e-8
                || !count
                || out.phase_strength != r["phase_strength"].as_f64().unwrap()
                || out.phase_frac != r["phase_frac_used"].as_f64().unwrap()
                || out.fir.len() != r["fir_taps"].as_u64().unwrap() as usize
                || dd > 0.000051;
            for i in 0..4 {
                maxima_mag[i] = maxima_mag[i].max(md[i]);
                maxima_phase[i] = maxima_phase[i].max(pd[i]);
            }
            if bad {
                failures.push(format!("{entry}/{speaker}"));
            }
        }
    }
    println!("RC MAX magnitude={maxima_mag:?} phase={maxima_phase:?}");
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
    let p = [impulse(48000)];
    let a = RcOptions::default();
    let b = RcOptions {
        policy: PositionPolicy::Listening {
            index: 0,
            weight: 5.,
        },
        ..a.clone()
    };
    let band = correction_band(&[&p], &a);
    let a = design_rc(&p, &a, &band, None, None).unwrap();
    let b = design_rc(&p, &b, &band, None, None).unwrap();
    for (a, b) in a.response.iter().zip(b.response) {
        assert!((*a - b).norm() < 1e-12);
    }
}
#[test]
fn room_tuning2_symmetric_is_order_free() {
    let mut p = vec![impulse(48000), impulse(48000)];
    p[1].data[100] = 0.;
    p[1].data[200] = 1.;
    p[1].data[600] = 0.2;
    let opts = RcOptions::default();
    let band = correction_band(&[&p], &opts);
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
    assert_eq!(correction_band(&[&p], &o).high, 500.);
    o.schroeder = Some(60.);
    assert_eq!(correction_band(&[&p], &o).high, 100.);
    o.schroeder = Some(250.);
    assert_eq!(correction_band(&[&p], &o).source, BandSource::Override);
    assert_eq!(correction_band(&[&p], &o).high, 250.);
    o.high = Some(300.);
    assert_eq!(correction_band(&[&p], &o).source, BandSource::Manual);
    o.high = None;
    o.schroeder = None;
    o.room_volume = Some(40.);
    assert_eq!(correction_band(&[&p], &o).source, BandSource::Volume);
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
