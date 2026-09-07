#![forbid(unsafe_code)]
//! P05 isolated scipy/2.x oracle comparisons; --nocapture prints actual maxima.
use impulcifer_dsp::{
    fft, fir,
    interp::{self, Spline},
    peaks, smoothing,
};
use serde_json::Value;
use std::{fs, path::PathBuf};

/// Locate scipy/2.x p05_* fixtures without consulting runtime Python.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
/// Load named scipy/2.x p05_<family> JSON fixtures in deterministic order.
fn fixtures(prefix: &str) -> Vec<(String, Value)> {
    let mut paths: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json")
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(&format!("p05_{prefix}"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "missing fixtures: {prefix}");
    paths
        .into_iter()
        .map(|p| {
            let bytes = fs::read(&p).unwrap();
            assert!(bytes.len() < 200_000);
            (
                p.file_name().unwrap().to_str().unwrap().to_owned(),
                serde_json::from_slice(&bytes).unwrap(),
            )
        })
        .collect()
}
/// Decode scipy nonfinite scalars in p05_peaks_* / p05_first_peak_*.
fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("nan") => f64::NAN,
        Some("+inf") => f64::INFINITY,
        Some("-inf") => f64::NEG_INFINITY,
        _ => v.as_f64().unwrap(),
    }
}
/// Decode integer parameters/indices from scipy/2.x p05_* fixtures.
fn integer(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
/// Decode inline or LE-f64 scipy arrays in p05_*, checking saved end taps/count.
fn array(v: &Value) -> Vec<f64> {
    if let Some(values) = v.as_array() {
        return values.iter().map(number).collect();
    }
    let name = v["file"].as_str().unwrap();
    assert!(name.starts_with("p05_") && !name.contains('/') && !name.contains('\\'));
    let bytes = fs::read(root().join(name)).unwrap();
    assert!(bytes.len() < 200_000);
    assert_eq!(bytes.len(), integer(&v["length"]) * 8);
    let values: Vec<_> = bytes
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(&values[..values.len().min(256)], array(&v["first"]));
    assert_eq!(
        &values[values.len().saturating_sub(256)..],
        array(&v["last"])
    );
    values
}

#[derive(Default)]
struct Errors {
    max: f64,
    count: usize,
}
impl Errors {
    /// Enforce scipy P05 absolute+relative budget on every finite output sample.
    fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], atol: f64, rtol: f64) {
        assert_eq!(got.len(), expected.len(), "{name}: length");
        for (i, (&g, &e)) in got.iter().zip(expected).enumerate() {
            let error = (g - e).abs();
            assert!(
                g.is_finite() && e.is_finite() && error <= atol + rtol * e.abs(),
                "{name}[{i}]: got={g:.17e}, expected={e:.17e}, error={error:.17e}, budget={:.17e}",
                atol + rtol * e.abs()
            );
            self.max = self.max.max(error);
            self.count += 1;
        }
    }
    /// Print measured scipy/2.x p05_* maxima for the packet's report.
    fn report(&self, family: &str) {
        println!(
            "MEASURE {family}: max_abs={:.17e}, values={}",
            self.max, self.count
        );
    }
}
/// Compare scipy FIR/minimum_phase spectra above relative -100 dB; p05_* FIRs.
/// Below the floor enforce absolute complex error <= 1e-9 * max(1, spectral peak).
fn spectral(
    name: &str,
    got: &[f64],
    expected: &[f64],
    n: usize,
    db: &mut Errors,
    deep: &mut Errors,
) {
    let mut g = vec![0.0; n];
    let mut e = vec![0.0; n];
    g[..got.len()].copy_from_slice(got);
    e[..expected.len()].copy_from_slice(expected);
    let g = fft::rfft(&g);
    let e = fft::rfft(&e);
    let peak = e.iter().map(|v| v.norm()).fold(0.0, f64::max);
    for (i, (g, e)) in g.iter().zip(&e).enumerate() {
        if e.norm() > peak * 1e-5 {
            db.compare(
                &format!("{name}/bin{i}"),
                &[20.0 * (g.norm() / e.norm()).log10()],
                &[0.0],
                1e-5,
                0.0,
            );
        } else {
            deep.compare(
                &format!("{name}/deep{i}"),
                &[(*g - *e).norm()],
                &[0.0],
                1e-9 * peak.max(1.0),
                0.0,
            );
        }
    }
}

/// Mirrors scipy.signal.firwin2; p05_firwin2_*.json, full coefficient + spectra gate.
#[test]
fn golden_firwin2_matches_scipy() {
    let mut errors = Errors::default();
    let mut db = Errors::default();
    let mut deep = Errors::default();
    for (name, v) in fixtures("firwin2_") {
        let i = &v["inputs"];
        let expected = array(&v["outputs"]["h"]);
        let got = fir::firwin2(
            integer(&i["numtaps"]),
            &array(&i["freq"]),
            &array(&i["gain"]),
            i["nfreqs"].as_u64().map(|v| v as usize),
            number(&i["fs"]),
        )
        .unwrap();
        let peak = expected.iter().map(|v| v.abs()).fold(1.0, f64::max);
        errors.compare(&name, &got, &expected, 1e-11 * peak, 1e-9);
        spectral(
            &name,
            &got,
            &expected,
            (got.len() * 2).next_power_of_two(),
            &mut db,
            &mut deep,
        );
    }
    errors.report("firwin2_taps");
    db.report("firwin2_spectrum_db");
    deep.report("firwin2_deep_absolute");
}
/// Mirrors scipy.signal.minimum_phase 1.17.1; p05_minimum_* frozen input/output.
#[test]
fn golden_minimum_phase_matches_scipy() {
    let mut taps = Errors::default();
    let mut db = Errors::default();
    let mut deep = Errors::default();
    for (name, v) in fixtures("minimum_") {
        let i = &v["inputs"];
        if name == "p05_minimum_default.json" {
            for (length, expected) in i["lengths"]
                .as_array()
                .unwrap()
                .iter()
                .zip(v["outputs"]["n_fft"].as_array().unwrap())
            {
                assert_eq!(
                    fir::minimum_phase_default_nfft(integer(length)),
                    integer(expected)
                );
            }
            continue;
        }
        let got = fir::minimum_phase(
            &array(&i["h"]),
            integer(&i["n_fft"]),
            i["half"].as_bool().unwrap(),
        )
        .unwrap();
        let expected = array(&v["outputs"]["y"]);
        // Time-domain gate: the FIR-coefficient budget of report-02 section 3.2
        // (rtol 1e-9, atol 1e-11 * max(1, peak)). The spectral budget below
        // discards phase, so this assertion is what pins the waveform/timing.
        assert_eq!(got.len(), expected.len(), "{name}");
        let peak = expected.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let atol = 1e-11 * peak.max(1.0);
        for (k, (&a, &b)) in got.iter().zip(&expected).enumerate() {
            let err = (a - b).abs();
            assert!(
                err <= atol + 1e-9 * b.abs(),
                "{name}: tap {k} differs by {err:e} (expected {b:e}, got {a:e})"
            );
            taps.max = taps.max.max(err);
            taps.count += 1;
        }
        spectral(
            &name,
            &got,
            &expected,
            integer(&i["n_fft"]) * 2,
            &mut db,
            &mut deep,
        );
    }
    taps.report("minimum_phase_taps");
    db.report("minimum_phase_spectrum_db");
    deep.report("minimum_phase_deep_absolute");
    println!("MEASURE minimum_phase_default_nfft: max_abs=0 (exact integers)");
}
/// Mirror scipy.signal.savgol_filter one/1000 iterations; p05_savgol_*.json.
fn savgol_passes(passes: usize) {
    let mut errors = Errors::default();
    for (name, v) in fixtures("savgol_") {
        let i = &v["inputs"];
        if integer(&i["passes"]) != passes {
            continue;
        }
        let mut y = array(&i["x"]);
        for _ in 0..passes {
            y = smoothing::savgol_filter(&y, integer(&i["window"]), integer(&i["polyorder"]))
                .unwrap();
        }
        errors.compare(
            &name,
            &y,
            &array(&v["outputs"]["y"]),
            if passes == 1 { 1e-10 } else { 1e-7 },
            0.0,
        );
    }
    assert!(errors.count > 0);
    errors.report(&format!("savgol_{passes}_passes"));
}
/// Mirrors scipy.signal.savgol_filter; p05_savgol_*_1.json.
#[test]
fn golden_savgol_matches_scipy() {
    savgol_passes(1);
}
/// Mirrors repeated AutoEQ/scipy smoothing; p05_savgol_*_1000.json.
#[test]
fn golden_savgol_thousand_passes_stay_within_budget() {
    savgol_passes(1000);
}
/// Mirrors FrequencyResponse._window_size; p05_fractional_window.json.
#[test]
fn golden_fractional_octave_window_matches_python() {
    let (_, v) = fixtures("fractional_window").remove(0);
    let i = &v["inputs"];
    for ((o, r), w) in array(&i["octaves"])
        .iter()
        .zip(array(&i["ratios"]))
        .zip(v["outputs"]["window"].as_array().unwrap())
    {
        assert_eq!(smoothing::fractional_octave_window(*o, r), integer(w));
    }
    println!("MEASURE fractional_octave_window: max_abs=0 (exact integers)");
}
/// Mirrors scipy.signal.find_peaks height/plateau rules; p05_peaks_*.json.
#[test]
fn golden_find_peaks_matches_scipy() {
    for (name, v) in fixtures("peaks_") {
        let i = &v["inputs"];
        let height = if i["height"].is_null() {
            None
        } else {
            Some(number(&i["height"]))
        };
        let expected: Vec<_> = v["outputs"]["indices"]
            .as_array()
            .unwrap()
            .iter()
            .map(integer)
            .collect();
        assert_eq!(
            peaks::find_peaks(&array(&i["x"]), height),
            expected,
            "{name}"
        );
    }
    println!("MEASURE find_peaks: max_abs=0 (exact indices)");
}
/// Mirrors core.decay._peak_index and ImpulseResponse.peak_index;
/// p05_first_peak_*.json indices come from the Python functions verbatim.
#[test]
fn golden_first_peak_index_matches_python() {
    for (name, v) in fixtures("first_peak_") {
        let i = &v["inputs"];
        assert_eq!(
            peaks::first_peak_index(
                &array(&i["data"]),
                integer(&i["start"]),
                i["end"].as_u64().map(|v| v as usize),
                number(&i["height"])
            ),
            integer(&v["outputs"]["index"]),
            "{name}"
        );
    }
    println!("MEASURE first_peak_index: max_abs=0 (exact indices)");
}
/// Mirror FITPACK interpolation and ext=0; p05_spline_k*.json.
fn spline_degree(k: usize) {
    let mut errors = Errors::default();
    for (name, v) in fixtures(&format!("spline_k{k}_")) {
        let i = &v["inputs"];
        let s = Spline::new(&array(&i["x"]), &array(&i["y"]), k).unwrap();
        errors.compare(
            &name,
            &s.eval(&array(&i["at"])),
            &array(&v["outputs"]["y"]),
            1e-9,
            1e-11,
        );
    }
    errors.report(&format!("spline_k{k}"));
}
/// Mirrors FITPACK k=1; p05_spline_k1_*.json.
#[test]
fn golden_spline_k1_matches_fitpack() {
    spline_degree(1);
}
/// Mirrors FITPACK k=2; p05_spline_k2_*.json.
#[test]
fn golden_spline_k2_matches_fitpack() {
    spline_degree(2);
}
/// Mirrors FITPACK not-a-knot k=3; p05_spline_k3_*.json.
#[test]
fn golden_spline_k3_matches_fitpack() {
    spline_degree(3);
}
/// Mirrors AutoEQ.interpolate/get_center_value; p05_log_axis_*.json.
#[test]
fn golden_interp_log_axis_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("log_axis_") {
        let i = &v["inputs"];
        let got = interp::interp_log_axis(
            &array(&i["freq"]),
            &array(&i["values"]),
            integer(&i["k"]),
            &array(&i["at"]),
        )
        .unwrap();
        errors.compare(&name, &got, &array(&v["outputs"]["y"]), 1e-9, 1e-11);
    }
    errors.report("interp_log_axis");
}
/// Prove scipy.interpolate.CubicSpline(bc_type="natural") fails the required
/// FITPACK k=3 budget on p05_spline_k3_four.json, including inside the domain.
#[test]
fn natural_cubic_fails_not_a_knot_fixture() {
    let (_, v) = fixtures("spline_k3_four").remove(0);
    let expected = array(&v["outputs"]["y"]);
    let natural = array(&v["outputs"]["natural_cubic"]);
    let at = array(&v["inputs"]["at"]);
    let x = array(&v["inputs"]["x"]);
    let mut max: f64 = 0.0;
    let mut inside: f64 = 0.0;
    for ((a, b), at) in natural.iter().zip(expected).zip(at) {
        max = max.max((a - b).abs());
        if at >= x[0] && at <= x[x.len() - 1] {
            inside = inside.max((a - b).abs());
        }
    }
    assert!(max > 1e-9 && inside > 1e-9);
    println!(
        "MEASURE negative_natural_cubic: max_abs={max:.17e}, interior={inside:.17e} (must exceed 1e-9)"
    );
}
