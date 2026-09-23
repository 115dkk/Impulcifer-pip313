#![forbid(unsafe_code)]
//! P24: the interactive-report numbers against the 2.x oracle
//! (`tests/migration/export_goldens_interactive.py`, `p24_interactive_*.json`).
//! `--nocapture` prints the measured maxima.
//!
//! Budgets. Integer and structural results (octave band edges, FFT length, lag
//! grids, NaN placement, labels, chart order) must match exactly. Float results
//! differ only by summation order and FFT rounding (pocketfft vs rustfft, NumPy
//! pairwise sums vs sequential ones, SciPy's FFT correlation vs the direct lag
//! sum): ILD and EDC 1e-9 dB, IPD 1e-9 degrees on the circle (a cross spectrum
//! on the negative real axis may land on either side of the ±180° cut, as it
//! does between NumPy builds), normalized IACF/IACC 1e-12. The result overview
//! uses the P09 same-input FR budget `1e-9 + 1e-11 * |reference|` dB.
use impulcifer_analysis::model::{
    self, DEFAULT_OCTAVE_CENTERS, EDC_RANGE_DB, Ear, IACC_MAX_DELAY_MS, Panel,
};
use impulcifer_dsp::{
    fft,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/migration/goldens")
        .join(format!("p24_interactive_{name}.json"));
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() < 200_000, "{} is too large", path.display());
    serde_json::from_slice(&bytes).unwrap()
}
fn number(v: &Value) -> f64 {
    match v.as_str() {
        Some("nan") => f64::NAN,
        Some("-inf") => f64::NEG_INFINITY,
        Some("+inf") => f64::INFINITY,
        _ => v.as_f64().unwrap(),
    }
}
fn array(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(number).collect()
}
fn bands(v: &Value) -> Vec<(f64, f64)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| (number(&b[0]), number(&b[1])))
        .collect()
}

/// The stored arrays plus the derived inputs rebuilt from their recipes.
fn signals() -> BTreeMap<String, Vec<f64>> {
    let g = golden("signals");
    let mut out: BTreeMap<String, Vec<f64>> = g["signals"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), array(v)))
        .collect();
    let derived = g["derived"].as_object().unwrap();
    // Impulses first (no dependencies), then shifted/scaled copies.
    for (name, recipe) in derived {
        if let Some(length) = recipe.get("length") {
            let mut x = vec![0.0; length.as_u64().unwrap() as usize];
            x[recipe["index"].as_u64().unwrap() as usize] = number(&recipe["value"]);
            out.insert(name.clone(), x);
        }
    }
    for (name, recipe) in derived {
        if let Some(source) = recipe.get("source") {
            let source = &out[source.as_str().unwrap()];
            let shift = recipe["shift"].as_u64().unwrap() as usize;
            let gain = number(&recipe["gain"]);
            let mut x = vec![0.0; source.len()];
            for i in shift..x.len() {
                x[i] = source[i - shift] * gain;
            }
            out.insert(name.clone(), x);
        }
    }
    out
}

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

/// NaN in the same places, finite values within `budget`.
fn assert_close(label: &str, actual: &[f64], expected: &[f64], budget: f64) -> f64 {
    assert_eq!(actual.len(), expected.len(), "{label}: length");
    let mut max = 0.0_f64;
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(a.is_nan(), e.is_nan(), "{label}[{i}]: {a} vs {e}");
        if !e.is_nan() {
            let d = (a - e).abs();
            assert!(
                d <= budget,
                "{label}[{i}]: {a} vs {e} (|d| = {d:e} > {budget:e})"
            );
            max = max.max(d);
        }
    }
    max
}

/// Angular distance in degrees, NaN positions must agree.
fn assert_close_degrees(label: &str, actual: &[f64], expected: &[f64], budget: f64) -> f64 {
    assert_eq!(actual.len(), expected.len(), "{label}: length");
    let mut max = 0.0_f64;
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(a.is_nan(), e.is_nan(), "{label}[{i}]: {a} vs {e}");
        if !e.is_nan() {
            assert!(
                a.abs() <= 180.0,
                "{label}[{i}]: {a} is not a principal angle"
            );
            let d = (a - e).rem_euclid(360.0);
            let d = d.min(360.0 - d);
            assert!(
                d <= budget,
                "{label}[{i}]: {a} vs {e} ({d:e} deg > {budget:e})"
            );
            max = max.max(d);
        }
    }
    max
}

#[test]
fn golden_octave_bands_match_python() {
    for case in golden("analysis")["octave_bands"].as_array().unwrap() {
        let fs = number(&case["fs"]);
        let actual = model::octave_bands(fs, &array(&case["centers"]));
        let expected = bands(&case["bands"]);
        assert_eq!(actual.len(), expected.len(), "fs {fs}");
        for (a, e) in actual.iter().zip(&expected) {
            // Same IEEE operations (one division or multiplication by sqrt 2, one min).
            assert_eq!(a.0.to_bits(), e.0.to_bits(), "fs {fs} lower");
            assert_eq!(a.1.to_bits(), e.1.to_bits(), "fs {fs} upper");
        }
    }
    assert_eq!(
        model::octave_bands(48000.0, &DEFAULT_OCTAVE_CENTERS),
        model::octave_bands(48000.0, &DEFAULT_OCTAVE_CENTERS[..])
    );
}

#[test]
fn golden_band_ild_and_ipd_match_python() {
    let s = signals();
    let (mut ild_max, mut ipd_max) = (0.0_f64, 0.0_f64);
    for case in golden("analysis")["pairs"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let left = &s[case["left"].as_str().unwrap()];
        let right = &s[case["right"].as_str().unwrap()];
        let fs = number(&case["fs"]);
        let bands = bands(&case["bands"]);
        assert_eq!(
            fft::next_fast_len_complex(left.len().max(right.len())) as u64,
            case["fft_len"].as_u64().unwrap(),
            "{name}: fft length"
        );
        let ild = model::band_interaural_level_difference(left, right, fs, &bands);
        let ipd = model::band_interaural_phase_difference(left, right, fs, &bands);
        ild_max = ild_max.max(assert_close(
            &format!("{name} ild"),
            &ild,
            &array(&case["ild"]),
            1e-9,
        ));
        ipd_max = ipd_max.max(assert_close_degrees(
            &format!("{name} ipd"),
            &ipd,
            &array(&case["ipd"]),
            1e-9,
        ));
    }
    println!("ILD max |d| = {ild_max:e} dB; IPD max |d| = {ipd_max:e} deg");
}

#[test]
fn golden_iacc_matches_python() {
    let s = signals();
    let mut worst = 0.0_f64;
    for case in golden("analysis")["pairs"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let left = &s[case["left"].as_str().unwrap()];
        let right = &s[case["right"].as_str().unwrap()];
        let fs = number(&case["fs"]);
        for key in ["iacc", "iacc_wide"] {
            let expected = &case[key];
            let r = model::interaural_cross_correlation(
                left,
                right,
                fs,
                number(&expected["max_delay_ms"]),
            );
            let label = format!("{name} {key}");
            // Lags are integer * 1000 / fs on both sides: exact.
            assert_eq!(r.lags_ms, array(&expected["lags_ms"]), "{label} lags");
            worst = worst.max(assert_close(
                &label,
                &r.iacf,
                &array(&expected["iacf"]),
                1e-12,
            ));
            let (iacc, tau) = (number(&expected["iacc"]), number(&expected["tau_ms"]));
            assert_eq!(r.iacc.is_nan(), iacc.is_nan(), "{label} iacc");
            if !iacc.is_nan() {
                assert!(
                    (r.iacc - iacc).abs() <= 1e-12,
                    "{label}: {} vs {iacc}",
                    r.iacc
                );
                assert_eq!(r.tau_ms, tau, "{label} tau");
            } else {
                assert!(r.tau_ms.is_nan());
            }
        }
    }
    println!("IACF max |d| = {worst:e}");
}

#[test]
fn golden_energy_decay_curves_match_python() {
    let s = signals();
    let mut worst = 0.0_f64;
    for case in golden("analysis")["edc"].as_array().unwrap() {
        let name = case["signal"].as_str().unwrap();
        let data = if name == "empty" {
            Vec::new()
        } else {
            s[name].clone()
        };
        let floor = number(&case["floor_db"]);
        let actual = model::energy_decay_curve_db(&data, floor);
        worst = worst.max(assert_close(
            &format!("{name} edc {floor}"),
            &actual,
            &array(&case["edc"]),
            1e-9,
        ));
        for w in actual.windows(2) {
            assert!(w[1] <= w[0] + 1e-9, "{name}: EDC must not increase");
        }
    }
    println!("EDC max |d| = {worst:e} dB");
}

fn ir(data: &[f64], fs: u32) -> Option<ImpulseResponse> {
    Some(ImpulseResponse {
        data: data.to_vec(),
        fs,
        recording: None,
    })
}

fn layout_hrir() -> (Hrir, Value) {
    let s = signals();
    let g = golden("layouts");
    let fs = g["fs"].as_u64().unwrap() as u32;
    let speakers = g["speakers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| SpeakerIrs {
            speaker: row[0].as_str().unwrap().into(),
            left: ir(&s[row[1].as_str().unwrap()], fs),
            right: ir(&s[row[2].as_str().unwrap()], fs),
        })
        .collect();
    (Hrir { fs, speakers }, g)
}

fn speaker_of(title: &str) -> &str {
    title.rsplit(" - ").next().unwrap()
}

#[test]
fn golden_report_assembly_matches_bokeh_generators() {
    let (hrir, g) = layout_hrir();
    let report = model::build_report(&hrir);
    // The synthetic speakers have different lengths, so 2.x `np.vstack` raises in
    // the result overview (logged as a panel warning) while the others render.
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].panel, Panel::ResultOverview);
    assert!(report.overview.is_none());

    let overlay = g["overlay"].as_array().unwrap();
    assert_eq!(report.overlay.len(), overlay.len());
    for (chart, expected) in report.overlay.iter().zip(overlay) {
        assert_eq!(
            chart.speaker,
            speaker_of(expected["title"].as_str().unwrap())
        );
        for (data, series) in [chart.left, chart.right]
            .iter()
            .zip(expected["series"].as_array().unwrap())
        {
            assert_eq!(data.len() as u64, series["length"].as_u64().unwrap());
            // (i - origin) / fs * 1000: identical IEEE operations.
            assert_eq!(chart.time_ms(0), number(&series["time_first"]));
            assert_eq!(chart.time_ms(1), number(&series["time_second"]));
            assert_eq!(chart.time_ms(data.len() - 1), number(&series["time_last"]));
        }
        assert_eq!(
            array(&expected["x_range"]),
            [model::OVERLAY_RANGE_MS.0, model::OVERLAY_RANGE_MS.1]
        );
    }

    for (charts, key) in [(&report.ild, "ild"), (&report.ipd, "ipd")] {
        let expected = g[key].as_array().unwrap();
        assert_eq!(charts.len(), expected.len(), "{key} charts");
        for (chart, e) in charts.iter().zip(expected) {
            assert_eq!(chart.speaker, speaker_of(e["title"].as_str().unwrap()));
            let labels: Vec<_> = e["bands"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(chart.labels, labels, "{key} {} labels", chart.speaker);
            if key == "ild" {
                assert_close(
                    &format!("ild {}", chart.speaker),
                    &chart.values,
                    &array(&e["values"]),
                    1e-9,
                );
            } else {
                assert_close_degrees(
                    &format!("ipd {}", chart.speaker),
                    &chart.values,
                    &array(&e["values"]),
                    1e-9,
                );
            }
        }
    }

    let iacc = g["iacc"].as_array().unwrap();
    assert_eq!(
        report.iacc.len(),
        iacc.len(),
        "silent speakers have no IACC chart"
    );
    for (chart, e) in report.iacc.iter().zip(iacc) {
        assert_eq!(chart.speaker, speaker_of(e["title"].as_str().unwrap()));
        assert_eq!(chart.max_delay_ms, IACC_MAX_DELAY_MS);
        assert_eq!(
            vec![chart.legend()],
            e["legend"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(chart.result.lags_ms, array(&e["lags_ms"]));
        assert_close("layout iacf", &chart.result.iacf, &array(&e["iacf"]), 1e-12);
    }

    let edc = g["edc"].as_array().unwrap();
    assert_eq!(report.edc.len(), edc.len());
    for (chart, e) in report.edc.iter().zip(edc) {
        assert_eq!(chart.speaker, speaker_of(e["title"].as_str().unwrap()));
        let legend: Vec<_> = chart.curves.iter().map(|(ear, _)| ear.label()).collect();
        assert_eq!(
            legend,
            e["legend"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        for ((_, curve), series) in chart.curves.iter().zip(e["series"].as_array().unwrap()) {
            let n = curve.len();
            assert_eq!(n as u64, series["length"].as_u64().unwrap());
            assert_close("edc first", &curve[..4], &array(&series["first"]), 1e-9);
            assert_close("edc last", &curve[n - 4..], &array(&series["last"]), 1e-9);
            // np.arange(n) * 1000 / fs
            assert_eq!(
                ((n - 1) * 1000) as f64 / f64::from(chart.fs),
                number(&series["time_last"])
            );
        }
        assert_eq!(array(&e["y_range"]), [EDC_RANGE_DB.0, EDC_RANGE_DB.1]);
    }
    assert!(
        Panel::ALL
            .iter()
            .all(|p| report.has(*p) == (*p != Panel::ResultOverview))
    );
}

/// Chart titles and axis labels in the HTML manifest are the 2.x Bokeh ones.
/// Deliberate difference: ILD/IPD bars are labelled by octave centre, so their
/// x-axis reads "Octave band centre (Hz)" instead of "Frequency Band" (the 2.x
/// band strings stay in the hover readout).
#[test]
fn golden_report_text_matches_bokeh_labels() {
    let (hrir, g) = layout_hrir();
    let html = impulcifer_analysis::html::summary_html(&model::build_report(&hrir)).unwrap();
    let manifest = manifest_of(&html);
    for (id, key) in [
        ("interaural_overlay", "overlay"),
        ("ild", "ild"),
        ("ipd", "ipd"),
        ("iacc", "iacc"),
        ("etc", "edc"),
    ] {
        let tab = manifest["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap();
        let charts = tab["charts"].as_array().unwrap();
        let expected = g[key].as_array().unwrap();
        assert_eq!(charts.len(), expected.len(), "{id}");
        for (chart, e) in charts.iter().zip(expected) {
            assert_eq!(chart["title"], e["title"], "{id} title");
            assert_eq!(chart["y"]["label"], e["y_label"], "{id} y label");
            if !matches!(id, "ild" | "ipd") {
                assert_eq!(chart["x"]["label"], e["x_label"], "{id} x label");
                let names: Vec<_> = chart["series"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["name"].clone())
                    .collect();
                assert_eq!(Value::from(names), e["legend"], "{id} legend");
            }
            if let Some(range) = e.get("x_range") {
                assert_eq!(array(&chart["x"]["view"]), array(range), "{id} x range");
            }
            if let Some(range) = e.get("y_range") {
                assert_eq!(array(&chart["y"]["view"]), array(range), "{id} y range");
            }
        }
    }
    // The layout HRIR has unequal lengths, so its summary has no overview tab;
    // the overview's text is checked on the overview fixture instead.
    assert!(
        manifest["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["id"] != "result_overview")
    );
    let g = golden("overview_48000");
    let html =
        impulcifer_analysis::html::summary_html(&model::build_report(&overview_hrir(&g))).unwrap();
    let manifest = manifest_of(&html);
    let tab = manifest["tabs"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(tab["id"], "result_overview");
    assert_eq!(tab["charts"][0]["title"], g["title"]);
    assert_eq!(array(&tab["charts"][0]["x"]["view"]), array(&g["x_range"]));
    let names: Vec<_> = tab["charts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| {
            c["series"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["name"].clone())
        })
        .collect();
    let labels: Vec<_> = g["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["label"].clone())
        .collect();
    assert_eq!(names, labels);
}

fn manifest_of(html: &str) -> Value {
    let open = "<script type=\"application/json\" id=\"report-manifest\">";
    let start = html.find(open).unwrap() + open.len();
    let end = start + html[start..].find("</script>").unwrap();
    serde_json::from_str(&html[start..end]).unwrap()
}

fn overview_hrir(g: &Value) -> Hrir {
    let fs = g["fs"].as_u64().unwrap() as u32;
    let mut hrir = Hrir {
        fs,
        speakers: Vec::new(),
    };
    for input in g["inputs"].as_array().unwrap() {
        let speaker = input["speaker"].as_str().unwrap();
        if hrir.get(speaker).is_none() {
            hrir.speakers.push(SpeakerIrs {
                speaker: speaker.into(),
                left: None,
                right: None,
            });
        }
        let data = ir(&array(&input["data"]), fs);
        let pair = hrir.get_mut(speaker).unwrap();
        if input["side"] == "left" {
            pair.left = data;
        } else {
            pair.right = data;
        }
    }
    hrir
}

#[test]
fn golden_result_overview_matches_python() {
    for fs in [44100_u32, 48000] {
        let g = golden(&format!("overview_{fs}"));
        assert_eq!(
            model::overview_treble_f_upper(fs),
            number(&g["treble_f_upper"])
        );
        let overview = model::result_overview(&overview_hrir(&g)).unwrap().unwrap();
        assert_eq!(overview.frequency, array(&g["frequency"]), "{fs}: grid");
        let lines: BTreeMap<_, _> = g["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| (l["label"].as_str().unwrap().to_owned(), array(&l["values"])))
            .collect();
        let mut worst = 0.0_f64;
        for (label, actual) in [
            ("Left Raw", &overview.left_raw),
            ("Right Raw", &overview.right_raw),
            ("Left Smoothed", &overview.left_smoothed),
            ("Right Smoothed", &overview.right_smoothed),
            ("Difference (L-R)", &overview.difference),
        ] {
            let expected = &lines[label];
            assert_eq!(actual.len(), expected.len());
            for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
                let budget = 1e-9 + 1e-11 * e.abs();
                assert!((a - e).abs() <= budget, "{fs} {label}[{i}]: {a} vs {e}");
            }
            worst = worst.max(max_abs(actual, expected));
        }
        println!("overview {fs} Hz max |d| = {worst:e} dB");
    }
}

#[test]
fn degenerate_reports_follow_the_2x_generators() {
    let fs = 48000;
    // One ear only: no overlay/ILD/IPD/IACC, an EDC with one curve, no overview.
    let lonely = Hrir {
        fs,
        speakers: vec![SpeakerIrs {
            speaker: "FC".into(),
            left: ir(&[0.0, 1.0, 0.5, 0.25], fs),
            right: None,
        }],
    };
    let report = model::build_report(&lonely);
    assert!(report.overlay.is_empty() && report.ild.is_empty() && report.iacc.is_empty());
    assert_eq!(report.edc.len(), 1);
    assert_eq!(report.edc[0].curves.len(), 1);
    assert_eq!(report.edc[0].curves[0].0, Ear::Left);
    assert!(report.overview.is_none() && report.errors.is_empty());
    assert!(!report.is_empty());

    // Empty responses count as missing (2.x `not ir` is len() == 0).
    let empty = Hrir {
        fs,
        speakers: vec![SpeakerIrs {
            speaker: "FL".into(),
            left: ir(&[], fs),
            right: ir(&[], fs),
        }],
    };
    assert!(model::build_report(&empty).is_empty());
    assert!(
        model::build_report(&Hrir {
            fs,
            speakers: Vec::new()
        })
        .is_empty()
    );

    // Unequal lengths: np.vstack raises in 2.x; only the overview is lost.
    let unequal = Hrir {
        fs,
        speakers: vec![
            SpeakerIrs {
                speaker: "FL".into(),
                left: ir(&[1.0, 0.5, 0.0], fs),
                right: ir(&[0.5, 1.0, 0.0], fs),
            },
            SpeakerIrs {
                speaker: "FR".into(),
                left: ir(&[1.0, 0.5], fs),
                right: ir(&[0.5, 1.0], fs),
            },
        ],
    };
    let report = model::build_report(&unequal);
    assert!(report.overview.is_none());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].panel, Panel::ResultOverview);
    assert_eq!(report.overlay.len(), 2);

    // A one-sample sum has no overview (2.x `len(summed) <= 1`).
    let single = Hrir {
        fs,
        speakers: vec![SpeakerIrs {
            speaker: "FL".into(),
            left: ir(&[1.0], fs),
            right: ir(&[0.5], fs),
        }],
    };
    let report = model::build_report(&single);
    assert!(report.overview.is_none() && report.errors.is_empty());
    assert!(report.ild.is_empty(), "no FFT bin falls in an octave band");
    assert_eq!(report.iacc.len(), 1);
}
