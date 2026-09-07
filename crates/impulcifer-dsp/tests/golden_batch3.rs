#![forbid(unsafe_code)]
//! P06 full Python arrays; --nocapture prints measured errors, not estimates.
use impulcifer_dsp::{fft, fir, resample, spectrogram};
use serde_json::Value;
use std::{fs, path::PathBuf};

/// Locate nnresample/scipy p06_* fixtures without runtime Python.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
/// Load nnresample/scipy p06_<prefix> JSON in deterministic order.
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
                    .starts_with(&format!("p06_{prefix}"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "missing {prefix} fixtures");
    paths
        .into_iter()
        .map(|p| {
            (
                p.file_name().unwrap().to_str().unwrap().to_owned(),
                serde_json::from_slice(&fs::read(p).unwrap()).unwrap(),
            )
        })
        .collect()
}
/// Decode scipy/nnresample p06_* integer parameters.
fn integer(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
/// Decode full scipy/nnresample p06_* LE-f64 arrays and verify end taps/count.
fn array(v: &Value) -> Vec<f64> {
    if let Some(a) = v.as_array() {
        return a.iter().map(|x| x.as_f64().unwrap()).collect();
    }
    let name = v["file"].as_str().unwrap();
    assert!(name.starts_with("p06_") && !name.contains('/') && !name.contains('\\'));
    let bytes = fs::read(root().join(name)).unwrap();
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
    /// Enforce nnresample/scipy p06_* combined absolute/relative budgets.
    fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], atol: f64, rtol: f64) {
        assert_eq!(got.len(), expected.len(), "{name}: shape");
        for (i, (&g, &e)) in got.iter().zip(expected).enumerate() {
            let error = (g - e).abs();
            assert!(
                g.is_finite() && error <= atol + rtol * e.abs(),
                "{name}[{i}]: got={g:.17e}, expected={e:.17e}, error={error:.17e}, budget={:.17e}",
                atol + rtol * e.abs()
            );
            self.max = self.max.max(error);
            self.count += 1;
        }
    }
    /// Print measured nnresample/scipy p06_* full-array error maxima.
    fn report(&self, family: &str) {
        println!(
            "MEASURE {family}: max_abs={:.17e}, values={}",
            self.max, self.count
        );
    }
}

/// Mirrors nnresample.compute_filt 0.2.4.1; p06_design_* and p06_taps_*.
#[test]
fn golden_nnresample_design_matches_python() {
    let mut errors = Errors::default();
    let mut parameters = Errors::default();
    for (name, v) in fixtures("design_") {
        let i = &v["inputs"];
        let o = &v["outputs"];
        let result = resample::nnresample_design(integer(&i["up"]), integer(&i["down"]));
        if o.get("error").is_some() {
            assert!(result.is_err());
            assert!(resample::nnresample(&[1.0; 4800], 48000, 48000).is_err());
            continue;
        }
        let d = result.unwrap();
        assert_eq!((d.up, d.down), (integer(&o["up"]), integer(&o["down"])));
        let expected = array(&o["taps"]);
        let peak = expected.iter().map(|v| v.abs()).fold(1.0, f64::max);
        errors.compare(&name, &d.taps, &expected, 1e-11 * peak, 1e-9);
        parameters.compare(
            &name,
            &[d.cutoff, d.beta],
            &[o["cutoff"].as_f64().unwrap(), o["beta"].as_f64().unwrap()],
            1e-12,
            0.0,
        );
        // Independently reconstruct the initial null-search spectrum and exact
        // argmin; the public design does not expose a diagnostic-only field.
        let mut initial =
            fir::firwin_lowpass(32001, 1.0 / d.up.max(d.down) as f64, d.beta).unwrap();
        initial.resize(1 << 19, 0.0);
        let spectrum = fft::rfft(&initial);
        let bot = integer(&o["bot"]);
        let top = integer(&o["top"]);
        let argmin = spectrum[bot..top]
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.norm().total_cmp(&b.norm()))
            .unwrap()
            .0;
        assert_eq!(argmin, integer(&o["argmin"]));
        assert_eq!(bot + argmin, integer(&o["null_bin"]));
        parameters.compare(
            &name,
            &[d.cutoff],
            &[2.0 / d.up.max(d.down) as f64 - (bot + argmin) as f64 / spectrum.len() as f64],
            1e-12,
            0.0,
        );
    }
    errors.report("nnresample_design_taps");
    parameters.report("nnresample_design_cutoff_beta");
    println!(
        "MEASURE nnresample_design_argmin: max_abs=0 (exact indices); same-rate design error matched"
    );
}
/// Compare scipy.resample_poly or nnresample.resample full p06_poly_* arrays.
fn compare_resampling(composed: bool) {
    let mut errors = Errors::default();
    let mut demo_errors = Errors::default();
    for (name, v) in fixtures("poly_") {
        let i = &v["inputs"];
        let x = array(&i["x"]);
        let up = integer(&i["up"]);
        let down = integer(&i["down"]);
        let got = if composed {
            resample::nnresample(&x, up as u32, down as u32).unwrap()
        } else {
            resample::resample_poly(&x, up, down, &array(&i["taps"])).unwrap()
        };
        assert_eq!(got.len(), (x.len() * up).div_ceil(down));
        let expected = array(&v["outputs"][if composed { "nnresample" } else { "scipy" }]);
        if name.contains("demo") {
            demo_errors.compare(
                &name,
                &got,
                &expected,
                1e-9 * x.iter().map(|v| v.abs()).fold(0.0, f64::max),
                0.0,
            );
        } else {
            errors.compare(&name, &got, &expected, 1e-10, 0.0);
        }
    }
    errors.report(if composed {
        "nnresample_impulse_sine"
    } else {
        "resample_poly_impulse_sine"
    });
    demo_errors.report(if composed {
        "nnresample_demo"
    } else {
        "resample_poly_demo"
    });
}
/// Mirrors scipy.signal.resample_poly; p06_poly_* frozen explicit taps.
#[test]
fn golden_resample_poly_matches_scipy() {
    compare_resampling(false);
}
/// Mirrors nnresample.resample; p06_poly_* frozen complete design+resampling.
#[test]
fn golden_nnresample_matches_python() {
    compare_resampling(true);
}
/// Mirrors scipy.resample_poly short/even/empty/gcd/same-rate cases; p06_edge_poly_*.
#[test]
fn golden_resample_poly_edges_match_scipy() {
    let mut errors = Errors::default();
    for (name, v) in fixtures("edge_poly_") {
        let i = &v["inputs"];
        let got = resample::resample_poly(
            &array(&i["x"]),
            integer(&i["up"]),
            integer(&i["down"]),
            &array(&i["taps"]),
        )
        .unwrap();
        errors.compare(&name, &got, &array(&v["outputs"]["y"]), 1e-14, 0.0);
    }
    errors.report("resample_poly_edges");
}
/// Mirrors nnresample.resample DC output; p06_dc.json records the requested
/// per-sample 1e-6 unity property's failure in the unmodified Python oracle.
#[test]
fn golden_dc_matches_python_despite_unity_property_failure() {
    let (_, v) = fixtures("dc").remove(0);
    let y =
        resample::nnresample(&vec![1.0; integer(&v["inputs"]["length"])], 44100, 48000).unwrap();
    let mut errors = Errors::default();
    errors.compare("dc", &y, &array(&v["outputs"]["y"]), 1e-10, 0.0);
    assert_eq!(v["outputs"]["oracle_passes_requested_budget"], false);
    assert!(v["outputs"]["max_deviation_from_unity"].as_f64().unwrap() > 1e-6);
    errors.report("nnresample_dc_oracle");
}

/// Mirrors actual ImpulseResponsePlotter.plot_spectrogram; p06_params.json.
#[test]
fn golden_spectrogram_params_match_python() {
    let (_, v) = fixtures("params").remove(0);
    for case in v["outputs"]["cases"].as_array().unwrap() {
        let got = spectrogram::spectrogram_params(
            integer(&case["n"]),
            integer(&case["fs"]) as u32,
            case["f_res"].as_f64().unwrap(),
            integer(&case["n_segments"]),
        );
        let p = &case["params"];
        let expected = if p.is_null() {
            None
        } else {
            Some(spectrogram::SpectrogramParams {
                nfft: integer(&p["nfft"]),
                noverlap: integer(&p["noverlap"]),
            })
        };
        assert_eq!(got, expected, "{case}");
    }
    println!("MEASURE spectrogram_params: max_abs=0 (exact integers/None)");
}
/// Mirrors scipy.signal.spectrogram legacy periodic-Hann PSD; p06_spectrogram_*.
#[test]
fn golden_spectrogram_matches_scipy() {
    let mut errors = Errors::default();
    let mut axes = Errors::default();
    for (name, v) in fixtures("spectrogram_") {
        let i = &v["inputs"];
        let o = &v["outputs"];
        let got = spectrogram::spectrogram(
            &array(&i["x"]),
            integer(&i["fs"]) as u32,
            integer(&i["nperseg"]),
            integer(&i["noverlap"]),
        )
        .unwrap();
        assert_eq!(got.power.len(), integer(&o["shape"][0]));
        assert!(
            got.power
                .iter()
                .all(|row| row.len() == integer(&o["shape"][1]))
        );
        axes.compare(&name, &got.freqs, &array(&o["freqs"]), 1e-12, 0.0);
        axes.compare(&name, &got.times, &array(&o["times"]), 1e-12, 0.0);
        errors.compare(
            &name,
            &got.power.into_iter().flatten().collect::<Vec<_>>(),
            &array(&o["power"]),
            1e-14,
            1e-9,
        );
    }
    errors.report("spectrogram_power");
    axes.report("spectrogram_freqs_times");
}
