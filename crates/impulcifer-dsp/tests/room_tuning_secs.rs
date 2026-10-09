#![forbid(unsafe_code)]
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft,
    hrir::Hrir,
    stages::{
        room,
        room_tuning::{self, TuningOptions},
    },
};
use impulcifer_io::read_wav;
use serde_json::Value;
use std::{f64::consts::PI, path::PathBuf};
// provisional: set by the owner after review
const MAG_MAX_DB: [f64; 5] = [0.01; 5];
// provisional: set by the owner after review
const GD_MAX_MS: [f64; 5] = [0.01; 5];
const AUTO_SCORE_MAX: f64 = 0.002;
#[test]
fn room_tuning_auto_delay_matches_secs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs_auto_delay.json"
    ))
    .unwrap();
    let wav = read_wav(&root.join(golden["ir"]["estimator"].as_str().unwrap())).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut sums = [0.; 9];
    let mut choices = Vec::new();
    for (file, expected) in golden["chosen_delay_ms"].as_object().unwrap() {
        let names = room::parse_room_measurement_name(file).unwrap();
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let mut h = Hrir {
            fs: 48000,
            speakers: vec![],
        };
        h.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &names
                .speakers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            names.side,
            2.,
        )
        .unwrap();
        let pairs = room_tuning::recording_pairs(&h, &names.speakers, names.side);
        assert_eq!(pairs.len(), 1);
        let mut best = (0, f64::INFINITY);
        let mut curve = Vec::new();
        for d in 2..=10 {
            let score = room_tuning::auto_delay_metric(
                &pairs[0],
                &TuningOptions::default(),
                d as f64,
                None,
                None,
            )
            .unwrap();
            let secs = golden["metric_by_candidate_ms"][file][d.to_string()]
                .as_f64()
                .unwrap();
            println!(
                "SCORE {file} {d}: ours={score:.9} SECS={secs:.9} delta={:.9}",
                score - secs
            );
            assert!((score - secs).abs() <= AUTO_SCORE_MAX, "{file} D={d}");
            sums[d - 2] += score;
            curve.push(score);
            if score < best.1 {
                best = (d, score);
            }
        }
        println!(
            "AUTO {file}: {curve:?}; chosen={} golden={expected}",
            best.0
        );
        choices.push((file, best.0, expected.as_u64().unwrap() as usize));
    }
    println!(
        "AUTO run sums {sums:?}; chosen={}",
        sums.iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0
            + 2
    );
    for (file, actual, expected) in choices {
        assert_eq!(actual, expected, "{file}");
    }
}
#[test]
fn room_tuning_matches_secs_stages() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs_stages.json"
    ))
    .unwrap();
    let wav = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let bins: Vec<usize> = values(&golden["bins"])
        .iter()
        .map(|v| *v as usize)
        .collect();
    let mut bad = false;
    for (entry, reference) in golden["entries"].as_object().unwrap() {
        let (file, speaker) = entry.split_once('/').unwrap();
        let names = room::parse_room_measurement_name(file).unwrap();
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let mut h = Hrir {
            fs: 48000,
            speakers: vec![],
        };
        h.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &names
                .speakers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            names.side,
            2.,
        )
        .unwrap();
        h.for_each_ir(|ir| ir.crop_head(1.));
        let s = h.get(speaker).unwrap();
        let mut ir = s.left.as_ref().or(s.right.as_ref()).unwrap().clone();
        ir.data.resize(48000, 0.);
        let peak = ir
            .data
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        let (out, stages) = room_tuning::design_speaker_with_stages(
            speaker,
            &[(names.side.unwrap(), ir)],
            &TuningOptions::default(),
            10.,
        )
        .unwrap();
        for (key, value) in [
            ("peak_idx", peak as f64),
            ("ref_mag", out.reference),
            ("low_cutoff", out.low_cutoff),
            ("high_cutoff", out.high_cutoff),
        ] {
            let expected = reference[key].as_f64().unwrap();
            let error = (value - expected).abs();
            let tolerance = if key == "ref_mag" {
                expected.abs() * 1e-4
            } else {
                1e-9
            };
            println!(
                "STAGE {entry} {key}: ours={value:.12} golden={expected:.12} error={error:.9}"
            );
            bad |= error > tolerance;
        }
        let db = |x: &[f64]| {
            x.iter()
                .map(|v| 20. * v.max(1e-12).log10())
                .collect::<Vec<_>>()
        };
        let final_mag = fft::rfft(&out.full_filter)
            .iter()
            .map(|v| v.norm())
            .collect::<Vec<_>>();
        for (key, ours, tolerance) in [
            ("phase_reg", out.weights.clone(), 1e-3),
            ("smoothed_mag_db", db(&stages.smoothed_magnitude), 0.01),
            ("track_weight", stages.track_weight, 1e-3),
            ("target_curve_db", db(&out.target), 0.01),
            ("mag_inv_db", db(&out.inverse), 0.01),
            ("windowed_min_db", db(&stages.windowed_minimum), 0.01),
            ("macro_db", db(&out.macro_gain), 0.01),
            ("crush_db", db(&stages.crush), 0.01),
            ("final_mag_db", db(&final_mag), 0.01),
        ] {
            let expected = values(&reference[key]);
            assert_eq!(expected.len(), bins.len());
            let (index, error) = bins
                .iter()
                .enumerate()
                .map(|(i, k)| (i, (ours[*k] - expected[i]).abs()))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            println!(
                "STAGE {entry} {key}: max={error:.9} bin={} ours={:.9} golden={:.9}",
                bins[index], ours[bins[index]], expected[index]
            );
            bad |= error > tolerance;
        }
    }
    assert!(!bad, "intermediate stage differs from SECS");
}
fn values(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
fn grid_mean(x: &[f64], grid: &[f64]) -> Vec<f64> {
    grid.iter()
        .map(|g| {
            let lo = (g * 2_f64.powf(-1. / 24.)).ceil() as usize;
            let hi = (g * 2_f64.powf(1. / 24.)).floor() as usize;
            if hi >= lo {
                x[lo..=hi.min(x.len() - 1)].iter().sum::<f64>() / (hi - lo + 1) as f64
            } else {
                let k = g.floor() as usize;
                x[k] * (1. - g.fract()) + x[k + 1] * g.fract()
            }
        })
        .collect()
}
fn stats(v: &[f64]) -> (f64, f64, f64) {
    let mut a: Vec<_> = v.iter().map(|v| v.abs()).collect();
    a.sort_by(f64::total_cmp);
    if a.is_empty() {
        return (0., 0., 0.);
    }
    (
        (a.iter().map(|v| v * v).sum::<f64>() / a.len() as f64).sqrt(),
        (a[(a.len() - 1) / 2] + a[a.len() / 2]) / 2.,
        *a.last().unwrap(),
    )
}
#[test]
fn room_tuning_matches_secs_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs_reference.json"
    ))
    .unwrap();
    // The JSON labels are rounded to four decimals. Reconstruct the generating
    // 1/24-octave grid: rounded endpoints can add/drop a 1 Hz analysis bin.
    let labels = values(&golden["grid_hz"]);
    let grid: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let f = 15. * 2_f64.powf(i as f64 / 24.);
            assert!((f - label).abs() <= 0.000051);
            f
        })
        .collect();
    assert_eq!(grid.len(), 250);
    let wav = read_wav(&root.join(golden["ir"]["estimator"].as_str().unwrap())).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut bad = false;
    let mut all_mag: Vec<Vec<f64>> = vec![vec![]; 5];
    let mut all_gd: Vec<Vec<f64>> = vec![vec![]; 5];
    for (file, entries) in golden["files"].as_object().unwrap() {
        let names = room::parse_room_measurement_name(file).unwrap();
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let mut h = Hrir {
            fs: 48000,
            speakers: vec![],
        };
        h.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &names
                .speakers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            names.side,
            2.,
        )
        .unwrap();
        h.for_each_ir(|ir| ir.crop_head(1.));
        for (speaker, reference) in entries.as_object().unwrap() {
            let s = h.get(speaker).unwrap();
            let mut ir = s.left.as_ref().or(s.right.as_ref()).unwrap().clone();
            ir.data.resize(48000, 0.);
            let out = room_tuning::design_speaker(
                speaker,
                &[(names.side.unwrap(), ir)],
                &TuningOptions {
                    level_match: false,
                    ..Default::default()
                },
                10.,
                None,
                None,
            )
            .unwrap();
            let spec = fft::rfft(&out.full_filter);
            let mag: Vec<_> = spec
                .iter()
                .map(|v| 20. * v.norm().max(1e-12).log10())
                .collect();
            let minimum = room_tuning::minimum_phase(
                &spec.iter().map(|v| v.norm()).collect::<Vec<_>>(),
                48000,
            );
            let mut last = 0.;
            let phase: Vec<_> = spec
                .iter()
                .zip(minimum)
                .enumerate()
                .map(|(k, (h, m))| {
                    let z = h / m * fft::Complex64::from_polar(1., 2. * PI * k as f64 * 0.010);
                    let p = last + (z.arg() - last + PI).rem_euclid(2. * PI) - PI;
                    last = p;
                    p
                })
                .collect();
            let mut gd = vec![0.; phase.len()];
            for k in 1..phase.len() - 1 {
                gd[k] = -(phase[k + 1] - phase[k - 1]) * 1000. / (4. * PI);
            }
            gd[1] = -(phase[2] - phase[1]) * 1000. / (2. * PI);
            let end = phase.len() - 2;
            gd[end] = -(phase[end] - phase[end - 1]) * 1000. / (2. * PI);
            let ours = grid_mean(&mag, &grid);
            let ours_gd = grid_mean(&gd, &grid);
            let g = grid_mean(&out.weights, &grid);
            let theirs = values(&reference["mag_db"]);
            let theirs_gd = values(&reference["excess_gd_ms"]);
            let center = |x: &[f64]| {
                let v: Vec<_> = x
                    .iter()
                    .zip(&grid)
                    .filter(|(_, f)| **f >= 100. && **f <= 10000.)
                    .map(|(v, _)| *v)
                    .collect();
                v.iter().sum::<f64>() / v.len() as f64
            };
            let oc = center(&ours);
            let tc = center(&theirs);
            println!(
                "{file} {speaker}: cutoffs {:.0}/{:.0} reference {}/{}",
                out.low_cutoff,
                out.high_cutoff,
                reference["low_cutoff_hz"],
                reference["high_cutoff_hz"]
            );
            for (i, (lo, hi)) in [
                (20., 100.),
                (100., 300.),
                (300., 600.),
                (600., 10000.),
                (10000., 16000.),
            ]
            .iter()
            .enumerate()
            {
                let diff: Vec<_> = grid
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| **f >= *lo && **f < *hi)
                    .map(|(k, _)| (ours[k] - oc) - (theirs[k] - tc))
                    .collect();
                let (r, med, m) = stats(&diff);
                println!(" magnitude {lo}-{hi}: RMS {r:.6} median {med:.6} max {m:.6} dB");
                all_mag[i].push(m);
                bad |= m > MAG_MAX_DB[i];
            }
            for (i, (lo, hi)) in [
                (20., 40.),
                (40., 100.),
                (100., 300.),
                (300., 1000.),
                (1000., 5000.),
            ]
            .iter()
            .enumerate()
            {
                let selected: Vec<_> = grid
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| **f >= *lo && **f < *hi)
                    .map(|(k, _)| k)
                    .collect();
                let diff: Vec<_> = selected
                    .iter()
                    .filter(|k| g[**k] >= 0.9)
                    .map(|k| ours_gd[*k] - theirs_gd[*k])
                    .collect();
                let (r, med, m) = stats(&diff);
                println!(
                    " excess GD {lo}-{hi}: RMS {r:.6} median {med:.6} max {m:.6} ms kept {}/{}",
                    diff.len(),
                    selected.len()
                );
                all_gd[i].push(m);
                if i > 0 {
                    bad |= m > GD_MAX_MS[i];
                }
            }
            for (v, key) in [
                (out.low_cutoff, "low_cutoff_hz"),
                (out.high_cutoff, "high_cutoff_hz"),
            ] {
                let oct = (v / reference[key].as_f64().unwrap()).log2().abs();
                println!(" {key} difference {oct:.6} octave");
                bad |= oct > 1. / 12.;
            }
            if file == "room-FL,FR-left.wav" && speaker == "FL" {
                println!("FL table: Hz golden_mag ours_mag golden_GD ours_GD G");
                for f in [
                    30., 40., 50., 63., 80., 100., 125., 160., 200., 250., 300., 500., 1000.,
                    2000., 5000., 10000.,
                ] {
                    let k = grid
                        .iter()
                        .enumerate()
                        .min_by(|a, b| (*a.1 - f).abs().total_cmp(&(*b.1 - f).abs()))
                        .unwrap()
                        .0;
                    println!(
                        "{f:.0} {:.4} {:.4} {:.4} {:.4} {:.4}",
                        theirs[k] - tc,
                        ours[k] - oc,
                        theirs_gd[k],
                        ours_gd[k],
                        g[k]
                    );
                }
            }
        }
    }
    for i in 0..5 {
        println!(
            "aggregate band {i}: magnitude maxima median/max {:?}; GD maxima median/max {:?}",
            stats(&all_mag[i]),
            stats(&all_gd[i])
        );
    }
    assert!(
        !bad,
        "processor/reference discrepancy exceeds investigation threshold"
    );
}
