#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_dsp::{
    conv, fft,
    pipeline::{PipelineOutputs, StageObserver, StageProgress, run_pipeline},
    stages::room_tuning2::{analysis_len, apply_tuning2, primitives},
};
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{
    BrirError, BrirEvents, Catalog, discovery::discover, estimator::open_estimator,
    inputs::load_inputs, outputs::render_readme, run::run_brir,
};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::IPSILATERAL_PAIRS,
    job::{JobKind, JobStatus},
};
use serde_json::{Value, json};
use std::sync::OnceLock;

struct Silent;
impl BrirEvents for Silent {
    fn step(&mut self, _: &str, _: Value) -> Result<(), BrirError> {
        Ok(())
    }
    fn log(&mut self, _: &str, _: &str, _: Value) {}
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
impl StageObserver for Silent {
    fn on_stage(&mut self, _: StageProgress) {}
    fn check_cancelled(&self) -> Result<(), impulcifer_dsp::DspError> {
        Ok(())
    }
}

fn output(config: ProcessingConfig) -> PipelineOutputs {
    let temp = Temp::demo();
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let inputs = load_inputs(&dir, &estimator, &config, &mut Silent).unwrap();
    run_pipeline(&config, inputs, &mut Silent).unwrap()
}
fn baseline() -> &'static PipelineOutputs {
    static DATA: OnceLock<PipelineOutputs> = OnceLock::new();
    DATA.get_or_init(|| output(ProcessingConfig::default()))
}
fn mixed() -> &'static PipelineOutputs {
    static DATA: OnceLock<PipelineOutputs> = OnceLock::new();
    DATA.get_or_init(|| {
        output(ProcessingConfig {
            room_mode: "tuning2".into(),
            ..Default::default()
        })
    })
}
fn direct(track: &[f64]) -> usize {
    let maximum = track.iter().map(|v| v.abs()).fold(0.0, f64::max);
    let threshold = maximum * 10_f64.powf(-18.0 / 20.0);
    (1..track.len() - 1)
        .find(|&i| {
            track[i].abs() >= threshold
                && track[i].abs() >= track[i - 1].abs()
                && track[i].abs() > track[i + 1].abs()
        })
        .unwrap()
}
fn latency(output: &PipelineOutputs, ms: usize) {
    assert!(output.hesuvi_tracks.iter().flatten().all(|v| v.is_finite()));
    for (i, (b, a)) in baseline()
        .hesuvi_tracks
        .iter()
        .zip(&output.hesuvi_tracks)
        .enumerate()
    {
        if b.iter().all(|v| *v == 0.0) {
            continue;
        }
        let delta = direct(a) as isize - direct(b) as isize;
        assert!(
            (delta - (ms * 48) as isize).abs() <= 5,
            "channel {i}: {delta}, expected {}",
            ms * 48
        );
    }
}
fn logged(config: ProcessingConfig) -> (Temp, Vec<Value>) {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        dir_path: Some(temp.0.to_string_lossy().into_owned()),
        ..config
    };
    let jobs = JobRegistry::new();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            let result = run_brir(&config, &Catalog::english(), ctx)?;
            Ok(json!({"output_path":result.output_path}))
        })
        .unwrap();
    let poll = wait(&jobs, &job.job_id);
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    (temp, poll.events.into_iter().map(|e| e.payload).collect())
}
#[test]
fn config_room_tuning2_defaults_and_validation() {
    let temp = Temp::new();
    let svc = service(&temp, JobRegistry::new());
    let boot = svc.call("bootstrap", vec![]);
    let defaults = serde_json::to_value(ProcessingConfig::default()).unwrap();
    for field in [
        "mode",
        "max_boost",
        "low",
        "high",
        "mimo",
        "fsiso",
        "mimo_low",
        "dip_limit",
        "low_protect",
        "bands",
        "lf_align",
        "mode_decay",
        "hf_phase",
        "high_basis",
        "high_transition",
        "input_hp",
        "mimo_early",
        "mimo_ear_guard",
    ] {
        let field = format!("room_tuning2_{field}");
        assert_eq!(boot["data"]["brir_defaults"][&field], defaults[&field]);
    }
    for (field, value) in [
        ("mode", json!(29)),
        ("high", json!(99)),
        ("low", json!(201)),
        ("max_boost", json!(11)),
        ("mimo", json!("true")),
        ("fsiso", json!(151)),
        ("mimo_low", json!(14)),
        ("dip_limit", json!("true")),
        ("low_protect", json!("true")),
        ("lf_align", json!("true")),
        ("mode_decay", json!("true")),
        ("hf_phase", json!("true")),
        ("high_basis", json!("agreement")),
        ("high_transition", json!(3)),
        ("input_hp", json!(9)),
        ("input_hp", json!(51)),
        ("mimo_early", json!(4.99)),
        ("mimo_early", json!(60.01)),
        ("mimo_early", json!("NaN")),
        ("mimo_ear_guard", json!("true")),
        ("bands", json!(20)),
        ("bands", json!("bad")),
        ("bands", json!("20-30,40-50,60-70,80-90,100-110")),
    ] {
        let field = format!("room_tuning2_{field}");
        let mut request = json!({"dir_path":temp.0,"room_mode":"tuning2","room_range":"legacy"});
        request[&field] = value;
        let result = svc.call("start_brir", vec![request]);
        assert_eq!(result["error"]["code"], "INVALID_REQUEST", "{result}");
        assert_eq!(result["error"]["details"]["field"], field, "{result}");
    }
    let request = json!({"dir_path":temp.0,"room_mode":"tuning2","room_range":"legacy","room_tuning2_mode":60,"room_tuning2_high":300,"room_tuning2_mimo_low":50,"room_tuning2_dip_limit":true,"room_tuning2_low_protect":true,"room_tuning2_bands":"1000-3000, 20-500","room_tuning2_lf_align":true,"room_tuning2_mode_decay":true,"room_tuning2_hf_phase":true,"room_tuning2_high_basis":"lenient","room_tuning2_high_transition":2,"room_tuning2_input_hp":25,"room_tuning2_mimo_early":20,"room_tuning2_mimo_ear_guard":true});
    let result = svc.call("start_brir", vec![request]);
    assert_eq!(result["ok"], true, "{result}");
}
#[test]
fn room_tuning2_runs_on_the_demo() {
    let output = mixed();
    latency(output, 8);
    let report = output.readme.tuning2.as_ref().unwrap();
    assert_eq!(report.latency_samples, 384);
    let band = report.band.as_ref().unwrap();
    assert_eq!((band.low, band.high), (20.0, 500.0));
    for speaker in &report.speakers {
        assert!(!speaker.mismatch && !speaker.weak, "{speaker:?}");
    }
    assert!((output.applied_gain_db - baseline().applied_gain_db).abs() < 6.0);
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        ..Default::default()
    });
    let log = logs
        .iter()
        .find(|l| l["key"] == "cli_room_tuning2")
        .unwrap();
    let expected = Catalog::english().translate(
        "cli_room_tuning2",
        &json!({"mode":"mixed","delay":8,"low":20,"high":500}),
    );
    assert_eq!(log["message"], expected);
    for key in [
        "cli_room_tuning",
        "cli_room_tuning_off",
        "cli_room_tuning_preecho",
        "cli_room_tuning_mismatch",
        "cli_room_tuning_weak",
    ] {
        assert!(!logs.iter().any(|l| l["key"] == key), "{key}");
    }
    let wav = impulcifer_io::read_wav(&temp.0.join("hesuvi.wav")).unwrap();
    assert!(wav.tracks.iter().flatten().all(|v| v.is_finite()));
    assert!(
        std::fs::read_to_string(temp.0.join("README.md"))
            .unwrap()
            .contains("## Virtual room tuning v2")
    );
    let (_, eq_logs) = logged(ProcessingConfig::default());
    assert!(!eq_logs.iter().any(|l| {
        l["key"]
            .as_str()
            .is_some_and(|k| k.starts_with("cli_room_tuning"))
    }));
    // Mixed mode corrects part of the excess phase inside its 8 ms lift, so a
    // speaker may ring before its direct sound; on the demo BL's left ear
    // reaches about -29 dB, just over the -30 dB warning line. The warning is
    // information: it must match the report, and the ringing stays small.
    let mut warned = Vec::new();
    for speaker in &report.speakers {
        for p in &speaker.pre_echo_channels {
            assert!(p.after_db <= -25., "{speaker:?}");
            if p.warns() && !warned.contains(&speaker.speaker) {
                warned.push(speaker.speaker.clone());
            }
        }
    }
    let logged_preecho: Vec<_> = logs
        .iter()
        .filter(|l| l["key"] == "cli_room_tuning2_preecho")
        .map(|l| l["message"].as_str().unwrap())
        .collect();
    assert_eq!(logged_preecho.len(), warned.len(), "{logged_preecho:?}");
    for speaker in &warned {
        assert!(
            logged_preecho
                .iter()
                .any(|m| m.contains(&format!("{speaker}:"))),
            "{speaker}: {logged_preecho:?}"
        );
    }
}
#[test]
fn room_tuning2_new_options_log_on_demo() {
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_dip_limit: true,
        room_tuning2_low_protect: true,
        room_tuning2_bands: "1000-3000".into(),
        ..Default::default()
    });
    assert_eq!(
        logs.iter()
            .filter(|event| event["key"] == "cli_room_tuning2_bands")
            .count(),
        1
    );
    let low: Vec<_> = logs
        .iter()
        .filter(|event| event["key"] == "cli_room_tuning2_low_protect")
        .collect();
    assert_eq!(low.len(), 7);
    assert!(low.iter().all(|event| {
        let message = event["message"].as_str().unwrap();
        message.contains("rolloff") && message.contains("limit satisfied")
    }));
    let dips: Vec<_> = logs
        .iter()
        .filter(|event| event["key"] == "cli_room_tuning2_dip_limit")
        .collect();
    assert!(!dips.is_empty());
    assert!(dips.iter().all(|event| {
        let message = event["message"].as_str().unwrap();
        message.contains("deepest") && message.contains("allowed fill")
    }));
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    for key in [
        "Virtual room tuning 2 correction bands:",
        "low-end protection, rolloff (Hz):",
        "boost limited in",
    ] {
        assert!(readme.contains(key), "{key}: {readme}");
    }
}

#[test]
fn room_tuning2_bundle2_options_log_on_demo() {
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_lf_align: true,
        room_tuning2_mode_decay: true,
        room_tuning2_hf_phase: true,
        ..Default::default()
    });
    assert_eq!(
        logs.iter()
            .filter(|event| event["key"] == "cli_room_tuning2_lf_align")
            .count(),
        7
    );
    assert_eq!(
        logs.iter()
            .filter(|event| {
                event["key"] == "cli_room_tuning2_mode_decay"
                    || event["key"] == "cli_room_tuning2_mode_decay_none"
            })
            .count(),
        7
    );
    assert_eq!(
        logs.iter()
            .filter(|event| {
                event["key"] == "cli_room_tuning2_hf_phase"
                    || event["key"] == "cli_room_tuning2_hf_phase_none"
            })
            .count(),
        7
    );
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    for text in [
        "bass arrival alignment from",
        "mode-decay target",
        "treble phase",
    ] {
        assert!(readme.contains(text), "{text}: {readme}");
    }

    let (_, zero_logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_mode: "zero".into(),
        room_tuning2_lf_align: true,
        room_tuning2_hf_phase: true,
        ..Default::default()
    });
    assert_eq!(
        zero_logs
            .iter()
            .filter(|event| event["key"] == "cli_room_tuning2_zero_inert")
            .count(),
        1
    );
    assert!(!zero_logs.iter().any(|event| {
        event["key"] == "cli_room_tuning2_lf_align" || event["key"] == "cli_room_tuning2_hf_phase"
    }));
}

#[test]
fn room_tuning2_bundle3_options_and_reports_log_on_demo() {
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_volume: Some(95.0),
        room_tuning2_high_basis: "lenient".into(),
        room_tuning2_high_transition: 2,
        room_tuning2_input_hp: "25".into(),
        ..Default::default()
    });
    assert_eq!(
        logs.iter()
            .filter(|event| event["key"] == "cli_room_tuning2_high")
            .count(),
        1
    );
    assert_eq!(
        logs.iter()
            .filter(|event| event["key"] == "cli_room_tuning2_input_hp")
            .count(),
        1
    );
    let pair_count = logs
        .iter()
        .filter(|event| event["key"] == "cli_room_tuning2_pair_phase")
        .count();
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    assert!(readme.contains("automatic upper limit"), "{readme}");
    assert!(readme.contains("input high-pass"), "{readme}");
    assert_eq!(pair_count > 0, readme.contains("### Pair phase"));

    let (_, no_volume_logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_high_basis: "lenient".into(),
        ..Default::default()
    });
    assert_eq!(
        no_volume_logs
            .iter()
            .filter(|event| event["key"] == "cli_room_tuning2_high_no_volume")
            .count(),
        1
    );
}

#[test]
fn room_tuning2_bundle3_volume_evidence_on_demo() {
    let mut strict = None;
    for basis in ["strict", "lenient"] {
        for transition in [1_u32, 2, 4] {
            let output = output(ProcessingConfig {
                room_mode: "tuning2".into(),
                room_volume: Some(95.0),
                room_tuning2_high_basis: basis.into(),
                room_tuning2_high_transition: transition,
                ..Default::default()
            });
            let band = output
                .readme
                .tuning2
                .as_ref()
                .unwrap()
                .band
                .as_ref()
                .unwrap();
            strict.get_or_insert(band.schroeder.unwrap());
            println!(
                "DEMO UPPER basis={basis} factor={transition} strict={:.17} lenient={:?} t60={:?} high={:.17}",
                band.schroeder.unwrap(),
                band.lenient,
                band.lenient_t60_s,
                band.high
            );
            if basis == "strict" {
                assert_eq!(band.lenient, None);
            } else {
                assert_eq!(band.lenient, Some(125.0));
                assert!(band.lenient_t60_s.is_some());
            }
        }
    }
    assert!(strict.unwrap() > 119.0 && strict.unwrap() < 120.0);
}

#[test]
fn room_tuning2_no_measurements_service_path_applies_exact_input_high_pass() {
    let temp = Temp::demo();
    for entry in std::fs::read_dir(&temp.0).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name.starts_with("room-") && path.extension().is_some_and(|value| value == "wav") {
            std::fs::remove_file(path).unwrap();
        }
    }
    let config = ProcessingConfig {
        dir_path: Some(temp.0.to_string_lossy().into_owned()),
        room_mode: "tuning2".into(),
        room_tuning2_input_hp: "25".into(),
        do_headphone_compensation: false,
        do_equalization: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let inputs = load_inputs(&dir, &estimator, &config, &mut Silent).unwrap();
    let plan = inputs.room.unwrap().tuning2.unwrap();
    assert!(plan.speakers.is_empty());
    assert_eq!(plan.options.input_hp, Some(25.0));

    let mut actual = inputs.hrir;
    actual.crop_heads(config.head_ms).unwrap();
    actual.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
    actual.align_onset_groups_peak_leftref(None).unwrap();
    actual.crop_tails(&estimator).unwrap();
    let mut expected = actual.clone();
    let n = analysis_len(expected.fs);
    let filter = fft::irfft(
        &primitives::hp8(&primitives::grid(n, expected.fs as f64), 25.0),
        n,
    );
    let fade_len = (0.005 * expected.fs as f64).round() as usize;
    for ir in expected
        .speakers
        .iter_mut()
        .flat_map(|speaker| speaker.left.iter_mut().chain(&mut speaker.right))
    {
        let keep = ir.len();
        let mut data = conv::convolve(&ir.data, &filter, conv::Mode::Full);
        data.truncate(keep);
        let start = data.len().saturating_sub(fade_len);
        for (index, value) in data[start..].iter_mut().enumerate() {
            *value *= 0.5 * (1.0 + (std::f64::consts::PI * index as f64 / fade_len as f64).cos());
        }
        ir.data = data;
    }

    let report = apply_tuning2(&mut actual, &plan).unwrap();
    assert!(report.speakers.is_empty());
    assert_eq!(actual.speakers.len(), expected.speakers.len());
    for (actual, expected) in actual.speakers.iter().zip(&expected.speakers) {
        assert_eq!(actual.speaker, expected.speaker);
        for (actual, expected) in actual
            .left
            .iter()
            .chain(&actual.right)
            .zip(expected.left.iter().chain(&expected.right))
        {
            assert_eq!(actual.data, expected.data);
        }
    }
}

#[test]
fn room_tuning2_modes_on_the_demo() {
    for (mode, ms) in [("zero", 0), ("linear", 28), ("60", 58)] {
        let output = output(ProcessingConfig {
            room_mode: "tuning2".into(),
            room_tuning2_mode: mode.into(),
            ..Default::default()
        });
        latency(&output, ms);
        assert_eq!(
            output.readme.tuning2.as_ref().unwrap().latency_samples,
            ms * 48
        );
    }
}
#[test]
fn room_tuning2_ignores_v1_options() {
    let changed = output(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning_delay: "20".into(),
        room_tuning_curtain: 5000.0,
        room_tuning_phase_limit: "off".into(),
        room_tuning_max_boost: 12.0,
        ..Default::default()
    });
    assert_eq!(changed.hesuvi_tracks, mixed().hesuvi_tracks);
    let temp = Temp::new();
    for (name, tracks) in [
        ("base", &mixed().hesuvi_tracks),
        ("changed", &changed.hesuvi_tracks),
    ] {
        impulcifer_io::write_wav(&temp.0.join(name), 48000, tracks, 32).unwrap();
    }
    assert_eq!(
        std::fs::read(temp.0.join("base")).unwrap(),
        std::fs::read(temp.0.join("changed")).unwrap()
    );
}
#[test]
fn room_tuning2_level_match_off() {
    let out = output(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning_level_match: false,
        ..Default::default()
    });
    let report = out.readme.tuning2.as_ref().unwrap();
    let readme = render_readme(&out.readme, &Catalog::english(), "test");
    for s in &report.speakers {
        assert_eq!(s.trim_db, 0.0);
        assert_eq!(s.trim_source, "off");
        assert!(readme.contains(&format!(
            "| {} | {} | {:.2} | 0.00 | off |",
            s.speaker, s.points, s.level_db
        )));
    }
}
#[test]
fn room_tuning2_mimo_on_the_demo() {
    let out = output(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_mimo: true,
        ..Default::default()
    });
    let report = out.readme.tuning2.as_ref().unwrap();
    let mimo = report.mimo.as_ref().unwrap();
    let mut supported = 0;
    // Undo final normalization before comparing the fixed late windows.
    for input in &mimo.inputs {
        if input.supports.is_empty() {
            continue;
        }
        supported += 1;
        let mut energies = [0.0, 0.0];
        for (i, track) in impulcifer_types::constants::HESUVI_TRACK_ORDER
            .iter()
            .enumerate()
        {
            if track.split('-').next() != Some(input.speaker.as_str()) {
                continue;
            }
            let before = &mixed().hesuvi_tracks[i];
            let start = (primitives::onset(before) + 2880).min(before.len());
            for (n, output) in [mixed(), &out].iter().enumerate() {
                let gain = 10_f64.powf(-output.applied_gain_db / 20.0);
                let mut data: Vec<_> = output.hesuvi_tracks[i].iter().map(|x| x * gain).collect();
                data[..start].fill(0.0);
                let spectrum = primitives::rfft(&data, 131072);
                energies[n] += spectrum
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| (20.0..=150.0).contains(&(*k as f64 * 48000.0 / 131072.0)))
                    .map(|(_, v)| v.norm_sqr())
                    .sum::<f64>();
            }
        }
        let db = 10.0 * (energies[1] / energies[0]).log10();
        println!("LATE {} {db:.6} dB", input.speaker);
        assert!(db < 0.0, "{}: {db}", input.speaker);
    }
    assert!(supported > 0);
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_mimo: true,
        ..Default::default()
    });
    assert_eq!(
        logs.iter()
            .filter(
                |l| l["key"] == "cli_room_tuning2_mimo" || l["key"] == "cli_room_tuning2_mimo_off"
            )
            .count(),
        report.speakers.len()
    );
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    assert!(readme.contains("### MIMO supports"));
    assert!(readme.contains("| Input | Supports | Support level (dB) | Guard |"));
}

#[test]
fn room_tuning2_mimo_bundle4_options_are_inert_when_mimo_is_off() {
    let baseline = output(ProcessingConfig {
        room_mode: "tuning2".into(),
        ..Default::default()
    });
    let configured = output(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_mimo_early: 20.0,
        room_tuning2_mimo_ear_guard: true,
        ..Default::default()
    });
    assert_eq!(baseline.hesuvi_tracks, configured.hesuvi_tracks);
    assert_eq!(baseline.applied_gain_db, configured.applied_gain_db);
}

#[test]
fn room_tuning2_mimo_bundle4_options_log_on_demo() {
    let (temp, logs) = logged(ProcessingConfig {
        room_mode: "tuning2".into(),
        room_tuning2_mimo: true,
        room_tuning2_mimo_early: 20.0,
        room_tuning2_mimo_ear_guard: true,
        ..Default::default()
    });
    assert_eq!(
        logs.iter()
            .filter(|entry| entry["key"] == "cli_room_tuning2_mimo_early")
            .count(),
        1
    );
    assert!(logs.iter().any(|entry| {
        matches!(
            entry["key"].as_str(),
            Some("cli_room_tuning2_mimo_ear_band" | "cli_room_tuning2_mimo_ear_tail")
        )
    }));
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    assert!(readme.contains("Early window: 20.0 ms. Ear guard: on."));
}

#[test]
fn room_tuning2_mimo_bundle4_changes_demo_guard_scales() {
    let run = |early, ear_guard| {
        output(ProcessingConfig {
            room_mode: "tuning2".into(),
            room_tuning2_mimo: true,
            room_tuning2_fsiso: 120.0,
            room_tuning2_mimo_early: early,
            room_tuning2_mimo_ear_guard: ear_guard,
            ..Default::default()
        })
        .readme
        .tuning2
        .unwrap()
        .mimo
        .unwrap()
    };
    let default = run(60.0, false);
    let early = run(20.0, false);
    let guarded = run(60.0, true);
    let mut early_changes = 0;
    let mut guard_changes = 0;
    for index in 0..default.inputs.len() {
        let baseline = &default.inputs[index];
        let changed = &early.inputs[index];
        let guarded = &guarded.inputs[index];
        assert_eq!(baseline.speaker, changed.speaker);
        assert_eq!(baseline.speaker, guarded.speaker);
        early_changes += usize::from(baseline.scale_db != changed.scale_db);
        guard_changes += usize::from(baseline.scale_db != guarded.scale_db);
        println!(
            "MIMO_DEMO {} default={:?} early20={:?} ear_guard={:?} ear_limit={:?}",
            baseline.speaker,
            baseline.scale_db,
            changed.scale_db,
            guarded.scale_db,
            guarded.ear_limit
        );
    }
    assert!(early_changes > 0, "20 ms must exercise a different scale");
    assert!(
        guard_changes > 0,
        "the ear guard must exercise a different scale"
    );
}
