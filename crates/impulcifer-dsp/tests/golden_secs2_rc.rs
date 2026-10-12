#![forbid(unsafe_code)]
//! Pre-fit measurement gates only; downstream RC gates await fit parity.
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
