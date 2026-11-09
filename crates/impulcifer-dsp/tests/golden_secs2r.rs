#![forbid(unsafe_code)]
//! Clean-room gates for the SECS 2R (`s2r_*`) fixtures from ADR 0015 bundle 1.
mod golden_secs2_helpers;
use golden_secs2_helpers::{
    Errors as NumericErrors, array, bands, complex, fixture, fixture_r, fixtures_r, number, sha256,
};
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft::Complex64,
    hrir::Hrir,
    ir::ImpulseResponse,
    stages::{room, room_tuning2::*},
};
use impulcifer_io::read_wav;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::OnceLock,
};

const FS: f64 = 48_000.0;
const N: usize = 131_072;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pairs(value: &Value) -> Vec<(f64, f64)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| (number(&pair[0]), number(&pair[1])))
        .collect()
}

fn flatten_bands(values: &[(f64, f64, f64)]) -> Vec<f64> {
    values
        .iter()
        .flat_map(|band| [band.0, band.1, band.2])
        .collect()
}

fn flatten_pairs(values: &[(f64, f64)]) -> Vec<f64> {
    values.iter().flat_map(|pair| [pair.0, pair.1]).collect()
}

fn sha_f64(values: &[f64]) -> String {
    sha256(
        &values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

fn compare_input_sha(name: &str, got: &str, expected: &str) {
    if oracle_libm_r() {
        assert_eq!(got, expected, "{name}");
    } else {
        println!(
            "REPORT {name}: exact={} got={got} expected={expected}",
            got == expected
        );
    }
}

fn bins() -> &'static Vec<usize> {
    static BINS: OnceLock<Vec<usize>> = OnceLock::new();
    BINS.get_or_init(|| {
        fixture_r("bins")["outputs"]["bins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect()
    })
}

fn sampled(values: &[f64]) -> Vec<f64> {
    bins().iter().map(|index| values[*index]).collect()
}

fn sampled_complex(values: &[Complex64]) -> Vec<Complex64> {
    bins().iter().map(|index| values[*index]).collect()
}

fn grid() -> Vec<f64> {
    primitives::grid(N, FS)
}

fn oracle_libm_r() -> bool {
    static SAME: OnceLock<bool> = OnceLock::new();
    *SAME.get_or_init(|| {
        let fixture = fixture_r("libm_probe");
        let x = array(&fixture["inputs"]["x"]);
        let checks = [
            ("ln", x.iter().map(|value| value.ln()).collect::<Vec<_>>()),
            (
                "exp_scaled",
                x.iter().map(|value| (value / 1000.0).exp()).collect(),
            ),
            ("log10", x.iter().map(|value| value.log10()).collect()),
            ("log2", x.iter().map(|value| value.log2()).collect()),
            ("sqrt", x.iter().map(|value| value.sqrt()).collect()),
            ("pow_half", x.iter().map(|value| value.powf(0.5)).collect()),
            ("sin", x.iter().map(|value| value.sin()).collect()),
            ("cos", x.iter().map(|value| value.cos()).collect()),
        ];
        let mut same = true;
        for (name, got) in checks {
            let expected = array(&fixture["outputs"][name]);
            let equal = got
                .iter()
                .zip(&expected)
                .all(|(got, expected)| got.to_bits() == expected.to_bits());
            println!("LIBM {name}: exact={equal}");
            same &= equal;
        }
        println!("MEASURE s2r_libm_probe: oracle_libm={same}");
        same
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comparison {
    /// Stored-input, decision-free values are exact on the oracle libm and 1e-9 elsewhere.
    Exact,
    /// Stored-input curves use 1e-12 on the oracle libm and 1e-9 elsewhere.
    Curve,
    /// Values rebuilt through Rust's FFT use 1e-9 on every platform.
    Rebuilt,
    /// Values downstream of PEQ refinement are report-only on every platform.
    FitDependent,
}

#[derive(Default)]
struct Errors {
    absolute: f64,
    relative: f64,
    values: usize,
    failures: Vec<String>,
}

impl Errors {
    fn compare(&mut self, name: &str, got: &[f64], expected: &[f64], class: Comparison) {
        assert_ne!(
            class,
            Comparison::FitDependent,
            "{name}: wrong comparison helper"
        );
        assert_eq!(got.len(), expected.len(), "{name}: shape");
        let oracle_libm = oracle_libm_r();
        let exact = oracle_libm && class == Comparison::Exact;
        let budget = match class {
            Comparison::Exact | Comparison::Curve if oracle_libm => 1e-12,
            Comparison::Exact | Comparison::Curve | Comparison::Rebuilt => 1e-9,
            Comparison::FitDependent => unreachable!(),
        };
        let mut error_energy = 0.0;
        let mut expected_energy = 0.0;
        let mut first = None;
        for (index, (got, expected)) in got.iter().zip(expected).enumerate() {
            self.values += 1;
            let error = (got - expected).abs();
            self.absolute = self.absolute.max(error);
            error_energy += error * error;
            expected_energy += expected * expected;
            let failed = if exact {
                got.to_bits() != expected.to_bits()
            } else {
                !got.is_finite() || error > budget + budget * expected.abs()
            };
            if failed && first.is_none() {
                first = Some(format!(
                    "{name}[{index}] ({class:?}): got={got:.17e} expected={expected:.17e} error={error:.17e}"
                ));
            }
        }
        let relative = if expected_energy > 0.0 {
            (error_energy / expected_energy).sqrt()
        } else {
            error_energy.sqrt()
        };
        self.relative = self.relative.max(relative);
        if !exact && relative > budget && first.is_none() {
            first = Some(format!(
                "{name} ({class:?}): relative L2 {relative:e} > {budget:e}"
            ));
        }
        if let Some(failure) = first {
            self.failures.push(failure);
        }
    }

    fn compare_rebuilt_values_only(&mut self, name: &str, got: &[f64], expected: &[f64]) {
        assert_eq!(got.len(), expected.len(), "{name}: shape");
        let budget = 1e-9;
        let mut first = None;
        for (index, (got, expected)) in got.iter().zip(expected).enumerate() {
            self.values += 1;
            let error = (got - expected).abs();
            self.absolute = self.absolute.max(error);
            if (!got.is_finite() || error > budget + budget * expected.abs()) && first.is_none() {
                first = Some(format!(
                    "{name}[{index}] (Rebuilt): got={got:.17e} expected={expected:.17e} error={error:.17e}"
                ));
            }
        }
        if let Some(failure) = first {
            self.failures.push(failure);
        }
    }

    fn finish(self, family: &str) {
        println!(
            "MEASURE {family}: max_abs={:.17e}, max_relative_l2={:.17e}, values={}, budgets=class-specific",
            self.absolute, self.relative, self.values
        );
        assert!(self.failures.is_empty(), "{}", self.failures.join("\n"));
    }
}

fn compare_decision<T: std::fmt::Debug + PartialEq>(
    name: &str,
    got: &T,
    expected: &T,
    class: Comparison,
) {
    match class {
        Comparison::Exact if oracle_libm_r() => assert_eq!(got, expected, "{name} ({class:?})"),
        Comparison::Exact | Comparison::FitDependent => println!(
            "REPORT {name} ({class:?}): match={} got={got:?} expected={expected:?}",
            got == expected
        ),
        Comparison::Curve | Comparison::Rebuilt => {
            panic!("{name}: wrong decision comparison class {class:?}")
        }
    }
}

fn compare_refinement(
    name: &str,
    got: &ResonanceRefinement,
    expected: &Value,
    class: Comparison,
    errors: &mut Errors,
) {
    assert_eq!(got.low_index, expected["lo"].as_u64().unwrap() as usize);
    assert_eq!(got.high_index, expected["hi"].as_u64().unwrap() as usize);
    assert_eq!(
        got.peak_index,
        expected["peak"].as_u64().map(|value| value as usize),
        "{name}/peak"
    );
    assert_eq!(
        got.rejection,
        expected["rejection"].as_str(),
        "{name}/rejection"
    );
    for (label, actual) in [
        ("left", got.left_db),
        ("middle", got.middle_db),
        ("right", got.right_db),
        ("curvature", got.curvature),
        ("offset", got.offset),
    ] {
        if expected[label].is_null() {
            assert!(actual.is_none(), "{name}/{label}");
        } else {
            let actual = [actual.unwrap()];
            let expected = [number(&expected[label])];
            if class == Comparison::Rebuilt && matches!(label, "curvature" | "offset") {
                errors.compare_rebuilt_values_only(&format!("{name}/{label}"), &actual, &expected);
            } else {
                errors.compare(&format!("{name}/{label}"), &actual, &expected, class);
            }
        }
    }
}

fn compare_realize_call(
    name: &str,
    got: &RealizeDiagnostics,
    expected: &Value,
    errors: &mut Errors,
) {
    for (label, actual) in [
        ("applied_before", got.applied_before.as_slice()),
        ("applied_after", got.applied_after.as_slice()),
    ] {
        let want = array(&expected[label]);
        let actual = if want.len() == actual.len() {
            actual.to_vec()
        } else {
            sampled(actual)
        };
        errors.compare(
            &format!("{name}/{label}"),
            &actual,
            &want,
            Comparison::Exact,
        );
    }
    for (label, actual) in [
        ("total", got.total),
        ("rho", got.rho),
        ("wrapped", got.wrapped),
        ("correction", got.correction),
        ("rotation", got.rotation),
    ] {
        if let Some(want) = expected[label].as_f64() {
            errors.compare(
                &format!("{name}/{label}"),
                &[actual.unwrap()],
                &[want],
                Comparison::Exact,
            );
        } else {
            assert!(actual.is_none(), "{name}/{label}");
        }
    }
    if let Some(want) = expected["limited"].as_bool() {
        compare_decision(
            &format!("{name}/limited"),
            &got.limited,
            &want,
            Comparison::Exact,
        );
    }
    errors.compare(
        &format!("{name}/h"),
        &got.fir,
        &array(&expected["h"]),
        Comparison::Curve,
    );
    errors.compare(
        &format!("{name}/rms_max"),
        &[got.rms, got.max],
        &[number(&expected["rms"]), number(&expected["max"])],
        Comparison::Curve,
    );
}

#[test]
fn room_tuning2_s2r_libm_probe() {
    let same = oracle_libm_r();
    println!("SECS 2R oracle libm match: {same}");
}

#[test]
fn room_tuning2_s2r_lf_weight_and_pull() {
    let mut errors = Errors::default();
    let fixture = fixture_r("lf_weight");
    let frequencies = array(&fixture["inputs"]["f"]);
    let expected = fixture["outputs"]["w"].as_array().unwrap();
    for (index, f_lin) in fixture["inputs"]["f_lin"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        errors.compare(
            &format!("lf_weight/{index}"),
            &lf_alignment_weight(&frequencies, number(f_lin)),
            &array(&expected[index]),
            Comparison::Exact,
        );
    }
    let fixture = fixture_r("lf_pull");
    let got: Vec<_> = fixture["inputs"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let row = case.as_array().unwrap();
            lf_pull_bound(
                number(&row[0]),
                number(&row[1]),
                number(&row[2]),
                number(&row[3]),
                number(&row[4]),
            )
        })
        .collect();
    errors.compare(
        "lf_pull",
        &got,
        &array(&fixture["outputs"]["value"]),
        Comparison::Exact,
    );
    errors.finish("s2r_lf_weight_pull");
}

#[test]
fn room_tuning2_s2r_decay_cut_and_hf_region() {
    let mut errors = Errors::default();
    let fixture = fixture_r("decay_cut");
    let expected = fixture["outputs"]["cut"].as_array().unwrap();
    for (index, case) in fixture["inputs"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let row = case.as_array().unwrap();
        let got = mode_decay_cut(
            number(&row[0]),
            number(&row[1]),
            number(&row[2]),
            number(&row[3]),
            number(&row[4]),
        );
        if expected[index].is_null() {
            assert!(got.is_none(), "decay_cut/{index}");
        } else {
            let got = got.unwrap();
            errors.compare(
                &format!("decay_cut/{index}"),
                &[got.frequency_hz, got.q, -got.depth_db],
                &array(&expected[index]),
                Comparison::Exact,
            );
        }
    }
    let frequencies = grid();
    for (name, fixture) in fixtures_r("hf_region_") {
        let extras = pairs(&fixture["inputs"]["rc_bands"]);
        let region = hf_phase_region(
            &frequencies,
            FS,
            number(&fixture["inputs"]["rc_hi"]),
            &merge_band_set((20.0, number(&fixture["inputs"]["rc_hi"])), &extras),
        );
        if fixture["outputs"]["inert"].as_bool().unwrap() {
            assert!(region.is_none(), "{name}");
        } else {
            let region = region.unwrap();
            errors.compare(
                &format!("{name}/scalars"),
                &[
                    region.low_hz,
                    region.high_hz,
                    region.log_width,
                    region.weight_sum,
                ],
                &[
                    number(&fixture["outputs"]["lo"]),
                    number(&fixture["outputs"]["hi"]),
                    number(&fixture["outputs"]["log_width"]),
                    number(&fixture["outputs"]["weight_sum"]),
                ],
                Comparison::Exact,
            );
            errors.compare(
                &format!("{name}/weight"),
                &sampled(&region.weight),
                &array(&fixture["outputs"]["weight"]),
                Comparison::Exact,
            );
        }
    }
    errors.finish("s2r_decay_cut_hf_region");
}

#[test]
fn room_tuning2_s2r_mode_t60_and_refinement() {
    let mut errors = Errors::default();
    let fixture = fixture_r("mode_t60_48k_synthetic");
    let impulse = array(&fixture["inputs"]["x"]);
    let centers = array(&fixture["inputs"]["fc"]);
    let half = array(&fixture["inputs"]["half"]);
    let got: Vec<_> = centers
        .iter()
        .zip(half)
        .map(|(center, half)| primitives::mode_decay_at_rate(&impulse, *center, half, FS).unwrap())
        .collect();
    errors.compare(
        "mode_t60_48k_synthetic",
        &got,
        &array(&fixture["outputs"]["t60"]),
        Comparison::Curve,
    );
    for (name, fixture) in fixtures_r("mode_t60_48k_") {
        if name.ends_with("synthetic") {
            continue;
        }
        let speaker = fixture["meta"]["case"]
            .as_str()
            .unwrap()
            .to_ascii_uppercase();
        let spectrum = point_spectrum(&points(&speaker)[0]);
        let impulse = impulcifer_dsp::fft::irfft(&spectrum, N);
        let centers = array(&fixture["inputs"]["fc"]);
        let half = array(&fixture["inputs"]["half"]);
        let got: Vec<_> = centers
            .iter()
            .zip(half)
            .map(|(center, half)| {
                primitives::mode_decay_at_rate(&impulse, *center, half, FS).unwrap()
            })
            .collect();
        errors.compare(
            &name,
            &got,
            &array(&fixture["outputs"]["t60"]),
            Comparison::Rebuilt,
        );
    }

    let fixture = fixture_r("refine_resonance");
    let response = complex(&fixture["inputs"]["H"]);
    let frequencies = grid();
    for (index, candidate) in array(&fixture["inputs"]["p"]).iter().enumerate() {
        let got = refine_resonance(&response, &frequencies, *candidate);
        let expected = &fixture["outputs"]["results"][index];
        compare_refinement(
            &format!("refine_resonance/{index}"),
            &got,
            expected,
            Comparison::Exact,
            &mut errors,
        );
        if let Some(mode) = got.mode {
            errors.compare(
                &format!("refine/{index}/mode"),
                &[mode.0],
                &[number(&expected["output"][0])],
                Comparison::Exact,
            );
            errors.compare(
                &format!("refine/{index}/t60"),
                &[mode.1],
                &[number(&expected["output"][1])],
                Comparison::Curve,
            );
        } else {
            assert!(expected["output"].is_null(), "refine/{index}");
        }
    }
    for (name, fixture) in fixtures_r("refine_") {
        if name == "refine_resonance" {
            continue;
        }
        let speaker = fixture["meta"]["case"]
            .as_str()
            .unwrap()
            .to_ascii_uppercase();
        let response = point_spectrum(&points(&speaker)[0]);
        for (index, candidate) in array(&fixture["inputs"]["candidates"]).iter().enumerate() {
            let got = refine_resonance(&response, &frequencies, *candidate);
            let expected = &fixture["outputs"]["results"][index];
            compare_refinement(
                &format!("{name}/{index}"),
                &got,
                expected,
                Comparison::Rebuilt,
                &mut errors,
            );
            if let Some(mode) = got.mode {
                errors.compare(
                    &format!("{name}/{index}/mode"),
                    &[mode.0, mode.1],
                    &array(&expected["output"]),
                    Comparison::Rebuilt,
                );
            } else {
                assert!(expected["output"].is_null(), "{name}/{index}");
            }
        }
    }
    errors.finish("s2r_mode_t60_refine");
}

#[test]
fn room_tuning2_s2r_band_set_and_masks() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("band_set_") {
        let input = &fixture["inputs"];
        let got = merge_band_set(
            (number(&input["lo"]), number(&input["hi"])),
            &pairs(&input["extra"]),
        );
        let expected = pairs(&fixture["outputs"]["bands"]);
        errors.compare(
            &name,
            &flatten_pairs(&got),
            &flatten_pairs(&expected),
            Comparison::Exact,
        );
    }

    let frequencies = grid();
    for (name, fixture) in fixtures_r("masks_") {
        let input = &fixture["inputs"];
        let primary = (number(&input["rc_lo"]), number(&input["rc_hi"]));
        let extras = pairs(&input["rc_bands"]);
        let merged = merge_band_set(primary, &extras);
        errors.compare(
            &format!("{name}/bands"),
            &flatten_pairs(&merged),
            &flatten_pairs(&pairs(&fixture["outputs"]["bands"])),
            Comparison::Exact,
        );
        errors.compare(
            &format!("{name}/magnitude_mask"),
            &sampled(&magnitude_mask(&frequencies, &merged, FS)),
            &array(&fixture["outputs"]["magnitude_mask"]),
            Comparison::Curve,
        );
        errors.compare(
            &format!("{name}/phase_fade"),
            &sampled(&phase_fade(
                &frequencies,
                primary.1,
                &merged,
                !extras.is_empty(),
            )),
            &array(&fixture["outputs"]["phase_fade"]),
            Comparison::Curve,
        );
        errors.compare(
            &format!("{name}/phase_error_band"),
            &sampled(&phase_error_band(
                &frequencies,
                primary.1,
                &extras,
                number(&input["lo_fir"]),
                number(&input["floor"]),
                FS,
            )),
            &array(&fixture["outputs"]["phase_error_band"]),
            Comparison::Exact,
        );
        errors.compare(
            &format!("{name}/band_grid"),
            &band_grid(&merged),
            &array(&fixture["outputs"]["band_grid"]),
            Comparison::Curve,
        );
        let reach = fit_reach(primary.0, primary.1, primary.1, &extras);
        let expected = &fixture["outputs"]["fit_reach"];
        if expected.is_null() {
            assert!(reach.is_none(), "{name}/fit_reach");
        } else {
            let got: Vec<_> = reach
                .unwrap()
                .iter()
                .flat_map(|band| [band.0, band.1])
                .collect();
            let expected: Vec<_> = pairs(expected)
                .iter()
                .flat_map(|band| [band.0, band.1])
                .collect();
            errors.compare(
                &format!("{name}/fit_reach"),
                &got,
                &expected,
                Comparison::Exact,
            );
        }
    }
    errors.finish("s2r_band_set_masks");
}

#[test]
fn room_tuning2_s2r_clamp() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("clamp_") {
        let mut got = bands(&fixture["inputs"]["peq"]);
        protect::clamp(&mut got, number(&fixture["inputs"]["rolloff_hz"]));
        errors.compare(
            &name,
            &flatten_bands(&got),
            &flatten_bands(&bands(&fixture["outputs"]["peq"])),
            Comparison::Exact,
        );
    }
    let mut edge = vec![(20.0, 1.0, 3.0)];
    protect::clamp(&mut edge, 20.0);
    assert_eq!(
        edge[0].2, 3.0,
        "a centre exactly at the rolloff is retained"
    );
    errors.finish("s2r_clamp");
}

#[test]
fn room_tuning2_s2r_limit() {
    let full_grid = grid();
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("limit_") {
        let input = &fixture["inputs"];
        let fixed = complex(&input["fixed"]);
        let frequencies = &full_grid[..fixed.len()];
        let rolloff = number(&input["rolloff_hz"]);
        let mut got = bands(&input["peq"]);
        let before = protect::worst_excess(&got, &fixed, frequencies, rolloff, FS);
        let outcome_in = protect::LimitOutcome {
            reduced: input["outcome_in"]["reduced"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect(),
            cuts: input["outcome_in"]["cuts"].as_u64().unwrap() as usize,
        };
        let outcome = protect::limit(&mut got, &fixed, frequencies, rolloff, outcome_in, FS);
        let after = protect::worst_excess(&got, &fixed, frequencies, rolloff, FS);
        errors.compare(
            &format!("{name}/peq"),
            &flatten_bands(&got),
            &flatten_bands(&bands(&fixture["outputs"]["peq"])),
            Comparison::Curve,
        );
        let expected_reduced: Vec<_> = fixture["outputs"]["outcome"]["reduced"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect();
        let expected_outcome = (
            expected_reduced,
            fixture["outputs"]["outcome"]["cuts"].as_u64().unwrap() as usize,
        );
        compare_decision(
            &format!("{name}/outcome"),
            &(outcome.reduced.clone(), outcome.cuts),
            &expected_outcome,
            Comparison::Exact,
        );
        compare_decision(
            &format!("{name}/limited"),
            &protect::limited(&got, &fixed, frequencies, rolloff, FS),
            &fixture["outputs"]["limited"].as_bool().unwrap(),
            Comparison::Exact,
        );
        for (label, actual) in [("worst_before", before), ("worst_after", after)] {
            let expected = &fixture["outputs"][label];
            let actual_bin = actual.map(|(bin, _)| bin);
            let expected_bin =
                (!expected.is_null()).then(|| expected["bin"].as_u64().unwrap() as usize);
            compare_decision(
                &format!("{name}/{label}/bin"),
                &actual_bin,
                &expected_bin,
                Comparison::Exact,
            );
            if let (Some((_, value)), false) = (actual, expected.is_null()) {
                errors.compare(
                    &format!("{name}/{label}/excess"),
                    &[value],
                    &[number(&expected["excess"])],
                    Comparison::Curve,
                );
            }
        }
    }
    // Synthetic exclusion: unity is already above the ceiling and the positive
    // band cannot fix that bin by itself, so the bin is excluded and a cut is added.
    let frequencies = &full_grid[..164];
    let mut excluded = vec![(42.0, 1.0, 1.0)];
    let fixed = vec![Complex64::new(2.0, 0.0); frequencies.len()];
    let outcome = protect::limit(
        &mut excluded,
        &fixed,
        frequencies,
        60.0,
        protect::LimitOutcome::default(),
        FS,
    );
    assert!(outcome.cuts > 0);
    errors.finish("s2r_limit");
}

#[test]
fn room_tuning2_s2r_dc_correction() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("dc_") {
        let mut got = array(&fixture["inputs"]["h"]);
        let (applied, excess) = protect::correct_fir_dc(&mut got);
        assert_eq!(
            applied,
            fixture["outputs"]["applied"].as_bool().unwrap(),
            "{name}"
        );
        errors.compare(
            &format!("{name}/excess"),
            &[excess],
            &[number(&fixture["outputs"]["excess"])],
            Comparison::Exact,
        );
        errors.compare(
            &format!("{name}/h"),
            &got,
            &array(&fixture["outputs"]["h"]),
            Comparison::Curve,
        );
    }
    let threshold = 10_f64.powf(0.5 / 20.0) - 1.0;
    let mut below = vec![1.0 + threshold, 0.0, 0.0];
    let (applied, _) = protect::correct_fir_dc(&mut below);
    assert!(!applied, "the threshold itself is not corrected");
    errors.finish("s2r_dc");
}

#[test]
fn room_tuning2_s2r_residual_guard() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("residual_") {
        let input = &fixture["inputs"];
        let got = peq::fit_residual(
            &array(&input["g"]),
            &array(&input["corr_db"]),
            &bands(&input["peq"]),
            number(&input["lo"]),
            number(&input["hi"]),
            &pairs(&input["bands"]),
            FS,
        );
        errors.compare(
            &name,
            &got,
            &array(&fixture["outputs"]["residual"]),
            Comparison::Curve,
        );
        assert!(got.iter().all(|value| value.is_finite()), "{name}");
    }
    errors.finish("s2r_residual");
}

fn demo() -> &'static BTreeMap<(String, String), ImpulseResponse> {
    static DEMO: OnceLock<BTreeMap<(String, String), ImpulseResponse>> = OnceLock::new();
    DEMO.get_or_init(|| {
        let root = root();
        let sweep =
            read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
        let estimator = SweepEstimator::from_samples(sweep.sample_rate, &sweep.tracks[0]).unwrap();
        let mut out = BTreeMap::new();
        for entry in std::fs::read_dir(root.join("data/demo")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_str().unwrap();
            let Some(names) = room::parse_room_measurement_name(name) else {
                continue;
            };
            if names.side.is_none() {
                continue;
            }
            let wav = read_wav(&path).unwrap();
            let mut hrir = Hrir {
                fs: wav.sample_rate,
                speakers: Vec::new(),
            };
            hrir.open_recording_samples(
                &estimator,
                wav.sample_rate,
                &wav.tracks,
                &names
                    .speakers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                names.side,
                2.0,
            )
            .unwrap();
            for speaker in &names.speakers {
                let response = hrir.get(speaker).unwrap();
                if let Some(ir) = &response.left {
                    out.insert((speaker.clone(), "left".into()), ir.clone());
                }
                if let Some(ir) = &response.right {
                    out.insert((speaker.clone(), "right".into()), ir.clone());
                }
            }
        }
        out
    })
}

fn points(speaker: &str) -> Vec<ImpulseResponse> {
    ["left", "right"]
        .iter()
        .map(|side| demo()[&(speaker.to_owned(), (*side).to_owned())].clone())
        .collect()
}

fn point_spectrum(point: &ImpulseResponse) -> Vec<Complex64> {
    let onset = primitives::onset(&point.data);
    let start = onset.saturating_sub((0.002 * FS) as usize);
    let end = (start + (0.5 * FS) as usize).min(point.data.len());
    let mut window = point.data[start..end].to_vec();
    primitives::taper(&mut window, 0.2);
    let frequencies = grid();
    let mut response = primitives::rfft(&window, N);
    for (value, frequency) in response.iter_mut().zip(frequencies) {
        *value *= Complex64::from_polar(
            1.0,
            -2.0 * std::f64::consts::PI * frequency * (start as f64 - onset as f64) / FS,
        );
    }
    response
}

fn options() -> RcOptions {
    RcOptions {
        policy: PositionPolicy::Listening {
            index: 0,
            weight: 5.0,
        },
        ..Default::default()
    }
}

struct PreFit {
    power: Vec<f64>,
    level_db: f64,
    correction: Vec<f64>,
    long_modes: Vec<(f64, f64)>,
}

fn prefit() -> &'static BTreeMap<String, PreFit> {
    static PREFIT: OnceLock<BTreeMap<String, PreFit>> = OnceLock::new();
    PREFIT.get_or_init(|| {
        let frequencies = grid();
        ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]
            .into_iter()
            .map(|speaker| {
                let points = points(speaker);
                let opts = options();
                let band = correction_band(&[&points], &opts).unwrap();
                let mut stages = RcStages::default();
                let result =
                    design_rc_with_stages(&points, &opts, &band, None, None, Some(&mut stages))
                        .unwrap();
                let correction = frequencies
                    .iter()
                    .enumerate()
                    .map(|(index, frequency)| {
                        let smoothed = (10.0 * (stages.smoothed_variable[index] + 1e-30).log10())
                            .max(10.0 * (stages.smoothed_third[index] + 1e-30).log10() - 3.0);
                        let cap = if *frequency > 16_000.0 { 2.0 } else { 10.0 };
                        (result.level_db - smoothed).clamp(-25.0, cap)
                    })
                    .collect();
                (
                    speaker.to_owned(),
                    PreFit {
                        power: stages.power,
                        level_db: result.level_db,
                        correction,
                        long_modes: result.long_modes,
                    },
                )
            })
            .collect()
    })
}

#[test]
fn room_tuning2_s2r_rolloff() {
    let frequencies = grid();
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("rolloff_") {
        let input = &fixture["inputs"];
        let synthetic = name.starts_with("rolloff_synthetic_");
        let class = if synthetic {
            Comparison::Curve
        } else {
            Comparison::Rebuilt
        };
        let power = if synthetic {
            array(&input["power"])
        } else {
            let speaker = fixture["meta"]["speaker"].as_str().unwrap();
            let power = prefit()[speaker].power.clone();
            errors.compare(
                &format!("{name}/power"),
                &sampled(&power),
                &array(&input["power"]),
                Comparison::Rebuilt,
            );
            power
        };
        let details = protect::rolloff(&power, &frequencies);
        let db: Vec<_> = primitives::smooth(&power, &frequencies, &[3.0])
            .iter()
            .map(|value| 10.0 * (value + 1e-30).log10())
            .collect();
        errors.compare(
            &format!("{name}/db"),
            &sampled(&db),
            &array(&input["db"]),
            class,
        );
        errors.compare(
            &format!("{name}/reference_threshold"),
            &[details.reference, details.threshold],
            &[
                number(&fixture["outputs"]["reference"]),
                number(&fixture["outputs"]["threshold"]),
            ],
            class,
        );
        match (details.frequency, fixture["outputs"]["rolloff_hz"].as_f64()) {
            (Some(got), Some(expected)) => {
                errors.compare(&format!("{name}/rolloff"), &[got], &[expected], class)
            }
            (None, None) => (),
            _ => panic!("{name}: rolloff presence differs"),
        }
        match (details.crossing, &fixture["outputs"]["crossing"]) {
            (Some((upper, upper_db, lower, lower_db)), expected) if !expected.is_null() => {
                errors.compare(
                    &format!("{name}/crossing"),
                    &[upper, upper_db, lower, lower_db],
                    &[
                        number(&expected["upper"]),
                        number(&expected["upper_db"]),
                        number(&expected["lower"]),
                        number(&expected["lower_db"]),
                    ],
                    class,
                );
            }
            (None, expected) if expected.is_null() => (),
            _ => panic!("{name}: crossing presence differs"),
        }
    }
    let endpoint = protect::rolloff_from_db(&[-10.0, 0.0, 0.0], &[20.0, 200.0, 1000.0]);
    assert_eq!(endpoint.frequency, Some(20.0));
    errors.finish("s2r_rolloff");
}

fn dip_source(name: &str, fixture: &Value) -> (Vec<f64>, f64) {
    if name.starts_with("dip_synthetic_") {
        (
            array(&fixture["inputs"]["power"]),
            number(&fixture["inputs"]["level_db"]),
        )
    } else {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        (prefit()[speaker].power.clone(), prefit()[speaker].level_db)
    }
}

#[test]
fn room_tuning2_dip_limit_matches_secs2r() {
    let frequencies = grid();
    let mask = magnitude_mask(&frequencies, &[(20.0, 500.0)], FS);
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("dip_") {
        let input = &fixture["inputs"];
        let class = if name.starts_with("dip_synthetic_") {
            Comparison::Curve
        } else {
            Comparison::Rebuilt
        };
        let (power, level_db) = dip_source(&name, &fixture);
        let long_modes = fixture["meta"]["speaker"]
            .as_str()
            .map_or(&[][..], |speaker| prefit()[speaker].long_modes.as_slice());
        let resolution = primitives::resolution(&frequencies);
        let variable = primitives::smooth(&power, &frequencies, &resolution);
        let third = primitives::smooth(&power, &frequencies, &[3.0]);
        let smoothed: Vec<_> = variable
            .iter()
            .zip(third)
            .map(|(variable, third)| {
                (10.0 * (variable + 1e-30).log10()).max(10.0 * (third + 1e-30).log10() - 3.0)
            })
            .collect();
        let correction: Vec<_> = frequencies
            .iter()
            .enumerate()
            .map(|(index, frequency)| {
                let cap = if *frequency > 16_000.0 { 2.0 } else { 10.0 };
                (level_db - smoothed[index]).clamp(-25.0, cap)
            })
            .collect();
        let expected_power = array(&input["power"]);
        let got_power = if expected_power.len() == power.len() {
            power.clone()
        } else {
            sampled(&power)
        };
        errors.compare(&format!("{name}/power"), &got_power, &expected_power, class);
        errors.compare(
            &format!("{name}/sm"),
            &sampled(&smoothed),
            &array(&input["sm"]),
            class,
        );
        errors.compare(
            &format!("{name}/corr_before"),
            &sampled(&correction),
            &array(&input["corr_before"]),
            class,
        );
        let target = vec![0.0; frequencies.len()];
        let result = dip_limit_curve(
            &power,
            &frequencies,
            &smoothed,
            &target,
            level_db,
            &correction,
        );
        for (label, got) in [
            ("env", result.envelope.as_slice()),
            ("depth", result.depth.as_slice()),
            ("fill", result.fill.as_slice()),
            ("corr_after", result.correction.as_slice()),
        ] {
            errors.compare(
                &format!("{name}/{label}"),
                &sampled(got),
                &array(&fixture["outputs"][label]),
                class,
            );
        }
        let mut unlimited = correction.clone();
        let mut limited = result.correction.clone();
        for (index, frequency) in frequencies.iter().enumerate() {
            if long_modes.iter().any(|(peak, _)| {
                *frequency >= peak * 2_f64.powf(-1.0 / 24.0)
                    && *frequency <= peak * 2_f64.powf(1.0 / 24.0)
            }) {
                unlimited[index] = unlimited[index].min(3.0);
                limited[index] = limited[index].min(3.0);
            }
            unlimited[index] *= mask[index];
            limited[index] *= mask[index];
        }
        errors.compare(
            &format!("{name}/unlimited"),
            &sampled(&unlimited),
            &array(&fixture["outputs"]["unlimited"]),
            class,
        );
        errors.compare(
            &format!("{name}/limited"),
            &sampled(&limited),
            &array(&fixture["outputs"]["limited"]),
            class,
        );
        let notes = limited_dips(
            &frequencies,
            &result.depth,
            &result.fill,
            &limited,
            &unlimited,
        );
        let expected: Vec<_> = fixture["outputs"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(array)
            .collect();
        errors.compare(
            &format!("{name}/notes/hz"),
            &notes.iter().map(|note| note.0).collect::<Vec<_>>(),
            &expected.iter().map(|note| note[0]).collect::<Vec<_>>(),
            Comparison::Exact,
        );
        errors.compare(
            &format!("{name}/notes/depth_fill"),
            &notes
                .iter()
                .flat_map(|note| [note.1, note.2])
                .collect::<Vec<_>>(),
            &expected
                .iter()
                .flat_map(|note| [note[1], note[2]])
                .collect::<Vec<_>>(),
            class,
        );
    }
    errors.finish("s2r_dip");
}

#[test]
fn room_tuning2_low_protect_matches_secs2r() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("mask_") {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let pre = &prefit()[speaker];
        errors.compare(
            &format!("{name}/corr"),
            &sampled(&pre.correction),
            &array(&fixture["inputs"]["corr"]),
            Comparison::Rebuilt,
        );
        let speaker_points = points(speaker);
        let opts = RcOptions {
            low_protect: true,
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let result = design_rc(&speaker_points, &opts, &band, None, None).unwrap();
        errors.compare(
            &format!("{name}/design"),
            &sampled(&result.design_db),
            &array(&fixture["outputs"]["design_db"]),
            Comparison::Rebuilt,
        );
        errors.compare(
            &format!("{name}/rolloff"),
            &[result.rolloff_hz.unwrap()],
            &[number(&fixture["inputs"]["rolloff_hz"])],
            Comparison::Rebuilt,
        );
    }
    errors.finish("s2r_mask");
}

#[test]
fn room_tuning2_realize_synthetic_matches_secs2r() {
    let frequencies = grid();
    let excess: Vec<_> = frequencies
        .iter()
        .map(|frequency| {
            if *frequency > 0.0 && *frequency < 500.0 * 2_f64.sqrt() {
                let value = (*frequency / 100.0).log2();
                0.002 * (-value.powi(2)).exp()
            } else {
                0.0
            }
        })
        .collect();
    let magnitude = vec![Complex64::new(1.0, 0.0); frequencies.len()];
    let mut errors = Errors::default();
    for case in ["unity_hf", "unity_off", "unity_beta", "unity_inert"] {
        let fixture = fixture_r(&format!("realize_{case}"));
        let input = &fixture["inputs"];
        let lift = number(&input["lift"]);
        let taps = input["taps"].as_u64().unwrap() as usize;
        let extras = input["hf_phase"]["rc_bands"]
            .as_array()
            .map_or_else(Vec::new, |_| pairs(&input["hf_phase"]["rc_bands"]));
        let region = input["hf_phase"].as_object().and_then(|_| {
            hf_phase_region(
                &frequencies,
                FS,
                number(&input["hf_phase"]["rc_hi"]),
                &merge_band_set((20.0, number(&input["hf_phase"]["rc_hi"])), &extras),
            )
        });
        compare_input_sha(
            &format!("{case}/excess_sha256"),
            &sha_f64(&excess),
            input["excess_sha256"].as_str().unwrap(),
        );
        let kappa = input["beta_rule"]
            .as_str()
            .map(|_| lf_alignment_weight(&frequencies, 40.0));
        if let (Some(kappa), Some(expected)) = (&kappa, input["beta_sha256"].as_str()) {
            compare_input_sha(&format!("{case}/beta_sha256"), &sha_f64(kappa), expected);
        }
        let mut error_mask = vec![false; frequencies.len()];
        for run in input["band_true_runs"].as_array().unwrap() {
            let row = run.as_array().unwrap();
            let start = row[0].as_u64().unwrap() as usize;
            let end = row[1].as_u64().unwrap() as usize;
            error_mask[start..end].fill(true);
        }
        compare_input_sha(
            &format!("{case}/band_sha256"),
            &sha_f64(
                &error_mask
                    .iter()
                    .map(|value| f64::from(*value))
                    .collect::<Vec<_>>(),
            ),
            input["band_sha256"].as_str().unwrap(),
        );
        let re: Vec<_> = magnitude.iter().map(|value| value.re).collect();
        let im: Vec<_> = magnitude.iter().map(|value| value.im).collect();
        compare_input_sha(
            &format!("{case}/m_re_sha256"),
            &sha_f64(&re),
            input["m_sha256"]["re"].as_str().unwrap(),
        );
        compare_input_sha(
            &format!("{case}/m_im_sha256"),
            &sha_f64(&im),
            input["m_sha256"]["im"].as_str().unwrap(),
        );
        let got = realize_for_fixture(
            &frequencies,
            &excess,
            &magnitude,
            taps,
            lift,
            1.5 * FS / taps as f64,
            500.0 * 2_f64.sqrt(),
            &extras,
            None,
            FS,
            number(&input["frac"]),
            kappa.as_deref(),
            region.as_ref(),
            Some(&error_mask),
        );
        errors.compare(
            &format!("{case}/applied_before"),
            &sampled(&got.applied_before),
            &array(&fixture["outputs"]["applied_before"]),
            Comparison::Exact,
        );
        errors.compare(
            &format!("{case}/applied_after"),
            &sampled(&got.applied_after),
            &array(&fixture["outputs"]["applied_after"]),
            Comparison::Exact,
        );
        errors.compare(
            &format!("{case}/h"),
            &got.fir,
            &array(&fixture["outputs"]["h"]),
            Comparison::Curve,
        );
        errors.compare(
            &format!("{case}/rms_max"),
            &[got.rms, got.max],
            &[
                number(&fixture["outputs"]["rms"]),
                number(&fixture["outputs"]["max"]),
            ],
            Comparison::Curve,
        );
        for (label, actual) in [
            ("total", got.total),
            ("rho", got.rho),
            ("wrapped", got.wrapped),
            ("correction", got.correction),
            ("rotation", got.rotation),
        ] {
            if let Some(expected) = fixture["outputs"][label].as_f64() {
                errors.compare(
                    &format!("{case}/{label}"),
                    &[actual.unwrap()],
                    &[expected],
                    Comparison::Exact,
                );
            } else {
                assert!(actual.is_none(), "{case}/{label}");
            }
        }
        if let Some(expected) = fixture["outputs"]["limited"].as_bool() {
            compare_decision(
                &format!("{case}/limited"),
                &got.limited,
                &expected,
                Comparison::Exact,
            );
        } else {
            assert!(got.rotation_deg.is_none());
        }
    }

    let fixture = fixture_r("realize_fl_phase_linear");
    let input = &fixture["inputs"]["selected_repeat"];
    let excess = array(&input["excess"]);
    let band = array(&input["band"])
        .into_iter()
        .map(|value| value != 0.0)
        .collect::<Vec<_>>();
    let kappa = array(&input["beta"]);
    let magnitude = complex(&input["m"]);
    let taps = input["taps"].as_u64().unwrap() as usize;
    let region = hf_phase_region(&frequencies, FS, 500.0, &[(20.0, 500.0)]).unwrap();
    let got = realize_for_fixture(
        &frequencies,
        &excess,
        &magnitude,
        taps,
        number(&input["lift"]),
        1.5 * FS / taps as f64,
        500.0 * 2_f64.sqrt(),
        &[],
        None,
        FS,
        number(&input["frac"]),
        Some(&kappa),
        Some(&region),
        Some(&band),
    );
    let expected = fixture["outputs"]["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|call| call["selected"].as_bool() == Some(true))
        .unwrap();
    compare_realize_call("fl_phase_linear/selected", &got, expected, &mut errors);
    errors.finish("s2r_realize_synthetic");
}

#[test]
fn room_tuning2_realize_demo_reports_secs2r() {
    for (name, fixture) in fixtures_r("realize_") {
        if name.starts_with("realize_unity_") {
            continue;
        }
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let settings = &fixture["meta"]["settings"];
        let speaker_points = points(speaker);
        let opts = RcOptions {
            mode: Tuning2Mode::parse(settings["mode"].as_str().unwrap()).unwrap(),
            low_protect: settings["low_protect"].as_bool().unwrap(),
            dip_limit: settings["dip_limit"].as_bool().unwrap(),
            lf_align: settings["lf_align"].as_bool().unwrap(),
            mode_decay: settings["mode_decay"].as_bool().unwrap(),
            hf_phase: settings["hf_phase"].as_bool().unwrap(),
            extra_bands: pairs(&settings["rc_bands"]),
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let mut stages = RcStages::default();
        let result =
            design_rc_with_stages(&speaker_points, &opts, &band, None, None, Some(&mut stages))
                .unwrap();
        let actual: Vec<_> = stages
            .sessions
            .iter()
            .flat_map(|session| &session.realizations)
            .collect();
        let expected = fixture["outputs"]["calls"].as_array().unwrap();
        println!(
            "REPORT {name}: selected_fraction got={:.1} expected={:.1}; calls got={} expected={}",
            result.phase_strength,
            number(&fixture["outputs"]["selected_fraction"]),
            actual.len(),
            expected.len()
        );
        for (index, (got, want)) in actual.iter().zip(expected).enumerate() {
            println!(
                "REPORT {name}/call_{index:02}: fraction got={:.1} expected={:.1}; limited got={} expected={:?}; rotation got={:?} expected={:?}; rms got={:.9e} expected={:.9e}; max got={:.9e} expected={:.9e}",
                got.strength,
                number(&want["frac"]),
                got.limited,
                want["limited"].as_bool(),
                got.rotation,
                want["rotation"].as_f64(),
                got.rms,
                number(&want["rms"]),
                got.max,
                number(&want["max"]),
            );
        }
        if let Some(index) = expected
            .iter()
            .position(|call| call["selected"].as_bool() == Some(true))
            && let Some(got) = actual.get(index)
        {
            let want = &expected[index];
            let want_adv = array(&want["applied_after"]);
            let got_adv = if want_adv.len() == got.applied_after.len() {
                got.applied_after.clone()
            } else {
                sampled(&got.applied_after)
            };
            let adv_max = got_adv
                .iter()
                .zip(want_adv)
                .map(|(got, want)| (got - want).abs())
                .fold(0.0, f64::max);
            let h_max = got
                .fir
                .iter()
                .zip(array(&want["h"]))
                .map(|(got, want)| (got - want).abs())
                .fold(0.0, f64::max);
            println!(
                "REPORT {name}/selected_call_{index:02}: adv_max={adv_max:.9e}; h_max={h_max:.9e}"
            );
        }
    }
}

#[test]
fn room_tuning2_lf_excess_recombines_secs2r() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("lf_excess_") {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let prep = fixture_r(&format!(
            "lf_prep_{}_lf_linear",
            speaker.to_ascii_lowercase()
        ));
        let kappa = array(&prep["outputs"]["beta"]);
        let target = array(&prep["outputs"]["target"]);
        let tau = array(&fixture["outputs"]["tau"]);
        let reference = number(&fixture["outputs"]["reference"]);
        let tau_total = array(&fixture["outputs"]["tau_tot"]);
        let reference_total = number(&fixture["outputs"]["reference_tot"]);
        let fade = array(&fixture["outputs"]["fade"]);
        let expected_error = array(&fixture["outputs"]["total_error"]);
        let error: Vec<_> = tau_total
            .iter()
            .zip(&target)
            .map(|(delay, target)| delay - reference_total - target)
            .collect();
        errors.compare(
            &format!("{name}/error"),
            &error,
            &expected_error,
            Comparison::Exact,
        );
        let total_valid: Vec<_> = error.iter().map(|value| value.is_finite()).collect();
        let excess = blend_lf_excess(&tau, reference, &error, &total_valid, &kappa, &fade);
        errors.compare(
            &format!("{name}/excess"),
            &excess,
            &array(&fixture["outputs"]["excess"]),
            Comparison::Exact,
        );
        if fixture["meta"]["full_resolution"].is_array() {
            let full_frequencies = grid();
            let full_total_phase = complex(&fixture["outputs"]["ut_full"]);
            let beta = number(&fixture["outputs"]["phase_frac"]);
            let total = primitives::smooth_phasors(&full_total_phase, &full_frequencies, &[beta]);
            errors.compare(
                &format!("{name}/full_total"),
                &sampled_complex(&total)
                    .iter()
                    .flat_map(|value| [value.re, value.im])
                    .collect::<Vec<_>>(),
                &complex(&fixture["outputs"]["total"])
                    .iter()
                    .flat_map(|value| [value.re, value.im])
                    .collect::<Vec<_>>(),
                Comparison::Curve,
            );
            let full_delay = smoothed_phase_delay(&total, &full_frequencies);
            errors.compare(
                &format!("{name}/full_delay"),
                &sampled(&full_delay),
                &array(&fixture["outputs"]["tau_tot"]),
                Comparison::Curve,
            );
            let full_reference = primitives::median(
                &full_frequencies
                    .iter()
                    .enumerate()
                    .filter(|(_, frequency)| **frequency >= 3000.0 && **frequency <= 6000.0)
                    .map(|(index, _)| full_delay[index])
                    .collect::<Vec<_>>(),
            );
            errors.compare(
                &format!("{name}/full_reference"),
                &[full_reference],
                &[reference_total],
                Comparison::Curve,
            );
            let full_target = array(&fixture["outputs"]["target_full"]);
            let full_error: Vec<_> = full_delay
                .iter()
                .zip(&full_target)
                .map(|(delay, target)| delay - full_reference - target)
                .collect();
            errors.compare(
                &format!("{name}/full_error"),
                &sampled(&full_error),
                &expected_error,
                Comparison::Curve,
            );
            let full_tau = array(&fixture["outputs"]["tau_full"]);
            let full_fade = array(&fixture["outputs"]["fade_full"]);
            let full_kappa = array(&fixture["outputs"]["beta_full"]);
            let full_valid: Vec<_> = full_total_phase
                .iter()
                .zip(&total)
                .map(|(raw, smoothed)| {
                    raw.re.is_finite()
                        && raw.im.is_finite()
                        && smoothed.re.is_finite()
                        && smoothed.im.is_finite()
                })
                .collect();
            let full_excess = blend_lf_excess(
                &full_tau,
                reference,
                &full_error,
                &full_valid,
                &full_kappa,
                &full_fade,
            );
            errors.compare(
                &format!("{name}/full_excess"),
                &sampled(&full_excess),
                &array(&fixture["outputs"]["excess"]),
                Comparison::Curve,
            );
            let realize = fixture_r("realize_fl_phase_linear");
            let realize_excess = array(&realize["inputs"]["selected_repeat"]["excess"]);
            if fixture["meta"]["realize_excess_identity"]["bit_identical"]
                .as_bool()
                .unwrap()
            {
                assert_eq!(full_excess.len(), realize_excess.len());
                assert!(
                    full_excess
                        .iter()
                        .zip(&realize_excess)
                        .all(|(got, expected)| got.to_bits() == expected.to_bits()),
                    "{name}/realize excess bit identity"
                );
            }

            let speaker_points = points("FL");
            let opts = RcOptions {
                mode: Tuning2Mode::Linear,
                lf_align: true,
                ..options()
            };
            let band = correction_band(&[&speaker_points], &opts).unwrap();
            let mut stages = RcStages::default();
            design_rc_with_stages(&speaker_points, &opts, &band, None, None, Some(&mut stages))
                .unwrap();
            let session = &stages.sessions[0];
            for (label, got, want) in [
                (
                    "ut",
                    sampled_complex(&session.v_total),
                    complex(&fixture["outputs"]["ut"]),
                ),
                (
                    "total",
                    sampled_complex(&session.v_total_smoothed),
                    complex(&fixture["outputs"]["total"]),
                ),
            ] {
                let maximum = got
                    .iter()
                    .zip(want)
                    .map(|(got, want)| (got - want).norm())
                    .fold(0.0, f64::max);
                println!("REPORT {name}/rebuilt_{label}: max_complex={maximum:.9e}");
            }
            for (label, got, want) in [
                (
                    "tau_tot",
                    sampled(&session.total_delay),
                    array(&fixture["outputs"]["tau_tot"]),
                ),
                (
                    "total_error",
                    sampled(&session.total_error),
                    expected_error.clone(),
                ),
                (
                    "excess",
                    sampled(&session.excess),
                    array(&fixture["outputs"]["excess"]),
                ),
            ] {
                let maximum = got
                    .iter()
                    .zip(want)
                    .map(|(got, want)| (got - want).abs())
                    .fold(0.0, f64::max);
                println!("REPORT {name}/rebuilt_{label}: max_abs={maximum:.9e}");
            }
        }
    }
    errors.finish("s2r_lf_excess");
}

#[test]
fn room_tuning2_lf_preparation_matches_secs2r() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("lf_prep_") {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let protect = fixture["meta"]["setting"]
            .as_str()
            .unwrap()
            .contains("protect");
        let speaker_points = points(speaker);
        let opts = RcOptions {
            mode: Tuning2Mode::Linear,
            low_protect: protect,
            lf_align: true,
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let mut stages = RcStages::default();
        let result =
            design_rc_with_stages(&speaker_points, &opts, &band, None, None, Some(&mut stages))
                .unwrap();
        let f_lin = stages.lf_f_lin_hz.unwrap();
        errors.compare(
            &format!("{name}/f_lin"),
            &[f_lin],
            &[number(&fixture["outputs"]["f_lin"])],
            Comparison::Rebuilt,
        );
        if let Some(expected) = fixture["outputs"]["rolloff_hz"].as_f64() {
            let got = if f_lin > opts.low {
                f_lin / 1.5
            } else {
                opts.low
            };
            errors.compare(
                &format!("{name}/rolloff"),
                &[got],
                &[expected],
                Comparison::Rebuilt,
            );
        }
        for (label, got) in [
            ("beta_raw", stages.lf_weight_raw.as_slice()),
            ("beta", stages.lf_weight.as_slice()),
            ("sm", stages.lf_smoothed_db.as_slice()),
            ("corr", stages.lf_target_magnitude.as_slice()),
            ("tau", stages.lf_target_delay.as_slice()),
            ("target", stages.lf_target.as_slice()),
        ] {
            errors.compare(
                &format!("{name}/{label}"),
                &sampled(got),
                &array(&fixture["outputs"][label]),
                Comparison::Rebuilt,
            );
        }
        assert!(result.lf_align.is_some());
    }
    errors.finish("s2r_lf_prep");
}

#[test]
fn room_tuning2_mode_decay_matches_secs2r() {
    let frequencies = grid();
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("mode_decay_") {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let speaker_points = points(speaker);
        let opts = RcOptions {
            mode_decay: true,
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let mut stages = RcStages::default();
        let result =
            design_rc_with_stages(&speaker_points, &opts, &band, None, None, Some(&mut stages))
                .unwrap();
        let details = stages.mode_decay.as_ref().unwrap();
        errors.compare(
            &format!("{name}/candidates"),
            &flatten_pairs(&details.candidates),
            &flatten_pairs(&pairs(&fixture["outputs"]["long_modes"])),
            Comparison::Rebuilt,
        );
        errors.compare(
            &format!("{name}/band_t60"),
            &details.band_t60,
            &array(&fixture["outputs"]["band_t60"]),
            Comparison::Rebuilt,
        );
        let expected_refined = fixture["outputs"]["refined"].as_array().unwrap();
        assert_eq!(
            details.refinements.len(),
            expected_refined.len(),
            "{name}/refined length"
        );
        for (index, ((candidate, refinement), expected)) in
            details.refinements.iter().zip(expected_refined).enumerate()
        {
            errors.compare(
                &format!("{name}/refined/{index}/candidate"),
                &[*candidate],
                &[number(&expected["p"])],
                Comparison::Exact,
            );
            compare_refinement(
                &format!("{name}/refined/{index}"),
                refinement,
                expected,
                Comparison::Rebuilt,
                &mut errors,
            );
        }
        errors.compare(
            &format!("{name}/merged"),
            &flatten_pairs(&details.merged),
            &flatten_pairs(&pairs(&fixture["outputs"]["merged"])),
            Comparison::Rebuilt,
        );
        errors.compare(
            &format!("{name}/heights"),
            &details.heights,
            &array(&fixture["outputs"]["heights"]),
            Comparison::Rebuilt,
        );
        let expected_attempts = fixture["outputs"]["decay_cuts"].as_array().unwrap();
        assert_eq!(
            details.decay_cuts.len(),
            expected_attempts.len(),
            "{name}/decay_cuts length"
        );
        for (index, (got, expected)) in details.decay_cuts.iter().zip(expected_attempts).enumerate()
        {
            if expected.is_null() {
                assert!(got.is_none(), "{name}/decay_cuts/{index}");
            } else {
                let got = got.unwrap();
                errors.compare(
                    &format!("{name}/decay_cuts/{index}"),
                    &[got.frequency_hz, got.q, -got.depth_db],
                    &array(expected),
                    Comparison::Rebuilt,
                );
            }
        }
        let smoothed: Vec<_> = stages
            .smoothed_variable
            .iter()
            .zip(&stages.smoothed_third)
            .map(|(variable, third)| {
                (10.0 * (variable + 1e-30).log10()).max(10.0 * (third + 1e-30).log10() - 3.0)
            })
            .collect();
        errors.compare(
            &format!("{name}/power"),
            &sampled(&stages.power),
            &array(&fixture["outputs"]["power"]),
            Comparison::Rebuilt,
        );
        errors.compare(
            &format!("{name}/sm"),
            &sampled(&smoothed),
            &array(&fixture["outputs"]["sm"]),
            Comparison::Rebuilt,
        );
        errors.compare(
            &format!("{name}/design"),
            &sampled(&stages.design_db),
            &array(&fixture["outputs"]["design_db"]),
            Comparison::Rebuilt,
        );
        let decay = result.mode_decay.as_ref().unwrap();
        errors.compare(
            &format!("{name}/target"),
            &[decay.target_t60_s],
            &[number(&fixture["outputs"]["target_t60_s"])],
            Comparison::Rebuilt,
        );
        let expected_selected: Vec<_> = fixture["outputs"]["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(array)
            .collect();
        errors.compare(
            &format!("{name}/selected"),
            &decay
                .cuts
                .iter()
                .flat_map(|cut| [cut.frequency_hz, cut.measured_t60_s, cut.depth_db])
                .collect::<Vec<_>>(),
            &expected_selected.into_iter().flatten().collect::<Vec<_>>(),
            Comparison::Rebuilt,
        );
        let expected_cuts: Vec<_> = fixture["outputs"]["cuts"]
            .as_array()
            .unwrap()
            .iter()
            .map(array)
            .collect();
        errors.compare(
            &format!("{name}/cuts"),
            &decay
                .cuts
                .iter()
                .flat_map(|cut| [cut.frequency_hz, cut.q, -cut.depth_db])
                .collect::<Vec<_>>(),
            &expected_cuts.into_iter().flatten().collect::<Vec<_>>(),
            Comparison::Rebuilt,
        );
        let decay_bands: Vec<_> = decay
            .cuts
            .iter()
            .map(|cut| (cut.frequency_hz, cut.q, -cut.depth_db))
            .collect();
        errors.compare(
            &format!("{name}/dr"),
            &sampled_complex(&peq::response(&frequencies, &decay_bands, FS))
                .iter()
                .flat_map(|value| [value.re, value.im])
                .collect::<Vec<_>>(),
            &complex(&fixture["outputs"]["dr"])
                .iter()
                .flat_map(|value| [value.re, value.im])
                .collect::<Vec<_>>(),
            Comparison::Rebuilt,
        );
    }
    errors.finish("s2r_mode_decay");
}

#[derive(Default)]
struct RcReportOnly {
    peq_f: f64,
    peq_q: f64,
    peq_gain: f64,
    fir: f64,
    response_db: f64,
    phase_strength: f64,
}

#[derive(Debug)]
enum RcActualNote {
    Bands(Vec<(f64, f64)>),
    LongModes {
        cap_db: f64,
        modes: Vec<(f64, f64)>,
    },
    Dips {
        role: String,
        dips: Vec<(f64, f64, f64)>,
    },
    LowProtect {
        role: String,
        rolloff_hz: Option<f64>,
        hp_hz: Option<f64>,
        limited: bool,
        reduced: usize,
        cuts: usize,
    },
    LfAlign {
        role: String,
        strength: f64,
        arrival_ms: Option<f64>,
        limited: bool,
    },
    ModeDecay {
        role: String,
        target_t60_s: f64,
        modes: Vec<(f64, f64, f64)>,
    },
    HfPhase {
        role: String,
        rotation_deg: Option<f64>,
        limited: bool,
    },
}

fn actual_rc_notes(speaker: &str, opts: &RcOptions, result: &SpeakerRc) -> Vec<RcActualNote> {
    let mut notes = Vec::new();
    if !opts.extra_bands.is_empty() {
        notes.push(RcActualNote::Bands(result.bands.clone()));
    }
    if !result.long_modes.is_empty() {
        notes.push(RcActualNote::LongModes {
            cap_db: 3.0,
            modes: result.long_modes.clone(),
        });
        if matches!(opts.mode, Tuning2Mode::Linear | Tuning2Mode::Ultra(_)) {
            notes.push(RcActualNote::LongModes {
                cap_db: 3.0,
                modes: result.long_modes.clone(),
            });
        }
    }
    if opts.dip_limit && !result.limited_dips.is_empty() {
        notes.push(RcActualNote::Dips {
            role: speaker.to_owned(),
            dips: result.limited_dips.clone(),
        });
    }
    if opts.low_protect {
        notes.push(RcActualNote::LowProtect {
            role: speaker.to_owned(),
            rolloff_hz: result.rolloff_hz,
            hp_hz: None,
            limited: result.low_limited,
            reduced: result.limit_outcome.reduced.len(),
            cuts: result.limit_outcome.cuts,
        });
    }
    if let Some(mode_decay) = &result.mode_decay {
        notes.push(RcActualNote::ModeDecay {
            role: speaker.to_owned(),
            target_t60_s: mode_decay.target_t60_s,
            modes: mode_decay
                .cuts
                .iter()
                .map(|cut| (cut.frequency_hz, cut.measured_t60_s, cut.depth_db))
                .collect(),
        });
    }
    if let Some(hf_phase) = &result.hf_phase {
        notes.push(RcActualNote::HfPhase {
            role: speaker.to_owned(),
            rotation_deg: hf_phase.rotation_deg,
            limited: hf_phase.limited,
        });
    }
    if let Some(lf_align) = &result.lf_align {
        notes.push(RcActualNote::LfAlign {
            role: speaker.to_owned(),
            strength: result.phase_strength,
            arrival_ms: lf_align.arrival_ms,
            limited: lf_align.limited,
        });
    }
    notes
}

fn compare_rc_notes(name: &str, actual: &[RcActualNote], expected: &[Value], errors: &mut Errors) {
    assert_eq!(actual.len(), expected.len(), "{name}/notes length");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let label = format!("{name}/notes[{index}]");
        match (actual, expected["kind"].as_str().unwrap()) {
            (RcActualNote::Bands(got), "RcBands") => {
                assert_eq!(expected.as_object().unwrap().len(), 2, "{label} payload");
                errors.compare(
                    &format!("{label}/bands"),
                    &flatten_pairs(got),
                    &flatten_pairs(&pairs(&expected["bands"])),
                    Comparison::Exact,
                );
            }
            (RcActualNote::LongModes { cap_db, modes }, "LongModeBoost") => {
                assert_eq!(expected.as_object().unwrap().len(), 3, "{label} payload");
                errors.compare(
                    &format!("{label}/cap_db"),
                    &[*cap_db],
                    &[number(&expected["cap_db"])],
                    Comparison::Exact,
                );
                // Peak frequencies are grid bins (exact); their T60 values come
                // from the low-rate response rebuilt from the demo recordings.
                let expected_modes = pairs(&expected["modes"]);
                errors.compare(
                    &format!("{label}/modes/hz"),
                    &modes.iter().map(|mode| mode.0).collect::<Vec<_>>(),
                    &expected_modes.iter().map(|mode| mode.0).collect::<Vec<_>>(),
                    Comparison::Exact,
                );
                errors.compare(
                    &format!("{label}/modes/t60"),
                    &modes.iter().map(|mode| mode.1).collect::<Vec<_>>(),
                    &expected_modes.iter().map(|mode| mode.1).collect::<Vec<_>>(),
                    Comparison::Rebuilt,
                );
            }
            (RcActualNote::Dips { role, dips }, "DipLimit") => {
                assert_eq!(expected.as_object().unwrap().len(), 3, "{label} payload");
                assert_eq!(role, expected["role"].as_str().unwrap(), "{label}/role");
                let expected_dips: Vec<_> = expected["dips"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(array)
                    .collect();
                errors.compare(
                    &format!("{label}/dips/hz"),
                    &dips.iter().map(|dip| dip.0).collect::<Vec<_>>(),
                    &expected_dips.iter().map(|dip| dip[0]).collect::<Vec<_>>(),
                    Comparison::Exact,
                );
                errors.compare(
                    &format!("{label}/dips/depth_fill"),
                    &dips
                        .iter()
                        .flat_map(|dip| [dip.1, dip.2])
                        .collect::<Vec<_>>(),
                    &expected_dips
                        .iter()
                        .flat_map(|dip| [dip[1], dip[2]])
                        .collect::<Vec<_>>(),
                    Comparison::Rebuilt,
                );
            }
            (
                RcActualNote::LowProtect {
                    role,
                    rolloff_hz,
                    hp_hz,
                    limited,
                    reduced,
                    cuts,
                },
                "LowProtect",
            ) => {
                assert_eq!(expected.as_object().unwrap().len(), 7, "{label} payload");
                assert_eq!(role, expected["role"].as_str().unwrap(), "{label}/role");
                match (*rolloff_hz, expected["rolloff_hz"].as_f64()) {
                    (Some(got), Some(want)) => errors.compare(
                        &format!("{label}/rolloff_hz"),
                        &[got],
                        &[want],
                        Comparison::Rebuilt,
                    ),
                    (None, None) => (),
                    _ => panic!("{label}/rolloff_hz presence differs"),
                }
                match (*hp_hz, expected["hp_hz"].as_f64()) {
                    (Some(got), Some(want)) => errors.compare(
                        &format!("{label}/hp_hz"),
                        &[got],
                        &[want],
                        Comparison::Exact,
                    ),
                    (None, None) => (),
                    _ => panic!("{label}/hp_hz presence differs"),
                }
                compare_decision(
                    &format!("{label}/limited"),
                    limited,
                    &expected["limited"].as_bool().unwrap(),
                    Comparison::FitDependent,
                );
                compare_decision(
                    &format!("{label}/outcome"),
                    &(*reduced, *cuts),
                    &(
                        expected["reduced"].as_u64().unwrap() as usize,
                        expected["cuts"].as_u64().unwrap() as usize,
                    ),
                    Comparison::FitDependent,
                );
            }
            (
                RcActualNote::ModeDecay {
                    role,
                    target_t60_s,
                    modes,
                },
                "ModeDecay",
            ) => {
                assert_eq!(role, expected["role"].as_str().unwrap(), "{label}/role");
                errors.compare(
                    &format!("{label}/target"),
                    &[*target_t60_s],
                    &[number(&expected["target_t60_s"])],
                    Comparison::Rebuilt,
                );
                let expected_modes: Vec<_> = expected["modes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(array)
                    .collect();
                errors.compare(
                    &format!("{label}/modes"),
                    &modes
                        .iter()
                        .flat_map(|mode| [mode.0, mode.1, mode.2])
                        .collect::<Vec<_>>(),
                    &expected_modes.into_iter().flatten().collect::<Vec<_>>(),
                    Comparison::Rebuilt,
                );
            }
            (
                RcActualNote::HfPhase {
                    role,
                    rotation_deg,
                    limited,
                },
                "HfPhase",
            ) => {
                assert_eq!(role, expected["role"].as_str().unwrap(), "{label}/role");
                println!(
                    "REPORT {label}: rotation got={rotation_deg:?} expected={:?}; limited got={limited} expected={}",
                    expected["rotation_deg"].as_f64(),
                    expected["limited"].as_bool().unwrap()
                );
            }
            (
                RcActualNote::LfAlign {
                    role,
                    strength,
                    arrival_ms,
                    limited,
                },
                "LfAlign",
            ) => {
                assert_eq!(role, expected["role"].as_str().unwrap(), "{label}/role");
                println!(
                    "REPORT {label}: strength got={strength:.9e} expected={:.9e}; arrival got={arrival_ms:?} expected={:?}; limited got={limited} expected={}",
                    number(&expected["strength"]),
                    expected["arrival_ms"].as_f64(),
                    expected["limited"].as_bool().unwrap()
                );
            }
            (_, kind) => panic!("{label}: actual {actual:?}, expected kind {kind}"),
        }
    }
}

#[test]
fn room_tuning2_bands_matches_secs2r() {
    let mut errors = Errors::default();
    let mut reports: BTreeMap<String, RcReportOnly> = BTreeMap::new();
    for (name, fixture) in fixtures_r("rc_") {
        if name.ends_with("_upper_mixed") {
            continue;
        }
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let setting = fixture["meta"]["setting"].as_str().unwrap();
        let settings = &fixture["inputs"]["settings"];
        let extras = pairs(&settings["rc_bands"]);
        let speaker_points = points(speaker);
        let opts = RcOptions {
            mode: Tuning2Mode::parse(settings["mode"].as_str().unwrap()).unwrap(),
            dip_limit: settings["dip_limit"].as_bool().unwrap(),
            low_protect: settings["low_protect"].as_bool().unwrap(),
            extra_bands: extras,
            lf_align: settings["lf_align"].as_bool().unwrap_or(false),
            mode_decay: settings["mode_decay"].as_bool().unwrap_or(false),
            hf_phase: settings["hf_phase"].as_bool().unwrap_or(false),
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let result = design_rc(&speaker_points, &opts, &band, None, None).unwrap();
        if fixture["outputs"]["phase_candidates"].is_object() {
            let expected = &fixture["outputs"]["phase_candidates"];
            println!(
                "REPORT {name}/phase_candidates: got={:?}, kept={}; expected requested={:.9e}, fallback={:.9e}, kept={}",
                result.phase_candidates,
                result.phase_frac,
                number(&expected["requested_score"]),
                number(&expected["fallback_score"]),
                number(&expected["kept"]),
            );
        }
        errors.compare(
            &format!("{name}/design_db"),
            &sampled(&result.design_db),
            &array(&fixture["outputs"]["design_db"]),
            Comparison::Rebuilt,
        );
        match (result.rolloff_hz, fixture["outputs"]["rolloff_hz"].as_f64()) {
            (Some(got), Some(expected)) => errors.compare(
                &format!("{name}/rolloff"),
                &[got],
                &[expected],
                Comparison::Rebuilt,
            ),
            (None, None) => (),
            _ => panic!("{name}: rolloff presence differs"),
        }
        let actual_notes = actual_rc_notes(speaker, &opts, &result);
        if setting == "off" {
            assert!(
                actual_notes
                    .iter()
                    .all(|note| matches!(note, RcActualNote::LongModes { .. })),
                "{name}: off emitted an option note: {actual_notes:?}"
            );
        }
        compare_rc_notes(
            &name,
            &actual_notes,
            fixture["outputs"]["notes"].as_array().unwrap(),
            &mut errors,
        );

        let report = reports.entry(setting.to_owned()).or_default();
        let expected_peq = bands(&fixture["outputs"]["peq"]);
        for (got, expected) in result.peq.iter().zip(&expected_peq) {
            report.peq_f = report.peq_f.max((got.0 - expected.0).abs());
            report.peq_q = report.peq_q.max((got.1 - expected.1).abs());
            report.peq_gain = report.peq_gain.max((got.2 - expected.2).abs());
        }
        let expected_fir = array(&fixture["outputs"]["fir"]);
        report.fir = report.fir.max(
            result
                .fir
                .iter()
                .zip(expected_fir)
                .map(|(got, expected)| (got - expected).abs())
                .fold(0.0, f64::max),
        );
        let expected_response = complex(&fixture["outputs"]["response"]);
        report.response_db = report.response_db.max(
            sampled_complex(&result.response)
                .iter()
                .zip(expected_response)
                .map(|(got, expected)| {
                    (20.0 * (got.norm() + 1e-12).log10() - 20.0 * (expected.norm() + 1e-12).log10())
                        .abs()
                })
                .fold(0.0, f64::max),
        );
        report.phase_strength = report
            .phase_strength
            .max((result.phase_strength - number(&fixture["outputs"]["phase_strength"])).abs());
    }
    for (setting, report) in reports {
        println!(
            "REPORT s2r_rc_{setting}: peq_f={:.9e} peq_q={:.9e} peq_gain={:.9e} fir={:.9e} response_db={:.9e} phase_strength={:.9e}",
            report.peq_f,
            report.peq_q,
            report.peq_gain,
            report.fir,
            report.response_db,
            report.phase_strength
        );
    }
    errors.finish("s2r_rc_design");
}

fn upper_band_cases() -> Vec<(&'static str, f64, f64)> {
    let third = 2_f64.powf(1.0 / 6.0);
    vec![
        ("octave_500", 500.0 / 2_f64.sqrt(), 500.0 * 2_f64.sqrt()),
        ("octave_1000", 1000.0 / 2_f64.sqrt(), 1000.0 * 2_f64.sqrt()),
        ("third_100", 100.0 / third, 100.0 * third),
        ("third_125", 125.0 / third, 125.0 * third),
        ("third_160", 160.0 / third, 160.0 * third),
        ("third_200", 200.0 / third, 200.0 * third),
        ("third_250", 250.0 / third, 250.0 * third),
        ("third_315", 315.0 / third, 315.0 * third),
        ("third_400", 400.0 / third, 400.0 * third),
        ("third_500", 500.0 / third, 500.0 * third),
    ]
}

#[test]
fn room_tuning2_s2r_band_sos() {
    let fixture = fixture_r("band_sos");
    let mut errors = Errors::default();
    for (name, low, high) in upper_band_cases() {
        let got = impulcifer_dsp::filters::butter_bandpass(4, low / (FS / 2.0), high / (FS / 2.0));
        let got: Vec<_> = got.0.into_iter().flatten().collect();
        let expected: Vec<_> = fixture["outputs"][name]["sos"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(array)
            .collect();
        errors.compare(name, &got, &expected, Comparison::Curve);
    }
    errors.finish("s2r_band_sos_curve");
}

#[test]
fn room_tuning2_s2r_band_t60_synthetic() {
    let fixture = fixture_r("band_t60_synthetic");
    let impulse = array(&fixture["inputs"]["ir"]);
    let mut errors = Errors::default();
    for (name, low, high) in upper_band_cases() {
        let got = primitives::band_decay(&impulse, FS, low, high);
        let expected = &fixture["outputs"][name];
        assert_eq!(
            got.i5,
            expected["i5"].as_u64().unwrap() as usize,
            "{name}/i5"
        );
        assert_eq!(
            got.i25,
            expected["i25"].as_u64().unwrap() as usize,
            "{name}/i25"
        );
        errors.compare(
            name,
            &[got.decay_s.unwrap()],
            &[number(&expected["decay_s"])],
            Comparison::Exact,
        );
    }
    errors.finish("s2r_band_t60_synthetic_exact");
}

#[test]
fn room_tuning2_s2r_band_t60_demo() {
    let fixture = fixture_r("band_t60_demo");
    let mut errors = Errors::default();
    for (case, input) in fixture["inputs"].as_object().unwrap() {
        let speaker = input["speaker"].as_str().unwrap();
        let position = input["position"].as_u64().unwrap() as usize;
        let impulse = points(speaker)[position].data.clone();
        for (band, low, high) in upper_band_cases() {
            let got = primitives::band_decay(&impulse, FS, low, high);
            let expected = &fixture["outputs"][case][band];
            assert_eq!(
                got.i5,
                expected["i5"].as_u64().unwrap() as usize,
                "{case}/{band}/i5"
            );
            assert_eq!(
                got.i25,
                expected["i25"].as_u64().unwrap() as usize,
                "{case}/{band}/i25"
            );
            match (got.decay_s, expected["decay_s"].as_f64()) {
                (Some(got), Some(expected)) => errors.compare(
                    &format!("{case}/{band}"),
                    &[got],
                    &[expected],
                    Comparison::Rebuilt,
                ),
                (None, None) => (),
                _ => panic!("{case}/{band}: decay presence"),
            }
        }
    }
    errors.finish("s2r_band_t60_demo_rebuilt");
}

#[test]
fn room_tuning2_s2r_upper_rule() {
    let fixture = fixture_r("upper_rule");
    let got: Vec<_> = fixture["inputs"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            automatic_high(
                row["strict"].as_f64(),
                row["band"].as_f64(),
                row["factor"].as_u64().unwrap() as u32,
            )
        })
        .collect();
    let mut errors = Errors::default();
    errors.compare(
        "upper_rule",
        &got,
        &array(&fixture["outputs"]["hz"]),
        Comparison::Exact,
    );
    errors.finish("s2r_upper_rule_exact");
}

#[test]
fn room_tuning2_s2r_upper_demo() {
    let all_points: Vec<_> = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]
        .into_iter()
        .map(points)
        .collect();
    let refs: Vec<_> = all_points.iter().map(Vec::as_slice).collect();
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("upper_demo_v") {
        let volume = number(&fixture["inputs"]["room_volume_m3"]);
        let evidence = lenient_band_evidence(&refs, volume);
        for (got, expected) in evidence
            .iter()
            .zip(fixture["outputs"]["lenient_centres"].as_array().unwrap())
        {
            errors.compare(
                &format!("{name}/lenient/{}", got.centre_hz),
                &[got.centre_hz, got.median_t60_s.unwrap(), got.g_hz.unwrap()],
                &[
                    number(&expected["centre_hz"]),
                    number(&expected["median_t60_s"]),
                    number(&expected["g_hz"]),
                ],
                Comparison::Rebuilt,
            );
        }
        let choice = &fixture["outputs"]["lenient_choice"];
        let expected_lenient = choice["centre_hz"].as_f64().unwrap_or(0.0);
        let expected_lenient_t60 = choice["median_t60_s"].as_f64().unwrap_or(0.0);
        for (basis, fixture_key) in [("strict", "strict"), ("lenient", "lenient")] {
            for transition in [1_u32, 2, 4] {
                let opts = RcOptions {
                    room_volume: Some(volume),
                    high_basis: basis.into(),
                    high_transition: transition,
                    ..options()
                };
                let band = correction_band(&refs, &opts).unwrap();
                errors.compare(
                    &format!("{name}/{basis}/{transition}"),
                    &[
                        band.schroeder.unwrap(),
                        band.lenient.unwrap_or(0.0),
                        band.lenient_t60_s.unwrap_or(0.0),
                        band.high,
                    ],
                    &[
                        number(&fixture["outputs"]["strict"]),
                        if basis == "lenient" {
                            expected_lenient
                        } else {
                            0.0
                        },
                        if basis == "lenient" {
                            expected_lenient_t60
                        } else {
                            0.0
                        },
                        number(&fixture["outputs"]["rc_hi"][fixture_key][transition.to_string()]),
                    ],
                    Comparison::Rebuilt,
                );
            }
        }
    }
    errors.finish("s2r_upper_demo_rebuilt");
}

#[test]
fn room_tuning2_s2r_hp8() {
    let fixture = fixture_r("hp8");
    let mut errors = Errors::default();
    errors.compare(
        "hp8/coefficients",
        &primitives::hp8_coefficients(),
        &array(&fixture["outputs"]["coefficients"]),
        Comparison::Exact,
    );
    let frequencies = grid();
    for cutoff in [10.0, 25.0, 27.0, 50.0] {
        let response = primitives::hp8(&frequencies, cutoff);
        let got = sampled_complex(&response);
        let expected = complex(&fixture["outputs"]["response"][cutoff.to_string()]);
        errors.compare(
            &format!("hp8/{cutoff}"),
            &got.iter()
                .flat_map(|value| [value.re, value.im])
                .collect::<Vec<_>>(),
            &expected
                .iter()
                .flat_map(|value| [value.re, value.im])
                .collect::<Vec<_>>(),
            Comparison::Exact,
        );
        let probes = [
            cutoff,
            cutoff / 2_f64.sqrt(),
            cutoff / 2.0,
            cutoff * 2_f64.sqrt(),
        ];
        let magnitude_db: Vec<_> = primitives::hp8(&probes, cutoff)
            .iter()
            .map(|value| 20.0 * value.norm().log10())
            .collect();
        errors.compare(
            &format!("hp8/{cutoff}/magnitude_db"),
            &magnitude_db,
            &array(&fixture["outputs"]["magnitude_db"][cutoff.to_string()]),
            Comparison::Exact,
        );
    }
    let full = primitives::hp8(&frequencies, 25.0);
    let expected_full = complex(&fixture["outputs"]["response"]["25_full"]);
    errors.compare(
        "hp8/25_full",
        &full
            .iter()
            .flat_map(|value| [value.re, value.im])
            .collect::<Vec<_>>(),
        &expected_full
            .iter()
            .flat_map(|value| [value.re, value.im])
            .collect::<Vec<_>>(),
        Comparison::Exact,
    );
    errors.finish("s2r_hp8_exact");
}

fn fixture_pair_responses(fixture: &Value) -> (Vec<f64>, Vec<PairPhaseResponse>) {
    let frequencies = if fixture["inputs"].get("f").is_some() {
        array(&fixture["inputs"]["f"])
    } else {
        array(&fixture["inputs"]["band_f"])
    };
    let responses = fixture["inputs"]["responses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|response| {
            let values = complex(&response["response"]);
            PairPhaseResponse {
                speaker: response["role"].as_str().unwrap().to_owned(),
                left: values.clone(),
                right: values,
            }
        })
        .collect();
    (frequencies, responses)
}

fn flatten_phase_bands(values: &[PairPhaseBand]) -> Vec<f64> {
    values
        .iter()
        .flat_map(|band| [band.low_hz, band.high_hz, band.phase_deg, band.loss_db])
        .collect()
}

#[test]
fn room_tuning2_s2r_pair_phase() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("pair_phase_") {
        let (frequencies, responses) = fixture_pair_responses(&fixture);
        let expected_pairs = fixture["outputs"]["bands"].as_array().unwrap();
        let mut pair_index = 0;
        for a in 0..responses.len() {
            for b in a + 1..responses.len() {
                let expected = &expected_pairs[pair_index];
                assert_eq!(responses[a].speaker, expected["a"]);
                assert_eq!(responses[b].speaker, expected["b"]);
                errors.compare(
                    &format!("{name}/bands/{a}/{b}"),
                    &flatten_phase_bands(&pair_phase_bands(
                        &frequencies,
                        &responses[a].left,
                        &responses[b].left,
                    )),
                    &expected["bands"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(array)
                        .collect::<Vec<_>>(),
                    Comparison::Exact,
                );
                pair_index += 1;
            }
        }
        let got = pair_phase_report(&frequencies, &responses);
        let expected = fixture["outputs"]["notes"].as_array().unwrap();
        assert_eq!(got.len(), expected.len(), "{name}/notes length");
        for (index, (got, expected)) in got.iter().zip(expected).enumerate() {
            assert_eq!(got.a, expected["a"], "{name}/notes/{index}/a");
            assert_eq!(got.b, expected["b"], "{name}/notes/{index}/b");
            errors.compare(
                &format!("{name}/notes/{index}"),
                &flatten_phase_bands(&got.left),
                &expected["bands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(array)
                    .collect::<Vec<_>>(),
                Comparison::Exact,
            );
        }
    }
    errors.finish("s2r_pair_phase_exact");
}

#[test]
fn room_tuning2_s2r_rc_upper_mixed() {
    let mut errors = Errors::default();
    let mut report = RcReportOnly::default();
    let mut seen = BTreeSet::new();
    for (name, fixture) in fixtures_r("rc_") {
        if !name.ends_with("_upper_mixed") {
            continue;
        }
        let speaker = fixture["inputs"]["speaker"].as_str().unwrap();
        assert!(
            seen.insert(speaker.to_owned()),
            "duplicate upper RC case {speaker}"
        );
        let speaker_points = points(speaker);
        let opts = RcOptions {
            room_volume: Some(number(&fixture["inputs"]["settings"]["room_volume_m3"])),
            high_basis: "lenient".into(),
            high_transition: 2,
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        errors.compare(
            &format!("{name}/limit"),
            &[
                band.high,
                band.schroeder.unwrap(),
                band.lenient.unwrap(),
                band.lenient_t60_s.unwrap(),
            ],
            &[
                number(&fixture["outputs"]["rc_hi"]),
                number(&fixture["outputs"]["rc_limit"]["schroeder_hz"]),
                number(&fixture["outputs"]["rc_limit"]["band_hz"]),
                number(&fixture["outputs"]["rc_limit"]["band_t60_s"]),
            ],
            Comparison::Rebuilt,
        );
        let result = design_rc(&speaker_points, &opts, &band, None, None).unwrap();
        errors.compare(
            &format!("{name}/design"),
            &sampled(&result.design_db),
            &array(&fixture["outputs"]["design_db"]),
            Comparison::Rebuilt,
        );
        match (result.rolloff_hz, fixture["outputs"]["rolloff_hz"].as_f64()) {
            (Some(got), Some(expected)) => errors.compare(
                &format!("{name}/rolloff"),
                &[got],
                &[expected],
                Comparison::Rebuilt,
            ),
            (None, None) => (),
            _ => panic!("{name}: rolloff presence differs"),
        }
        compare_rc_notes(
            &name,
            &actual_rc_notes(speaker, &opts, &result),
            fixture["outputs"]["notes"].as_array().unwrap(),
            &mut errors,
        );
        let expected_peq = bands(&fixture["outputs"]["peq"]);
        for (got, expected) in result.peq.iter().zip(&expected_peq) {
            report.peq_f = report.peq_f.max((got.0 - expected.0).abs());
            report.peq_q = report.peq_q.max((got.1 - expected.1).abs());
            report.peq_gain = report.peq_gain.max((got.2 - expected.2).abs());
        }
        report.fir = report.fir.max(
            result
                .fir
                .iter()
                .zip(array(&fixture["outputs"]["fir"]))
                .map(|(got, expected)| (got - expected).abs())
                .fold(0.0, f64::max),
        );
        let expected_response = complex(&fixture["outputs"]["response"]);
        report.response_db = report.response_db.max(
            sampled_complex(&result.response)
                .iter()
                .zip(expected_response)
                .map(|(got, expected)| {
                    (20.0 * (got.norm() + 1e-12).log10() - 20.0 * (expected.norm() + 1e-12).log10())
                        .abs()
                })
                .fold(0.0, f64::max),
        );
        report.phase_strength = report
            .phase_strength
            .max((result.phase_strength - number(&fixture["outputs"]["phase_strength"])).abs());
    }
    assert_eq!(
        seen,
        ["BL", "BR", "FC", "FL", "FR", "SL", "SR"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "upper RC family must contain all seven speakers"
    );
    println!(
        "REPORT s2r_rc_upper_mixed: peq_f={:.9e} peq_q={:.9e} peq_gain={:.9e} fir={:.9e} response_db={:.9e} phase_strength={:.9e}",
        report.peq_f,
        report.peq_q,
        report.peq_gain,
        report.fir,
        report.response_db,
        report.phase_strength
    );
    errors.finish("s2r_rc_upper_mixed_rebuilt");
}

fn mimo_fixture_paths(setting: &str, input: &str) -> (Vec<Vec<f64>>, Vec<Vec<Vec<f64>>>) {
    let source = fixture(&format!("mimo_{setting}_{input}"));
    let outputs = &source["outputs"];
    let x = outputs["x"].as_array().unwrap().iter().map(array).collect();
    let g = outputs["g"]
        .as_array()
        .unwrap()
        .iter()
        .map(|ear| {
            array(ear)
                .chunks(720)
                .map(|path| path.to_vec())
                .collect::<Vec<_>>()
        })
        .collect();
    (x, g)
}

fn mimo_convolution(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; a.len() + b.len() - 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            result[i + j] += a * b;
        }
    }
    result
}

fn optional_fixture_number(value: &Value) -> Option<f64> {
    (!value.is_null()).then(|| number(value))
}

#[test]
fn room_tuning2_s2r_mimo_early_samples() {
    let fixture = fixture_r("mimo_early_samples");
    let mut errors = Errors::default();
    for (index, row) in fixture["outputs"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let row = row.as_array().unwrap();
        let got = mimo::early_samples(number(&row[0]));
        let expected = row[1].as_u64().unwrap() as usize;
        assert_eq!(got, expected, "row {index}");
        errors.compare(
            &format!("mimo_early_samples/{index}"),
            &[got as f64],
            &[expected as f64],
            Comparison::Exact,
        );
    }
    errors.finish("s2r_mimo_early_samples");
}

#[test]
fn room_tuning2_s2r_mimo_seat_change() {
    let fixture = fixture_r("mimo_seat_change");
    let mut errors = Errors::default();
    for (index, row) in fixture["outputs"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let row = row.as_array().unwrap();
        let got = mimo::seat_change(number(&row[0]), number(&row[1]));
        let expected = optional_fixture_number(&row[2]);
        match (got, expected) {
            (Some(got), Some(expected)) => errors.compare(
                &format!("mimo_seat_change/{index}"),
                &[got],
                &[expected],
                Comparison::Exact,
            ),
            (None, None) => (),
            _ => panic!("mimo_seat_change/{index}: presence differs"),
        }
    }
    errors.finish("s2r_mimo_seat_change");
}

#[test]
fn room_tuning2_s2r_mimo_normal_equations() {
    let mut errors = Errors::default();
    for (name, fixture) in fixtures_r("mimo_normal_") {
        let settings = &fixture["meta"]["settings"];
        let setting = settings["setting"].as_str().unwrap();
        let input = settings["input"].as_str().unwrap();
        let early = settings["early_samples"].as_u64().unwrap() as usize;
        let (x, g) = mimo_fixture_paths(setting, input);
        let lp = array(&fixture["inputs"]["lp"]);
        let c: Vec<Vec<Vec<f64>>> = g
            .iter()
            .map(|ear| {
                ear.iter()
                    .map(|path| {
                        let mut value = mimo_convolution(path, &lp);
                        value.truncate(720);
                        value
                    })
                    .collect()
            })
            .collect();
        let (normal, rhs) = mimo::normal_equations_with(&x, &c, early).unwrap();
        let m = rhs.len();
        errors.compare(
            &format!("{name}/diagonal"),
            &(0..m).map(|i| normal[(i, i)]).collect::<Vec<_>>(),
            &array(&fixture["outputs"]["diagonal"]),
            Comparison::Curve,
        );
        let candidates = m / 180;
        let mut rows = Vec::new();
        for j in 0..candidates {
            for jj in 0..candidates {
                for l in 0..180 {
                    rows.push(normal[(j * 180, jj * 180 + l)]);
                }
            }
        }
        errors.compare(
            &format!("{name}/block_first_rows"),
            &rows,
            &array(&fixture["outputs"]["block_first_rows"]),
            Comparison::Curve,
        );
        errors.compare(
            &format!("{name}/Atb"),
            &rhs,
            &array(&fixture["outputs"]["Atb"]),
            Comparison::Curve,
        );
        let frobenius = normal
            .col_iter()
            .flat_map(|column| column.iter())
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt();
        let trace = (0..m).map(|i| normal[(i, i)]).sum::<f64>();
        errors.compare(
            &format!("{name}/norms"),
            &[frobenius, trace],
            &[
                number(&fixture["outputs"]["frobenius"]),
                number(&fixture["outputs"]["trace"]),
            ],
            Comparison::Curve,
        );
    }
    errors.finish("s2r_mimo_normal_equations");
}

fn compare_optional_hz(name: &str, got: Option<f64>, expected: &Value) {
    match (got, optional_fixture_number(expected)) {
        (Some(got), Some(expected)) => assert_eq!(got, expected, "{name}"),
        (None, None) => (),
        _ => panic!("{name}: presence differs"),
    }
}

#[test]
fn room_tuning2_s2r_mimo_guard() {
    let mut errors = Errors::default();
    let mut energy_relative: f64 = 0.0;
    let mut energy_count = 0;
    for (name, fixture) in fixtures_r("mimo_guard_") {
        let input = &fixture["inputs"];
        let x = vec![array(&input["x0"]), array(&input["x1"])];
        let y = vec![array(&input["y0"]), array(&input["y1"])];
        let fsiso = number(&input["fsiso"]);
        let stored_energy = &fixture["outputs"]["seat_on"]["band_energies"];
        let stored_bands: Vec<_> = stored_energy["bands_hz"]
            .as_array()
            .unwrap()
            .iter()
            .map(|band| (number(&band[0]), number(&band[1])))
            .collect();
        let frequencies = primitives::grid(1440, 600.0);
        for (ear, (before, after)) in x.iter().zip(&y).enumerate() {
            let peak = before.iter().map(|value| value.abs()).fold(0.0, f64::max);
            let first = before
                .iter()
                .position(|value| value.abs() > 0.1 * peak)
                .unwrap_or(0);
            let end = (first + 36).min(before.len());
            let mut before = before.clone();
            let mut after = after.clone();
            before[..end].fill(0.0);
            after[..end].fill(0.0);
            let before = primitives::rfft(&before, 1440);
            let after = primitives::rfft(&after, 1440);
            for (label, spectrum, expected) in [
                ("before", &before, &stored_energy["before"][ear]),
                ("after", &after, &stored_energy["after"][ear]),
            ] {
                let got: Vec<f64> = stored_bands
                    .iter()
                    .map(|(low, high)| {
                        frequencies
                            .iter()
                            .zip(spectrum)
                            .filter(|(frequency, _)| **frequency >= *low && **frequency < *high)
                            .map(|(_, value)| value.norm_sqr())
                            .sum()
                    })
                    .collect();
                // Relative 1e-12 on every platform: the helper's foreign-libm
                // widening does not apply to these plain sums of |rfft|².
                let expected = array(expected);
                assert_eq!(got.len(), expected.len(), "{name}/band_energy shape");
                for (band, (g, e)) in got.iter().zip(&expected).enumerate() {
                    let error = (g - e).abs();
                    assert!(
                        error <= 1e-12 * e.abs() + 1e-30,
                        "{name}/band_energy/{ear}/{label}[{band}]: got={g:e} expected={e:e}"
                    );
                    energy_relative = energy_relative.max(error / e.abs().max(1e-30));
                    energy_count += 1;
                }
            }
        }
        let stored = |key: &str| -> Vec<Vec<f64>> {
            (0..2).map(|ear| array(&stored_energy[key][ear])).collect()
        };
        let (before, after) = (stored("before"), stored("after"));
        for (label, enabled) in [("seat_off", false), ("seat_on", true)] {
            let got = mimo::guard_evaluation(&x, &y, fsiso, enabled);
            let expected = &fixture["outputs"][label];
            let expected_band_db = number(&expected["band"]["db"]);
            errors.compare(
                &format!("{name}/{label}/values"),
                &[got.band_db, got.tail_db],
                &[expected_band_db, number(&expected["tail"]["db"])],
                Comparison::Curve,
            );
            // The band choice is the strict-replacement maximum. Rebuild every
            // candidate from the oracle's own band energies: when the oracle's
            // maximum is unique by more than the value budget the choice must be
            // the oracle's exactly; when several (ear, band) candidates are equal
            // within rounding (a uniformly scaled late part changes every band by
            // the same dB) the last bits decide, and the choice must be one of them.
            let mut candidates: Vec<(Option<usize>, (f64, f64), f64)> = vec![];
            for (m, &band) in stored_bands.iter().enumerate() {
                if enabled {
                    for ear in 0..2 {
                        if let Some(value) = mimo::seat_change(before[ear][m], after[ear][m]) {
                            candidates.push((Some(ear), band, value));
                        }
                    }
                } else {
                    let a = before[0][m] + before[1][m];
                    let b = after[0][m] + after[1][m];
                    if a > 0.0 {
                        candidates.push((None, band, 10.0 * ((b + 1e-30) / a).log10()));
                    }
                }
            }
            if let Some(top) = candidates.iter().map(|c| c.2).reduce(f64::max)
                && top > -99.0
            {
                assert!(
                    (top - expected_band_db).abs() <= 1e-9,
                    "{name}/{label}: stored energies give {top} dB, oracle {expected_band_db} dB"
                );
            }
            let expected_choice = (
                expected["band"]["position"].as_u64().map(|v| v as usize),
                expected["band"]["hz"]
                    .as_array()
                    .map(|v| (number(&v[0]), number(&v[1]))),
            );
            let got_choice = (got.band_ear, got.band_hz);
            let tied: Vec<_> = candidates
                .iter()
                .filter(|c| c.2 >= expected_band_db - 1e-9)
                .map(|c| (c.0, Some(c.1)))
                .collect();
            if tied.len() <= 1 {
                assert_eq!(got_choice, expected_choice, "{name}/{label}/band choice");
            } else {
                assert!(
                    tied.contains(&got_choice) && tied.contains(&expected_choice),
                    "{name}/{label}: {got_choice:?} is not among the {} candidates tied at the maximum",
                    tied.len()
                );
                println!(
                    "REPORT {name}/{label}/band: {} candidates tied within 1e-9 dB; got {got_choice:?}, oracle {expected_choice:?}",
                    tied.len()
                );
            }
            assert_eq!(
                got.mode_hz,
                optional_fixture_number(&expected["tail"]["hz"]),
                "{name}/{label}/mode_hz"
            );
            assert_eq!(
                got.tail_ear,
                expected["tail"]["position"].as_u64().map(|v| v as usize),
                "{name}/{label}/tail_ear"
            );
        }
    }
    errors.finish("s2r_mimo_guard");
    println!(
        "MEASURE s2r_mimo_guard_band_energies: max_relative={energy_relative:e}, values={energy_count}, budget=1e-12 relative on every platform"
    );
}

#[test]
fn room_tuning2_s2r_mimo_design() {
    let mut solve = NumericErrors::default();
    let mut guards = NumericErrors::default();
    let mut supports = NumericErrors::default();
    let mut cases = 0;
    for (name, fixture) in fixtures_r("mimo_design_") {
        cases += 1;
        let settings = &fixture["meta"]["settings"];
        let setting = settings["setting"].as_str().unwrap();
        let input = settings["input"].as_str().unwrap();
        let fsiso = number(&settings["fsiso"]);
        let early = fixture["outputs"]["early_samples"].as_u64().unwrap() as usize;
        let ear_guard = settings["seat_guard"].as_bool().unwrap();
        let candidates: Vec<_> = fixture["inputs"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| (value.as_str().unwrap().to_owned(), 50.0))
            .collect();
        let (x, g) = mimo_fixture_paths(setting, input);
        let (result, stages) = mimo::design_paths_with(
            input,
            &candidates,
            x,
            g,
            array(&fixture["inputs"]["lp"]),
            number(&fixture["inputs"]["direct_energy"]),
            fsiso,
            mimo::MimoRule { early, ear_guard },
        )
        .unwrap();
        let trajectory = fixture["outputs"]["trajectory"].as_array().unwrap();
        assert_eq!(
            stages.logs.len(),
            trajectory.len(),
            "{name}/trajectory length"
        );
        for (index, (log, ratio)) in stages.logs.iter().zip(&stages.ratios).enumerate() {
            assert_eq!(
                *log,
                number(&trajectory[index]["mid"]),
                "{name}/log/{index}"
            );
            assert_eq!(
                *ratio <= 10_f64.powf(-1.2),
                trajectory[index]["accepted"].as_bool().unwrap(),
                "{name}/decision/{index}"
            );
        }
        solve.compare(
            &format!("{name}/ratios"),
            &stages.ratios,
            &trajectory
                .iter()
                .map(|row| number(&row["ratio"]))
                .collect::<Vec<_>>(),
            3e-9,
            1e-12,
        );
        assert_eq!(
            stages.logs[stages.kept],
            number(&fixture["outputs"]["log_lambda"]),
            "{name}/kept"
        );
        solve.compare(
            &format!("{name}/kept_ratio"),
            &[stages.ratios[stages.kept]],
            &[number(&fixture["outputs"]["ratio"])],
            3e-9,
            1e-12,
        );
        let expected_guards = fixture["outputs"]["guards"].as_array().unwrap();
        assert_eq!(
            stages.guard_steps.len(),
            expected_guards.len(),
            "{name}/guards"
        );
        for (index, (got, expected)) in stages.guard_steps.iter().zip(expected_guards).enumerate() {
            let expected_scale = number(&expected["scale"]);
            let db = [0.0, -3.0, -6.0, -9.0, -12.0][index];
            let got_scale = 10_f64.powf(db / 20.0);
            assert!(
                (got_scale - expected_scale).abs() <= 1e-14 * expected_scale.abs().max(1.0),
                "{name}/scale/{index}"
            );
            guards.compare(
                &format!("{name}/guard/{index}"),
                &[got.band_db, got.tail_db],
                &[number(&expected["band_db"]), number(&expected["tail_db"])],
                0.0,
                1e-9,
            );
            assert_eq!(
                got.band_hz,
                expected["band_hz"]
                    .as_array()
                    .map(|v| (number(&v[0]), number(&v[1]))),
                "{name}/band_hz/{index}"
            );
            compare_optional_hz(
                &format!("{name}/mode_hz/{index}"),
                got.mode_hz,
                &expected["mode_hz"],
            );
            assert_eq!(
                got.band_ear,
                expected["band_position"].as_u64().map(|v| v as usize),
                "{name}/band_ear/{index}"
            );
            assert_eq!(
                got.tail_ear,
                expected["tail_position"].as_u64().map(|v| v as usize),
                "{name}/tail_ear/{index}"
            );
            assert_eq!(
                got.band_db <= 1.0 && got.tail_db <= 1.0,
                expected["passes"].as_bool().unwrap(),
                "{name}/passes/{index}"
            );
        }
        let expected_scale = number(&fixture["outputs"]["guard_scale"]);
        let expected_step = (expected_scale > 0.0).then(|| expected_guards.len() - 1);
        let actual_step = result.scale_db.map(|db| (-db / 3.0).round() as usize);
        assert_eq!(actual_step, expected_step, "{name}/accepted step");
        let expected_names: Vec<_> = fixture["outputs"]["support_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            result
                .supports
                .iter()
                .map(|v| v.speaker.as_str())
                .collect::<Vec<_>>(),
            expected_names,
            "{name}/support names"
        );
        let expected_supports = array(&fixture["outputs"]["supports"]);
        let actual_supports: Vec<_> = result
            .supports
            .iter()
            .flat_map(|support| support.fir.iter().copied())
            .collect();
        supports.compare(
            &format!("{name}/supports"),
            &actual_supports,
            &expected_supports,
            1e-9,
            1e-12,
        );
        supports.compare(
            &format!("{name}/level"),
            &[result.level_db],
            &[number(&fixture["outputs"]["level_db"])],
            0.0,
            1e-9,
        );
        if ear_guard && !fixture["outputs"]["guard_cause"].is_null() {
            let got = result.ear_limit.as_ref().expect("ear limit");
            let expected = &fixture["outputs"]["guard_cause"];
            assert_eq!(
                (-got.scale_db / 3.0).round(),
                (-number(&expected["rejected_db"]) / 3.0).round(),
                "{name}/limit step"
            );
            match (&got.band, expected["band"].as_array()) {
                (Some(got), Some(expected_band)) => {
                    assert_eq!(
                        got.ear,
                        expected["band_position"].as_u64().unwrap() as usize
                    );
                    guards.compare(
                        &format!("{name}/limit band"),
                        &[got.lo, got.hi, got.db],
                        &array(&Value::Array(expected_band.clone())),
                        0.0,
                        1e-9,
                    );
                }
                (None, None) => (),
                _ => panic!("{name}/limit band presence"),
            }
            match (&got.tail, expected["tail"].as_array()) {
                (Some(got), Some(expected_tail)) => {
                    assert_eq!(
                        got.ear,
                        expected["tail_position"].as_u64().unwrap() as usize
                    );
                    guards.compare(
                        &format!("{name}/limit tail"),
                        &[got.hz, got.db],
                        &array(&Value::Array(expected_tail.clone())),
                        0.0,
                        1e-9,
                    );
                }
                (None, None) => (),
                _ => panic!("{name}/limit tail presence"),
            }
        } else {
            assert!(result.ear_limit.is_none(), "{name}/ear limit off");
        }
    }
    assert_eq!(cases, 70);
    solve.finish("s2r_mimo_design_solve");
    guards.finish("s2r_mimo_design_guard");
    supports.finish("s2r_mimo_design_supports");
}
