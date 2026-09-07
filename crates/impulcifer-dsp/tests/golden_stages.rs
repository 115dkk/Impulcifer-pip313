#![forbid(unsafe_code)]
//! Actual Python stage fixtures, core/pipeline.py:424-972; p10_*.
use impulcifer_dsp::{
    DspError,
    channel_balance::{ChannelBalance, channel_balance_firs, correct_channel_balance},
    estimator::SweepEstimator,
    fr::{FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    mic_deviation::{MicDeviationOptions, MicMatching, apply_mic_deviation_correction},
    pipeline::{self, PipelineInputs, StageObserver, StageProgress},
    stages::{
        eq_files::*,
        equalize::*,
        headphone::*,
        readme::{ReverbKind, readme_data},
        room::*,
        target::create_target,
    },
    virtual_bass::{VirtualBassOptions, apply_virtual_bass},
};
use impulcifer_types::{config::ProcessingConfig, constants::*};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::OnceLock};
thread_local! { static FAILURES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; }
/// Collect all numeric failures without relaxing assertions; core/pipeline.py:424-972; p10_*.
struct NumericGate;
impl Drop for NumericGate {
    /// Assert all measured budgets after the scenario; core/pipeline.py:424-972; p10_*.
    fn drop(&mut self) {
        let failures = FAILURES.with(|errors| std::mem::take(&mut *errors.borrow_mut()));
        if !std::thread::panicking() {
            assert!(failures.is_empty(), "{}", failures.join("\n"));
        }
    }
}
/// Python fixture storage; export_goldens_stages.py save; p10_manifest.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
/// Python fixture storage; export_goldens_stages.py save; p10_manifest.
fn file(name: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join("tests/migration/goldens").join(name)).unwrap())
        .unwrap()
}
/// Python fixture storage; export_goldens_stages.py save; p10_manifest.
fn fixture(name: &str) -> Value {
    file(&format!("p10_{name}.json"))["outputs"].clone()
}
/// Budgets for quantities downstream of the minimum-phase EQ FIRs. That FIR
/// chain is noise-limited in Python itself (tests/migration/README-fr.md), and
/// `golden_frozen_eq_firs_isolate_downstream_noise` shows that with Python's
/// own FIRs these same quantities match to 1e-13 dB and exact integers. The
/// values below are the observed propagation of the FIR noise (applied gain
/// 2.7e-6 dB default / 1.7e-4 dB with bass boost or tilt; PNR 0.028 dB; knee
/// index shifted by 2 samples; window level 0.004 dB) with a margin of about 5.
const GAIN_ATOL_DB: f64 = 1e-3;
const README_DB_ATOL: f64 = 0.1;
const README_MS_ATOL: f64 = 0.5;
const REVERB_MS_ATOL: f64 = 2.0;
const KNEE_SAMPLES_ATOL: f64 = 16.0;
const DECAY_LEVEL_ATOL_DB: f64 = 0.02;

/// Python nonfinite serialization; core/pipeline_stages.py:577-626; p10_readme.
fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| match v.as_str().unwrap() {
        "nan" => f64::NAN,
        "+inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        _ => panic!("invalid number"),
    })
}
/// Python f64 sidecars; export_goldens_stages.py encode; p10_eq_firs.
fn array(v: &Value) -> Vec<f64> {
    if let Some(a) = v.as_array() {
        a.iter().map(number).collect()
    } else {
        let b = fs::read(
            root()
                .join("tests/migration/goldens")
                .join(v["file"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(b.len(), v["length"].as_u64().unwrap() as usize * 8);
        b.chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect()
    }
}
/// Python numerical outputs; core/pipeline.py:424-972; all p10 fixtures.
fn compare(name: &str, a: &[f64], b: &[f64], atol: f64, rtol: f64) {
    assert_eq!(a.len(), b.len(), "{name} length");
    let mut max = 0.0_f64;
    let mut failure = None;
    for (i, (&a, &b)) in a.iter().zip(b).enumerate() {
        if a == b || (a.is_nan() && b.is_nan()) {
            continue;
        }
        let e = if a.is_finite() && b.is_finite() {
            (a - b).abs()
        } else {
            f64::INFINITY
        };
        max = max.max(e);
        if (!e.is_finite() || e > atol + rtol * b.abs()) && failure.is_none() {
            failure = Some((i, a, b, e));
        }
    }
    println!("MEASURE {name}: max_abs={max:.17e} atol={atol:.3e} rtol={rtol:.3e}");
    if let Some(failure) = failure {
        FAILURES.with(|errors| errors.borrow_mut().push(format!("{name}: {failure:?}")));
    }
}
/// Python FR dB outputs; core/room_correction.py:185-292; p10_room.
fn db(name: &str, a: &[f64], v: &Value) {
    compare(name, a, &array(v), 1e-9, 1e-11);
}
/// Python minimum-phase noise envelope; parallel_workers.py:129; p10_eq_firs.
fn fir(name: &str, a: &[f64], v: &Value, fs: u32) {
    let b = array(v);
    let peak = b.iter().map(|v| v.abs()).fold(0.0, f64::max);
    compare(name, a, &b, 1e-3 * peak, 0.0);
    let n = (a.len() * 4).max(8192);
    let mut aa = a.to_vec();
    let mut bb = b;
    aa.resize(n, 0.0);
    bb.resize(n, 0.0);
    let aa = impulcifer_dsp::fft::rfft(&aa);
    let bb = impulcifer_dsp::fft::rfft(&bb);
    let mut maxima = [0.0_f64; 4];
    for (i, (a, b)) in aa.iter().zip(bb).enumerate() {
        let f = i as f64 * fs as f64 / n as f64;
        let band = if f < 20000.0 {
            0
        } else if f < 23000.0 {
            1
        } else if f < 23900.0 {
            2
        } else {
            3
        };
        let delta = if a.norm() == b.norm() {
            0.0
        } else if a.norm() == 0.0 || b.norm() == 0.0 {
            f64::INFINITY
        } else {
            (20.0 * (a.norm() / b.norm()).log10()).abs()
        };
        maxima[band] = maxima[band].max(delta);
    }
    println!("MEASURE {name} spectrum: {maxima:?}");
    for (e, t) in maxima.into_iter().zip([2e-2, 5e-2, 0.3, 20.0]) {
        if !e.is_finite() || e > t {
            FAILURES.with(|errors| errors.borrow_mut().push(format!("{name} spectrum {e}>{t}")));
        }
    }
}
/// Python track summaries; core/hrir.py:427-565; p10_default_final.
fn summary(name: &str, x: &[f64], v: &Value, budget: f64) {
    assert_eq!(
        x.len(),
        v["length"].as_u64().unwrap() as usize,
        "{name} length"
    );
    let atol = budget * number(&v["max_abs"]);
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
    let peak = x.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let rms = (x.iter().map(|x| x * x).sum::<f64>() / x.len() as f64).sqrt();
    compare(
        &format!("{name} metrics"),
        &[peak, rms],
        &[number(&v["max_abs"]), number(&v["rms"])],
        atol,
        0.0,
    );
    let argmax = x
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()).then_with(|| b.0.cmp(&a.0)))
        .map(|(i, _)| i)
        .unwrap_or(0);
    assert_eq!(
        argmax,
        v["argmax"].as_u64().unwrap() as usize,
        "{name} argmax"
    );
    if v.get("samples").is_some() {
        compare(&format!("{name} full"), x, &array(&v["samples"]), atol, 0.0);
        if budget == 1e-3 {
            fir(
                &format!("{name} full spectrum"),
                x,
                &v["samples"],
                if name.contains("resample") {
                    44100
                } else {
                    48000
                },
            );
        }
    }
}
/// Python snapshots; core/hrir.py:368-938; p10_default_crop/equalize.
fn snapshots(h: &Hrir, files: &Value, budget: f64) {
    for name in files.as_array().unwrap() {
        let name = name.as_str().unwrap();
        let v = file(name)["outputs"].clone();
        let s = h.get(v["speaker"].as_str().unwrap()).unwrap();
        let ir = if v["side"] == "left" {
            s.left.as_ref()
        } else {
            s.right.as_ref()
        }
        .unwrap();
        assert_eq!(
            ir.peak_index(0, None, 0.12589),
            v["peak_index"].as_u64().unwrap() as usize,
            "{name} peak_index"
        );
        summary(name, &ir.data, &v, budget);
    }
}
/// Python from_wav/open_recording/room/headphone; pipeline_stages.py:353-522; p10_protocol.
fn inputs() -> &'static PipelineInputs {
    static INPUTS: OnceLock<PipelineInputs> = OnceLock::new();
    INPUTS.get_or_init(|| {
        let protocol = fixture("protocol");
        let w = impulcifer_io::wav::read_wav(&root().join(protocol["sweep"].as_str().unwrap()))
            .unwrap();
        let estimator = SweepEstimator::from_samples(w.sample_rate, &w.tracks[0]).unwrap();
        let mut hrir = Hrir {
            fs: estimator.fs,
            speakers: Vec::new(),
        };
        for n in protocol["recordings"].as_array().unwrap() {
            let n = n.as_str().unwrap();
            let w = impulcifer_io::wav::read_wav(&root().join("data/demo").join(n)).unwrap();
            hrir.open_recording_samples(
                &estimator,
                w.sample_rate,
                &w.tracks,
                &n.trim_end_matches(".wav").split(',').collect::<Vec<_>>(),
                None,
                2.0,
            )
            .unwrap();
        }
        let mut rir = Hrir {
            fs: estimator.fs,
            speakers: Vec::new(),
        };
        for n in protocol["room_recordings"].as_array().unwrap() {
            let n = n.as_str().unwrap();
            let m = parse_room_measurement_name(n).unwrap();
            let w = impulcifer_io::wav::read_wav(&root().join("data/demo").join(n)).unwrap();
            rir.open_recording_samples(
                &estimator,
                w.sample_rate,
                &w.tracks,
                &m.speakers.iter().map(String::as_str).collect::<Vec<_>>(),
                m.side,
                2.0,
            )
            .unwrap();
        }
        let target = room_target();
        let mic = calibration();
        let room = room_correction(
            &mut rir,
            &[],
            &target,
            Some(&mic),
            &estimator,
            &RoomCorrectionOptions {
                fr_combination_method: FrCombination::Average,
                specific_limit: 400.0,
                generic_limit: 300.0,
            },
        )
        .unwrap();
        let mut hp = Hrir {
            fs: estimator.fs,
            speakers: Vec::new(),
        };
        let w = impulcifer_io::wav::read_wav(&root().join("data/demo/headphones.wav")).unwrap();
        hp.open_recording_samples(
            &estimator,
            w.sample_rate,
            &w.tracks,
            &["FL", "FR"],
            None,
            2.0,
        )
        .unwrap();
        PipelineInputs {
            estimator,
            hrir,
            room,
            headphone: Some(headphone_compensation(&hp).unwrap()),
            eq_left: None,
            eq_right: None,
        }
    })
}
/// Python _open_room_target; core/room_correction.py:431-442; p10_room.
fn room_target() -> FrequencyResponse {
    let text = fs::read_to_string(root().join("data/demo/room-target.csv"))
        .unwrap()
        .replace("\r\n", "\n");
    let csv = FrequencyResponse::parse_csv("room-target", &text).unwrap();
    prepare_room_target(Some(&csv), 48000).unwrap()
}
/// Python _open_mic_calibration; core/room_correction.py:450-461; p10_room.
fn calibration() -> FrequencyResponse {
    let text = fs::read_to_string(root().join("data/demo/room-mic-calibration.txt"))
        .unwrap()
        .replace("\r\n", "\n");
    prepare_mic_calibration(&FrequencyResponse::parse_csv("mic", &text).unwrap(), 48000).unwrap()
}
/// Python crop-and-align; core/pipeline.py:593-609; p10_default_crop.
fn cropped() -> Hrir {
    let mut h = inputs().hrir.clone();
    h.crop_heads(1.0).unwrap();
    h.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
    h.align_onset_groups_peak_leftref(None).unwrap();
    h.crop_tails(&inputs().estimator).unwrap();
    h
}
/// Python EqInputs composition; core/parallel_workers.py:69-131; p10_eq_firs.
fn eq_inputs<'a>(target: &'a FrequencyResponse) -> EqInputs<'a> {
    EqInputs {
        room_frs: inputs().room.as_ref().map(|r| &r.frs),
        hp: inputs().headphone.as_ref(),
        eq_left: None,
        eq_right: None,
        target,
        fs: 48000,
    }
}
/// Python equalize; core/pipeline.py:655-700; p10_default_equalize.
fn equalized() -> Hrir {
    let mut h = cropped();
    let t = create_target(48000, 0.0, 105.0, 0.76, 0.0);
    equalize_hrir(&mut h, &eq_inputs(&t)).unwrap();
    h
}
/// Python stage observer; core/pipeline.py:487-509; p10_stage_table.
#[derive(Default)]
struct Observer(Vec<StageProgress>);
impl StageObserver for Observer {
    /// Python logger.step; core/pipeline.py:516-972; p10_stage_table.
    fn on_stage(&mut self, p: StageProgress) {
        self.0.push(p);
    }
    /// Python check_cancelled; core/pipeline.py:505-509; p10_stage_table.
    fn check_cancelled(&self) -> Result<(), DspError> {
        Ok(())
    }
}
/// Python reports enabled stages including zero-step and file/plot rows; pipeline.py:424-509; p10_stage_table.
#[test]
fn pipeline_observer_reports_order_steps_and_totals() {
    for config in [
        ProcessingConfig::default(),
        ProcessingConfig {
            microphone_deviation_correction: true,
            plot: true,
            interactive_plots: true,
            fs: Some(44100),
            jamesdsp: true,
            hangloose: true,
            output_truehd_layouts: true,
            ..Default::default()
        },
    ] {
        let mut observer = Observer::default();
        let out = pipeline::run_pipeline(&config, inputs().clone(), &mut observer).unwrap();
        let expected: Vec<_> = pipeline::stage_table(&config)
            .into_iter()
            .filter(|(_, enabled, _)| *enabled)
            .collect();
        assert_eq!(observer.0.len(), expected.len());
        let mut step = 0;
        for (got, (key, _, increment)) in observer.0.iter().zip(expected) {
            step += increment;
            assert_eq!(got.key, key);
            assert_eq!(got.step, step);
            assert_eq!(got.total, pipeline::total_steps(&config));
        }
        assert_eq!(step, pipeline::total_steps(&config));
        assert_eq!(out.readme.fs, config.fs.unwrap_or(48000));
        assert_eq!(out.readme.applied_gain_db, out.applied_gain_db);
        // README numbers must be captured before the resampling stage.
        let mut before = equalized();
        let gain = before.normalize(Some(-0.1), None).unwrap();
        let readme = readme_data(&before, out.readme.fs, gain);
        for (a, b) in out.readme.rows.iter().zip(readme.rows) {
            assert_eq!(a.pnr_db, b.pnr_db);
            assert_eq!(a.length_ms, b.length_ms);
            assert_eq!(a.reverb, b.reverb);
        }
    }
}
/// Python cancellation between enabled stages; pipeline.py:505-509; p10_stage_table.
#[test]
fn pipeline_observer_cancels_before_and_after_each_stage() {
    struct CancelAt {
        checks: std::cell::Cell<usize>,
        at: usize,
        events: Vec<StageProgress>,
    }
    impl StageObserver for CancelAt {
        /// Python progress notification; pipeline.py:516-972; p10_stage_table.
        fn on_stage(&mut self, p: StageProgress) {
            self.events.push(p);
        }
        /// Python cancellation propagation; pipeline.py:505-509; p10_stage_table.
        fn check_cancelled(&self) -> Result<(), DspError> {
            let check = self.checks.get();
            self.checks.set(check + 1);
            if check == self.at {
                Err(DspError::InvalidArgument("test cancellation".into()))
            } else {
                Ok(())
            }
        }
    }
    let config = ProcessingConfig::default();
    let keys: Vec<_> = pipeline::stage_table(&config)
        .into_iter()
        .filter(|(_, e, _)| *e)
        .map(|(k, _, _)| k)
        .collect();
    for at in 0..keys.len() * 2 {
        let mut observer = CancelAt {
            checks: std::cell::Cell::new(0),
            at,
            events: vec![],
        };
        let error = pipeline::run_pipeline(&config, inputs().clone(), &mut observer).unwrap_err();
        assert_eq!(
            error.to_string(),
            DspError::InvalidArgument("test cancellation".into()).to_string()
        );
        assert_eq!(observer.checks.get(), at + 1);
        let emitted = at.div_ceil(2);
        assert_eq!(
            observer.events.iter().map(|p| p.key).collect::<Vec<_>>(),
            keys[..emitted]
        );
    }
}
/// Python room FRs; core/room_correction.py:185-210; p10_room.
fn check_room(frs: &RoomFrs, files: &Value) {
    for name in files.as_array().unwrap() {
        let name = name.as_str().unwrap();
        let v = file(name)["outputs"].clone();
        let side = if v["side"] == "left" {
            Side::Left
        } else {
            Side::Right
        };
        let (_, _, fr) = frs
            .0
            .iter()
            .find(|(s, e, _)| s == v["speaker"].as_str().unwrap() && *e == side)
            .unwrap();
        db(name, &fr.error, &v["error"]);
        db(name, &fr.target, &v["target"]);
    }
}
/// Python final write_wav ordering; core/pipeline.py:873-895; p10_default_final.
fn check_final(h: &Hrir, hrir: &[Vec<f64>], hesuvi: &[Vec<f64>], v: &Value) {
    snapshots(h, &v["tracks"], 1e-3);
    for (name, tracks, order) in [
        ("hrir", hrir, HEXADECAGONAL_TRACK_ORDER.as_slice()),
        ("hesuvi", hesuvi, HESUVI_TRACK_ORDER.as_slice()),
    ] {
        let refs = v["layouts"][name].as_array().unwrap();
        assert_eq!(tracks.len(), refs.len());
        for (i, (track, reference)) in tracks.iter().zip(refs).enumerate() {
            let reference = file(reference.as_str().unwrap())["outputs"].clone();
            assert_eq!(reference["name"], order[i]);
            summary(&format!("{name} {i}"), track, &reference, 1e-3);
        }
    }
}
/// Python default pipeline; core/pipeline.py:424-895; p10_default_*.
#[test]
fn golden_default_pipeline_matches_python() {
    let _gate = NumericGate;
    let manifest = fixture("manifest");
    check_room(&inputs().room.as_ref().unwrap().frs, &manifest["room"]);
    let tracks = &inputs().room.as_ref().unwrap().responses_tracks;
    let expected_tracks = manifest["room_response_tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), expected_tracks.len());
    for (track, reference) in tracks.iter().zip(expected_tracks) {
        let name = reference.as_str().unwrap();
        summary(name, track, &file(name)["outputs"], 1e-9);
    }
    let hp = inputs().headphone.as_ref().unwrap();
    let expected = fixture("headphone");
    db("hp left", &hp.left.error, &expected["left"]);
    db("hp right", &hp.right.error, &expected["right"]);
    let target = create_target(48000, 0.0, 105.0, 0.76, 0.0);
    db("target", &target.raw, &fixture("target"));
    for name in manifest["firs"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        let v = file(name)["outputs"].clone();
        let side = if v["side"] == "left" {
            Side::Left
        } else {
            Side::Right
        };
        let got =
            equalization_fir(&eq_inputs(&target), v["speaker"].as_str().unwrap(), side).unwrap();
        fir(name, &got, &v["fir"], 48000);
    }
    snapshots(&cropped(), &manifest["crop"], 1e-9);
    snapshots(&equalized(), &manifest["equalize"], 1e-3);
    let mut observer = Observer::default();
    let output = pipeline::run_pipeline(
        &ProcessingConfig::default(),
        inputs().clone(),
        &mut observer,
    )
    .unwrap();
    compare(
        "applied gain",
        &[output.applied_gain_db],
        &[number(&fixture("gain"))],
        GAIN_ATOL_DB,
        0.0,
    );
    snapshots(&output.hrir, &manifest["normalize"], 1e-3);
    check_final(
        &output.hrir,
        &output.hrir_tracks,
        &output.hesuvi_tracks,
        &manifest["default"],
    );
    let expected = fixture("readme");
    check_readme(&output.readme, &expected);
}
/// Python write_readme numeric state at its actual stage; pipeline_stages.py:525-687; p10_readme.
fn check_readme(got: &impulcifer_dsp::stages::readme::ReadmeData, expected: &Value) {
    assert_eq!(got.fs as u64, expected["fs"].as_u64().unwrap());
    compare(
        "readme applied gain",
        &[got.applied_gain_db],
        &[number(&expected["applied_gain"])],
        GAIN_ATOL_DB,
        0.0,
    );
    assert_eq!(got.rows.len(), expected["rows"].as_array().unwrap().len());
    let expected_keys: std::collections::BTreeSet<_> = expected["reflections"]
        .as_object()
        .unwrap()
        .iter()
        .flat_map(|(speaker, pair)| {
            pair.as_object()
                .unwrap()
                .keys()
                .map(move |side| (speaker.as_str(), side.as_str()))
        })
        .collect();
    let actual_keys: std::collections::BTreeSet<_> = got
        .reflections
        .iter()
        .map(|(speaker, side, _)| {
            (
                speaker.as_str(),
                if *side == Side::Left { "left" } else { "right" },
            )
        })
        .collect();
    assert_eq!(got.reflections.len(), expected_keys.len());
    assert_eq!(actual_keys, expected_keys);
    let label = |kind| match kind {
        ReverbKind::Rt60 => "RT60",
        ReverbKind::Rt30 => "RT30",
        ReverbKind::Rt20 => "RT20",
        ReverbKind::Edt => "EDT",
        ReverbKind::Rtxx => "RTxx",
    };
    assert_eq!(label(got.reverb_header), expected["reverb_header"]);
    for (speaker, side, levels) in &got.reflections {
        let side = if *side == Side::Left { "left" } else { "right" };
        let v = &expected["reflections"][speaker][side];
        compare(
            &format!("readme reflection {speaker} {side}"),
            &[levels.early_db, levels.late_db],
            &[number(&v["early_db"]), number(&v["late_db"])],
            README_DB_ATOL,
            0.0,
        );
    }
    for (row, v) in got.rows.iter().zip(expected["rows"].as_array().unwrap()) {
        assert_eq!(
            if row.side == Side::Left {
                "left"
            } else {
                "right"
            },
            v["side"]
        );
        assert_eq!(row.speaker, v["speaker"].as_str().unwrap());
        compare(
            "readme pnr",
            &[row.pnr_db],
            &[number(&v["pnr_db"])],
            README_DB_ATOL,
            0.0,
        );
        compare(
            "readme length",
            &[row.length_ms.unwrap_or(f64::NAN)],
            &[number(&v["length_ms"])],
            README_MS_ATOL,
            0.0,
        );
        assert_eq!(row.itd_us, number(&v["itd_us"]));
        assert_eq!(
            row.reverb.is_some(),
            !v["reverb"].is_null(),
            "{} reverb presence",
            row.speaker
        );
        if let Some((kind, t)) = row.reverb {
            let label = match kind {
                ReverbKind::Rt60 => "RT60",
                ReverbKind::Rt30 => "RT30",
                ReverbKind::Rt20 => "RT20",
                ReverbKind::Edt => "EDT",
                ReverbKind::Rtxx => "RTxx",
            };
            assert_eq!(label, v["reverb"]);
            compare(
                "readme reverb",
                &[t],
                &[number(&v["reverb_ms"])],
                REVERB_MS_ATOL,
                0.0,
            );
        }
    }
}
/// Isolate existing minimum-phase composition from stage arithmetic;
/// core/pipeline.py:698-743; p10_eq_fir_*/p10_gain.
#[test]
fn golden_frozen_eq_firs_isolate_downstream_noise() {
    let _gate = NumericGate;
    let manifest = fixture("manifest");
    let mut h = cropped();
    for name in manifest["firs"].as_array().unwrap() {
        let v = file(name.as_str().unwrap())["outputs"].clone();
        let pair = h.get_mut(v["speaker"].as_str().unwrap()).unwrap();
        let ir = if v["side"] == "left" {
            &mut pair.left
        } else {
            &mut pair.right
        };
        ir.as_mut().unwrap().equalize(&array(&v["fir"]));
    }
    let expected = fixture("decay_params");
    for speaker in ["FL", "FR"] {
        let pair = h.pair(speaker).unwrap();
        for (side, ir) in [("left", pair.0), ("right", pair.1)] {
            let got = ir.decay_adjustment_params(0.3).unwrap();
            let reference = &expected[speaker][side];
            compare(
                "frozen FIR decay decisions",
                &[
                    got.window_start as f64,
                    got.half_window as f64,
                    got.knee_point_index as f64,
                ],
                &[
                    number(&reference[0]),
                    number(&reference[1]),
                    number(&reference[2]),
                ],
                0.0,
                0.0,
            );
            compare(
                "frozen FIR decay level",
                &[got.window_level],
                &[number(&reference[3])],
                1e-6,
                0.0,
            );
        }
    }
    let gain = h.normalize(Some(-0.1), None).unwrap();
    compare(
        "frozen FIR normalize",
        &[gain],
        &[number(&fixture("gain"))],
        1e-6,
        0.0,
    );
    let got = readme_data(&h, 48000, gain);
    let expected = fixture("readme");
    for (row, v) in got.rows.iter().zip(expected["rows"].as_array().unwrap()) {
        compare(
            "frozen FIR readme",
            &[row.pnr_db, row.length_ms.unwrap_or(f64::NAN)],
            &[number(&v["pnr_db"]), number(&v["length_ms"])],
            1e-6,
            0.0,
        );
        if let Some((_, t)) = row.reverb {
            compare(
                "frozen FIR reverb",
                &[t],
                &[number(&v["reverb_ms"])],
                1e-6,
                0.0,
            );
        }
    }
}
/// Python virtual bass; core/virtual_bass.py:82-198; p10_vbass.
#[test]
fn golden_vbass_matches_python() {
    let _gate = NumericGate;
    let manifest = fixture("manifest");
    let mut h = cropped();
    apply_virtual_bass(
        &mut h,
        &VirtualBassOptions {
            crossover_freq: 250,
            head_ms: 1.0,
            hp_freq: 15.0,
            invert_polarity: None,
        },
    )
    .unwrap();
    snapshots(&h, &manifest["vbass"], 1e-9);
    let out = pipeline::run_pipeline(
        &ProcessingConfig {
            vbass: true,
            ..Default::default()
        },
        inputs().clone(),
        &mut Observer::default(),
    )
    .unwrap();
    check_final(
        &out.hrir,
        &out.hrir_tracks,
        &out.hesuvi_tracks,
        &manifest["vbass_final"],
    );
}
/// Python channel balance; core/hrir.py:674-818; p10_channel_balance.
#[test]
fn golden_channel_balance_matches_python() {
    let _gate = NumericGate;
    let manifest = fixture("manifest");
    for method in ["trend", "left", "right", "avg", "min", "mids", "3"] {
        let mut h = equalized();
        let mut frs = Vec::new();
        for left in [true, false] {
            let pair = h.pair("FL").unwrap();
            let other = h.pair("FR").unwrap();
            let (a, b) = if left {
                (pair.0, other.0)
            } else {
                (pair.1, other.1)
            };
            let data: Vec<_> = a
                .data
                .iter()
                .zip(&b.data)
                .map(|(a, b)| (a + b) / 2.0)
                .collect();
            frs.push(magnitude_to_frequency_response("Frequency response", h.fs, &data).unwrap());
        }
        let mut right = frs.pop().unwrap();
        let mut left = frs.pop().unwrap();
        let p = ChannelBalance::parse(method).unwrap();
        let got = channel_balance_firs(&mut left, &mut right, p, h.fs).unwrap();
        let v = fixture(&format!("channel_balance_{method}_firs"));
        for (i, side) in ["left", "right"].iter().enumerate() {
            fir(
                &format!("balance {method} {side}"),
                &got[i],
                &v[side],
                48000,
            );
        }
        correct_channel_balance(&mut h, p).unwrap();
        snapshots(&h, &manifest["channel_balance"][method], 1e-3);
    }
}
/// Python mic matching; core/microphone_deviation_correction.py:57-454; p10_mic_deviation.
#[test]
fn golden_mic_deviation_matches_python() {
    let _gate = NumericGate;
    let mut h = cropped();
    let options = MicDeviationOptions::default();
    let mut matching = MicMatching::new(h.fs, &options);
    for s in &h.speakers {
        let (l, r) = h.pair(&s.speaker).unwrap();
        matching.collect_speaker(
            &s.speaker,
            &l.data,
            &r.data,
            Some(l.peak_index(0, None, 0.12589)),
            Some(r.peak_index(0, None, 0.12589)),
        );
    }
    let v = fixture("mic_deviation");
    db(
        "mic mismatch",
        matching.estimate_interaural_mismatch().unwrap(),
        &v["mismatch_db"],
    );
    let got = matching.design_correction_filters().unwrap();
    for (i, side) in ["left", "right"].iter().enumerate() {
        fir(side, &got[i], &v[side], 48000);
    }
    let summary = matching.analysis_summary().unwrap();
    compare(
        "mic summary",
        &[summary.avg_error_db, summary.max_error_db],
        &[
            number(&v["summary"]["avg_error_db"]),
            number(&v["summary"]["max_error_db"]),
        ],
        1e-9,
        1e-11,
    );
    assert_eq!(summary.anchor, v["summary"]["anchor"].as_str().unwrap());
    apply_mic_deviation_correction(&mut h, &options).unwrap();
    snapshots(&h, &fixture("manifest")["mic"], 1e-3);
}
/// Python decay stage; core/pipeline.py:702-724; p10_decay.
#[test]
fn golden_decay_matches_python() {
    let _gate = NumericGate;
    let mut h = equalized();
    let v = fixture("decay_params");
    for s in ["FL", "FR"] {
        let pair = h.pair(s).unwrap();
        for (side, ir) in [("left", pair.0), ("right", pair.1)] {
            let got = ir.decay_adjustment_params(0.3);
            if let Some(got) = got {
                let reference = &v[s][side];
                assert_eq!(
                    got.window_start as f64,
                    number(&reference[0]),
                    "{s} {side} window_start"
                );
                compare(
                    &format!("decay {s} {side} knee decisions"),
                    &[got.half_window as f64, got.knee_point_index as f64],
                    &[number(&reference[1]), number(&reference[2])],
                    KNEE_SAMPLES_ATOL,
                    0.0,
                );
                compare(
                    "decay level",
                    &[got.window_level],
                    &[number(&reference[3])],
                    DECAY_LEVEL_ATOL_DB,
                    0.0,
                );
            } else {
                assert!(v[s][side].is_null());
            }
        }
    }
    impulcifer_dsp::stages::decay::adjust_decay(&mut h, &[("FL".into(), 0.3), ("FR".into(), 0.3)])
        .unwrap();
    snapshots(&h, &fixture("manifest")["decay"], 1e-3);
}
/// Python resample and second normalization; core/pipeline.py:859-895; p10_resample.
#[test]
fn golden_resample_matches_python() {
    let _gate = NumericGate;
    let out = pipeline::run_pipeline(
        &ProcessingConfig {
            fs: Some(44100),
            ..Default::default()
        },
        inputs().clone(),
        &mut Observer::default(),
    )
    .unwrap();
    check_final(
        &out.hrir,
        &out.hrir_tracks,
        &out.hesuvi_tracks,
        &fixture("manifest")["resample"],
    );
    let mut h = equalized();
    h.normalize(Some(-0.1), None).unwrap();
    h.resample(44100).unwrap();
    compare(
        "second normalization",
        &[h.normalize(Some(-0.1), None).unwrap()],
        &[number(&fixture("resample_gain"))],
        GAIN_ATOL_DB,
        0.0,
    );
}
/// Python option variants; core/pipeline.py:526-743; p10_option_*.
#[test]
fn golden_option_variants_match_python() {
    let _gate = NumericGate;
    let manifest = fixture("manifest");
    for (key, v) in manifest["options"].as_object().unwrap() {
        let config: ProcessingConfig = serde_json::from_value(json!({key:v["value"]})).unwrap();
        let mut input = inputs().clone();
        if key == "specific_limit" {
            let mut rir = Hrir {
                fs: 48000,
                speakers: Vec::new(),
            };
            for name in fixture("protocol")["room_recordings"].as_array().unwrap() {
                let name = name.as_str().unwrap();
                let m = parse_room_measurement_name(name).unwrap();
                let w = impulcifer_io::wav::read_wav(&root().join("data/demo").join(name)).unwrap();
                rir.open_recording_samples(
                    &input.estimator,
                    w.sample_rate,
                    &w.tracks,
                    &m.speakers.iter().map(String::as_str).collect::<Vec<_>>(),
                    m.side,
                    2.0,
                )
                .unwrap();
            }
            input.room = room_correction(
                &mut rir,
                &[],
                &room_target(),
                Some(&calibration()),
                &input.estimator,
                &RoomCorrectionOptions {
                    fr_combination_method: FrCombination::Average,
                    specific_limit: 0.0,
                    generic_limit: 300.0,
                },
            )
            .unwrap();
        }
        check_room(&input.room.as_ref().unwrap().frs, &v["room"]);
        let target = create_target(
            48000,
            config.bass_boost_gain,
            config.bass_boost_fc,
            config.bass_boost_q,
            config.tilt,
        );
        db(
            key,
            &target.raw,
            &file(v["target"].as_str().unwrap())["outputs"],
        );
        let out = pipeline::run_pipeline(&config, input, &mut Observer::default()).unwrap();
        compare(
            &format!("{key} gain"),
            &[out.applied_gain_db],
            &[number(&v["gain"])],
            GAIN_ATOL_DB,
            0.0,
        );
        snapshots(&out.hrir, &v["tracks"], 1e-3);
    }
}
/// Python generic room split/combine; core/room_correction.py:231-386; p10_generic_room.
#[test]
fn golden_generic_room_matches_python() {
    let _gate = NumericGate;
    let e = &inputs().estimator;
    let w = impulcifer_io::wav::read_wav(&root().join("data/demo/FL,FR.wav")).unwrap();
    let n = 4 * w.sample_rate as usize + e.test_signal.len();
    let a = w.tracks[0][..n].to_vec();
    let mut b = a.clone();
    b.rotate_right(17);
    let original = vec![a, b];
    for columns in [1, 2] {
        let mut tracks = original.clone();
        if columns == 2 {
            for track in &mut tracks {
                let mut second = track[2 * w.sample_rate as usize..].to_vec();
                second.rotate_right(31);
                track.extend(second.iter().map(|x| -0.5 * x));
            }
        }
        let irs = split_generic_room_recording(e, w.sample_rate, &tracks).unwrap();
        let split = fixture(&format!("generic_split_{columns}"));
        let boundaries = split["boundaries"].as_array().unwrap();
        assert_eq!(irs.len(), 2 * columns);
        assert_eq!(irs.len(), boundaries.len());
        assert_eq!(irs.len(), split["segments"].as_array().unwrap().len());
        for (i, (ir, boundary)) in irs.iter().zip(boundaries).enumerate() {
            let start = boundary[0].as_u64().unwrap() as usize;
            let end = boundary[1].as_u64().unwrap() as usize;
            // Retained recordings assert the actual split against Python's captured boundaries.
            let retained = ir.recording.as_ref().unwrap();
            assert_eq!(retained, &tracks[i / columns][start..end]);
            let v = file(split["segments"][i].as_str().unwrap())["outputs"].clone();
            summary("generic retained recording", retained, &v["recording"], 0.0);
            summary("generic estimated segment", &ir.data, &v["ir"], 1e-9);
            assert_eq!(
                ir.peak_index(0, None, 0.12589),
                v["peak_index"].as_u64().unwrap() as usize
            );
        }
        for (method, name, limit) in [
            (FrCombination::Average, "average", 300.0),
            (FrCombination::Conservative, "conservative", 300.0),
            (FrCombination::Average, "average", 0.0),
        ] {
            let label = if columns == 1 && limit == 300.0 {
                name.to_string()
            } else {
                format!("{columns}_{name}_{limit:.0}")
            };
            let v = fixture(&format!("generic_room_{label}"));
            assert_eq!(
                json!(irs.iter().map(|ir| ir.len()).collect::<Vec<_>>()),
                v["lengths"]
            );
            assert_eq!(v["boundaries"], split["boundaries"]);
            let got = calculate_generic_room_correction(
                &irs,
                &room_target(),
                Some(&calibration()),
                method,
                limit,
            )
            .unwrap();
            for (name, a) in [
                ("raw", &got.raw),
                ("error", &got.error),
                ("error_smoothed", &got.error_smoothed),
                ("target", &got.target),
            ] {
                db(&format!("generic {label} {name}"), a, &v[name]);
            }
            if limit == 0.0 {
                assert!(
                    got.frequency
                        .iter()
                        .zip(&got.error)
                        .any(|(f, e)| *f > 300.0 && e.abs() > 1e-6)
                );
            }
        }
        if columns == 1 {
            let mut rir = Hrir {
                fs: 48000,
                speakers: vec![],
            };
            for name in fixture("protocol")["room_recordings"].as_array().unwrap() {
                let name = name.as_str().unwrap();
                let measurement = parse_room_measurement_name(name).unwrap();
                let wav =
                    impulcifer_io::wav::read_wav(&root().join("data/demo").join(name)).unwrap();
                rir.open_recording_samples(
                    e,
                    wav.sample_rate,
                    &wav.tracks,
                    &measurement
                        .speakers
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    measurement.side,
                    2.0,
                )
                .unwrap();
            }
            let generic = room_correction(
                &mut rir,
                &irs,
                &room_target(),
                Some(&calibration()),
                e,
                &RoomCorrectionOptions {
                    fr_combination_method: FrCombination::Average,
                    specific_limit: 400.0,
                    generic_limit: 0.0,
                },
            )
            .unwrap()
            .unwrap();
            let expected = fixture("manifest")["generic_limit_room"].clone();
            assert_eq!(generic.frs.0.len(), expected.as_array().unwrap().len());
            check_room(&generic.frs, &expected);
        }
    }
}
/// Python headphone resolution on temporary paths; pipeline_stages.py:369-418; p10_headphone_resolution.
#[test]
fn golden_headphone_resolution_matches_python() {
    let _gate = NumericGate;
    for v in fixture("headphone_resolution").as_array().unwrap() {
        let listing: Vec<_> = v["listing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            resolve_headphone_file(v["requested"].as_str(), &listing).as_deref(),
            v["result"].as_str(),
            "{v}"
        );
    }
}
/// Python plain EQ files; pipeline_stages.py:183-350; p10_eq_files.
#[test]
fn golden_eq_files_match_python() {
    let _gate = NumericGate;
    let v = fixture("eq_files");
    for c in v["csv"].as_array().unwrap() {
        let fr =
            read_eq_settings_csv(c["name"].as_str().unwrap(), c["text"].as_str().unwrap()).unwrap();
        db("csv raw", &fr.raw, &c["raw"]);
        db("csv error", &fr.error, &c["error"]);
    }
    for c in v["detection"].as_array().unwrap() {
        assert_eq!(
            looks_like_eqapo_config(c["text"].as_str().unwrap()),
            c["expected"].as_bool().unwrap()
        );
    }
    assert_eq!(
        read_eq_settings_csv("apo", "Preamp: -3 dB")
            .unwrap_err()
            .to_string(),
        DspError::InvalidArgument("EqualizerAPO configs are not supported yet (P12)".into())
            .to_string()
    );
    let make =
        |n| Some(read_eq_settings_csv("eq", &format!("20.0,{n}.0\n24000.0,{n}.0\n")).unwrap());
    let (l, r) = select_eq_pair(make(1), None, make(2), make(3));
    let (l, r) = finalize_eq(l, r, 48000).unwrap();
    let v = fixture("eq_precedence");
    db("eq left precedence", &l.unwrap().error, &v["left"]);
    db("eq right precedence", &r.unwrap().error, &v["right"]);
}
/// Python optional output stages on synthetic extended speakers; core/pipeline.py:873-972; p10_outputs.
#[test]
fn golden_optional_outputs_match_python() {
    let _gate = NumericGate;
    for case in fixture("outputs").as_array().unwrap() {
        let mut hrir = Hrir {
            fs: 48000,
            speakers: vec![],
        };
        for spec in case["specs"].as_array().unwrap() {
            let speaker = spec["speaker"].as_str().unwrap();
            if hrir.get(speaker).is_none() {
                hrir.speakers.push(impulcifer_dsp::hrir::SpeakerIrs {
                    speaker: speaker.into(),
                    left: None,
                    right: None,
                });
            }
            let mut data: Vec<_> = (0..8192)
                .map(|i| ((i * 37 % 101) as f64 - 50.0) / 50000.0)
                .collect();
            data[48] += 1.0;
            let scale = if spec["silent"].as_bool().unwrap() {
                0.0
            } else {
                number(&spec["scale"])
            };
            data.iter_mut().for_each(|x| *x *= scale);
            let ir = Some(impulcifer_dsp::ir::ImpulseResponse {
                data,
                fs: 48000,
                recording: None,
            });
            let pair = hrir.get_mut(speaker).unwrap();
            if spec["side"] == "left" {
                pair.left = ir;
            } else {
                pair.right = ir;
            }
        }
        let compact = case["compact"].as_bool().unwrap();
        let config = ProcessingConfig {
            do_room_correction: false,
            do_headphone_compensation: false,
            do_equalization: false,
            output_truehd_layouts: true,
            jamesdsp: true,
            hangloose: true,
            remove_silent_channels: compact,
            ..Default::default()
        };
        let out = pipeline::run_pipeline(
            &config,
            PipelineInputs {
                estimator: inputs().estimator.clone(),
                hrir,
                room: None,
                headphone: None,
                eq_left: None,
                eq_right: None,
            },
            &mut Observer::default(),
        )
        .unwrap();
        compare(
            "optional gain",
            &[out.applied_gain_db],
            &[number(&case["applied_gain"])],
            GAIN_ATOL_DB,
            0.0,
        );
        let mut matrices = vec![
            ("hrir.wav".to_string(), &out.hrir_tracks),
            ("hesuvi.wav".to_string(), &out.hesuvi_tracks),
        ];
        for (name, names, tracks) in &out.truehd {
            let order = if name.contains("11ch") {
                TRUEHD_11CH_ORDER.as_slice()
            } else {
                TRUEHD_13CH_ORDER.as_slice()
            };
            let expected: Vec<_> = order
                .iter()
                .filter(|s| out.hrir.get(s).is_some())
                .flat_map(|s| [format!("{s}-left"), format!("{s}-right")])
                .collect();
            assert_eq!(names, &expected);
            matrices.push((name.clone(), tracks));
        }
        matrices.push(("jamesdsp.wav".into(), out.jamesdsp.as_ref().unwrap()));
        for (s, tracks) in &out.hangloose {
            matrices.push((format!("Hangloose/{s}.wav"), tracks));
        }
        let expected = case["matrices"].as_array().unwrap();
        assert_eq!(matrices.len(), expected.len());
        for ((path, tracks), reference) in matrices.iter().zip(expected) {
            assert_eq!(path, reference["path"].as_str().unwrap());
            assert_eq!(out.hrir.fs as u64, reference["fs"].as_u64().unwrap());
            let refs = reference["tracks"].as_array().unwrap();
            assert_eq!(tracks.len(), refs.len());
            for (track, name) in tracks.iter().zip(refs) {
                summary(path, track, &file(name.as_str().unwrap())["outputs"], 1e-9);
            }
        }
        if compact {
            for (name, order, tracks) in [
                (
                    "hrir.wav",
                    HEXADECAGONAL_TRACK_ORDER.as_slice(),
                    &out.hrir_tracks,
                ),
                (
                    "hesuvi.wav",
                    HESUVI_TRACK_ORDER.as_slice(),
                    &out.hesuvi_tracks,
                ),
            ] {
                let full = out.hrir.stack_tracks(order, false).unwrap();
                let names: Vec<_> = order
                    .iter()
                    .zip(&full)
                    .filter(|(_, row)| row.iter().any(|x| *x != 0.0))
                    .map(|(s, _)| *s)
                    .collect();
                assert_eq!(json!(names), case["compact_names"][name]);
                assert_eq!(tracks.len(), names.len());
                assert!(tracks.len() < order.len());
            }
        }
        assert!(
            out.jamesdsp.as_ref().unwrap()[0]
                .iter()
                .zip(&out.hrir_tracks[0])
                .any(|(a, b)| (a - b).abs() > 1e-3)
        );
    }
}
/// Python exact stage table; core/pipeline.py:424-470; p10_stage_table.
#[test]
fn golden_stage_table_matches_python() {
    let _gate = NumericGate;
    for v in fixture("stage_table").as_array().unwrap() {
        let config: ProcessingConfig = serde_json::from_value(v["config"].clone()).unwrap();
        let rows: Vec<_> = pipeline::stage_table(&config)
            .iter()
            .map(|(k, e, n)| json!([k.key(), e, n]))
            .collect();
        assert_eq!(json!(rows), v["rows"]);
        assert_eq!(
            pipeline::total_steps(&config),
            v["total_steps"].as_u64().unwrap() as usize
        );
    }
}
