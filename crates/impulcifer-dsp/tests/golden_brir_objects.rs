#![forbid(unsafe_code)]
//! P08 Python object acceptance tests. Run --nocapture for numeric measurements.
use impulcifer_dsp::{
    decay,
    estimator::SweepEstimator,
    hrir::{Hrir, compact_tracks},
    ir::ImpulseResponse,
};
use impulcifer_types::constants::*;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::OnceLock};

/// Python oracle fixture directory, export_goldens_brir.py.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
/// Read P08 JSON produced by real Python calls.
fn fixture(name: &str) -> Value {
    serde_json::from_slice(
        &fs::read(root().join(format!("tests/migration/goldens/p08_{name}.json"))).unwrap(),
    )
    .unwrap()
}
/// Parse common nonfinite encoding (audio_io:100-113; p08_ir.json).
fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| match v.as_str().unwrap() {
        "-inf" => f64::NEG_INFINITY,
        "+inf" => f64::INFINITY,
        "nan" => f64::NAN,
        _ => panic!("invalid number"),
    })
}
/// Read full LE-f64 or JSON arrays; P08 descriptors preserve every sample.
fn array(v: &Value) -> Vec<f64> {
    if let Some(a) = v.as_array() {
        return a.iter().map(number).collect();
    }
    let bytes = fs::read(
        root()
            .join("tests/migration/goldens")
            .join(v["file"].as_str().unwrap()),
    )
    .unwrap();
    assert_eq!(bytes.len(), v["length"].as_u64().unwrap() as usize * 8);
    bytes
        .chunks_exact(8)
        .map(|x| f64::from_le_bytes(x.try_into().unwrap()))
        .collect()
}
/// Absolute error for P08 comparisons; unmatched nonfinite values count as infinity.
fn comparison_error(a: f64, b: f64) -> f64 {
    if a == b || (a.is_nan() && b.is_nan()) {
        0.0
    } else if !a.is_finite() || !b.is_finite() {
        f64::INFINITY
    } else {
        (a - b).abs()
    }
}
/// Compare full arrays with explicit tolerance, reporting max error before failure.
fn compare(name: &str, got: &[f64], expected: &[f64], atol: f64, rtol: f64) {
    assert_eq!(got.len(), expected.len(), "{name}: lengths");
    let mut max = 0.0_f64;
    let mut failed = None;
    for (i, (&a, &b)) in got.iter().zip(expected).enumerate() {
        if a == b || (a.is_nan() && b.is_nan()) {
            continue;
        }
        let error = comparison_error(a, b);
        max = max.max(error);
        if (!error.is_finite() || error > atol + rtol * b.abs()) && failed.is_none() {
            failed = Some((i, a, b, error));
        }
    }
    println!(
        "MEASURE {name}: max_abs={max:.17e}, count={}, atol={atol:.3e}, rtol={rtol:.3e}",
        got.len()
    );
    assert!(failed.is_none(), "{name}: first failure {failed:?}");
}
/// P08 comparison reporting must not hide unmatched NaN or infinity in max_abs.
#[test]
fn numeric_comparison_reports_nonfinite_mismatches() {
    for (a, b) in [
        (f64::NAN, 1.0),
        (1.0, f64::NAN),
        (f64::INFINITY, 1.0),
        (1.0, f64::NEG_INFINITY),
        (f64::INFINITY, f64::NEG_INFINITY),
        (f64::NAN, f64::INFINITY),
    ] {
        assert_eq!(comparison_error(a, b), f64::INFINITY);
        assert!(
            std::panic::catch_unwind(|| compare("nonfinite regression", &[a], &[b], 1e-9, 0.0))
                .is_err()
        );
    }
    compare(
        "matching nonfinite",
        &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY],
        &[f64::NAN, f64::INFINITY, f64::NEG_INFINITY],
        0.0,
        0.0,
    );
    assert_eq!(comparison_error(2.0, 1.0), 1.0);
}

/// Shared real generated estimator, Python constructor:33-50; p08_estimator.json.
fn estimator() -> &'static SweepEstimator {
    static E: OnceLock<SweepEstimator> = OnceLock::new();
    E.get_or_init(|| SweepEstimator::new(5.0, 48000).unwrap())
}
/// Python from_wav:234-262; p08_repair.json.
fn loaded() -> SweepEstimator {
    let v = fixture("repair");
    let w =
        impulcifer_io::wav::read_wav(&root().join(v["inputs"]["path"].as_str().unwrap())).unwrap();
    SweepEstimator::from_samples(w.sample_rate, &w.tracks[0]).unwrap()
}
/// Actual demo WAV ingestion, Python HRIR.open_recording:397-425; p08_protocol.json.
fn opened() -> Hrir {
    let e = loaded();
    let mut h = Hrir {
        fs: e.fs,
        speakers: Vec::new(),
    };
    for name in fixture("protocol")["outputs"]["recordings"]
        .as_array()
        .unwrap()
    {
        let name = name.as_str().unwrap();
        let w = impulcifer_io::wav::read_wav(&root().join("data/demo").join(name)).unwrap();
        let names: Vec<_> = name.trim_end_matches(".wav").split(',').collect();
        h.open_recording_samples(&e, w.sample_rate, &w.tracks, &names, None, 2.0)
            .unwrap();
    }
    h
}
/// Cache head-cropped demo for isolated decay/magnitude tests; Python hrir:567-631.
fn cropped() -> &'static Hrir {
    static H: OnceLock<Hrir> = OnceLock::new();
    H.get_or_init(|| {
        let mut h = opened();
        h.crop_heads(1.0).unwrap();
        h
    })
}
/// Scalar/endpoint verification, Python stage snapshots; p08_demo_*.json.
fn check_summary(name: &str, x: &[f64], v: &Value, atol: f64) {
    assert_eq!(x.len(), v["length"].as_u64().unwrap() as usize, "{name}");
    compare(
        &format!("{name} first"),
        &x[..x.len().min(256)],
        &array(&v["first"]),
        atol,
        0.0,
    );
    compare(
        &format!("{name} last"),
        &x[x.len().saturating_sub(256)..],
        &array(&v["last"]),
        atol,
        0.0,
    );
}
/// Compare complete per-stage snapshot, Python hrir:397-672; p08_demo_*.json.
fn check_stage(stage: &str, h: &Hrir) {
    let v = fixture(&format!("demo_{stage}"));
    for row in v["outputs"].as_array().unwrap() {
        let speaker = row["speaker"].as_str().unwrap();
        let side = row["side"].as_str().unwrap();
        let s = h.get(speaker).unwrap();
        let ir = if side == "left" {
            s.left.as_ref().unwrap()
        } else {
            s.right.as_ref().unwrap()
        };
        let name = format!("{stage}/{speaker}/{side}");
        let peak = number(&row["max_abs"]);
        check_summary(&name, &ir.data, row, 1e-9 * peak);
        assert_eq!(
            ir.peak_index(0, None, 0.12589),
            row["peak_index"].as_u64().unwrap() as usize,
            "{name} peak"
        );
        let mut argmax = 0;
        for i in 1..ir.len() {
            if ir.data[i].abs() > ir.data[argmax].abs() {
                argmax = i;
            }
        }
        assert_eq!(
            argmax,
            row["argmax"].as_u64().unwrap() as usize,
            "{name} argmax"
        );
        let rms = (ir.data.iter().map(|x| x * x).sum::<f64>() / ir.len() as f64).sqrt();
        compare(
            &format!("{name} metrics"),
            &[ir.data[argmax].abs(), rms],
            &[peak, number(&row["rms"])],
            0.0,
            1e-9,
        );
        if row.get("file").is_some() {
            compare(
                &format!("{name} full"),
                &ir.data,
                &array(row),
                1e-9 * peak,
                0.0,
            );
        }
    }
}
/// Python decay fixture enumeration; core/decay.py:44-403.
fn decay_cases() -> Vec<(String, ImpulseResponse, Value)> {
    let mut cases = Vec::new();
    for speaker in ["FL", "FR", "FC"] {
        for side in ["left", "right"] {
            let name = format!("decay_{speaker}_{side}");
            let pair = cropped().get(speaker).unwrap();
            let ir = if side == "left" {
                pair.left.as_ref()
            } else {
                pair.right.as_ref()
            }
            .unwrap()
            .clone();
            cases.push((name.clone(), ir, fixture(&name)));
        }
    }
    let v = fixture("decay_first_pass");
    cases.push((
        "decay_synthetic".into(),
        ImpulseResponse {
            data: array(&v["inputs"]["data"]),
            fs: 48000,
            recording: None,
        },
        fixture("decay_synthetic"),
    ));
    cases
}

/// Python constants:14-118; p08_constants.json.
#[test]
fn golden_constants_match_python() {
    let v = fixture("constants");
    let o = &v["outputs"];
    assert_eq!(json!(SPEAKER_NAMES), o["SPEAKER_NAMES"]);
    assert_eq!(json!(HESUVI_TRACK_ORDER), o["HESUVI_TRACK_ORDER"]);
    assert_eq!(
        json!(HEXADECAGONAL_TRACK_ORDER),
        o["HEXADECAGONAL_TRACK_ORDER"]
    );
    assert_eq!(json!(IPSILATERAL_PAIRS), o["IPSILATERAL_PAIRS"]);
    for (name, mut values) in [
        ("LEFT_SIDE_SPEAKERS", LEFT_SIDE_SPEAKERS.to_vec()),
        ("RIGHT_SIDE_SPEAKERS", RIGHT_SIDE_SPEAKERS.to_vec()),
        ("CENTER_SPEAKERS", CENTER_SPEAKERS.to_vec()),
    ] {
        values.sort();
        assert_eq!(json!(values), o[name]);
    }
    for (i, s) in SPEAKER_NAMES.iter().enumerate() {
        assert_eq!(SPEAKER_DELAYS[i], number(&o["SPEAKER_DELAYS"][s]));
    }
    for (name, order) in SEQUENCE_TRACK_ORDERS {
        assert_eq!(json!(order), o["SEQUENCE_TRACK_ORDERS"][name]);
    }
    assert_eq!(
        json!([
            base_channel_count(&HESUVI_TRACK_ORDER),
            base_channel_count(&HEXADECAGONAL_TRACK_ORDER),
            base_channel_count(&["FL-left"])
        ]),
        o["base_counts"]
    );
    for (name, side) in o["speaker_sides"].as_object().unwrap() {
        let expected = match side.as_str().unwrap() {
            "left" => Side::Left,
            "right" => Side::Right,
            _ => Side::Center,
        };
        assert_eq!(speaker_side(name), expected);
    }
    let names: Vec<_> = SPEAKER_NAMES
        .iter()
        .flat_map(|s| [track_name(s, "left"), track_name(s, "right")])
        .collect();
    assert_eq!(json!(names), o["track_names"]);
}
/// Python estimator:33-147; p08_estimator.json, full f64 and inline PCM32.
#[test]
fn golden_sweep_estimator_matches_python() {
    let v = fixture("estimator");
    let o = &v["outputs"];
    let e = estimator();
    let reference = array(&o["test_signal"]);
    let quantize = |x: f64| {
        (x * 2147483648.0)
            .round_ties_even()
            .clamp(-2147483648.0, 2147483647.0) as i32
    };
    let mismatches = e
        .test_signal
        .iter()
        .zip(&reference)
        .filter(|(a, b)| quantize(**a) != quantize(**b))
        .count();
    let max = e
        .test_signal
        .iter()
        .zip(&reference)
        .map(|(a, b)| comparison_error(*a, *b))
        .fold(0.0, f64::max);
    println!(
        "MEASURE PCM_32 mismatches={mismatches}/{}; sweep max_abs={max:.17e}",
        reference.len()
    );
    assert_eq!(e.test_signal.len(), o["length"].as_u64().unwrap() as usize);
    compare(
        "estimator scalars",
        &[e.low, e.high, e.n_octaves, e.duration],
        &[
            number(&o["low"]),
            number(&o["high"]),
            number(&o["n_octaves"]),
            number(&o["duration"]),
        ],
        0.0,
        0.0,
    );
    compare(
        "inverse_filter",
        &e.inverse_filter,
        &array(&o["inverse_filter"]),
        1e-12,
        0.0,
    );
    compare("test_signal", &e.test_signal, &reference, 1e-12, 0.0);
    assert!(mismatches <= 8, "PCM32 mismatches={mismatches}");
}
/// Python estimate:149-151; p08_estimator.json.
#[test]
fn golden_estimate_matches_scipy() {
    let e = estimator();
    let mut recording = vec![0.0; e.test_signal.len() + 3000];
    for (i, x) in e.test_signal.iter().enumerate() {
        recording[i + 1000] += x;
        recording[i + 3000] += 0.1 * x;
    }
    let v = fixture("estimator");
    let o = &v["outputs"]["estimate"];
    let got = e.estimate(&recording);
    check_summary("estimate", &got, o, 1e-9 * number(&o["max_abs"]));
    let rms = (got.iter().map(|x| x * x).sum::<f64>() / got.len() as f64).sqrt();
    compare(
        "estimate metrics",
        &[got.iter().fold(0.0f64, |m, x| m.max(x.abs())), rms],
        &[number(&o["max_abs"]), number(&o["rms"])],
        0.0,
        1e-9,
    );
}
/// Python sweep_sequence:153-232; p08_sequence.json (all samples, shared sweep).
#[test]
fn golden_sweep_sequence_matches_python() {
    let reference = array(&fixture("estimator")["outputs"]["test_signal"]);
    for case in fixture("sequence")["outputs"].as_array().unwrap() {
        let names: Vec<_> = case["speakers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        let layout = case["layout"].as_str().unwrap();
        let got = estimator().sweep_sequence(&names, layout).unwrap();
        assert_eq!(got.len(), case["shape"][0].as_u64().unwrap() as usize);
        for (i, row) in got.iter().enumerate() {
            let track = &case["tracks"][i];
            let mut expected = vec![0.0; case["shape"][1].as_u64().unwrap() as usize];
            if let Some(active) = track.get("active") {
                let start = active["start"].as_u64().unwrap() as usize;
                let stop = active["stop"].as_u64().unwrap() as usize;
                let offset = active["sweep_offset"].as_u64().unwrap() as usize;
                expected[start..stop].copy_from_slice(&reference[offset..offset + stop - start]);
            }
            check_summary("sequence", row, track, 1e-12);
            compare(
                &format!("sequence {layout}/{i}"),
                row,
                &expected,
                1e-12,
                0.0,
            );
        }
    }
}
/// Python from_wav:234-262; p08_repair.json.
#[test]
fn golden_from_samples_repair_matches_python() {
    let e = loaded();
    let v = fixture("repair");
    let o = &v["outputs"];
    assert_eq!(e.duration, number(&o["duration"]));
    check_summary("repair signal", &e.test_signal, &o["test_signal"], 1e-12);
    check_summary(
        "repair inverse",
        &e.inverse_filter,
        &o["inverse_filter"],
        1e-12,
    );
    let original = array(&fixture("estimator")["outputs"]["test_signal"]);
    for case in fixture("repair_branches")["outputs"].as_array().unwrap() {
        let samples = if case["branch"] == "length" {
            original[..original.len() - 17].to_vec()
        } else {
            original.iter().map(|x| x * 0.9).collect()
        };
        let repaired = SweepEstimator::from_samples(48000, &samples).unwrap();
        assert_eq!(repaired.test_signal, samples);
        assert_eq!(repaired.duration, number(&case["duration"]));
        check_summary(
            "forced repair inverse",
            &repaired.inverse_filter,
            &case["inverse_filter"],
            1e-12,
        );
    }
}
/// Python file_name:264-273; p08_estimator.json and p08_repair.json.
#[test]
fn golden_file_name_matches_python() {
    assert_eq!(
        estimator().file_name(32),
        fixture("estimator")["outputs"]["file_name"]
            .as_str()
            .unwrap()
    );
    assert_eq!(loaded().file_name(32), "6.15s-48000Hz-32bit-2.93Hz-24000Hz");
}
/// Python IR shift/crop/equalize:82-119; p08_ir.json.
#[test]
fn golden_shift_crop_equalize_match_python() {
    let v = fixture("ir");
    // The input is the crop_heads-stage FL left ear (verified against its summary).
    let base = cropped().pair("FL").unwrap().0.clone();
    check_summary(
        "ir input",
        &base.data,
        &v["inputs"]["data"],
        1e-9 * number(&v["inputs"]["data"]["max_abs"]),
    );
    for name in ["shift_plus", "shift_minus", "crop", "equalize"] {
        let mut ir = base.clone();
        match name {
            "shift_plus" => ir.shift(37),
            "shift_minus" => ir.shift(-37),
            "crop" => ir.crop_head(1.0),
            _ => ir.equalize(&array(&v["inputs"]["fir"])),
        };
        let o = &v["outputs"][name];
        check_summary(
            name,
            &ir.data,
            o,
            if name == "equalize" { 1e-12 } else { 0.0 },
        );
        let rms = (ir.data.iter().map(|x| x * x).sum::<f64>() / ir.len() as f64).sqrt();
        compare(
            &format!("{name} metrics"),
            &[ir.data.iter().fold(0.0f64, |m, x| m.max(x.abs())), rms],
            &[number(&o["max_abs"]), number(&o["rms"])],
            0.0,
            1e-12,
        );
    }
}
/// Python magnitude_response:153-155; p08_ir.json.
#[test]
fn golden_magnitude_response_matches_python() {
    let ir = cropped().pair("FL").unwrap().0;
    let (f, m) = ir.magnitude_response();
    let v = fixture("ir");
    check_summary("frequency", &f, &v["outputs"]["frequency"], 0.0);
    check_summary("magnitude dB", &m, &v["outputs"]["magnitude"], 1e-9);
    let finite: Vec<f64> = m.iter().copied().filter(|x| x.is_finite()).collect();
    let rms = (finite.iter().map(|x| x * x).sum::<f64>() / finite.len() as f64).sqrt();
    compare(
        "magnitude metrics",
        &[finite.iter().fold(0.0f64, |a, x| a.max(x.abs())), rms],
        &[
            number(&v["outputs"]["magnitude"]["max_abs"]),
            number(&v["outputs"]["magnitude"]["rms"]),
        ],
        0.0,
        1e-9,
    );
}
/// Python decay_params:44-260; p08_decay_*.json.
#[test]
fn golden_decay_params_match_python() {
    for (name, ir, v) in decay_cases() {
        let p = ir.decay_params();
        let o = &v["outputs"]["params"];
        assert_eq!(
            json!([p.peak_index, p.knee_index, p.window_size]),
            json!([o[0], o[1], o[3]]),
            "{name}"
        );
        compare(&name, &[p.noise_floor_db], &[number(&o[2])], 1e-9, 0.0);
    }
    for n in [0, 1, 9] {
        let v = fixture(&format!("decay_short_{n}"));
        let p = decay::decay_params(&vec![0.0; n], 48000);
        assert_eq!(
            json!([
                p.peak_index as f64,
                p.knee_index as f64,
                p.noise_floor_db,
                p.window_size as f64
            ]),
            json!(array(&v["outputs"]["params"]))
        );
    }
}
/// Python decay_times:263-352; p08_decay_*.json.
#[test]
fn golden_decay_times_match_python() {
    for (name, ir, v) in decay_cases() {
        let t = ir.decay_times(None);
        for (i, time) in [t.edt, t.rt20, t.rt30, t.rt60].iter().enumerate() {
            let o = &v["outputs"]["times"][i];
            if o.is_null() {
                assert!(time.is_none(), "{name}/{i}");
            } else {
                compare(
                    &format!("{name} time {i}"),
                    &[time.unwrap()],
                    &[number(o)],
                    1e-9,
                    0.0,
                );
            }
        }
    }
}
/// Python decay_adjustment_params/apply_decay_window:355-403; p08_decay_*.json.
#[test]
fn golden_decay_adjustment_matches_python() {
    for (name, ir, v) in decay_cases() {
        let p = ir.decay_adjustment_params(0.3);
        let o = &v["outputs"]["adjustment"];
        if o.is_null() {
            assert!(p.is_none());
            continue;
        }
        assert!(o.is_array(), "Python adjustment error: {o}");
        let p = p.unwrap();
        assert_eq!(
            json!([p.window_start, p.half_window, p.knee_point_index]),
            json!([o[0], o[1], o[2]])
        );
        compare(
            &format!("{name} adjustment"),
            &[p.window_level],
            &[number(&o[3])],
            1e-9,
            0.0,
        );
        let mut data = ir.data.clone();
        decay::apply_decay_window(&mut data, Some(&p));
        check_summary(&name, &data, &v["outputs"]["adjusted"], 1e-12);
        if v["outputs"]["adjusted"].get("file").is_some() {
            compare(
                &format!("{name} window"),
                &data,
                &array(&v["outputs"]["adjusted"]),
                1e-12,
                0.0,
            );
        }
    }
}
/// Python complete sorted demo stage protocol; p08_demo_*.json.
#[test]
fn golden_demo_stages_match_python() {
    let mut h = opened();
    check_stage("open", &h);
    h.crop_heads(1.0).unwrap();
    check_stage("crop_heads", &h);
    h.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
    check_stage("ipsilateral", &h);
    h.align_onset_groups_peak_leftref(None).unwrap();
    check_stage("onset", &h);
    let tail = h.crop_tails(&loaded()).unwrap();
    assert_eq!(
        tail,
        fixture("stage_returns")["outputs"]["crop_tails"]
            .as_u64()
            .unwrap() as usize
    );
    check_stage("crop_tails", &h);
    let gain = h.normalize(Some(-0.1), None).unwrap();
    compare(
        "normalize gain",
        &[gain],
        &[number(&fixture("stage_returns")["outputs"]["normalize"])],
        1e-9,
        0.0,
    );
    check_stage("normalize", &h);
}
/// Python stacking/trimming/compaction:427-474 and brir_layout:20-44; p08_stack.json.
#[test]
fn golden_stack_tracks_match_python() {
    let mut h = cropped().clone();
    h.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
    h.align_onset_groups_peak_leftref(None).unwrap();
    h.crop_tails(&loaded()).unwrap();
    h.normalize(Some(-0.1), None).unwrap();
    check_stage("normalize", &h);
    for case in fixture("stack")["outputs"].as_array().unwrap() {
        let order: &[&str] = if case["layout"] == "HESUVI_TRACK_ORDER" {
            &HESUVI_TRACK_ORDER
        } else {
            &HEXADECAGONAL_TRACK_ORDER
        };
        let rows = h.stack_tracks(order, true).unwrap();
        assert_eq!(json!(&order[..rows.len()]), case["names"]);
        for (row, name) in rows.iter().zip(order) {
            let (speaker, side) = name.split_once('-').unwrap();
            let ir = h.get(speaker).and_then(|s| {
                if side == "left" {
                    s.left.as_ref()
                } else {
                    s.right.as_ref()
                }
            });
            let expected = ir
                .map(|ir| ir.data.clone())
                .unwrap_or_else(|| vec![0.0; row.len()]);
            assert_eq!(*row, expected);
        }
        let full = h.stack_tracks(order, false).unwrap();
        let (_, names) = compact_tracks(&full, order);
        assert_eq!(json!(names), case["compact_names"]);
    }
}

/// Python reflection RMS ratios, core/hrir.py:1022-1109; p08_reflections.json.
#[test]
fn golden_reflection_levels_match_python() {
    let v = fixture("reflections");
    let levels = cropped().calculate_reflection_levels(2.0, 20.0, 50.0, 50.0, 150.0, 1e-12);
    let expected = v["outputs"].as_object().unwrap();
    let mut expected_keys: Vec<_> = expected
        .iter()
        .flat_map(|(speaker, sides)| {
            sides
                .as_object()
                .unwrap()
                .keys()
                .map(move |side| (speaker.as_str(), side.as_str()))
        })
        .collect();
    let side_name = |side| match side {
        Side::Left => "left",
        Side::Right => "right",
        Side::Center => panic!("unexpected center ear in reflection levels"),
    };
    let mut actual_keys: Vec<_> = levels
        .iter()
        .map(|(speaker, side, _)| (speaker.as_str(), side_name(*side)))
        .collect();
    expected_keys.sort_unstable();
    actual_keys.sort_unstable();
    assert_eq!(
        actual_keys, expected_keys,
        "complete reflection speaker/side keys and counts"
    );
    for (speaker, side, levels) in levels {
        let side = side_name(side);
        let o = &v["outputs"][&speaker][side];
        compare(
            "reflection levels",
            &[levels.early_db, levels.late_db],
            &[number(&o["early_db"]), number(&o["late_db"])],
            1e-9,
            0.0,
        );
    }
}
