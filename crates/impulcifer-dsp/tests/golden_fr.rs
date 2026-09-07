#![forbid(unsafe_code)]
//! P09 full-array comparisons; --nocapture reports measured errors, never estimates.
use impulcifer_dsp::{
    fft,
    fr::{
        self, CenterAt, CompensateOptions, EqualizeParams, FrFields, FrequencyResponse,
        SmoothingParams,
    },
    smoothing,
};
use serde_json::Value;
use std::{fs, path::PathBuf};

/// Locate the Python AutoEQ fixtures p09_* (frequency_response.py:42-1310).
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
/// Read deterministic fixture families for the Python methods under test.
fn fixtures(prefix: &str) -> Vec<(String, Value)> {
    let mut paths: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json")
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(&format!("p09_{prefix}"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "missing fixture family {prefix}");
    paths
        .into_iter()
        .map(|p| {
            (
                p.file_name().unwrap().to_str().unwrap().into(),
                serde_json::from_slice(&fs::read(p).unwrap()).unwrap(),
            )
        })
        .collect()
}
/// Decode P05-compatible nonfinite JSON scalars; p09_interpolate_nan.json.
fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("nan") => f64::NAN,
        Some("+inf") => f64::INFINITY,
        Some("-inf") => f64::NEG_INFINITY,
        _ => v.as_f64().unwrap(),
    }
}
/// Read every sample of inline/LE-f64 fixtures; p09_minimum_*.json.
fn array(v: &Value) -> Vec<f64> {
    if let Some(values) = v.as_array() {
        return values.iter().map(number).collect();
    }
    let name = v["file"].as_str().unwrap();
    assert!(name.starts_with("p09_") && !name.contains('/') && !name.contains('\\'));
    let bytes = fs::read(root().join(name)).unwrap();
    assert_eq!(bytes.len(), v["length"].as_u64().unwrap() as usize * 8);
    let result: Vec<_> = bytes
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(&result[..256], array(&v["first"]));
    assert_eq!(&result[result.len() - 256..], array(&v["last"]));
    result
}
/// Construct Python __init__ (42-117) frozen field states; p09_* fixtures.
fn response(v: &Value) -> FrequencyResponse {
    FrequencyResponse::with_fields(
        "fixture",
        Some(array(&v["frequency"])),
        FrFields {
            raw: Some(array(&v["raw"])),
            smoothed: Some(array(&v["smoothed"])),
            error: Some(array(&v["error"])),
            error_smoothed: Some(array(&v["error_smoothed"])),
            equalization: Some(array(&v["equalization"])),
            equalized_raw: Some(array(&v["equalized_raw"])),
            equalized_smoothed: Some(array(&v["equalized_smoothed"])),
            target: Some(array(&v["target"])),
        },
    )
    .unwrap()
}
#[derive(Default)]
struct Errors {
    max: f64,
    count: usize,
}
impl Errors {
    /// Enforce every Python output's absolute+relative tolerance, p09_*.json.
    fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], atol: f64, rtol: f64) {
        assert_eq!(got.len(), expected.len(), "{name}: shape");
        let mut worst: f64 = 0.0;
        for (i, (&g, &e)) in got.iter().zip(expected).enumerate() {
            if !g.is_finite() || !e.is_finite() {
                assert!(
                    g == e || (g.is_nan() && e.is_nan()),
                    "{name}/{i}: nonfinite mask"
                );
                continue;
            }
            let error = (g - e).abs();
            assert!(
                error <= atol + rtol * e.abs(),
                "{name}/{i}: got={g:.17e}, expected={e:.17e}, error={error:.17e}, budget={:.17e}",
                atol + rtol * e.abs()
            );
            worst = worst.max(error);
            self.max = self.max.max(error);
            self.count += 1;
        }
        println!("MEASURE {name}: max_abs={worst:.17e}, values={}", got.len());
    }
    /// Python FR field/reset comparison (148-179); p09_* state fixtures.
    fn state(&mut self, name: &str, got: &FrequencyResponse, expected: &Value, exact: bool) {
        assert_eq!(
            got.frequency,
            array(&expected["frequency"]),
            "{name}/frequency"
        );
        for (key, values) in [
            ("raw", &got.raw),
            ("smoothed", &got.smoothed),
            ("error", &got.error),
            ("error_smoothed", &got.error_smoothed),
            ("equalization", &got.equalization),
            ("equalized_raw", &got.equalized_raw),
            ("equalized_smoothed", &got.equalized_smoothed),
            ("target", &got.target),
        ] {
            self.compare(
                &format!("{name}/{key}"),
                values,
                &array(&expected[key]),
                if exact { 0.0 } else { 1e-9 },
                if exact { 0.0 } else { 1e-11 },
            );
        }
    }
    /// Report measured aggregate maxima; p09_* fixtures.
    fn report(&self, name: &str) {
        println!(
            "MEASURE FAMILY {name}: max_abs={:.17e}, values={}",
            self.max, self.count
        );
    }
}

/// Python generate_frequencies (850-857); p09_generate_*.json, exact grids/ends.
#[test]
fn golden_generate_frequencies_match_python() {
    let cases = fixtures("generate_");
    assert_eq!(cases.len(), 3);
    for ((name, v), count) in cases.into_iter().zip([783, 695, 713]) {
        let i = &v["inputs"];
        let got =
            fr::generate_frequencies(number(&i["min"]), number(&i["max"]), number(&i["step"]));
        assert_eq!(got, array(&v["outputs"]["frequency"]), "{name}");
        assert_eq!(got.len(), count);
        assert_eq!(*got.last().unwrap(), number(&v["outputs"]["last"]));
    }
    println!(
        "MEASURE FAMILY generate: max_abs=0, counts=783/695/713; complete grids and last values exact"
    );
}

/// Python _window_size/_sigmoid (1033-1058); p09_window.json/p09_sigmoid_*.json.
#[test]
fn golden_window_size_and_sigmoid_match_python() {
    let mut errors = Errors::default();
    for (_, v) in fixtures("window") {
        let f = array(&v["inputs"]["frequency"]);
        let octaves = array(&v["inputs"]["octaves"]);
        let expected = array(&v["outputs"]["sizes"]);
        assert_eq!(expected, [7.0, 13.0, 15.0, 23.0, 91.0, 139.0]);
        for (o, e) in octaves.into_iter().zip(expected) {
            assert_eq!(fr::window_size(&f, o), e as usize);
            assert_eq!(
                fr::window_size(&f, o),
                smoothing::fractional_octave_window(o, 1.01)
            );
        }
    }
    for (name, v) in fixtures("sigmoid_") {
        let p = array(&v["inputs"]["args"]);
        let got = fr::sigmoid(&array(&v["inputs"]["frequency"]), p[0], p[1], p[2], p[3]);
        errors.compare(&name, &got, &array(&v["outputs"]["y"]), 1e-9, 1e-11);
    }
    errors.report("sigmoid (window sizes exact)");
}

/// Python create_target/_tilt (942-982), biquad.py:52-79,112-131; p09_target_*.
#[test]
fn golden_create_target_matches_autoeq() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("target_") {
        let p = &v["inputs"]["args"];
        let got = fr::create_target(
            &array(&v["inputs"]["frequency"]),
            number(&p[0]),
            number(&p[1]),
            number(&p[2]),
            p[3].as_f64(),
        );
        errors.compare(&name, &got, &array(&v["outputs"]["y"]), 1e-9, 1e-11);
    }
    errors.report("target");
}

/// Python interpolate (859-901); p09_interpolate_{log,linear,nan}.json.
#[test]
fn golden_interpolate_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("interpolate_") {
        let mut fr = response(&v["inputs"]["fr"]);
        fr.interpolate(Some(&array(&v["inputs"]["query"])), 1.01, 1, 20.0, 20000.0)
            .unwrap();
        errors.state(&name, &fr, &v["outputs"], false);
    }
    errors.report("interpolate");
}

/// Python center (903-940) and core/hrir.py get_center_value (44-74); p09_center_*.
#[test]
fn golden_center_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("center_") {
        let mut fr = response(&v["inputs"]["fr"]);
        let at = &v["inputs"]["at"];
        if !v["inputs"]["bands"].is_null() {
            let got: Vec<_> = v["inputs"]["bands"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| fr.center_value((number(&b[0]), number(&b[1]))))
                .collect();
            errors.compare(&name, &got, &array(&v["outputs"]["values"]), 1e-9, 1e-11);
            errors.compare(
                &name,
                &[fr.center_value_at(number(at)).unwrap()],
                &[number(&v["outputs"]["scalar"])],
                1e-9,
                1e-11,
            );
        } else {
            let at = if at.is_array() {
                CenterAt::Band(number(&at[0]), number(&at[1]))
            } else {
                CenterAt::Frequency(number(at))
            };
            let shift = fr.center(at).unwrap();
            errors.compare(
                &name,
                &[shift],
                &[number(&v["outputs"]["shift"])],
                1e-9,
                1e-11,
            );
            errors.state(&name, &fr, &v["outputs"]["fr"], false);
        }
    }
    errors.report("center");
}

/// Python compensate (984-1031); p09_compensate_{zero,harman}.json.
#[test]
fn golden_compensate_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("compensate_") {
        let mut fr = response(&v["inputs"]["fr"]);
        let compensation = response(&v["inputs"]["compensation"]);
        fr.compensate(
            &compensation,
            &CompensateOptions {
                min_mean_error: v["inputs"]["min_mean_error"].as_bool().unwrap(),
                ..CompensateOptions::default()
            },
        )
        .unwrap();
        errors.state(&name, &fr, &v["outputs"], false);
    }
    errors.report("compensate");
}

/// Python smoothen_heavy_light (1181-1239); p09_heavy_light.json, real headphones.
#[test]
fn golden_smoothen_heavy_light_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("heavy_light") {
        let mut fr = response(&v["inputs"]["fr"]);
        fr.smoothen_heavy_light().unwrap();
        errors.state(&name, &fr, &v["outputs"], false);
    }
    errors.report("heavy_light");
}

/// Python smoothen/_smoothen_fractional_octave (1060-1179); p09_smoothen_*.json.
#[test]
fn golden_smoothen_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("smoothen_") {
        let mut fr = response(&v["inputs"]["fr"]);
        let p = &v["inputs"]["params"];
        let params = SmoothingParams {
            window_size: number(&p["window_size"]),
            treble_window_size: p["treble_window_size"].as_f64().unwrap_or(1.0 / 3.0),
            treble_f_lower: p["treble_f_lower"].as_f64().unwrap_or(100.0),
            treble_f_upper: p["treble_f_upper"].as_f64().unwrap_or(10000.0),
            ..SmoothingParams::default()
        };
        if name.contains("two") {
            fr.smoothen_fractional_octave(&params).unwrap();
        } else {
            fr.smoothen(
                params.window_size,
                params.treble_window_size,
                params.treble_f_lower,
                params.treble_f_upper,
            )
            .unwrap();
        }
        errors.state(&name, &fr, &v["outputs"], false);
    }
    errors.report("smoothen");
}

/// Python equalize (1241-1310); p09_equalize_*.json, frozen and chained smoothers.
#[test]
fn golden_equalize_matches_python() {
    let mut errors = Errors::default();
    let headphone = fixtures("headphones").remove(0).1;
    for (name, v) in fixtures("equalize_") {
        let mut fr = response(&v["inputs"]["fr"]);
        let p = &v["inputs"]["params"];
        let params = EqualizeParams {
            max_gain: number(&p["max_gain"]),
            treble_f_lower: number(&p["treble_f_lower"]),
            treble_f_upper: number(&p["treble_f_upper"]),
            ..EqualizeParams::default()
        };
        fr.equalize(&params).unwrap();
        errors.state(&name, &fr, &v["outputs"], false);
        let mut chained = response(&headphone["outputs"]["left"]);
        chained.smoothen_heavy_light().unwrap();
        chained.equalize(&params).unwrap();
        errors.state(&format!("{name}/chained"), &chained, &v["outputs"], false);
    }
    errors.report("equalize");
}

/// Python minimum_phase_impulse_response (637-681); p09_minimum_*.json.
/// Full taps plus P05 spectra at twice the internal minimum-phase FFT size.
#[test]
fn golden_minimum_phase_impulse_response_matches_python() {
    let mut taps = Errors::default();
    let mut db = Errors::default();
    let mut deep = Errors::default();
    let mut failures = Vec::new();
    for (name, v) in fixtures("minimum_") {
        let i = &v["inputs"];
        let fr = response(&i["fr"]);
        let got = fr
            .minimum_phase_impulse_response(
                i["fs"].as_u64().unwrap() as u32,
                number(&i["f_res"]),
                i["normalize"].as_bool().unwrap(),
            )
            .unwrap();
        assert_eq!(
            got.len(),
            if number(&i["f_res"]) == 5.0 {
                9600
            } else {
                4800
            }
        );
        let expected = array(&v["outputs"]["y"]);
        let peak = expected.iter().map(|v| v.abs()).fold(1.0, f64::max);
        let linear = array(&i["linear_ir"]);
        let isolated = impulcifer_dsp::fir::minimum_phase(&linear, linear.len(), true).unwrap();
        let designed = impulcifer_dsp::fir::firwin2(
            linear.len(),
            &array(&i["mesh"]),
            &array(&i["gain"]),
            None,
            48000.0,
        )
        .unwrap();
        println!(
            "DIAGNOSTIC {name}: frozen_linear_minimum_max={:.17e}, frozen_gain_firwin2_max={:.17e}, full_chain_taps_max={:.17e}",
            isolated
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max),
            designed
                .iter()
                .zip(&linear)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max),
            got.iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max)
        );
        let tap_max = got
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        taps.max = taps.max.max(tap_max);
        taps.count += got.len();
        // The homomorphic transform is ill-conditioned at the Type II FIR's
        // Nyquist zero (|H| there is FFT rounding noise, and it also sets the
        // 1e-7*min log floor), so the Python chain itself moves by up to
        // 4.4e-4 * peak in the taps when its linear FIR is perturbed by 1e-12
        // (tests/migration/oracle_noise_minimum_phase.py, 2026-09-07). The
        // budget is that measured spread with margin, not the primitive's.
        if tap_max > 1e-3 * peak {
            failures.push(format!(
                "{name}: taps {tap_max:.17e} > {:.17e}",
                1e-3 * peak
            ));
        }
        let designed_min =
            impulcifer_dsp::fir::minimum_phase(&designed, designed.len(), true).unwrap();
        println!(
            "DIAGNOSTIC {name}: frozen_gain_through_both_primitives_max={:.17e}",
            designed_min
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max)
        );
        let mut work = FrequencyResponse::new(
            "diagnostic",
            Some(fr.frequency.clone()),
            Some(fr.equalization.clone()),
        )
        .unwrap();
        let f_min = work.frequency[0].max(number(&i["f_res"]) / 2.0);
        let gain_min = -work.center_value_at(f_min).unwrap();
        work.interpolate(Some(&array(&i["mesh"])), 1.01, 1, 20.0, 20000.0)
            .unwrap();
        for (f, r) in work.frequency.iter().zip(&mut work.raw) {
            if *f <= f_min {
                *r = gain_min;
            }
            *r = 10.0_f64.powf(*r * 2.0 / 20.0);
        }
        *work.raw.last_mut().unwrap() = 0.0;
        let gain_max = work
            .raw
            .iter()
            .zip(array(&i["gain"]))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        println!("DIAGNOSTIC {name}: interpolated_linear_gain_max={gain_max:.17e}");
        let n = got.len() * 4;
        let mut g = got.clone();
        g.resize(n, 0.0);
        let mut e = expected.clone();
        e.resize(n, 0.0);
        let g = fft::rfft(&g);
        let e = fft::rfft(&e);
        let peak = e.iter().map(|v| v.norm()).fold(0.0, f64::max);
        let mut db_values = Vec::new();
        let mut deep_values = Vec::new();
        // Band-wise spectral budget (dB) from the measured oracle noise floor
        // (Python spread under 1e-12 perturbation: 4.9e-3 below 16 kHz,
        // 7.6e-3 at 20-23 kHz, 7.8e-2 at 23-23.9 kHz, 9.2 in the last 100 Hz
        // where the Nyquist zero dominates), each with a margin of about 4x.
        let fs = i.get("fs").and_then(|v| v.as_f64()).unwrap_or(48000.0);
        let bands: [(f64, f64, f64); 4] = [
            (0.0, 20000.0, 2e-2),
            (20000.0, 23000.0, 5e-2),
            (23000.0, 23900.0, 0.3),
            (23900.0, f64::INFINITY, 20.0),
        ];
        let mut band_max = [0.0f64; 4];
        for (k, (g, e)) in g.iter().zip(&e).enumerate() {
            if e.norm() > peak * 1e-5 {
                let value = 20.0 * (g.norm() / e.norm()).log10();
                let freq = k as f64 * fs / n as f64;
                let band = bands
                    .iter()
                    .position(|(lo, hi, _)| freq >= *lo && freq < *hi)
                    .unwrap_or(3);
                band_max[band] = band_max[band].max(value.abs());
                db_values.push(value);
            } else {
                deep_values.push((*g - *e).norm());
            }
        }
        let db_max = db_values.iter().map(|v| v.abs()).fold(0.0, f64::max);
        println!(
            "MEASURE {name}: spectrum_db by band <20k={:.3e} 20-23k={:.3e} 23-23.9k={:.3e} >23.9k={:.3e}",
            band_max[0], band_max[1], band_max[2], band_max[3]
        );
        for (band, (lo, hi, tol)) in bands.iter().enumerate() {
            if band_max[band] >= *tol {
                failures.push(format!(
                    "{name}: spectrum {:.17e} dB >= {tol} dB in {lo}-{hi} Hz",
                    band_max[band]
                ));
            }
        }
        let deep_max = deep_values.iter().copied().fold(0.0, f64::max);
        assert!(
            got.iter().all(|v| v.is_finite())
                && db_values.iter().chain(&deep_values).all(|v| v.is_finite())
        );
        db.max = db.max.max(db_max);
        db.count += db_values.len();
        deep.max = deep.max.max(deep_max);
        deep.count += deep_values.len();
        println!(
            "MEASURE {name}: taps={tap_max:.17e}, spectrum_db={db_max:.17e}, deep_absolute={deep_max:.17e}"
        );
        if deep_max > 1e-9 * peak.max(1.0) {
            failures.push(format!("{name}: below-floor complex error {deep_max:.17e}"));
        }
    }
    taps.report("minimum_taps");
    db.report("minimum_spectrum_db");
    deep.report("minimum_deep_absolute");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Python read_from_csv (181-242); p09_csv_*.json, exact parsing of all columns.
#[test]
fn golden_read_from_csv_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("csv_") {
        let i = &v["inputs"];
        let got =
            FrequencyResponse::parse_csv(i["name"].as_str().unwrap(), i["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(got.name, i["name"].as_str().unwrap());
        errors.state(&name, &got, &v["outputs"], true);
        if let Some(path) = i["path"].as_str() {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path);
            let got = FrequencyResponse::read_from_csv(&path).unwrap();
            errors.state(&format!("{name}/file"), &got, &v["outputs"], true);
        }
    }
    errors.report("csv");
}

/// core/impulse_response.py frequency_response (157-188); p09_magnitude_*.json.
#[test]
fn golden_magnitude_to_frequency_response_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("magnitude_") {
        let i = &v["inputs"];
        let got = fr::magnitude_to_frequency_response(
            "magnitude",
            i["fs"].as_u64().unwrap() as u32,
            &array(&i["data"]),
        )
        .unwrap();
        errors.state(&name, &got, &v["outputs"], false);
    }
    errors.report("magnitude");
}
