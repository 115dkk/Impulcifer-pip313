#![forbid(unsafe_code)]
mod brir_support;
#[allow(dead_code)]
#[path = "../src/brir/plots.rs"]
mod plot_data;
use brir_support::*;
use impulcifer_dsp::{
    DspError,
    fr::FrequencyResponse,
    hrir::Hrir,
    pipeline::{StageObserver, StageProgress, run_pipeline},
};
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{
    BrirError, BrirEvents, Catalog, discovery::discover, estimator::open_estimator,
    inputs::load_inputs, run::run_brir,
};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobKind, JobStatus},
    stages::StageKey,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
fn oracle() -> Value {
    serde_json::from_slice(&std::fs::read(golden("p19_plots.json")).unwrap()).unwrap()
}
fn values(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
struct Quiet;
impl BrirEvents for Quiet {
    fn step(&mut self, _: &str, _: Value) -> Result<(), BrirError> {
        Ok(())
    }
    fn log(&mut self, _: &str, _: &str, _: Value) {}
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
fn config(t: &Temp, plot: bool) -> ProcessingConfig {
    ProcessingConfig {
        dir_path: Some(t.0.to_string_lossy().into_owned()),
        test_signal: Some(
            root()
                .join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
                .to_string_lossy()
                .into_owned(),
        ),
        plot,
        ..Default::default()
    }
}
fn inputs(t: &Temp, c: &ProcessingConfig) -> impulcifer_dsp::pipeline::PipelineInputs {
    let d = discover(&t.0, c).unwrap();
    let e = open_estimator(&d, c.test_signal.as_deref()).unwrap();
    load_inputs(&d, &e, c, &mut Quiet).unwrap()
}
fn compare(actual: &[f64], expected: &[f64], budget: f64, label: &str) -> f64 {
    assert_eq!(actual.len(), expected.len(), "{label} shape");
    let max = actual
        .iter()
        .zip(expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("{label}: max_abs={max:.12e}, budget={budget:.12e}");
    assert!(actual.iter().all(|v| v.is_finite()));
    assert!(max <= budget, "{label}: {max} > {budget}");
    max
}
#[derive(Default)]
struct Capture {
    results: Option<(impulcifer_plots::FrSeries, impulcifer_plots::FrSeries)>,
    rates: Vec<(StageKey, u32)>,
}
impl StageObserver for Capture {
    fn on_stage(&mut self, _: StageProgress) {}
    fn check_cancelled(&self) -> Result<(), DspError> {
        Ok(())
    }
    fn on_plot(&mut self, key: StageKey, hrir: &Hrir) -> Result<(), DspError> {
        self.rates.push((key, hrir.fs));
        if key == StageKey::PlotResults {
            self.results = Some(plot_data::results_series(hrir)?);
        }
        Ok(())
    }
}
#[test]
fn results_from_python_ir_meet_strict_fr_budget() {
    let o = oracle();
    let lines = &o["runs"]["default"]["series"]["results.png"][0]["lines"];
    // Strict P09 gate isolates plotting preparation from upstream FIR oracle noise.
    for (side, raw_index, smooth_index) in [("left", 0, 2), ("right", 1, 3)] {
        let bytes = std::fs::read(golden(&format!("p19_results_{side}.f64"))).unwrap();
        let ir: Vec<_> = bytes
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let s = plot_data::series(&ir, 48000, true).unwrap();
        compare(
            &s.frequency,
            &values(&lines[raw_index]["x"]),
            0.0,
            &format!("{side} grid"),
        );
        compare(
            &s.raw,
            &values(&lines[raw_index]["y"]),
            0.05,
            &format!("{side} frozen IR raw"),
        );
        let expected = values(&lines[smooth_index]["y"]);
        let max = s
            .smoothed
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        println!("{side} frozen IR smoothed: max_abs={max:.12e}; budget=1e-9+1e-11*abs(reference)");
        assert!(
            s.smoothed
                .iter()
                .zip(&expected)
                .all(|(a, b)| (a - b).abs() <= 1e-9 + 1e-11 * b.abs())
        );
    }
}
#[test]
fn golden_results_series_match_python() {
    let o = oracle();
    let lines = &o["runs"]["default"]["series"]["results.png"][0]["lines"];
    // Downstream P10 budget includes the measured upstream FIR oracle noise.
    let t = Temp::demo();
    let c = config(&t, false);
    let mut capture = Capture::default();
    run_pipeline(&c, inputs(&t, &c), &mut capture).unwrap();
    let (l, r) = capture.results.unwrap();
    let difference: Vec<_> = l
        .smoothed
        .iter()
        .zip(&r.smoothed)
        .map(|(l, r)| l - r)
        .collect();
    compare(
        &difference,
        &values(&lines[4]["y"]),
        0.05,
        "pipeline smoothed difference",
    );
    for (side, s, raw_index, smooth_index) in [("left", l, 0, 2), ("right", r, 1, 3)] {
        compare(
            &s.raw,
            &values(&lines[raw_index]["y"]),
            0.05,
            &format!("{side} pipeline raw"),
        );
        compare(
            &s.frequency,
            &values(&lines[raw_index]["x"]),
            0.0,
            &format!("{side} pipeline grid"),
        );
        compare(
            &s.smoothed,
            &values(&lines[smooth_index]["y"]),
            0.05,
            &format!("{side} pipeline smoothed"),
        );
    }
}
#[test]
fn golden_headphones_series_match_python() {
    let o = oracle();
    let axes = &o["runs"]["default"]["series"]["headphones.png"];
    let t = Temp::demo();
    let c = config(&t, false);
    let hp = inputs(&t, &c).headphone.unwrap();
    for (i, fr) in [(0, &hp.left), (1, &hp.right)] {
        compare(
            &fr.frequency,
            &values(&axes[2]["lines"][i]["x"]),
            0.0,
            "headphone frequency",
        );
        compare(
            &fr.raw,
            &values(&axes[2]["lines"][i]["y"]),
            0.05,
            "headphone raw",
        );
        let expected = axes[2]["legend"][i].as_str().unwrap();
        let side = if i == 0 { "Left" } else { "Right" };
        assert_eq!(
            format!("{side} raw {:+.1} dB", fr.center_value((100.0, 10000.0))),
            expected
        );
    }
    let diff: Vec<_> = hp
        .left
        .raw
        .iter()
        .zip(&hp.right.raw)
        .map(|(l, r)| l - r)
        .collect();
    compare(
        &diff,
        &values(&axes[2]["lines"][2]["y"]),
        0.05,
        "headphone difference",
    );
    let limits = impulcifer_plots::headphones_limits(
        &plot_data::curve(&hp.left),
        &plot_data::curve(&hp.right),
    )
    .unwrap();
    for i in 0..3 {
        compare(
            &[limits.0, limits.1],
            &values(&axes[i]["ylim"]),
            0.05,
            "headphone limits",
        );
    }
}
#[test]
fn golden_eq_series_match_python() {
    let o = oracle();
    for (name, case) in o["eq_cases"].as_object().unwrap() {
        let t = Temp::demo();
        for (filename, text) in case["files"].as_object().unwrap() {
            std::fs::write(t.0.join(filename), text.as_str().unwrap()).unwrap();
        }
        let c = config(&t, false);
        let input = inputs(&t, &c);
        let left = input.eq_left.unwrap();
        let right = input.eq_right.unwrap();
        let axes = case["axes"].as_array().unwrap();
        for (i, fr) in [(0, &left), (if axes.len() == 1 { 0 } else { 1 }, &right)] {
            let lines = axes[i]["lines"].as_array().unwrap();
            let expected = lines.iter().find(|l| l["label"] == "Raw").unwrap();
            compare(
                &fr.frequency,
                &values(&expected["x"]),
                0.0,
                &format!("eq {name} grid"),
            );
            compare(
                &fr.raw,
                &values(&expected["y"]),
                0.05,
                &format!("eq {name} raw"),
            );
            let expected = lines.iter().find(|l| l["label"] == "Error").unwrap();
            compare(
                &fr.error,
                &values(&expected["y"]),
                0.05,
                &format!("eq {name} error"),
            );
        }
        assert_eq!(
            png_dimensions(&t.0.join("plots/eq.png")),
            if axes.len() == 1 {
                (1200, 900)
            } else {
                (2200, 900)
            }
        );
    }
}
fn png_dimensions(path: &Path) -> (u32, u32) {
    let b = std::fs::read(path).unwrap();
    assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(b[16..20].try_into().unwrap()),
        u32::from_be_bytes(b[20..24].try_into().unwrap()),
    )
}
fn png_set(dir: &Path, root: &Path, out: &mut BTreeMap<String, (u32, u32)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            png_set(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "png") {
            out.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                png_dimensions(&path),
            );
        }
    }
}
struct Product {
    pngs: BTreeMap<String, (u32, u32)>,
    wav: Vec<u8>,
    readme: String,
}
fn product(plot: bool) -> &'static Product {
    static DEFAULT: OnceLock<Product> = OnceLock::new();
    static PLOT: OnceLock<Product> = OnceLock::new();
    (if plot { &PLOT } else { &DEFAULT }).get_or_init(|| {
        let t = Temp::demo();
        let c = config(&t, plot);
        let jobs = JobRegistry::new();
        let mut catalog = Catalog::english();
        catalog.readme_date = Some("2026-09-08 12:00:00".into());
        let job = jobs
            .start(JobKind::Brir, true, move |ctx| {
                run_brir(&c, &catalog, ctx).map(|r| json!({"output_path":r.output_path}))
            })
            .unwrap();
        let p = wait(&jobs, &job.job_id);
        assert_eq!(p.job.status, JobStatus::Succeeded, "{:?}", p.job.error);
        let mut pngs = BTreeMap::new();
        png_set(&t.0, &t.0, &mut pngs);
        let out = Product {
            pngs,
            wav: std::fs::read(t.0.join("hesuvi.wav")).unwrap(),
            readme: std::fs::read_to_string(t.0.join("README.md")).unwrap(),
        };
        println!("P19_SERVICE_DIRECTORY {}", t.0.display());
        // Preserve temp-only evidence for visual inspection, never data/ outputs.
        std::mem::forget(t);
        out
    })
}
fn compare_files(plot: bool) {
    let p = product(plot);
    let o = oracle();
    let expected = o["runs"][if plot { "plot" } else { "default" }]["pngs"]
        .as_object()
        .unwrap();
    assert_eq!(
        p.pngs.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    for (name, &(w, h)) in &p.pngs {
        let size = values(&expected[name]["size"]);
        let canvas = values(&expected[name]["canvas"]);
        println!(
            "{name}: Python cropped {}x{} canvas {}x{} Rust {w}x{h}",
            size[0], size[1], canvas[0], canvas[1]
        );
        assert_eq!(
            (f64::from(w), f64::from(h)),
            (canvas[0], canvas[1]),
            "{name}"
        );
    }
}
#[test]
fn plot_files_match_python_default_run() {
    compare_files(false);
}
#[test]
fn plot_files_match_python_plot_run() {
    compare_files(true);
}
#[test]
fn generic_only_room_plot_matches_python_canvas() {
    let t = Temp::new();
    // The canonical demo has no room.wav. Reuse an unchanged measured room
    // recording under the generic filename, identically to the Python oracle.
    for (source, target) in [
        ("FL,FR.wav", "FL,FR.wav"),
        ("headphones.wav", "headphones.wav"),
        ("room-FL,FR-left.wav", "room.wav"),
    ] {
        std::fs::copy(root().join("data/demo").join(source), t.0.join(target)).unwrap();
    }
    let c = config(&t, true);
    let jobs = JobRegistry::new();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            run_brir(&c, &Catalog::english(), ctx).map(|r| json!({"output_path":r.output_path}))
        })
        .unwrap();
    let poll = wait(&jobs, &job.job_id);
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let size = png_dimensions(&t.0.join("plots/room/room.png"));
    let o = oracle();
    let canvas = values(&o["generic_room"]["canvas"]);
    assert_eq!(size, (canvas[0] as u32, canvas[1] as u32));
    assert_eq!(size, (1500, 900));
    assert_eq!(
        std::fs::read_dir(t.0.join("plots/room")).unwrap().count(),
        1
    );
    println!("P19_GENERIC_ROOM_DIRECTORY {}", t.0.display());
    std::mem::forget(t);
}
#[test]
fn plots_do_not_change_wavs() {
    assert_eq!(product(false).wav, product(true).wav);
    assert_eq!(product(false).readme, product(true).readme);
}
#[test]
fn plot_observer_sees_pre_resample_state_and_propagates_failure() {
    let t = Temp::demo();
    let mut c = config(&t, true);
    c.fs = Some(44100);
    let mut capture = Capture::default();
    let input = inputs(&t, &c);
    let output = run_pipeline(&c, input.clone(), &mut capture).unwrap();
    assert_eq!(output.hrir.fs, 44100);
    assert_eq!(
        capture.rates,
        vec![
            (StageKey::PlotPre, 48000),
            (StageKey::PlotPost, 48000),
            (StageKey::PlotResults, 48000),
            (StageKey::PlotAdditional, 48000)
        ]
    );
    struct Fail;
    impl StageObserver for Fail {
        fn on_stage(&mut self, _: StageProgress) {}
        fn check_cancelled(&self) -> Result<(), DspError> {
            Ok(())
        }
        fn on_plot(&mut self, _: StageKey, _: &Hrir) -> Result<(), DspError> {
            Err(DspError::InvalidArgument("intentional plot failure".into()))
        }
    }
    assert!(
        run_pipeline(&c, input, &mut Fail)
            .unwrap_err()
            .to_string()
            .contains("intentional plot failure")
    );
}
#[test]
fn panel_limits_match_python_two_pass() {
    let o = oracle();
    let mut prepared = Vec::new();
    let mut expected = Vec::new();
    for case in o["panel_cases"].as_array().unwrap() {
        let ir = impulcifer_dsp::ir::ImpulseResponse {
            data: values(&case["data"]),
            fs: case["fs"].as_u64().unwrap() as u32,
            recording: Some(values(&case["recording"])),
        };
        prepared.push(
            plot_data::prepare_panels(&ir, "synthetic".into(), ir.recording.clone()).unwrap(),
        );
        let mut limits = impulcifer_plots::PanelLimits::default();
        for (i, axis) in case["limits"].as_array().unwrap().iter().enumerate() {
            let x = values(&axis["x"]);
            let y = values(&axis["y"]);
            limits.0[i] = Some(impulcifer_plots::AxisLimits {
                x: (x[0], x[1]),
                y: (y[0], y[1]),
            });
        }
        expected.push(limits);
    }
    let actual = impulcifer_plots::PanelLimits::synchronize(prepared.iter().map(|p| &p.limits));
    let expected = impulcifer_plots::PanelLimits::synchronize(&expected);
    for i in 0..6 {
        let a = actual.0[i].unwrap();
        let b = expected.0[i].unwrap();
        compare(
            &[a.x.0, a.x.1, a.y.0, a.y.1],
            &[b.x.0, b.x.1, b.y.0, b.y.1],
            1e-7,
            &format!("synchronized panel {i}"),
        );
    }
}
#[test]
fn smoothing_from_python_raw_meets_fr_budget() {
    let o = oracle();
    let lines = &o["runs"]["default"]["series"]["results.png"][0]["lines"];
    for (raw, smooth) in [(0, 2), (1, 3)] {
        let mut fr = FrequencyResponse::new(
            "oracle",
            Some(values(&lines[raw]["x"])),
            Some(values(&lines[raw]["y"])),
        )
        .unwrap();
        fr.smoothen(1.0 / 3.0, 1.0 / 5.0, 20000.0, 23999.0).unwrap();
        let expected = values(&lines[smooth]["y"]);
        let max = fr
            .smoothed
            .iter()
            .zip(&expected)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        println!("smoothing isolated max_abs={max:.12e}");
        assert!(
            fr.smoothed
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() <= 1e-9 + 1e-11 * b.abs())
        );
    }
}
