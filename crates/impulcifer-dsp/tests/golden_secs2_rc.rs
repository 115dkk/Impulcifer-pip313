#![forbid(unsafe_code)]
//! Production-chain gates before fitting, with post-horizon diagnostics.
mod golden_secs2_helpers;
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fr::FrequencyResponse,
    hrir::Hrir,
    ir::ImpulseResponse,
    stages::{room, room_tuning2::*},
};
use impulcifer_io::read_wav;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};
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

fn fixture(name: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens");
    serde_json::from_slice(&std::fs::read(root.join(format!("s2_{name}.json"))).unwrap()).unwrap()
}
fn measure(name: &str, pairs: &[(f64, f64)], budget: f64) {
    let max = pairs.iter().map(|(a, b)| (a - b).abs()).fold(0., f64::max);
    let relative = (pairs.iter().map(|(a, b)| (a - b).powi(2)).sum::<f64>()
        / pairs.iter().map(|(_, b)| b * b).sum::<f64>().max(1e-30))
    .sqrt();
    println!(
        "MEASURE {name}: max_abs={max:.16e}, max_relative_l2={relative:.16e}, values={}, budget={budget:.1e}",
        pairs.len()
    );
    assert!(pairs.iter().all(|(a, b)| a.is_finite() && b.is_finite()));
    assert!(max <= budget, "{name}: {max:e} > {budget:e}");
}
#[test]
fn secs2_measurement_levels_and_bands() {
    let demo = demo();
    for name in ["levels_w1", "levels_w5"] {
        let v = fixture(name);
        let i = &v["inputs"];
        let policy = PositionPolicy::Listening {
            index: 0,
            weight: i["main_weight"].as_f64().unwrap(),
        };
        let mut pairs = Vec::new();
        for speaker in i["speakers"].as_array().unwrap() {
            let speaker = speaker.as_str().unwrap();
            let points: Vec<_> = i["sides"]
                .as_array()
                .unwrap()
                .iter()
                .map(|side| demo[&(speaker.into(), side.as_str().unwrap().into())].clone())
                .collect();
            pairs.push((
                channel_level(&points, &policy, None, None).unwrap(),
                v["outputs"]["level_db"][speaker].as_f64().unwrap(),
            ));
        }
        measure(name, &pairs, 1e-12);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = FrequencyResponse::read_from_csv(&root.join("data/demo/room-target.csv")).unwrap();
    let mut bands = Vec::new();
    let mut levels = Vec::new();
    for name in [
        "bl_both_zero",
        "br_both_mixed_band30_300",
        "fc_both_long",
        "fl_both_mixed",
        "fl_both_mixed_main5",
        "fl_both_mixed_target_volume40",
        "fl_left_mixed",
        "sl_both_linear",
        "sr_both_ultra60",
    ] {
        let v = fixture(&format!("rc_{name}"));
        let i = &v["inputs"];
        let points: Vec<_> = i["sides"]
            .as_array()
            .unwrap()
            .iter()
            .map(|side| {
                demo[&(
                    i["speaker"].as_str().unwrap().into(),
                    side.as_str().unwrap().into(),
                )]
                    .clone()
            })
            .collect();
        let opts = RcOptions {
            policy: PositionPolicy::Listening {
                index: 0,
                weight: i["main_weight"].as_f64().unwrap(),
            },
            low: i["rc_range"][0].as_f64().unwrap(),
            high: i["rc_range"][1].as_f64(),
            room_volume: i["room_volume_m3"].as_f64(),
            ..Default::default()
        };
        let band = correction_band(&[&points], &opts).unwrap();
        assert_eq!(band.low, opts.low);
        bands.push((band.high, v["outputs"]["rc_hi"].as_f64().unwrap()));
        match (band.schroeder, v["outputs"]["schroeder_hz"].as_f64()) {
            (Some(a), Some(b)) => bands.push((a, b)),
            (None, None) => (),
            _ => panic!("{name}: Schroeder presence"),
        }
        let target = if i["target"] == "demo" {
            Some(&target)
        } else {
            None
        };
        levels.push((
            channel_level(&points, &opts.policy, target, None).unwrap(),
            v["outputs"]["level_db"].as_f64().unwrap(),
        ));
    }
    measure("rc_bands", &bands, 1e-12);
    measure("rc_levels", &levels, 1e-12);
}

#[test]
fn secs2_rc_stage_chain() {
    use golden_secs2_helpers::{Errors, array, complex, fixtures};
    let demo = demo();
    let bins: Vec<_> = array(&fixture("bins")["outputs"]["bins"])
        .iter()
        .map(|v| *v as usize)
        .collect();
    let target = FrequencyResponse::read_from_csv(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/demo/room-target.csv"),
    )
    .unwrap();
    for (name, v) in fixtures("rc_") {
        if v["outputs"]["sessions"].is_null() {
            continue;
        }
        let i = &v["inputs"];
        let points: Vec<_> = i["sides"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                demo[&(
                    i["speaker"].as_str().unwrap().into(),
                    s.as_str().unwrap().into(),
                )]
                    .clone()
            })
            .collect();
        let mode = i["mode"].as_str().unwrap();
        let opts = RcOptions {
            mode: Tuning2Mode::parse(mode.strip_prefix("ultra-").unwrap_or(mode)).unwrap(),
            low: i["rc_range"][0].as_f64().unwrap(),
            high: i["rc_range"][1].as_f64(),
            room_volume: i["room_volume_m3"].as_f64(),
            policy: PositionPolicy::Listening {
                index: 0,
                weight: i["main_weight"].as_f64().unwrap(),
            },
            ..Default::default()
        };
        let band = correction_band(&[&points], &opts).unwrap();
        let mut stages = RcStages::default();
        let result = design_rc_with_stages(
            &points,
            &opts,
            &band,
            if i["target"] == "demo" {
                Some(&target)
            } else {
                None
            },
            None,
            Some(&mut stages),
        )
        .unwrap();
        let sessions = v["outputs"]["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), stages.sessions.len());
        for (session, key) in stages.sessions.iter().zip(sessions) {
            let key = key.as_str().unwrap();
            let expected = fixture(key);
            let o = &expected["outputs"];
            let fit = fixture(&format!("fit_{}_0", key.strip_prefix("rc_").unwrap()));
            let horizon = &fit["outputs"]["horizon"];
            let downstream_report =
                !fit["outputs"]["oracle_gesvd"]["first_index_over_1e-9"].is_null();
            let mut first: Option<(String, f64)> = None;
            let mut check = |field: &str, got: Vec<f64>, want: Vec<f64>, pre: bool| {
                let size = got
                    .iter()
                    .zip(&want)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0_f64, f64::max);
                let mismatch = got.len() != want.len()
                    || got
                        .iter()
                        .zip(&want)
                        .any(|(a, b)| !a.is_finite() || (a - b).abs() > 1e-12 + 1e-12 * b.abs());
                if mismatch && first.is_none() {
                    first = Some((field.into(), size));
                }
                println!(
                    "MEASURE {key}/{field}: max_abs={size:e}, values={}/{}, gate={}",
                    got.len(),
                    want.len(),
                    pre || !downstream_report
                );
                if pre || !downstream_report {
                    let mut errors = Errors::default();
                    errors.real(field, &got, &want);
                    errors.finish(&format!("{key}/{field}"));
                }
            };
            for (field, got) in [
                ("power", &stages.power),
                ("smoothed_variable", &stages.smoothed_variable),
                ("smoothed_third", &stages.smoothed_third),
                ("design_db", &stages.design_db),
            ] {
                check(
                    field,
                    bins.iter().map(|k| got[*k]).collect(),
                    array(&o[field]),
                    true,
                );
            }
            check(
                "long_mode_bins",
                stages.long_mode_bins.iter().map(|k| *k as f64).collect(),
                array(&o["long_mode_bins"]),
                true,
            );
            check(
                "fit_0/corr_db",
                session.fit_inputs[0].clone(),
                array(&fit["inputs"]["corr_db"]),
                true,
            );
            for (field, got) in [
                ("M", &session.m),
                ("V", &session.v),
                ("V_smoothed", &session.v_smoothed),
            ] {
                if !o[field].is_null() {
                    check(
                        field,
                        bins.iter().flat_map(|k| [got[*k].re, got[*k].im]).collect(),
                        complex(&o[field])
                            .iter()
                            .flat_map(|z| [z.re, z.im])
                            .collect(),
                        false,
                    );
                }
            }
            for (field, got) in [
                ("tau", &session.tau),
                ("tau_smoothed", &session.tau_smoothed),
            ] {
                if !o[field].is_null() {
                    check(
                        field,
                        bins.iter().map(|k| got[*k]).collect(),
                        array(&o[field]),
                        false,
                    );
                }
            }
            for (field, got) in [("firs", &session.firs)] {
                let want = o[field].as_array().unwrap();
                check(
                    &format!("{field}/count"),
                    vec![got.len() as f64],
                    vec![want.len() as f64],
                    false,
                );
                for (k, (a, b)) in got.iter().zip(want).enumerate() {
                    check(&format!("{field}/{k}"), a.clone(), array(b), false);
                }
            }
            if let Some(want) = o["touchup_M"].as_array() {
                check(
                    "touchup_M/count",
                    vec![session.touchup_m.len() as f64],
                    vec![want.len() as f64],
                    false,
                );
                for (k, (a, b)) in session.touchup_m.iter().zip(want).enumerate() {
                    check(
                        &format!("touchup_M/{k}"),
                        bins.iter().flat_map(|k| [a[*k].re, a[*k].im]).collect(),
                        complex(b).iter().flat_map(|z| [z.re, z.im]).collect(),
                        false,
                    );
                }
            }
            let want_fir = array(&o["fir"]);
            check(
                "fir",
                if opts.mode == Tuning2Mode::Zero {
                    vec![1.]
                } else {
                    session.fir.clone()
                },
                want_fir,
                false,
            );
            check(
                "response",
                bins.iter()
                    .flat_map(|k| [session.response[*k].re, session.response[*k].im])
                    .collect(),
                complex(&o["response"])
                    .iter()
                    .flat_map(|z| [z.re, z.im])
                    .collect(),
                false,
            );
            check(
                "peq",
                session
                    .peq
                    .iter()
                    .flat_map(|&(f, q, g)| [f, q, g])
                    .collect(),
                golden_secs2_helpers::bands(&o["peq"])
                    .iter()
                    .flat_map(|&(f, q, g)| [f, q, g])
                    .collect(),
                false,
            );
            check(
                "strength",
                vec![session.strength],
                vec![o["strength"].as_f64().unwrap()],
                false,
            );
            println!("STAGE {key}: first={first:?} horizon={horizon}");
        }
        let expected = complex(&v["outputs"]["response"]);
        let max = bins
            .iter()
            .zip(expected)
            .map(|(k, b)| (result.response[*k] - b).norm())
            .fold(0_f64, f64::max);
        println!(
            "MEASURE {name}/result: max_abs={max:e}, strength={}/{}, resolution={}/{}, bands={}/{} report_only=true",
            result.phase_strength,
            v["outputs"]["strength"],
            result.phase_frac,
            v["outputs"]["phase_frac_used"],
            result.peq.len(),
            v["outputs"]["peq"].as_array().unwrap().len()
        );
    }
}
