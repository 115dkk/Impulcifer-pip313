#![forbid(unsafe_code)]

//! Frozen Python inputs isolate every primitive; --nocapture prints measured errors.
use impulcifer_dsp::{
    conv::{self, Mode},
    fft::{self, Complex64},
    filters::{self, BType, Sos},
    stats, windows,
};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn fixtures(prefix: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&format!("p03_{prefix}"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures for {prefix}");
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
fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("-inf") => f64::NEG_INFINITY,
        Some("+inf") => f64::INFINITY,
        Some("nan") => f64::NAN,
        _ => v.as_f64().unwrap(),
    }
}
fn array(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(number).collect()
}
fn complex(v: &Value) -> Vec<Complex64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| Complex64::new(number(&v[0]), number(&v[1])))
        .collect()
}
fn integer(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn norm(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn mode(v: &Value) -> Mode {
    match v.as_str().unwrap() {
        "full" => Mode::Full,
        "same" => Mode::Same,
        "valid" => Mode::Valid,
        _ => panic!("invalid mode"),
    }
}
fn sos(v: &Value) -> Sos {
    Sos(v
        .as_array()
        .unwrap()
        .iter()
        .map(|v| array(v).try_into().unwrap())
        .collect())
}

#[derive(Default)]
struct Errors {
    absolute: f64,
    relative_l2: f64,
    count: usize,
}
impl Errors {
    fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], rtol: f64, atol: f64) {
        assert_eq!(got.len(), expected.len(), "{name}: shape mismatch");
        let mut square_error = 0.0;
        let mut square_reference = 0.0;
        for (i, (&g, &e)) in got.iter().zip(expected).enumerate() {
            if !e.is_finite() {
                assert!(
                    (e.is_nan() && g.is_nan()) || e == g,
                    "{name}[{i}]: got {g}, expected {e}, nonfinite mask mismatch"
                );
                continue;
            }
            let error = (g - e).abs();
            let tolerance = atol + rtol * e.abs();
            assert!(
                g.is_finite() && error <= tolerance,
                "{name}[{i}]: got {g:.17e}, expected {e:.17e}, error {error:.17e}, tolerance {tolerance:.17e}"
            );
            self.absolute = self.absolute.max(error);
            square_error += error * error;
            square_reference += e * e;
            self.count += 1;
        }
        if square_reference > 0.0 {
            self.relative_l2 = self
                .relative_l2
                .max((square_error / square_reference).sqrt());
        }
    }
    fn report(&self, family: &str) {
        println!(
            "MEASURE {family}: max_abs={:.17e}, max_relative_l2={:.17e}, finite_values={}",
            self.absolute, self.relative_l2, self.count
        );
    }
}
fn compare_complex(
    errors: &mut Errors,
    name: &str,
    got: &[Complex64],
    expected: &[Complex64],
    scale: f64,
) {
    assert_eq!(got.len(), expected.len(), "{name}: complex shape");
    // Complex norm gate, not independent real/imag relative thresholds.
    for (i, (g, e)) in got.iter().zip(expected).enumerate() {
        let tolerance = 1e-12 * scale.max(1.0) + 1e-11 * e.norm();
        assert!(
            (*g - *e).norm() <= tolerance,
            "{name}[{i}]: got {g}, expected {e}, tolerance {tolerance:.17e}"
        );
    }
    let g: Vec<_> = got.iter().flat_map(|v| [v.re, v.im]).collect();
    let e: Vec<_> = expected.iter().flat_map(|v| [v.re, v.im]).collect();
    errors.compare(name, &g, &e, 1e-11, 1e-12 * scale.max(1.0));
}

#[test]
fn golden_rfft_matches_numpy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("rfft_") {
        let x = array(&v["inputs"]["x"]);
        compare_complex(
            &mut errors,
            &name,
            &fft::rfft(&x),
            &complex(&v["outputs"]["spectrum"]),
            x.iter().map(|v| v.abs()).sum(),
        );
    }
    assert!(errors.relative_l2 <= 1e-11);
    errors.report("rfft");
}
#[test]
fn golden_irfft_matches_numpy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("irfft_") {
        let x = complex(&v["inputs"]["spectrum"]);
        if v["outputs"].get("error").is_some() {
            assert!(
                std::panic::catch_unwind(|| fft::irfft(&x, integer(&v["inputs"]["n"]))).is_err()
            );
            continue;
        }
        let scale = x.iter().map(|x| x.norm()).sum::<f64>().max(1.0);
        errors.compare(
            &name,
            &fft::irfft(&x, integer(&v["inputs"]["n"])),
            &array(&v["outputs"]["x"]),
            1e-11,
            1e-12 * scale,
        );
    }
    assert!(errors.relative_l2 <= 1e-11);
    errors.report("irfft");
}
#[test]
fn golden_complex_fft_matches_numpy() {
    for (prefix, transform) in [
        ("fft_", fft::fft as fn(&[Complex64]) -> Vec<Complex64>),
        ("ifft_", fft::ifft),
    ] {
        let mut errors = Errors::default();
        for (name, v) in fixtures(prefix) {
            let x = complex(&v["inputs"]["x"]);
            compare_complex(
                &mut errors,
                &name,
                &transform(&x),
                &complex(&v["outputs"]["spectrum"]),
                x.iter().map(|v| v.norm()).sum(),
            );
        }
        assert!(errors.relative_l2 <= 1e-11);
        errors.report(prefix.trim_end_matches('_'));
    }
}
#[test]
fn golden_magnitude_response_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("magnitude_") {
        let x = array(&v["inputs"]["x"]);
        errors.compare(
            &name,
            &fft::magnitude_response(&x),
            &array(&v["outputs"]["db"]),
            1e-11,
            1e-12,
        );
    }
    errors.report("magnitude_response_db");
}
#[test]
fn golden_fast_lengths_match_scipy() {
    for (name, v) in fixtures("fast_lengths") {
        for (i, n) in v["inputs"]["n"].as_array().unwrap().iter().enumerate() {
            let n = integer(n);
            assert_eq!(
                fft::next_fast_len_legacy(n),
                integer(&v["outputs"]["legacy"][i]),
                "{name}[{i}]"
            );
            assert_eq!(
                fft::next_fast_len(n),
                integer(&v["outputs"]["real"][i]),
                "{name}[{i}]"
            );
        }
    }
    println!("MEASURE next_fast_len/legacy: max_abs=0 (exact integers)");
}
#[test]
fn golden_convolve_modes_match_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("convolve_") {
        let a = array(&v["inputs"]["a"]);
        let h = array(&v["inputs"]["v"]);
        errors.compare(
            &name,
            &conv::convolve(&a, &h, mode(&v["inputs"]["mode"])),
            &array(&v["outputs"]["y"]),
            0.0,
            1e-11 * (norm(&a) * norm(&h)).max(1.0),
        );
    }
    assert!(errors.relative_l2 <= 1e-11);
    errors.report("convolve");
}
#[test]
fn golden_correlate_modes_match_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("correlate_") {
        let a = array(&v["inputs"]["a"]);
        let h = array(&v["inputs"]["v"]);
        errors.compare(
            &name,
            &conv::correlate(&a, &h, mode(&v["inputs"]["mode"])),
            &array(&v["outputs"]["y"]),
            0.0,
            1e-11 * (norm(&a) * norm(&h)).max(1.0),
        );
    }
    assert!(errors.relative_l2 <= 1e-11);
    errors.report("correlate");
}
#[test]
fn golden_correlation_lags_match_scipy() {
    for (name, v) in fixtures("correlate_") {
        let a = array(&v["inputs"]["a"]);
        let h = array(&v["inputs"]["v"]);
        let expected: Vec<_> = v["outputs"]["lags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap())
            .collect();
        assert_eq!(
            conv::correlation_lags(a.len(), h.len(), mode(&v["inputs"]["mode"])),
            expected,
            "{name}"
        );
    }
    println!("MEASURE correlation_lags: max_abs=0 (exact integers)");
}
#[test]
fn golden_windows_match_scipy() {
    for family in ["hann", "hamming", "boxcar"] {
        let mut errors = Errors::default();
        for (name, v) in fixtures(&format!("window_{family}_")) {
            let i = &v["inputs"];
            let n = integer(&i["n"]);
            let sym = i["sym"].as_bool().unwrap();
            let expected = array(&v["outputs"]["y"]);
            if v["outputs"]["get_window_error"] == true {
                assert!(windows::get_window(family, n, !sym).is_err());
            } else {
                errors.compare(
                    &name,
                    &windows::get_window(family, n, !sym).unwrap(),
                    &expected,
                    0.0,
                    1e-12,
                );
            }
            match family {
                "hann" => errors.compare(&name, &windows::hann(n, sym), &expected, 0.0, 1e-12),
                "hamming" => {
                    errors.compare(&name, &windows::hamming(n, sym), &expected, 0.0, 1e-12)
                }
                _ => (),
            }
        }
        errors.report(family);
    }
}
#[test]
fn golden_kaiser_matches_scipy() {
    let mut errors = Errors::default();
    let mut sums = Errors::default();
    for (name, v) in fixtures("kaiser_") {
        let i = &v["inputs"];
        let got = windows::kaiser(
            integer(&i["n"]),
            number(&i["beta"]),
            i["sym"].as_bool().unwrap(),
        );
        if let Some(y) = v["outputs"].get("y") {
            errors.compare(&name, &got, &array(y), 0.0, 1e-12);
        } else {
            errors.compare(
                &name,
                &got[..64],
                &array(&v["outputs"]["first"]),
                0.0,
                1e-12,
            );
            errors.compare(
                &name,
                &got[got.len() - 64..],
                &array(&v["outputs"]["last"]),
                0.0,
                1e-12,
            );
            // A sum of N taps has N times the absolute per-tap error budget.
            sums.compare(
                &name,
                &[got.iter().sum()],
                &[number(&v["outputs"]["sum"])],
                0.0,
                got.len() as f64 * 1e-12,
            );
        }
    }
    errors.report("kaiser_taps");
    sums.report("kaiser_sum");
}
#[test]
fn golden_butter_sos_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("butter_") {
        let i = &v["inputs"];
        let kind = if i["kind"] == "lowpass" {
            BType::Lowpass
        } else {
            BType::Highpass
        };
        let got = filters::butter(integer(&i["order"]), number(&i["wn"]), kind);
        let expected = sos(&v["outputs"]["sos"]);
        assert_eq!(got.0.len(), expected.0.len(), "{name}: SOS shape");
        errors.compare(
            &name,
            &got.0.iter().flatten().copied().collect::<Vec<_>>(),
            &expected.0.iter().flatten().copied().collect::<Vec<_>>(),
            1e-11,
            1e-13,
        );
        for row in got.0 {
            assert_eq!(row[3], 1.0);
            // Jury's conditions for stable real second-order sections.
            assert!(
                row[5].abs() < 1.0 && 1.0 + row[4] + row[5] > 0.0 && 1.0 - row[4] + row[5] > 0.0,
                "{name}: unstable section"
            );
        }
    }
    errors.report("butter_sos");
}
#[test]
fn golden_sosfilt_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("sosfilt_") {
        let x = array(&v["inputs"]["x"]);
        let coefficients = sos(&v["inputs"]["sos"]);
        let y = array(&v["outputs"]["y"]);
        let peak = y.iter().map(|x| x.abs()).fold(0.0, f64::max);
        errors.compare(
            &name,
            &filters::sosfilt(&coefficients, &x),
            &y,
            1e-9,
            1e-11 * peak,
        );
    }
    errors.report("sosfilt");
}
#[test]
fn golden_butter_sosfilt_chain_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("sosfilt_") {
        let i = &v["inputs"];
        if i.get("order").is_none() {
            continue;
        }
        let kind = if i["kind"] == "lowpass" {
            BType::Lowpass
        } else {
            BType::Highpass
        };
        let coefficients = filters::butter(integer(&i["order"]), number(&i["wn"]), kind);
        let y = array(&v["outputs"]["y"]);
        let peak = y.iter().map(|x| x.abs()).fold(0.0, f64::max);
        errors.compare(
            &name,
            &filters::sosfilt(&coefficients, &array(&i["x"])),
            &y,
            1e-9,
            1e-11 * peak,
        );
    }
    assert!(errors.count > 0);
    errors.report("butter_sosfilt_chain");
}
#[test]
fn golden_tf2sos_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("tf2sos_") {
        let got = filters::tf2sos(&array(&v["inputs"]["b"]), &array(&v["inputs"]["a"]));
        let expected = sos(&v["outputs"]["sos"]);
        assert_eq!(got.0.len(), expected.0.len(), "{name}: SOS shape");
        errors.compare(
            &name,
            &got.0.iter().flatten().copied().collect::<Vec<_>>(),
            &expected.0.iter().flatten().copied().collect::<Vec<_>>(),
            1e-11,
            1e-13,
        );
    }
    errors.report("tf2sos");
}
#[test]
fn golden_rbj_matches_autoeq() {
    for family in ["peaking", "low_shelf", "high_shelf"] {
        let mut coefficients = Errors::default();
        let mut response = Errors::default();
        for (name, v) in fixtures(&format!("rbj_{family}_")) {
            let i = &v["inputs"];
            let designer = match family {
                "peaking" => filters::rbj_peaking,
                "low_shelf" => filters::rbj_low_shelf,
                _ => filters::rbj_high_shelf,
            };
            let b = designer(
                number(&i["fc"]),
                number(&i["q"]),
                number(&i["gain"]),
                number(&i["fs"]),
            );
            coefficients.compare(&name, &b.b, &array(&v["outputs"]["b"]), 1e-11, 1e-13);
            coefficients.compare(&name, &b.a, &array(&v["outputs"]["a"]), 1e-11, 1e-13);
            // Isolate response evaluation from coefficient-design roundoff.
            let reference_b = filters::Biquad {
                b: array(&v["outputs"]["b"]).try_into().unwrap(),
                a: array(&v["outputs"]["a"]).try_into().unwrap(),
            };
            response.compare(
                &name,
                &filters::biquad_response_db(&reference_b, &array(&i["freqs"]), number(&i["fs"])),
                &array(&v["outputs"]["db"]),
                1e-11,
                1e-9,
            );
        }
        coefficients.report(&format!("rbj_{family}_coefficients"));
        response.report(&format!("rbj_{family}_response_db"));
    }
}
#[test]
fn golden_linregress_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("linregress_") {
        let r = stats::linregress(&array(&v["inputs"]["x"]), &array(&v["inputs"]["y"]));
        assert!(r.pvalue.is_none());
        for (field, got) in [
            ("slope", r.slope),
            ("intercept", r.intercept),
            ("rvalue", r.rvalue),
            ("stderr", r.stderr),
        ] {
            errors.compare(
                &format!("{name}/{field}"),
                &[got],
                &[number(&v["outputs"][field])],
                1e-10,
                0.0,
            );
        }
    }
    errors.report("linregress");
}
#[test]
fn golden_expit_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("expit") {
        errors.compare(
            &name,
            &array(&v["inputs"]["x"])
                .into_iter()
                .map(stats::expit)
                .collect::<Vec<_>>(),
            &array(&v["outputs"]["y"]),
            1e-14,
            1e-15,
        );
    }
    errors.report("expit");
}
#[test]
fn golden_running_mean_matches_python() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("running_") {
        errors.compare(
            &name,
            &stats::running_mean(&array(&v["inputs"]["x"]), integer(&v["inputs"]["n"])),
            &array(&v["outputs"]["y"]),
            1e-11,
            1e-12,
        );
    }
    errors.report("running_mean");
}
#[test]
fn golden_uniform_filter_matches_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("uniform_") {
        let i = &v["inputs"];
        errors.compare(
            &name,
            &stats::uniform_filter2d_size3_zero(
                integer(&i["rows"]),
                integer(&i["cols"]),
                &array(&i["data"]),
            ),
            &array(&v["outputs"]["y"]),
            1e-9,
            1e-12,
        );
    }
    errors.report("uniform_filter2d");
}
