#![forbid(unsafe_code)]
//! Clean-room gates for the SECS 2R (`s2r_*`) fixtures from ADR 0015 bundle 1.
mod golden_secs2_helpers;
use golden_secs2_helpers::{array, bands, complex, fixture_r, fixtures_r, number};
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft::Complex64,
    hrir::Hrir,
    ir::ImpulseResponse,
    stages::{room, room_tuning2::*},
};
use impulcifer_io::read_wav;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

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

#[test]
fn room_tuning2_s2r_libm_probe() {
    let same = oracle_libm_r();
    println!("SECS 2R oracle libm match: {same}");
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
            (_, kind) => panic!("{label}: actual {actual:?}, expected kind {kind}"),
        }
    }
}

#[test]
fn room_tuning2_bands_matches_secs2r() {
    let mut errors = Errors::default();
    let mut reports: BTreeMap<String, RcReportOnly> = BTreeMap::new();
    for (name, fixture) in fixtures_r("rc_") {
        let speaker = fixture["meta"]["speaker"].as_str().unwrap();
        let setting = fixture["meta"]["setting"].as_str().unwrap();
        let settings = &fixture["inputs"]["settings"];
        let extras = pairs(&settings["rc_bands"]);
        let speaker_points = points(speaker);
        let opts = RcOptions {
            dip_limit: settings["dip_limit"].as_bool().unwrap(),
            low_protect: settings["low_protect"].as_bool().unwrap(),
            extra_bands: extras,
            ..options()
        };
        let band = correction_band(&[&speaker_points], &opts).unwrap();
        let result = design_rc(&speaker_points, &opts, &band, None, None).unwrap();
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
